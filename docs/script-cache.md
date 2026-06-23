# Script Cache Schema (v1)

## Назначение

Хранить успешные цепочки команд для повторного выполнения без модели.

## Формат

JSON.

## Поля

| Поле | Тип | Описание |
|------|-----|----------|
| `schema_version` | int | 1 |
| `script_id` | string | SHA-256 нормализованного запроса |
| `normalized_request` | string | Нормализованный текст запроса |
| `match_policy` | string | `exact_normalized` (MVP) |
| `risk` | string | Общий risk скрипта |
| `parameters` | object | Параметры: { name: { source, pattern/default, required } } |
| `preconditions` | array | Условия перед выполнением |
| `steps` | array | Команды: { command, argv, risk } |
| `artifacts` | array | Созданные файлы: { path_glob, kind } |
| `success_count` | int | Сколько раз успешно выполнен |
| `trust_threshold` | int | После скольких успехов auto-run |
| `created_at` | ISO8601 | Когда создан |
| `last_used_at` | ISO8601 | Когда последний раз выполнен |

## Параметры

```json
"parameters": {
  "flac_file": {
    "source": "glob",
    "pattern": "*.flac",
    "required": true
  },
  "output_dir": {
    "source": "default",
    "value": "./tracks"
  }
}
```

- `source: glob` — ищет файлы по pattern в CWD. Берётся первый.
- `source: default` — фиксированное значение.
- `required: true` — выполнение невозможно без этого параметра.

## Preconditions

```json
"preconditions": [
  {"binary_exists": "ffmpeg"},
  {"glob_one": "*.flac"},
  {"file_exists": "/path/to/file"}
]
```

- `binary_exists` — проверяет, что команда доступна в PATH.
- `glob_one` — в CWD есть хотя бы один файл по glob.
- `file_exists` — конкретный файл существует.

## Artifacts

```json
"artifacts": [
  {"path_glob": "./tracks/*.flac", "kind": "created_file"}
]
```

Для renderer, future undo, cleanup.

## Match Policy (MVP)

- Только `exact_normalized`: нормализованный запрос совпадает полностью.
- Fuzzy match — в будущем, с подтверждением, никогда auto-run.
