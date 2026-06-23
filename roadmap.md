# Roadmap

## 0. Определение

- [x] Сформулировать суть: агрегатор интерфейсов. Все программы с CLI/API — из одной точки.
- [x] Scope расширяется лениво — от пользовательских запросов.
- [x] Стек: Rust, CLI-first.
- [x] docs/mvp.md, docs/trust-model.md, docs/behavior-log.md, docs/agent-protocol.md.
- [x] LICENSE.
- [x] Экономическая модель: разделение стоимости внимания (user/agent/system).
- [x] JSON Schema: agent-output, script-cache, behavior-log.
- [ ] Cargo.toml и базовая src/ структура.
- [ ] CI: линтер + сборка.

## 1. Agent MVP

`terio ask` + agent + script cache — ядро.

### 1A: Shell execution + базовый лог

- [ ] Cargo.toml, src/main.rs, src/cli.rs, src/run.rs.
- [ ] `terio run -- <command>` — shell без модели.
- [ ] Захват stdout, stderr, exit code, duration.
- [ ] `terio rerun`.
- [ ] Plain renderer.
- [ ] JSONL лог (command_run) с attention_cost.
- [ ] CI: cargo test + cargo build.

**Критерий:** `terio run -- echo hello`, `terio run -- ls -l`, `terio log`.

### 1B: Mock agent + exact cache (без реальной модели)

- [ ] `terio ask "list files"` — mock: если request == "list files", вернуть `ls -l`.
- [ ] Script Cache: первый ask → сохранить chain.
- [ ] Request Matcher: exact normalized match.
- [ ] Повторный `terio ask "list files"` — cache hit, без mock/model.
- [ ] `terio stats` — model_calls, cache_hits, attention_cost.
- [ ] Table renderer.

**Критерий:** `terio ask "list files"` (первый) → mock. Повторный → cache hit, быстрее, без вызова. `terio stats` показывает cache_hits > 0.

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

## 4. Расширение рендеринга + экономическая модель

- [ ] Timeline (git log).
- [ ] Card (статусы).
- [ ] Progress (длинные операции).
- [ ] Readable page (лог, новости).
- [ ] Авто-выбор renderer.
- [ ] `terio stats` с разделением attention_cost: user_sec, agent_sec, system_sec.
- [ ] Минимизация total_attention_cost при выборе маршрута (cache vs model).

**Критерий:** `git log` — timeline. `terio stats` показывает внимание пользователя, агента, системы.

## 5. Интеграции (ленивые)

- [ ] Каждая новая программа — через запрос пользователя.
- [ ] terio учится работать с Git, GitHub, медиа, Docker и т.д.
- [ ] Никаких заранее написанных коннекторов.
- [ ] **Автоматическая интеграция:** агент идентифицирует программу, читает --help/man/wiki, пишет integration script, прогоняет тесты.
- [ ] Интеграционный скрипт сохраняется как Script Cache entry с высоким trust_threshold (требуется ручное подтверждение).

## 6. Оптимизация стоимости

- [ ] Раздельные счётчики внимания (user/agent/system) в единой метрике user-seconds.
- [ ] cache vs model: terio выбирает маршрут с минимальной total_attention_cost.
- [ ] История стоимости: `terio cost` — отчёт по затратам внимания.
- [ ] Auto-tuning: terio предлагает выключить auto-run для дорогих скриптов.

## 7. Desktop + сообщество

- [ ] Desktop (Rust + webview).
- [ ] Экспорт/импорт скриптов.
- [ ] Шэринг сессий.
- [ ] Реестр скриптов.
