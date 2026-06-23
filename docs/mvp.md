# MVP — Minimum Viable Product

## Принцип

MVP доказывает одно: terio может принять задачу на естественном языке, выполнить её через AI-модель, запомнить цепочку команд и в следующий раз выполнить без модели.

Максимум функциональности при минимуме реализации — за счёт того, что **AI-модель берёт на себя сложность**, а terio только кеширует результат её работы.

## Что входит в MVP-0

### 1. `terio ask "..."` — естественно-языковой запрос
- terio принимает запрос, ищет в кеше скриптов.
- Если скрипт найден — выполняет без модели.
- Если не найден — отправляет модели.

### 2. Agent (AI-модель)
- Модель получает: запрос, CWD, список файлов в CWD.
- Модель возвращает: цепочку shell-команд (structured: command + argv) с обоснованием.
- Провайдер: конфигурируемый (локальный: ollama/llama.cpp; удалённый: OpenAI, Anthropic).
- **Правило:** модель не исполняет. Только планирует. Исполняет terio.

### 3. Plan → Confirm → Execute
- Агент возвращает план. terio показывает: `Will execute: 3 commands. Risk: local_write. Proceed? [Y/n]`.
- Пользователь подтверждает (или отклоняет).
- terio выполняет команды, показывает результат.

### 4. Script Cache
- Успешная цепочка сохраняется: `normalized_request → script`.
- Формат: JSON-файл.
- В следующий раз: `terio ask "тот же запрос"` → проверка в кеше → скрипт выполняется без модели.
- **Ключевое:** это не recipe engine, не YAML-формат, не компилятор. Просто запрос → скрипт.

### 5. Execution Layer
- `terio run -- <command>` — прямая shell-команда (без модели).
- Захват stdout, stderr, exit code, duration.
- `terio rerun` — повтор последней команды.

### 6. Renderer
- Plain text (fallback).
- Table (для ls, csv-подобного вывода).
- Авто-определение типа вывода (первые N строк).

### 7. Behavior Log
- JSONL, schema v1.
- Три вида записей: `agent_turn`, `command_run`, `script_run`.
- Секреты редэктятся из всех полей.

### 8. Trust (минимальный)
- Risk levels: read_only, local_write, destructive, network_read, network_write.
- Destructive/network_write → всегда подтверждение (даже для скрипта из кеша).
- Для скриптов из кеша с risk <= local_write: авто-запуск после 3 успехов.

### 9. Undo/Redo (Experimental, off by default)
- **Не гарантируется.** Best-effort для кешированных скриптов.
- Два режима (в конфиге): sandbox (bubblewrap) или warn-only.
- По умолчанию: выключен.

## Что НЕ входит в MVP-0

- ❌ Recipe compiler / YAML recipes.
- ❌ GitHub/Git/media connectors как отдельные компоненты (но можно управлять через shell).
- ❌ Полноценный undo/redo.
- ❌ Desktop-сборка.
- ❌ Маркетплейс скриптов.
- ❌ Шэринг.
- ❌ Команда `terio recipe ...` — только `terio ask` и `terio run`.

## CLI контракт (MVP-0)

```bash
terio ask "<request>"         # запрос на естественном языке
terio run -- <command...>     # shell-команда (без модели)
terio rerun                   # повтор последней команды
terio log                     # история
terio log --json              # история в JSON
terio cancel                  # отмена текущего выполнения
terio config                  # настройки (провайдер, риск-политики)
```

## Критерии успеха

1. `terio ask "list files"` → показывает файлы как таблицу.
2. `terio ask "list files"` (повторно) → выполняется без вызова модели (быстрее).
3. `terio run -- echo hello` → hello.
4. `terio log` показывает историю.
5. Агент не выполняет destructive-команды без подтверждения.
6. Secrets не попадают в лог.

## Структура репозитория (MVP)

```
terio/
  README.md
  LICENSE
  Cargo.toml
  src/
    main.rs
    cli.rs
    ask.rs          # agent → plan → execute cycle
    run.rs          # shell execution
    matcher.rs      # request matcher (cache lookup)
    cache.rs        # script cache (read/write)
    render/
      mod.rs
      table.rs
      plain.rs
    log.rs          # JSONL logger
    trust.rs        # risk check
    config.rs
  docs/
    mvp.md
    architecture.md
    trust-model.md
    behavior-log.md
```
