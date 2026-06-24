// Ask flow: cache → provider → confirm → execute → cache

use crate::agent::{plan_to_steps, AgentPlan};
use crate::cache::ScriptCache;
use crate::config::Config;
use crate::identity::Identity;
use crate::log::{LogReader, LogStore};
use crate::provider::{needs_confirmation, Provider};
use crate::redact::redact;
use crate::run::{self, CommandResult};
use crate::types::*;
use anyhow::Result;
use serde::Serialize;

/// Результат обработки запроса.
pub enum AskResult {
    /// Cache hit: выполнен из кеша.
    CacheHit {
        entry: crate::cache::CacheEntry,
        results: Vec<CommandResult>,
        total_duration: std::time::Duration,
        all_exit_zero: bool,
    },
    /// Cache miss: provider generated plan, commands executed.
    FromAgent {
        entry: Option<crate::cache::CacheEntry>,
        results: Vec<CommandResult>,
        total_duration: std::time::Duration,
        all_exit_zero: bool,
        plan: AgentPlan,
    },
    /// Provider not available (no mock match, no real provider configured).
    Unknown,
    /// User declined confirmation.
    Declined,
}

/// Обрабатывает запрос: cache → provider → confirm → execute → save.
pub fn process_request(
    request: &str,
    identity: &Identity,
    log_store: &LogStore,
    cache: &ScriptCache,
    provider: &dyn Provider,
    skip_confirm: bool,
) -> Result<AskResult> {
    // 1. Поиск в кеше
    if let Some(entry) = cache.lookup(request)? {
        // Выполняем шаги из кеша
        let mut results = Vec::new();
        let mut total_duration = std::time::Duration::default();

        for step in &entry.steps {
            let result = run::execute(&step.argv)?;
            total_duration += result.duration;
            results.push(result);
        }

        let all_exit_zero = results.iter().all(|r| r.exit_code == 0);
        let interaction_id = Identity::new_interaction_id();
        let cwd = std::env::current_dir()?.to_string_lossy().to_string();

        log_script_run(
            identity,
            &interaction_id,
            request,
            &cwd,
            &entry,
            &results,
            total_duration,
            log_store,
        )?;

        if all_exit_zero {
            cache.increment_success(&entry.request_hash)?;
        }

        return Ok(AskResult::CacheHit {
            entry,
            results,
            total_duration,
            all_exit_zero,
        });
    }

    // 2. Кеш промах — используем provider
    let plan = match provider.plan(request) {
        Ok(p) => p,
        Err(_) => return Ok(AskResult::Unknown),
    };

    // 3. Запрашиваем подтверждение если нужно
    if !skip_confirm && needs_confirmation(&plan) {
        eprintln!("⚠️  План требует подтверждения (risk: {:?})", plan.risk);
        eprintln!("   {}", plan.summary);
        for cmd in &plan.commands {
            eprintln!(
                "   > {} (risk: {:?}): {}",
                cmd.argv.join(" "),
                cmd.risk,
                cmd.reason
            );
        }
        eprint!("Подтвердить? [y/N] ");
        use std::io::Write;
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            return Ok(AskResult::Declined);
        }
    }

    let steps = plan_to_steps(&plan);

    // 4. Выполняем команды
    let mut results = Vec::new();
    let mut total_duration = std::time::Duration::default();
    let mut all_exit_zero = true;

    for cmd in &plan.commands {
        let result = run::execute(&cmd.argv)?;
        all_exit_zero = all_exit_zero && (result.exit_code == 0);
        total_duration += result.duration;
        results.push(result);
    }

    // 5. Сохраняем в кеш только если все команды успешны
    let entry = if all_exit_zero {
        Some(cache.save(request, plan.risk.clone(), steps)?)
    } else {
        None
    };

    // 6. Логируем agent_turn + command_run
    let interaction_id = Identity::new_interaction_id();
    let cwd = std::env::current_dir()?.to_string_lossy().to_string();

    log_agent_turn(identity, &interaction_id, request, &cwd, &plan, log_store)?;
    for (i, cmd) in plan.commands.iter().enumerate() {
        if i < results.len() {
            log_command_run(
                identity,
                &interaction_id,
                request,
                &cwd,
                &cmd.argv,
                &results[i],
                log_store,
            )?;
        }
    }

    Ok(AskResult::FromAgent {
        entry,
        results,
        total_duration,
        all_exit_zero,
        plan,
    })
}

