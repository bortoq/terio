# Phase 4 Hardening And Provider Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish phase 4 with hash-bound pending confirmation, stricter provider validation, safer cache/pending handling, improved OpenAI provider semantics, and a less fragile UI execution path.

**Architecture:** Keep trust and execution semantics centralized in `ask.rs` and `trust.rs`, with the UI acting as a thin client over saved pending state and the log. Extend the provider contract carefully: runtime validation and optional `cache_template` parsing should enrich existing behavior without introducing a second execution model.

**Tech Stack:** Rust, serde/serde_json, clap, Dioxus desktop, ureq, chrono, tempfile unit tests

---

### Task 1: Harden provider plan validation and hash-bound confirmation

**Files:**
- Modify: `src/ask.rs`
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Test: `src/ask.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn test_pending_confirmation_requires_matching_hash() { /* save preview+exec, tamper preview, confirm should decline */ }

#[test]
fn test_provider_cache_template_command_mismatch_rejected() { /* mismatched command/argv[0] => Declined */ }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_pending_confirmation_requires_matching_hash test_provider_cache_template_command_mismatch_rejected`
Expected: FAIL because pending state is not hash-bound and provider validation is incomplete.

- [ ] **Step 3: Write minimal implementation**

```rust
pub struct PendingConfirmationState {
    pub plan_hash: String,
    // ...
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_pending_confirmation_requires_matching_hash test_provider_cache_template_command_mismatch_rejected`
Expected: PASS

### Task 2: Add cache_template support and stricter provider response handling

**Files:**
- Modify: `src/provider.rs`
- Modify: `src/agent.rs`
- Modify: `src/ask.rs`
- Test: `src/provider.rs`
- Test: `src/ask.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn test_provider_parses_cache_template_steps() { /* response with cache_template should be preserved */ }

#[test]
fn test_process_request_uses_cache_template_steps_when_present() { /* saved cache uses template steps, not raw commands */ }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_provider_parses_cache_template_steps test_process_request_uses_cache_template_steps_when_present`
Expected: FAIL because cache_template is ignored.

- [ ] **Step 3: Write minimal implementation**

```rust
pub struct AgentPlan {
    pub summary: String,
    pub risk: RiskLevel,
    pub commands: Vec<AgentCommand>,
    pub cache_template: Option<AgentCacheTemplate>,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_provider_parses_cache_template_steps test_process_request_uses_cache_template_steps_when_present`
Expected: PASS

### Task 3: Improve secret handling and output size limits

**Files:**
- Modify: `src/cache.rs`
- Modify: `src/ask.rs`
- Modify: `src/run.rs`
- Test: `src/cache.rs`
- Test: `src/ask.rs`
- Test: `src/run.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn test_pending_exec_file_is_permissioned_private() { /* unix perms 0600 */ }

#[test]
fn test_execute_truncates_large_output() { /* long stdout/stderr gets capped */ }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_pending_exec_file_is_permissioned_private test_execute_truncates_large_output`
Expected: FAIL because pending exec perms/size limits are incomplete.

- [ ] **Step 3: Write minimal implementation**

```rust
const MAX_CAPTURE_BYTES: usize = 64 * 1024;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_pending_exec_file_is_permissioned_private test_execute_truncates_large_output`
Expected: PASS

### Task 4: Upgrade UI execution flow and details rendering

**Files:**
- Modify: `src/ui/app.rs`
- Test: `src/ui/app.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn test_prepare_rows_uses_stdout_or_description_summary() { /* details visible in row data */ }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_prepare_rows_uses_stdout_or_description_summary`
Expected: FAIL because UI only shows command/display summary.

- [ ] **Step 3: Write minimal implementation**

```rust
// move child execution into background thread and expose stdout/stderr summary/details in UI rows
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_prepare_rows_uses_stdout_or_description_summary`
Expected: PASS

### Task 5: OpenAI response_format, usage parsing, and docs alignment

**Files:**
- Modify: `src/provider.rs`
- Modify: `README.md`
- Modify: `docs/agent-protocol.md`
- Modify: `roadmap.md`
- Test: `src/provider.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn test_extract_usage_tokens() { /* usage.total_tokens parsed */ }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_extract_usage_tokens`
Expected: FAIL because usage parsing does not exist.

- [ ] **Step 3: Write minimal implementation**

```rust
// request response_format=json_object where supported; parse usage tokens into provider metadata
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_extract_usage_tokens`
Expected: PASS

### Task 6: Full verification

**Files:**
- Verify: `src/ask.rs`, `src/provider.rs`, `src/cache.rs`, `src/run.rs`, `src/ui/app.rs`, `README.md`, `docs/agent-protocol.md`, `roadmap.md`

- [ ] **Step 1: Run formatting**

Run: `cargo fmt --check`
Expected: PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 3: Run builds**

Run: `cargo build && cargo build --no-default-features`
Expected: PASS

- [ ] **Step 4: Run full tests**

Run: `cargo test`
Expected: PASS
