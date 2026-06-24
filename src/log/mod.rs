// LogWriter / LogReader traits + LogStore

pub mod reader;
pub mod writer;

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
    /// Подписка на in-memory stream.
    fn stream(&self) -> broadcast::Receiver<LogEntry>;
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

    /// Добавить запись: validate → writer.append → broadcast.
    pub fn append(&self, entry: LogEntry) -> Result<()> {
        // validate (MVP: minimal — просто десериализация)
        let _: serde_json::Value = serde_json::to_value(&entry)
            .map_err(|e| anyhow::anyhow!("serialization failed: {e}"))?;

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