fn log_agent_turn(
    identity: &Identity,
    interaction_id: &str,
    request: &str,
    cwd: &str,
    plan: &AgentPlan,
    store: &LogStore,
) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let provider_name = format!("{:?}", config.provider.provider_type).to_lowercase();

    let entry = LogEntry {
        schema_version: 1,
        instance_id: identity.instance_id.clone(),
        session_id: identity.session_id.clone(),
        ts: chrono::Utc::now().to_rfc3339(),
        interaction_id: Some(interaction_id.to_string()),
        parent_interaction_id: None,
        kind: LogKind::AgentTurn,
        display_profile: DisplayProfile::default(),
        cost_counters: CostCounters::default(),
        request: Some(redact(request)),
        cwd: Some(redact(cwd)),
        risk: Some(plan.risk.clone()),
        status: Some(LogStatus::Success),
        failure_kind: None,
        prompt_summary: Some(format!("{}: {}", provider_name, redact(request))),
        plan: Some(serde_json::to_value(&plan.commands).unwrap_or_default()),
        model_provider: Some(provider_name),
        model_name: Some(config.provider.model.clone().unwrap_or_default()),
        duration_ms: Some(0),
        tokens_used: Some(0),
        command: None,
        exit: None,
        stdout_summary: None,
        stderr_summary: None,
        script_id: None,
        cache_hit: None,
        model_called: Some(true),
        tokens_saved_estimate: None,
        success_count_before: None,
        success_count_after: None,
        steps: None,
        description: None,
    };
    store.append(entry)?;
    Ok(())
}

