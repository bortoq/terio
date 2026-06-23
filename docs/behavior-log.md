# Behavior Log Schema

## Формат

JSONL. schema_version: 1.

Machine-readable schema: [docs/schemas/behavior-log.schema.json](../docs/schemas/behavior-log.schema.json)

## Типы записей

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
  "failure_kind": "validation_failed|model_error|timeout|cancelled",
  "prompt_summary": "files: album.flac, album.cue (redacted)",
  "plan": [
    {"command": "mkdir", "argv": ["-p", "./tracks"], "risk": "local_write"}
  ],
  "model_provider": "openai",
  "model_name": "gpt-4o",
  "duration_ms": 3400,
  "tokens_used": 450,
  "attention_cost": {
    "user_sec": 8.0,
    "agent_sec": 3.4,
    "system_sec": 0.05
  }
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
  "parent_run_id": "uuid_agent",
  "cwd": "/home/user/music",
  "risk": "local_write",
  "status": "success|failed",
  "failure_kind": "non_zero_exit|timeout|signal|risk_blocked",
  "command": {
    "display": "mkdir -p ./tracks",
    "argv": ["mkdir", "-p", "./tracks"]
  },
  "exit": 0,
  "duration_ms": 5,
  "stdout_summary": null,
  "stderr_summary": null,
  "attention_cost": {
    "user_sec": 0.0,
    "agent_sec": 0.0,
    "system_sec": 0.005
  }
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
  "failure_kind": "precondition_failed|command_exit|timeout|risk_blocked",
  "script_id": "sha256-...",
  "cache_hit": true,
  "model_called": false,
  "tokens_saved_estimate": 450,
  "success_count_before": 2,
  "success_count_after": 3,
  "steps": [
    {"command": "mkdir", "argv": ["-p", "./tracks"], "exit": 0},
    {"command": "ffmpeg", "argv": ["-i", "other.flac", "..."], "exit": 0}
  ],
  "duration_ms": 12500,
  "attention_cost": {
    "user_sec": 1.0,
    "agent_sec": 0.0,
    "system_sec": 0.01
  }
}
```

## Стоимость внимания (attention_cost)

Каждая запись содержит раздельные счётчики затрат внимания:

- `user_sec` — секунды внимания пользователя (ввод запроса, чтение вывода, подтверждение).
- `agent_sec` — секунды внимания агента (LLM — время генерации).
- `system_sec` — секунды внимания ОС/программ (время выполнения команд, ввод-вывод).

Стоимость внимания используется для:
- `terio stats` — агрегация по типам ресурсов.
- Минимизация общей стоимости эксплуатации (см. Экономическая модель в architecture.md).
- Оценка эффективности кеширования: сколько внимания пользователя и агента сэкономил cache hit.

## Метрики

Каждая запись обновляет счётчики. Агрегация — по запросу `terio stats`:

| Метрика | Источник |
|---------|----------|
| model_calls | agent_turn (каждый) |
| cache_hits | script_run (каждый) |
| tokens_consumed | agent_turn.tokens_used |
| tokens_saved | script_run.tokens_saved_estimate |
| commands_executed | command_run (каждый) |
| failures | status=failed |
| total_duration_ms | sum of duration_ms |
| attention_cost_user_sec | sum of attention_cost.user_sec |
| attention_cost_agent_sec | sum of attention_cost.agent_sec |
| attention_cost_system_sec | sum of attention_cost.system_sec |

## Хранение

- `~/.terio/log/terio-YYYY-MM.jsonl`
- Ротация: 50MB или месяц.
- Старые: `terio-2026-05.jsonl.gz`
- Raw output: `~/.terio/runs/<run_id>/stdout.log`

## Правила

1. Secrets редэктятся из всех полей перед записью.
2. Для `credential_access` — stdout/stderr не пишутся.
3. prompt_summary — не более 512 символов, redacted.
4. stdout_summary/stderr_summary — не более 1024 символов.
5. Полный prompt не логируется (только summary).
6. attention_cost.system_sec вычисляется как duration_ms команд в секундах.
7. attention_cost.user_sec вычисляется как длительность ввода/чтения (измеряется terio).
8. attention_cost.agent_sec равен duration_ms вызова модели.
