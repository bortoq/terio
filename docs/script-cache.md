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
| `scope` | object | Область применения скрипта (см. ниже) |
| `parameters` | object | Параметры: { name: { source, pattern/default, required } } |
| `preconditions` | array | Условия перед выполнением |
| `steps` | array | Команды: { command, argv, risk } |
| `artifacts` | array | Созданные файлы: { path_glob, kind } |
| `success_count` | int | Сколько раз успешно выполнен |
| `trust_threshold` | int | После скольких успехов auto-run |
| `created_at` | ISO8601 | Когда создан |
| `last_used_at` | ISO8601 | Когда последний раз выполнен |

## Scope

```json
"scope": {
  "cwd_policy": "same_cwd_only|any_cwd_with_parameters",
  "cwd": "/home/user/music"
}
```

- `cwd_policy: same_cwd_only` — скрипт работает только в том же CWD, в котором был создан. Используется для Option B (фиксированный plan).
- `cwd_policy: any_cwd_with_parameters` — скрипт параметризован и может выполняться в любом CWD (при условии, что все glob_one дают ровно один файл).
- `cwd` — CWD, в котором скрипт был создан (для same_cwd_only — обязательный; для any_cwd_with_parameters — информационный).

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

### Валидация путей (path constraints)

Параметры с `source: default` и `value`, содержащим путь, проверяются:
- `output_dir` не должен выходить за пределы CWD (проверка на `../../`).
- `path_glob` в artifacts проверяется: результирующий путь должен быть внутри CWD или разрешённой директории.
- Симлинки разрешаются до проверки границ.

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
    "steps": [
      {"command": "ls", "argv": ["ls", "${flags}", "${dir}"], "risk": "read_only"}
    ],
    "artifacts": [ ... ]
  }
}
```

Шаги для кеша берутся из `cache_template.steps`. Если `cache_template.steps` не передан, terio использует `commands` как шаги (с понижением параметризации — фиксированные argv).

terio валидирует template и показывает пользователю перед сохранением.

### Option B: Фиксированный plan (запасной)

Если модель не вернула cache_template, terio сохраняет точный structured plan для данного CWD.
Cache entry работает только при exact match запроса И том же CWD (`scope.cwd_policy = "same_cwd_only"`). Параметризация не выполняется.

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