fn log_command_run(
    identity: &Identity,
    interaction_id: &str,
    request: &str,
    cwd: &str,
    argv: &[String],
    result: &CommandResult,
    store: &LogStore,
) -> Result<()> {
    let entry = run::make_command_run_entry(
        &identity.instance_id,
        &identity.session_id,
        Some(interaction_id.to_string()),
        request,
        cwd,
        argv,
        result,
    );
    store.append(entry)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn log_script_run(
    identity: &Identity,
    interaction_id: &str,
    request: &str,
    cwd: &str,
    cache_entry: &crate::cache::CacheEntry,
    results: &[CommandResult],
    total_duration: std::time::Duration,
    store: &LogStore,
) -> Result<()> {
    let all_exit_zero = results.iter().all(|r| r.exit_code == 0);

    let steps: Vec<StepInfo> = results
        .iter()
        .zip(cache_entry.steps.iter())
        .map(|(r, s)| StepInfo {
            command: s.command.clone(),
            argv: s.argv.clone(),
            exit: r.exit_code,
        })
        .collect();

    let total_bytes: u64 = results
        .iter()
        .map(|r| r.stdout.len() as u64 + r.stderr.len() as u64)
        .sum();

    let success_before = cache_entry.success_count as u64;
    let success_after = if all_exit_zero {
        success_before + 1
    } else {
        success_before
    };

    let status = if all_exit_zero {
        LogStatus::Success
    } else {
        LogStatus::Failed
    };

    let failure_kind = if all_exit_zero {
        None
    } else {
        Some("command_exit".to_string())
    };

    let entry = LogEntry {
        schema_version: 1,
        instance_id: identity.instance_id.clone(),
        session_id: identity.session_id.clone(),
        ts: chrono::Utc::now().to_rfc3339(),
        interaction_id: Some(interaction_id.to_string()),
        parent_interaction_id: None,
        kind: LogKind::ScriptRun,
        display_profile: DisplayProfile::default(),
        cost_counters: CostCounters {
            execution_cost: ExecutionCost {
                duration_ms: total_duration.as_millis() as u64,
                commands_executed: results.len() as u64,
                bytes_read: total_bytes,
                bytes_written: 0,
            },
            cache_cost: CacheCost {
                lookup_ms: 0,
                hit: true,
            },
            ..CostCounters::default()
        },
        request: Some(redact(request)),
        cwd: Some(redact(cwd)),
        risk: Some(cache_entry.risk.clone()),
        status: Some(status),
        failure_kind,
        prompt_summary: None,
        plan: None,
        model_provider: None,
        model_name: None,
        duration_ms: Some(total_duration.as_millis() as u64),
        tokens_used: None,
        command: None,
        exit: None,
        stdout_summary: None,
        stderr_summary: None,
        script_id: Some(cache_entry.script_id.clone()),
        cache_hit: Some(true),
        model_called: Some(false),
        tokens_saved_estimate: Some(50),
        success_count_before: Some(success_before),
        success_count_after: Some(success_after),
        steps: Some(steps),
        description: None,
    };
    store.append(entry)?;
    Ok(())
}

/// Возвращает statistics на основе записей лога.
pub fn compute_stats(reader: &dyn LogReader) -> Result<Stats> {
    let entries = reader.recent(usize::MAX)?;
    let mut stats = Stats::default();

    for entry in &entries {
        match entry.kind {
            LogKind::AgentTurn => {
                stats.model_calls += 1;
            }
            LogKind::ScriptRun => {
                stats.cache_hits += 1;
            }
            LogKind::CommandRun => {}
            _ => {}
        }

        let cc = &entry.cost_counters;
        stats.total_tokens += cc.llm_cost.tokens;
        stats.total_duration_ms += cc.execution_cost.duration_ms;
        stats.total_commands += cc.execution_cost.commands_executed;
    }

    Ok(stats)
}

#[derive(Debug, Default, Serialize)]
pub struct Stats {
    pub model_calls: u64,
    pub cache_hits: u64,
    pub total_tokens: u64,
    pub total_duration_ms: u64,
    pub total_commands: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{CacheEntry, CachedStep};
    use crate::log::reader::JsonlLogReader;
    use crate::log::writer::JsonlLogWriter;
    use crate::log::LogWriter;
    use tempfile::TempDir;

    #[test]
    fn test_compute_stats_empty() {
        let dir = TempDir::new().unwrap();
        let reader = JsonlLogReader::new(dir.path());
        let stats = compute_stats(&reader).unwrap();
        assert_eq!(stats.model_calls, 0);
        assert_eq!(stats.cache_hits, 0);
    }

    #[test]
    fn test_compute_stats_with_entries() {
        let dir = TempDir::new().unwrap();
        let writer = JsonlLogWriter::new(dir.path()).unwrap();

        let mut entry = LogEntry::new_command_run(
            "i1",
            "s1",
            Some("int1".into()),
            "test",
            "/tmp",
            &["echo".into(), "hi".into()],
            0,
            std::time::Duration::from_millis(1),
            "hi",
            "",
            CostCounters::default(),
        );
        entry.kind = LogKind::AgentTurn;
        entry.cost_counters.llm_cost.tokens = 100;
        writer.append(entry).unwrap();

        let mut entry = LogEntry::new_command_run(
            "i1",
            "s1",
            Some("int2".into()),
            "test",
            "/tmp",
            &["echo".into(), "hi".into()],
            0,
            std::time::Duration::from_millis(1),
            "hi",
            "",
            CostCounters::default(),
        );
        entry.kind = LogKind::ScriptRun;
        entry.cost_counters.execution_cost.duration_ms = 50;
        writer.append(entry).unwrap();
        writer.flush().unwrap();

        let reader = JsonlLogReader::new(dir.path());
        let stats = compute_stats(&reader).unwrap();
        assert_eq!(stats.model_calls, 1);
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.total_tokens, 100);
    }

    #[test]
    fn test_log_script_run_failed_command() {
        let dir = TempDir::new().unwrap();
        let writer = Box::new(JsonlLogWriter::new(dir.path()).unwrap());
        let reader = Box::new(JsonlLogReader::new(dir.path()));
        let store = LogStore::new(writer, reader, 16);
        let identity = Identity::load_or_create().unwrap();

        let cache_entry = CacheEntry {
            schema_version: 1,
            script_id: "s1".to_string(),
            request_hash: "h1".to_string(),
            version: 1,
            normalized_request: "test".to_string(),
            match_policy: "exact_normalized".to_string(),
            scope: crate::cache::ScriptScope {
                cwd_policy: "same_cwd_only".to_string(),
                cwd: "/tmp".to_string(),
            },
            risk: RiskLevel::ReadOnly,
            parameters: serde_json::json!({}),
            preconditions: vec![],
            steps: vec![CachedStep {
                command: "sh".to_string(),
                argv: vec!["sh".to_string(), "-c".to_string(), "exit 1".to_string()],
                risk: RiskLevel::ReadOnly,
            }],
            artifacts: vec![],
            success_count: 5,
            trust_threshold: 3,
            created_at: "now".to_string(),
            last_used_at: "now".to_string(),
        };

        let results = vec![CommandResult {
            exit_code: 1,
            stdout: String::new(),
            stderr: "error".to_string(),
            duration: std::time::Duration::from_millis(1),
        }];

        log_script_run(
            &identity,
            "int1",
            "test",
            "/tmp",
            &cache_entry,
            &results,
            std::time::Duration::from_millis(1),
            &store,
        )
        .unwrap();

        let recent = store.recent(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].status, Some(LogStatus::Failed));
        assert_eq!(recent[0].failure_kind, Some("command_exit".to_string()));
        assert_eq!(recent[0].success_count_before, Some(5));
        assert_eq!(recent[0].success_count_after, Some(5));
    }

    #[test]
    fn test_log_script_run_success() {
        let dir = TempDir::new().unwrap();
        let writer = Box::new(JsonlLogWriter::new(dir.path()).unwrap());
        let reader = Box::new(JsonlLogReader::new(dir.path()));
        let store = LogStore::new(writer, reader, 16);
        let identity = Identity::load_or_create().unwrap();

        let cache_entry = CacheEntry {
            schema_version: 1,
            script_id: "s1".to_string(),
            request_hash: "h1".to_string(),
            version: 1,
            normalized_request: "test".to_string(),
            match_policy: "exact_normalized".to_string(),
            scope: crate::cache::ScriptScope {
                cwd_policy: "same_cwd_only".to_string(),
                cwd: "/tmp".to_string(),
            },
            risk: RiskLevel::ReadOnly,
            parameters: serde_json::json!({}),
            preconditions: vec![],
            steps: vec![CachedStep {
                command: "echo".to_string(),
                argv: vec!["echo".to_string(), "ok".to_string()],
                risk: RiskLevel::ReadOnly,
            }],
            artifacts: vec![],
            success_count: 5,
            trust_threshold: 3,
            created_at: "now".to_string(),
            last_used_at: "now".to_string(),
        };

        let results = vec![CommandResult {
            exit_code: 0,
            stdout: "ok".to_string(),
            stderr: String::new(),
            duration: std::time::Duration::from_millis(1),
        }];

        log_script_run(
            &identity,
            "int1",
            "test",
            "/tmp",
            &cache_entry,
            &results,
            std::time::Duration::from_millis(1),
            &store,
        )
        .unwrap();

        let recent = store.recent(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].status, Some(LogStatus::Success));
        assert_eq!(recent[0].failure_kind, None);
        assert_eq!(recent[0].success_count_before, Some(5));
        assert_eq!(recent[0].success_count_after, Some(6));
    }
}
