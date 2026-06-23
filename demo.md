# Demo

## Сценарий: повторяемый workflow становится дешёвым

### Путь A: через AI-агента

Пользователь открывает terio и вводит:

```
terio ask "split this flac/cue into tracks and rename them like last time"
```

У terio ещё нет рецепта для этой задачи, поэтому **встроенная AI-модель**:
1. Определяет FLAC и CUE файлы в текущей директории.
2. Генерирует команду `ffmpeg` с нужными аргументами.
3. Показывает, что будет выполнено:

```
 terio  I'll split album.flac using album.cue.
        Command: ffmpeg -i album.flac -i album.cue -map 0:0 -c copy -f segment ... -segment_times ... ./tracks/track_%02d.flac
        Risk: local_write (undo available)
        Proceed? [Y/n] (show trace)
```

4. После подтверждения выполняет.
5. Показывает прогресс и результат — таблицу треков.
6. Записывает в лог.

### Путь B: через shell напрямую

Пользователь, который знает команду, может ввести:

```
terio run -- ffmpeg -i album.flac -i album.cue -map 0:0 -c copy -f segment ... ./tracks/track_%02d.flac
```

terio исполняет, рендерит результат, логирует.

### Распознавание паттерна

После 3 успешных выполнений (через агента или вручную) с похожей структурой Behavior Compiler обнаруживает паттерн:

```
ffmpeg -i <flac> -i <cue> ... → split tracks
```

terio предлагает:

```
 terio  Pattern detected: "Split FLAC/CUE album"
  Run 1: album.flac → 12 tracks (success)
  Run 2: best_of.flac → 8 tracks (success)
  Run 3: live.flac → 6 tracks (success)

  Save as recipe? [Y/n]
```

### Реплей рецепта

На четвёртый раз пользователь вводит `terio ask "split this flac/cue"`. terio:

1. Находит готовый рецепт (confidence: 0.6).
2. Валидирует аргументы.
3. Показывает трейс и спрашивает подтверждение.
4. Исполняет **без LLM**.
5. Рендерит таблицу.

Если confidence превышает порог (0.8), рецепт исполняется без запроса.

### Отказ рецепта

Если файл не найден или ffmpeg не установлен:

```
 Recipe failed: precondition not met
   - ffmpeg: found ✓
   - album.flac: NOT FOUND ✗

  Falling back to agent.
```

Агент подхватывает, объясняет ошибку и предлагает альтернативу.

### Undo

После любого выполнения пользователь может ввести `terio undo`.

Если были созданы файлы — они удаляются. Если файлы были удалены — восстанавливаются из корзины.

### Цель демо

Показать: terio — это панель управления, которая:
- исполняет команды;
- красиво показывает результат;
- помнит, что вы делали;
- автоматизирует повторения;
- позволяет откатить изменения.
