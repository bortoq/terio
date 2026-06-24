# Roadmap

## 0. Определение

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
- [ ] Cargo.toml и базовая src/ структура.
- [ ] CI: линтер + сборка.

## 1. Минимальный UI + shell + лог

**Принцип:** terio — оконное приложение с первого коммита. Dioxus webview — основной UI. CLI (`terio run --`) — дополнительный интерфейс для автоматизации и отладки.

- [ ] Cargo.toml, src/main.rs, src/cli.rs, src/ui/app.rs, src/run.rs.
- [ ] Dioxus webview: окно с полем ввода и областью вывода.
- [ ] `terio run -- <command>` — shell без модели.
- [ ] Захват stdout, stderr, exit code, duration.
- [ ] **Identity:** instance_id генерируется при первом запуске; session_id на каждый запуск.
- [ ] **LogWriter trait + JsonlLogWriter:** append (validate→redact→write→broadcast).
- [ ] **LogReader trait + JsonlLogReader:** stream(), recent(n).
- [ ] **LogStore:** writer + reader + broadcaster.
- [ ] **Accounting:** cost_counters required+nested в каждой записи; aggregate; заглушка compute_attention_cost.
- [ ] **display_profile:** required nested поля (type, renderer_hint, user_visible).
- [ ] Renderer подписан на LogEventStream. Dioxus показывает лог (plain text).
- [ ] `terio log --json` — история в JSON.
- [ ] CI: cargo test + cargo build.

**Критерий:** `cargo run` открывает Dioxus-окно. `terio run -- ls -l` → запись в логе, отображается в окне. `terio log --json` показывает cost_counters и display_profile.

## 2. Mock agent + кеш (без реальной модели)

- [ ] `terio ask "list files"` — mock: если request совпадает, вернуть `ls -l`.
- [ ] Script Cache: первый ask → сохранить chain.
- [ ] Request Matcher: exact normalized match.
- [ ] Повторный `terio ask "list files"` — cache hit, без mock.
- [ ] `terio stats` — model_calls, cache_hits, cost_counters.
- [ ] Table renderer в Dioxus.
- [ ] Группировка по interaction_id в логе.

**Критерий:** `terio ask "list files"` (первый) → mock, таблица. Повторный → cache hit, быстрее. `terio stats` показывает cache_hits > 0.

## 3. Реальный LLM provider

- [ ] Конфигурация провайдера (OpenAI, Anthropic, ollama).
- [ ] Agent возвращает structured plan (command + argv).
- [ ] cache_template с steps от модели → terio сохраняет.
- [ ] План → подтверждение → выполнение.
- [ ] Script Cache: scope.cwd_policy.
- [ ] Risk: destructive/network_write → всегда подтверждение.
- [ ] Redaction secrets до отправки в модель.
- [ ] `terio cancel`.

**Критерий:** `terio ask "list files"` — реальная модель генерирует `ls -l`, terio показывает таблицу. Secrets не уходят в модель.

## 4. Trust + безопасность

- [ ] Risk taxonomy во всех компонентах.
- [ ] Redaction до модели и до лога.
- [ ] Policy: always_ask / ask_once / allow.
- [ ] Auto-run: exact match + risk <= local_write + N успехов + scope соблюдён.
- [ ] Fuzzy match: никогда auto-run, только с подтверждением.
- [ ] Agent risk — рекомендательный. terio вычисляет финальный.
- [ ] Path boundary validation (защита от ../../).
- [ ] `terio config`.

**Критерий:** destructive требует подтверждения. Fuzzy match не auto-run. Path traversal blocked.

## 5. Undo/Redo (Experimental)

- [ ] Sandbox (bubblewrap/overlay FS).
- [ ] Warn (только предупреждение).
- [ ] Best-effort snapshot для скриптов.
- [ ] `terio undo`, `terio redo`.
- [ ] Off by default.

## 6. Рендеринг + оконная система

- [ ] Timeline (git log).
- [ ] Card (статусы).
- [ ] Progress (длинные операции).
- [ ] Readable page (лог, новости).
- [ ] Авто-выбор renderer на основе display_profile.
- [ ] Блок → Window эволюция (каждый блок — будущее окно).
- [ ] Чат-окно: последовательность встроенных окон (картинки, сообщения, результаты).
- [ ] `terio stats` с разделением cost_counters.
- [ ] Минимизация total_attention_cost при выборе маршрута (cache vs model).

**Критерий:** `git log` — timeline. `terio log` показывает пары (interaction_id).

## 7. Интеграции (ленивые) + шэринг

- [ ] Каждая новая программа — через запрос пользователя.
- [ ] terio учится работать с Git, GitHub, медиа, Docker и т.д.
- [ ] Никаких заранее написанных коннекторов.
- [ ] **Автоматическая интеграция:** агент идентифицирует программу, читает --help/man/wiki, пишет integration script, прогоняет тесты.
- [ ] Интеграционный скрипт сохраняется в Script Cache.
- [ ] **Шэринг:** копирование окон между экземплярами terio (через instance_id).
- [ ] `terio share`, `terio receive`.

## 8. Оптимизация стоимости + предсказание

- [ ] Раздельные счётчики cost_counters в единой метрике total_attention_cost (реальные веса).
- [ ] cache vs model: terio выбирает маршрут с минимальной total_attention_cost.
- [ ] История стоимости: `terio cost` — отчёт по затратам.
- [ ] Auto-tuning: terio предлагает выключить auto-run для дорогих скриптов.
- [ ] **Pre-execution:** отдельный режим, terio предсказывает запрос до нажатия Enter, выполняет read_only шаги, показывает preview.

## 9. Desktop + сообщество + локальная LLM

- [ ] **Desktop (standalone-пакет, system tray, автообновление).** До этого — Dioxus webview как встроенное окно.
- [ ] Экспорт/импорт скриптов.
- [ ] **Документ = мультиокно:** объединение окон в документ, экспорт как документация.
- [ ] Реестр скриптов.
- [ ] **Локальная LLM:** open-source модель с открытыми весами, обучаемая на базе скриптов пользователя.
