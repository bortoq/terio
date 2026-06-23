# Demo

## Сценарий: terio учится на одном примере

### Первый раз

Пользователь вводит:

```
terio ask "split this flac/cue album into separate tracks"
```

1. Request Matcher ищет exact match в кеше. Пусто.
2. Запрос уходит к модели. Модель получает: CWD, список файлов (`album.flac`, `album.cue`).
3. Модель возвращает structured plan:

```
План (2 шага):
  1. mkdir -p ./tracks                 local_write
  2. ffmpeg -i album.flac -i album.cue ...  local_write
  Риск: local_write
  Выполнить? [Y/n]
```

4. Подтверждение → выполнение → таблица треков.
5. Цепочка сохраняется в Script Cache: `"split flac cue album into separate tracks" → {...}`.

### Второй раз (другой альбом)

Тот же запрос → exact match → скрипт найден.

terio:
- Заполняет параметры: `*.flac` → `best_of.flac`, `*.cue` → `best_of.cue`.
- Проверяет preconditions: ffmpeg есть, файлы есть.
- Выполняет **без модели**. Показывает таблицу.

### Auto-run

После 3 успехов скрипт получает `trusted`. При exact match + risk <= local_write → выполняется без запроса.

### Если ошибка

Скрипт не нашёл `live.cue`:

```
Script failed (run_id: ...): precondition *.cue not found
  Call model with error context? [Y/n]
```

Пользователь может вызвать модель для исправления.

## Что демонстрирует сценарий

1. **Агрегатор интерфейсов.** terio управляет ffmpeg, mkdir, файловой системой — из одной точки.
2. **Ленивое обучение.** Не требует настройки. Просто работаете.
3. **Кеш поведения.** Второй раз без модели.
4. **Безопасность.** План показывается. Destructive — подтверждение.
5. **Метрики.** `terio stats` покажет: model_calls, cache_hits, tokens_saved.
