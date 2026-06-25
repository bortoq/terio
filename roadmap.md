# Roadmap

Каждая фаза — список конкретных вещей, которые надо сделать.
Сделано → `[x]` → все `[x]` → фаза готова.

---

> **Важно:** этот roadmap описывает **новый target direction** после pivot `8acb1fa`.
> Текущий код содержит прежний baseline (ask/confirm/undo/integrations/multi-view Dioxus UI),
> который будет постепенно мигрирован к новой архитектуре.
> См. [architecture.md](architecture.md) (раздел Current) для текущего состояния
> и [docs/migration-to-window-model.md](docs/migration-to-window-model.md) для плана миграции.


## Текущее видение

**terio — интегратор интерфейсов.** Пользователь работает в одном окне-терминале.
terio под капотом использует LLM, кеш скриптов и песочницу, а результат отдаёт как окно — от текста до видео.

### Ключевые принципы

- **Терминальная парадигма**: взаимодействие с terio неотличимо от терминала.
  Единственное видимое отличие — результат может быть rich-окном (плеер, браузер, графика).
- **Окно = результат**: каждый ответ terio — окно. Тип окна определяет terio (текст, видео, подтверждение).
- **Никаких режимов просмотра**: Table/Timeline/Cards/Readable/Chat/Auto — удалены.
  Каждый тип сообщения сам знает, как себя отобразить.
- **Два фокуса**: FocusIn (ввод, всегда внизу) + FocusOut (вывод для скролла, переключается).
- **Scrollback**: окна не уничтожаются, а уходят в историю при overflow.
- **Скрипты — единственный способ управления**: help, config, focus, confirm, security — всё скрипты.
  Пользователь может менять логику terio под себя.
- **Проактивность**: terio предугадывает следующий запрос пользователя по контексту.
- **Экономия внимания**: по умолчанию — тихий режим (не отвлекает).
  Режим диалога — только когда пользователь явно включил отладку.
- **Безопасность через песочницу (CoW)**: untrusted-команды выполняются в copy-on-write-окружении.
  После первого успеха скрипт переезжает в trusted.

### Что убрали/упростили

- AI-агрегатор → интегратор интерфейсов
- Режимы просмотра (Table/Timeline/Cards/Readable/Chat/Auto) → окна
- Сложный risk management (trust policy, fuzzy/exact distinction) → песочница + скрипты безопасности
- ask (отдельный диалог) → окно-подтверждение в потоке
- Отдельные UI-элементы управления → скрипты

---

## Фаза 0. Ядро: терминал + окна

- [x] Переписать UI: чёрный экран, ввод внизу, вывод — окнами
- [x] Модель Window: id, content (Text | Rich), focusable
- [~] Два фокуса: FocusIn (всегда виден) + FocusOut (переключение) [x model + UI signal, ] CLI stub]
- [x] Scrollback: VecDeque<Window>, viewport с прокруткой
- [x] Убрать режимы (Table/Timeline/Cards/Readable/Chat/Auto)
- [~] Подтверждение риска — окно типа Confirm (y/N в потоке) [x baseline, ~ input routing works, ] полноценный поток
- [~] Режимы внимания: quiet / normal / debug [x config + CLI, ~ enforced in ask flow]
- [x] `terio help` — встроенная справка (CLI + UI input routing)
- [~] `terio focus ↑/↓` — переключение окна вывода [x UI routing, ~ persistent focus]
- [~] `terio scroll N` — скролл [x UI routing, x WindowManager.focus_move, ~ real scroll]
- [~] `terio repeat` — повторить последний запрос [x CLI side, ~ UI input routing]
- [x] Log → Window: восстановление окон из лога при запуске (WindowManager.from_log)
- [x] CI: cargo fmt + clippy + test (132 теста)

## Фаза 1. Песочница (CoW)

- [x] Copy-on-Write для untrusted-команд (на базе undo.rs / bubblewrap)
- [x] Snapshot до выполнения, rollback при ошибке/отмене
- [x] Untrusted → Trusted после 1 успеха (N=1 для read-only, N=3 для local_write)
- [x] Изоляция чтения: bwrap с пустым rootfs + bind mounts (strict mode)
- [x] Белые списки no_read_paths в конфиге
- [x] `terio sandbox status` — просмотр состояния песочницы

## Фаза 2. Скриптовая система

- [ ] Интерпретатор скриптов (ядро terio, Rust)
- [ ] Структура: `terio-scripts/core/` (встроенные) + `user/` + `learned/`
- [ ] Формат скрипта: triggers, steps, show
- [ ] Перенос help/config/focus/confirm в скрипты
- [ ] `terio script install`, `terio script list`
- [ ] Переопределение встроенных скриптов пользователем

## Фаза 3. Словарь синонимов

- [ ] Normalize запроса → HashMap<NormalizedQuery, ScriptId>
- [ ] Автоматическое создание синонимов из успешных LLM-запросов
- [ ] Редко используемые синонимы → удаление из индекса
- [ ] `terio alias list` / `terio alias remove`

## Фаза 4. Проактивный режим (предугадывание)

- [ ] terio смотрит историю и предлагает следующую команду
- [ ] Автодополнение: `# terio: ls /tmp? [Enter]`
- [ ] Молчаливое исполнение при точности > 0.95
- [ ] Окно-лог в углу: «terio: +3 команды»

## Фаза 5. Оптимизация стоимости

- [ ] Формула: C_total = C_llm_tokens + C_user_attention + C_risk
- [ ] Выбор маршрута: скрипт (дёшево) vs LLM (гибко)
- [ ] `terio cost` — отчёт по затратам
- [ ] Байесовский классификатор для точности предсказаний

## Фаза 6. Desktop + сообщество

- [ ] Локальная LLM (llama.cpp / Ollama)
- [ ] Desktop-пакет, автообновление
- [ ] Экспорт/импорт скриптов
- [ ] Реестр скриптов сообщества
