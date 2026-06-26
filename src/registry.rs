// Phase 6: Community script registry
//
// Реестр скриптов сообщества — позволяет публиковать, искать и устанавливать
// скрипты из центрального репозитория (GitHub-based registry).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const REGISTRY_INDEX_URL: &str =
    "https://raw.githubusercontent.com/bortoq/terio-registry/main/index.json";

/// Метаданные скрипта в реестре.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryScript {
    /// Уникальный ID скрипта (slug).
    pub id: String,
    /// Название.
    pub name: String,
    /// Описание.
    pub description: String,
    /// Теги для поиска.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Автор.
    pub author: Option<String>,
    /// Версия.
    #[serde(default = "default_version")]
    pub version: String,
    /// URL для скачивания скрипта.
    pub download_url: String,
    /// SHA-256 хеш содержимого для проверки целостности.
    pub sha256: Option<String>,
    /// Уровень риска по умолчанию.
    #[serde(default)]
    pub risk: String,
}

fn default_version() -> String {
    "0.1.0".to_string()
}

/// Индекс реестра: все доступные скрипты.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryIndex {
    /// Версия формата индекса.
    pub version: u32,
    /// Скрипты в реестре.
    pub scripts: Vec<RegistryScript>,
}

/// Загрузить индекс реестра из GitHub.
pub fn fetch_registry_index() -> Result<RegistryIndex> {
    let resp = ureq::get(REGISTRY_INDEX_URL)
        .call()
        .context("failed to fetch registry index")?;

    let index: RegistryIndex = resp
        .into_body()
        .read_json()
        .context("failed to parse registry index")?;

    Ok(index)
}

/// Поиск скриптов в реестре по запросу.
pub fn search_registry(query: &str) -> Result<Vec<RegistryScript>> {
    let index = fetch_registry_index()?;
    let query_lower = query.to_lowercase();

    let mut results: Vec<RegistryScript> = index
        .scripts
        .into_iter()
        .filter(|s| {
            s.id.to_lowercase().contains(&query_lower)
                || s.name.to_lowercase().contains(&query_lower)
                || s.description.to_lowercase().contains(&query_lower)
                || s.tags.iter().any(|t| t.to_lowercase() == query_lower)
        })
        .collect();

    // Sort by relevance: ID match > name match > tag match > description match
    results.sort_by_key(|s| {
        let id_match = s.id.to_lowercase().contains(&query_lower) as u8;
        let name_match = s.name.to_lowercase().contains(&query_lower) as u8;
        let tag_match = s.tags.iter().any(|t| t.to_lowercase() == query_lower) as u8;
        // Higher score = better match (reverse sort)
        !(id_match * 3 + name_match * 2 + tag_match)
    });

    Ok(results)
}

/// Скачать и установить скрипт из реестра по ID.
pub fn download_and_install(registry_id: &str) -> Result<String> {
    let index = fetch_registry_index()?;
    let script = index
        .scripts
        .iter()
        .find(|s| s.id == registry_id)
        .ok_or_else(|| anyhow::anyhow!("script '{}' not found in registry", registry_id))?;

    // Download the script content
    let resp = ureq::get(&script.download_url)
        .call()
        .context("failed to download script from registry")?;

    let content = resp
        .into_body()
        .read_to_string()
        .context("failed to read script content")?;

    // Verify SHA-256 if provided
    if let Some(ref expected_hash) = script.sha256 {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let actual_hash = hex::encode(hasher.finalize());
        if &actual_hash != expected_hash {
            anyhow::bail!(
                "SHA-256 mismatch: expected {}, got {}",
                expected_hash,
                actual_hash
            );
        }
    }

    // Determine file extension from content
    let ext = if content.trim_start().starts_with('{') || content.trim_start().starts_with('[') {
        if content.contains("triggers") && content.contains("step") {
            "toml"
        } else {
            "rhai"
        }
    } else if content.contains("fn ") || content.contains("let ") {
        "rhai"
    } else {
        "toml"
    };

    // Save to ~/.terio/scripts/user/
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let user_dir = std::path::PathBuf::from(&home)
        .join(".terio")
        .join("scripts")
        .join("user");
    std::fs::create_dir_all(&user_dir)?;

    let filename = format!("{}.{}", script.id, ext);
    let dest = user_dir.join(&filename);
    std::fs::write(&dest, &content).context("failed to write script file")?;

    Ok(format!("{}/{}", user_dir.display(), filename))
}

/// Подготовить скрипт к публикации в реестр.
pub fn prepare_publish(
    script_id: &str,
    content: &str,
    name: &str,
    description: &str,
    tags: Vec<String>,
    author: Option<&str>,
    risk: Option<&str>,
) -> Result<RegistryScript> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let sha256 = hex::encode(hasher.finalize());

    // Default download URL for local testing
    let download_url = format!(
        "https://raw.githubusercontent.com/bortoq/terio-registry/main/scripts/{}",
        script_id
    );

    Ok(RegistryScript {
        id: script_id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        tags,
        author: author.map(|a| a.to_string()),
        version: "0.1.0".to_string(),
        download_url,
        sha256: Some(sha256),
        risk: risk.unwrap_or("read_only").to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_registry_filters_by_id() {
        let index = RegistryIndex {
            version: 1,
            scripts: vec![
                RegistryScript {
                    id: "ls-enhanced".into(),
                    name: "ls Enhanced".into(),
                    description: "Better ls with colors".into(),
                    tags: vec!["files".into(), "list".into()],
                    author: Some("test".into()),
                    version: "0.1.0".into(),
                    download_url: "https://example.com/ls-enhanced.rhai".into(),
                    sha256: None,
                    risk: "read_only".into(),
                },
                RegistryScript {
                    id: "git-status".into(),
                    name: "Git Status".into(),
                    description: "Show git status".into(),
                    tags: vec!["git".into(), "vcs".into()],
                    author: Some("test".into()),
                    version: "0.1.0".into(),
                    download_url: "https://example.com/git-status.rhai".into(),
                    sha256: None,
                    risk: "read_only".into(),
                },
            ],
        };

        // We can't test fetch_registry_index() without network, but we can test the filter logic
        let results: Vec<RegistryScript> = index
            .scripts
            .into_iter()
            .filter(|s| {
                let q = "ls";
                s.id.contains(q)
                    || s.name.to_lowercase().contains(q)
                    || s.description.to_lowercase().contains(q)
                    || s.tags.iter().any(|t| t == q)
            })
            .collect();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "ls-enhanced");
    }

    #[test]
    fn test_prepare_publish_generates_sha256() {
        let result = prepare_publish(
            "test-script",
            "fn main() { terio_execute(\"ls\"); }",
            "Test Script",
            "A test script",
            vec!["test".into()],
            Some("author"),
            Some("read_only"),
        )
        .unwrap();
        assert_eq!(result.id, "test-script");
        assert!(result.sha256.is_some());
        assert_eq!(result.sha256.as_deref().map(|s| s.len()), Some(64));
    }

    #[test]
    fn test_registry_script_defaults() {
        let s = RegistryScript {
            id: "test".into(),
            name: "Test".into(),
            description: "desc".into(),
            tags: vec![],
            author: None,
            version: default_version(),
            download_url: "https://example.com/test".into(),
            sha256: None,
            risk: "read_only".into(),
        };
        assert_eq!(s.version, "0.1.0");
    }
}
