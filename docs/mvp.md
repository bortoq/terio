# MVP — Minimum Viable Product

## Цель

Доказать, что terio может:
1. Исполнить shell-команду и отрендерить результат.
2. Принять задачу на естественном языке, построить команды через AI-модель и выполнить их.
3. Распознать повторяемый workflow и превратить его в рецепт.
4. Откатить изменения (undo).

## ICP (Ideal Customer Profile)

**Пользователь командной строки**, который:
- ежедневно работает в терминале;
- повторяет одни и те же ручные последовательности;
- использует AI-агенты (OpenCode, Codex) или хотел бы попробовать;
- готов попробовать новый инструмент, если он сокращает переключения.

Выбор первого демо-воркфлоу определяется простотой подключения. Сейчас выбран FLAC/CUE, но Git, `gh`, `rsync`, `mpv` — такие же кандидаты. В MVP не фиксируется жёсткий список интеграций — фиксируется механика.

## Что входит в MVP

### 1. Shell execution
- `terio run -- <command>` — запуск произвольной shell-команды в CWD.
- Захват stdout, stderr, exit code, duration, argv.
- `terio rerun` — повтор последней команды.
- Stream вывода в реальном времени.

### 2. Agent (Built-in AI)
- `terio ask "..."` — естественно-языковой запрос.
- Агент генерирует shell-команды.
- Показывает трейс команд перед выполнением.
- Исполняет, рендерит результат.
- Провайдер: конфигурируемый (OpenAI, Anthropic, локальный).

### 3. Рендеринг вывода
- Plain text renderer (fallback).
- Table renderer для табличного вывода.
- Card renderer для статусов.
- Timeline renderer для git log.

### 4. Behavior Log
- JSONL-файл (v1) в `~/.terio/log/`.
- Каждая запись: schema_version, run_id, session_id, ts, kind, request, command, cwd, risk, exit, duration.
- Хранение raw output: `~/.terio/runs/<run_id>/stdout.log`.
- **Секреты редэктятся** из всех полей.

### 5. Behavior Compiler
- Анализ лога на повторяющиеся паттерны.
- Предложение рецепта после 3+ успешных выполнений.
- Recipe v1: YAML, structured argv, preconditions, postconditions.
- Валидация аргументов перед выполнением.

### 6. Trust Engine (MVP)
- Risk taxonomy.
- Confidence score: +0.2 за успех, -0.3 за неудачу.
- Порог предложения рецепта: 3 успеха.
- Порог авто-запуска: 0.8 (local_write), 0.95 (network_read).
- Expandable trace.

### 7. Undo/Redo
- Все изменения файлов (создание, запись, удаление) логируются.
- `rm` заменяется на trash (перемещение в `~/.terio/trash/`).
- `terio undo` — откат последнего изменения.
- `terio redo` — повтор отменённого.

### 8. Git / GitHub (как пример интеграции)
- `terio run -- git status` → таблица.
- `terio run -- gh issue list` → карточки.
- Не в виде коннекторов, а как shell-рендеринг.

### 9. Recipes
- `terio recipe list`.
- `terio recipe run <id> -- arg=value`.
- `terio recipe validate <file>`.

## Что НЕ входит в MVP

- ❌ Desktop-сборка (CLI only).
- ❌ Продвинутый редактор рецептов (редактирование через `$EDITOR` ок).
- ❌ Реестр рецептов / маркетплейс.
- ❌ Шэринг сессий.
- ❌ Team features.
- ❌ Cloud sync.
- ❌ Плагины / WebAssembly.
- ❌ Мобильный интерфейс.

Всё остальное (интеграция конкретных программ) **не исключается** — выбирается по принципу «что проще подключить».

## Критерии успеха MVP

1. `terio run -- ls -l` показывает таблицу.
2. `terio ask "list files"` генерирует `ls`, показывает таблицу.
3. `terio log` показывает историю.
4. После 3+ FLAC/CUE сплитов terio предлагает рецепт.
5. Рецепт с новыми аргументами даёт корректный результат.
6. `terio undo` откатывает созданные файлы.
7. Secrets не попадают в лог.
8. `terio run -- git status` показывает статус как таблицу.

## CLI контракт (MVP)

```bash
terio run -- <command...>    # shell-команда
terio ask "<request>"        # естественный язык
terio rerun                  # повтор последней
terio undo                   # откат
terio redo                   # повтор отменённого
terio log                    # история
terio log --json             # история в JSON
terio recipe list            # список рецептов
terio recipe run <id> -- arg=value  # запуск рецепта
terio recipe validate <file> # валидация рецепта
terio trace <run_id>         # трейс выполнения
terio config                 # настройки
```

## Структура репозитория (MVP)

```
terio/
  README.md
  LICENSE
  Cargo.toml
  src/
    main.rs
    cli.rs
    exec.rs
    agent.rs
    render/
      mod.rs
      table.rs
      card.rs
      plain.rs
    log.rs
    recipe/
      mod.rs
      yaml.rs
      validate.rs
      compile.rs
    trust.rs
    undo.rs
    config.rs
  docs/
    mvp.md
    architecture.md
    trust-model.md
    behavior-log.md
  examples/
    recipes/
    logs/
    blocks/
```
