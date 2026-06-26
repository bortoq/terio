// Phase 4: Proactive mode — predict next command from history
//
// Uses a simple n-gram model: given the last request, predict the most
// likely next request based on transition frequencies from the log.

use crate::log::reader::JsonlLogReader;
use crate::log::LogReader;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Получить директорию данных (~/.terio).
fn data_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".terio")
}

const PREDICTION_FILE: &str = "prediction.json";

/// Результат предсказания.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    /// Предсказанный запрос.
    pub request: String,
    /// Уверенность 0.0..1.0.
    pub confidence: f64,
}

/// Проактивный движок: использует BayesianPredictor из accounting для предсказаний.
#[derive(Debug, Clone)]
pub struct ProactiveEngine {
    /// Байесовский предиктор.
    predictor: crate::accounting::BayesianPredictor,
    /// Количество авто-выполненных предсказаний.
    auto_executed: u64,
}

impl ProactiveEngine {
    pub fn new() -> Self {
        Self {
            predictor: crate::accounting::BayesianPredictor::new(),
            auto_executed: 0,
        }
    }

    /// Загрузить модель из лога (последние N записей).
    pub fn load_from_log(log_dir: &Path, max_entries: usize) -> anyhow::Result<Self> {
        let reader = JsonlLogReader::new(log_dir);
        let entries = reader.recent(max_entries)?;

        let mut predictor = crate::accounting::BayesianPredictor::new();
        let requests: Vec<String> = entries.iter().filter_map(|e| e.request.clone()).collect();
        predictor.observe(&requests);

        // Загружаем счётчик авто-выполненных
        let data_dir = data_dir();
        std::fs::create_dir_all(&data_dir).ok();
        let count_file = data_dir.join("auto_executed_count");
        let auto_executed = if count_file.exists() {
            std::fs::read_to_string(&count_file)
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0)
        } else {
            0
        };

        Ok(Self {
            predictor,
            auto_executed,
        })
    }

    /// Предсказать следующий запрос (делегирует BayesianPredictor).
    pub fn predict(&self, last_request: &str) -> Option<Prediction> {
        self.predictor
            .predict(last_request)
            .map(|(req, conf)| Prediction {
                request: req,
                confidence: conf,
            })
    }

    /// Записать запрос в историю для transition model.
    /// Сохраняет в файл ~/.terio/last_request.txt
    pub fn record_last_request(request: &str) -> anyhow::Result<()> {
        let data_dir = data_dir();
        std::fs::create_dir_all(&data_dir)?;
        std::fs::write(data_dir.join("last_request.txt"), request)?;
        Ok(())
    }

    /// Прочитать последний запрос.
    pub fn read_last_request() -> Option<String> {
        let data_dir = data_dir();
        let path = data_dir.join("last_request.txt");
        if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .map(|s| s.trim().to_string())
        } else {
            None
        }
    }

    /// Инкрементировать счётчик авто-выполненных команд.
    pub fn increment_auto_executed() -> anyhow::Result<u64> {
        let data_dir = data_dir();
        std::fs::create_dir_all(&data_dir)?;
        let count_file = data_dir.join("auto_executed_count");
        let current: u64 = if count_file.exists() {
            std::fs::read_to_string(&count_file)
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0)
        } else {
            0
        };
        let new_count = current + 1;
        std::fs::write(&count_file, new_count.to_string())?;
        Ok(new_count)
    }

    /// Получить счётчик авто-выполненных команд.
    pub fn auto_executed_count(&self) -> u64 {
        self.auto_executed
    }

    /// Сохранить предсказание для отображения в следующий раз.
    pub fn save_prediction(pred: &Prediction) -> anyhow::Result<()> {
        let data_dir = data_dir();
        std::fs::create_dir_all(&data_dir)?;
        let json = serde_json::to_string(pred)?;
        std::fs::write(data_dir.join(PREDICTION_FILE), json)?;
        Ok(())
    }

    /// Загрузить сохранённое предсказание.
    pub fn load_prediction() -> Option<Prediction> {
        let data_dir = data_dir();
        let path = data_dir.join(PREDICTION_FILE);
        if path.exists() {
            let content = std::fs::read_to_string(&path).ok()?;
            serde_json::from_str(&content).ok()
        } else {
            None
        }
    }

    /// Удалить сохранённое предсказание (после отображения).
    pub fn clear_prediction() -> anyhow::Result<()> {
        let data_dir = data_dir();
        let path = data_dir.join(PREDICTION_FILE);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}

