# Web Terminal

Web Terminal is a terminal-based standalone interface for controlling local tools, web services, files, media, agents, and APIs from one workspace.

## Simple Description

Web Terminal keeps the command-line power of a terminal, but renders results like a web interface. Commands can produce tables, cards, galleries, timelines, previews, controls, and long readable pages instead of plain scrolling text.

The product also learns repeated user behavior. If a user repeatedly asks for the same workflow, such as splitting a FLAC/CUE album, renaming tracks, copying files, resuming a playlist, or downloading a missing episode, Web Terminal can compile that workflow into a trusted script and run it next time without spending LLM tokens.

## Full Description

Most software forces the user to move between many separate interfaces: terminal, browser, file manager, editor, media player, service dashboards, and AI agent UIs. Each interface has its own commands, layout, shortcuts, state, and mental model. The user pays for that fragmentation with attention.

Web Terminal treats the interface as its own layer. Programs, services, APIs, local tools, and agents remain separate execution engines. Web Terminal becomes the user's command surface and result surface. The user asks for an outcome, the right engine performs the work, and the terminal displays the result in the form that fits the task.

This makes the terminal more than a shell emulator. It becomes an aggregator of control: a browser-like renderer, an agent interface, a workflow recorder, a modal workspace, and a bridge to local and remote functions.

## Product Principle

User attention is the scarce resource. Every app switch, repeated command, hidden menu, manual copy step, and unnecessary LLM call has a cost. Web Terminal should reduce that cost without asking the user to learn a new automation language.

## Market Context

Existing terminals are powerful but mostly text-first. Existing AI terminals add natural language help, but still tend to output blocks of plain text and rerun reasoning for repeated tasks. Existing browsers are excellent renderers, but weak command surfaces. Existing automation tools require users to design workflows in advance.

Web Terminal sits between terminals, browsers, and AI agent interfaces. It keeps terminal control, adds browser-grade rendering, and uses an agent only where reasoning is needed. Repeated behavior should become cheaper and faster over time.

## Main Use Cases

- Copy files, inspect results, and see structured progress without leaving the terminal.
- Split FLAC/CUE albums, rename tracks with a remembered template, and reuse the workflow without another LLM call.
- Resume the last playlist, show the queue, and control playback from the same workspace.
- Detect and download a missing episode in a show season using local library state and configured services.
- Review GitHub issues, branches, commits, pull requests, and CI output as readable cards and timelines.
- Ask for news, logs, or search results and read them as a long browser-like page.
- Edit text, inspect media, query databases, and run commands through modal workspace views.
- Share part of a terminal session with another user or team when collaboration is useful.

## MVP

- Local terminal shell with command execution.
- Web-rendered output blocks for a small set of commands.
- Agent command entry for natural-language tasks.
- Behavior log that records request, command chain, result, and errors.
- Behavior Compiler prototype for one safe repeated workflow.
- Trust threshold before automatic script replay.
- Explicit fallback to agent reasoning when confidence is low or a compiled script fails.
- Basic modal views for command, file preview, and rendered output.

## Similar Projects And Difference

- Plan 9 pursued a unified computing environment, but required a different system model. Web Terminal should work on top of existing systems.
- TermKit explored browser-like terminal output, but lacked modern agents and behavior compilation.
- Hyper proved that terminals can be built with web technology, but did not redefine the terminal as a control layer.
- Extraterm treated command output as objects, but did not combine this with agentic workflow learning.
- Warp added AI to the terminal, but Web Terminal's core bet is broader: web rendering, compiled behavior, modal workspace, and interface aggregation.
- Blink Shell demonstrates mobile terminal workflows and Mosh support, but is not a browser-like agentic workspace.

The main difference is that Web Terminal is not only a nicer terminal. It is a standalone interface layer between the user and executable functions.

## Risks

- The concept can become too broad unless the MVP proves one narrow workflow first.
- Full browser, editor, media, and agent integration can overload the product if built too early.
- Automatic replay of commands requires strict trust, argument validation, logging, and fallback.
- GUI-heavy programs whose interface and engine are tightly coupled may not be practical integration targets.
- Users may not understand the product if it is described as replacing every app. It should start by saving attention in concrete repeated workflows.

## Monetization

- Freemium local app with limited agent minutes or compiled workflows.
- Paid tier for more agent usage, workflow history, cloud sync, and advanced connectors.
- Team tier for shared sessions, shared workflow libraries, and audit logs.
- Marketplace or registry for connector packs and reusable behavior templates.
- Cost-savings display that shows avoided LLM calls and time saved by compiled workflows.

## Documents

See [roadmap.md](roadmap.md), [architecture.md](architecture.md), [use-cases.md](use-cases.md), and [demo.md](demo.md).
