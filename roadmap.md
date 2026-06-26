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
- [x] Два фокуса: FocusIn (всегда виден) + FocusOut (переключение) [x model + UI signal + CLI stub]
- [x] Scrollback: VecDeque<Window>, viewport с прокруткой
- [x] Убрать режимы (Table/Timeline/Cards/Readable/Chat/Auto)
- [x] Подтверждение риска — окно типа Confirm (y/N в потоке): baseline + input routing + полный поток
- [x] Режимы внимания: quiet / normal / debug [x config + CLI + enforcement в ask flow]
- [x] `terio help` — встроенная справка (CLI + UI input routing)
- [x] `terio focus ↑/↓` — переключение окна вывода [x UI routing + persistent focus (CLI stub + UI signal)]
- [x] `terio scroll N` — скролл [x UI routing + WindowManager.focus_move + scroll_offset signal]
- [x] `terio repeat` — повторить последний запрос [x CLI + UI input routing]
- [x] Log → Window: восстановление окон из лога при запуске (WindowManager.from_log)
- [x] CI: cargo fmt + clippy + test (189+ тестов)

## Фаза 1. Песочница (CoW)

- [x] Copy-on-Write — best-effort snapshot (undo.rs): file-copy snapshot с undo/redo
- [x] Snapshot до выполнения, rollback при ошибке/отмене (undo.rs)
- [x] Untrusted → Trusted после 1 успеха (N=1 read-only, N=3 local_write): config wired to cache trust_threshold
- [x] Изоляция чтения: bwrap strict mode (пустой rootfs + bind mounts + no network + no_read_paths tmpfs)
- [x] Белые списки no_read_paths в конфиге (6 default entries)
- [x] `terio sandbox status` — просмотр состояния песочницы
- [x] Fail closed: strict mode без bwrap → hard error

## Фаза 2. Скриптовая система

**Выбранный язык:** [Rhai](https://rhai.rs/) — Rust-native скриптовый язык.

**Почему Rhai, а не свой DSL / Lua / WASM:**
- Rust-native (`cargo add rhai`), без C-зависимостей (в отличие от Lua через `mlua`).
- Синтаксис близок к Rust — минимальный порог для аудитории terio.
- Безопасный sandbox из коробки: `Engine::new()` без файловой системы, лимиты на итерации/операторы.
- Нет GC — предсказуемая производительность.
- Прямые Rust-коллы: регистрируются функции напрямую, никакой сериализации.

**Declarative overlay (TOML):** 80% скриптов не требуют программирования.
```toml
triggers = ["list files", "ls"]
[step]
command = "ls"
args = ["-la"]
```
TOML-файл компилируется в Rhai-скрипт. Пользователь может начать с TOML
и перейти на Rhai, когда понадобится логика (условия, циклы, вызов terio API).

- [x] Интерпретатор: `rhai::Engine` + TOML → RhaiAST транслятор
- [x] Структура директорий: `~/.terio/scripts/{core,user,learned}/`
- [x] API для скриптов: `terio_execute()`, `terio_confirm()`, `terio_show()`, `terio_config_get/set()`
- [x] `terio_execute` safety: basic risk check + confirmation для Destructive/NetworkWrite
- [x] Builtin Rhai/TOML скрипты существуют
- [x] Input routing через ScriptEngine: help/mode/focus/scroll/repeat/ask → scripts + fallback
- [x] `terio script install <file>`, `terio script list`
- [x] Override: по id + по trigger priority (User > Core > Learned > Builtin)

## Фаза 3. Словарь синонимов

- [x] Normalize запроса → HashMap<NormalizedQuery, ScriptId> (baseline)
- [x] Автоматическое создание синонимов (baseline)
- [x] Редко используемые синонимы → prune (baseline)
- [x] `terio alias list/remove` CLI (baseline)
- [ ] Trust-aware synonym routing (Prefix/BagOfWords → confirmation для non-ReadOnly)

## Фаза 4. Проактивный режим (предугадывание)

- [x] terio смотрит историю и предлагает следующую команду (baseline)
- [x] Автодополнение: `# terio: <request>?` (baseline)
- [ ] Безопасное авто-исполнение (только read-only cached, exact trust)
- [x] Счётчик авто-выполненных команд (baseline)
- [x] Recursion guard + config-gated auto-exec (off by default)

## Фаза 5. Оптимизация стоимости

- [x] Формула: C_total = C_llm + C_attention + C_risk (baseline)
- [x] Выбор маршрута: скрипт vs LLM (baseline)
- [x] `terio cost` — отчёт по затратам (baseline)
- [x] Байесовский классификатор для предсказаний (baseline)
- [ ] Per-entry risk cost в cost report
- [ ] Реальный route optimizer (cost-aware)

## Фаза 6. Desktop + сообщество

- [ ] Локальная LLM (llama.cpp / Ollama)
- [ ] Desktop-пакет, автообновление
- [ ] Экспорт/импорт скриптов
- [ ] Реестр скриптов сообщества
