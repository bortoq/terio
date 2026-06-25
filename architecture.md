# Architecture

Этот документ содержит два раздела:
1. **Current architecture** — как код устроен сейчас
2. **Target architecture** — новое видение после pivot `8acb1fa`
3. **Migration plan** — как перейти от current к target

---

# 1. Current architecture

Текущая реализация terio.

## Core components

```
┌──────────────────────────────────────────────────┐
│              COMMAND SURFACE                      │
│   terio ask / terio run -- <command>              │
│   interaction_id генерируется здесь               │
├──────────────────────────────────────────────────┤
│              REQUEST MATCHER                      │
│   exact normalized match → script; иначе → agent  │
├──────────┬───────────────────┬───────────────────┤
│  AGENT   │  EXECUTION LAYER  │  UI (Dioxus)      │
│  (LLM)   │  (shell, process) │  multi-view       │
├──────────┴───────────────────┴───────────────────┤
│           SCRIPT CACHE                            │
│   normalized_request → { steps, risk, metadata }  │
├──────────────────────────────────────────────────┤
│           LOG (LogStore)                          │
│   LogWriter trait → JsonlLogWriter                │
│   LogReader trait → JsonlLogReader                │
│   LogEventStream (broadcast)                      │
├──────────────────────────────────────────────────┤
│           INTEGRATION MANAGER                     │
│   learn/forget/share/receive                      │
├──────────────────────────────────────────────────┤
│           ACCOUNTING                              │
│   cost_counters в каждой записи лога              │
├──────────────────────────────────────────────────┤
│           TRUST + UNDO                            │
│   Policy, auto-run, scope validation              │
│   Best-effort snapshot/undo/bubblewrap            │
├──────────────────────────────────────────────────┤
│           STORAGE + IDENTITY                      │
│   config, cache, log, trash                       │
│   instance_id + session_id                        │
└──────────────────────────────────────────────────┘
```

### CLI commands (from `src/cli.rs`)

| Команда | Назначение | Статус |
|---------|-----------|--------|
| `ask` | Запрос через LLM | ✓ stable |
| `run` | Shell-команда | ✓ stable |
| `log` | Просмотр лога | ✓ stable |
| `ui` | Открыть Dioxus UI | ✓ stable |
| `stats` | Метрики | ✓ stable |
| `cancel` | Отмена операции | ✓ stable |
| `confirm` | Подтверждение плана | ✓ stable |
| `undo/redo` | Откат/повтор | ⚠ experimental |
| `config` | Настройки | ✓ stable |
| `learn/integrations/forget` | Интеграции | ✓ Phase 7 |
| `share/receive` | Шэринг окон | ✓ Phase 7 |

### UI

- Dioxus 0.6 desktop webview, опционально (feature `desktop`)
- Multi-view workspace: Auto, Table, Timeline, Cards, Readable, Chat
- Компоненты в `src/ui/`:
  - `app.rs` — Dioxus-компоненты
  - `state.rs` — глобальное состояние, RowData, prepare_rows
  - `renderer.rs` — выбор режима отображения

### Log

- `LogWriter` trait → `JsonlLogWriter` (append, flush)
- `LogReader` trait → `JsonlLogReader` (recent, by_session, by_interaction)
- `LogStore` — writer + reader + broadcast channel
- Каждая запись: `instance_id`, `session_id`, `interaction_id`, `cost_counters`

### Agent

- Provider trait: `plan(&self, request) → AgentPlan`
- Реализации: OpenAI, Anthropic, Ollama, Mock
- Secrets redact перед отправкой

### Trust

- Risk levels: read_only, local_write, destructive, network, credential
- Policy: always_ask / ask_once / allow
- Auto-run: exact match + risk ≤ local_write + N успехов + scope
- Fuzzy match: никогда не auto-run

### Integration Manager

- `terio learn <program>` — запускает --help, парсит, генерирует скрипт
- `terio integrations` — список изученных
- `terio forget` — удаление
- `terio share/receive` — экспорт/импорт SharedWindow

---

# 2. Target architecture

Новое видение после pivot `8acb1fa`. Постепенно заменяет current architecture.

## Product metaphor

terio — интегратор интерфейсов, внешне неотличимый от терминала.
Единственное отличие — результат может быть rich-окном (плеер, браузер, графика).

