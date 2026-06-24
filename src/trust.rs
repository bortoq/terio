// Trust engine: policies, auto-run logic, fuzzy match rules.

use crate::cache::CacheEntry;
use crate::cache::CachedStep;
use crate::config::Config;
use crate::types::RiskLevel;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

/// Политика доверия для скрипта.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrustPolicy {
    /// Всегда спрашивать подтверждение перед выполнением.
    AlwaysAsk,
    /// Спросить один раз, потом auto-run если условия соблюдены.
    AskOnce,
    /// Auto-run без подтверждения (если условия соблюдены).
    #[default]
    Allow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TrustMatchKind {
    Exact,
    Fuzzy,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustEvaluation {
    pub policy: TrustPolicy,
    pub match_kind: TrustMatchKind,
    pub scope_ok: bool,
    pub path_boundary_ok: bool,
    pub eligible_for_auto_run: bool,
    pub requires_confirmation: bool,
    pub trust_label: String,
    pub reason: String,
}

/// Проверяет, можно ли auto-run для entry.
/// Условия:
///   - match_policy == "exact_normalized" (fuzzy никогда не auto-run)
///   - success_count >= trust_threshold
///   - risk <= LocalWrite (не destructive/network_write/credential_access/financial)
///   - scope соответствует текущей cwd
pub fn can_auto_run(entry: &CacheEntry) -> bool {
    // Fuzzy match — никогда auto-run
    if entry.match_policy != "exact_normalized" {
        return false;
    }

    // Недостаточно успешных запусков
    if entry.success_count < entry.trust_threshold {
        return false;
    }

    // Высокорисковые операции — только с подтверждением
    matches!(
        entry.risk,
        RiskLevel::ReadOnly | RiskLevel::LocalWrite | RiskLevel::NetworkRead
    )
}

/// Проверяет, нужно ли auto-run с учётом политики.
pub fn should_auto_run(entry: &CacheEntry, config: &Config) -> bool {
    let policy = resolve_policy(entry, config);

    match policy {
        TrustPolicy::AlwaysAsk => false,
        TrustPolicy::AskOnce => {
            // AskOnce: первый раз спросить, потом auto-run
            entry.success_count > 1 && can_auto_run(entry)
        }
        TrustPolicy::Allow => can_auto_run(entry),
    }
}

/// Добавляет политику для скрипта в конфиг.
pub fn set_policy(config: &mut Config, request_hash: &str, policy: TrustPolicy) {
    config
        .policy_overrides
        .insert(request_hash.to_string(), policy);
}

/// Возвращает политику для скрипта (из переопределений или дефолт).
pub fn get_policy<'a>(entry: &CacheEntry, config: &'a Config) -> &'a TrustPolicy {
    config
        .policy_overrides
        .get(&entry.request_hash)
        .unwrap_or(match entry.risk {
            RiskLevel::ReadOnly => &config.default_trust_policy,
            RiskLevel::LocalWrite | RiskLevel::NetworkRead => &TrustPolicy::AskOnce,
            _ => &TrustPolicy::AlwaysAsk,
        })
}

/// Форматирует trust level как строку для UI.
pub fn trust_level_str(success_count: u32, trust_threshold: u32) -> String {
    if success_count >= trust_threshold {
        format!("✓ {}/{}", success_count, trust_threshold)
    } else {
        format!("{}/{}", success_count, trust_threshold)
    }
}

pub fn evaluate_cache_entry(
    entry: &CacheEntry,
    config: &Config,
    cwd: &str,
) -> Result<TrustEvaluation> {
    let match_kind = classify_match_kind(&entry.match_policy);
    let scope_ok = scope_matches(entry, cwd);
    let path_boundary_ok = validate_step_paths(&entry.steps, cwd).is_ok();
    let policy = resolve_policy(entry, config).clone();
    let eligible_for_auto_run = scope_ok
        && path_boundary_ok
        && can_auto_run(entry)
        && matches!(policy, TrustPolicy::Allow | TrustPolicy::AskOnce);
    let requires_confirmation = !eligible_for_auto_run;

    let reason = if !scope_ok {
        "scope_mismatch".to_string()
    } else if !path_boundary_ok {
        "path_boundary_violation".to_string()
    } else if match_kind == TrustMatchKind::Fuzzy {
        "fuzzy_match_requires_confirmation".to_string()
    } else {
        match policy {
            TrustPolicy::AlwaysAsk => "policy_always_ask".to_string(),
            TrustPolicy::AskOnce if entry.success_count <= 1 => "policy_ask_once".to_string(),
            TrustPolicy::Allow | TrustPolicy::AskOnce if eligible_for_auto_run => {
                "eligible_for_auto_run".to_string()
            }
            _ => "trust_threshold_not_met".to_string(),
        }
    };

    Ok(TrustEvaluation {
        policy,
        match_kind,
        scope_ok,
        path_boundary_ok,
        eligible_for_auto_run,
        requires_confirmation,
        trust_label: trust_level_str(entry.success_count, entry.trust_threshold),
        reason,
    })
}

pub fn validate_step_paths(steps: &[CachedStep], cwd: &str) -> Result<()> {
    for step in steps {
        for arg in step.argv.iter().skip(1) {
            if !looks_like_local_path(arg) {
                continue;
            }
            ensure_path_within_boundary(arg, cwd)?;
        }
    }
    Ok(())
}

fn resolve_policy<'a>(entry: &CacheEntry, config: &'a Config) -> &'a TrustPolicy {
    get_policy(entry, config)
}

fn classify_match_kind(match_policy: &str) -> TrustMatchKind {
    match match_policy {
        "exact_normalized" => TrustMatchKind::Exact,
        "fuzzy" => TrustMatchKind::Fuzzy,
        _ => TrustMatchKind::Unknown,
    }
}

