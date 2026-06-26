// UI state: static globals, UiCommand, helpers for the terminal-like UI.
// Phase 0: simplified — no modes, no WorkspaceView.

use crate::log::LogReader;
use crate::types::{LogEntry, LogKind};
use crate::undo::UndoStatus;
use std::sync::{mpsc::Sender, Mutex};
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// Static global state
// ---------------------------------------------------------------------------

static LOG_ENTRIES: Mutex<Vec<LogEntry>> = Mutex::new(Vec::new());
static LIVE_STREAM: Mutex<Option<broadcast::Receiver<LogEntry>>> = Mutex::new(None);
static ACTION_SENDER: Mutex<Option<Sender<UiCommand>>> = Mutex::new(None);

#[derive(Debug, Clone)]
pub enum UiCommand {
    Ask(String),
    Confirm,
    Undo,
    Redo,
    Focus(String), // "up" | "down"
    Scroll(i32),   // lines (positive = down, negative = up)
    Repeat,
    Help,
    Mode(String), // "quiet" | "normal" | "debug"
    Exit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActivityState {
    Idle,
    Busy,
}

/// Initialize global state from external sources.
pub fn init_globals(
    entries: Vec<LogEntry>,
    stream: Option<broadcast::Receiver<LogEntry>>,
    sender: Option<Sender<UiCommand>>,
) {
    if let Ok(mut guard) = LOG_ENTRIES.lock() {
        *guard = entries;
    }
    if let Ok(mut guard) = LIVE_STREAM.lock() {
        *guard = stream;
    }
    if let Ok(mut guard) = ACTION_SENDER.lock() {
        *guard = sender;
    }
}

pub fn get_entries() -> Vec<LogEntry> {
    LOG_ENTRIES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

pub fn replace_entries(entries: Vec<LogEntry>) {
    if let Ok(mut guard) = LOG_ENTRIES.lock() {
        *guard = entries;
    }
}

pub fn append_live_entry(entry: LogEntry) {
    if let Ok(mut guard) = LOG_ENTRIES.lock() {
        guard.push(entry);
        if guard.len() > 500 {
            let excess = guard.len() - 500;
            guard.drain(0..excess);
        }
    }
}

pub fn take_live_stream() -> Option<broadcast::Receiver<LogEntry>> {
    LIVE_STREAM.lock().ok().and_then(|mut guard| guard.take())
}

pub fn send_ui_command(command: UiCommand) {
    if let Ok(guard) = ACTION_SENDER.lock() {
        if let Some(sender) = guard.as_ref() {
            let _ = sender.send(command);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers (same as before)
// ---------------------------------------------------------------------------

pub fn refresh_entries() {
    if let Ok(log_dir) = crate::log::writer::JsonlLogWriter::default_dir() {
        let reader = crate::log::reader::JsonlLogReader::new(&log_dir);
        if let Ok(fresh) = reader.recent(100) {
            replace_entries(fresh);
        }
    }
}

pub fn refresh_undo_status() -> UndoStatus {
    crate::undo::latest_status().unwrap_or_default()
}

pub fn undo_summary_label(status: &UndoStatus) -> String {
    status
        .summary
        .clone()
        .unwrap_or_else(|| "undo/redo unavailable".to_string())
}

pub fn is_completion_entry(entry: &LogEntry) -> bool {
    matches!(
        entry.kind,
        LogKind::AgentTurn | LogKind::CommandRun | LogKind::ScriptRun | LogKind::SystemEvent
    )
}

/// Парсинг ввода: internal terio commands vs LLM ask.
pub fn parse_input(input: &str) -> UiCommand {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return UiCommand::Ask(String::new());
    }
    match trimmed {
        "exit" | "quit" => return UiCommand::Exit,
        "help" => return UiCommand::Help,
        "repeat" => return UiCommand::Repeat,
        "undo" => return UiCommand::Undo,
        "redo" => return UiCommand::Redo,
        "confirm y" | "confirm yes" | "y" | "Y" => return UiCommand::Confirm,
        "confirm n" | "confirm no" | "n" | "N" => {
            // no-op: decline is just not confirming
            return UiCommand::Ask(String::new());
        }
        _ => {}
    }
    if let Some(mode) = trimmed.strip_prefix("mode ") {
        return UiCommand::Mode(mode.trim().to_string());
    }
    if let Some(dir) = trimmed.strip_prefix("focus ") {
        return UiCommand::Focus(dir.trim().to_string());
    }
    if let Some(lines) = trimmed.strip_prefix("scroll ") {
        if let Ok(n) = lines.trim().parse::<i32>() {
            return UiCommand::Scroll(n);
        }
    }
    UiCommand::Ask(trimmed.to_string())
}

/// Безопасное усечение строки по символам (не байтам).
pub fn truncate_safe(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Получить последний запрос из лога (для repeat).
pub fn last_request() -> Option<String> {
    let entries = get_entries();
    entries.iter().rev().find_map(|e| e.request.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn test_is_completion_entry_for_system_event() {
        let entry = LogEntry::new_system_event("i1", "s1", "done");
        assert!(is_completion_entry(&entry));
    }

    #[test]
    fn test_is_completion_entry_for_agent_turn() {
        let entry = LogEntry {
            schema_version: 1,
            instance_id: "i1".into(),
            session_id: "s1".into(),
            ts: "2026-06-24T00:00:00Z".into(),
            interaction_id: None,
            parent_interaction_id: None,
            kind: LogKind::AgentTurn,
            display_profile: DisplayProfile::default(),
            cost_counters: CostCounters::default(),
            request: None,
            cwd: None,
            risk: None,
            status: Some(LogStatus::Success),
            failure_kind: None,
            prompt_summary: None,
            plan: None,
            model_provider: None,
            model_name: None,
            duration_ms: None,
            tokens_used: None,
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
            description: Some("agent".into()),
        };
        assert!(is_completion_entry(&entry));
    }

    #[test]
    fn test_undo_summary_label_prefers_summary() {
        let label = undo_summary_label(&UndoStatus {
            can_undo: true,
            can_redo: false,
            summary: Some("Create file".into()),
        });
        assert_eq!(label, "Create file");
    }

    #[test]
    fn test_undo_summary_label_has_fallback() {
        assert_eq!(
            undo_summary_label(&UndoStatus::default()),
            "undo/redo unavailable"
        );
    }

    #[test]
    fn test_truncate_safe_short() {
        assert_eq!(truncate_safe("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_safe_long() {
        assert_eq!(truncate_safe("hello world", 5), "hello");
    }

    #[test]
    fn test_parse_input_help() {
        assert!(matches!(parse_input("help"), UiCommand::Help));
    }

    #[test]
    fn test_parse_input_repeat() {
        assert!(matches!(parse_input("repeat"), UiCommand::Repeat));
    }

    #[test]
    fn test_parse_input_mode() {
        match parse_input("mode quiet") {
            UiCommand::Mode(m) => assert_eq!(m, "quiet"),
            _ => panic!("expected Mode"),
        }
    }

    #[test]
    fn test_parse_input_focus() {
        match parse_input("focus up") {
            UiCommand::Focus(d) => assert_eq!(d, "up"),
            _ => panic!("expected Focus"),
        }
    }

    #[test]
    fn test_parse_input_confirm_y() {
        assert!(matches!(parse_input("y"), UiCommand::Confirm));
        assert!(matches!(parse_input("Y"), UiCommand::Confirm));
        assert!(matches!(parse_input("confirm y"), UiCommand::Confirm));
        assert!(matches!(parse_input("confirm yes"), UiCommand::Confirm));
    }

    #[test]
    fn test_parse_input_confirm_n() {
        assert!(matches!(parse_input("n"), UiCommand::Ask(_)));
        assert!(matches!(parse_input("confirm n"), UiCommand::Ask(_)));
    }

    #[test]
    fn test_parse_input_exit() {
        assert!(matches!(parse_input("exit"), UiCommand::Exit));
        assert!(matches!(parse_input("quit"), UiCommand::Exit));
    }

    #[test]
    fn test_parse_input_scroll() {
        match parse_input("scroll 5") {
            UiCommand::Scroll(n) => assert_eq!(n, 5),
            _ => panic!("expected Scroll"),
        }
    }

    #[test]
    fn test_parse_input_ask_fallback() {
        match parse_input("list files in /tmp") {
            UiCommand::Ask(s) => assert_eq!(s, "list files in /tmp"),
            _ => panic!("expected Ask"),
        }
    }

    #[test]
    fn test_parse_input_empty_ask() {
        assert!(matches!(parse_input(""), UiCommand::Ask(_)));
    }
}
