// JsonlLogWriter — запись лога в JSONL-файл.

use crate::log::LogWriter;
use crate::types::LogEntry;
use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct JsonlLogWriter {
    file: Mutex<fs::File>,
    #[allow(dead_code)]
    path: PathBuf,
}

impl JsonlLogWriter {
    /// Создаёт writer в директории `dir/terio-YYYY-MM.jsonl`.
    pub fn new(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir).context("failed to create log dir")?;
        let path = Self::log_path(dir);
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("failed to open log: {}", path.display()))?;
        Ok(Self {
            file: Mutex::new(file),
            path,
        })
    }

    /// Путь к лог-файлу для текущего месяца.
    fn log_path(dir: &Path) -> PathBuf {
        let now = chrono::Utc::now();
        let filename = format!("terio-{}.jsonl", now.format("%Y-%m"));
        dir.join(filename)
    }

    /// Директория лога по умолчанию: ~/.terio/log/
    pub fn default_dir() -> Result<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("HOME not set")?;
        Ok(PathBuf::from(home).join(".terio").join("log"))
    }
}

impl LogWriter for JsonlLogWriter {
    fn append(&self, entry: LogEntry) -> Result<()> {
        let line = serde_json::to_string(&entry)?;
        let mut file = self.file.lock().unwrap();
        writeln!(file, "{line}")?;
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        let mut file = self.file.lock().unwrap();
        file.flush()?;
        if let Err(e) = file.sync_all() {
            eprintln!("sync_all warning: {e}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use tempfile::TempDir;

    fn make_entry(instance_id: &str, session_id: &str) -> LogEntry {
        LogEntry::new_command_run(
            instance_id,
            session_id,
            Some("inter1".into()),
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
    fn test_jsonl_writer_append() {
        let dir = TempDir::new().unwrap();
        let writer = JsonlLogWriter::new(dir.path()).unwrap();
        let entry = make_entry("inst1", "sess1");
        writer.append(entry).unwrap();
        writer.flush().unwrap();

        let path = JsonlLogWriter::log_path(dir.path());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("inst1"));
        assert!(content.contains("sess1"));
    }

    #[test]
    fn test_jsonl_writer_appends() {
        let dir = TempDir::new().unwrap();
        let writer = JsonlLogWriter::new(dir.path()).unwrap();
        writer.append(make_entry("i1", "s1")).unwrap();
        writer.append(make_entry("i2", "s2")).unwrap();
        writer.flush().unwrap();

        let path = JsonlLogWriter::log_path(dir.path());
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_log_path_format() {
        let dir = Path::new("/tmp/test");
        let path = JsonlLogWriter::log_path(dir);
        let name = path.file_name().unwrap().to_str().unwrap();
        // terio-2026-06.jsonl
        assert!(name.starts_with("terio-"));
        assert!(name.ends_with(".jsonl"));
        assert_eq!(name.len(), 19); // "terio-YYYY-MM.jsonl"
    }
}
