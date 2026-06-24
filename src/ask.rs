// Ask flow: cache → provider → confirm → execute → cache

use crate::agent::{plan_to_steps, AgentPlan};
use crate::cache::ScriptCache;
use crate::config::Config;
use crate::identity::Identity;
use crate::log::{LogReader, LogStore};
use crate::provider::{needs_confirmation, Provider};
use crate::redact::redact;
use crate::run::{self, CommandResult};
use crate::trust::{evaluate_cache_entry, trust_level_str, validate_step_paths, TrustEvaluation};
use crate::types::*;
use anyhow::{bail, Result};
use serde::Serialize;
use std::path::PathBuf;

/// Результат обработки запроса.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub enum PendingSource {
    Cache,
    Agent,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct PendingCommand {
    pub argv: Vec<String>,
    pub risk: RiskLevel,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct PendingPlanSummary {
    pub request: String,
    pub summary: String,
    pub risk: RiskLevel,
    pub requires_confirmation: bool,
    pub trust: Option<TrustEvaluation>,
    pub commands: Vec<PendingCommand>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct PendingConfirmationState {
    pub source: PendingSource,
    pub plan_summary: PendingPlanSummary,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub enum PendingExecutionPayload {
    Cache {
        entry: Box<crate::cache::CacheEntry>,
    },
    Agent {
        plan: Box<AgentPlan>,
    },
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct PendingExecutionState {
    pub request: String,
    pub payload: PendingExecutionPayload,
}

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
    PendingConfirmation {
        source: PendingSource,
        plan_summary: PendingPlanSummary,
        execution: PendingExecutionState,
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
    let config = Config::load().unwrap_or_default();
    let cwd = std::env::current_dir()?.to_string_lossy().to_string();

    // 1. Поиск в кеше
    if let Some(entry) = cache.lookup(request)? {
        let evaluation = evaluate_cache_entry(&entry, &config, &cwd)?;
        if !evaluation.scope_ok || !evaluation.path_boundary_ok {
            return Ok(AskResult::Declined);
        }
        if !skip_confirm && evaluation.requires_confirmation {
            return Ok(AskResult::PendingConfirmation {
                source: PendingSource::Cache,
                plan_summary: pending_summary_from_cache(request, &entry, evaluation),
                execution: PendingExecutionState {
                    request: request.to_string(),
                    payload: PendingExecutionPayload::Cache {
                        entry: Box::new(entry),
                    },
                },
            });
        }

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
    let (plan, has_unknown_commands) = match recompute_plan_safety(plan) {
        Ok(v) => v,
        Err(_) => return Ok(AskResult::Declined),
    };

    let steps = plan_to_steps(&plan);

    if validate_step_paths(&steps, &cwd).is_err() {
        return Ok(AskResult::Declined);
    }

    // 3. Подтверждение для плана от провайдера
    let requires_confirmation = needs_confirmation(&plan)
        || matches!(plan.risk, RiskLevel::LocalWrite)
        || has_unknown_commands;
    if has_unknown_commands || (!skip_confirm && requires_confirmation) {
        return Ok(AskResult::PendingConfirmation {
            source: PendingSource::Agent,
            plan_summary: pending_summary_from_plan(request, &plan),
            execution: PendingExecutionState {
                request: request.to_string(),
                payload: PendingExecutionPayload::Agent {
                    plan: Box::new(plan.clone()),
                },
            },
        });
    }

    // 4. Выполняем команды
    let (results, total_duration, all_exit_zero) = execute_plan_commands(&plan.commands)?;

    // 5. Сохраняем в кеш только если все команды успешны
    let entry = if all_exit_zero && !ScriptCache::contains_sensitive_data(request, &steps) {
        Some(cache.save(request, plan.risk.clone(), steps)?)
    } else {
        None
    };

    // 6. Логируем agent_turn + command_run
    let interaction_id = Identity::new_interaction_id();

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

fn execute_plan_commands(
    commands: &[crate::agent::AgentCommand],
) -> Result<(Vec<CommandResult>, std::time::Duration, bool)> {
    let mut results = Vec::new();
    let mut total_duration = std::time::Duration::default();
    let mut all_exit_zero = true;

    for cmd in commands {
        let result = run::execute(&cmd.argv)?;
        all_exit_zero = all_exit_zero && (result.exit_code == 0);
        total_duration += result.duration;
        results.push(result);
    }

    Ok((results, total_duration, all_exit_zero))
}

fn pending_summary_from_cache(
    request: &str,
    entry: &crate::cache::CacheEntry,
    evaluation: TrustEvaluation,
) -> PendingPlanSummary {
    PendingPlanSummary {
        request: request.to_string(),
        summary: format!(
            "Cached script · trust {} · {}",
            trust_level_str(entry.success_count, entry.trust_threshold),
            entry.normalized_request
        ),
        risk: entry.risk.clone(),
        requires_confirmation: evaluation.requires_confirmation,
        trust: Some(evaluation),
        commands: entry
            .steps
            .iter()
            .map(|step| PendingCommand {
                argv: step.argv.clone(),
                risk: step.risk.clone(),
                reason: "cached step".to_string(),
            })
            .collect(),
    }
}

fn pending_summary_from_plan(request: &str, plan: &AgentPlan) -> PendingPlanSummary {
    PendingPlanSummary {
        request: request.to_string(),
        summary: plan.summary.clone(),
        risk: plan.risk.clone(),
        requires_confirmation: true,
        trust: None,
        commands: plan
            .commands
            .iter()
            .map(|cmd| PendingCommand {
                argv: cmd.argv.clone(),
                risk: cmd.risk.clone(),
                reason: cmd.reason.clone(),
            })
            .collect(),
    }
}

fn recompute_plan_safety(mut plan: AgentPlan) -> Result<(AgentPlan, bool)> {
    let mut has_unknown_commands = false;
    let mut overall_risk = plan.risk.clone();

    for cmd in &mut plan.commands {
        let executable = cmd
            .argv
            .first()
            .ok_or_else(|| anyhow::anyhow!("provider returned empty argv"))?;
        if executable != &cmd.command {
            bail!("provider command/argv mismatch");
        }

        if !run::is_known_command(executable) {
            has_unknown_commands = true;
        }

        let computed = run::compute_risk(executable, &cmd.argv[1..]);
        if risk_rank(&computed) > risk_rank(&cmd.risk) {
            cmd.risk = computed.clone();
        }
        if risk_rank(&cmd.risk) > risk_rank(&overall_risk) {
            overall_risk = cmd.risk.clone();
        }
    }

    if risk_rank(&overall_risk) > risk_rank(&plan.risk) {
        plan.risk = overall_risk;
    }

    Ok((plan, has_unknown_commands))
}

fn risk_rank(risk: &RiskLevel) -> u8 {
    match risk {
        RiskLevel::ReadOnly => 0,
        RiskLevel::LocalWrite => 1,
        RiskLevel::NetworkRead => 2,
        RiskLevel::CredentialAccess => 3,
        RiskLevel::NetworkWrite => 4,
        RiskLevel::Financial => 5,
        RiskLevel::Destructive => 6,
    }
}

fn pending_state_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)?;
    Ok(home.join(".terio").join("pending-plan.json"))
}

fn pending_exec_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)?;
    Ok(home.join(".terio").join("pending-exec.json"))
}

pub fn save_pending_confirmation(
    state: &PendingConfirmationState,
    execution: &PendingExecutionState,
) -> Result<()> {
    let path = pending_state_path()?;
    let exec_path = pending_exec_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut state = state.clone();
    state.plan_summary.request = redact(&state.plan_summary.request);
    state.plan_summary.summary = redact(&state.plan_summary.summary);
    for cmd in &mut state.plan_summary.commands {
        cmd.argv = cmd.argv.iter().map(|a| redact(a)).collect();
        cmd.reason = redact(&cmd.reason);
    }
    std::fs::write(&path, serde_json::to_string_pretty(&state)?)?;
    write_private_file(&exec_path, &serde_json::to_string_pretty(execution)?)?;
    Ok(())
}

pub fn load_pending_confirmation() -> Result<Option<PendingConfirmationState>> {
    let path = pending_state_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&content)?))
}

