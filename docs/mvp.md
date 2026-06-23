# MVP — Minimum Viable Product

## Принцип

terio доказывает: пользователь вводит задачу на естественном языке, AI-модель строит цепочку команд, terio выполняет и запоминает. Тот же запрос в следующий раз — без модели.

## Что входит в MVP

### 1. `terio ask "..."` — естественно-языковой запрос
- Принимает запрос, ищет **exact normalized match** в Script Cache.
- Нашёл и trust >= threshold → выполняет скрипт без модели.
- Не нашёл → отправляет модели.

### 2. Agent (AI-модель)
- Получает: запрос, CWD, список файлов (без содержимого).
- Возвращает: structured plan (см. agent-protocol.md).
- Провайдер: ollama, OpenAI, Anthropic — конфигурируется.
- **Правило:** модель не исполняет. Только планирует.
- **Правило:** риск от модели — рекомендательный. terio вычисляет финальный.

### 3. Plan → Confirm → Execute
- Агент возвращает план. terio показывает: команды, risk level.
- Пользователь подтверждает. terio выполняет.
- Результат рендерится.

### 4. Script Cache
- Success → сохранение structured command chain.
- Параметры: glob-паттерны для файлов (заполняются из CWD при реплее).
- Preconditions: binary_exists, glob_one, file_exists.
- Match: только exact normalized.
- Auto-run: после trust_threshold успехов, только exact match, risk <= local_write.

### 5. Execution Layer
- `terio run -- <command>` — прямая shell-команда.
- Захват stdout, stderr, exit code, duration.
- `terio rerun`.

### 6. Renderer
- Plain text (fallback).
- Table (ls, csv-подобный вывод).
- Авто-определение.

### 7. Behavior Log + Metrics
- JSONL, schema v1: agent_turn, command_run, script_run.
- **Логируется каждый ввод пользователя.**
- **Считаются все метрики:** model_calls, cache_hits, tokens, errors, duration.
- Секреты редэктятся из всех полей.
- Агрегация — по запросу (`terio stats`).

### 8. Trust (минимальный)
- Risk: read_only, local_write, destructive, network_read, network_write.
- Destructive/network_write → всегда подтверждение.
- Exact match только. Fuzzy match не в MVP.
- Auto-run: exact match + risk <= local_write + success_count >= 3.

### 9. Undo/Redo (Experimental, off)
- Не гарантируется. Best-effort.
- Sandbox или warn (конфиг).
- Off by default.

## Что НЕ входит в MVP

- ❌ Fuzzy match.
- ❌ YAML recipes.
- ❌ Полноценный undo/redo.
- ❌ Desktop.
- ❌ Маркетплейс.
- ❌ Шэринг.
- ❌ `terio recipe` — только `ask` и `run`.

## CLI контракт (MVP)

```bash
terio ask "<request>"         # запрос на естественном языке
terio run -- <command...>     # shell-команда
terio rerun                   # повтор последней
terio log                     # история
terio log --json              # история в JSON
terio stats                   # метрики
terio cancel                  # отмена
terio config                  # настройки
```

## Критерии успеха

1. `terio ask "list files"` → таблица.
2. `terio ask "list files"` (повторно) → без модели, быстрее.
3. `terio run -- echo hello` → hello.
4. `terio log` показывает историю.
5. `terio stats` показывает cache_hits > 0, model_calls > 0.
6. Destructive-запросы требуют подтверждения.
7. Secrets не в логе.

## Структура репозитория

```
terio/
  README.md
  LICENSE
  Cargo.toml
  src/
    main.rs
    cli.rs
    ask.rs       # agent → plan → execute → cache
    run.rs       # shell execution
    matcher.rs   # request matcher (exact normalized)
    cache.rs     # script cache
    agent.rs     # LLM client
    render/
      mod.rs
      table.rs
      plain.rs
    log.rs       # JSONL logger
    metrics.rs   # counters
    trust.rs     # risk
    config.rs
  docs/
    mvp.md
    architecture.md
    trust-model.md
    behavior-log.md
    agent-protocol.md
    script-cache.md
```
