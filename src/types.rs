// Core data types for terio (LogEntry, CostCounters, DisplayProfile, etc.)

use serde::{Deserialize, Serialize};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LogKind {
    AgentTurn,
    CommandRun,
    ScriptRun,
    SystemEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    ReadOnly,
    LocalWrite,
    Destructive,
    NetworkRead,
    NetworkWrite,
    CredentialAccess,
    Financial,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LogStatus {
    Success,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DisplayType {
    Auto,
    Text,
    Table,
    Media,
    Hidden,
    Summary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RendererHint {
    Auto,
    Table,
    Plain,
    Timeline,
    Card,
}

// ---------------------------------------------------------------------------
// DisplayProfile
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayProfile {
    #[serde(rename = "type")]
    pub display_type: DisplayType,
    pub renderer_hint: RendererHint,
    pub user_visible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary_max_lines: Option<u32>,
}

impl Default for DisplayProfile {
    fn default() -> Self {
        Self {
            display_type: DisplayType::Auto,
            renderer_hint: RendererHint::Auto,
            user_visible: true,
            summary_max_lines: None,
        }
    }
}

// ---------------------------------------------------------------------------
// CostCounters
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationCostHint {
    pub user_sec: f64,
}

impl Default for ObservationCostHint {
    fn default() -> Self {
        Self { user_sec: 0.0 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LlmCost {
    pub tokens: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExecutionCost {
    pub duration_ms: u64,
    pub commands_executed: u64,
    pub bytes_read: u64,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CacheCost {
    pub lookup_ms: u64,
    pub hit: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StorageCost {
    pub bytes_written: u64,
    pub bytes_read: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CostCounters {
    pub observation_cost_hint: ObservationCostHint,
    pub llm_cost: LlmCost,
    pub execution_cost: ExecutionCost,
    pub cache_cost: CacheCost,
    pub storage_cost: StorageCost,
}

// ---------------------------------------------------------------------------
// CommandInfo & StepInfo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandInfo {
    pub display: String,
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepInfo {
    pub command: String,
    pub argv: Vec<String>,
    pub exit: i32,
}

// ---------------------------------------------------------------------------
// LogEntry — основная запись лога
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    // Required common fields
    pub schema_version: u32,
    pub instance_id: String,
    pub session_id: String,
    pub ts: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub interaction_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_interaction_id: Option<String>,

    pub kind: LogKind,
    pub display_profile: DisplayProfile,
    pub cost_counters: CostCounters,

    // Optional fields (kind-specific)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk: Option<RiskLevel>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<LogStatus>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_summary: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_used: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<CommandInfo>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_summary: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_summary: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_hit: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_called: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_saved_estimate: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_count_before: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_count_after: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<StepInfo>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl LogEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new_command_run(
        instance_id: &str,
        session_id: &str,
        interaction_id: Option<String>,
        request: &str,
        cwd: &str,
        argv: &[String],
        exit_code: i32,
        duration: Duration,
        stdout: &str,
        stderr: &str,
        cost_counters: CostCounters,
    ) -> Self {
        let display = argv.join(" ");
        let stdout_summary = Self::truncate(stdout, 1024);
        let stderr_summary = Self::truncate(stderr, 1024);

        let status = if exit_code == 0 {
            LogStatus::Success
        } else {
            LogStatus::Failed
        };

        Self {
            schema_version: 1,
            instance_id: instance_id.to_string(),
            session_id: session_id.to_string(),
            ts: chrono::Utc::now().to_rfc3339(),
            interaction_id,
            parent_interaction_id: None,
            kind: LogKind::CommandRun,
            display_profile: DisplayProfile::default(),
            cost_counters,
            request: Some(request.to_string()),
            cwd: Some(cwd.to_string()),
            risk: None,
            status: Some(status),
            failure_kind: None,
            prompt_summary: None,
            plan: None,
            model_provider: None,
            model_name: None,
            duration_ms: Some(duration.as_millis() as u64),
            tokens_used: None,
            command: Some(CommandInfo {
                display,
                argv: argv.to_vec(),
            }),
            exit: Some(exit_code),
            stdout_summary: Some(stdout_summary),
            stderr_summary: Some(stderr_summary),
            script_id: None,
            cache_hit: None,
            model_called: None,
            tokens_saved_estimate: None,
            success_count_before: None,
            success_count_after: None,
            steps: None,
            description: None,
        }
    }

    pub fn new_system_event(instance_id: &str, session_id: &str, description: &str) -> Self {
        Self {
            schema_version: 1,
            instance_id: instance_id.to_string(),
            session_id: session_id.to_string(),
            ts: chrono::Utc::now().to_rfc3339(),
            interaction_id: None,
            parent_interaction_id: None,
            kind: LogKind::SystemEvent,
            display_profile: DisplayProfile {
                display_type: DisplayType::Text,
                renderer_hint: RendererHint::Plain,
                user_visible: true,
                summary_max_lines: None,
            },
            cost_counters: CostCounters::default(),
            request: None,
            cwd: None,
            risk: None,
            status: Some(LogStatus::Success),
            failure_kind: None,
            prompt_summary: None,
            plan: None,
            model_provider: None,
            model_name: None,
            duration_ms: None,
            tokens_used: None,
            command: None,
            exit: None,
            stdout_summary: None,
            stderr_summary: None,
            script_id: None,
            cache_hit: None,
            model_called: None,
            tokens_saved_estimate: None,
            success_count_before: None,
            success_count_after: None,
            steps: None,
            description: Some(description.to_string()),
        }
    }

    fn truncate(s: &str, max: usize) -> String {
        if s.len() <= max {
            s.to_string()
        } else {
            format!("{}... (truncated)", &s[..max])
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 7: LogEvent — событийная модель лога
// ---------------------------------------------------------------------------

/// Пользовательское предсказание: что пользователь хотел получить.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPrediction {
    pub request: String,
    pub interaction_id: Option<String>,
    pub ts: String,
    pub risk: Option<RiskLevel>,
    pub attention_mode: Option<String>,
}

/// Предсказание terio: результат, показанный в окне.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerioPrediction {
    pub kind: LogKind,
    pub risk: Option<RiskLevel>,
    pub status: Option<LogStatus>,
    pub duration_ms: Option<u64>,
    pub tokens_used: Option<u64>,
    pub cache_hit: Option<bool>,
    pub model_name: Option<String>,
    pub cost_counters: CostCounters,
    pub stdout_summary: Option<String>,
    pub stderr_summary: Option<String>,
    pub command: Option<CommandInfo>,
    pub description: Option<String>,
    pub plan: Option<serde_json::Value>,
    pub prompt_summary: Option<String>,
    pub ts: String,
    pub interaction_id: Option<String>,
}

/// Событие лога — пара (UserPrediction, TerioPrediction).
/// Каждое событие соответствует одному взаимодействию пользователя с terio.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    pub user_prediction: UserPrediction,
    pub terio_predictions: Vec<TerioPrediction>,
    pub ts: String,
}

impl LogEvent {
    /// Создать LogEvent из одной LogEntry (1:1-отображение для Phase 7).
    pub fn from_entry(entry: &LogEntry) -> Self {
        let user_prediction = UserPrediction {
            request: entry.request.clone().unwrap_or_default(),
            interaction_id: entry.interaction_id.clone(),
            ts: entry.ts.clone(),
            risk: entry.risk.clone(),
            attention_mode: None,
        };
        let terio_prediction = TerioPrediction {
            kind: entry.kind.clone(),
            risk: entry.risk.clone(),
            status: entry.status.clone(),
            duration_ms: entry.duration_ms,
            tokens_used: entry.tokens_used,
            cache_hit: entry.cache_hit,
            model_name: entry.model_name.clone(),
            cost_counters: entry.cost_counters.clone(),
            stdout_summary: entry.stdout_summary.clone(),
            stderr_summary: entry.stderr_summary.clone(),
            command: entry.command.clone(),
            description: entry.description.clone(),
            plan: entry.plan.clone(),
            prompt_summary: entry.prompt_summary.clone(),
            ts: entry.ts.clone(),
            interaction_id: entry.interaction_id.clone(),
        };
        Self {
            user_prediction,
            terio_predictions: vec![terio_prediction],
            ts: entry.ts.clone(),
        }
    }

    /// Группировка LogEntry -> Vec<LogEvent> по interaction_id
    pub fn group_entries(entries: &[LogEntry]) -> Vec<Self> {
        use std::collections::HashMap;
        let mut events: Vec<Self> = Vec::new();
        let mut pending: HashMap<String, Vec<LogEntry>> = HashMap::new();

        for entry in entries {
            if let Some(ref iid) = entry.interaction_id {
                pending.entry(iid.clone()).or_default().push(entry.clone());
            } else {
                // No interaction_id — standalone event
                events.push(Self::from_entry(entry));
            }
        }

        // Merge pending groups
        for (_iid, group) in pending.drain() {
            if let Some(first) = group.first() {
                let mut event = Self::from_entry(first);
                event.terio_predictions = group
                    .iter()
                    .map(|e| TerioPrediction {
                        kind: e.kind.clone(),
                        risk: e.risk.clone(),
                        status: e.status.clone(),
                        duration_ms: e.duration_ms,
                        tokens_used: e.tokens_used,
                        cache_hit: e.cache_hit,
                        model_name: e.model_name.clone(),
                        cost_counters: e.cost_counters.clone(),
                        stdout_summary: e.stdout_summary.clone(),
                        stderr_summary: e.stderr_summary.clone(),
                        command: e.command.clone(),
                        description: e.description.clone(),
                        plan: e.plan.clone(),
                        prompt_summary: e.prompt_summary.clone(),
                        ts: e.ts.clone(),
                        interaction_id: e.interaction_id.clone(),
                    })
                    .collect();
                events.push(event);
            }
        }

        events
    }
}

// ---------------------------------------------------------------------------
// AggregatedCosts
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AggregatedCosts {
    pub total_duration_ms: u64,
    pub total_commands: u64,
    pub total_bytes_read: u64,
    pub total_bytes_written: u64,
    pub total_tokens: u64,
    pub total_llm_duration_ms: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub total_storage_written: u64,
    pub total_storage_read: u64,
}
