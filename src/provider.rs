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
    #[serde(default)]
    cache_template: Option<LlmCacheTemplateResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmCommandResponse {
    command: String,
    argv: Vec<String>,
    risk: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmCacheTemplateResponse {
    parameters: serde_json::Value,
    preconditions: Vec<serde_json::Value>,
    steps: Vec<LlmCacheStepResponse>,
    artifacts: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmCacheStepResponse {
    command: String,
    argv: Vec<String>,
    risk: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    description: Option<String>,
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
            "response_format": { "type": "json_object" },
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
        validate_llm_plan_shape(&llm)?;

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

        let cache_template = llm
            .cache_template
            .map(|template| crate::agent::AgentCacheTemplate {
                parameters: template.parameters,
                preconditions: template.preconditions,
                steps: template
                    .steps
                    .into_iter()
                    .map(|step| crate::agent::AgentCacheStep {
                        command: step.command,
                        argv: step.argv,
                        risk: parse_risk(&step.risk),
                        description: step.description.or(step.reason),
                    })
                    .collect(),
                artifacts: template.artifacts,
            });
        let tokens_used = value["usage"]["total_tokens"].as_u64();

        Ok(AgentPlan {
            summary: llm.summary,
            risk: parse_risk(&llm.risk),
            commands,
            cache_template,
            tokens_used,
        })
    }
}

/// Ollama provider — calls local Ollama instance via its HTTP API.
pub struct OllamaProvider {
    model: String,
    base_url: String,
}

impl OllamaProvider {
    pub fn new(config: &ProviderConfig) -> Self {
        let model = config
            .model
            .clone()
            .unwrap_or_else(|| "llama3.2".to_string());
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        Self { model, base_url }
    }
}

impl Provider for OllamaProvider {
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

        // Try OpenAI-compatible endpoint first (Ollama >= 0.1.32)
        let openai_url = format!("{}/v1/chat/completions", self.base_url);
        let ollama_url = format!("{}/api/chat", self.base_url);

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "stream": false,
            "temperature": 0.1,
            "max_tokens": 1000,
        });

        // Try OpenAI-compatible path first
        let result = ureq::post(&openai_url)
            .header("Content-Type", "application/json")
            .send_json(&body);

        let (response_text, tokens_used): (String, Option<u64>) = match result {
            Ok(resp) => {
                let value: serde_json::Value = resp
                    .into_body()
                    .read_json()
                    .context("failed to parse Ollama (OpenAI-compat) response")?;
                let content = value["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let tokens = value["usage"]["total_tokens"].as_u64();
                (content, tokens)
            }
            Err(_) => {
                // Fallback to native Ollama /api/chat endpoint
                let resp = ureq::post(&ollama_url)
                    .header("Content-Type", "application/json")
                    .send_json(&body)
                    .context("Ollama API request failed (tried both /v1/chat/completions and /api/chat). Is Ollama running?")?;

                let value: serde_json::Value = resp
                    .into_body()
                    .read_json()
                    .context("failed to parse Ollama response")?;

                let content = value["message"]["content"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Ollama response missing message.content"))?
                    .to_string();
                let tokens = value["eval_count"].as_u64();
                (content, tokens)
            }
        };

        let json_str = extract_json(&response_text)?;
        let llm: LlmPlanResponse =
            serde_json::from_str(json_str).context("failed to parse LLM plan JSON from Ollama")?;
        validate_llm_plan_shape(&llm)?;

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

        let cache_template = llm
            .cache_template
            .map(|template| crate::agent::AgentCacheTemplate {
                parameters: template.parameters,
                preconditions: template.preconditions,
                steps: template
                    .steps
                    .into_iter()
                    .map(|step| crate::agent::AgentCacheStep {
                        command: step.command,
                        argv: step.argv,
                        risk: parse_risk(&step.risk),
                        description: step.description.or(step.reason),
                    })
                    .collect(),
                artifacts: template.artifacts,
            });

        Ok(AgentPlan {
            summary: llm.summary,
            risk: parse_risk(&llm.risk),
            commands,
            cache_template,
            tokens_used,
        })
    }
}

