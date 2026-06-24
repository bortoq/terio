// Script Cache: хранение и поиск кешированных цепочек команд.

use crate::matcher::{hash_normalized, normalize};
use crate::types::RiskLevel;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Одна команда в кеше.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedStep {
    pub command: String,
    pub argv: Vec<String>,
    pub risk: RiskLevel,
}

/// Scope выполнения скрипта (соответствует schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptScope {
    pub cwd_policy: String,
    pub cwd: String,
}

/// Запись в кеше скриптов.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub schema_version: u32,
    pub script_id: String,
    pub request_hash: String,
    pub version: u32,
    pub normalized_request: String,
    pub match_policy: String,
    pub scope: ScriptScope,
    pub risk: RiskLevel,
    pub parameters: serde_json::Value,
    pub preconditions: Vec<serde_json::Value>,
    pub steps: Vec<CachedStep>,
    pub artifacts: Vec<serde_json::Value>,
    pub success_count: u32,
    pub trust_threshold: u32,
    pub created_at: String,
    pub last_used_at: String,
}

/// Script Cache.
pub struct ScriptCache {
    dir: PathBuf,
}

impl ScriptCache {
    /// Создаёт кеш в `~/.terio/cache/`.
    pub fn new() -> Result<Self> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("HOME not set")?;
        let dir = PathBuf::from(home).join(".terio").join("cache");
        std::fs::create_dir_all(&dir).context("failed to create cache dir")?;
        Ok(Self { dir })
    }

    /// Поиск по нормализованному запросу.
    pub fn lookup(&self, request: &str) -> Result<Option<CacheEntry>> {
        let normalized = normalize(request);
        let hash = hash_normalized(&normalized);
        let path = self.dir.join(format!("{}.json", hash));

        if !path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read cache entry: {}", path.display()))?;
        let entry: CacheEntry =
            serde_json::from_str(&content).context("invalid cache entry JSON")?;

        // Проверяем match_policy
        if entry.match_policy != "exact_normalized" {
            return Ok(None);
        }

        Ok(Some(entry))
    }

    /// Сохраняет шаги в кеш.
    pub fn save(
        &self,
        request: &str,
        risk: RiskLevel,
        steps: Vec<CachedStep>,
    ) -> Result<CacheEntry> {
        let normalized = normalize(request);
        let hash = hash_normalized(&normalized);
        let path = self.dir.join(format!("{}.json", hash));

        let now = iso_now();
        let script_id = hash.clone(); // в MVP script_id = hash содержимого
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let entry = CacheEntry {
            schema_version: 1,
            script_id,
            request_hash: hash.clone(),
            version: 1,
            normalized_request: normalized,
            match_policy: "exact_normalized".to_string(),
            scope: ScriptScope {
                cwd_policy: "same_cwd_only".to_string(),
                cwd,
            },
            risk,
            parameters: serde_json::json!({}),
            preconditions: vec![],
            steps,
            artifacts: vec![],
            success_count: 1,
            trust_threshold: 3,
            created_at: now.clone(),
            last_used_at: now,
        };

        let json =
            serde_json::to_string_pretty(&entry).context("failed to serialize cache entry")?;
        std::fs::write(&path, json)
            .with_context(|| format!("failed to write cache entry: {}", path.display()))?;

        Ok(entry)
    }

    /// Обновляет success_count после успешного выполнения из кеша.
    pub fn increment_success(&self, request_hash: &str) -> Result<()> {
        let path = self.dir.join(format!("{}.json", request_hash));
        if !path.exists() {
            return Ok(());
        }
        let content = std::fs::read_to_string(&path)?;
        let mut entry: CacheEntry =
            serde_json::from_str(&content).context("invalid cache entry JSON")?;
        entry.success_count += 1;
        entry.last_used_at = iso_now();
        let json = serde_json::to_string_pretty(&entry)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Проверяет, можно ли auto-run.
    pub fn can_auto_run(entry: &CacheEntry) -> bool {
        entry.success_count >= entry.trust_threshold
            && entry.risk != RiskLevel::Destructive
            && entry.risk != RiskLevel::NetworkWrite
            && entry.risk != RiskLevel::CredentialAccess
            && entry.risk != RiskLevel::Financial
    }
}

