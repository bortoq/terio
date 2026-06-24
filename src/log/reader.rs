// JsonlLogReader — чтение лога из JSONL-файла.

use crate::log::LogReader;
use crate::types::LogEntry;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub struct JsonlLogReader {
    dir: PathBuf,
}

impl JsonlLogReader {
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }

    /// Найти все лог-файлы в директории (по убыванию времени)
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
        // Сортировка: сначала новые файлы
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

    /// Прочитать последние N записей из одного файла (reverse search).
    fn read_last_n(path: &Path, n: usize) -> Result<Vec<LogEntry>> {
        let content = std::fs::read_to_string(path)?;
        let mut entries = Vec::new();
        for line in content.lines().rev() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<LogEntry>(line) {
                Ok(entry) => {
                    entries.push(entry);
                    if entries.len() >= n {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("warning: skipping invalid log line: {e}");
                }
            }
        }
        entries.reverse(); // восстановить хронологический порядок
        Ok(entries)
    }
}

impl LogReader for JsonlLogReader {
    fn recent(&self, n: usize) -> Result<Vec<LogEntry>> {
        let files = self.log_files()?;
        if files.is_empty() {
            return Ok(Vec::new());
        }
        // Берём из самого нового файла
        let newest = &files[0];
        let mut entries = Self::read_last_n(newest, n)?;

        // Если не хватило — читаем из предыдущих файлов
        if entries.len() < n {
            let mut need = n - entries.len();
            for file in &files[1..] {
                let older = Self::read_last_n(file, need)?;
                need -= older.len();
                // Старые записи идут раньше новых
                let mut combined = older;
                combined.extend(entries);
                entries = combined;
                if need == 0 {
                    break;
                }
            }
        }

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::writer::JsonlLogWriter;
    use crate::log::LogWriter;
    use crate::types::*;
    use tempfile::TempDir;

    fn make_entry(instance_id: &str, counter: usize) -> LogEntry {
        LogEntry::new_command_run(
            instance_id,
            "sess1",
            Some(format!("int{counter}")),
            "test",
            "/tmp",
            &["echo".into(), counter.to_string().into()],
            0,
            std::time::Duration::from_millis(1),
            &counter.to_string(),
            "",
            CostCounters::default(),
        )
    }

    #[test]
    fn test_reader_recent_returns_latest() {
        let dir = TempDir::new().unwrap();
        let writer = JsonlLogWriter::new(dir.path()).unwrap();
        // Пишем 100 записей
        for i in 0..100 {
            writer.append(make_entry("i1", i)).unwrap();
        }
        writer.flush().unwrap();

        let reader = JsonlLogReader::new(dir.path());
        let recent = reader.recent(3).unwrap();
        assert_eq!(recent.len(), 3);
        // Последние 3 записи в хронологическом порядке: 97, 98, 99
        assert_eq!(recent[0].command.as_ref().unwrap().display, "echo 97");
        assert_eq!(recent[1].command.as_ref().unwrap().display, "echo 98");
        assert_eq!(recent[2].command.as_ref().unwrap().display, "echo 99");
    }

    #[test]
    fn test_reader_recent_less_than_total() {
        let dir = TempDir::new().unwrap();
        let writer = JsonlLogWriter::new(dir.path()).unwrap();
        for i in 0..5 {
            writer.append(make_entry("i1", i)).unwrap();
        }
        writer.flush().unwrap();

        let reader = JsonlLogReader::new(dir.path());
        let recent = reader.recent(10).unwrap();
        assert_eq!(recent.len(), 5);
    }

    #[test]
    fn test_reader_by_interaction() {
        let dir = TempDir::new().unwrap();
        let writer = JsonlLogWriter::new(dir.path()).unwrap();
        writer.append(make_entry("i1", 0)).unwrap();
        writer.append(make_entry("i2", 1)).unwrap();
        writer.flush().unwrap();

        let reader = JsonlLogReader::new(dir.path());
        let entries = reader.by_interaction("int0").unwrap();
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
