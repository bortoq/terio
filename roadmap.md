# Roadmap

## 0. Определение

- [x] Сформулировать суть: агрегатор интерфейсов. Все программы с CLI/API — из одной точки.
- [x] Scope расширяется лениво — от пользовательских запросов.
- [x] Стек: Rust, CLI-first, Dioxus UI.
- [x] docs/mvp.md, docs/trust-model.md, docs/behavior-log.md, docs/agent-protocol.md.
- [x] LICENSE.
- [x] Экономическая модель: разделение стоимости внимания (user/agent/system).
- [x] JSON Schema: agent-output, script-cache, behavior-log.
- [x] LogWriter/LogReader traits — дизайн для смены формата.
- [x] identity: instance_id + session_id.
- [x] display_profile — типизация записей лога.
- [x] interaction_id — группировка пар.
- [ ] Cargo.toml и базовая src/ структура.
- [ ] CI: линтер + сборка.

## 1. Agent MVP

`terio ask` + agent + script cache — ядро.

### 1A: Shell execution + лог + identity + accounting

- [ ] Cargo.toml, src/main.rs, src/cli.rs, src/run.rs.
- [ ] `terio run -- <command>` — shell без модели.
- [ ] Захват stdout, stderr, exit code, duration.
- [ ] `terio rerun`.
- [ ] **Identity:** instance_id генерируется при первом запуске; session_id на каждый запуск.
- [ ] **LogWriter trait + JsonlLogWriter:** append, in-memory broadcast channel.
- [ ] **LogReader trait + JsonlLogReader:** stream(), recent(n), by_session(id), by_interaction(id).
- [ ] **Accounting:** cost_counters в каждой записи; aggregate; заглушка compute_attention_cost.
- [ ] **display_profile:** type, renderer_hint, user_visible.
- [ ] Plain renderer (читает из лога).
- [ ] Dioxus webview (показывает лог).
- [ ] JSONL лог (command_run + system_event).
- [ ] CI: cargo test + cargo build.

**Критерий:** `terio run -- echo hello`, `terio run -- ls -l`, `terio log`. `~/.terio/instance.json` создан. `terio log --json` показывает cost_counters и display_profile.

### 1B: Mock agent + exact cache (без реальной модели)

- [ ] `terio ask "list files"` — mock: если request == "list files", вернуть `ls -l`.
- [ ] Script Cache: первый ask → сохранить chain.
- [ ] Request Matcher: exact normalized match.
- [ ] Повторный `terio ask "list files"` — cache hit, без mock/model.
- [ ] `terio stats` — model_calls, cache_hits, cost_counters.
- [ ] Table renderer.
- [ ] Группировка по interaction_id в логе.

**Критерий:** `terio ask "list files"` (первый) → mock. Повторный → cache hit, быстрее, без вызова. `terio stats` показывает cache_hits > 0 и cost_counters.

### 1C: Реальный LLM provider

- [ ] Конфигурация провайдера (OpenAI, Anthropic, ollama).
- [ ] Agent возвращает structured plan (command + argv).
- [ ] cache_template с steps от модели → terio сохраняет.
- [ ] План → подтверждение → выполнение.
- [ ] Script Cache: scope.cwd_policy (same_cwd_only / any_cwd_with_parameters).
- [ ] Risk: destructive/network_write → всегда подтверждение.
- [ ] Redaction secrets до отправки в модель.
- [ ] `terio cancel`.

**Критерий:** `terio ask "list files"` — реальная модель генерирует `ls -l`, terio показывает таблицу. Secrets не уходят в модель.

## 2. Trust + безопасность

- [ ] Risk taxonomy во всех компонентах.
- [ ] Redaction до модели и до лога.
- [ ] Policy: always_ask / ask_once / allow.
- [ ] Auto-run: exact match + risk <= local_write + N успехов + scope соблюдён.
- [ ] Fuzzy match: никогда auto-run, только с подтверждением.
- [ ] Agent risk — рекомендательный. terio вычисляет финальный.
- [ ] Path boundary validation (защита от ../../).
- [ ] `terio config`.

**Критерий:** destructive требует подтверждения. Fuzzy match не auto-run. Path traversal blocked.

## 3. Undo/Redo (Experimental)

- [ ] Sandbox (bubblewrap/overlay FS).
- [ ] Warn (только предупреждение).
- [ ] Best-effort snapshot для скриптов.
- [ ] `terio undo`, `terio redo`.
- [ ] Off by default.

## 4. Расширение рендеринга + оконная система

- [ ] Timeline (git log).
- [ ] Card (статусы).
- [ ] Progress (длинные операции).
- [ ] Readable page (лог, новости).
- [ ] Авто-выбор renderer на основе display_profile.
- [ ] Блок → Window эволюция (каждый блок — будущее окно).
- [ ] Чат-окно: последовательность встроенных окон (картинки, сообщения, результаты).
- [ ] `terio stats` с разделением attention_cost: user_sec, agent_sec, system_sec.
- [ ] Минимизация total_attention_cost при выборе маршрута (cache vs model).

**Критерий:** `git log` — timeline. `terio log` показывает пары (interaction_id). `terio stats` — cost_counters.

## 5. Интеграции (ленивые) + шэринг

- [ ] Каждая новая программа — через запрос пользователя.
- [ ] terio учится работать с Git, GitHub, медиа, Docker и т.д.
- [ ] Никаких заранее написанных коннекторов.
- [ ] **Автоматическая интеграция:** агент идентифицирует программу, читает --help/man/wiki, пишет integration script, прогоняет тесты.
- [ ] Интеграционный скрипт сохраняется как Script Cache entry с высоким trust_threshold.
- [ ] **Шэринг:** копирование окон между экземплярами terio (через instance_id).
- [ ] `terio share`, `terio receive`.

## 6. Оптимизация стоимости + предсказание

- [ ] Раздельные счётчики cost_counters в единой метрике total_attention_cost (реальные веса).
- [ ] cache vs model: terio выбирает маршрут с минимальной total_attention_cost.
- [ ] История стоимости: `terio cost` — отчёт по затратам внимания.
- [ ] Auto-tuning: terio предлагает выключить auto-run для дорогих скриптов.
- [ ] **Pre-execution (предсказание ввода):** отдельный режим, terio предсказывает запрос до нажатия Enter, выполняет read_only шаги, показывает preview.
- [ ] Request Matcher вызывается на partial input.

## 7. Desktop + сообщество + локальная LLM

- [ ] Desktop (Rust + webview / Dioxus desktop).
- [ ] Экспорт/импорт скриптов.
- [ ] **Документ = мультиокно:** объединение окон в документ, экспорт как документация.
- [ ] Реестр скриптов.
- [ ] **Instance identity:** уникальные ID для шэринга и возобновления.
- [ ] **Локальная LLM:** open-source модель с открытыми весами, обучаемая на хосте пользователя под создаваемые скрипты.
