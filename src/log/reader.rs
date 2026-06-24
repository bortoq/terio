// JsonlLogReader — чтение лога из JSONL-файла.

use crate::log::LogReader;
use crate::types::LogEntry;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::sync::broadcast;

pub struct JsonlLogReader {
    dir: PathBuf,
    /// Для MVP: храним broadcast sender, чтобы можно было подписаться.
    /// Реальные сообщения приходят от LogStore, а не от reader.
    broadcaster: broadcast::Sender<LogEntry>,
}

impl JsonlLogReader {
    pub fn new(dir: &Path) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            dir: dir.to_path_buf(),
            broadcaster: tx,
        }
    }

    /// Найти все лог-файлы в директории (по убыванию времени).
    fn log_files(&self) -> Result<Vec<PathBuf>> {
        let mut files: Vec<PathBuf> = Vec::new();
        if !self.dir.exists() {
            return Ok(files);
        }
        for entry in std::fs::read_dir(&self.dir).context("read log dir")? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                files.push(path);
            }
        }
        // Сортировка: сначала новые
        files.sort();
        files.reverse();
        Ok(files)
    }

    /// Прочитать все записи из файла.
    fn read_file(path: &Path) -> Result<Vec<LogEntry>> {
        let content = std::fs::read_to_string(path)?;
        let mut entries = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<LogEntry>(line) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    eprintln!("warning: skipping invalid log line: {e}");
                }
            }
        }
        Ok(entries)
    }
}

impl LogReader for JsonlLogReader {
    fn recent(&self, n: usize) -> Result<Vec<LogEntry>> {
        let files = self.log_files()?;
        let mut entries = Vec::new();
        for file in files {
            let file_entries = Self::read_file(&file)?;
            entries.extend(file_entries);
            if entries.len() >= n {
                break;
            }
        }
        entries.truncate(n);
        Ok(entries)
    }

    fn by_session(&self, session_id: &str) -> Result<Vec<LogEntry>> {
        let files = self.log_files()?;
        let mut entries = Vec::new();
        for file in files {
            for entry in Self::read_file(&file)? {
                if entry.session_id == session_id {
                    entries.push(entry);
                }
            }
        }
        Ok(entries)
    }

    fn by_interaction(&self, interaction_id: &str) -> Result<Vec<LogEntry>> {
        let files = self.log_files()?;
        let mut entries = Vec::new();
        for file in files {
            for entry in Self::read_file(&file)? {
                if entry.interaction_id.as_deref() == Some(interaction_id) {
                    entries.push(entry);
                }
            }
        }
        Ok(entries)
    }

    fn stream(&self) -> broadcast::Receiver<LogEntry> {
        self.broadcaster.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::writer::JsonlLogWriter;
    use crate::log::LogWriter;
    use crate::types::*;
    use tempfile::TempDir;

    fn make_entry(iid: &str, sid: &str, interaction: &str) -> LogEntry {
        LogEntry::new_command_run(
            iid,
            sid,
            Some(interaction.to_string()),
            "test",
            "/tmp",
            &["echo".into(), "hi".into()],
            0,
            std::time::Duration::from_millis(1),
            "hi",
            "",
            CostCounters::default(),
        )
    }

    #[test]
    fn test_reader_recent() {
        let dir = TempDir::new().unwrap();
        let writer = JsonlLogWriter::new(dir.path()).unwrap();
        writer.append(make_entry("i1", "s1", "int1")).unwrap();
        writer.append(make_entry("i2", "s2", "int2")).unwrap();
        writer.flush().unwrap();

        let reader = JsonlLogReader::new(dir.path());
        let recent = reader.recent(10).unwrap();
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn test_reader_by_interaction() {
        let dir = TempDir::new().unwrap();
        let writer = JsonlLogWriter::new(dir.path()).unwrap();
        writer.append(make_entry("i1", "s1", "abc")).unwrap();
        writer.append(make_entry("i2", "s2", "xyz")).unwrap();
        writer.flush().unwrap();

        let reader = JsonlLogReader::new(dir.path());
        let entries = reader.by_interaction("abc").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].instance_id, "i1");
    }

    #[test]
    fn test_reader_empty_dir() {
        let dir = TempDir::new().unwrap();
        let empty_sub = dir.path().join("empty_log");
        std::fs::create_dir(&empty_sub).unwrap();
        let reader = JsonlLogReader::new(&empty_sub);
        assert!(reader.recent(10).unwrap().is_empty());
    }
}
