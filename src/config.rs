// Config: persistence for provider settings and user preferences.
// File: ~/.terio/config.json

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Supported LLM provider types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    Openai,
    Anthropic,
    Ollama,
    #[default]
    Mock,
}

/// Provider-specific settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider_type: ProviderType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            provider_type: ProviderType::Mock,
            api_key: None,
            model: None,
            base_url: None,
        }
    }
}

/// Top-level config.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub provider: ProviderConfig,
    /// Auto-confirm risk levels (skip y/N prompt).
    #[serde(default)]
    pub auto_confirm: Vec<String>,
}

impl Config {
    fn path() -> Result<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("HOME not set")?;
        Ok(PathBuf::from(home).join(".terio").join("config.json"))
    }

    /// Load config from ~/.terio/config.json. Returns default if file missing.
    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Config::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read config: {}", path.display()))?;
        let config: Config = serde_json::from_str(&content).context("invalid config JSON")?;
        Ok(config)
    }

    /// Save config to ~/.terio/config.json.
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("failed to create .terio directory")?;
        }
        let json = serde_json::to_string_pretty(self).context("failed to serialize config")?;
        std::fs::write(&path, json)
            .with_context(|| format!("failed to write config: {}", path.display()))?;
        Ok(())
    }

    /// Set a config key from CLI: `terio config set provider.type openai`
    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "provider.type" | "provider_type" => {
                self.provider.provider_type = match value {
                    "openai" => ProviderType::Openai,
                    "anthropic" => ProviderType::Anthropic,
                    "ollama" => ProviderType::Ollama,
                    "mock" => ProviderType::Mock,
                    other => anyhow::bail!(
                        "unknown provider: {other}. Use: openai, anthropic, ollama, mock"
                    ),
                };
            }
            "provider.api_key" | "api_key" => {
                self.provider.api_key = Some(value.to_string());
            }
            "provider.model" | "model" => {
                self.provider.model = Some(value.to_string());
            }
            "provider.base_url" | "base_url" => {
                self.provider.base_url = Some(value.to_string());
            }
            "auto_confirm" => {
                self.auto_confirm = value.split(',').map(|s| s.trim().to_string()).collect();
            }
            _ => anyhow::bail!("unknown config key: {key}"),
        }
        Ok(())
    }
}

impl Config {
    /// Display config in human-readable format (method, was free function).
    pub fn print(&self) {
        println!("Provider:   {:?}", self.provider.provider_type);
        if let Some(ref key) = self.provider.api_key {
            let masked = if key.len() > 8 {
                format!("{}…{}", &key[..4], &key[key.len() - 4..])
            } else {
                "[set]".to_string()
            };
            println!("API key:    {}", masked);
        }
        if let Some(ref model) = self.provider.model {
            println!("Model:      {}", model);
        }
        if let Some(ref url) = self.provider.base_url {
            println!("Base URL:   {}", url);
        }
        if !self.auto_confirm.is_empty() {
            println!("Auto-confirm: {}", self.auto_confirm.join(", "));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.provider.provider_type, ProviderType::Mock);
        assert!(config.provider.api_key.is_none());
    }

    #[test]
    fn test_config_set_provider_type() {
        let mut config = Config::default();
        config.set("provider.type", "openai").unwrap();
        assert_eq!(config.provider.provider_type, ProviderType::Openai);
    }

    #[test]
    fn test_config_set_api_key() {
        let mut config = Config::default();
        config.set("api_key", "sk-test123").unwrap();
        assert_eq!(config.provider.api_key, Some("sk-test123".to_string()));
    }

    #[test]
    fn test_config_set_unknown_key() {
        let mut config = Config::default();
        assert!(config.set("nonexistent", "value").is_err());
    }

    #[test]
    fn test_config_save_and_load() {
        // Override HOME for test
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let mut config = Config::default();
        config.set("provider.type", "openai").unwrap();
        config.set("api_key", "sk-test-key").unwrap();
        config.save().unwrap();

        let loaded = Config::load().unwrap();
        assert_eq!(loaded.provider.provider_type, ProviderType::Openai);
        assert_eq!(loaded.provider.api_key, Some("sk-test-key".to_string()));
    }
}
