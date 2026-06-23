# Behavior Log Schema

## Формат

JSONL (JSON Lines) — одна запись на строку, каждая строка — валидный JSON.

## Версия

`schema_version: 1`

## Поля

| Поле | Тип | Обязательное | Описание |
|------|-----|-------------|----------|
| `schema_version` | int | да | Версия схемы (1) |
| `run_id` | string | да | UUID выполнения |
| `session_id` | string | да | UUID сессии (группа run_id) |
| `ts` | ISO8601 | да | Время выполнения |
| `kind` | string | да | `command` / `recipe_step` / `recipe_run` / `agent_turn` |
| `request` | string | да | Ввод пользователя |
| `cwd` | string | да | Working directory |
| `risk` | string | да | Risk level |
| `exit` | int | да | Exit code (0 = success) |
| `duration_ms` | int | да | Время выполнения в мс |
| `command` | object | да | `{ display: "ls -l", argv: ["ls", "-l"] }` |
| `stdout_summary` | string | нет | Первые 1024 символа stdout |
| `stderr_summary` | string | нет | Первые 4096 символов stderr |
| `parent_run_id` | string | нет | Для recipe_step — run_id рецепта |
| `recipe_id` | string | нет | ID рецепта (если рецепт) |
| `confidence_before` | float | нет | Confidence рецепта до |
| `confidence_after` | float | нет | Confidence рецепта после |
| `error` | string | нет | Описание ошибки |
| `artifacts` | object | нет | `{ stdout_path, stderr_path, block_path }` |
| `undo_available` | bool | нет | Можно ли откатить |

## Хранение артефактов

```
~/.terio/
  log/
    terio-2026-06.jsonl          # активный лог
    terio-2026-05.jsonl.gz       # сжатый после ротации
  runs/
    <run_id>/
      stdout.log                 # полный stdout
      stderr.log                 # полный stderr
      block.json                 # rendered block (если был)
  trash/
    <run_id>/                    # файлы, перемещённые в корзину
```

## Ротация

- Активный файл: `terio-YYYY-MM.jsonl`.
- Ротация: по достижении 50MB или первого дня следующего месяца.
- Старый файл сжимается gzip.

## Пример

```jsonl
{"schema_version":1,"run_id":"01J...","session_id":"01J...","ts":"2026-06-23T10:00:00Z","kind":"command","request":"ls -l","cwd":"/home/user","risk":"read_only","exit":0,"duration_ms":8,"command":{"display":"ls -l","argv":["ls","-l"]},"stdout_summary":"14 entries, 3 dirs","artifacts":{"stdout_path":"~/.terio/runs/01J.../stdout.log"}}
{"schema_version":1,"run_id":"01J...","session_id":"01J...","ts":"2026-06-23T10:05:00Z","kind":"recipe_run","request":"split album.flac","cwd":"/home/user/music","risk":"local_write","exit":0,"duration_ms":12300,"command":{"display":"ffmpeg -i album.flac ...","argv":["ffmpeg","-i","album.flac","..."]},"stdout_summary":"12 tracks extracted","recipe_id":"split_flac_cue_v1","confidence_before":0.6,"confidence_after":0.8,"undo_available":true}
{"schema_version":1,"run_id":"01J...","session_id":"01J...","ts":"2026-06-23T10:30:00Z","kind":"recipe_step","request":"split live","cwd":"/home/user/music","risk":"local_write","exit":1,"duration_ms":3200,"command":{"display":"ffmpeg -i live.flac ...","argv":["ffmpeg","-i","live.flac","..."]},"stderr_summary":"ffmpeg: No such file: live.cue","error":"CUE file not found","parent_run_id":"01J...","recipe_id":"split_flac_cue_v1","confidence_before":0.8,"confidence_after":0.5}
```
