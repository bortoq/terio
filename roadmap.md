# Roadmap

## Фаза 0: Определение (текущая)

- [x] Сформулировать суть: terio = панель управления для программ с отсоединяемым интерфейсом.
- [x] Определить три столпа: терминал + рендеринг вывода + кеширование поведения.
- [x] Зафиксировать: AI-модель встроена с первого дня.
- [x] Выбрать стек: Rust, CLI-first.
- [x] Написать docs/mvp.md, docs/trust-model.md, docs/behavior-log.md.
- [x] Добавить LICENSE.
- [ ] Создать Cargo.toml и базовую src/ структуру.
- [ ] CI: линтер + сборка.

## Фаза 1: Shell execution + базовый рендеринг

- [ ] `terio run -- <command>` — исполнить shell-команду, показать stdout/stderr.
- [ ] Захват exit code, duration, argv, working directory.
- [ ] `terio rerun` — повтор последней команды.
- [ ] JSONL-лог (v1) каждого выполнения.
- [ ] Plain text renderer (fallback).
- [ ] Table renderer для `ls -l` / csv-подобного вывода.
- [ ] Undo/Redo: удалённые файлы → `~/.terio/trash/`. `terio undo`.

**Критерий:** `terio run -- ls -l` показывает таблицу. Лог пишется. `terio undo` откатывает.

## Фаза 2: Agent + поведение

- [ ] `terio ask "..."` — естественно-языковый ввод к встроенной AI-модели.
- [ ] Агент генерирует shell-команды по запросу, объясняет рискованные.
- [ ] Агент исполняет сгенерированные команды и показывает результат.
- [ ] Behavior Log фиксирует request → agent → commands → result.
- [ ] После 3+ успешных однотипных выполнений — предложение рецепта.
- [ ] Recipe: YAML (v1), structured argv, preconditions, postconditions.
- [ ] Выполнение рецепта с валидацией аргументов.

**Критерий:** `terio ask "split this flac/cue"` — агент генерирует ffmpeg, исполняет, показывает таблицу. После 3 раз — предлагает рецепт.

## Фаза 3: Trust Engine + безопасность

- [ ] Risk taxonomy (read_only, local_write, destructive, network_*, credential, financial).
- [ ] Permission policy (always_ask / ask_once / allow_for_dir / allow_for_recipe / never).
- [ ] Confidence score + авто-запуск по порогу.
- [ ] Expandable trace: пользователь видит команды до/после выполнения.
- [ ] Redaction secrets из всех полей лога.
- [ ] Shell quoting и structured argv (shell_allowed: false в рецептах).

**Критерий:** destructive-рецепт требует подтверждения. Secrets не пишутся в лог.

## Фаза 4: Расширение рендеринга

- [ ] Timeline renderer (git log).
- [ ] Card renderer (статусы, предупреждения).
- [ ] Gallery renderer (файлы, превью).
- [ ] Progress renderer (длинные операции).
- [ ] Readable page renderer (новости, логи, документация).
- [ ] Авто-выбор renderer на основе типа вывода.

**Критерий:** `git log` — commit timeline. `curl news` — читаемая страница.

## Фаза 5: Интеграции (по мере готовности)

Здесь не «коннекторы» — здесь terio учится управлять программами через их существующие CLI/API.

- [ ] Git: `git status` → table, `git log` → timeline.
- [ ] GitHub через `gh`: issue list → card, PR → card, CI → status.
- [ ] Медиа через `mpv`/`cmus`: плейлист → card, текущий трек → now playing.
- [ ] Файловые операции через `cp`/`rsync`: прогресс, таблица.
- [ ] Браузер через `open`/`xdg-open`: открыть ссылку, показать превью.
- [ ] Download через `curl`/`aria2c`: прогресс, статус.
- [ ] Любая новая интеграция, которую пользователь может описать естественным языком.

**Принцип:** не строить коннекторы, а показать пользователю, как управлять любой программой через terio. Каждая интеграция — это просто shell-команда + рендеринг. Агент помогает их составить.

## Фаза 6: Working environment

- [ ] `terio config` — политики доверия, провайдеры, алиасы.
- [ ] `terio recipe list` / `terio recipe edit`.
- [ ] `terio log` — просмотр истории с фильтрами.
- [ ] `terio stats` — сколько LLM-токенов и времени сэкономлено.
- [ ] Cost-savings display.

**Критерий:** пользователь настраивает terio под себя, не выходя из CLI.

## Фаза 7: Desktop / Advanced

- [ ] Desktop-сборка (Rust + webview).
- [ ] Расширенная корзина с квотами.
- [ ] Продвинутый undo/redo с историей изменений.
- [ ] Экспорт/импорт рецептов.
- [ ] Session export: поделиться блоком или историей.

## Фаза 8: Сообщество

- [ ] Реестр рецептов (community recipes).
- [ ] Шаблоны renderer'ов.
- [ ] API для пользовательских renderer'ов.
- [ ] Плагины на WebAssembly?
