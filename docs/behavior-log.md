# Behavior Log Schema

## Формат

JSONL (JSON Lines) — одна запись на строку.

## Версия

`schema_version: 1`

## Типы записей (kind)

Три вида записей, у каждого свои поля.

### 1. `agent_turn` — запрос к AI-модели

```json
{
  "schema_version": 1,
  "run_id": "uuid",
  "session_id": "uuid",
  "ts": "2026-06-23T10:00:00Z",
  "kind": "agent_turn",
  "request": "split this flac/cue",
  "cwd": "/home/user/music",
  "risk": "local_write",
  "status": "success|failed|cancelled",
  "prompt_summary": "files in CWD: album.flac, album.cue",
  "plan": [
    {"command": "mkdir", "argv": ["-p", "./tracks"], "risk": "local_write"},
    {"command": "ffmpeg", "argv": ["-i", "album.flac", ...], "risk": "local_write"}
  ],
  "model_provider": "openai",
  "model_name": "gpt-4o",
  "duration_ms": 3400,
  "tokens_used": 450
}
```

### 2. `command_run` — выполнение shell-команды

```json
{
  "schema_version": 1,
  "run_id": "uuid",
  "session_id": "uuid",
  "ts": "2026-06-23T10:00:05Z",
  "kind": "command_run",
  "request": "split this flac/cue",
  "parent_run_id": "uuid_agent_turn",
  "cwd": "/home/user/music",
  "risk": "local_write",
  "status": "success|failed",
  "command": {
    "display": "mkdir -p ./tracks",
    "argv": ["mkdir", "-p", "./tracks"]
  },
  "exit": 0,
  "duration_ms": 5,
  "stdout_summary": null,
  "stderr_summary": null
}
```

### 3. `script_run` — выполнение скрипта из кеша (без модели)

```json
{
  "schema_version": 1,
  "run_id": "uuid",
  "session_id": "uuid",
  "ts": "2026-06-23T12:00:00Z",
  "kind": "script_run",
  "request": "split this flac/cue",
  "cwd": "/home/user/music/other_album",
  "risk": "local_write",
  "status": "success|failed",
  "script_id": "sha256_of_normalized_request",
  "script_success_count": 4,
  "steps": [
    {"command": "mkdir", "argv": ["-p", "./tracks"], "exit": 0},
    {"command": "ffmpeg", "argv": ["-i", "other.flac", ...], "exit": 0}
  ],
  "duration_ms": 12500
}
```

## Пример лога

```jsonl
{"schema_version":1,"run_id":"R1","session_id":"S1","ts":"2026-06-23T10:00:00Z","kind":"agent_turn","request":"list files","cwd":"/home/user","risk":"read_only","status":"success","plan":[{"command":"ls","argv":["ls","-l"],"risk":"read_only"}],"model_provider":"openai","duration_ms":800,"tokens_used":120}
{"schema_version":1,"run_id":"R2","session_id":"S1","ts":"2026-06-23T10:00:05Z","kind":"command_run","request":"list files","parent_run_id":"R1","cwd":"/home/user","risk":"read_only","status":"success","command":{"display":"ls -l","argv":["ls","-l"]},"exit":0,"duration_ms":8}
{"schema_version":1,"run_id":"R3","session_id":"S1","ts":"2026-06-23T12:00:00Z","kind":"script_run","request":"list files","cwd":"/home/user/other","risk":"read_only","status":"success","script_id":"abc123","script_success_count":1,"steps":[{"command":"ls","argv":["ls","-l"],"exit":0}],"duration_ms":6}
```

## Хранение

- Директория: `~/.terio/log/`
- Файл: `terio-YYYY-MM.jsonl`
- Ротация: по 50MB или по месяцу.
- Старые: `terio-2026-05.jsonl.gz`

## Правила

1. Secrets редэктятся из всех полей перед записью.
2. Для `credential_access` — stdout/stderr не пишутся.
3. prompt_summary — не более 512 символов.
4. stdout_summary/stderr_summary — не более 1024 символов.
