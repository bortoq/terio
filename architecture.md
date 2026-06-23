# Architecture

## Overview

Web Terminal is a standalone interface layer that combines shell execution, web-rendered output, agent control, behavior compilation, modal workspaces, and connectors to external functions.

The first architecture should stay local-first. The terminal runs commands in the user's environment, records workflows, renders selected outputs as structured views, and only calls an LLM when reasoning is actually needed.

## Components

- Command Surface: accepts shell commands, natural-language requests, hotkeys, and modal actions.
- Execution Layer: runs local shell commands and connector actions with explicit process state, exit codes, logs, and permissions.
- Web Renderer: converts command results into readable HTML-like blocks such as tables, cards, galleries, timelines, previews, and progress views.
- Agent Layer: plans unknown tasks, extracts arguments, chooses tools, explains risky actions, and falls back when compiled behavior cannot run.
- Behavior Log: stores user request, resolved arguments, command chain, result summary, exit status, duration, and error signals.
- Behavior Compiler: detects repeated successful patterns and turns them into parameterized scripts or recipes.
- Trust Engine: decides when a compiled behavior can run automatically based on confidence, prior success, failure history, and action risk.
- Modal Workspace: switches between command mode, rendered reading mode, file edit mode, media mode, database/table mode, and shared-session mode.
- Connector Layer: integrates with local filesystem, Git, media tools, download managers, APIs, calendars, mail, GitHub, and future Self OS plugins.
- Session Sharing Layer: exposes selected terminal blocks, sessions, or behavior recipes to other users without sharing the whole machine.
- Storage Layer: stores preferences, history, compiled scripts, trust scores, connector credentials, and rendering metadata.
- Safety And Permissions Layer: isolates credentials, validates script arguments, blocks destructive auto-actions, and shows confirmations for risky operations.

## Data Flow

1. The user enters a command or natural-language request.
2. The command surface classifies it as direct shell, known compiled behavior, connector action, or agent task.
3. If a trusted compiled behavior matches, the Trust Engine validates confidence and arguments.
4. Trusted behavior executes directly through the Execution Layer.
5. Unknown, low-confidence, or failed behavior falls back to the Agent Layer.
6. The Execution Layer returns output, exit code, logs, artifacts, and metadata.
7. The Web Renderer chooses an appropriate display block.
8. The Behavior Log stores the request, actions, arguments, result, and errors.
9. The Behavior Compiler periodically detects repeated successful patterns.
10. When a pattern is stable, Web Terminal offers or silently prepares a reusable behavior depending on risk level.

## Behavior Compiler

The Behavior Compiler caches behavior, not answers. It should not store a brittle LLM response and replay it blindly. It should extract a stable workflow with parameters.

For example, "split this FLAC/CUE album and rename tracks like last time" becomes a recipe with arguments for the FLAC file, CUE file, output directory, and naming template. If the recipe has enough successful history and the current arguments validate, it runs without another LLM call.

## Trust Rule

Automatic execution requires confidence, successful history, safe action type, valid arguments, and observable rollback or fallback behavior. Destructive, financial, publishing, credential, and external-send actions should require confirmation unless the user explicitly changes the policy.

## Rendering Rule

Output should be rendered in the form that reduces attention cost. Plain text remains valid when plain text is best. Structured views are useful only when they make results easier to inspect, compare, navigate, or act on.

## Integration Boundary

Web Terminal should start with engines that already expose usable control surfaces:

- command-line tools such as git, ffmpeg, curl, rsync, and package managers;
- local files and open data formats;
- APIs such as GitHub, media managers, calendars, mail, and download services;
- Self OS delegation and trust components;
- browser-readable content such as news feeds, search results, logs, and dashboards.

GUI-heavy products whose functionality is inseparable from their interface are not first-phase targets. Web Terminal should not pretend to replace every application immediately.

## Key Design Rule

The user should feel like a commander, not an operator of many unrelated programs. The interface should ask for outcomes, show what happened clearly, and turn repeated successful behavior into cheaper execution.
