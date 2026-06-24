// Shell execution: запуск команды, захват stdout/stderr/exit/duration

use crate::types::{
    CommandInfo, CostCounters, ExecutionCost, LogEntry, LogKind, LogStatus, RiskLevel,
};
use anyhow::Result;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::Instant;

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

/// PID текущего выполняемого дочернего процесса (0 = нет процесса).
static CURRENT_PID: AtomicI32 = AtomicI32::new(0);

/// Флаг запроса отмены.
static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Результат выполнения команды.
#[derive(Debug)]
pub struct CommandResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration: std::time::Duration,
}

/// Выполняет shell-команду с заданными argv.
/// Поддерживает отмену через `cancel_current()` или Ctrl+C.
/// Безопасность: команда запускается напрямую, без shell-оболочки.
pub fn execute(argv: &[String]) -> Result<CommandResult> {
    if argv.is_empty() {
        anyhow::bail!("empty command");
    }

    CANCEL_REQUESTED.store(false, Ordering::SeqCst);

    let start = Instant::now();
    let child = std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let pid = child.id() as i32;
    CURRENT_PID.store(pid, Ordering::SeqCst);

    let output = child.wait_with_output()?;

    CURRENT_PID.store(0, Ordering::SeqCst);
    let duration = start.elapsed();

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Если процесс был убит сигналом (отмена)
    let exit_code = if let Some(sig) = output.status.signal() {
        if CANCEL_REQUESTED.load(Ordering::SeqCst) {
            -1 // cancelled
        } else {
            -sig // negative = killed by signal
        }
    } else {
        output.status.code().unwrap_or(-1)
    };

    Ok(CommandResult {
        exit_code,
        stdout,
        stderr,
        duration,
    })
}

/// Отменить текущее выполнение (SIGTERM на Unix, taskkill на Windows).
/// Возвращает true, если процесс был активен и отправлен сигнал.
pub fn cancel_current() -> bool {
    CANCEL_REQUESTED.store(true, Ordering::SeqCst);
    let pid = CURRENT_PID.load(Ordering::SeqCst);
    if pid > 0 {
        kill_process(pid as u32);
        true
    } else {
        false
    }
}

/// Проверить, был ли запрошен cancel (для использования из сигнального обработчика).
pub fn is_cancel_requested() -> bool {
    CANCEL_REQUESTED.load(Ordering::SeqCst)
}

#[cfg(unix)]
fn kill_process(pid: u32) {
    unsafe {
        // libc always linked on Unix — declare extern for raw syscall
        extern "C" {
            fn kill(pid: i32, sig: i32) -> i32;
        }
        kill(pid as i32, 15); // SIGTERM (pid: u32 → i32 lossless)
    }
}

#[cfg(not(unix))]
fn kill_process(pid: u32) {
    let _ = std::process::Command::new("taskkill")
        .args(&["/PID", &pid.to_string(), "/F"])
        .output();
}

