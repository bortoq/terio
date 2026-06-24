// Ask flow: matcher → cache → agent → execute → cache

use crate::agent::{get_mock_plan, plan_to_steps};
use crate::cache::ScriptCache;
use crate::identity::Identity;
use crate::log::{LogReader, LogStore};
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
    },
    /// Cache miss: mock agent → execute → save.
    FromAgent {
        entry: crate::cache::CacheEntry,
        results: Vec<CommandResult>,
        total_duration: std::time::Duration,
    },
    /// Неизвестный запрос (нет mock).
    Unknown,
}

/// Обрабатывает запрос: exact cache → miss → mock agent.
pub fn process_request(
    request: &str,
    identity: &Identity,
    log_store: &LogStore,
    cache: &ScriptCache,
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

        // Логируем script_run
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

        // Увеличиваем success_count
        cache.increment_success(&entry.request_hash)?;

        return Ok(AskResult::CacheHit {
            entry,
            results,
            total_duration,
        });
    }

    // 2. Кеш промах — пробуем mock agent
    let plan = get_mock_plan(request);
    let plan = match plan {
        Some(p) => p,
        None => return Ok(AskResult::Unknown),
    };

    let steps = plan_to_steps(&plan);

    // 3. Выполняем команды
    let mut results = Vec::new();
    let mut total_duration = std::time::Duration::default();

    for cmd in &plan.commands {
        let result = run::execute(&cmd.argv)?;
        total_duration += result.duration;
        results.push(result);
    }

    // 4. Сохраняем в кеш
    let entry = cache.save(request, plan.risk.clone(), steps)?;

    // 5. Логируем agent_turn + command_run
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
    })
}

fn log_agent_turn(
    identity: &Identity,
    interaction_id: &str,
    request: &str,
    cwd: &str,
    plan: &crate::agent::AgentPlan,
    store: &LogStore,
) -> Result<()> {
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
        prompt_summary: Some(format!("mock: {}", redact(request))),
        plan: Some(serde_json::to_value(&plan.commands).unwrap_or_default()),
        model_provider: Some("mock".to_string()),
        model_name: Some("mock-agent-v1".to_string()),
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
        status: Some(LogStatus::Success),
        failure_kind: None,
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
        tokens_saved_estimate: Some(50), // estimate for mock agent
        success_count_before: Some((cache_entry.success_count - 1) as u64),
        success_count_after: Some(cache_entry.success_count as u64),
        steps: Some(steps),
        description: None,
    };
    store.append(entry)?;
    Ok(())
}

/// Возвращает statistics на основе записей лога.
pub fn compute_stats(reader: &dyn LogReader) -> Result<Stats> {
    let entries = reader.recent(1000)?;
    let mut stats = Stats::default();

    for entry in &entries {
        match entry.kind {
            LogKind::AgentTurn => {
                stats.model_calls += 1;
            }
            LogKind::ScriptRun => {
                stats.cache_hits += 1;
            }
            LogKind::CommandRun => {
                // считаем executed commands
            }
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

        // agent_turn — используем command_run как базу, меняем kind
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

        // script_run
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
}
