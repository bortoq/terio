# Script Cache Schema (v1)

## Назначение

Хранить успешные цепочки команд для повторного выполнения без модели.

## Формат

JSON.

## Поля

| Поле | Тип | Описание |
|------|-----|----------|
| `schema_version` | int | 1 |
| `script_id` | string | ULID или SHA-256 содержимого (уникальный, меняется при изменении) |
| `request_hash` | string | SHA-256 нормализованного запроса (для поиска) |
| `version` | int | Инкремент при изменении скрипта для того же запроса |
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
    "source": "glob_one",
    "pattern": "*.flac",
    "required": true
  },
  "output_dir": {
    "source": "default",
    "value": "./tracks"
  }
}
```

- `source: glob_one` — ищет файлы по pattern в CWD. Требуется ровно один файл. Если найдено 0 или >1 — terio спрашивает пользователя.
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

## Artifacts

```json
"artifacts": [
  {"path_glob": "./tracks/*.flac", "kind": "created_file"}
]
```

## Generation Pipeline (MVP)

После успешного выполнения через Agent Layer cache entry создаётся так:

### Option A: Agent возвращает cache_template (рекомендуется)

Модель возвращает не только plan, но и template для кеша:

```json
{
  "summary": "...",
  "risk": "local_write",
  "commands": [...],
  "cache_template": {
    "parameters": { ... },
    "preconditions": [ ... ],
    "artifacts": [ ... ]
  }
}
```

terio валидирует template и показывает пользователю перед сохранением.

### Option B: Фиксированный plan (запасной)

Если модель не вернула cache_template, terio сохраняет точный structured plan для данного CWD.
Cache entry работает только при exact match запроса И том же CWD. Параметризация не выполняется.

### Выбор для MVP

MVP использует Option A как основной. Если модель не поддерживает — Option B.

## Match Policy (MVP)

- Только `exact_normalized`: нормализованный запрос совпадает полностью.
- Fuzzy match — в будущем, с подтверждением пользователя, никогда auto-run.

## Versioning

- `request_hash` — хеш нормализованного запроса. Используется для поиска.
- `script_id` — ULID или SHA-256 содержимого. Меняется при изменении скрипта.
- `version` — инкремент для того же request_hash.
- Это позволяет различать разные версии скрипта для одного запроса.
