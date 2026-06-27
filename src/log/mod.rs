// LogWriter / LogReader traits + LogStore

pub mod reader;
pub mod writer;

use crate::redact::redact;
use crate::types::LogEntry;
use anyhow::Result;
use tokio::sync::broadcast;

pub use reader::JsonlLogReader;
pub use writer::JsonlLogWriter;

/// Writer: запись лога.
pub trait LogWriter: Send + Sync {
    /// Добавить запись (validate → redact → write → broadcast).
    fn append(&self, entry: LogEntry) -> Result<()>;
    /// fsync на диск.
    fn flush(&self) -> Result<()>;
}

/// Reader: чтение лога.
pub trait LogReader: Send + Sync {
    /// Последние N записей.
    fn recent(&self, n: usize) -> Result<Vec<LogEntry>>;
    /// Записи по session_id.
    fn by_session(&self, session_id: &str) -> Result<Vec<LogEntry>>;
    /// Записи по interaction_id.
    fn by_interaction(&self, interaction_id: &str) -> Result<Vec<LogEntry>>;
    /// Последние N событий (сгруппированные LogEvent).
    fn recent_events(&self, n: usize) -> Result<Vec<crate::types::LogEvent>> {
        let entries = self.recent(n)?;
        Ok(crate::types::LogEvent::group_entries(&entries))
    }
}

/// LogStore — объединяет writer + reader + broadcast.
pub struct LogStore {
    writer: Box<dyn LogWriter>,
    reader: Box<dyn LogReader>,
    broadcaster: broadcast::Sender<LogEntry>,
}

impl LogStore {
    /// Создаёт LogStore с заданной ёмкостью broadcast.
    pub fn new(writer: Box<dyn LogWriter>, reader: Box<dyn LogReader>, capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            writer,
            reader,
            broadcaster: tx,
        }
    }

    /// Добавить запись: redact → writer.append → broadcast.
    pub fn append(&self, entry: LogEntry) -> Result<()> {
        let entry = apply_redaction(entry);
        self.writer.append(entry.clone())?;
        let _ = self.broadcaster.send(entry);
        Ok(())
    }

    /// fsync.
    pub fn flush(&self) -> Result<()> {
        self.writer.flush()
    }

    /// Последние N записей.
    pub fn recent(&self, n: usize) -> Result<Vec<LogEntry>> {
        self.reader.recent(n)
    }

    pub fn recent_events(&self, n: usize) -> Result<Vec<crate::types::LogEvent>> {
        self.reader.recent_events(n)
    }

    /// Подписка на in-memory stream.
    pub fn stream(&self) -> broadcast::Receiver<LogEntry> {
        self.broadcaster.subscribe()
    }
}

/// Применяет redaction ко всем текстовым полям LogEntry.
fn apply_redaction(mut entry: LogEntry) -> LogEntry {
    if let Some(ref mut s) = entry.request {
        *s = redact(s);
    }
    if let Some(ref mut s) = entry.cwd {
        *s = redact(s);
    }
    if let Some(ref mut s) = entry.prompt_summary {
        *s = redact(s);
    }
    if let Some(ref mut s) = entry.stdout_summary {
        *s = redact(s);
    }
    if let Some(ref mut s) = entry.stderr_summary {
        *s = redact(s);
    }
    if let Some(ref mut s) = entry.description {
        *s = redact(s);
    }
    if let Some(ref mut plan) = entry.plan {
        redact_json_value(plan);
    }
    if let Some(ref mut cmd) = entry.command {
        let display = redact(&cmd.display);
        let argv: Vec<String> = cmd.argv.iter().map(|a| redact(a)).collect();
        entry.command = Some(crate::types::CommandInfo { display, argv });
    }
    if let Some(ref mut steps) = entry.steps {
        for step in steps {
            step.command = redact(&step.command);
            step.argv = step.argv.iter().map(|a| redact(a)).collect();
        }
    }
    entry
}

fn redact_json_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(s) => {
            *s = redact(s);
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_json_value(item);
            }
        }
        serde_json::Value::Object(map) => {
            for value in map.values_mut() {
                redact_json_value(value);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::writer::JsonlLogWriter;
    use crate::types::*;
    use tempfile::TempDir;

    #[test]
    fn test_redaction_applied() {
        let dir = TempDir::new().unwrap();
        let writer = Box::new(JsonlLogWriter::new(dir.path()).unwrap());
        let reader = Box::new(JsonlLogReader::new(dir.path()));
        let store = LogStore::new(writer, reader, 16);

        let mut entry = LogEntry::new_command_run(
            "i1",
            "s1",
            Some("int1".into()),
            "api_key=secret123",
            "/tmp",
            &["echo".into(), "api_key=secret123".into()],
            0,
            std::time::Duration::from_millis(1),
            "api_key=secret123",
            "",
            CostCounters::default(),
        );
        entry.stdout_summary = Some("token: abc123".into());
        entry.plan = Some(serde_json::json!([{
            "argv": ["echo", "api_key=secret123"]
        }]));
        entry.steps = Some(vec![crate::types::StepInfo {
            command: "echo".into(),
            argv: vec!["echo".into(), "token=abc123".into()],
            exit: 0,
        }]);

        store.append(entry).unwrap();

        let recent = store.recent(10).unwrap();
        assert_eq!(recent.len(), 1);
        let e = &recent[0];
        assert!(e.request.as_deref().unwrap().contains("[REDACTED]"));
        assert!(e.stdout_summary.as_deref().unwrap().contains("[REDACTED]"));
        assert!(!e.request.as_deref().unwrap().contains("secret123"));
        assert!(e.plan.as_ref().unwrap().to_string().contains("[REDACTED]"));
        assert!(e.steps.as_ref().unwrap()[0]
            .argv
            .join(" ")
            .contains("[REDACTED]"));
    }

    #[test]
    fn test_command_run_entry_has_required_schema_fields() {
        let entry = LogEntry::new_command_run(
            "i1",
            "s1",
            Some("int1".into()),
            "echo hello",
            "/tmp",
            &["echo".into(), "hello".into()],
            0,
            std::time::Duration::from_millis(1),
            "hello",
            "",
            CostCounters::default(),
        );

        let instance = serde_json::to_value(entry).unwrap();
        assert_eq!(instance["schema_version"], 1);
        assert_eq!(instance["kind"], "command_run");
        assert_eq!(instance["request"], "echo hello");
        assert!(instance["cost_counters"].is_object());
        assert!(instance["command"]["argv"].is_array());
    }
}
