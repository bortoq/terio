# Architecture

## Core Idea

terio — это агентный терминал с ленивым кешированием поведения.

Упрощённо:

```
Пользователь → [Запрос] → Поиск в кеше скриптов → Найден? → Выполнить скрипт
                                                      ↓
                                                   Не найден?
                                                      ↓
                                              AI-модель строит план
                                                      ↓
                                              Показать план → Подтверждение
                                                      ↓
                                              Выполнить команды → Показать результат
                                                      ↓
                                              Сохранить цепочку как скрипт
```

## Компоненты

```
┌──────────────────────────────────────────────┐
│              COMMAND SURFACE                  │
│   terio ask "..." / terio run -- <command>    │
├──────────────────────────────────────────────┤
│              REQUEST MATCHER                  │
│   Сопоставляет запрос с кешем скриптов       │
│   (нормализация, fuzzy match, exact match)   │
├──────────┬───────────────────┬───────────────┤
│  AGENT   │  EXECUTION LAYER  │  RENDERER     │
│  (LLM)   │  (shell, process) │  (web blocks) │
├──────────┴───────────────────┴───────────────┤
│           SCRIPT CACHE (поведения)            │
│   request_hash → { script, risk, metadata }  │
├──────────────────────────────────────────────┤
│           BEHAVIOR LOG (JSONL)                │
│   schema_version, run_id, kind, request, ...  │
├──────────────────────────────────────────────┤
│           STORAGE LAYER                       │
│   preferences, credentials (encrypted),       │
│   scripts, log, trash (experimental undo)     │
└──────────────────────────────────────────────┘
```

### 1. Command Surface
- Принимает: естественно-языковые запросы (`terio ask "..."`) и shell-команды (`terio run -- <command>`).
- **Инвариант:** каждый запрос проходит через Request Matcher.

### 2. Request Matcher
- Нормализует запрос (lowercase, стоп-слова, токенизация).
- Ищет совпадение в Script Cache: exact match → fuzzy match → ничего.
- Если совпадение найдено и script validated → исполняет скрипт без модели.
- **Ключевое:** это единственное место, где terio решает, вызывать модель или нет.

### 3. Agent Layer (Built-in AI)
- Вызывается только когда Request Matcher не нашёл скрипт.
- Получает: запрос пользователя, CWD, доступные файлы, контекст.
- Возвращает: план выполнения (chain of shell-команд с обоснованием).
- Провайдер: конфигурируемый — локальный (llama.cpp, ollama) или удалённый (OpenAI, Anthropic).
- **Правило:** агент только предлагает план. Пользователь подтверждает. terio исполняет.
- **Правило:** агент не имеет доступа к credentials (редэкция до отправки в модель).

### 4. Execution Layer
- Принимает: argv-строку или structured command.
- Запускает процесс, стримит stdout/stderr.
- Возвращает: exit code, stdout, stderr, duration.
- **Отмена:** по Ctrl+C, таймауту или `terio cancel`.
- **Безопасность:** перед выполнением проверяется risk level. Destructive/network_write → обязательное подтверждение.

### 5. Renderer
- Принимает: `ExecutionResult { stdout, stderr, exit_code, duration, command }`.
- Определяет тип вывода: таблица, карточка, таймлайн, plain text.
- Показывает результат пользователю.

### 6. Script Cache
- Хранит: `{ request_hash, normalized_request, script (shell chain), risk, args_template, success_count, created_at, last_used_at }`.
- Источник: успешные выполнения через Agent Layer.
- Ключ: нормализованный запрос.
- **Формат скрипта:** простой shell-скрипт с комментариями, сгенерированный агентом.
- **Правило:** скрипт не выполняется, если не прошёл валидацию (аргументы существуют, команды доступны).

### 7. Behavior Log
- JSONL, schema v1.
- Три вида записей: `agent_turn` (запрос к модели), `command_run` (выполнение команды), `script_run` (выполнение скрипта из кеша).
- Секреты редэктятся из всех полей.

### 8. Trust Engine
- Risk levels: read_only, local_write, destructive, network_read, network_write, credential_access, financial.
- Политики: `always_ask`, `ask_once`, `allow`.
- Для скриптов из кеша: если risk <= local_write и скрипт выполнялся успешно 3+ раза → авто-запуск.

### 9. Undo/Redo (Experimental)
- **Не гарантируется** для произвольных shell-команд.
- Best-effort для скриптов из кеша: snapshot затронутых файлов до выполнения.
- Два режима (выбирается в конфиге):
  - **Sandbox:** тяжело, но безопасно. Исполнение в изолированном окружении (overlay FS, bubblewrap).
  - **Warn:** быстро, но рискованно. Только предупреждение перед destructive-действиями.
- По умолчанию: выключен. Включается пользователем явно.

## Risk Taxonomy (MVP)

| Risk Level | Примеры | Default Policy |
|------------|---------|----------------|
| `read_only` | `ls`, `cat`, `git status` | Auto |
| `local_write` | `mkdir`, `cp`, `ffmpeg` | Confirm (agent) / auto (cached script >=3 success) |
| `destructive` | `rm`, `mv --overwrite` | Always confirm |
| `network_read` | `curl`, `git fetch` | Confirm (agent) / auto (cached) |
| `network_write` | `git push`, `curl -X POST` | Always confirm |
| `credential_access` | токены, ключи | Always confirm, не логировать |
| `financial` | покупки, API billing | Always confirm |

## Data Flow (MVP)

1. `terio ask "split this flac/cue album"` → Command Surface → Request Matcher.
2. Matcher ищет в Script Cache. Если найден → шаг 7.
3. Не найден → Agent Layer (LLM).
4. Агент возвращает план: `["ffmpeg -i album.flac ...", "mkdir -p tracks"]`.
5. План показывается пользователю. Подтверждение.
6. Execution Layer выполняет. Renderer показывает результат. Log пишет.
7. Цепочка сохраняется в Script Cache как скрипт для этого запроса.
8. В следующий раз: `terio ask "split this flac/cue album"` → Matcher находит → скрипт выполняется без модели.

## Формат скрипта в кеше (MVP)

```json
{
  "request_hash": "sha256(normalized_request)",
  "normalized": "split flac cue album",
  "risk": "local_write",
  "success_count": 0,
  "max_success_count": 3,
  "script": [
    {"command": "mkdir", "argv": ["-p", "./tracks"], "risk": "local_write"},
    {"command": "ffmpeg", "argv": ["-i", "album.flac", "-i", "album.cue", "-map", "0:0", "-c", "copy", "-f", "segment", "./tracks/track_%02d.flac"], "risk": "local_write"}
  ],
  "created_at": "2026-06-23T12:00:00Z",
  "last_used_at": "2026-06-23T12:00:00Z"
}
```

## Stack

- **Язык:** Rust.
- **CLI:** clap.
- **Shell:** duct / std::process::Command.
- **Renderer:** HTML (webview или браузер).
- **Log:** serde_json + JSONL.
- **Script Cache:** SQLite или JSON-файл.
- **Agent:** HTTP client к LLM (openai, anthropic, ollama).
- **Sandbox (exp):** bubblewrap / nsjail.

## Key Design Rule

Пользователь работает. terio запоминает. Модель вызывается только когда нужно впервые.
