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
    if let Some(ref mut cmd) = entry.command {
        let display = redact(&cmd.display);
        let argv: Vec<String> = cmd.argv.iter().map(|a| redact(a)).collect();
        entry.command = Some(crate::types::CommandInfo { display, argv });
    }
    entry
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

        store.append(entry).unwrap();

        let recent = store.recent(10).unwrap();
        assert_eq!(recent.len(), 1);
        let e = &recent[0];
        assert!(e.request.as_deref().unwrap().contains("[REDACTED]"));
        assert!(e.stdout_summary.as_deref().unwrap().contains("[REDACTED]"));
        assert!(!e.request.as_deref().unwrap().contains("secret123"));
    }
}
