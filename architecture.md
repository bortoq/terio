# Architecture

## Core Metaphor

terio — это **панель управления** (control panel). Как пульт для телевизора, stereo-системы и приставки: он не заменяет эти устройства, но даёт единую точку контроля над ними.

terio контролирует программы, которые имеют отсоединяемый интерфейс:
- CLI-инструменты (git, ffmpeg, curl, rsync, mpv, npm, docker);
- программы с API (GitHub, медиасерверы, download-индексаторы);
- сервисы, доступные через shell (файловая система, базы данных, браузер через `open`/`xdg-open`).

terio **не становится** этими программами — он даёт интерфейс для управления ими.

## Компоненты

```
┌──────────────────────────────────────────────────────┐
│                 COMMAND SURFACE                       │
│   (терминальный ввод / естественный язык / горячие    │
│    клавиши / голос)                                   │
├──────────────────────────────────────────────────────┤
│                    CORE LAYER                         │
│                                                      │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐  │
│  │  Execution   │  │    Agent     │  │  Behavior   │  │
│  │    Layer     │  │    Layer     │  │  Compiler   │  │
│  │  (shell)     │  │  (LLM/planner)│  │  (recipes)  │  │
│  └─────────────┘  └──────────────┘  └─────────────┘  │
│                                                      │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐  │
│  │   Web       │  │  Behavior    │  │    Trust     │  │
│  │  Renderer   │  │     Log      │  │    Engine    │  │
│  └─────────────┘  └──────────────┘  └─────────────┘  │
│                                                      │
├──────────────────────────────────────────────────────┤
│                   STORAGE LAYER                       │
│   (preferences, log, recipes, trust scores, undo      │
│    snapshots, trash)                                  │
└──────────────────────────────────────────────────────┘
```

### 1. Command Surface
- Принимает: shell-команды (`terio run -- ls -l`), естественно-языковые запросы (`terio ask "split this flac"`), горячие клавиши.
- Классифицирует ввод: shell → Execution Layer; запрос → Agent Layer; известный рецепт → Behavior Compiler.
- **Инвариант:** каждый ввод классифицируется до исполнения.

### 2. Execution Layer
- Запускает shell-процессы в CWD пользователя.
- Модель: `spawn → stream → collect → return`.
- stdouterr стримятся в реальном времени.
- Захватывает: stdout, stderr, exit code, duration, argv.
- **Отмена:** SIGTERM → SIGKILL по таймауту; для сложных случаев — отменяющее задание (process group kill + cleanup).
- **Предусловие:** команда не исполняется, если не прошла validation (recipe) или confirmation (risk level).
- **Постусловие:** результат + метаданные → Renderer + Log.

### 3. Agent Layer (Built-in AI Model)
- Встроенная AI-модель (конфигурируемый LLM-провайдер: OpenAI, Anthropic, локальный).
- Получает запрос на естественном языке, планирует последовательность shell-команд.
- Извлекает аргументы из контекста (текущая директория, файлы, история).
- Объясняет рискованные действия перед исполнением.
- **Инвариант:** агент не вызывается, если сработал доверенный рецепт (экономия LLM).
- **Инвариант:** если рецепт упал, агент подхватывает с контекстом ошибки (fallback).

### 4. Behavior Compiler
- **Pipeline:** Capture → Normalize → Cluster → Propose → Validate → Execute → Observe → Score → Fallback.
- **Вход:** Behavior Log — успешные повторяющиеся последовательности.
- **Выход:** Recipe (YAML) с аргументами, шагами, прекондишенами, риск-уровнем.
- Кеширует **поведение**, а не ответы LLM.
- Не предлагает рецепт, пока нет N успешных выполнений (default: 3, настраивается).

### 5. Web Renderer
- Получает: `ExecutionResult { stdout, stderr, exit_code, duration, risk, command }`.
- Выбирает renderer по типу вывода: `table`, `timeline`, `card`, `gallery`, `progress`, `log`, `readable_page`, `plain_text`.
- Формат блока:
  ```json
  {
    "schema_version": 1,
    "block_id": "01J...",
    "run_id": "01J...",
    "type": "table",
    "title": "Track Split Results",
    "status": "success",
    "columns": [{"key":"track","label":"#"}],
    "rows": [{"track":"01","title":"Intro","duration":"3:45","file":"01 - Intro.flac"}],
    "actions": [
      {"label":"Open folder","kind":"command","risk":"local_write","command":"xdg-open /home/user/tracks"}
    ]
  }
  ```
- **Правило:** plain text остаётся валидным, когда он лучше структуры.

