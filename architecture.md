# Architecture

## Core Idea

terio — агрегатор интерфейсов. Любая программа, с которой можно обменяться действием и результатом, должна быть управляема из terio.

В MVP это работает через CLI:

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
                                              Сохранить как скрипт (structured chain)
```

## Компоненты

```
┌──────────────────────────────────────────────────┐
│              COMMAND SURFACE                      │
│   terio ask "..." / terio run -- <command>        │
├──────────────────────────────────────────────────┤
│              REQUEST MATCHER                      │
│   exact normalized match → script; иначе → agent  │
├──────────┬───────────────────┬───────────────────┤
│  AGENT   │  EXECUTION LAYER  │   RENDERER        │
│  (LLM)   │  (shell, process) │  (web blocks)     │
├──────────┴───────────────────┴───────────────────┤
│           SCRIPT CACHE                            │
│   normalized_request → { steps, risk, metadata }  │
│   (structured command chains, НЕ shell-скрипты)   │
├──────────────────────────────────────────────────┤
│           BEHAVIOR LOG + METRICS                  │
│   JSONL (agent_turn, command_run, script_run)     │
│   + счётчики: model_calls, cache_hits, errors    │
├──────────────────────────────────────────────────┤
│           STORAGE LAYER                           │
│   config, credentials (env/keychain), cache, log, │
│   metrics, trash (experimental undo)              │
└──────────────────────────────────────────────────┘
```

### 1. Command Surface
- Принимает: естественно-языковые запросы (`terio ask "..."`) и shell-команды (`terio run -- <command>`).
- **Инвариант:** каждый запрос проходит через Request Matcher.

### 2. Request Matcher
- Нормализует запрос (lowercase, стоп-слова, приведение).
- **MVP: только exact normalized match.**
- Fuzzy match: в будущем, только с подтверждением пользователя, никогда auto-run для local_write/destructive.
- **Ключевое:** единственное место, где terio решает, вызывать модель или нет.

### 3. Agent Layer (Built-in AI)
- Вызывается только когда Request Matcher не нашёл скрипт.
- Получает: запрос пользователя, CWD, список файлов (без содержимого).
- Возвращает: structured plan (см. docs/agent-protocol.md).
- Провайдер: локальный (ollama, llama.cpp) или удалённый (OpenAI, Anthropic).
- **Правило:** агент только планирует. Исполняет terio.
- **Правило:** агент не получает credentials (редэкция до отправки).
- **Правило:** risk от модели — рекомендательный. terio вычисляет финальный risk локально и использует более строгий.

### 4. Execution Layer
- Принимает structured command (command + argv).
- Запускает процесс, стримит stdout/stderr.
- Возвращает: exit code, stdout, stderr, duration.
- **Отмена:** Ctrl+C, таймаут, `terio cancel`.
- **Безопасность:** перед выполнением проверяется risk. Destructive/network_write → обязательное подтверждение.

### 5. Renderer
- Принимает: `ExecutionResult { stdout, stderr, exit_code, duration, command }`.
- Авто-определяет тип вывода: таблица, таймлайн, карточка, plain text.
- Показывает результат.

### 6. Script Cache
- Хранит **structured command chains** (не shell-скрипты).
- Ключ: нормализованный запрос.
- Формат (v1):
  ```json
  {
    "schema_version": 1,
    "script_id": "sha256-...",
    "normalized_request": "split flac cue album",
    "match_policy": "exact_normalized",
    "risk": "local_write",
    "parameters": {
      "flac_file": {"source": "glob_one", "pattern": "*.flac", "required": true},
      "cue_file": {"source": "glob_one", "pattern": "*.cue", "required": true},
      "output_dir": {"source": "default", "value": "./tracks"}
    },
    "preconditions": [
      {"binary_exists": "ffmpeg"},
      {"glob_one": "*.flac"},
      {"glob_one": "*.cue"}
    ],
    "steps": [
      {"command": "mkdir", "argv": ["-p", "${output_dir}"], "risk": "local_write"},
      {"command": "ffmpeg", "argv": ["-i", "${flac_file}", "-i", "${cue_file}", "-map", "0:0", "-c", "copy", "-f", "segment", "${output_dir}/track_%02d.flac"], "risk": "local_write"}
    ],
    "artifacts": [{"path_glob": "./${output_dir}/*.flac", "kind": "created_file"}],
    "success_count": 0,
    "trust_threshold": 3,
    "created_at": "2026-06-23T12:00:00Z",
    "last_used_at": "2026-06-23T12:00:00Z"
  }
  ```
- **Правило:** скрипт не выполняется, если не прошёл preconditions.
- **Правило:** после `trust_threshold` успехов и exact match → может auto-run (если риск <= local_write).

### 7. Behavior Log + Metrics
- JSONL, schema v1.
- Три вида записей: `agent_turn`, `command_run`, `script_run`.
- Секреты редэктятся из всех полей.
- **Метрики (счётчики, не агрегаты):**
  - Каждый `agent_turn` → model_called += 1, tokens_consumed += N.
  - Каждый `script_run` (cache hit) → cache_hit += 1, tokens_saved += estimate.
  - Каждый `command_run` → commands_executed += 1.
  - Каждая ошибка → failure_count += 1, failure_kind записывается.
- Агрегация: по запросу (`terio stats`), из лога.

### 8. Trust Engine
- Risk levels: read_only, local_write, destructive, network_read, network_write, credential_access, financial.
- Политики: `always_ask`, `ask_once`, `allow`.
- **Auto-run** (MVP): только exact normalized match + risk <= local_write + success_count >= trust_threshold + все parameters resolved однозначно + preconditions пройдены + все output внутри CWD или разрешённой директории + нет destructive/network_write шагов + пользователь не отключал auto-run + предыдущий запуск был успешен в эквивалентном контексте.
- **Fuzzy match:** никогда не auto-run в MVP. Только предложить и спросить.
- **Model risk:** рекомендательный. terio вычисляет финальный risk по команде.

### 9. Undo/Redo (Experimental)
- **Не гарантируется.** Best-effort для кешированных скриптов.
- Два режима (в конфиге): sandbox (bubblewrap) или warn-only.
- По умолчанию: выключен.

## Risk Taxonomy (MVP)

| Risk Level | Примеры | Default Policy |
|------------|---------|----------------|
| `read_only` | `ls`, `cat`, `git status` | Auto |
| `local_write` | `mkdir`, `cp`, `ffmpeg` | Confirm / auto (exact cache match, >=3 success) |
| `destructive` | `rm`, `mv --overwrite` | Always confirm |
| `network_read` | `curl`, `git fetch` | Confirm / auto (exact cache match) |
| `network_write` | `git push`, `curl -X POST` | Always confirm |
| `credential_access` | токены, ключи | Always confirm, не логировать |
| `financial` | покупки, API billing | Always confirm |

## Data Flow (MVP)

1. `terio ask "split this flac/cue album"` → Command Surface → Request Matcher.
2. Matcher ищет exact normalized match в Script Cache. Найден и trust >= threshold → шаг 7.
3. Не найден или trust < threshold → Agent Layer (LLM).
4. Агент возвращает structured plan. terio проверяет risk и показывает план.
5. Пользователь подтверждает. Execution Layer выполняет.
6. Renderer показывает результат. Log пишет. Metrics обновляются.
7. Цепочка сохраняется в Script Cache (с параметрами и preconditions).
8. В следующий раз: exact match → скрипт выполняется без модели.

## Agent Protocol (MVP)

См. [docs/agent-protocol.md](docs/agent-protocol.md) — полный контракт.

Кратко:
- Вход: `{ request, cwd, files[], allowed_risks[] }` (secrets redacted).
- Выход: `{ summary, risk, commands: [{ command, argv, risk, reason }] }`.
- terio проверяет: JSON валиден, команды в allow list, risk не занижен.
- Финальный risk = max(model.risk, terio.computed_risk).

## Логирование

- Логируется **каждый ввод пользователя**: `terio ask "..."` и `terio run -- ...`.
- Логируется **каждый шаг выполнения**: команда, exit code, duration, stdout summary.
- Логируется **каждый вызов модели**: провайдер, токены, статус.
- **Секреты не логируются** — редэкция до записи.
- Всё хранится в JSONL. Агрегация — по запросу.

## Stack

- **Язык:** Rust.
- **CLI:** clap.
- **Shell:** duct / std::process::Command.
- **Renderer:** HTML (webview или браузер).
- **Log:** serde_json + JSONL.
- **Script Cache:** SQLite или JSON.
- **Agent:** HTTP client к LLM (openai, anthropic, ollama).

## Key Design Rule

Пользователь работает. terio запоминает. Модель вызывается только когда нужно впервые. Любая программа с отсоединяемым интерфейсом должна быть управляема из terio.
