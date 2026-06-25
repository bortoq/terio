# Migration plan: multi-view Dioxus → terminal-like window model

## Цель

Перейти от текущего multi-view workspace (Table/Timeline/Cards/Chat/Auto) к новой терминальной парадигме:
чёрный экран, ввод внизу, результат = окно, два фокуса, scrollback.

## Этапы

### Этап 0: Документация (текущий коммит)

- [x] README: Current vs Target
- [x] architecture.md: current + target + migration plan
- [x] roadmap.md: pivot note
- [x] docs/current-status.md
- [x] docs/migration-to-window-model.md

### Этап 1: Window model (замена multi-view)

**Что делаем:**
1. Добавляем `Window` struct и `WindowManager` (VecDeque<Window> + FocusIn + FocusOut)
2. Создаём `WindowKind` enum: Text | Confirm (+ позже Rich)
3. Заменяем текущие renderer modes (Table/Timeline/Cards/Chat/Auto) на `WindowKind::Text`
4. InputSurface: строка ввода внизу (как в терминале)
5. Confirm из отдельного диалога → окно `WindowKind::Confirm`
6. Log → Window: восстановление окон из лога при запуске

**Что удаляем:**
- Переключатель режимов из UI
- `EntryRenderer` enum
- `WorkspaceView` enum

**Что сохраняем:**
- LogStore (без изменений)
- существующие renderer functions как внутренние форматеры для Window::content

**Кодовая сложность:** ~400 новых строк, ~200 удалённых

### Этап 2: Attention modes

**Что делаем:**
1. Три режима: quiet / normal / debug (поле в конфиге)
2. quiet: никаких подтверждений, всё автоматически
3. normal: подтверждение untrusted (1 раз за сессию на скрипт)
4. debug: каждый шаг — окно-подтверждение
5. `terio mode` как скрипт (позже) или хардкод (сейчас)

**Что удаляем:**
- Отдельный `terio confirm` диалог (заменяется на окно в потоке)

### Этап 3: Sandbox (CoW)

**Что делаем:**
1. Используем существующий `undo.rs` и `bubblewrap` как базис
2. CoW: snapshot файлов до untrusted-команды
3. Rollback при ошибке / отмене
4. Продвижение в trusted:
   - read-only: 1 успех → trusted
   - local_write: N успехов (N из конфига, default 3)
   - network/destructive: никогда auto-trust

**Кодовая сложность:** ~200 строк (дополнение к undo.rs)

### Этап 4: Script engine

**Язык: [Rhai](https://rhai.rs/) + TOML overlay.**

- **Rhai** — Rust-native скриптовый язык (синтаксис близок к Rust, без GC, sandbox из коробки)
- **TOML** — декларативный формат для 80% скриптов: `triggers + steps + show`
- TOML → RhaiAST транслятор в ядре terio

**Что делаем:**
1. `rhai::Engine` + TOML-парсер в ядре terio
2. API: `terio::execute()`, `terio::confirm()`, `terio::show()`, `terio::config_get/set()`
3. Структура директорий: `terio-scripts/core/`, `user/`, `learned/`
4. Перенос help/config/focus/confirm в скрипты
5. synonym dictionary на базе `matcher.rs`

**Почему не свой DSL:**
- Свой DSL = парсер + AST + интерпретатор + документация + IDE = месяцы работы
- Rhai — готовый язык, одна зависимость (`cargo add rhai`)
- Пользователи не знают DSL; Rhai знаком Rust-сообществу

**Кодовая сложность:** ~400 строк (ядро) + ~200 (миграция команд)

### Этап 5: Проактивность + cost optimizer

**Что делаем:**
1. Предугадывание следующей команды по логу
2. Формула C_total = C_llm + C_attention + C_risk
3. Выбор маршрута: скрипт vs LLM

**Кодовая сложность:** ~300 строк

---

## Что НЕ меняется

- LogStore, LogWriter/LogReader traits
- Identity (instance_id, session_id)
- Accounting (cost_counters)
- Redaction
- Provider trait (OpenAI, Ollama, Mock)

---

## Риски

| Риск | Вероятность | Смягчение |
|------|------------|-----------|
| Потеря существующей UI-функциональности | Low | Окна восстанавливаются через WindowKind |
| Поломка тестов при удалении renderer modes | Medium | Тесты переписать на Window model |
| Script engine затянет MVP | High | Скрипты — фаза 4; до неё — хардкод команд |
| Пользователи привыкли к multi-view | Low | MVP-фаза, пользователей пока нет |

## Текущий прогресс

```
Этап 0: ████████░░ 80% (документация)
Этап 1: ██████░░░░ 60%
Этап 2: ████░░░░░░ 40%
Этап 3: ████████░░ 80%
Этап 4: ░░░░░░░░░░ 0%
Этап 5: ░░░░░░░░░░ 0%
```