### 6. Behavior Log
- Формат: JSONL (v1 — одна запись на команду или рецепт-шаг).
- Поля: `schema_version`, `run_id`, `session_id`, `ts`, `kind` (command|recipe_step|recipe_run), `request`, `command { display, argv }`, `cwd`, `risk`, `exit`, `duration_ms`, `stdout_summary`, `stderr_summary`, `artifacts` (paths).
- Хранится в `~/.terio/log/`.
- **Секреты редэктятся** из ВСЕХ полей перед записью.
- Ротация: по месяцу или 50MB.

### 7. Trust Engine
- Confidence score: `+0.2` за успех, `-0.3` за неудачу (диапазон 0.0–1.0).
- Порог предложения рецепта: 3 успешных выполнения.
- Порог авто-запуска: `0.8` для `local_write`, `0.95` для `network_read`.
- Политики: `always_ask` / `ask_once` / `allow_in_dir` / `allow_for_recipe` / `never_allow`.
- **Undo safety:** тривиально обратимые действия (создание файлов, запись) могут выполняться без запроса, т.к. terio хранит undo-снапшоты и корзину (trash).

### 8. Undo/Redo Layer
- Все изменения (создание, запись, перемещение, удаление) логируются для undo.
- Удалённые файлы перемещаются в `~/.terio/trash/<run_id>/` вместо `rm`.
- `terio undo` откатывает последнее изменение.
- Ограничение по дисковому использованию: `TERIO_TRASH_SIZE_MB=1024`.

## Risk Taxonomy

| Risk Level | Примеры | Default Policy |
|------------|---------|----------------|
| `read_only` | `ls`, `cat`, `git status` | Auto |
| `local_write` | `mkdir`, `cp`, `ffmpeg` | Auto (undo available) |
| `destructive` | `rm -rf`, `mv --overwrite` | Confirm, undo via trash |
| `network_read` | `curl url`, `git fetch` | Auto (recipe: ask_once) |
| `network_write` | `git push`, `curl -X POST` | Always confirm |
| `credential_access` | токены, ключи, .env | Always confirm, не логировать |
| `financial` | покупки, API с billing | Always confirm |

## Data Flow

1. Пользователь вводит: `terio ask "split this flac/cue album"`.
2. Command Surface классифицирует: естественный язык → Agent Layer.
3. Agent Layer проверяет готовый рецепт (через Behavior Compiler).
4. Если рецепт найден и confidence > порога → Trust Engine → Execution Layer.
5. Если рецепта нет или confidence низкий → Agent Layer планирует команды.
6. Execution Layer выполняет, возвращает результат.
7. Renderer показывает блок. Log записывает.
8. Если рецепта не было, но последовательность повторяется 3+ раз → Behavior Compiler предлагает рецепт.

## Recipe Format (MVP v1)

```yaml
schema_version: 1
id: split_flac_cue_v1
name: Split FLAC/CUE album
risk: local_write
shell_allowed: false
arguments:
  flac_file:
    type: file
    required: true
    extensions: ["flac", "FLAC"]
  cue_file:
    type: file
    required: true
    extensions: ["cue", "CUE"]
  output_dir:
    type: directory
    required: false
    default: "./tracks"
steps:
  - id: create_output_dir
    command: mkdir
    argv: ["-p", "${output_dir}"]
  - id: split_tracks
    command: ffmpeg
    argv:
      - "-i"
      - "${flac_file}"
      - "-i"
      - "${cue_file}"
      - "-map"
      - "0:0"
      - "-c"
      - "copy"
      - "-f"
      - "segment"
      - "-segment_times"
      - "${segment_times}"
      - "${output_dir}/track_%02d.flac"
preconditions:
  - binary_exists: ffmpeg
  - file_exists: "${flac_file}"
  - file_exists: "${cue_file}"
postconditions:
  - files_created_in: "${output_dir}"
  - min_files: 1
on_failure:
  cleanup_created_files: true
render:
  type: track_table
fallback: agent  # или manual — настраивается
```

> **Безопасность:** `shell_allowed: false` — шаги используют structured argv, не shell-строки. Аргументы экранируются для shell. Command injection через имена файлов невозможен.

## Stack

- **Язык:** Rust.
- **CLI:** clap или bpaf.
- **Shell execution:** duct или std::process::Command.
- **Renderer:** HTML-шаблоны (терминальный webview или браузер).
- **Log:** serde_json + JSONL.
- **Recipe:** serde_yaml.
- **Agent:** HTTP-клиент к LLM API (OpenAI, Anthropic, локальный).

## Key Design Rule

Пользователь — командир за панелью управления. Интерфейс — единый пульт. Программы — исполнительные механизмы. terio не заменяет их, а даёт контроль над ними.
