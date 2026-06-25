// UI state: static globals, RowData, helpers for the Dioxus workspace.
// Extracted from app.rs per audit recommendation (P0.4).

use crate::log::LogReader;
use crate::types::{LogEntry, LogKind};
use crate::undo::UndoStatus;
use std::sync::{mpsc::Sender, Mutex};
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// Static global state (MVP pattern — explicit per audit note)
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
// RowData — prepared UI row
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
pub struct RowData {
    pub key: String,
    pub ts: String,
    pub kind: String,
    pub kind_color: &'static str,
    pub status: String,
    pub status_color: &'static str,
    pub desc: String,
    pub exit: String,
    pub risk: String,
    pub trust: String,
    pub stdout: String,
    pub stderr: String,
}

impl RowData {
    pub fn detail_text(&self) -> String {
        let mut parts = vec![format!("[{}] {} {}", self.ts, self.kind, self.desc)];
        if !self.status.is_empty() {
            parts.push(format!("status: {}", self.status));
        }
        if !self.risk.is_empty() {
            parts.push(format!("risk: {}", self.risk));
        }
        if !self.trust.is_empty() {
            parts.push(format!("trust: {}", self.trust));
        }
        parts.join("\n")
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn status_color(status: &str) -> &'static str {
    match status {
        "Success" => "#6a9955",
        "Failed" => "#f14c4c",
        "Cancelled" => "#d7ba7d",
        _ => "#888",
    }
}

pub fn kind_color(kind: &str) -> &'static str {
    match kind {
        "AgentTurn" => "#569cd6",
        "CommandRun" => "#ce9178",
        "ScriptRun" => "#6a9955",
        "SystemEvent" => "#9cdcfe",
        _ => "#888",
    }
}

fn risk_label(entry: &LogEntry) -> String {
    entry
        .risk
        .as_ref()
        .map(|r| format!("{:?}", r))
        .unwrap_or_default()
}

pub fn trust_badge(entry: &LogEntry) -> String {
    if entry.cache_hit == Some(true) {
        let level = entry
            .success_count_after
            .or(entry.success_count_before)
            .unwrap_or(0) as u32;
        return format!("auto {}", crate::trust::trust_level_str(level, 3));
    }

    if entry.model_called == Some(true) {
        return "confirm".to_string();
    }

    match entry.risk.as_ref() {
        Some(crate::types::RiskLevel::ReadOnly) => "manual".to_string(),
        Some(_) => "confirm".to_string(),
        None => String::new(),
    }
}

/// Подготовка строк для отображения.
/// Использует interaction_id + kind + index для уникальности ключа (fix P0.2).
pub fn prepare_rows(entries: &[LogEntry]) -> Vec<RowData> {
    entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let kind = format!("{:?}", entry.kind);
            let status = entry
                .status
                .as_ref()
                .map(|s| format!("{:?}", s))
                .unwrap_or_default();
            let ts = truncate_safe(&entry.ts, 19);
            let desc = entry
                .command
                .as_ref()
                .map(|c| truncate_safe(&c.display, 120))
                .or_else(|| entry.description.clone())
                .or_else(|| entry.prompt_summary.clone())
                .unwrap_or_else(|| "—".to_string());
            let exit = entry.exit.map(|e| e.to_string()).unwrap_or_default();
            let risk = risk_label(entry);
            let trust = trust_badge(entry);
            let stdout = entry.stdout_summary.clone().unwrap_or_default();
            let stderr = entry.stderr_summary.clone().unwrap_or_default();

            // Уникальный ключ: interaction_id + kind + index (fix P0.2 row key uniqueness)
            let key = format!(
                "{}-{}-{}",
                entry.interaction_id.as_deref().unwrap_or("none"),
                kind,
                idx
            );

            RowData {
                key,
                ts,
                kind_color: kind_color(&kind),
                status_color: status_color(&status),
                kind,
                status,
                desc,
                exit,
                risk,
                trust,
                stdout,
                stderr,
            }
        })
        .collect()
}

/// Безопасное усечение строки по символам (не байтам).
pub fn truncate_safe(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn test_prepare_rows_key_uniqueness() {
        // Несколько записей с одинаковым interaction_id должны иметь разные ключи
        let e1 = LogEntry::new_system_event("i1", "s1", "event1");
        let e2 = LogEntry::new_system_event("i1", "s1", "event2");
        let rows = prepare_rows(&[e1, e2]);
        assert_ne!(rows[0].key, rows[1].key, "row keys must be unique");
    }

    #[test]
    fn test_prepare_rows_key_with_interaction_id() {
        let mut entry = LogEntry::new_system_event("i1", "s1", "event");
        entry.interaction_id = Some("int-123".into());
        let rows = prepare_rows(&[entry]);
        assert!(rows[0].key.starts_with("int-123-"));
    }

    #[test]
    fn test_prepare_rows_includes_stdout_and_stderr_details() {
        let mut entry = LogEntry::new_command_run(
            "i1",
            "s1",
            Some("int1".into()),
            "echo hello",
            "/tmp",
            &["echo".into(), "hello".into()],
            0,
            std::time::Duration::from_millis(1),
            "hello",
            "warn",
            CostCounters::default(),
        );
        entry.stdout_summary = Some("hello".into());
        entry.stderr_summary = Some("warn".into());
        let rows = prepare_rows(&[entry]);
        assert_eq!(rows[0].stdout, "hello");
        assert_eq!(rows[0].stderr, "warn");
    }

    #[test]
    fn test_trust_badge_for_cache_hit_uses_threshold_label() {
        let entry = LogEntry {
            schema_version: 1,
            instance_id: "i1".into(),
            session_id: "s1".into(),
            ts: "2026-06-24T00:00:00Z".into(),
            interaction_id: None,
            parent_interaction_id: None,
            kind: LogKind::ScriptRun,
            display_profile: DisplayProfile::default(),
            cost_counters: CostCounters::default(),
            request: None,
            cwd: None,
            risk: Some(crate::types::RiskLevel::ReadOnly),
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
            cache_hit: Some(true),
            model_called: Some(false),
            tokens_saved_estimate: None,
            success_count_before: Some(2),
            success_count_after: Some(3),
            steps: None,
            description: None,
        };

        assert_eq!(trust_badge(&entry), "auto ✓ 3/3");
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
    fn test_prepare_rows_caps_at_500() {
        // Заполняем >500 записей, проверяем что append_live_entry обрезает
        let mut entries = Vec::new();
        for i in 0..600 {
            entries.push(LogEntry::new_system_event(
                "i1",
                "s1",
                &format!("event{}", i),
            ));
        }
        replace_entries(entries);

        // Добавляем ещё одну
        append_live_entry(LogEntry::new_system_event("i1", "s1", "extra"));

        let current = get_entries();
        assert_eq!(
            current.len(),
            500,
            "should cap at 500 after append_live_entry"
        );
    }

    #[test]
    fn test_status_color_variants() {
        assert_eq!(status_color("Success"), "#6a9955");
        assert_eq!(status_color("Failed"), "#f14c4c");
        assert_eq!(status_color("Cancelled"), "#d7ba7d");
        assert_eq!(status_color("Unknown"), "#888");
    }

    #[test]
    fn test_kind_color_variants() {
        assert_eq!(kind_color("AgentTurn"), "#569cd6");
        assert_eq!(kind_color("CommandRun"), "#ce9178");
        assert_eq!(kind_color("ScriptRun"), "#6a9955");
        assert_eq!(kind_color("SystemEvent"), "#9cdcfe");
        assert_eq!(kind_color("Other"), "#888");
    }
}
