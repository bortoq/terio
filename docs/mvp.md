# MVP — Minimum Viable Product

## Цель

Доказать, что terio может:
1. Исполнить shell-команду.
2. Отрендерить результат как структурированный веб-блок.
3. Распознать повторяемый workflow и превратить его в доверенный локальный рецепт.

## ICP (Ideal Customer Profile)

**Power user командной строки.** Разработчик, DevOps, data/media hoarder, который:
- уже работает в терминале ежедневно;
- повторяет одни и те же ручные последовательности команд;
- использует или пробовал AI-агенты для кода/терминала;
- готов попробовать новый инструмент, если он сокращает переключения.

## Что входит в MVP

### 1. Shell execution
- `terio run <command>` — запуск произвольной shell-команды в CWD.
- Захват stdout, stderr, exit code, duration.
- Stream вывода в реальном времени.
- `terio rerun` — повтор последней команды.

### 2. Рендеринг вывода
- Plain text renderer (fallback для всего).
- Table renderer для: `ls -l`, csv-подобного вывода, ffmpeg progress.
- Card renderer для статуса и предупреждений.

### 3. Behavior Log
- JSONL-файл в `~/.terio/log/`.
- Каждая запись: timestamp, request, command, args, stdout summary, exit code, duration, risk level, error.
- **Секреты не пишутся** — редэкция до записи.

### 4. Один рецепт (FLAC/CUE split)
- Формат: YAML.
- Аргументы: flac_file, cue_file, output_dir, naming_template.
- Прекондишены: проверка наличия ffmpeg, существования файлов.
- Шаги: 2–3 shell-команды с подстановкой аргументов.
- Посткондишены: проверка, что файлы созданы.
- Risk: `local_write`.

### 5. Trust Engine (MVP)
- Порог confidence: 3 успешных выполнения перед предложением рецепта.
- Ручное подтверждение перед первым реплеем.
- После 5 успешных реплеев — auto-confirm для `local_write`.
- Expandable trace: пользователь видит команды рецепта.

### 6. Fallback
- Если рецепт не найден → сообщение "Recipe not found".
- Если рецепт упал → показать ошибку, exit code, упавший шаг.
- В MVP fallback к агенту НЕ входит (агент — Фаза 4).

## Что НЕ входит в MVP

- ❌ GitHub connector.
- ❌ Media controller / плейлисты.
- ❌ Missing episode downloader.
- ❌ Modal workspace / редактор.
- ❌ Shared sessions.
- ❌ Marketplace.
- ❌ Team features.
- ❌ Cloud sync.
- ❌ Agent / LLM integration.
- ❌ Mobile / web remote.
- ❌ GUI / desktop app (CLI only).

## Формат Behavior Log (MV0)

```jsonl
{"ts":"2026-06-23T10:00:00Z","request":"ls -l","command":"ls -l","exit":0,"duration_ms":12,"risk":"read_only","stdout_summary":"12 entries, 3 dirs","cwd":"/home/user"}
{"ts":"2026-06-23T10:01:00Z","request":"split flac/cue","command":"ffmpeg -i album.flac ...","exit":0,"duration_ms":4500,"risk":"local_write","stdout_summary":"12 tracks extracted","cwd":"/home/user/music","recipe_id":"split_flac_cue_v1"}
```

## Формат Rendered Block (MVP)

```json
{
  "type": "table",
  "title": "Track Split Results",
  "status": "success",
  "headers": ["#", "Title", "Duration", "File"],
  "rows": [
    ["01", "Intro", "3:45", "01 - Intro.flac"],
    ["02", "Main", "4:12", "02 - Main.flac"]
  ],
  "actions": [
    {"label": "Open folder", "command": "xdg-open /home/user/music/tracks"},
    {"label": "Play album", "command": "mpv /home/user/music/tracks"}
  ],
  "rawOutput": "stdout lines..."
}
```

## Критерии успеха MVP

1. `terio run ls -l` показывает красиво отформатированную таблицу файлов.
2. `terio log` показывает историю.
3. После трёх выполнений сплита FLAC/CUE terio предлагает сохранить рецепт.
4. Рецепт запускается с новыми файлами и даёт корректный результат.
5. Рецепт с невалидными аргументами не запускается (validation failure).
6. Рецепт с risk `local_write` требует подтверждения при первом запуске.
7. Все секреты редэктятся из лога.

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
    config.rs
  docs/
    mvp.md
    architecture.md
    trust-model.md
    behavior-log.md
  recipes/
    split_flac_cue.yaml
  terio_log/
    (пример лога)
```
