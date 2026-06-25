# Current implementation status

После коммита `9e0bbf3` (Phase 0: terminal-like UI + window model).

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
| `terio help` | `src/main.rs` | Встроенная справка |
| `terio mode <quiet|normal|debug>` | `src/main.rs` | Режим внимания |
| `terio focus <up|down>` | `src/main.rs` | Переключение окна (UI) |
| `terio scroll <N>` | `src/main.rs` | Скролл (UI) |
| `terio repeat` | `src/main.rs` | Повтор последнего запроса |
| `terio sandbox status` | `src/main.rs` | Состояние песочницы |

### CLI (экспериментально)

| Команда | Статус | Описание |
|---------|--------|----------|
| `terio undo` | ⚠ experimental | Откат snapshot-backed выполнения |
| `terio redo` | ⚠ experimental | Повтор undo |

### UI (Dioxus desktop webview)

- Опционально, feature `desktop`
- Terminal-like: чёрный экран, ввод внизу (`$`), вывод — окнами
- Window model: Window { id, kind: Text | Confirm }, WindowManager (VecDeque + FocusIn/FocusOut)
- Input routing: `help`, `mode`, `focus`, `scroll`, `repeat`, `y/n` → UiCommand
- Persistent FocusOut (Dioxus signal, не сбрасывается при ререндере)
- PendingConfirmation → `WindowKind::Confirm` (окно-подтверждение в потоке)
- Activity indicator + undo/redo status bar
- Ctrl+L для refresh/очистки

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

### Песочница (Phase 1: CoW)

- `bubblewrap` wrapper с двумя режимами:
  - legacy: `--ro-bind / /` + `--share-net` (по умолчанию)
  - strict: пустой rootfs + bind mounts `/bin`, `/usr`, `/lib*` + без сети
- Snapshot до выполнения, rollback при ошибке/отмене
- `no_read_paths` в конфиге (path → `--tmpfs` override)
- Auto-trust: read-only=1 успех, local_write=3 успеха
- `terio sandbox status` — просмотр состояния

---

## Тесты

- 143+ unit-тестов (lib)
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
