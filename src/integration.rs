// Integration system: agent learns programs via --help/man/wiki, stores integration scripts in cache.
// Phase 7: Lazy integrations — no pre-written connectors, all learned on demand.

use crate::cache::{CachedStep, ScriptCache};
use crate::types::RiskLevel;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Learning status for a program.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LearningStatus {
    /// Not yet learned
    Unknown,
    /// Learning in progress
    Learning,
    /// Learned — integration script available in cache
    Learned,
    /// Failed to learn
    Failed(String),
}

/// Integration record: tracks what terio knows about a program.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationRecord {
    pub schema_version: u32,
    pub program: String,
    pub status: LearningStatus,
    pub script_id: Option<String>,
    pub help_snippet: Option<String>,
    pub learned_at: Option<String>,
    pub source: LearnSource,
}

/// How the integration was learned.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LearnSource {
    Help,
    Man,
    Wiki,
}

/// Manages known program integrations.
pub struct IntegrationManager {
    dir: PathBuf,
    records: HashMap<String, IntegrationRecord>,
}

impl IntegrationManager {
    /// Create or load integration manager from ~/.terio/integrations/
    pub fn new() -> Result<Self> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("HOME not set")?;
        let dir = PathBuf::from(home).join(".terio").join("integrations");
        std::fs::create_dir_all(&dir).context("failed to create integrations dir")?;

        let mut mgr = Self {
            dir,
            records: HashMap::new(),
        };
        mgr.load_all()?;
        Ok(mgr)
    }

    /// Load all integration records from disk.
    fn load_all(&mut self) -> Result<()> {
        let index_path = self.dir.join("index.json");
        if index_path.exists() {
            let content = std::fs::read_to_string(&index_path)?;
            let records: Vec<IntegrationRecord> = serde_json::from_str(&content)?;
            for record in records {
                self.records.insert(record.program.clone(), record);
            }
        }
        Ok(())
    }

    /// Save all integration records to disk.
    pub fn save_all(&self) -> Result<()> {
        let index_path = self.dir.join("index.json");
        let records: Vec<&IntegrationRecord> = self.records.values().collect();
        let json = serde_json::to_string_pretty(&records)?;
        std::fs::write(&index_path, json)?;
        Ok(())
    }

    /// Get status for a program.
    pub fn get_status(&self, program: &str) -> LearningStatus {
        self.records
            .get(program)
            .map(|r| r.status.clone())
            .unwrap_or(LearningStatus::Unknown)
    }

    /// List all known programs with their status.
    pub fn list_programs(&self) -> Vec<IntegrationRecord> {
        let mut records: Vec<IntegrationRecord> = self.records.values().cloned().collect();
        records.sort_by(|a, b| a.program.cmp(&b.program));
        records
    }

    /// Learn a program by reading its --help output.
    pub fn learn_program(&mut self, program: &str) -> Result<IntegrationRecord> {
        let status_before = self.get_status(program);
        if status_before == LearningStatus::Learning {
            anyhow::bail!("already learning '{}'", program);
        }

        // Mark as learning
        let record = IntegrationRecord {
            schema_version: 1,
            program: program.to_string(),
            status: LearningStatus::Learning,
            script_id: None,
            help_snippet: None,
            learned_at: None,
            source: LearnSource::Help,
        };
        self.records.insert(program.to_string(), record);
        self.save_all()?;

        // Check if program exists
        let which_result = std::process::Command::new("which")
            .arg(program)
            .output();

        let _program_path = match which_result {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            }
            _ => {
                let record = IntegrationRecord {
                    schema_version: 1,
                    program: program.to_string(),
                    status: LearningStatus::Failed("program not found in PATH".to_string()),
                    script_id: None,
                    help_snippet: None,
                    learned_at: None,
                    source: LearnSource::Help,
                };
                self.records.insert(program.to_string(), record.clone());
                self.save_all()?;
                return Ok(record);
            }
        };

        // Read --help output
        let help_output = std::process::Command::new(program)
            .arg("--help")
            .output();

        let help_text = match help_output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let combined = if stdout.len() > stderr.len() { stdout } else { stderr };
                combined
            }
            Err(e) => {
                let record = IntegrationRecord {
                    schema_version: 1,
                    program: program.to_string(),
                    status: LearningStatus::Failed(format!("failed to run --help: {e}")),
                    script_id: None,
                    help_snippet: None,
                    learned_at: None,
                    source: LearnSource::Help,
                };
                self.records.insert(program.to_string(), record.clone());
                self.save_all()?;
                return Ok(record);
            }
        };

        let help_snippet = truncate_help(&help_text, 2048);
        let now = iso_now();

        // Try to generate integration script
        let script_result = generate_integration_script(program, &help_text);
        let script_id = match script_result {
            Ok(script_id) => Some(script_id),
            Err(_) => None,
        };

        let record = IntegrationRecord {
            schema_version: 1,
            program: program.to_string(),
            status: LearningStatus::Learned,
            script_id,
            help_snippet: Some(help_snippet),
            learned_at: Some(now),
            source: LearnSource::Help,
        };
        self.records.insert(program.to_string(), record.clone());
        self.save_all()?;

        Ok(record)
    }

    /// Remove a learned program.
    pub fn forget_program(&mut self, program: &str) -> Result<()> {
        self.records.remove(program);
        self.save_all()?;
        Ok(())
    }
}

