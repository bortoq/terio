# Behavior Log Schema

## Формат

JSONL. schema_version: 1.

**Дизайн для смены формата:** весь код работает через `LogWriter`/`LogReader` traits. Конкретная реализация (`JsonlLogWriter`/`JsonlLogReader`) может быть заменена без изменения остальных компонентов.

Machine-readable schema: [docs/schemas/behavior-log.schema.json](../docs/schemas/behavior-log.schema.json)

## Общая структура записи

Все записи лога имеют общие поля:

```json
{
  "schema_version": 1,
  "instance_id": "01JAN3XINSTANCE001",
  "session_id": "uuid",
  "interaction_id": "uuid",
  "ts": "2026-06-23T10:00:00Z",
  "kind": "agent_turn|command_run|script_run|system_event",
  "display_profile": {
    "type": "auto",
    "renderer_hint": "auto",
    "user_visible": true,
    "summary_max_lines": 10
  },
  "cost_counters": {
    "observation_cost_hint": { "user_sec": 0.0 },
    "llm_cost": { "tokens": 0, "duration_ms": 0 },
    "execution_cost": { "duration_ms": 0, "commands_executed": 0, "bytes_read": 0, "bytes_written": 0 },
    "cache_cost": { "lookup_ms": 0, "hit": false },
    "storage_cost": { "bytes_written": 0, "bytes_read": 0 }
  },
  // ... поля специфичные для kind
}
```

### Поля

| Поле | Тип | Описание |
|------|-----|----------|
| `schema_version` | int | 1 |
| `instance_id` | string | ULID экземпляра terio (не меняется) |
| `session_id` | string | UUID сессии (один запуск) |
| `interaction_id` | string | UUID одного запроса пользователя. Группирует пары (запрос → ответ) |
| `ts` | ISO8601 | Время события |
| `kind` | string | Тип записи |
| `display_profile` | object | Как показывать пользователю |
| `cost_counters` | object | Сырые счётчики расходов |

### display_profile

Управляет отображением записи в UI. Пользователь настраивает через `terio config set display.<kind>.<field>`.

| Поле | Тип | Описание |
|------|-----|----------|
| `type` | string | `auto` (определяется terio), `text`, `table`, `media`, `hidden`, `summary` |
| `renderer_hint` | string | `auto`, `table`, `plain`, `timeline`, `card` |
| `user_visible` | bool | Показывать пользователю или скрыть |
| `summary_max_lines` | int | Для `type: summary` — сколько строк показывать |

**Правила:**
- `type: hidden` — запись не показывается в UI (например, `credential_access`).
- `type: summary` — показываются первые `summary_max_lines` строк.
- `type: media` — запись содержит ссылку на внешний файл (видео, аудио, изображение).
- `user_visible: false` — скрыть, но не удалять из лога.
- Если `display_profile` не передан, terio использует тип по умолчанию для данного `kind`.

### cost_counters

Сырые счётчики расходов для будущей метрики `total_attention_cost`. В MVP — только накопление. Формулы — в будущем.

| Поле | Тип | Описание |
|------|-----|----------|
| `observation_cost_hint.user_sec` | f64 | Секунд внимания пользователя (MVP: заглушка, 0.0) |
| `llm_cost.tokens` | int | Потраченные токены |
| `llm_cost.duration_ms` | int | Время вызова модели (мс) |
| `execution_cost.duration_ms` | int | Время выполнения команд (мс) |
| `execution_cost.commands_executed` | int | Количество выполненных команд |
| `execution_cost.bytes_read` | int | Байт прочитано из stdout/stderr |
| `execution_cost.bytes_written` | int | Байт записано (артефакты) |
| `cache_cost.lookup_ms` | int | Время поиска в кеше (мс) |
| `cache_cost.hit` | bool | Был ли cache hit |
| `storage_cost.bytes_written` | int | Байт записано в лог |
| `storage_cost.bytes_read` | int | Байт прочитано из лога |

## Типы записей

### 1. `agent_turn` — запрос к AI-модели

```json
{
  "schema_version": 1,
  "instance_id": "01JAN3XINSTANCE001",
  "session_id": "uuid",
  "interaction_id": "uuid",
  "ts": "2026-06-23T10:00:00Z",
  "kind": "agent_turn",
  "display_profile": {
    "type": "auto",
    "renderer_hint": "auto",
    "user_visible": true
  },
  "cost_counters": {
    "observation_cost_hint": { "user_sec": 5.0 },
    "llm_cost": { "tokens": 450, "duration_ms": 3400 },
    "execution_cost": { "duration_ms": 0, "commands_executed": 0, "bytes_read": 0, "bytes_written": 0 },
    "cache_cost": { "lookup_ms": 0, "hit": false },
    "storage_cost": { "bytes_written": 320, "bytes_read": 0 }
  },
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
  "tokens_used": 450
}
```

### 2. `command_run` — выполнение shell-команды

```json
{
  "schema_version": 1,
  "instance_id": "01JAN3XINSTANCE001",
  "session_id": "uuid",
  "interaction_id": "uuid",
  "ts": "2026-06-23T10:00:05Z",
  "kind": "command_run",
  "display_profile": {
    "type": "auto",
    "renderer_hint": "auto",
    "user_visible": true
  },
  "cost_counters": {
    "observation_cost_hint": { "user_sec": 0.0 },
    "llm_cost": { "tokens": 0, "duration_ms": 0 },
    "execution_cost": { "duration_ms": 5, "commands_executed": 1, "bytes_read": 120, "bytes_written": 0 },
    "cache_cost": { "lookup_ms": 0, "hit": false },
    "storage_cost": { "bytes_written": 180, "bytes_read": 0 }
  },
  "request": "split this flac/cue",
  "parent_interaction_id": "uuid_agent",
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
  "stderr_summary": null
}
```

