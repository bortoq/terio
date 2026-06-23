# Behavior Log Schema

## Формат

JSONL (JSON Lines) — одна запись на строку.

## Поля

| Поле | Тип | Обязательное | Описание |
|------|-----|-------------|----------|
| `ts` | ISO8601 | да | Время выполнения |
| `request` | string | да | Ввод пользователя |
| `command` | string | да | Фактическая shell-команда |
| `exit` | int | да | Exit code (0 = success) |
| `duration_ms` | int | да | Время выполнения в мс |
| `risk` | string | да | Risk level (см. trust-model.md) |
| `cwd` | string | да | Working directory |
| `stdout_summary` | string | нет | Краткое содержание stdout (первые N символов) |
| `stderr` | string | нет | stderr (если был) |
| `recipe_id` | string | нет | ID рецепта, если выполнялся рецепт |
| `error` | string | нет | Описание ошибки, если exit != 0 |
| `confidence_before` | float | нет | Confidence рецепта до выполнения |
| `confidence_after` | float | нет | Confidence рецепта после выполнения |

## Правила

1. **Секреты не пишутся.** Поля, содержащие паттерны `token=`, `Authorization:`, `password=`, `secret=`, `api_key=` редэктятся до записи.
2. **stdout_summary** — не более 1024 символов. Полный stdout хранится отдельно (опционально).
3. **stderr** — не более 4096 символов.
4. **recipe_id** заполняется только если команда была частью рецепта.

## Пример

```jsonl
{"ts":"2026-06-23T10:00:00Z","request":"ls -l","command":"ls -l","exit":0,"duration_ms":8,"risk":"read_only","stdout_summary":"14 entries, 3 dirs","cwd":"/home/user","recipe_id":null}
{"ts":"2026-06-23T10:01:00Z","request":"split album.flac","command":"ffmpeg -i /home/user/album.flac ...","exit":0,"duration_ms":12300,"risk":"local_write","stdout_summary":"12 tracks extracted","cwd":"/home/user/music","recipe_id":"split_flac_cue_v1","confidence_before":0.6,"confidence_after":0.8}
{"ts":"2026-06-23T10:30:00Z","request":"split live","command":"ffmpeg -i /home/user/live.flac ...","exit":1,"duration_ms":3200,"risk":"local_write","stderr":"ffmpeg: No such file: live.cue","error":"CUE file not found","cwd":"/home/user/music","recipe_id":"split_flac_cue_v1","confidence_before":0.8,"confidence_after":0.5}
```

## Хранение

- Директория: `~/.terio/log/`
- Файл: `terio-2026-06.jsonl` (одна ротация в месяц, или по размеру > 50MB).
- Старые логи: `terio-2026-05.jsonl.gz` (сжатие после ротации).