```
┌─────────────────────────────────────────────┐
│  terio window                               │
│                                             │
│  ┌─── Window #3 (scrollback) ────────────┐  │
│  │  result of "cat file.txt"             │  │
│  └───────────────────────────────────────┘  │
│  ┌─── Window #2 (FocusOut) ──────────────┐  │
│  │  result of "ls -la"                   │  │
│  └───────────────────────────────────────┘  │
│  ┌─── Window #1 (FocusIn, always visible)┐  │
│  │  $ _                                  │  │
│  └───────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

## Key design rules

1. **Терминальная парадигма:** пользователь не замечает, что работает не с терминалом.
2. **Окно = результат:** каждый ответ — окно. Никаких режимов просмотра.
3. **Скрипты — единственный способ управления:** всё настраивается скриптами.
4. **Экономия внимания:** quiet mode по умолчанию.
5. **Песочница для untrusted:** CoW-изоляция без отвлечения пользователя.
6. **Проактивность:** terio предугадывает, но не навязывается.

## Target components

```
┌──────────────────────────────────────────────┐
│              WINDOW MANAGER                   │
│  VecDeque<Window>, FocusIn, FocusOut,         │
│  viewport, scrollback                         │
├──────────────────────────────────────────────┤
│              INPUT SURFACE                    │
│  Текст + Enter. Мышь/хоткеи — через скрипты.  │
├──────────┬───────────────────┬───────────────┤
│  SCRIPT  │  EXECUTION LAYER  │  SANDBOX      │
│  ENGINE  │  (shell, process) │  (CoW/bwrap)  │
├──────────┴───────────────────┴───────────────┤
│           SCRIPT CACHE + SYNONYM DICT         │
│  NormalizedQuery → ScriptId → steps          │
│  Синонимы: старые запросы → тот же скрипт     │
├──────────────────────────────────────────────┤
│           LOG (LogStore)                      │
│  (без изменений относительно current)         │
├──────────────────────────────────────────────┤
│           ACCOUNTING + COST OPTIMIZER         │
│  C_total = C_llm + C_attention + C_risk      │
├──────────────────────────────────────────────┤
│           STORAGE + IDENTITY                  │
│  (без изменений относительно current)         │
└──────────────────────────────────────────────┘
```

### Window model

```rust
struct Window {
    id: Uuid,
    kind: WindowKind,   // Text | Rich { url, mime } | Confirm
    content: String,
    focusable: bool,
    created_at: DateTime,
}
```

### Focus

- **FocusIn** — окно ввода (всегда внизу, всегда видимо)
- **FocusOut** — окно для скролла (подсветка, переключается скриптами)

### Scrollback

- Окна не уничтожаются, а уходят в историю при overflow
- Прокрутка вверх подтягивает старые окна

### Attention modes

| Режим | Поведение | Default |
|-------|-----------|---------|
| `quiet` | Нет подтверждений, всё в лог | ✓ Default |
| `normal` | Подтверждение untrusted (1 раз/сессию) | |
| `debug` | Каждый шаг подтверждается | |

### Sandbox (target)

- **CoW:** перед untrusted-командой — snapshot изменяемых файлов
- **Изоляция чтения:** bubblewrap с пустым rootfs + bind mounts
- **Продвижение в trusted:** read-only → 1 успех; local_write → N успехов; network/destructive → никогда auto-trust

### Script engine (target)

- Всё управление через скрипты: help, config, focus, confirm, security
- Интерпретатор — ядро terio (Rust)
- Три уровня: `core/` (встроенные), `user/` (пользовательские), `learned/` (из LLM)

### Cost optimizer (target)

```
C_total = C_llm_tokens + C_user_attention + C_risk
C_risk = P(failure) * C_rollback
```

---

# 3. Migration plan

Как перейти от current multi-view Dioxus workspace к target terminal-like window manager.

## Step 1: Документация и alignment (текущий коммит)

- [x] README: Current vs Target
- [x] architecture.md: current + target + migration plan
- [x] roadmap.md: добавить pivot note
- [x] docs/current-status.md
- [x] docs/migration-to-window-model.md
- [x] cli.rs: обновить about

## Step 2: Window model (замена multi-view)

- [ ] Добавить `Window` struct и `WindowManager` (VecDeque + FocusIn/Out)
- [ ] Заменить режимы (Table/Timeline/Cards/Chat/Auto) на `WindowKind::Text`
- [ ] InputSurface: строка ввода внизу (как терминал)
- [ ] Перенести существующие рендеры в WindowKind: текстовый → Text, подтверждение → Confirm
- [ ] Убрать переключатель режимов из UI
- [ ] Log → Window: восстановление окон из лога при запуске

## Step 3: Attention modes и confirm как окно

- [ ] Три режима: quiet / normal / debug (переключение через конфиг)
- [ ] Confirm из отдельного диалога → окно `WindowKind::Confirm` в потоке
- [ ] Убрать `terio confirm` как отдельную команду (заменить на окно-подтверждение)

## Step 4: Sandbox

- [ ] Использовать существующий `undo.rs` (snapshot/bubblewrap) как базис
- [ ] CoW: snapshot до untrusted → rollback при ошибке
- [ ] Изоляция чтения: bwrap с bind mounts
- [ ] Продвижение: read-only → 1 успех; local_write → N успехов

## Step 5: Script engine

- [ ] Интерпретатор скриптов в ядре
- [ ] Перенос help/config/focus/confirm в скрипты
- [ ] synonym dictionary на базе существующего matcher.rs

## Step 6: Проактивность и cost optimizer

- [ ] Предугадывание следующей команды по логу
- [ ] Формула C_total
- [ ] Байесовский классификатор

## Что сохраняется без изменений

- LogStore, LogWriter/LogReader traits
- Script Cache (с добавлением synonym dict)
- Identity (instance_id, session_id)
- Accounting (cost_counters)
- Redaction
- Integration Manager (learn/forget/share/receive будет переписан на script engine)