### 3. `script_run` — выполнение скрипта из кеша (без модели)

```json
{
  "schema_version": 1,
  "instance_id": "01JAN3XINSTANCE001",
  "session_id": "uuid",
  "interaction_id": "uuid",
  "ts": "2026-06-23T12:00:00Z",
  "kind": "script_run",
  "display_profile": {
    "type": "auto",
    "renderer_hint": "auto",
    "user_visible": true
  },
  "cost_counters": {
    "observation_cost_hint": { "user_sec": 1.0 },
    "llm_cost": { "tokens": 0, "duration_ms": 0 },
    "execution_cost": { "duration_ms": 8900, "commands_executed": 2, "bytes_read": 2048, "bytes_written": 52428800 },
    "cache_cost": { "lookup_ms": 2, "hit": true },
    "storage_cost": { "bytes_written": 450, "bytes_read": 0 }
  },
  "request": "split this flac/cue album",
  "cwd": "/home/user/music/other",
  "risk": "local_write",
  "status": "success|failed",
  "failure_kind": "precondition_failed|command_exit|timeout|risk_blocked",
  "script_id": "sha256-...",
  "cache_hit": true,
  "model_called": false,
  "tokens_saved_estimate": 320,
  "success_count_before": 1,
  "success_count_after": 2,
  "steps": [
    {"command": "mkdir", "argv": ["-p", "./tracks"], "exit": 0},
    {"command": "ffmpeg", "argv": ["-i", "other.flac", "..."], "exit": 0}
  ],
  "duration_ms": 8900
}
```

### 4. `system_event` — события системы (резервируется)

Для будущих событий: старт/стоп сессии, ошибки конфигурации, обновления. В MVP не используется.

## LogWriter / LogReader traits

### LogWriter (запись)

```rust
pub trait LogWriter: Send + Sync {
    fn append(&self, entry: LogEntry) -> Result<()>;
    fn flush(&self) -> Result<()>;
}
```

- `JsonlLogWriter` — реализация для MVP.
- Перед записью на диск отправляет запись в in-memory `LogEventStream`.
- Только append. Никаких удалений.

### LogReader (чтение)

```rust
pub trait LogReader: Send + Sync {
    fn recent(&self, n: usize) -> Result<Vec<LogEntry>>;
    fn by_session(&self, session_id: &str) -> Result<Vec<LogEntry>>;
    fn by_interaction(&self, interaction_id: &str) -> Result<Vec<LogEntry>>;
    fn stream(&self) -> Receiver<LogEntry>;
}
```

- `JsonlLogReader` — реализация для MVP.
- `stream()` — возвращает `broadcast::Receiver<LogEntry>` (in-memory, без I/O).
- `recent(n)` — читает с конца файла (seek + streaming), не загружая весь файл.
- `by_interaction` — группирует записи по `interaction_id` (для показа пар).

### Data flow

```
Execution Layer / Agent / Cache
         │
         ▼
    LogWriter::append(entry)
         │
         ├──▶ JsonlLogWriter пишет в файл
         │
         └──▶ LogEventStream (broadcast)
                    │
                    ▼
              Renderer подписан на stream()
              (получает записи без I/O)
```

## interaction_id — группировка пар

Каждый пользовательский запрос получает `interaction_id` (UUID) в Command Surface.

- Один запрос пользователя = один `interaction_id`.
- Все `agent_turn`, `command_run`, `script_run`, `system_event` для этого запроса получают этот `interaction_id`.
- `parent_interaction_id` (опционально) — если один запрос порождает другой (например, после ошибки вызывается модель для исправления).

Renderer группирует записи по `interaction_id` и показывает как одну пару (запрос → результат), даже если записи не идут подряд в логе.

## Метрики

| Метрика | Источник |
|---------|----------|
| model_calls | agent_turn (каждый) |
| cache_hits | script_run (каждый) |
| tokens_consumed | agent_turn.cost_counters.llm_cost.tokens |
| tokens_saved | script_run.tokens_saved_estimate |
| commands_executed | sum of cost_counters.execution_cost.commands_executed |
| failures | status=failed |
| total_duration_ms | sum of cost_counters.execution_cost.duration_ms |
| total_cost_user_sec | sum of cost_counters.observation_cost_hint.user_sec |
| total_cost_llm_tokens | sum of cost_counters.llm_cost.tokens |

## Хранение

- `~/.terio/log/terio-YYYY-MM.jsonl`
- Ротация: 50MB или месяц.
- Старые: `terio-2026-05.jsonl.gz`
- Raw assets: `~/.terio/runs/<interaction_id>/assets/` (для `type: media`)
- Instance ID: `~/.terio/instance.json`

## Правила

1. Secrets редэктятся из всех полей перед записью.
2. Для `credential_access` — `display_profile.user_visible = false`.
3. prompt_summary — не более 512 символов, redacted.
4. stdout_summary/stderr_summary — не более 1024 символов.
5. Полный prompt не логируется (только summary).
6. `cost_counters.observation_cost_hint.user_sec` — заглушка (0.0), реальное измерение — в будущем.
7. `interaction_id` обязателен для всех записей, кроме `system_event`.
