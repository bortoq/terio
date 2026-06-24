// Agent (Mock): hardcoded ответы для Phase 2.

use crate::cache::CachedStep;
use crate::types::RiskLevel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCacheStep {
    pub command: String,
    pub argv: Vec<String>,
    pub risk: RiskLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCacheTemplate {
    pub parameters: serde_json::Value,
    pub preconditions: Vec<serde_json::Value>,
    pub steps: Vec<AgentCacheStep>,
    pub artifacts: Vec<serde_json::Value>,
}

/// План от agent (соответствует schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPlan {
    pub summary: String,
    pub risk: RiskLevel,
    pub commands: Vec<AgentCommand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_template: Option<AgentCacheTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_used: Option<u64>,
}

/// Одна команда в плане.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCommand {
    pub command: String,
    pub argv: Vec<String>,
    pub risk: RiskLevel,
    pub reason: String,
}

/// Mock-ответы: известные запросы → планы.
fn mock_responses() -> Vec<(&'static str, AgentPlan)> {
    vec![
        (
            "list files",
            AgentPlan {
                summary: "List files in current directory with details".to_string(),
                risk: RiskLevel::ReadOnly,
                commands: vec![AgentCommand {
                    command: "ls".to_string(),
                    argv: vec!["ls".to_string(), "-l".to_string()],
                    risk: RiskLevel::ReadOnly,
                    reason: "Shows detailed file listing".to_string(),
                }],
                cache_template: None,
                tokens_used: None,
            },
        ),
        (
            "current directory",
            AgentPlan {
                summary: "Show current working directory".to_string(),
                risk: RiskLevel::ReadOnly,
                commands: vec![AgentCommand {
                    command: "pwd".to_string(),
                    argv: vec!["pwd".to_string()],
                    risk: RiskLevel::ReadOnly,
                    reason: "Prints current working directory".to_string(),
                }],
                cache_template: None,
                tokens_used: None,
            },
        ),
        (
            "who am i",
            AgentPlan {
                summary: "Show current user".to_string(),
                risk: RiskLevel::ReadOnly,
                commands: vec![AgentCommand {
                    command: "whoami".to_string(),
                    argv: vec!["whoami".to_string()],
                    risk: RiskLevel::ReadOnly,
                    reason: "Prints current user name".to_string(),
                }],
                cache_template: None,
                tokens_used: None,
            },
        ),
        (
            "date and time",
            AgentPlan {
                summary: "Show current date and time".to_string(),
                risk: RiskLevel::ReadOnly,
                commands: vec![AgentCommand {
                    command: "date".to_string(),
                    argv: vec!["date".to_string()],
                    risk: RiskLevel::ReadOnly,
                    reason: "Prints current date and time".to_string(),
                }],
                cache_template: None,
                tokens_used: None,
            },
        ),
        (
            "disk usage",
            AgentPlan {
                summary: "Show disk usage in human readable format".to_string(),
                risk: RiskLevel::ReadOnly,
                commands: vec![AgentCommand {
                    command: "df".to_string(),
                    argv: vec!["df".to_string(), "-h".to_string()],
                    risk: RiskLevel::ReadOnly,
                    reason: "Shows disk usage in human-readable format".to_string(),
                }],
                cache_template: None,
                tokens_used: None,
            },
        ),
    ]
}

/// Пытается найти mock-ответ для запроса.
pub fn get_mock_plan(request: &str) -> Option<AgentPlan> {
    let normalized = crate::matcher::normalize(request);
    for (pattern, plan) in mock_responses() {
        if crate::matcher::normalize(pattern) == normalized {
            return Some(plan);
        }
    }
    None
}

/// Преобразует AgentPlan в CachedStep для сохранения в кеш.
pub fn plan_to_steps(plan: &AgentPlan) -> Vec<CachedStep> {
    if let Some(template) = &plan.cache_template {
        return template
            .steps
            .iter()
            .map(|c| CachedStep {
                command: c.command.clone(),
                argv: c.argv.clone(),
                risk: c.risk.clone(),
            })
            .collect();
    }

    plan.commands
        .iter()
        .map(|c| CachedStep {
            command: c.command.clone(),
            argv: c.argv.clone(),
            risk: c.risk.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_list_files() {
        let plan = get_mock_plan("list files").unwrap();
        assert_eq!(plan.commands.len(), 1);
        assert_eq!(plan.commands[0].command, "ls");
    }

    #[test]
    fn test_mock_normalized() {
        let plan = get_mock_plan("  List   Files  ").unwrap();
        assert_eq!(plan.summary, "List files in current directory with details");
    }

    #[test]
    fn test_mock_unknown() {
        let plan = get_mock_plan("some random request");
        assert!(plan.is_none());
    }

    #[test]
    fn test_plan_to_steps() {
        let plan = get_mock_plan("list files").unwrap();
        let steps = plan_to_steps(&plan);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].command, "ls");
    }

    #[test]
    fn test_plan_to_steps_prefers_cache_template_steps() {
        let plan = AgentPlan {
            summary: "Template".into(),
            risk: RiskLevel::ReadOnly,
            commands: vec![AgentCommand {
                command: "pwd".into(),
                argv: vec!["pwd".into()],
                risk: RiskLevel::ReadOnly,
                reason: "fallback".into(),
            }],
            cache_template: Some(AgentCacheTemplate {
                parameters: serde_json::json!({}),
                preconditions: vec![],
                steps: vec![AgentCacheStep {
                    command: "ls".into(),
                    argv: vec!["ls".into(), "-la".into()],
                    risk: RiskLevel::ReadOnly,
                    description: Some("template step".into()),
                }],
                artifacts: vec![],
            }),
            tokens_used: Some(42),
        };

        let steps = plan_to_steps(&plan);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].command, "ls");
    }
}