/// Generate a basic integration script for a program and store in Script Cache.
/// Returns the script_id on success.
fn generate_integration_script(program: &str, help_text: &str) -> Result<String> {
    let request = format!("learn to use {}", program);

    // Build cached steps from help analysis
    let steps = vec![CachedStep {
        command: program.to_string(),
        argv: vec![program.to_string(), "--help".to_string()],
        risk: RiskLevel::ReadOnly,
    }];

    let cache = ScriptCache::new()?;
    let entry = cache.save_with_template(
        &request,
        RiskLevel::ReadOnly,
        serde_json::json!({
            "program": program,
            "help_snippet": truncate_help(help_text, 500),
        }),
        vec![serde_json::json!({
            "program_exists": true,
        })],
        steps,
        vec![],
    )?;

    Ok(entry.script_id)
}

/// Print integration status for all known programs.
pub fn print_integration_status(mgr: &IntegrationManager) {
    let records = mgr.list_programs();
    if records.is_empty() {
        println!("(no programs learned yet. Use `terio learn <program>` to start.)");
        return;
    }
    println!("{:<20} {:<12} {}", "Program", "Status", "Learned At");
    println!("{:-<20} {:-<12} {:-<20}", "", "", "");
    for r in &records {
        let status_str = match &r.status {
            LearningStatus::Unknown => "unknown",
            LearningStatus::Learning => "learning",
            LearningStatus::Learned => "learned",
            LearningStatus::Failed(_) => "failed",
        };
        let learned = r.learned_at.as_deref().unwrap_or("—");
        let learned_short = truncate_safe(learned, 19);
        println!("{:<20} {:<12} {}", r.program, status_str, learned_short);
    }
}

fn truncate_help(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut result: String = s.chars().take(max).collect();
        result.push_str("... (truncated)");
        result
    }
}

fn truncate_safe(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn iso_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let datetime = chrono::DateTime::from_timestamp(secs as i64, 0).unwrap_or_default();
    datetime.to_rfc3339()
}

// ---------------------------------------------------------------------------
// Share/receive: export/import window data between instances
// ---------------------------------------------------------------------------

/// Window data for sharing between terio instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedWindow {
    pub schema_version: u32,
    pub created_at: String,
    pub source_instance_id: String,
    pub entries: Vec<crate::types::LogEntry>,
    pub cache_entries: Vec<crate::cache::CacheEntry>,
}

/// Export recent entries and cache entries for sharing.
pub fn export_share_data(
    log_entries: Vec<crate::types::LogEntry>,
    _cache: &ScriptCache,
) -> Result<String> {
    let _home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("HOME not set")?;

    let instance_id = crate::identity::Identity::load_or_create()?.instance_id;

    let window = SharedWindow {
        schema_version: 1,
        created_at: iso_now(),
        source_instance_id: instance_id,
        entries: log_entries,
        cache_entries: Vec::new(), // cache entries are loaded by path
    };

    let json = serde_json::to_string_pretty(&window)?;
    Ok(json)
}

/// Import shared window data: save entries and cache entries.
pub fn import_share_data(json_data: &str, log_store: &crate::log::LogStore) -> Result<usize> {
    let window: SharedWindow = serde_json::from_str(json_data)
        .context("invalid shared window data")?;

    let mut count = 0;
    for entry in &window.entries {
        log_store.append(entry.clone())?;
        count += 1;
    }

    log_store.flush()?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_integration_manager_new_creates_empty_records() {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let mgr = IntegrationManager::new().unwrap();
        assert!(mgr.list_programs().is_empty());

        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn test_integration_manager_save_and_load() {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        {
            let mut mgr = IntegrationManager::new().unwrap();
            let record = IntegrationRecord {
                schema_version: 1,
                program: "git".to_string(),
                status: LearningStatus::Learned,
                script_id: Some("script123".to_string()),
                help_snippet: Some("Git is a version control system...".to_string()),
                learned_at: Some("2026-06-25T00:00:00Z".to_string()),
                source: LearnSource::Help,
            };
            mgr.records.insert("git".to_string(), record);
            mgr.save_all().unwrap();
        }

        {
            let mgr = IntegrationManager::new().unwrap();
            let programs = mgr.list_programs();
            assert_eq!(programs.len(), 1);
            assert_eq!(programs[0].program, "git");
            assert_eq!(programs[0].status, LearningStatus::Learned);
        }

        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn test_integration_manager_learn_nonexistent_program() {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let mut mgr = IntegrationManager::new().unwrap();
        let result = mgr.learn_program("__nonexistent_program_xyz__").unwrap();
        assert!(matches!(result.status, LearningStatus::Failed(_)));

        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn test_forget_program() {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let mut mgr = IntegrationManager::new().unwrap();
        mgr.records.insert(
            "git".to_string(),
            IntegrationRecord {
                schema_version: 1,
                program: "git".to_string(),
                status: LearningStatus::Learned,
                script_id: None,
                help_snippet: None,
                learned_at: None,
                source: LearnSource::Help,
            },
        );
        mgr.save_all().unwrap();

        mgr.forget_program("git").unwrap();
        assert!(mgr.list_programs().is_empty());

        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn test_truncate_help_short() {
        assert_eq!(truncate_help("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_help_long() {
        let long = "a".repeat(100);
        let truncated = truncate_help(&long, 50);
        assert!(truncated.len() <= 50 + "... (truncated)".len());
        assert!(truncated.ends_with("... (truncated)"));
    }

    #[test]
    fn test_export_share_data_creates_valid_json() {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let cache = ScriptCache::new().unwrap();
        let entries = vec![crate::types::LogEntry::new_system_event("i1", "s1", "test")];

        let json = export_share_data(entries, &cache).unwrap();
        let parsed: SharedWindow = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.schema_version, 1);
        assert_eq!(parsed.entries.len(), 1);

        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
    }
}
