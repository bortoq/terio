# Roadmap

Каждая фаза — список конкретных вещей, которые надо сделать.
Сделано → `[x]` → все `[x]` → фаза готова → аудит.

---

## 0. Определение архитектуры

- [x] Сформулировать суть: агрегатор интерфейсов
- [x] Scope: расширяется лениво, от пользовательских запросов
- [x] Стек: Rust, Dioxus (webview), CLI-совместимость
- [x] docs/mvp.md, docs/trust-model.md, docs/behavior-log.md, docs/agent-protocol.md
- [x] LICENSE
- [x] Экономическая модель: cost_counters в каждой записи лога
- [x] JSON Schema: agent-output, script-cache, behavior-log
- [x] LogWriter/LogReader traits — дизайн для смены формата
- [x] identity: instance_id + session_id
- [x] display_profile — типизация записей лога
- [x] interaction_id — группировка пар
- [x] Cargo.toml и базовая src/ структура
- [x] CI: линтер + сборка

---

## 1. Shell + log + scaffold

- [x] `terio run -- <command>` — запуск, захват stdout/stderr/exit/duration
- [x] Identity: instance_id (ULID) + session_id (UUID v4)
- [x] LogWriter trait + JsonlLogWriter: append → validate → write
- [x] LogReader trait + JsonlLogReader: recent(n), by_session(), by_interaction()
- [x] LogStore: writer + reader + broadcaster
- [x] Accounting: cost_counters, aggregate, compute_attention_cost stub
- [x] display_profile: type, renderer_hint, user_visible
- [x] Dioxus scaffold: окно, показывает лог через LogStore::recent(50)
- [x] `terio log --json` — история в JSON
- [x] CI: cargo test + cargo build

---

## 2. Mock agent + cache + redact + risk

- [x] `terio ask "list files"` — mock: 5 hardcoded запросов
- [x] Script Cache: первый ask → сохранить chain (JSON в `~/.terio/cache/`)
- [x] Request Matcher: exact normalized match (lowercase+trim+collapse + SHA-256)
- [x] Повторный `terio ask "list files"` — cache hit, без mock
- [x] `terio stats` — model_calls, cache_hits, cost_counters
- [x] Redact: Bearer, api_key, token, SSH key, URL credentials
- [x] Risk classifier: destructive/network_write/local_write/credential_access
- [x] Группировка по interaction_id в логе
- [x] Не кешировать non-zero exit
- [x] success_count_before/after в ScriptRun
- [x] Scope в CacheEntry
- [x] Mock только read-only команды (никаких mkdir/rm)
- [x] Warning для destructive/network_write/credential_access при `terio run`
- [x] Table renderer в Dioxus (6 колонок, цвета)

---

## 3. Реальный LLM provider

- [x] `terio config set provider.type openai/anthropic/ollama/mock`
- [x] `terio config set provider.api_key`, `provider.model`, `provider.base_url`
- [x] `terio config show` — маскирует API key
- [x] OpenAI provider — вызывает Chat Completions API, парсит JSON-план
- [x] Provider trait: plan(&self, request) → AgentPlan
- [x] MockProvider — обёртка над существующим get_mock_plan
- [x] create_provider(config) — фабрика по типу провайдера
- [x] План → подтверждение (y/N) → выполнение для risk >= destructive
- [x] Secrets redact перед отправкой в модель
- [x] `terio cancel` — отправляет SIGTERM активному процессу
- [x] Ctrl+C — перехватывает SIGINT, убивает процесс, чисто завершается
- [x] Поле ввода + кнопка Ask в Dioxus UI (spawn `terio ask --yes`)

---

## 4. Trust + безопасность

- [x] Policy: always_ask / ask_once / allow
- [x] Auto-run: exact match + risk <= local_write + N успехов + scope соблюдён
- [x] Fuzzy match: никогда auto-run, только подтверждение
- [x] Path boundary validation (защита от ../../)
- [x] Отображение подтверждения плана в UI (risk, команды, accept/decline)
- [x] Индикатор trust level для каждой команды в UI
- [x] Настройки в UI — окно конфигурации

---

## 5. Undo/Redo (Experimental)

- [ ] Sandbox (bubblewrap/overlay FS)
- [ ] Warn (только предупреждение)
- [ ] Best-effort snapshot для скриптов
- [ ] `terio undo`, `terio redo`
- [ ] Off by default
- [ ] Кнопки Undo/Redo в UI

---

## 6. Продвинутый рендеринг + интерактивность

- [ ] Live-stream: LogStore broadcast → Dioxus (вместо poll)
- [ ] Индикатор выполнения — spinner/progress для длительных операций
- [ ] Timeline (git log style)
- [ ] Card view (статусы, риски)
- [ ] Readable page (лог, новости)
- [ ] Авто-выбор renderer на основе display_profile
- [ ] Блок → Window эволюция
- [ ] Чат-окно: последовательность окон

---

## 7. Интеграции (ленивые)

- [ ] Каждая новая программа — через запрос пользователя
- [ ] terio учится работать с Git, GitHub, медиа, Docker
- [ ] Никаких заранее написанных коннекторов
- [ ] Агент читает --help/man/wiki, пишет integration script
- [ ] Integration script → Script Cache
- [ ] Окно интеграции: выбор программы, статус изучения
- [ ] Шэринг: копирование окон между экземплярами terio
- [ ] `terio share`, `terio receive`

---

## 8. Оптимизация стоимости

- [ ] Раздельные cost_counters → total_attention_cost
- [ ] cache vs model: выбор маршрута с минимальной стоимостью
- [ ] `terio cost` — отчёт по затратам
- [ ] Auto-tuning: предложение отключить auto-run для дорогих скриптов
- [ ] Pre-execution: read_only шаги до нажатия Enter
- [ ] Графики стоимости в UI
- [ ] Предпросмотр (preview) в окне

---

## 9. Desktop + сообщество

- [ ] Локальная LLM: open-source модель
- [ ] Desktop — standalone-пакет, system tray, автообновление
- [ ] Экспорт/импорт скриптов
- [ ] Документ = мультиокно: объединение окон, экспорт как документация
- [ ] Реестр скриптов
