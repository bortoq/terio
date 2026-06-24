// Config: persistence for provider settings and user preferences.
// File: ~/.terio/config.json

use crate::trust::TrustPolicy;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
pub struct UiConfig {
    #[serde(default)]
    pub show_config: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_selected_policy: Option<String>,
}

/// Top-level config.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub provider: ProviderConfig,
    /// Auto-confirm risk levels (skip y/N prompt).
    #[serde(default)]
    pub auto_confirm: Vec<String>,
    /// Политика доверия по умолчанию.
    #[serde(default)]
    pub default_trust_policy: TrustPolicy,
    /// Переопределения политики для конкретных скриптов (request_hash -> policy).
    #[serde(default)]
    pub policy_overrides: HashMap<String, TrustPolicy>,
    #[serde(default)]
    pub ui: UiConfig,
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
        if let Some(hash) = key.strip_prefix("policy_override.") {
            self.policy_overrides
                .insert(hash.to_string(), parse_policy(value)?);
            return Ok(());
        }

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
            "trust_policy" | "default_trust_policy" => {
                self.default_trust_policy = parse_policy(value)?;
            }
            "ui.show_config" => {
                self.ui.show_config = match value {
                    "true" | "1" | "yes" => true,
                    "false" | "0" | "no" => false,
                    other => anyhow::bail!("unknown bool value for ui.show_config: {other}"),
                };
            }
            "ui.last_selected_policy" => {
                self.ui.last_selected_policy = Some(value.to_string());
            }
            _ => anyhow::bail!("unknown config key: {key}"),
        }
        Ok(())
    }

    /// Display config in human-readable format.
    pub fn print(&self) {
        print!("{}", self.render_for_display());
    }

    pub fn render_for_display(&self) -> String {
        let mut lines = vec![format!("Provider:   {:?}", self.provider.provider_type)];
        if let Some(ref key) = self.provider.api_key {
            let masked = if key.len() > 8 {
                format!("{}…{}", &key[..4], &key[key.len() - 4..])
            } else {
                "[set]".to_string()
            };
            lines.push(format!("API key:    {}", masked));
        }
        if let Some(ref model) = self.provider.model {
            lines.push(format!("Model:      {}", model));
        }
        if let Some(ref url) = self.provider.base_url {
            lines.push(format!("Base URL:   {}", url));
        }
        if !self.auto_confirm.is_empty() {
            lines.push(format!("Auto-confirm: {}", self.auto_confirm.join(", ")));
        }
        let mut trust_line = format!("Trust policy: {:?}", self.default_trust_policy);
        if !self.policy_overrides.is_empty() {
            trust_line.push_str(&format!(" ({} overrides)", self.policy_overrides.len()));
        }
        lines.push(trust_line);
        lines.push(format!("UI config open: {}", self.ui.show_config));
        if let Some(ref policy) = self.ui.last_selected_policy {
            lines.push(format!("UI last policy: {}", policy));
        }
        format!("{}\n", lines.join("\n"))
    }
}

fn parse_policy(value: &str) -> Result<TrustPolicy> {
    match value {
        "always_ask" => Ok(TrustPolicy::AlwaysAsk),
        "ask_once" => Ok(TrustPolicy::AskOnce),
        "allow" => Ok(TrustPolicy::Allow),
        other => anyhow::bail!("unknown policy: {other}. Use: always_ask, ask_once, allow"),
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
        assert_eq!(config.default_trust_policy, TrustPolicy::Allow);
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
    fn test_config_set_trust_policy() {
        let mut config = Config::default();
        config.set("trust_policy", "always_ask").unwrap();
        assert_eq!(config.default_trust_policy, TrustPolicy::AlwaysAsk);
    }

    #[test]
    fn test_config_set_policy_override() {
        let mut config = Config::default();
        config.set("policy_override.h1", "always_ask").unwrap();
        assert_eq!(config.policy_overrides["h1"], TrustPolicy::AlwaysAsk);
    }

    #[test]
    fn test_config_set_unknown_key() {
        let mut config = Config::default();
        assert!(config.set("nonexistent", "value").is_err());
    }

    #[test]
    fn test_config_save_and_load() {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let mut config = Config::default();
        config.set("provider.type", "openai").unwrap();
        config.set("api_key", "sk-test-key").unwrap();
        config.set("trust_policy", "ask_once").unwrap();
        config.ui.show_config = true;
        config.ui.last_selected_policy = Some("ask_once".to_string());
        config.save().unwrap();

        let loaded = Config::load().unwrap();
        assert_eq!(loaded.provider.provider_type, ProviderType::Openai);
        assert_eq!(loaded.provider.api_key, Some("sk-test-key".to_string()));
        assert_eq!(loaded.default_trust_policy, TrustPolicy::AskOnce);
        assert!(loaded.ui.show_config);
        assert_eq!(loaded.ui.last_selected_policy.as_deref(), Some("ask_once"));

        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn test_config_render_for_display_includes_override_count() {
        let mut config = Config::default();
        config
            .policy_overrides
            .insert("abc".to_string(), TrustPolicy::Allow);
        let rendered = config.render_for_display();
        assert!(rendered.contains("1 overrides"));
    }
}