fn scope_matches(entry: &CacheEntry, cwd: &str) -> bool {
    match entry.scope.cwd_policy.as_str() {
        "same_cwd_only" => entry.scope.cwd == cwd,
        _ => true,
    }
}

fn looks_like_local_path(arg: &str) -> bool {
    if arg.starts_with('-') || arg.contains("://") {
        return false;
    }

    arg.contains('/') || arg.contains('\\') || arg.starts_with('.')
}

fn ensure_path_within_boundary(raw: &str, cwd: &str) -> Result<()> {
    let path = Path::new(raw);
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        bail!("parent traversal is not allowed: {raw}");
    }

    let base = canonicalize_or_self(Path::new(cwd));
    let candidate = if path.is_absolute() {
        canonicalize_or_self(path)
    } else {
        canonicalize_or_self(&PathBuf::from(cwd).join(path))
    };

    if !candidate.starts_with(&base) {
        bail!("path escapes cwd boundary: {raw}");
    }

    Ok(())
}

fn canonicalize_or_self(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::CachedStep;
    use crate::cache::{CacheEntry, ScriptScope};
    use anyhow::Result;

    fn make_entry(
        success_count: u32,
        trust_threshold: u32,
        risk: RiskLevel,
        cwd: &str,
    ) -> CacheEntry {
        CacheEntry {
            schema_version: 1,
            script_id: "s1".to_string(),
            request_hash: "h1".to_string(),
            version: 1,
            normalized_request: "test".to_string(),
            match_policy: "exact_normalized".to_string(),
            scope: ScriptScope {
                cwd_policy: "same_cwd_only".to_string(),
                cwd: cwd.to_string(),
            },
            risk,
            parameters: serde_json::json!({}),
            preconditions: vec![],
            steps: vec![],
            artifacts: vec![],
            success_count,
            trust_threshold,
            created_at: "now".to_string(),
            last_used_at: "now".to_string(),
        }
    }

    #[test]
    fn test_can_auto_run_basic() {
        let entry = make_entry(3, 3, RiskLevel::ReadOnly, "/tmp");
        assert!(can_auto_run(&entry));
    }

    #[test]
    fn test_can_auto_run_not_enough_success() {
        let entry = make_entry(2, 3, RiskLevel::ReadOnly, "/tmp");
        assert!(!can_auto_run(&entry));
    }

    #[test]
    fn test_can_auto_run_destructive_never() {
        let entry = make_entry(10, 3, RiskLevel::Destructive, "/tmp");
        assert!(!can_auto_run(&entry));
    }

    #[test]
    fn test_can_auto_run_fuzzy_never() {
        let mut entry = make_entry(10, 3, RiskLevel::ReadOnly, "/tmp");
        entry.match_policy = "fuzzy".to_string();
        assert!(!can_auto_run(&entry));
    }

    #[test]
    fn test_should_auto_run_allow() {
        let entry = make_entry(3, 3, RiskLevel::ReadOnly, "/tmp");
        let mut config = Config::default();
        config.default_trust_policy = TrustPolicy::Allow;
        assert!(should_auto_run(&entry, &config));
    }

    #[test]
    fn test_should_auto_run_always_ask() {
        let entry = make_entry(10, 3, RiskLevel::ReadOnly, "/tmp");
        let mut config = Config::default();
        config.default_trust_policy = TrustPolicy::AlwaysAsk;
        assert!(!should_auto_run(&entry, &config));
    }

    #[test]
    fn test_should_auto_run_override() {
        let entry = make_entry(3, 3, RiskLevel::ReadOnly, "/tmp");
        let mut config = Config::default();
        config.default_trust_policy = TrustPolicy::AlwaysAsk;
        config
            .policy_overrides
            .insert("h1".to_string(), TrustPolicy::Allow);
        assert!(should_auto_run(&entry, &config));
    }

    #[test]
    fn test_trust_level_str() {
        assert_eq!(trust_level_str(3, 3), "✓ 3/3");
        assert_eq!(trust_level_str(2, 3), "2/3");
    }

    #[test]
    fn test_default_policy() {
        let entry = make_entry(1, 3, RiskLevel::ReadOnly, "/tmp");
        let config = Config::default();
        // read_only + success=1 + Allow = can_auto_run false (not enough success)
        assert!(!should_auto_run(&entry, &config));
    }

    #[test]
    fn test_evaluate_cache_entry_blocks_scope_mismatch() -> Result<()> {
        let entry = make_entry(5, 3, RiskLevel::ReadOnly, "/tmp/a");
        let eval = evaluate_cache_entry(&entry, &Config::default(), "/tmp/b")?;
        assert!(!eval.scope_ok);
        assert!(eval.requires_confirmation);
        assert!(!eval.eligible_for_auto_run);
        Ok(())
    }

    #[test]
    fn test_evaluate_cache_entry_fuzzy_never_autoruns() -> Result<()> {
        let mut entry = make_entry(5, 3, RiskLevel::ReadOnly, "/tmp");
        entry.match_policy = "fuzzy".into();
        let eval = evaluate_cache_entry(&entry, &Config::default(), "/tmp")?;
        assert_eq!(eval.match_kind, TrustMatchKind::Fuzzy);
        assert!(!eval.eligible_for_auto_run);
        assert!(eval.requires_confirmation);
        Ok(())
    }

    #[test]
    fn test_validate_step_paths_rejects_parent_traversal() {
        let step = CachedStep {
            command: "cat".into(),
            argv: vec!["cat".into(), "../../secret.txt".into()],
            risk: RiskLevel::ReadOnly,
        };
        assert!(validate_step_paths(&[step], "/tmp/project").is_err());
    }
}