/// Установить обработчик Ctrl+C (SIGINT).
/// Вызывается один раз при старте terio.
pub fn setup_ctrlc_handler() {
    #[cfg(unix)]
    // SAFETY: сигнальный хендлер делает только signal-safe операции (atomics + kill).
    unsafe {
        extern "C" fn handle_sigint(_: i32) {
            CANCEL_REQUESTED.store(true, Ordering::SeqCst);
            let pid = CURRENT_PID.load(Ordering::SeqCst);
            if pid > 0 {
                // SAFETY: signal-safe — kill не блокируется и не использует malloc
                unsafe {
                    extern "C" {
                        fn kill(pid: i32, sig: i32) -> i32;
                    }
                    kill(pid, 15); // SIGTERM
                }
            }
        }

        extern "C" {
            fn signal(sig: i32, handler: usize) -> usize;
        }
        const SIGINT: i32 = 2;
        signal(SIGINT, handle_sigint as *const () as usize);
    }
    // На Windows Ctrl+C обрабатывается стандартно (терминация процесса)
    #[cfg(not(unix))]
    {
        let _ = CANCEL_REQUESTED; // suppress unused warning
    }
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
    let stdout_summary = truncate_safe(&result.stdout, 1024);
    let stderr_summary = truncate_safe(&result.stderr, 1024);

    let status = if result.exit_code == 0 {
        LogStatus::Success
    } else {
        LogStatus::Failed
    };

    let bytes_read = result.stdout.len() as u64 + result.stderr.len() as u64;

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

/// Создаёт LogEntry для command_run, когда команда не найдена.
pub fn make_spawn_failed_entry(
    instance_id: &str,
    session_id: &str,
    interaction_id: Option<String>,
    request: &str,
    cwd: &str,
    argv: &[String],
    error_msg: &str,
) -> LogEntry {
    LogEntry {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        session_id: session_id.to_string(),
        ts: chrono::Utc::now().to_rfc3339(),
        interaction_id,
        parent_interaction_id: None,
        kind: LogKind::CommandRun,
        display_profile: crate::types::DisplayProfile::default(),
        cost_counters: CostCounters::default(),
        request: Some(request.to_string()),
        cwd: Some(cwd.to_string()),
        risk: Some(RiskLevel::ReadOnly),
        status: Some(LogStatus::Failed),
        failure_kind: Some("spawn_failed".to_string()),
        prompt_summary: None,
        plan: None,
        model_provider: None,
        model_name: None,
        duration_ms: None,
        tokens_used: None,
        command: Some(CommandInfo {
            display: argv.join(" "),
            argv: argv.to_vec(),
        }),
        exit: None,
        stdout_summary: None,
        stderr_summary: Some(truncate_safe(error_msg, 1024)),
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

/// Простейший risk-анализ по command + argv (MVP).
pub fn compute_risk(command: &str, args: &[String]) -> RiskLevel {
    // destructive
    if command == "sudo" {
        return RiskLevel::Destructive;
    }
    if matches!(command, "rm" | "mv" | "dd") {
        return RiskLevel::Destructive;
    }
    if command == "git" {
        // git clean → всегда destructive
        if args.iter().any(|a| a == "clean") {
            return RiskLevel::Destructive;
        }
        // git reset --hard → destructive; git reset без --hard → local_write
        if args.iter().any(|a| a == "reset") && args.iter().any(|a| a == "--hard") {
            return RiskLevel::Destructive;
        }
        // git reset без --hard не попадает в destructive
    }
    if command == "find" && args.iter().any(|a| a == "-delete" || a == "-exec") {
        return RiskLevel::Destructive;
    }
    if command == "docker"
        && args
            .iter()
            .any(|a| a == "rm" || a == "rmi" || a == "system")
    {
        return RiskLevel::Destructive;
    }

    // network_write
    if command == "git" && args.iter().any(|a| a == "push") {
        return RiskLevel::NetworkWrite;
    }
    if command == "curl" {
        let next_is_post = args.windows(2).any(|w| {
            matches!(w[0].as_str(), "-X" | "--request")
                && matches!(
                    w[1].to_uppercase().as_str(),
                    "POST" | "PUT" | "PATCH" | "DELETE"
                )
        });
        let has_data = args
            .iter()
            .any(|a| a == "-d" || a == "--data" || a == "--data-binary");
        if next_is_post || has_data {
            return RiskLevel::NetworkWrite;
        }
    }
    if command == "rsync" {
        return RiskLevel::NetworkWrite;
    }

    // network_read
    if command == "curl" {
        return RiskLevel::NetworkRead; // curl без POST/data
    }
    if command == "wget" {
        return RiskLevel::NetworkRead;
    }
    if command == "git"
        && args
            .iter()
            .any(|a| matches!(a.as_str(), "fetch" | "clone" | "pull"))
    {
        return RiskLevel::NetworkRead;
    }
    if command == "ssh" || command == "scp" {
        return RiskLevel::NetworkRead;
    }

    // credential_access
    if command == "cat"
        && args.iter().any(|a| {
            a.contains(".ssh") || a.contains("id_rsa") || a.contains(".env") || a.contains("token")
        })
    {
        return RiskLevel::CredentialAccess;
    }

    // local_write
    if matches!(command, "mkdir" | "cp" | "touch" | "chmod" | "ln") {
        return RiskLevel::LocalWrite;
    }
    if command == "ffmpeg" || command == "ffprobe" {
        return RiskLevel::LocalWrite;
    }
    if command == "docker" && args.first().map(|s| s.as_str()) == Some("run") {
        return RiskLevel::LocalWrite;
    }
    if command == "git"
        && args
            .iter()
            .any(|a| a == "add" || a == "commit" || a == "checkout")
    {
        return RiskLevel::LocalWrite;
    }

    // read_only по умолчанию
    RiskLevel::ReadOnly
}

/// Безопасное усечение строки по символам (не по байтам).
pub fn truncate_safe(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = chars[..max_chars].iter().collect();
        format!("{}… (truncated)", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(cmd: &str, args: &[&str]) -> Vec<String> {
        let mut v = vec![cmd.to_string()];
        v.extend(args.iter().map(|s| s.to_string()));
        v
    }

    #[test]
    fn test_execute_echo() {
        let result = execute(&argv("echo", &["hello"])).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.trim(), "hello");
    }

    #[test]
    fn test_execute_non_zero_exit() {
        let result = execute(&argv("sh", &["-c", "exit 42"])).unwrap();
        assert_eq!(result.exit_code, 42);
    }

    #[test]
    fn test_execute_empty_fails() {
        assert!(execute(&[]).is_err());
    }

    #[test]
    fn test_risk_destructive() {
        assert_eq!(
            compute_risk("rm", &["-rf".into(), "/tmp".into()]),
            RiskLevel::Destructive
        );
        assert_eq!(
            compute_risk("sudo", &["apt".into()]),
            RiskLevel::Destructive
        );
        assert_eq!(
            compute_risk("git", &["reset".into(), "--hard".into()]),
            RiskLevel::Destructive
        );
        assert_eq!(
            compute_risk("git", &["clean".into(), "-fd".into()]),
            RiskLevel::Destructive
        );
    }

    #[test]
    fn test_risk_network_write() {
        assert_eq!(
            compute_risk("git", &["push".into()]),
            RiskLevel::NetworkWrite
        );
        assert_eq!(
            compute_risk(
                "curl",
                &[
                    "-X".into(),
                    "POST".into(),
                    "-d".into(),
                    "data".into(),
                    "http://x".into()
                ]
            ),
            RiskLevel::NetworkWrite
        );
    }

    #[test]
    fn test_risk_network_read() {
        assert_eq!(
            compute_risk("curl", &["http://example.com".into()]),
            RiskLevel::NetworkRead
        );
        assert_eq!(
            compute_risk("git", &["fetch".into(), "origin".into()]),
            RiskLevel::NetworkRead
        );
        assert_eq!(
            compute_risk("wget", &["http://x".into()]),
            RiskLevel::NetworkRead
        );
    }

    #[test]
    fn test_risk_read_only() {
        assert_eq!(compute_risk("ls", &["-la".into()]), RiskLevel::ReadOnly);
        assert_eq!(compute_risk("echo", &["hello".into()]), RiskLevel::ReadOnly);
    }

    #[test]
    fn test_risk_local_write() {
        assert_eq!(
            compute_risk("mkdir", &["-p".into(), "dir".into()]),
            RiskLevel::LocalWrite
        );
        assert_eq!(
            compute_risk("git", &["add".into(), "file".into()]),
            RiskLevel::LocalWrite
        );
    }

    #[test]
    fn test_credential_access() {
        assert_eq!(
            compute_risk("cat", &["~/.ssh/id_rsa".into()]),
            RiskLevel::CredentialAccess
        );
    }

    #[test]
    fn test_truncate_safe() {
        assert_eq!(truncate_safe("hello", 10), "hello");
        assert_eq!(truncate_safe("hello world", 5), "hello… (truncated)");
        // multibyte UTF-8
        let s = "привет мир";
        assert_eq!(truncate_safe(s, 6), "привет… (truncated)");
        // truncation to 1 char of multibyte
        let t = truncate_safe(s, 1);
        assert_eq!(t, "п… (truncated)");
        assert_eq!(t.chars().next(), Some('п'));
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
            &argv("echo", &["hello"]),
            &result,
        );
        assert_eq!(entry.instance_id, "inst1");
        assert_eq!(entry.kind, LogKind::CommandRun);
        assert_eq!(entry.exit, Some(0));
        assert_eq!(entry.stdout_summary, Some("hello".to_string()));
    }

    #[test]
    fn test_make_spawn_failed_entry() {
        let entry = make_spawn_failed_entry(
            "i1",
            "s1",
            Some("int1".into()),
            "run bad",
            "/tmp",
            &argv("nonexistent", &[]),
            "No such file or directory",
        );
        assert_eq!(entry.status, Some(LogStatus::Failed));
        assert_eq!(entry.failure_kind, Some("spawn_failed".to_string()));
    }

    #[test]
    fn test_bytes_read_from_original() {
        let result = CommandResult {
            exit_code: 0,
            stdout: "a".repeat(2000),
            stderr: String::new(),
            duration: std::time::Duration::from_millis(1),
        };
        let entry = make_command_run_entry(
            "i1",
            "s1",
            None,
            "big",
            "/tmp",
            &argv("echo", &["big"]),
            &result,
        );
        // bytes_read берётся из полного stdout, не из summary
        assert_eq!(entry.cost_counters.execution_cost.bytes_read, 2000);
        // stdout_summary усечён до 1024
        assert_eq!(
            entry.stdout_summary.as_ref().map(|s| s.len()),
            Some("a".repeat(1024).len() + "… (truncated)".len())
        );
    }
}
