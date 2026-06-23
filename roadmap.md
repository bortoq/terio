# Roadmap

## Фаза 0: Определение

- [x] Сформулировать суть: агентный терминал, который кеширует успешные цепочки.
- [x] Scope расширяется лениво — от пользовательских запросов, а не от заранее спроектированных коннекторов.
- [x] Выбрать стек: Rust, CLI-first.
- [x] Написать docs/mvp.md, docs/trust-model.md, docs/behavior-log.md.
- [x] Добавить LICENSE.
- [ ] Создать Cargo.toml и базовую src/ структуру.
- [ ] CI: линтер + сборка.

## Фаза 1: Agent MVP

**terio ask + agent + cache — ядро продукта.**

- [ ] `terio ask "..."` — запрос к модели (провайдер конфигурируется).
- [ ] Модель возвращает structured plan (command + argv).
- [ ] План показывается пользователю: подтверждение перед выполнением.
- [ ] Выполнение: shell commands, захват stdout/stderr/exit code/duration.
- [ ] Plain text renderer + table renderer (авто-определение).
- [ ] Script Cache: успешная цепочка → сохранение по запросу.
- [ ] Request Matcher: второй раз тот же запрос → скрипт без модели.
- [ ] JSONL лог (schema v1: agent_turn, command_run, script_run).
- [ ] Risk check: destructive/network_write → всегда подтверждение.
- [ ] `terio rerun`, `terio log`, `terio cancel`.

**Критерий:** `terio ask "list files"` — модель генерирует `ls -l`, terio показывает таблицу. Повторный `terio ask "list files"` — без модели.

## Фаза 2: Trust + безопасность

- [ ] Risk taxonomy во всех компонентах.
- [ ] Redaction secrets до отправки в модель и до записи в лог.
- [ ] Policy: always_ask / ask_once / allow для каждого risk level.
- [ ] Confidence: авто-запуск скрипта после N успехов.
- [ ] `terio config` — управление политиками и провайдерами.

**Критерий:** destructive-запросы требуют подтверждения. Secrets не видны модели и не пишутся в лог.

## Фаза 3: Undo/Redo (Experimental)

- [ ] Режим sandbox: bubblewrap/overlay FS для изоляции.
- [ ] Режим warn: только предупреждение перед destructive-действиями.
- [ ] Snapshot файлов перед скриптом (best-effort).
- [ ] `terio undo`, `terio redo`.
- [ ] Конфиг: UNDO_MODE = off | sandbox | warn.
- [ ] По умолчанию: off.

**Критерий:** пользователь включил sandbox → `terio ask "delete temp files"` → файлы в корзине, `terio undo` восстанавливает.

## Фаза 4: Расширение рендеринга

- [ ] Timeline renderer (git log, хронология).
- [ ] Card renderer (статусы, предупреждения).
- [ ] Progress renderer (длинные операции).
- [ ] Readable page renderer (лог, новости).
- [ ] Авто-выбор renderer по типу вывода.

**Критерий:** `git log` показывается как timeline.

## Фаза 5: Рабочая среда

- [ ] `terio stats` — экономия LLM-вызовов.
- [ ] `terio script list` — просмотр кеша скриптов.
- [ ] `terio script remove <hash>` — удаление скрипта.
- [ ] Управление скриптами через `$EDITOR`.

**Критерий:** пользователь видит, сколько LLM-вызовов сэкономлено.

## Фаза 6: Интеграции (ленивые)

- [ ] Каждая новая интеграция — просто новый запрос пользователя.
- [ ] terio учится управлять Git, GitHub, медиа, браузером, Docker и т.д. — когда пользователь спросит.
- [ ] Никаких заранее написанных коннекторов. Только то, что попросил пользователь и подтвердил.

## Фаза 7: Desktop / Sharing

- [ ] Desktop-сборка (Rust + webview).
- [ ] Экспорт/импорт скриптов.
- [ ] Шэринг сессий (read-only).
- [ ] Реестр скриптов (сообщество).
