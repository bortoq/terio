# Phase 6 Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a live multi-view workspace UI over one shared event stream.

**Architecture:** Add a hybrid snapshot+live UI runtime, route UI actions through an in-process worker, and render the same `LogEntry` list through multiple view modes plus an auto-renderer.

**Tech Stack:** Rust, Dioxus desktop, tokio broadcast, existing log/config/ask modules

---

### Task 1: Add hybrid runtime wiring

**Files:**
- Modify: `src/ui/app.rs`
- Modify: `src/main.rs`

- [ ] Add live receiver and UI action sender plumbing.
- [ ] Route UI actions through an in-process worker.
- [ ] Keep refresh as fallback for external changes.

### Task 2: Add workspace state and renderers

**Files:**
- Modify: `src/ui/app.rs`

- [ ] Add workspace mode state and activity state.
- [ ] Implement `Table`, `Timeline`, `Cards`, `Readable`, `Chat`, and `Auto`.
- [ ] Add details pane over the same selected event state.

### Task 3: Verify renderer selection and live helpers

**Files:**
- Modify: `src/ui/app.rs`

- [ ] Add tests for renderer selection.
- [ ] Add tests for activity/completion helpers and labels.

### Task 4: Update docs and roadmap

**Files:**
- Modify: `README.md`
- Modify: `docs/mvp.md`
- Modify: `roadmap.md`

- [ ] Document hybrid loading and shared workspace model.
- [ ] Mark phase 6 complete.
