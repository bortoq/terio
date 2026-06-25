# Current implementation status

После коммита `8acb1fa` (docs pivot) и `00fbbd8` (Phase 7 + audit fixes).

---

## Что работает

### CLI (стабильно)

| Команда | Файл | Описание |
|---------|------|----------|
| `terio ask <request>` | `src/ask.rs` | Запрос через LLM (OpenAI/Ollama/Mock) |
| `terio run -- <cmd>` | `src/run.rs` | Shell-команда |
| `terio confirm` | `src/ask.rs` | Подтверждение сохранённого плана |
| `terio cancel` | `src/main.rs` | SIGTERM активному процессу |
| `terio log --json` | `src/log/` | Просмотр лога |
| `terio stats` | `src/accounting.rs` | Агрегированные cost_counters |
| `terio config show/set` | `src/config.rs` | Настройки провайдера, политик |
| `terio learn <program>` | `src/integration.rs` | Обучение работе с программой |
| `terio integrations` | `src/integration.rs` | Статус изученных программ |
| `terio forget <program>` | `src/integration.rs` | Забыть программу |
| `terio share` | `src/integration.rs` | Экспорт окна |
| `terio receive` | `src/integration.rs` | Импорт окна |

### CLI (экспериментально)

| Команда | Статус | Описание |
|---------|--------|----------|
| `terio undo` | ⚠ experimental | Откат snapshot-backed выполнения |
| `terio redo` | ⚠ experimental | Повтор undo |

### UI (Dioxus desktop webview)

- Опционально, feature `desktop`
- Multi-view workspace: Auto, Table, Timeline, Cards, Readable, Chat
- Поле ввода + кнопка Ask
- Pending confirmation с exact execution
- Trust level индикатор
- Undo/Redo кнопки
- Live-stream обновление (in-process)

### Лог

- `LogWriter` trait → `JsonlLogWriter`
- `LogReader` trait → `JsonlLogReader`
- `LogStore`: writer + reader + broadcast
- Формат: JSONL, каждая запись с schema validation

### Кеш скриптов

- Exact normalized match (SHA-256)
- Structured command chains (не shell-скрипты)
- Risk, scope, success_count, template metadata
- Cache lookup + auto-run если trust threshold достигнут

### Безопасность

- Risk levels: read_only, local_write, destructive, network, credential
- Policy: always_ask / ask_once / allow
- Scope validation (+ path boundary)
- Redact: Bearer, API key, token, SSH, URL credentials
- Hash-bound confirmation для exact plan

### Интеграции (Phase 7)

- IntegrationManager: learn/forget/list/init
- Learn через --help программы
- ShareWindow: экспорт/импорт (LogEntry + cache entries)

### Песочница (experimental)

- `bubblewrap` wrapper (если доступен)
- Best-effort snapshot для скриптов
- Off by default

---

## Тесты

- 135 unit-тестов (lib)
- CI: `cargo fmt --check` + `cargo clippy -- -D warnings` + `cargo build` + `cargo test`

---

## Структура исходников

```
src/
  main.rs           # CLI entry point, handlers
  lib.rs            # pub mod declarations
  cli.rs            # Clap CLI definition
  config.rs         # Configuration
  identity.rs       # instance_id / session_id
  types.rs          # LogEntry, RowData, shared types
  log/              # LogStore, LogWriter, LogReader
  ui/               # Dioxus desktop UI
  run.rs            # Shell execution
  ask.rs            # LLM request / confirmation
  agent.rs          # Agent planning (mock)
  provider.rs       # Provider trait (OpenAI, Ollama, Mock)
  cache.rs          # Script cache
  matcher.rs        # Request normalization
  trust.rs          # Trust evaluation
  redact.rs         # Secrets redaction
  undo.rs           # Snapshot/undo
  integration.rs    # Integration manager
  accounting.rs     # Cost counters
```

## Документы

| Файл | Содержание |
|------|-----------|
| [architecture.md](../architecture.md) | Current + Target + Migration plan |
| [roadmap.md](../roadmap.md) | Target roadmap после pivot |
| [docs/migration-to-window-model.md](migration-to-window-model.md) | План миграции |
| [docs/todo.md](todo.md) | Отложенные фичи |