fn iso_now() -> String {
    // Используем SystemTime вместо chrono для простоты
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Формат: 2026-06-24T12:00:00Z
    let datetime = chrono::DateTime::from_timestamp(secs as i64, 0).unwrap_or_default();
    datetime.to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_lookup_miss() {
        let dir = TempDir::new().unwrap();
        let cache = ScriptCache {
            dir: dir.path().to_path_buf(),
        };
        let result = cache.lookup("nonexistent request").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_save_and_lookup() {
        let dir = TempDir::new().unwrap();
        let cache = ScriptCache {
            dir: dir.path().to_path_buf(),
        };

        let steps = vec![CachedStep {
            command: "ls".to_string(),
            argv: vec!["ls".to_string(), "-l".to_string()],
            risk: RiskLevel::ReadOnly,
        }];

        let saved = cache
            .save("list files", RiskLevel::ReadOnly, steps.clone())
            .unwrap();
        assert_eq!(saved.normalized_request, "list files");

        let found = cache.lookup("list files").unwrap();
        assert!(found.is_some());
        assert_eq!(found.as_ref().unwrap().steps.len(), 1);
        assert_eq!(found.as_ref().unwrap().steps[0].command, "ls");
    }

    #[test]
    fn test_cache_normalized_match() {
        let dir = TempDir::new().unwrap();
        let cache = ScriptCache {
            dir: dir.path().to_path_buf(),
        };

        let steps = vec![CachedStep {
            command: "ls".to_string(),
            argv: vec!["ls".to_string(), "-l".to_string()],
            risk: RiskLevel::ReadOnly,
        }];

        cache
            .save("list files", RiskLevel::ReadOnly, steps)
            .unwrap();

        // Должен найти по не совсем точному запросу
        let found = cache.lookup("  List   Files  ").unwrap();
        assert!(found.is_some());
    }

    #[test]
    fn test_increment_success() {
        let dir = TempDir::new().unwrap();
        let cache = ScriptCache {
            dir: dir.path().to_path_buf(),
        };

        let steps = vec![CachedStep {
            command: "echo".to_string(),
            argv: vec!["echo".to_string(), "hi".to_string()],
            risk: RiskLevel::ReadOnly,
        }];

        let saved = cache.save("say hi", RiskLevel::ReadOnly, steps).unwrap();
        assert_eq!(saved.success_count, 1);

        cache.increment_success(&saved.request_hash).unwrap();

        let entry = cache.lookup("say hi").unwrap().unwrap();
        assert_eq!(entry.success_count, 2);
    }

    #[test]
    fn test_can_auto_run() {
        let mut entry = CacheEntry {
            schema_version: 1,
            script_id: "s1".to_string(),
            request_hash: "h1".to_string(),
            version: 1,
            normalized_request: "test".to_string(),
            match_policy: "exact_normalized".to_string(),
            scope: ScriptScope {
                cwd_policy: "same_cwd_only".to_string(),
                cwd: "/tmp".to_string(),
            },
            risk: RiskLevel::ReadOnly,
            parameters: serde_json::json!({}),
            preconditions: vec![],
            steps: vec![],
            artifacts: vec![],
            success_count: 2,
            trust_threshold: 3,
            created_at: "now".to_string(),
            last_used_at: "now".to_string(),
        };
        assert!(!ScriptCache::can_auto_run(&entry));

        entry.success_count = 3;
        assert!(ScriptCache::can_auto_run(&entry));

        entry.risk = RiskLevel::Destructive;
        assert!(!ScriptCache::can_auto_run(&entry));
    }
}
