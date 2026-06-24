# Phase 4 Trust And Safety Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete roadmap phase 4 by enforcing trust policy evaluation in backend execution paths and exposing plan confirmation, trust indicators, and config controls in the desktop UI.

**Architecture:** Keep trust decisions centralized in `src/trust.rs` so CLI and UI both consume the same evaluation results. Extend ask/config/log/UI layers just enough to carry trust metadata and pending confirmation state without creating a second execution path.

**Tech Stack:** Rust, clap, serde, Dioxus desktop UI, JSONL log storage, tempfile-based unit tests

---

### Task 1: Define trust evaluation primitives

**Files:**
- Modify: `src/trust.rs`
- Modify: `src/types.rs`
- Test: `src/trust.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn test_evaluate_cache_entry_blocks_scope_mismatch() {
    let entry = make_entry(5, 3, RiskLevel::ReadOnly, "/tmp/a");
    let eval = evaluate_cache_entry(&entry, &Config::default(), "/tmp/b").unwrap();
    assert!(!eval.scope_ok);
    assert!(eval.requires_confirmation);
}

#[test]
fn test_evaluate_cache_entry_fuzzy_never_autoruns() {
    let mut entry = make_entry(5, 3, RiskLevel::ReadOnly, "/tmp");
    entry.match_policy = "fuzzy".into();
    let eval = evaluate_cache_entry(&entry, &Config::default(), "/tmp").unwrap();
    assert_eq!(eval.match_kind, TrustMatchKind::Fuzzy);
    assert!(!eval.eligible_for_auto_run);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test trust::tests::test_evaluate_cache_entry_blocks_scope_mismatch trust::tests::test_evaluate_cache_entry_fuzzy_never_autoruns`
Expected: FAIL because `evaluate_cache_entry` and related types do not exist yet.

- [ ] **Step 3: Write minimal implementation**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TrustMatchKind { Exact, Fuzzy, Unknown }

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

