# MVP — Minimum Viable Product

## Принцип

terio доказывает: пользователь вводит задачу на естественном языке, AI-модель строит цепочку команд, terio выполняет и запоминает. Тот же запрос в следующий раз — без модели.

Лог — центральный источник для UI. Renderer читает из лога. Каждая запись лога содержит счётчики расходов для будущей оптимизации стоимости.

## Что входит в MVP

### 1. `terio ask "..."` — естественно-языковой запрос
- Принимает запрос, генерирует `interaction_id` (UUID для группировки пар).
- Ищет **exact normalized match** в Script Cache.
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
- Параметры: glob_one-паттерны для файлов (ровно один файл; если 0 или >1 — terio спрашивает).
- Preconditions: binary_exists, glob_one, file_exists.
- Match: только exact normalized.
- Auto-run: после trust_threshold успехов, только exact match, risk <= local_write.

### 5. Execution Layer
- `terio run -- <command>` — прямая shell-команда.
- Захват stdout, stderr, exit code, duration.
- `terio rerun`.

### 6. Log (Writer + Reader)
- `LogWriter` trait: `append(LogEntry)`. Реализация — `JsonlLogWriter`.
- `LogReader` trait: `recent(n)`, `by_session(id)`, `by_interaction(id)`, `stream()`.
- JSONL на диске + in-memory broadcast channel для real-time.
- Каждая запись содержит: `instance_id`, `session_id`, `interaction_id`, `display_profile`, `cost_counters`.
- Renderer подписан на `LogReader.stream()`.
- **Подготовка к смене формата:** LogWriter/LogReader traits изолируют реализацию.

### 7. Renderer
- Читает из лога (LogReader).
- Plain text (fallback).
- Table (ls, csv-подобный вывод).
- Авто-определение типа на основе `display_profile`.
- UI: Dioxus (webview). Терминальный fallback: ratatui.

### 8. Accounting
- `cost_counters` в каждой записи лога.
- `fn aggregate(counters: &[CostCounters]) -> AggregatedCosts`.
- `fn compute_attention_cost(counters: &CostCounters) -> f64` — заглушка (0.0).
- `terio stats` показывает суммы cost_counters.

### 9. Identity
- `instance_id` — ULID, генерируется при первом запуске, хранится в `~/.terio/instance.json`.
- `session_id` — UUID на каждый запуск.
- `instance_id` и `session_id` пишутся в каждую запись лога.
- Подготовка к будущему: шэринг, возобновление сессий.

### 10. Trust (минимальный)
- Risk: read_only, local_write, destructive, network_read, network_write.
- Destructive/network_write → всегда подтверждение.
- Exact match только. Fuzzy match не в MVP.
- Auto-run: exact match + risk <= local_write + success_count >= 3 + все parameters однозначны + preconditions пройдены + output внутри CWD.

### 11. Undo/Redo (Experimental, off)
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
- ❌ Реальная метрика `total_attention_cost` (только счётчики-заглушки).
- ❌ Оконная система (только блоки).
- ❌ Предсказание ввода (pre-execution).
- ❌ Fine-tuning LLM.

## CLI контракт (MVP)

```bash
terio ask "<request>"         # запрос на естественном языке
terio run -- <command...>     # shell-команда
terio rerun                   # повтор последней
terio log                     # история
terio log --json              # история в JSON
terio stats                   # метрики + cost_counters
terio cancel                  # отмена
terio config                  # настройки
```

## Критерии успеха

1. `terio ask "list files"` → таблица.
2. `terio ask "list files"` (повторно) → без модели, быстрее.
3. `terio run -- echo hello` → hello.
4. `terio log` показывает историю, сгруппированную по `interaction_id`.
5. `terio stats` показывает cache_hits > 0, model_calls > 0, cost_counters.
6. Destructive-запросы требуют подтверждения.
7. Secrets не в логе.
8. `display_profile` скрывает `credential_access` записи.

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
    log/
      mod.rs     # LogWriter + LogReader traits
      writer.rs  # JsonlLogWriter
      reader.rs  # JsonlLogReader
    accounting.rs # cost_counters, aggregate, заглушка
    identity.rs  # instance_id, session_id
    trust.rs     # risk
    config.rs
  docs/
    mvp.md
    architecture.md
    trust-model.md
    behavior-log.md
    agent-protocol.md
    script-cache.md
  schemas/
    agent-output.schema.json
    script-cache.schema.json
    behavior-log.schema.json
```
