# Roadmap

**Правило прохождения фаз:** Core (backend, без system deps) и Shell (Dioxus UI, требует `feature desktop`) внутри одной фазы можно делать параллельно. Если Core готов, а Shell заблокирован отсутствием system deps — переходим к следующей фазе. Shell-пункты доделываются при первой возможности. Номер фазы обновляется, когда сделаны все её пункты (Core + Shell).

## 0. Определение (архитектура)

- [x] Сформулировать суть: агрегатор интерфейсов. Все программы с CLI/API — из одной точки.
- [x] Scope расширяется лениво — от пользовательских запросов.
- [x] Стек: Rust, Dioxus (webview), CLI-совместимость.
- [x] docs/mvp.md, docs/trust-model.md, docs/behavior-log.md, docs/agent-protocol.md.
- [x] LICENSE.
- [x] Экономическая модель: cost_counters в каждой записи лога.
- [x] JSON Schema: agent-output, script-cache, behavior-log.
- [x] LogWriter/LogReader traits — дизайн для смены формата.
- [x] identity: instance_id + session_id.
- [x] display_profile — типизация записей лога.
- [x] interaction_id — группировка пар.
- [x] Cargo.toml и базовая src/ структура.
- [x] CI: линтер + сборка.

## 1. Shell + log + scaffold

- [x] Cargo.toml, src/main.rs, src/cli.rs, src/ui/app.rs, src/run.rs.
- [x] `terio run -- <command>` — shell без модели.
- [x] Захват stdout, stderr, exit code, duration.
- [x] Identity: instance_id (ULID) + session_id (UUID v4).
- [x] LogWriter trait + JsonlLogWriter: append → validate → write.
- [x] LogReader trait + JsonlLogReader: recent(n), by_session(), by_interaction().
- [x] LogStore: writer + reader + broadcaster.
- [x] Accounting: cost_counters в каждой записи; aggregate; заглушка compute_attention_cost.
- [x] display_profile: required nested поля (type, renderer_hint, user_visible).
- [x] Dioxus scaffold: окно, поле ввода, область вывода (`feature desktop`).
- [x] Dioxus показывает лог (plain text) через `LogStore::recent(50)` при запуске.
- [x] `terio log --json` — история в JSON.
- [x] CI: cargo test + cargo build.

**Критерий:** `cargo run` открывает Dioxus-окно с логом. `terio run -- ls -l` → запись в лог. `terio log --json` показывает cost_counters и display_profile.

## 2. Mock agent + cache + redact + risk

- [x] `terio ask "list files"` — mock: 6 hardcoded запросов.
- [x] Script Cache: первый ask → сохранить chain (JSON в `~/.terio/cache/`).
- [x] Request Matcher: exact normalized match (lowercase+trim+collapse + SHA-256).
- [x] Повторный `terio ask "list files"` — cache hit, без mock.
- [x] `terio stats` — model_calls, cache_hits, cost_counters.
- [x] Redact: Bearer, api_key, token, SSH key, URL credentials.
- [x] Risk classifier: git clean/reset/push, curl -X POST, docker rm/rmi, cat .ssh и т.д.
- [x] Группировка по interaction_id в логе (поле + `by_interaction`).
- [x] Исправления аудита: не кешировать non-zero exit, success_count_before/after, scope в CacheEntry, mock только read-only, LogReader::stream() убран, stats на всех записях, warning для destructive при `terio run`.
- [ ] Table renderer в Dioxus (отображение команд и результатов в табличном виде).

**Критерий:** `terio ask "list files"` (первый) → mock, вывод. Повторный → cache hit, быстрее. `terio stats` показывает cache_hits > 0.

## 3. Реальный LLM provider

- [ ] Конфигурация провайдера (OpenAI, Anthropic, ollama) → `terio config set provider`.
- [ ] Agent возвращает structured plan (command + argv) от реальной модели.
- [ ] cache_template с steps от модели → terio сохраняет в Script Cache.
- [ ] План → подтверждение → выполнение (для risk >= local_write).
- [ ] Risk: destructive/network_write → всегда подтверждение.
- [ ] Redaction secrets до отправки в модель.
- [ ] `terio cancel` — прерывание выполнения.
- [ ] Поле ввода для `terio ask` + кнопка отправки (Shell).
- [ ] Отображение подтверждения (plan с risk) перед выполнением (Shell).
- [ ] Индикатор выполнения — spinner/progress (Shell).

