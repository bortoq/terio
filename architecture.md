# Architecture

## Core Idea

terio — это три слоя, которые вместе дают пользователю единое место управления:

```
┌─────────────────────────────────────────────────┐
│              COMMAND SURFACE                     │
│    (shell commands / natural language / hotkeys) │
├─────────────────────────────────────────────────┤
│              EXECUTION LAYER                     │
│      (локальный shell, process lifecycle)        │
├─────────────────────────────────────────────────┤
│           WEB RENDERER / OUTPUT LAYER            │
│   (рендеринг stdout/result в структурированные   │
│    блоки: таблицы, карточки, таймлайны и т.д.)   │
├─────────────────────────────────────────────────┤
│           BEHAVIOR COMPILER & LOG                │
│   (запись, распознавание, компиляция рецептов,   │
│    trust engine, fallback)                       │
├─────────────────────────────────────────────────┤
│                STORAGE LAYER                     │
│   (preferences, log, рецепты, trust scores)      │
└─────────────────────────────────────────────────┘
```

Всё остальное (Git, GitHub, медиа, базы данных, новости) — это **shell-команды**, которые пользователь вводит в Command Surface, а terio исполняет и рендерит. terio не строит для них отдельные коннекторы на этапе MVP — их заменяет агент (LLM), который генерирует нужную команду.

## Компоненты

### 1. Command Surface
- Принимает: shell-команды (строки), естественно-языковые запросы (для агента), горячие клавиши.
- Классифицирует ввод: прямая shell-команда → Execution Layer; запрос → Agent Layer; известный рецепт → Behavior Compiler.
- **Инвариант:** каждый ввод получает классификацию до исполнения.

### 2. Execution Layer
- Запускает shell-процессы в CWD пользователя.
- Захватывает: stdout, stderr, exit code, duration, working directory.
- Предусловие: команда не исполняется, если не прошла validation (для рецептов) или confirmation (по risk level).
- Постусловие: результат + метаданные передаются в Renderer и Log.
- **Модель процесса:** `spawn -> stream -> collect -> return`. stdout/stderr стримятся в реальном времени.
- **Отмена:** если процесс не отменяем стандартными сигналами, terio создаёт «отменяющее задание» (kill + cleanup).

### 3. Web Renderer (Output Layer)
- Получает на вход: `ExecutionResult { stdout, stderr, exit_code, duration, artifacts }`.
- Выбирает тип блока: `table`, `card`, `timeline`, `gallery`, `progress`, `log`, `plain_text`.
- Возвращает структурированный блок.
- Формат блока:
  ```json
  {
    "type": "table",
    "title": "Track Split Results",
    "status": "success",
    "data": { ... },
    "actions": ["open_folder", "play_album"],
    "raw_output": "stdout here"
  }
  ```
- **Правило:** plain text остаётся валидным, когда он лучше структуры.

### 4. Agent Layer
- Планирует неизвестные задачи.
- Извлекает аргументы и выбирает инструменты (shell-команды).
- Объясняет рискованные действия.
- Срабатывает только когда рецепт не найден или упал.
- **Инвариант:** агент не вызывается, если сработал доверенный рецепт.

### 5. Behavior Log
- Хранит: request, resolved arguments, command chain, result summary, exit status, duration, risk level, error signals.
- Формат: JSONL (одна запись на выполнение).
- **Правило:** секреты (токены, ключи) не попадают в лог — редэктятся до записи.
- Используется Behavior Compiler для обнаружения паттернов.

### 6. Behavior Compiler
- **Pipeline:** Capture → Normalize → Cluster → Propose → Validate → Execute → Observe → Score → Fallback.
- **Вход:** Behavior Log (история успешных выполнений).
- **Выход:** Recipe (YAML).
- Кеширует **поведение**, а не ответы LLM.
- Не предлагает рецепт, пока нет N успешных выполнений (N настраивается, default: 3).

### 7. Trust Engine
- Решает: можно ли выполнить рецепт автоматически?
- Вход: confidence score, история успехов/неудач, risk level, valid arguments.
- Политики: `always_ask` / `ask_once` / `allow_in_dir` / `allow_for_recipe` / `never_allow`.
- **Постусловие:** решение логируется и показывается пользователю (expandable trace).

## Risk Taxonomy

| Risk Level | Примеры | Default Policy |
|------------|---------|----------------|
| `read_only` | `ls`, `cat`, `git status` | Auto |
| `local_write` | `mkdir`, `cp`, `ffmpeg -i` | Confirm per recipe |
| `destructive` | `rm -rf`, `mv --overwrite` | Always confirm |
| `network_read` | `curl`, `wget`, `git fetch` | Auto (recipe: confirm) |
| `network_write` | `git push`, `POST` | Always confirm |
| `credential_access` | токены, ключи, .env | Always confirm, не логировать |
| `financial` | покупки, платежи | Always confirm |

## Recipe Format (MVP)

```yaml
id: split_flac_cue_v1
name: Split FLAC/CUE album
risk: local_write
arguments:
  flac_file:
    type: file
    required: true
    pattern: "*.flac"
  cue_file:
    type: file
    required: true
    pattern: "*.cue"
  output_dir:
    type: directory
    default: "./tracks"
  naming_template:
    type: string
    default: "{track:02d} - {title}"
preconditions:
  - command_exists: ffmpeg
  - file_exists: "${flac_file}"
  - file_exists: "${cue_file}"
steps:
  - run: "mkdir -p ${output_dir}"
  - run: "ffmpeg -i ${flac_file} -f segment -segment_times ... ${output_dir}/... "
postconditions:
  - files_created_in: "${output_dir}"
  - min_files: 1
render:
  type: track_table
fallback: agent_or_manual
```

> **Безопасность:** аргументы экранируются (shell quoting) перед подстановкой. Рецепт не может содержать произвольный shell injection — только параметризованные шаги.

## Data Flow (MVP)

1. Пользователь вводит `split this flac/cue`.
2. Command Surface классифицирует: похоже на известный рецепт.
3. Trust Engine проверяет confidence, аргументы, риск.
4. Если доверие > порога → рецепт исполняется через Execution Layer.
5. Результат уходит в Renderer → пользователь видит таблицу.
6. Результат уходит в Log.
7. Если рецепт неизвестен или упал → Agent Layer планирует команды.
8. После N успешных выполнений Behavior Compiler предлагает рецепт.

## Stack (MVP)

- **Язык:** Rust (CLI-first).
- **Shell execution:** `std::process::Command` / `duct` crate.
- **Renderer:** HTML-шаблоны → открываются в браузере или встроенном webview.
- **Log:** JSONL-файл.
- **Recipe:** YAML (serde + yaml-rust).
- **Agent:** API к LLM (провайдер настраивается).

## Key Design Rule

Пользователь — командир, а не оператор множества несвязанных программ. Интерфейс спрашивает результат, показывает что произошло, и превращает повторяющийся успех в дешёвое исполнение.
