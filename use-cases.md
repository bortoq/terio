# Use Cases

## FLAC/CUE Album Split

A user asks Web Terminal to split an album image into tracks. The agent identifies the FLAC file, CUE file, naming pattern, and output directory. The first runs are handled through normal tools such as `ffmpeg`, `shnsplit`, `cuetag`, or equivalent local commands.

After the same workflow succeeds several times, Behavior Compiler saves a parameterized recipe. Later, the user can ask the same thing in natural language and Web Terminal runs the recipe directly without an LLM call.

## Missing Episode In A Season

A user asks which episode is missing from a show season. Web Terminal inspects the local folder or media library, compares existing files to expected episode numbers, and uses a configured downloader or indexer to fetch the missing item.

The result is shown as a structured season table: present episodes, missing episodes, download status, and final file location.

## Resume Last Playlist

A user asks to play music from the last listened playlist. Web Terminal connects to the configured local player or service API, resumes the queue, shows the current track, and exposes keyboard controls.

The terminal becomes a media controller without forcing the user to open a separate music app.

## File Copy With Visual Progress

A user copies a large directory. Web Terminal runs the file operation, shows progress as a readable card, displays throughput, skipped files, warnings, and final destination, then offers a quick action to open or inspect the result.

The underlying operation remains a normal local command. The interface improves visibility.

## GitHub Work Session

A user asks what needs attention in a repository. Web Terminal fetches issues, pull requests, notifications, CI status, and local branch state. The result appears as a prioritized work feed with links to actions.

The user can review a PR, run tests, push a branch, and create a summary without switching between terminal, browser, and GitHub UI.

## Browser-Like News Reading

A user asks for news or updates on a topic. Web Terminal retrieves sources through configured feeds or search connectors and renders the result as a long readable page.

This uses the terminal's natural autoscroll and history while adopting the browser's readability.

## Modal Editing

A user opens a text file, config file, cue sheet, subtitle, or script from command output. Web Terminal switches from command mode to edit mode in the same workspace.

The user edits, saves, returns to command mode, and continues the workflow without changing applications.

## Shared Terminal Block

A user wants another person to see the result of a command, a log, a rendered report, or a workflow status. Web Terminal shares only the selected block or session area, not the whole machine.

This supports collaboration without making full terminal sharing the default.

## Cost-Saving Routine

A user repeatedly asks for the same task through the agent. Web Terminal notices the repeated structure, compiles the behavior, and later shows that the task used zero LLM tokens.

The product makes savings visible: avoided calls, time saved, and successful replays.