pub fn clear_pending_confirmation() -> Result<()> {
    let path = pending_state_path()?;
    let exec_path = pending_exec_path()?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    if exec_path.exists() {
        std::fs::remove_file(exec_path)?;
    }
    Ok(())
}

pub fn confirm_pending(
    identity: &Identity,
    log_store: &LogStore,
    cache: &ScriptCache,
) -> Result<AskResult> {
    let path = pending_exec_path()?;
    if !path.exists() {
        return Ok(AskResult::Unknown);
    }
    let content = std::fs::read_to_string(&path)?;
    let state: PendingExecutionState = serde_json::from_str(&content)?;
    let cwd = std::env::current_dir()?.to_string_lossy().to_string();
    let config = Config::load().unwrap_or_default();

    let result = match state.payload {
        PendingExecutionPayload::Cache { entry } => {
            let entry = *entry;
            let evaluation = evaluate_cache_entry(&entry, &config, &cwd)?;
            if !evaluation.scope_ok || !evaluation.path_boundary_ok {
                AskResult::Declined
            } else {
                let mut results = Vec::new();
                let mut total_duration = std::time::Duration::default();
                for step in &entry.steps {
                    let result = run::execute(&step.argv)?;
                    total_duration += result.duration;
                    results.push(result);
                }
                let all_exit_zero = results.iter().all(|r| r.exit_code == 0);
                let interaction_id = Identity::new_interaction_id();
                log_script_run(
                    identity,
                    &interaction_id,
                    &state.request,
                    &cwd,
                    &entry,
                    &results,
                    total_duration,
                    log_store,
                )?;
                if all_exit_zero {
                    cache.increment_success(&entry.request_hash)?;
                }
                AskResult::CacheHit {
                    entry,
                    results,
                    total_duration,
                    all_exit_zero,
                }
            }
        }
        PendingExecutionPayload::Agent { plan } => {
            let (plan, _) = recompute_plan_safety(*plan)?;
            let steps = plan_to_steps(&plan);
            if validate_step_paths(&steps, &cwd).is_err() {
                AskResult::Declined
            } else {
                let (results, total_duration, all_exit_zero) =
                    execute_plan_commands(&plan.commands)?;
                let entry = if all_exit_zero
                    && !ScriptCache::contains_sensitive_data(&state.request, &steps)
                {
                    Some(cache.save(&state.request, plan.risk.clone(), steps)?)
                } else {
                    None
                };
                let interaction_id = Identity::new_interaction_id();
                log_agent_turn(
                    identity,
                    &interaction_id,
                    &state.request,
                    &cwd,
                    &plan,
                    log_store,
                )?;
                for (i, cmd) in plan.commands.iter().enumerate() {
                    if i < results.len() {
                        log_command_run(
                            identity,
                            &interaction_id,
                            &state.request,
                            &cwd,
                            &cmd.argv,
                            &results[i],
                            log_store,
                        )?;
                    }
                }
                AskResult::FromAgent {
                    entry,
                    results,
                    total_duration,
                    all_exit_zero,
                    plan,
                }
            }
        }
    };

    clear_pending_confirmation()?;
    Ok(result)
}