impl Default for ProactiveEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::writer::JsonlLogWriter;
    use crate::log::LogWriter;
    use crate::types::*;
    use tempfile::TempDir;

    fn make_entry(request: &str, _counter: usize) -> LogEntry {
        LogEntry::new_command_run(
            "i1",
            "sess1",
            None,
            request,
            "/tmp",
            &["echo".into(), request.to_string().into()],
            0,
            std::time::Duration::from_millis(1),
            "ok",
            "",
            CostCounters::default(),
        )
    }

    #[test]
    fn test_predict_from_log() {
        let dir = TempDir::new().unwrap();
        let log_dir = dir.path().join("logs");
        std::fs::create_dir_all(&log_dir).unwrap();
        let writer = JsonlLogWriter::new(&log_dir).unwrap();

        // Write sequence: "list files", "show details", "list files", "show details", "edit config"
        writer.append(make_entry("list files", 0)).unwrap();
        writer.append(make_entry("show details", 1)).unwrap();
        writer.append(make_entry("list files", 2)).unwrap();
        writer.append(make_entry("show details", 3)).unwrap();
        writer.append(make_entry("edit config", 4)).unwrap();
        writer.flush().unwrap();

        let engine = ProactiveEngine::load_from_log(&log_dir, 100).unwrap();

        // After "list files", the most common next is "show details" (2/2)
        let pred = engine.predict("list files").unwrap();
        assert_eq!(pred.request, "show details");
        assert!((pred.confidence - 1.0).abs() < 0.01);

        // After "show details", next is "list files" (1/2) or "edit config" (1/2)
        let pred = engine.predict("show details").unwrap();
        assert!(pred.request == "list files" || pred.request == "edit config");
        assert!((pred.confidence - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_predict_no_data() {
        let engine = ProactiveEngine::new();
        assert!(engine.predict("anything").is_none());
    }

    #[test]
    fn test_record_and_read_last_request() {
        let dir = TempDir::new().unwrap();
        let old_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", dir.path().to_str().unwrap());

        ProactiveEngine::record_last_request("hello world").unwrap();
        let loaded = ProactiveEngine::read_last_request().unwrap();
        assert_eq!(loaded, "hello world");

        if let Some(h) = old_home {
            std::env::set_var("HOME", h);
        }
    }

    #[test]
    fn test_prediction_save_load_clear() {
        let dir = TempDir::new().unwrap();
        let old_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", dir.path().to_str().unwrap());

        let pred = Prediction {
            request: "list files".to_string(),
            confidence: 0.85,
        };
        ProactiveEngine::save_prediction(&pred).unwrap();
        let loaded = ProactiveEngine::load_prediction().unwrap();
        assert_eq!(loaded.request, "list files");
        assert!((loaded.confidence - 0.85).abs() < 0.01);

        ProactiveEngine::clear_prediction().unwrap();
        assert!(ProactiveEngine::load_prediction().is_none());

        if let Some(h) = old_home {
            std::env::set_var("HOME", h);
        }
    }

    #[test]
    fn test_increment_auto_executed() {
        let dir = TempDir::new().unwrap();
        let old_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", dir.path().to_str().unwrap());

        let c1 = ProactiveEngine::increment_auto_executed().unwrap();
        assert_eq!(c1, 1);
        let c2 = ProactiveEngine::increment_auto_executed().unwrap();
        assert_eq!(c2, 2);
        let c3 = ProactiveEngine::increment_auto_executed().unwrap();
        assert_eq!(c3, 3);

        if let Some(h) = old_home {
            std::env::set_var("HOME", h);
        }
    }
}
