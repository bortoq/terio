# Roadmap

## Phase 1: Product Definition

- Define the core promise: one command surface for local tools, web services, APIs, media, files, and agents.
- Define the first narrow MVP workflow.
- Define what terio is not: not a full OS, not a full browser replacement, not a universal Photoshop replacement.
- Define the trust model for compiled behavior.
- Define the first rendered output block types.

## Phase 2: Static Prototype

- Build mock documents and screenshots for terminal output as web-rendered blocks.
- Prototype `ls` as a file gallery or table.
- Prototype `git log` as a commit timeline.
- Prototype command output as a readable feed.
- Prototype a modal switch between command mode and rendered reading mode.
- Test whether users understand the product without abstract terms such as "post-program world".

## Phase 3: Local MVP

- Implement local shell command execution.
- Capture stdout, stderr, exit code, working directory, duration, and artifacts.
- Render a small set of command outputs into structured blocks.
- Keep normal plain-text fallback for everything else.
- Add local settings and history storage.
- Add a simple agent command entry for tasks that need reasoning.

## Phase 4: Behavior Compiler MVP

- Record repeated request and command-chain patterns.
- Implement argument extraction for one safe workflow.
- Compile the workflow into a parameterized local script or recipe.
- Add confidence thresholds and failure tracking.
- Add automatic fallback to the agent when the compiled behavior fails.
- Display saved token/time estimates for compiled behavior.

## Phase 5: Connector Expansion

- Add GitHub connector for issues, pull requests, notifications, and CI status.
- Add media connector for playlists, local library state, and playback control.
- Add download/library connector for missing episodes or media files.
- Add file operations connector with previews and confirmation rules.
- Add Self OS integration for trust and delegation reuse.

## Phase 6: Modal Workspace

- Add file preview and edit mode.
- Add media mode for audio/video playback controls and metadata.
- Add table/database mode for structured data inspection.
- Add long-page reading mode for news, logs, documentation, and summaries.
- Keep mode switching explicit and reversible.

## Phase 7: Sharing And Remote Sessions

- Add shareable terminal blocks.
- Add shared read-only sessions.
- Add team behavior recipe libraries.
- Add audit logs for shared actions.
- Evaluate Mosh-style persistent remote sessions.

## Phase 8: Commercial Product

- Package a local desktop build.
- Add paid connector packs or cloud sync.
- Add team plans with shared workflows and permissions.
- Add enterprise controls for audit, policy, and credential isolation.
- Publish measured savings: avoided LLM calls, repeated commands eliminated, time saved, and attention saved.
