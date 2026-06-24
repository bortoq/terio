// Shell execution: запуск команды, захват stdout/stderr/exit/duration

use crate::types::{
    CommandInfo, CostCounters, ExecutionCost, LogEntry, LogKind, LogStatus, RiskLevel,
};
use anyhow::Result;
use std::time::Instant;

/// Результат выполнения команды.
#[derive(Debug)]
pub struct CommandResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration: std::time::Duration,
}

/// Выполняет shell-команду с заданными argv.
/// Безопасность: команда запускается напрямую, без shell-оболочки.
pub fn execute(argv: &[String]) -> Result<CommandResult> {
    if argv.is_empty() {
        anyhow::bail!("empty command");
    }

    let start = Instant::now();
    let output = std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .output()?;
    let duration = start.elapsed();

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    Ok(CommandResult {
        exit_code,
        stdout,
        stderr,
        duration,
    })
}

/// Создаёт LogEntry для command_run по результату выполнения.
pub fn make_command_run_entry(
    instance_id: &str,
    session_id: &str,
    interaction_id: Option<String>,
    request: &str,
    cwd: &str,
    argv: &[String],
    result: &CommandResult,
) -> LogEntry {
    let stdout_summary = truncate(&result.stdout, 1024);
    let stderr_summary = truncate(&result.stderr, 1024);

    let status = if result.exit_code == 0 {
        LogStatus::Success
    } else {
        LogStatus::Failed
    };

    let bytes_read = stdout_summary.len() as u64 + stderr_summary.len() as u64;

    LogEntry {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        session_id: session_id.to_string(),
        ts: chrono::Utc::now().to_rfc3339(),
        interaction_id,
        parent_interaction_id: None,
        kind: LogKind::CommandRun,
        display_profile: crate::types::DisplayProfile::default(),
        cost_counters: CostCounters {
            execution_cost: ExecutionCost {
                duration_ms: result.duration.as_millis() as u64,
                commands_executed: 1,
                bytes_read,
                bytes_written: 0,
            },
            ..CostCounters::default()
        },
        request: Some(request.to_string()),
        cwd: Some(cwd.to_string()),
        risk: Some(compute_risk(&argv[0], &argv[1..])),
        status: Some(status),
        failure_kind: None,
        prompt_summary: None,
        plan: None,
        model_provider: None,
        model_name: None,
        duration_ms: Some(result.duration.as_millis() as u64),
        tokens_used: None,
        command: Some(CommandInfo {
            display: argv.join(" "),
            argv: argv.to_vec(),
        }),
        exit: Some(result.exit_code),
        stdout_summary: Some(stdout_summary),
        stderr_summary: Some(stderr_summary),
        script_id: None,
        cache_hit: None,
        model_called: None,
        tokens_saved_estimate: None,
        success_count_before: None,
        success_count_after: None,
        steps: None,
        description: None,
    }
}

/// Простейший risk-анализ по command + argv (MVP: только самые явные случаи).
pub fn compute_risk(command: &str, args: &[String]) -> RiskLevel {
    // destructive
    if matches!(command, "rm" | "mv") {
        return RiskLevel::Destructive;
    }
    if command == "git"
        && args
            .iter()
            .any(|a| matches!(a.as_str(), "push" | "clean" | "reset"))
    {
        return RiskLevel::Destructive;
    }
    if command == "find" && args.iter().any(|a| a == "-delete" || a == "-exec") {
        return RiskLevel::Destructive;
    }
    if command == "sudo" {
        return RiskLevel::Destructive;
    }

    // network_write
    if command == "curl"
        && args
            .iter()
            .any(|a| a == "-X" || a == "--request" || a == "-d" || a == "--data")
    {
        return RiskLevel::NetworkWrite;
    }
    if command == "git" && args.iter().any(|a| a == "push") {
        return RiskLevel::NetworkWrite;
    }

    // network_read
    if command == "curl" || command == "wget" {
        return RiskLevel::NetworkRead;
    }
    if command == "git"
        && args
            .iter()
            .any(|a| a == "fetch" || a == "clone" || a == "pull")
    {
        return RiskLevel::NetworkRead;
    }

    // local_write
    if matches!(command, "mkdir" | "cp" | "touch") {
        return RiskLevel::LocalWrite;
    }
    if command == "ffmpeg" {
        return RiskLevel::LocalWrite;
    }

    // read_only по умолчанию
    RiskLevel::ReadOnly
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}... (truncated)", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_echo() {
        let argv = vec!["echo".to_string(), "hello".to_string()];
        let result = execute(&argv).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.trim(), "hello");
    }

    #[test]
    fn test_execute_non_zero_exit() {
        let argv = vec!["sh".to_string(), "-c".to_string(), "exit 42".to_string()];
        let result = execute(&argv).unwrap();
        assert_eq!(result.exit_code, 42);
    }

    #[test]
    fn test_execute_empty_fails() {
        assert!(execute(&[]).is_err());
    }

    #[test]
    fn test_compute_risk_destructive() {
        assert_eq!(
            compute_risk("rm", &["-rf".to_string(), "/tmp".to_string()]),
            RiskLevel::Destructive
        );
        assert_eq!(
            compute_risk("sudo", &["apt".to_string()]),
            RiskLevel::Destructive
        );
    }

    #[test]
    fn test_compute_risk_read_only() {
        assert_eq!(
            compute_risk("ls", &["-la".to_string()]),
            RiskLevel::ReadOnly
        );
        assert_eq!(
            compute_risk("echo", &["hello".to_string()]),
            RiskLevel::ReadOnly
        );
    }

    #[test]
    fn test_make_command_run_entry() {
        let result = CommandResult {
            exit_code: 0,
            stdout: "hello".to_string(),
            stderr: String::new(),
            duration: std::time::Duration::from_millis(10),
        };
        let entry = make_command_run_entry(
            "inst1",
            "sess1",
            Some("inter1".into()),
            "say hello",
            "/tmp",
            &["echo".into(), "hello".into()],
            &result,
        );
        assert_eq!(entry.instance_id, "inst1");
        assert_eq!(entry.kind, LogKind::CommandRun);
        assert_eq!(entry.exit, Some(0));
        assert_eq!(entry.stdout_summary, Some("hello".to_string()));
    }
}