fn validate_llm_plan_shape(plan: &LlmPlanResponse) -> Result<()> {
    if plan.summary.trim().is_empty() {
        anyhow::bail!("provider returned empty summary");
    }
    if plan.commands.is_empty() {
        anyhow::bail!("provider returned empty commands");
    }
    for cmd in &plan.commands {
        if cmd.command.trim().is_empty() {
            anyhow::bail!("provider returned empty command name");
        }
        if cmd.reason.trim().is_empty() {
            anyhow::bail!("provider returned empty command reason");
        }
        if cmd.argv.is_empty() || cmd.argv.iter().any(|arg| arg.is_empty()) {
            anyhow::bail!("provider returned empty argv item");
        }
    }
    if let Some(template) = &plan.cache_template {
        if template.steps.is_empty() {
            anyhow::bail!("provider returned empty cache_template steps");
        }
        for step in &template.steps {
            if step.command.trim().is_empty() {
                anyhow::bail!("provider returned empty cache_template command");
            }
            if step.argv.is_empty() || step.argv.iter().any(|arg| arg.is_empty()) {
                anyhow::bail!("provider returned empty cache_template argv item");
            }
        }
    }
    Ok(())
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
        crate::config::ProviderType::Ollama => Box::new(OllamaProvider::new(config)),
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
            cache_template: None,
            tokens_used: None,
        };
        assert!(!needs_confirmation(&plan));

        let plan = AgentPlan {
            summary: "test".to_string(),
            risk: RiskLevel::Destructive,
            commands: vec![],
            cache_template: None,
            tokens_used: None,
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

    #[test]
    fn test_extract_usage_tokens() {
        let value = serde_json::json!({
            "usage": { "total_tokens": 321 }
        });
        assert_eq!(value["usage"]["total_tokens"].as_u64(), Some(321));
    }

    #[test]
    fn test_validate_llm_plan_shape_accepts_cache_template() {
        let plan = LlmPlanResponse {
            summary: "List files".into(),
            risk: "read_only".into(),
            commands: vec![LlmCommandResponse {
                command: "ls".into(),
                argv: vec!["ls".into(), "-la".into()],
                risk: "read_only".into(),
                reason: "inspect directory".into(),
            }],
            cache_template: Some(LlmCacheTemplateResponse {
                parameters: serde_json::json!({"path": "."}),
                preconditions: vec![serde_json::json!({"cwd_exists": true})],
                steps: vec![LlmCacheStepResponse {
                    command: "ls".into(),
                    argv: vec!["ls".into(), "-la".into()],
                    risk: "read_only".into(),
                    reason: None,
                    description: Some("list current directory".into()),
                }],
                artifacts: vec![],
            }),
        };
        assert!(validate_llm_plan_shape(&plan).is_ok());
    }

    #[test]
    fn test_validate_llm_plan_shape_rejects_empty_summary() {
        let plan = LlmPlanResponse {
            summary: "   ".into(),
            risk: "read_only".into(),
            commands: vec![LlmCommandResponse {
                command: "pwd".into(),
                argv: vec!["pwd".into()],
                risk: "read_only".into(),
                reason: "show cwd".into(),
            }],
            cache_template: None,
        };
        assert!(validate_llm_plan_shape(&plan).is_err());
    }

    #[test]
    fn test_validate_llm_plan_shape_rejects_empty_commands() {
        let plan = LlmPlanResponse {
            summary: "No commands".into(),
            risk: "read_only".into(),
            commands: vec![],
            cache_template: None,
        };
        assert!(validate_llm_plan_shape(&plan).is_err());
    }

    #[test]
    fn test_validate_llm_plan_shape_rejects_empty_cache_template_step_argv() {
        let plan = LlmPlanResponse {
            summary: "Bad cache template".into(),
            risk: "read_only".into(),
            commands: vec![LlmCommandResponse {
                command: "pwd".into(),
                argv: vec!["pwd".into()],
                risk: "read_only".into(),
                reason: "show cwd".into(),
            }],
            cache_template: Some(LlmCacheTemplateResponse {
                parameters: serde_json::json!({}),
                preconditions: vec![],
                steps: vec![LlmCacheStepResponse {
                    command: "pwd".into(),
                    argv: vec![],
                    risk: "read_only".into(),
                    reason: None,
                    description: None,
                }],
                artifacts: vec![],
            }),
        };
        assert!(validate_llm_plan_shape(&plan).is_err());
    }
}
