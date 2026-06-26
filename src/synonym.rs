// Phase 3: Словарь синонимов.
// Отображает нормализованный запрос пользователя → ScriptId.
// Автоматически пополняется из успешных LLM-запросов.

use crate::types::RiskLevel;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Тип совпадения при поиске синонима.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    /// Точное совпадение (normalized).
    Exact,
    /// Префиксное совпадение.
    Prefix,
    /// Bag-of-words совпадение (перестановка слов).
    BagOfWords,
}

impl MatchKind {
    /// Человекочитаемое имя.
    pub fn name(&self) -> &'static str {
        match self {
            MatchKind::Exact => "exact",
            MatchKind::Prefix => "prefix",
            MatchKind::BagOfWords => "bag-of-words",
        }
    }
}

/// Запись синонима: отображает пользовательский запрос на script_id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynonymEntry {
    /// ID скрипта, который обрабатывает этот запрос
    pub script_id: String,
    /// Оригинальный запрос пользователя (не нормализованный)
    pub original_query: String,
    /// Сколько раз этот синоним был использован
    pub frequency: u64,
    /// ISO timestamp последнего использования
    pub last_used: String,
    /// ISO timestamp создания
    pub created_at: String,
}

/// Индекс синонимов: нормализованный запрос → SynonymEntry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynonymIndex {
    version: u32,
    entries: HashMap<String, SynonymEntry>,
    #[serde(skip)]
    path: PathBuf,
}

impl SynonymIndex {
    /// Загрузить индекс из стандартной локации ~/.terio/synonyms.json.
    pub fn load_default() -> Result<Self> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("HOME not set")?;
        let path = PathBuf::from(home).join(".terio").join("synonyms.json");
        Self::load(&path)
    }

    /// Загрузить индекс из файла.
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let data = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read synonyms from {:?}", path))?;
            let mut index: SynonymIndex = serde_json::from_str(&data)
                .with_context(|| format!("Failed to parse synonyms from {:?}", path))?;
            index.path = path.to_path_buf();
            Ok(index)
        } else {
            // Создаём родительскую директорию и пустой индекс
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            Ok(SynonymIndex {
                version: 1,
                entries: HashMap::new(),
                path: path.to_path_buf(),
            })
        }
    }

    /// Сохранить индекс на диск.
    pub fn save(&self) -> Result<()> {
        let data = serde_json::to_string_pretty(&self)?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, &data)
            .with_context(|| format!("Failed to write synonyms to {:?}", self.path))
    }

    /// Нормализовать запрос: lowercase + collapse whitespace.
    /// Порядок слов сохраняется (для prefix matching).
    pub fn normalize(query: &str) -> String {
        query
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
            .to_lowercase()
    }

    /// Нормализовать запрос с сортировкой слов (bag-of-words инвариантность).
    fn normalize_bag(query: &str) -> String {
        let mut words: Vec<&str> = query.split_whitespace().collect();
        words.sort();
        words.join(" ").to_lowercase()
    }
}

/// Тип совпадения при поиске.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LookupResult<'a> {
    /// Как было найдено.
    pub match_kind: MatchKind,
    /// Запись синонима.
    pub entry: &'a SynonymEntry,
}

impl SynonymIndex {
    /// Найти script_id по запросу.
    /// 1) Точное совпадение (normalized) — Exact.
    /// 2) Prefix match: запрос начинается с ключа — Prefix.
    /// 3) Bag-of-words: слова совпадают независимо от порядка — BagOfWords.
    pub fn lookup(&self, query: &str) -> Option<LookupResult<'_>> {
        let normalized = Self::normalize(query);

        // 1. Точное совпадение
        if let Some(entry) = self.entries.get(&normalized) {
            return Some(LookupResult {
                match_kind: MatchKind::Exact,
                entry,
            });
        }

        // 2. Prefix match: проверяем от более длинных к более коротким (specificity)
        let mut prefix_candidates: Vec<(&String, &SynonymEntry)> = self
            .entries
            .iter()
            .filter(|(norm_key, _)| {
                *norm_key != &normalized
                    && (normalized.starts_with(&format!("{} ", *norm_key))
                        || normalized.starts_with(&format!("{}/", *norm_key))
                        || normalized.starts_with(&format!("{}-", *norm_key)))
            })
            .collect();
        // Сортируем: сначала более длинные ключи
        prefix_candidates.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
        if let Some((_, entry)) = prefix_candidates.into_iter().next() {
            return Some(LookupResult {
                match_kind: MatchKind::Prefix,
                entry,
            });
        }

        // 3. Bag-of-words match
        let query_bag = Self::normalize_bag(query);
        for (norm_key, entry) in &self.entries {
            let key_bag = Self::normalize_bag(norm_key);
            if query_bag == key_bag {
                return Some(LookupResult {
                    match_kind: MatchKind::BagOfWords,
                    entry,
                });
            }
        }

