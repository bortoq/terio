# Demo

## Scenario: Repeated Media Workflow Becomes Cheap

A user has a folder with a FLAC file and a CUE file. The user wants the album split into tracks, tagged, and renamed according to the same pattern used in previous sessions.

## First Manual Workflow

The user asks:

`split this flac/cue album into tracks and name them like last time`

terio:

- identifies the FLAC and CUE files in the current directory;
- asks for confirmation if multiple candidates exist;
- runs the required local commands;
- shows progress as a rendered card;
- displays a final table with track number, title, duration, filename, and status;
- records the request, arguments, commands, result, and errors in the Behavior Log.

## Pattern Recognition

After the workflow succeeds enough times with similar structure, Behavior Compiler detects a stable behavior:

`split album image -> tag tracks -> rename files -> verify output`

The variable arguments are:

- source audio file;
- cue sheet;
- output directory;
- naming template.

## Replay

Next time the user asks the same kind of request, terio does not call the LLM if confidence is high enough.

It extracts the current arguments, validates that files exist, runs the compiled recipe, and shows the rendered result. If validation fails or the script exits with an error, terio downgrades confidence and falls back to the agent.

## Web Rendering

The output is not a raw wall of terminal text. The user sees:

- a progress block while files are being generated;
- a track table after completion;
- warning cards for missing tags or suspicious filenames;
- quick actions for opening the folder, playing the album, or rerunning with a different template.

## Modal Continuation

If the user opens the CUE sheet or renaming template, terio switches to edit mode in the same workspace. After saving, the user returns to command mode and reruns the workflow.

## Demo Goal

The first demo should prove one behavior: a normal terminal workflow can become a trusted compiled behavior that runs faster, costs less, and displays results more clearly than a plain terminal session.
