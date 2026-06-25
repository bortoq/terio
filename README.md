# terio

**Интегратор интерфейсов.** Единое окно-терминал для работы с программами, LLM и скриптами.

terio выглядит как терминал: чёрный экран, ввод внизу, вывод наверх.
Отличие — результат может быть любым окном: текст, видео, плеер, браузер.
terio сам выбирает тип окна и передаёт ему фокус.

---

## Current implementation

Прототип terio уже содержит рабочий baseline:

- `terio ask "<request>"` — запрос на естественном языке через LLM (OpenAI / Ollama / Mock)
- `terio run -- <command>` — прямое выполнение shell-команды
- `terio confirm` — подтверждение сохранённого плана
- `terio cancel` — отмена текущей операции
- `terio undo / redo` — откат/повтор snapshot-backed операций (experimental)
- `terio log --json` — просмотр лога
- `terio stats` — метрики и cost_counters
- `terio config show / set` — управление настройками
- `terio learn <program>` / `terio integrations` / `terio forget` — интеграции
- `terio share / receive` — шэринг окон между экземплярами
- Dioxus desktop UI — multi-view workspace (Table, Timeline, Cards, Chat, Auto)
- JSONL-лог, Script Cache, Trust layer, Redaction

**Подробнее:** [docs/current-status.md](docs/current-status.md)

---

## Target direction

Новый vision — **терминальная парадигма без режимов просмотра**:

- каждый результат = окно (текст, плеер, видео, iframe)
- два фокуса: FocusIn (ввод, всегда внизу) + FocusOut (вывод, переключаемый)
- скрипты — единственный способ управления (help, config, focus, confirm, security)
- CoW-песочница для untrusted-команд
- проактивный режим: terio предугадывает следующий запрос
- экономия внимания: quiet mode по умолчанию, debug mode для отладки

**Текущий код будет постепенно мигрирован к этой архитектуре.**
См. [architecture.md](architecture.md) (раздел Target) и [docs/migration-to-window-model.md](docs/migration-to-window-model.md).

---

## Быстрый старт (реальные команды)

```bash
terio ask "list files"              # запрос через LLM
terio run -- ls -la                  # shell-команда
terio confirm                        # подтвердить план
terio log --json                     # лог в JSON
terio stats                          # метрики
terio config show                    # настройки
terio undo                           # откат (experimental)
terio learn docker                   # обучить интеграцию
terio share                          # экспорт окна
```

### Будущие команды (planned)

```bash
terio mode quiet|normal|debug       # переключение режима внимания
terio focus ↑|↓                     # переключение окна вывода
terio scroll <lines>                # скролл
terio repeat                        # повторить последний запрос
terio cost                          # отчёт по затратам
```

---

## Ключевые возможности (target)

- **Терминальная парадигма** — взаимодействие неотличимо от терминала
- **Rich-окна** — результат может быть плеером, браузером, графикой
- **Песочница (CoW)** — *target:* untrusted-команды в изолированном окружении; *current:* experimental snapshot/undo + bubblewrap wrapper
- **Скриптовая система** — *target:* всё управление через скрипты
- **Проактивный режим** — *target:* terio предугадывает следующий запрос
- **Экономия внимания** — *target:* quiet mode по умолчанию

---

## Документы

- [Архитектура: current + target](architecture.md)
- [Current status](docs/current-status.md)
- [Migration plan](docs/migration-to-window-model.md)
- [Roadmap (target)](roadmap.md)
- [TODO / отложенные фичи](docs/todo.md)