**Критерий:** `terio ask "list files"` — реальная модель генерирует `ls -l`, terio показывает план, пользователь подтверждает, terio выполняет. Secrets не уходят в модель.

## 4. Trust + безопасность

- [ ] Policy: always_ask / ask_once / allow (через `terio config`).
- [ ] Auto-run: exact match + risk <= local_write + N успехов + scope соблюдён.
- [ ] Fuzzy match: никогда auto-run, только с подтверждением.
- [ ] Path boundary validation (защита от ../../).
- [ ] `terio config` — полное управление настройками.
- [ ] Настройки в UI — окно конфигурации (Shell).
- [ ] Индикатор trust level для каждой команды (Shell).

**Критерий:** destructive требует подтверждения. Fuzzy match не auto-run. Path traversal blocked.

## 5. Undo/Redo (Experimental)

- [ ] Sandbox (bubblewrap/overlay FS).
- [ ] Warn (только предупреждение).
- [ ] Best-effort snapshot для скриптов.
- [ ] `terio undo`, `terio redo`.
- [ ] Off by default.
- [ ] Кнопки Undo/Redo в UI (Shell).

**Критерий:** `terio undo` откатывает последний cached скрипт (off by default).

## 6. Продвинутый рендеринг + интерактивность

- [ ] Live-stream: LogStore broadcast подключается к Dioxus (вместо poll на `recent`).
- [ ] `terio stats` с разделением cost_counters по типам.
- [ ] Timeline (git log style) (Shell).
- [ ] Card (статусы, риски) (Shell).
- [ ] Progress (длинные операции) (Shell).
- [ ] Readable page (лог, новости) (Shell).
- [ ] Авто-выбор renderer на основе display_profile (Shell).
- [ ] Блок → Window эволюция (каждый блок — будущее окно) (Shell).
- [ ] Чат-окно: последовательность встроенных окон (Shell).

**Критерий:** `git log` — timeline. `terio log` показывает пары (interaction_id). Окно обновляется в реальном времени.

## 7. Интеграции (ленивые) + шэринг

- [ ] Каждая новая программа — через запрос пользователя.
- [ ] terio учится работать с Git, GitHub, медиа, Docker и т.д.
- [ ] Никаких заранее написанных коннекторов.
- [ ] Автоматическая интеграция: агент идентифицирует программу, читает --help/man/wiki, пишет integration script, прогоняет тесты.
- [ ] Интеграционный скрипт сохраняется в Script Cache.
- [ ] Окно интеграции: выбор программы, статус изучения (Shell).
- [ ] Шэринг: копирование окон между экземплярами terio (через instance_id) (Shell).
- [ ] `terio share`, `terio receive` (Shell).

**Критерий:** первый запуск `git log` → агент изучает git, пишет скрипт, terio сохраняет.

## 8. Оптимизация стоимости + предсказание

- [ ] Раздельные счётчики cost_counters в единой метрике total_attention_cost (реальные веса).
- [ ] cache vs model: terio выбирает маршрут с минимальной total_attention_cost.
- [ ] История стоимости: `terio cost` — отчёт по затратам.
- [ ] Auto-tuning: terio предлагает выключить auto-run для дорогих скриптов.
- [ ] Pre-execution: отдельный режим, terio предсказывает запрос до нажатия Enter, выполняет read_only шаги, показывает preview.
- [ ] Графики стоимости в UI (Shell).
- [ ] Предпросмотр (preview) в окне (Shell).

**Критерий:** `terio cost` — отчёт. terio выбирает cache вместо модели, если дешевле.

## 9. Desktop + сообщество + локальная LLM

- [ ] Локальная LLM: open-source модель с открытыми весами, обучаемая на базе скриптов пользователя.
- [ ] Desktop — standalone-пакет, system tray, автообновление (Shell).
- [ ] Экспорт/импорт скриптов (Shell).
- [ ] Документ = мультиокно: объединение окон в документ, экспорт как документация (Shell).
- [ ] Реестр скриптов (Shell).

**Критерий:** terio — standalone-приложение в system tray. Окна объединяются в документы.