pub fn evaluate_cache_entry(entry: &CacheEntry, config: &Config, cwd: &str) -> anyhow::Result<TrustEvaluation> {
    // resolve policy, match kind, scope_ok, auto-run eligibility, and confirmation requirement
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test trust::tests::test_evaluate_cache_entry_blocks_scope_mismatch trust::tests::test_evaluate_cache_entry_fuzzy_never_autoruns`
Expected: PASS

### Task 2: Enforce path boundary validation for cached steps and plans

**Files:**
- Modify: `src/trust.rs`
- Modify: `src/ask.rs`
- Test: `src/trust.rs`
- Test: `src/ask.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn test_validate_step_paths_rejects_parent_traversal() {
    let step = CachedStep {
        command: "cat".into(),
        argv: vec!["cat".into(), "../../secret.txt".into()],
        risk: RiskLevel::ReadOnly,
    };
    assert!(validate_step_paths(std::slice::from_ref(&step), "/tmp/project").is_err());
}

#[test]
fn test_process_request_declines_cache_hit_with_invalid_path() {
    // Cache entry with argv ["cat", "../escape.txt"] should return Declined or Blocked before execution.
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test validate_step_paths process_request_declines_cache_hit_with_invalid_path`
Expected: FAIL because path validation is not enforced.

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn validate_step_paths(steps: &[CachedStep], cwd: &str) -> anyhow::Result<()> {
    // inspect relative path arguments, reject `..` and canonicalized paths outside cwd
}

fn current_cwd_string() -> Result<String> {
    Ok(std::env::current_dir()?.to_string_lossy().to_string())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test validate_step_paths process_request_declines_cache_hit_with_invalid_path`
Expected: PASS

### Task 3: Route ask flow through trust evaluation and explicit confirmation state

**Files:**
- Modify: `src/ask.rs`
- Modify: `src/provider.rs`
- Modify: `src/types.rs`
- Test: `src/ask.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn test_cache_hit_requires_confirmation_for_ask_once_before_threshold() {
    // exact match, local_write, ask_once override, success_count == 1
    // expect AskResult::PendingConfirmation { .. }
}

#[test]
fn test_from_agent_returns_pending_confirmation_for_local_write_plan() {
    // provider returns local_write plan, skip_confirm = false
    // expect AskResult::PendingConfirmation { .. }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test cache_hit_requires_confirmation from_agent_returns_pending_confirmation`
Expected: FAIL because `AskResult::PendingConfirmation` does not exist.

- [ ] **Step 3: Write minimal implementation**

```rust
pub enum AskResult {
    CacheHit { /* existing */ },
    FromAgent { /* existing */ },
    PendingConfirmation {
        source: PendingSource,
        plan_summary: PendingPlanSummary,
    },
    Unknown,
    Declined,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test cache_hit_requires_confirmation from_agent_returns_pending_confirmation`
Expected: PASS

### Task 4: Expose trust settings through config model and CLI parsing

**Files:**
- Modify: `src/config.rs`
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Test: `src/config.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn test_config_set_policy_override() {
    let mut config = Config::default();
    config.set("policy_override.h1", "always_ask").unwrap();
    assert_eq!(config.policy_overrides["h1"], TrustPolicy::AlwaysAsk);
}

#[test]
fn test_config_roundtrips_ui_preferences() {
    let mut config = Config::default();
    config.ui.show_config = true;
    config.ui.last_selected_policy = Some("ask_once".into());
    config.save().unwrap();
    let loaded = Config::load().unwrap();
    assert!(loaded.ui.show_config);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test config_set_policy_override config_roundtrips_ui_preferences`
Expected: FAIL because override key parsing and UI config fields do not exist.

- [ ] **Step 3: Write minimal implementation**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiConfig {
    pub show_config: bool,
    pub last_selected_policy: Option<String>,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test config_set_policy_override config_roundtrips_ui_preferences`
Expected: PASS

### Task 5: Add UI plan confirmation panel and trust badges

**Files:**
- Modify: `src/ui/app.rs`
- Modify: `src/types.rs`
- Modify: `src/main.rs`
- Test: `src/ui/app.rs` or compile coverage through `cargo test`

- [ ] **Step 1: Write the failing test or compile target**

```rust
#[test]
fn test_pending_plan_summary_serializes_for_ui() {
    let summary = PendingPlanSummary { /* fields */ };
    let json = serde_json::to_string(&summary).unwrap();
    assert!(json.contains("requires_confirmation"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test pending_plan_summary_serializes_for_ui`
Expected: FAIL because pending-plan UI types do not exist.

- [ ] **Step 3: Write minimal implementation**

```rust
struct RowData {
    // existing fields
    trust: String,
}

// render pending plan section above the log table with Accept / Decline buttons
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test pending_plan_summary_serializes_for_ui`
Expected: PASS

### Task 6: Add basic config window in UI and wire persistence

**Files:**
- Modify: `src/ui/app.rs`
- Modify: `src/config.rs`
- Modify: `src/main.rs`
- Test: `src/config.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_config_print_includes_policy_override_count() {
    let mut config = Config::default();
    config.policy_overrides.insert("abc".into(), TrustPolicy::Allow);
    let rendered = config.render_for_display();
    assert!(rendered.contains("1 overrides"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test config_print_includes_policy_override_count`
Expected: FAIL because `render_for_display` does not exist.

- [ ] **Step 3: Write minimal implementation**

```rust
impl Config {
    pub fn render_for_display(&self) -> String {
        // shared formatting used by CLI and UI config panel
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test config_print_includes_policy_override_count`
Expected: PASS

### Task 7: Full verification

**Files:**
- Modify: `roadmap.md`
- Verify: `src/trust.rs`, `src/ask.rs`, `src/config.rs`, `src/ui/app.rs`

- [ ] **Step 1: Run focused test suite**

Run: `cargo test trust::tests ask::tests config::tests provider::tests`
Expected: PASS

- [ ] **Step 2: Run full project tests**

Run: `cargo test`
Expected: PASS

- [ ] **Step 3: Run build with desktop feature**

Run: `cargo build --features desktop`
Expected: PASS

- [ ] **Step 4: Mark roadmap phase items done**

```markdown
- [x] Policy: always_ask / ask_once / allow
- [x] Auto-run: exact match + risk <= local_write + N успехов + scope соблюдён
- [x] Fuzzy match: никогда auto-run, только подтверждение
- [x] Path boundary validation (защита от ../../)
- [x] Отображение подтверждения плана в UI (risk, команды, accept/decline)
- [x] Индикатор trust level для каждой команды в UI
- [x] Настройки в UI — окно конфигурации
```
