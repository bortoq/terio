# Architecture

## Core Idea

**terio — интегратор интерфейсов.** Пользователь работает в едином окне-терминале.
terio под капотом использует LLM, кеш скриптов и песочницу, а результат отдаёт как окно — от текста до видео.

Внешне terio неотличим от терминала: чёрный экран, ввод внизу, вывод наверх.
Единственное видимое отличие — результат может быть rich-окном (плеер, браузер, графика),
которому terio передаёт фокус после вывода.

```
┌─────────────────────────────────────────────┐
│  terio window                               │
│                                             │
│  ┌─── Window #3 (scrollback) ────────────┐  │
│  │  result of "cat file.txt"             │  │
│  └───────────────────────────────────────┘  │
│  ┌─── Window #2 (FocusOut) ──────────────┐  │
│  │  result of "ls -la"                   │  │
│  │  total 42                             │  │
│  │  drwxr-xr-x  12 user  users   384 Jun │  │
│  └───────────────────────────────────────┘  │
│  ┌─── Window #1 (FocusIn) ──────────────┐  │
│  │  $ _                                  │  │
│  └───────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

## Компоненты

```
┌──────────────────────────────────────────────┐
│              WINDOW MANAGER                   │
│  VecDeque<Window>, FocusIn, FocusOut,         │
│  viewport, scrollback                         │
├──────────────────────────────────────────────┤
│              INPUT SURFACE                    │
│  Текст + Enter. Мышь/хоткеи — через скрипты.  │
│  interaction_id генерируется здесь.            │
├──────────┬───────────────────┬───────────────┤
│  SCRIPT  │  EXECUTION LAYER  │  SANDBOX      │
│  ENGINE  │  (shell, process) │  (CoW/bwrap)  │
├──────────┴───────────────────┴───────────────┤
│           SCRIPT CACHE + SYNONYM DICT         │
│  NormalizedQuery → ScriptId → steps          │
│  Синонимы: старые/неточные запросы → скрипт   │
├──────────────────────────────────────────────┤
│           LOG (LogStore)                      │
│  LogWriter trait → JsonlLogWriter             │
│  LogReader trait → JsonlLogReader             │
│  LogEventStream (broadcast)                   │
├──────────────────────────────────────────────┤
│           ACCOUNTING + COST OPTIMIZER         │
│  cost_counters, C_total формула,              │
│  выбор маршрута (script vs LLM)              │
├──────────────────────────────────────────────┤
│           STORAGE + IDENTITY                  │
│  config, cache, log, sandbox snapshots        │
│  instance_id + session_id                     │
└──────────────────────────────────────────────┘
```

### 1. Window Manager

Управляет массивом окон (`VecDeque<Window>`):

```rust
struct Window {
    id: Uuid,
    kind: WindowKind,       // Text | Rich { url, mime } | Confirm
    content: String,        // отображённый текст (или HTML для rich)
    focusable: bool,
    created_at: DateTime,
}

enum WindowKind {
    Text,                    // обычный текст (stdout)
    Rich { url: String, mime: String },  // плеер, iframe
    Confirm { prompt: String },          // запрос подтверждения
}
```

- **FocusIn** — окно ввода (всегда внизу, всегда видимо).
- **FocusOut** — окно для скролла (подсветка, переключение Tab/Shift+Tab или скриптами).
- **Scrollback**: при overflow окна уходят в историю (но не уничтожаются).
  Прокрутка вверх — подтягивает старые окна.
- Восстановление из лога при запуске: последние N окон загружаются из LogStore.

### 2. Input Surface

- **Единственный обязательный протокол ввода:** текст + Enter.
- Мышь, хоткеи, Tab-переключение — реализуются скриптами (см. Script Engine).
- Каждый запрос генерирует `interaction_id` (UUID).

### 3. Script Engine

Всё управление terio — через скрипты:

```
terio-scripts/
  core/          # встроенные (нельзя удалить, можно переопределить)
    help.ts
    focus.ts
    config.ts
    confirm.ts
    security.default.ts
    security.strict.ts
    security.off.ts
  user/          # пользовательские
  learned/       # созданные из успешных LLM-запросов