fn write_private_file(path: &std::path::Path, contents: &str) -> Result<()> {
    std::fs::write(path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms)?;
    }
    Ok(())
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
    use crate::agent::{AgentCommand, AgentPlan};
    use crate::cache::{CacheEntry, CachedStep};
    use crate::log::reader::JsonlLogReader;
    use crate::log::writer::JsonlLogWriter;
    use crate::log::LogWriter;
    use crate::provider::Provider;
    use anyhow::Result;
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

    struct FixedPlanProvider {
        plan: AgentPlan,
    }

    impl Provider for FixedPlanProvider {
        fn plan(&self, _request: &str) -> Result<AgentPlan> {
            Ok(self.plan.clone())
        }
    }

    #[test]
    fn test_cache_hit_requires_confirmation_for_ask_once_before_threshold() -> Result<()> {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let identity = Identity::load_or_create()?;
        let log_dir = dir.path().join("logs");
        std::fs::create_dir_all(&log_dir)?;
        let writer = Box::new(JsonlLogWriter::new(&log_dir)?);
        let reader = Box::new(JsonlLogReader::new(&log_dir));
        let store = LogStore::new(writer, reader, 16);
        let cache = ScriptCache::new()?;

        cache.save(
            "write note",
            RiskLevel::LocalWrite,
            vec![CachedStep {
                command: "echo".into(),
                argv: vec!["echo".into(), "hi".into()],
                risk: RiskLevel::LocalWrite,
            }],
        )?;

        let provider = FixedPlanProvider {
            plan: AgentPlan {
                summary: "unused".into(),
                risk: RiskLevel::ReadOnly,
                commands: vec![],
            },
        };

        let result = process_request("write note", &identity, &store, &cache, &provider, false)?;
        assert!(matches!(result, AskResult::PendingConfirmation { .. }));
        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
        Ok(())
    }

    #[test]
    fn test_from_agent_returns_pending_confirmation_for_local_write_plan() -> Result<()> {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let identity = Identity::load_or_create()?;
        let log_dir = dir.path().join("logs");
        std::fs::create_dir_all(&log_dir)?;
        let writer = Box::new(JsonlLogWriter::new(&log_dir)?);
        let reader = Box::new(JsonlLogReader::new(&log_dir));
        let store = LogStore::new(writer, reader, 16);
        let cache = ScriptCache::new()?;

        let provider = FixedPlanProvider {
            plan: AgentPlan {
                summary: "Write note to file".into(),
                risk: RiskLevel::LocalWrite,
                commands: vec![AgentCommand {
                    command: "sh".into(),
                    argv: vec!["sh".into(), "-c".into(), "printf hi > note.txt".into()],
                    risk: RiskLevel::LocalWrite,
                    reason: "Create a note".into(),
                }],
            },
        };

        let result = process_request("write note", &identity, &store, &cache, &provider, false)?;
        assert!(matches!(result, AskResult::PendingConfirmation { .. }));
        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
        Ok(())
    }

    #[test]
    fn test_cache_hit_scope_mismatch_declined_even_with_yes() -> Result<()> {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let prev_cwd = std::env::current_dir()?;
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let other_cwd = dir.path().join("other");
        std::fs::create_dir_all(&other_cwd)?;
        std::env::set_current_dir(&other_cwd)?;

        let identity = Identity::load_or_create()?;
        let log_dir = dir.path().join("logs");
        std::fs::create_dir_all(&log_dir)?;
        let writer = Box::new(JsonlLogWriter::new(&log_dir)?);
        let reader = Box::new(JsonlLogReader::new(&log_dir));
        let store = LogStore::new(writer, reader, 16);
        let cache = ScriptCache::new()?;

        let mut entry = cache.save(
            "list files",
            RiskLevel::ReadOnly,
            vec![CachedStep {
                command: "echo".into(),
                argv: vec!["echo".into(), "hi".into()],
                risk: RiskLevel::ReadOnly,
            }],
        )?;
        entry.scope.cwd = dir.path().join("saved").display().to_string();
        let path = dir
            .path()
            .join(".terio")
            .join("cache")
            .join(format!("{}.json", entry.request_hash));
        std::fs::write(&path, serde_json::to_string_pretty(&entry)?)?;

        let provider = FixedPlanProvider {
            plan: AgentPlan {
                summary: "unused".into(),
                risk: RiskLevel::ReadOnly,
                commands: vec![],
            },
        };

        let result = process_request("list files", &identity, &store, &cache, &provider, true)?;
        assert!(matches!(result, AskResult::Declined));

        std::env::set_current_dir(prev_cwd)?;

        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
        Ok(())
    }

    #[test]
    fn test_provider_plan_risk_recomputed_to_destructive() -> Result<()> {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let identity = Identity::load_or_create()?;
        let log_dir = dir.path().join("logs");
        std::fs::create_dir_all(&log_dir)?;
        let writer = Box::new(JsonlLogWriter::new(&log_dir)?);
        let reader = Box::new(JsonlLogReader::new(&log_dir));
        let store = LogStore::new(writer, reader, 16);
        let cache = ScriptCache::new()?;

        let provider = FixedPlanProvider {
            plan: AgentPlan {
                summary: "Misclassified destructive command".into(),
                risk: RiskLevel::ReadOnly,
                commands: vec![AgentCommand {
                    command: "rm".into(),
                    argv: vec!["rm".into(), "-rf".into(), "tmp".into()],
                    risk: RiskLevel::ReadOnly,
                    reason: "wrong risk".into(),
                }],
            },
        };

        let result = process_request("clean tmp", &identity, &store, &cache, &provider, false)?;
        match result {
            AskResult::PendingConfirmation { plan_summary, .. } => {
                assert_eq!(plan_summary.risk, RiskLevel::Destructive);
                assert_eq!(plan_summary.commands[0].risk, RiskLevel::Destructive);
            }
            _ => panic!("expected pending confirmation"),
        }

        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
        Ok(())
    }

    #[test]
    fn test_unknown_command_requires_confirmation() -> Result<()> {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let identity = Identity::load_or_create()?;
        let log_dir = dir.path().join("logs");
        std::fs::create_dir_all(&log_dir)?;
        let writer = Box::new(JsonlLogWriter::new(&log_dir)?);
        let reader = Box::new(JsonlLogReader::new(&log_dir));
        let store = LogStore::new(writer, reader, 16);
        let cache = ScriptCache::new()?;

        let provider = FixedPlanProvider {
            plan: AgentPlan {
                summary: "Run custom binary".into(),
                risk: RiskLevel::ReadOnly,
                commands: vec![AgentCommand {
                    command: "custom-tool".into(),
                    argv: vec!["custom-tool".into(), "--version".into()],
                    risk: RiskLevel::ReadOnly,
                    reason: "custom".into(),
                }],
            },
        };

        let result = process_request(
            "custom version",
            &identity,
            &store,
            &cache,
            &provider,
            false,
        )?;
        assert!(matches!(result, AskResult::PendingConfirmation { .. }));

        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
        Ok(())
    }

    #[test]
    fn test_mismatched_command_and_argv_is_rejected() -> Result<()> {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let identity = Identity::load_or_create()?;
        let log_dir = dir.path().join("logs");
        std::fs::create_dir_all(&log_dir)?;
        let writer = Box::new(JsonlLogWriter::new(&log_dir)?);
        let reader = Box::new(JsonlLogReader::new(&log_dir));
        let store = LogStore::new(writer, reader, 16);
        let cache = ScriptCache::new()?;

        let provider = FixedPlanProvider {
            plan: AgentPlan {
                summary: "Mismatch".into(),
                risk: RiskLevel::ReadOnly,
                commands: vec![AgentCommand {
                    command: "ls".into(),
                    argv: vec!["rm".into(), "-rf".into(), "tmp".into()],
                    risk: RiskLevel::ReadOnly,
                    reason: "bad".into(),
                }],
            },
        };

        let result = process_request("bad plan", &identity, &store, &cache, &provider, false)?;
        assert!(matches!(result, AskResult::Declined));

        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
        Ok(())
    }

    #[test]
    fn test_empty_argv_is_rejected() -> Result<()> {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let identity = Identity::load_or_create()?;
        let log_dir = dir.path().join("logs");
        std::fs::create_dir_all(&log_dir)?;
        let writer = Box::new(JsonlLogWriter::new(&log_dir)?);
        let reader = Box::new(JsonlLogReader::new(&log_dir));
        let store = LogStore::new(writer, reader, 16);
        let cache = ScriptCache::new()?;

        let provider = FixedPlanProvider {
            plan: AgentPlan {
                summary: "Empty argv".into(),
                risk: RiskLevel::ReadOnly,
                commands: vec![AgentCommand {
                    command: "ls".into(),
                    argv: vec![],
                    risk: RiskLevel::ReadOnly,
                    reason: "bad".into(),
                }],
            },
        };

        let result = process_request("bad plan", &identity, &store, &cache, &provider, false)?;
        assert!(matches!(result, AskResult::Declined));

        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
        Ok(())
    }

    #[test]
    fn test_confirm_pending_executes_saved_exact_plan() -> Result<()> {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let identity = Identity::load_or_create()?;
        let log_dir = dir.path().join("logs");
        std::fs::create_dir_all(&log_dir)?;
        let writer = Box::new(JsonlLogWriter::new(&log_dir)?);
        let reader = Box::new(JsonlLogReader::new(&log_dir));
        let store = LogStore::new(writer, reader, 16);
        let cache = ScriptCache::new()?;

        let preview = PendingConfirmationState {
            source: PendingSource::Agent,
            plan_summary: PendingPlanSummary {
                request: "say secret".into(),
                summary: "Echo secret".into(),
                risk: RiskLevel::ReadOnly,
                requires_confirmation: true,
                trust: None,
                commands: vec![PendingCommand {
                    argv: vec!["echo".into(), "api_key=secret123".into()],
                    risk: RiskLevel::ReadOnly,
                    reason: "preview".into(),
                }],
            },
        };
        let execution = PendingExecutionState {
            request: "say secret".into(),
            payload: PendingExecutionPayload::Agent {
                plan: Box::new(AgentPlan {
                    summary: "Echo secret".into(),
                    risk: RiskLevel::ReadOnly,
                    commands: vec![AgentCommand {
                        command: "echo".into(),
                        argv: vec!["echo".into(), "api_key=secret123".into()],
                        risk: RiskLevel::ReadOnly,
                        reason: "exact".into(),
                    }],
                }),
            },
        };

        save_pending_confirmation(&preview, &execution)?;
        let preview_loaded = load_pending_confirmation()?.unwrap();
        assert!(preview_loaded.plan_summary.commands[0].argv[1].contains("[REDACTED]"));

        let result = confirm_pending(&identity, &store, &cache)?;
        match result {
            AskResult::FromAgent { results, .. } => {
                assert_eq!(results[0].stdout.trim(), "api_key=secret123");
            }
            _ => panic!("expected exact pending plan execution"),
        }

        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
        Ok(())
    }
}
