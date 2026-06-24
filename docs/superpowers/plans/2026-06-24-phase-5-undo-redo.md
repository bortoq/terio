# Phase 5 Undo/Redo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add experimental snapshot-backed undo/redo for `terio ask` and cache-script execution, with a bubblewrap sandbox backend and graceful fallback.

**Architecture:** Introduce a focused `undo` module that owns snapshot manifests, path discovery, sandbox wrapping, and undo/redo state. Integrate it into ask/cache execution and surface it through CLI, logs, config, and UI without changing direct `terio run` semantics.

**Tech Stack:** Rust, serde, std::fs, Dioxus desktop UI, existing CLI/log/config infrastructure

---

### Task 1: Add undo config and command surface

**Files:**
- Modify: `src/config.rs`
- Modify: `src/cli.rs`
- Modify: `src/lib.rs`

- [ ] Add undo config types and defaults.
- [ ] Add `undo` and `redo` CLI commands.
- [ ] Export the new undo module from `lib.rs`.

### Task 2: Build undo module with failing tests first

**Files:**
- Create: `src/undo.rs`

- [ ] Write tests for path inference, snapshot persistence, restore, latest-state transitions, and bubblewrap wrapping.
- [ ] Implement minimal snapshot storage and restore logic.
- [ ] Implement bubblewrap wrapper construction and fallback detection.

### Task 3: Integrate undo into ask/cache execution

**Files:**
- Modify: `src/ask.rs`
- Modify: `src/run.rs`
- Modify: `src/main.rs`

- [ ] Add direct-run warning path for unsupported undo.
- [ ] Capture undo sessions around eligible ask/cache executions.
- [ ] Add `terio undo` and `terio redo` handlers and system-event logging.

### Task 4: Add UI controls and visibility

**Files:**
- Modify: `src/ui/app.rs`

- [ ] Surface latest undo state in the UI.
- [ ] Add non-blocking Undo/Redo buttons.
- [ ] Add tests for button visibility/state helpers.

### Task 5: Update docs and verify

**Files:**
- Modify: `README.md`
- Modify: `docs/trust-model.md`
- Modify: `docs/mvp.md`
- Modify: `roadmap.md`

- [ ] Document the experimental semantics and direct-run exclusion.
- [ ] Mark phase 5 complete.
- [ ] Run `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo build`, `cargo build --no-default-features`, and `cargo test`.
