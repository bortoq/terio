# Phase 5 Undo/Redo Design

## Scope

Phase 5 covers `terio ask` executions and cache-script replay only. Direct `terio run -- ...` stays outside snapshot and undo/redo guarantees and must show an explicit warning.

## Goals

- Add experimental undo/redo that is off by default.
- Add sandbox abstraction with a `bubblewrap` backend and a graceful fallback to warn mode.
- Capture best-effort filesystem snapshots for script executions with `risk >= local_write`.
- Expose `terio undo` and `terio redo`.
- Add Undo/Redo controls to the desktop UI.

## Non-goals

- No undo/redo guarantees for `terio run -- ...`.
- No attempt to replay shell commands during redo.
- No full filesystem sandbox on platforms without `bubblewrap`.
- No claim of complete coverage for shell wrappers such as `sh -c`.

## Architecture

### Undo domain

Add a dedicated `undo` module that owns:

- config-facing enums for `undo.mode = warn|bubblewrap`
- best-effort path discovery from execution steps
- snapshot storage under `~/.terio/undo/<operation-id>/`
- manifest persistence for latest operation state
- `undo_latest()` and `redo_latest()` entry points

### Snapshot model

For eligible script execution:

1. discover candidate paths relative to current CWD
2. capture `before` snapshot
3. execute commands, optionally wrapped in sandbox
4. capture `after` snapshot on success
5. persist manifest with `applied` state

Undo restores the `before` snapshot. Redo restores the `after` snapshot. Neither path re-runs shell commands.

### Sandbox model

The sandbox abstraction decides per execution:

- `warn`: run commands directly
- `bubblewrap`: if `bwrap` is present, wrap command in a read-mostly sandbox with writable binds for CWD and HOME; otherwise log a fallback notice and run direct

Sandboxing is only attempted for `ask`/cache script execution, never for direct `run`.

## UX

- Feature is disabled by default via config.
- `terio run -- ...` prints that undo/redo is unsupported for direct run.
- `terio undo` and `terio redo` print clear status if nothing is available.
- UI adds `Undo` and `Redo` buttons and shows availability from latest undo record.

## Risks

- Best-effort path inference can miss writes hidden behind shell strings or tool-specific config files.
- Bubblewrap may be unavailable or too restrictive on some systems; fallback must stay explicit.
- Snapshot restore can overwrite user changes made after the original execution.

## Testing

- unit tests for path inference
- unit tests for snapshot round-trip
- unit tests for manifest state transitions
- unit tests for bubblewrap command wrapping and fallback detection
- integration-style tests for ask-script execution producing undo records
- CLI tests for disabled-by-default behavior and direct-run warning