```

Формат скрипта (YAML):
```yaml
name: help
triggers: ["help", "помощь", "h"]
steps:
  - run: "terio --help"
  - show: window.kind.help
```

- **Инвариант:** скрипты запускает terio, не наоборот. Интерпретатор — часть ядра на Rust.

### 4. Execution Layer

- Запускает процесс, стримит stdout/stderr.
- Отмена: Ctrl+C, таймаут, `terio cancel`.
- Untrusted-команды → песочница (CoW).

### 5. Sandbox (CoW)

- **Copy-on-Write:** перед untrusted-командой все изменяемые файлы копируются в `~/.terio/sandbox/<id>/snap/`.
- При подтверждении — снапшоты удаляются (commit).
- При отказе/ошибке — снапшоты восстанавливаются (rollback).
- **Изоляция чтения:** через `bubblewrap` с пустым rootfs + bind mounts разрешённых путей.
- **Продвижение:** после 1 успешного выполнения untrusted → trusted.
  Trusted-скрипты выполняются без песочницы.
- Белые списки `no_read_paths` в конфиге.

### 6. Script Cache + Synonym Dictionary

```rust
struct ScriptCache {
    // Точное совпадение (normalized)
    exact: HashMap<NormalizedQuery, ScriptId>,
    // Синонимы: разные запросы → один скрипт
    synonyms: HashMap<NormalizedQuery, ScriptId>,
}

struct Script {
    id: ScriptId,
    steps: Vec<Step>,
    success_count: u64,
    total_runs: u64,
    confidence: f64,  // success_count / total_runs
}
```

- **Пополнение:** после успешного LLM-запроса → запись в exact.
- **Синонимы:** если `"help"` привёл к ошибке, а затем `"terio help"` — успех,
  то `"help"` → синоним → тот же скрипт.
- **Чистка:** синонимы с частотой < порога удаляются.

### 7. Log (LogStore)

Без изменений относительно текущей реализации:
- `LogWriter` trait → `JsonlLogWriter`
- `LogReader` trait → `JsonlLogReader`
- `LogEventStream` (broadcast после записи в буфер)
- Каждая запись лога: `instance_id`, `session_id`, `interaction_id`, `cost_counters`.

### 8. Accounting + Cost Optimizer

```rust
// Формула полной стоимости
C_total = C_llm_tokens + C_user_attention + C_risk

C_risk = P(failure) * C_rollback
C_rollback = время восстановления + потери при необратимых операциях
```

- Оптимизатор выбирает: выполнить скрипт (дёшево) или спросить LLM (гибко).
- Байесовский классификатор для точности предсказаний (phase 5).

### 9. Режимы внимания

| Режим | Поведение | Когда использовать |
|-------|-----------|------------------|
| `quiet` | Нет подтверждений, всё в лог | Default, 90% пользователей |
| `normal` | Подтверждение untrusted (1 раз/сессию/скрипт) | Обычная работа |
| `debug` | Каждый шаг подтверждается | Отладка новых действий |

Переключение: `terio mode quiet|normal|debug`.

## Data Flow

```
Пользователь → [текст + Enter]
  → Input Surface → interaction_id
    → Script Engine: поиск в exact + synonyms
      → Найден? → Execution Layer (trusted → напрямую,
                                        untrusted → Sandbox)
      → Не найден? → LLM → план → окно-подтверждение
        → Подтверждено? → Execution → Log → Window
        → Отклонено? → Window закрывается
    → Результат → Window Manager → viewport
```

## Key Design Rules

1. **Терминальная парадигма:** пользователь не должен замечать, что работает не с терминалом.
2. **Окно = результат:** каждый ответ — окно. Никаких режимов.
3. **Скрипты — единственный способ управления:** всё настраивается через скрипты.
4. **Экономия внимания:** quiet mode по умолчанию. Внимание пользователя — самый дорогой ресурс.
5. **Песочница для untrusted:** защита без отвлечения пользователя.
6. **Проактивность:** terio предугадывает, но не навязывается.