        None
    }

    /// Требуется ли подтверждение для данного совпадения.
    /// Для Exact — не требуется (безопасно).
    /// Для Prefix/BagOfWords — требуется, если скрипт не ReadOnly.
    pub fn requires_confirmation(lookup: &LookupResult<'_>, risk: &RiskLevel) -> bool {
        match lookup.match_kind {
            MatchKind::Exact => false,
            MatchKind::Prefix | MatchKind::BagOfWords => {
                !matches!(risk, RiskLevel::ReadOnly | RiskLevel::NetworkRead)
            }
        }
    }

    /// Добавить или обновить синоним.
    /// Возвращает true если создан новый, false если обновлён существующий.
    pub fn add(&mut self, query: &str, script_id: &str) -> bool {
        let normalized = Self::normalize(query);
        let now = chrono::Utc::now().to_rfc3339();

        if let Some(entry) = self.entries.get_mut(&normalized) {
            entry.frequency += 1;
            entry.last_used = now;
            entry.original_query = query.trim().to_string();
            entry.script_id = script_id.to_string();
            false
        } else {
            self.entries.insert(
                normalized,
                SynonymEntry {
                    script_id: script_id.to_string(),
                    original_query: query.trim().to_string(),
                    frequency: 1,
                    last_used: now.clone(),
                    created_at: now,
                },
            );
            true
        }
    }

    /// Удалить синоним по запросу (точное совпадение).
    pub fn remove(&mut self, query: &str) -> bool {
        let normalized = Self::normalize(query);
        self.entries.remove(&normalized).is_some()
    }

    /// Получить все записи.
    pub fn entries(&self) -> &HashMap<String, SynonymEntry> {
        &self.entries
    }

    /// Удалить синонимы с частотой ниже порога.
    /// Возвращает количество удалённых.
    pub fn prune(&mut self, min_frequency: u64) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|_, entry| entry.frequency >= min_frequency);
        before - self.entries.len()
    }

    /// Количество записей.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Пуст ли индекс.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_index() -> SynonymIndex {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("synonyms.json");
        SynonymIndex::load(&path).unwrap()
    }

    #[test]
    fn test_normalize_lowercases_and_trims() {
        let norm = SynonymIndex::normalize("File List");
        assert_eq!(norm, "file list");

        let norm2 = SynonymIndex::normalize("  LIST   FILES  ");
        assert_eq!(norm2, "list files");
    }

    #[test]
    fn test_normalize_bag() {
        let bag = SynonymIndex::normalize_bag("list files");
        assert_eq!(bag, "files list");

        let bag2 = SynonymIndex::normalize_bag("files list");
        assert_eq!(bag2, "files list");
    }

    #[test]
    fn test_lookup_exact_match() {
        let mut idx = make_index();
        idx.add("list files", "ls_script");
        let result = idx.lookup("list files").unwrap();
        assert_eq!(result.match_kind, MatchKind::Exact);
        assert_eq!(result.entry.script_id, "ls_script");
    }

    #[test]
    fn test_lookup_case_insensitive() {
        let mut idx = make_index();
        idx.add("list files", "ls_script");
        let result = idx.lookup("LIST FILES").unwrap();
        assert_eq!(result.match_kind, MatchKind::Exact);
        assert_eq!(result.entry.script_id, "ls_script");
    }

    #[test]
    fn test_lookup_word_order_invariant() {
        let mut idx = make_index();
        idx.add("list files", "ls_script");
        let result = idx.lookup("files list").unwrap();
        assert_eq!(result.match_kind, MatchKind::BagOfWords);
        assert_eq!(result.entry.script_id, "ls_script");
    }

    #[test]
    fn test_lookup_prefix_match() {
        let mut idx = make_index();
        idx.add("list files", "ls_script");
        let result = idx.lookup("list files in /tmp").unwrap();
        assert_eq!(result.match_kind, MatchKind::Prefix);
        assert_eq!(result.entry.script_id, "ls_script");
    }

    #[test]
    fn test_lookup_no_match() {
        let idx = make_index();
        assert!(idx.lookup("unknown query").is_none());
    }

    #[test]
    fn test_add_updates_frequency() {
        let mut idx = make_index();
        idx.add("hello", "greet");
        assert_eq!(idx.entries().len(), 1);
        assert_eq!(idx.entries().values().next().unwrap().frequency, 1);

        idx.add("hello", "greet");
        assert_eq!(idx.entries().len(), 1);
        assert_eq!(idx.entries().values().next().unwrap().frequency, 2);
    }

    #[test]
    fn test_remove() {
        let mut idx = make_index();
        idx.add("list files", "ls_script");
        assert!(idx.remove("list files"));
        assert!(idx.lookup("list files").is_none());
        assert!(!idx.remove("nonexistent"));
    }

    #[test]
    fn test_prune() {
        let mut idx = make_index();
        idx.add("frequent", "s1");
        idx.add("rare", "s2");
        // Add "frequent" again
        idx.add("frequent", "s1");
        let removed = idx.prune(2);
        assert_eq!(removed, 1);
        assert!(idx.lookup("frequent").is_some());
        assert!(idx.lookup("rare").is_none());
    }

    #[test]
    fn test_save_and_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("synonyms.json");
        {
            let mut idx = SynonymIndex::load(&path).unwrap();
            idx.add("list files", "ls_script");
            idx.save().unwrap();
        }
        {
            let idx = SynonymIndex::load(&path).unwrap();
            let result = idx.lookup("list files").unwrap();
            assert_eq!(result.entry.script_id, "ls_script");
        }
    }

    #[test]
    fn test_lookup_returns_highest_frequency_in_prefix() {
        let mut idx = make_index();
        idx.add("list", "list_all");
        idx.add("list files", "ls_script");
        // "list files in /tmp" should match "list files" first (more specific)
        let result = idx.lookup("list files in /tmp").unwrap();
        assert_eq!(result.match_kind, MatchKind::Prefix);
        assert_eq!(result.entry.script_id, "ls_script");
    }
}
