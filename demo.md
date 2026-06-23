# Demo

## Сценарий: повторяемый медиа-workflow становится дешёвым

### Первый запуск

В директории пользователя есть FLAC-файл и CUE-файл:

```
/home/user/music/
  album.flac
  album.cue
```

Пользователь вводит:

```
terio ask "split this flac/cue album into tracks"
```

Поскольку рецепта ещё нет, terio передаёт запрос агенту. Агент генерирует команду:

```
ffmpeg -i album.flac -i album.cue -map 0:0 -c copy -f segment ...
```

terio:
- исполняет команду;
- показывает прогресс в реальном времени;
- после завершения рендерит таблицу треков;
- записывает лог: request, команда, результат, exit code.

### Распознавание паттерна

После 3 успешных выполнений (с разными файлами) Behavior Compiler обнаруживает стабильный паттерн:

```
ffmpeg -i <flac> -i <cue> ... → split tracks
```

terio предлагает пользователю сохранить рецепт:

```
 terio  Pattern detected: "Split FLAC/CUE album"
  Run 1: album.flac → 12 tracks (success)
  Run 2: best_of.flac → 8 tracks (success)
  Run 3: live.flac → 6 tracks (success)

  Save as recipe? [Y/n] (show trace)
```

### Реплей рецепта

На четвёртый раз пользователь вводит ту же просьбу. terio:

1. Находит готовый рецепт.
2. Валидирует аргументы (файлы существуют, ffmpeg установлен).
3. Показывает трейс команд.
4. Спрашивает подтверждение (risk: local_write).
5. Исполняет без LLM.
6. Рендерит таблицу.

### Отказ рецепта

Если файл не найден или ffmpeg не установлен, terio показывает:

```
 Recipe failed: precondition not met
   - ffmpeg: found ✓
   - album.flac: NOT FOUND ✗

  Falling back to manual mode.
```

Пользователь может исправить путь и запустить `terio rerun`.

### Цель демо

Первый рабочий прототип доказывает: обычный терминальный workflow превращается в доверенный рецепт, который выполняется быстрее, стоит дешевле и показывает результат понятнее, чем сырой terminal.

### Что НЕ в демо

- ❌ Встроенный GitHub UI.
- ❌ Медиаплеер внутри terio.
- ❌ Редактор файлов.
- ❌ Шэринг сессий.

Только shell → render → behavior cache.
