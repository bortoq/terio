# Phase 6 Workspace Design

## Goal

Turn the desktop UI into a unified multi-view workspace over one shared event log.

## Decisions

- One source of truth: the log/event stream.
- One UI state store: snapshot + live updates over the same `LogEntry` list.
- Chat mode is only a renderer over the same event stream, not a separate protocol.
- Initial load uses `LogReader.recent(N)`, then live in-process updates arrive through `LogStore` broadcast.
- External processes remain visible through manual refresh fallback.

## Views

- `Auto`: picks a renderer from `display_profile` and log kind.
- `Table`: dense operational table.
- `Timeline`: chronological execution narrative.
- `Cards`: status/risk cards.
- `Readable`: document-like page.
- `Chat`: conversational renderer over the same events.

## Runtime model

- `launch_ui()` creates one `LogStore`, gets `recent(N)`, subscribes to `stream()`, and spawns an in-process UI command worker.
- UI actions (`ask`, `confirm`, `undo`, `redo`) are dispatched to that worker instead of spawning a new `terio` process.
- Worker writes log events into the same store, so Dioxus receives them live through broadcast.

## Layout

- Header with mode switches, activity state, undo/redo state, and input.
- Main workspace pane for the chosen renderer.
- Details pane bound to the currently selected event.

## Risks

- Live broadcast only covers actions executed inside the current process.
- External log writers still require snapshot refresh.
- Renderer heuristics remain intentionally lightweight in this phase.
