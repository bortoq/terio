# Roadmap

## 0. Определение

- [x] Сформулировать суть: агрегатор интерфейсов. Все программы с CLI/API — из одной точки.
- [x] Scope расширяется лениво — от пользовательских запросов.
- [x] Стек: Rust, CLI-first.
- [x] docs/mvp.md, docs/trust-model.md, docs/behavior-log.md, docs/agent-protocol.md.
- [x] LICENSE.
- [ ] Cargo.toml и базовая src/ структура.
- [ ] CI: линтер + сборка.

## 1. Agent MVP

`terio ask` + agent + script cache — ядро.

- [ ] `terio ask "..."` — запрос к модели.
- [ ] Модель возвращает structured plan (command + argv).
- [ ] План → подтверждение → выполнение.
- [ ] `terio run -- <command>` — shell без модели.
- [ ] Plain + table renderer.
- [ ] Script Cache: успех → сохранение с параметрами/preconditions.
- [ ] Request Matcher: exact normalized match.
- [ ] JSONL лог (agent_turn, command_run, script_run).
- [ ] Метрики: model_calls, cache_hits, errors (счётчики).
- [ ] Risk: destructive/network_write → всегда подтверждение.
- [ ] `terio rerun`, `terio log`, `terio cancel`, `terio stats`.

**Критерий:** `terio ask "list files"` — модель генерирует `ls -l`, terio показывает таблицу. Повторный запрос — без модели. `terio stats` показывает сколько вызовов сэкономлено.

## 2. Trust + безопасность

- [ ] Risk taxonomy во всех компонентах.
- [ ] Redaction до модели и до лога.
- [ ] Policy: always_ask / ask_once / allow.
- [ ] Auto-run: exact match + risk <= local_write + N успехов.
- [ ] Fuzzy match: никогда auto-run, только с подтверждением.
- [ ] Agent risk — рекомендательный. terio вычисляет финальный.
- [ ] `terio config`.

**Критерий:** destructive требует подтверждения. Fuzzy match не auto-run.

## 3. Undo/Redo (Experimental)

- [ ] Sandbox (bubblewrap/overlay FS).
- [ ] Warn (только предупреждение).
- [ ] Best-effort snapshot для скриптов.
- [ ] `terio undo`, `terio redo`.
- [ ] Off by default.

## 4. Расширение рендеринга

- [ ] Timeline (git log).
- [ ] Card (статусы).
- [ ] Progress (длинные операции).
- [ ] Readable page (лог, новости).
- [ ] Авто-выбор renderer.

**Критерий:** `git log` — timeline.

## 5. Рабочая среда

- [ ] `terio script list`, `terio script remove`.
- [ ] Управление скриптами через `$EDITOR`.
- [ ] `terio stats` — детальная агрегация из лога.

## 6. Интеграции (ленивые)

- [ ] Каждая новая программа — через запрос пользователя.
- [ ] terio учится работать с Git, GitHub, медиа, Docker и т.д.
- [ ] Никаких заранее написанных коннекторов.

## 7. Desktop + сообщество

- [ ] Desktop (Rust + webview).
- [ ] Экспорт/импорт скриптов.
- [ ] Шэринг сессий.
- [ ] Реестр скриптов.
