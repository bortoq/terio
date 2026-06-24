// Provider abstraction: trait + OpenAI implementation.

use crate::agent::{get_mock_plan, AgentPlan};
use crate::config::ProviderConfig;
use crate::redact::redact;
use crate::types::RiskLevel;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Structured response expected from LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmPlanResponse {
    summary: String,
    risk: String,
    commands: Vec<LlmCommandResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmCommandResponse {
    command: String,
    argv: Vec<String>,
    risk: String,
    reason: String,
}

/// Provider trait: generates a plan from a natural language request.
pub trait Provider: Send + Sync {
    fn plan(&self, request: &str) -> Result<AgentPlan>;
}

/// Mock provider — hardcoded responses for known requests.
pub struct MockProvider;

impl Provider for MockProvider {
    fn plan(&self, request: &str) -> Result<AgentPlan> {
        match get_mock_plan(request) {
            Some(plan) => Ok(plan),
            None => anyhow::bail!("mock provider doesn't know how to handle: {request}"),
        }
    }
}

/// OpenAI provider — calls Chat Completions API with structured prompt.
pub struct OpenAiProvider {
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAiProvider {
    pub fn new(config: &ProviderConfig) -> Result<Self> {
        let api_key = config.api_key.clone().ok_or_else(|| {
            anyhow::anyhow!("OpenAI API key not set. Use: terio config set api_key sk-...")
        })?;
        let model = config
            .model
            .clone()
            .unwrap_or_else(|| "gpt-4o-mini".to_string());
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        Ok(Self {
            api_key,
            model,
            base_url,
        })
    }
}

impl Provider for OpenAiProvider {
    fn plan(&self, request: &str) -> Result<AgentPlan> {
        let redacted = redact(request);
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        let files = top_level_entries().unwrap_or_default().join(", ");
        let prompt = format!(
            r#"You are a CLI assistant. Given a user request, output a JSON plan with commands to execute.

Rules:
- Only output valid JSON, no other text.
- risk must be one of: read_only, local_write, destructive, network_read, network_write, credential_access, financial.
- Each command must have: command, argv (array of strings), risk, reason.
- Prefer safe commands. Use read_only unless the task requires writing.

Example:
{{"summary": "List files in current directory", "risk": "read_only", "commands": [{{"command": "ls", "argv": ["ls", "-la"], "risk": "read_only", "reason": "List files with details"}}]}}

Current working directory: {cwd}
Top-level entries: {files}
User request: {redacted}
"#
        );

        let url = format!("{}/chat/completions", self.base_url);
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.1,
            "max_tokens": 1000,
        });

        let resp = ureq::post(&url)
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send_json(&body)
            .context("OpenAI API request failed")?;

        let value: serde_json::Value = resp
            .into_body()
            .read_json()
            .context("failed to parse OpenAI response")?;

        let content = value["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("OpenAI response missing content"))?;

        // Try to extract JSON from the response (may be wrapped in markdown)
        let json_str = extract_json(content)?;
        let llm: LlmPlanResponse =
            serde_json::from_str(json_str).context("failed to parse LLM plan JSON")?;

        let commands = llm
            .commands
            .into_iter()
            .map(|c| crate::agent::AgentCommand {
                command: c.command,
                argv: c.argv,
                risk: parse_risk(&c.risk),
                reason: c.reason,
            })
            .collect();

        Ok(AgentPlan {
            summary: llm.summary,
            risk: parse_risk(&llm.risk),
            commands,
        })
    }
}

fn top_level_entries() -> Result<Vec<String>> {
    let cwd = std::env::current_dir()?;
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(cwd)? {
        let entry = entry?;
        entries.push(entry.file_name().to_string_lossy().to_string());
        if entries.len() >= 32 {
            break;
        }
    }
    entries.sort();
    Ok(entries)
}

/// Parse risk string from LLM response.
fn parse_risk(s: &str) -> RiskLevel {
    match s.to_lowercase().as_str() {
        "read_only" => RiskLevel::ReadOnly,
        "local_write" => RiskLevel::LocalWrite,
        "destructive" => RiskLevel::Destructive,
        "network_read" => RiskLevel::NetworkRead,
        "network_write" => RiskLevel::NetworkWrite,
        "credential_access" => RiskLevel::CredentialAccess,
        "financial" => RiskLevel::Financial,
        _ => RiskLevel::ReadOnly,
    }
}

/// Extract JSON from LLM response (strip markdown fences if present).
fn extract_json(content: &str) -> Result<&str> {
    let content = content.trim();
    if let Some(stripped) = content
        .strip_prefix("```json")
        .or_else(|| content.strip_prefix("```"))
    {
        if let Some(end) = stripped.rfind("```") {
            Ok(stripped[..end].trim())
        } else {
            Ok(stripped.trim())
        }
    } else {
        Ok(content)
    }
}

/// Factory: create provider from config.
pub fn create_provider(config: &ProviderConfig) -> Box<dyn Provider> {
    match config.provider_type {
        crate::config::ProviderType::Openai => match OpenAiProvider::new(config) {
            Ok(p) => Box::new(p),
            Err(e) => {
                eprintln!("warning: failed to create OpenAI provider: {e}");
                eprintln!("warning: falling back to mock provider");
                Box::new(MockProvider)
            }
        },
        crate::config::ProviderType::Anthropic => {
            eprintln!("warning: Anthropic provider not yet implemented, using mock");
            Box::new(MockProvider)
        }
        crate::config::ProviderType::Ollama => {
            eprintln!("warning: Ollama provider not yet implemented, using mock");
            Box::new(MockProvider)
        }
        crate::config::ProviderType::Mock => Box::new(MockProvider),
    }
}

/// Check if a plan needs user confirmation based on risk.
pub fn needs_confirmation(plan: &AgentPlan) -> bool {
    matches!(
        plan.risk,
        RiskLevel::Destructive
            | RiskLevel::NetworkWrite
            | RiskLevel::CredentialAccess
            | RiskLevel::Financial
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_plain() {
        let input = r#"{"summary": "test"}"#;
        assert_eq!(extract_json(input).unwrap(), r#"{"summary": "test"}"#);
    }

    #[test]
    fn test_extract_json_markdown() {
        let input = "```json\n{\"summary\": \"test\"}\n```";
        let extracted = extract_json(input).unwrap();
        assert_eq!(extracted, r#"{"summary": "test"}"#);
    }

    #[test]
    fn test_parse_risk() {
        assert_eq!(parse_risk("read_only"), RiskLevel::ReadOnly);
        assert_eq!(parse_risk("destructive"), RiskLevel::Destructive);
        assert_eq!(parse_risk("unknown"), RiskLevel::ReadOnly);
    }

    #[test]
    fn test_needs_confirmation() {
        let plan = AgentPlan {
            summary: "test".to_string(),
            risk: RiskLevel::ReadOnly,
            commands: vec![],
        };
        assert!(!needs_confirmation(&plan));

        let plan = AgentPlan {
            summary: "test".to_string(),
            risk: RiskLevel::Destructive,
            commands: vec![],
        };
        assert!(needs_confirmation(&plan));
    }

    #[test]
    fn test_mock_provider() {
        let provider = MockProvider;
        let plan = provider.plan("list files").unwrap();
        assert_eq!(plan.commands[0].command, "ls");
    }

    #[test]
    fn test_mock_provider_unknown() {
        let provider = MockProvider;
        assert!(provider.plan("unknown request never heard of").is_err());
    }
}
