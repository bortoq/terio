# Roadmap

## Фаза 0: Определение и прототип (сейчас)

- [x] Сформулировать суть: terio = терминал + рендеринг + кеширование поведения.
- [x] Зафиксировать scope: не коннекторы, не модальные редакторы, не шэринг.
- [x] Выбрать стек: Rust, CLI-first.
- [ ] Написать docs/mvp.md с жёстким scope.
- [ ] Написать docs/trust-model.md с risk taxonomy и permission policy.
- [ ] Добавить LICENSE.
- [ ] Создать Cargo.toml и базовую структуру src/.
- [ ] CI: базовый линтер и сборка.

## Фаза 1: Shell execution + рендеринг (MVP Core)

- [ ] `terio run <command>` — исполнить shell-команду, показать stdout/stderr.
- [ ] Захват exit code, duration, working directory.
- [ ] Повторный запуск последней команды (`terio rerun`).
- [ ] Рендеринг результата как plain text (fallback).
- [ ] Один структурный renderer: таблица для `ls -l` / csv / ffmpeg output.
- [ ] JSONL-лог каждого выполнения (Behavior Log).

**Критерий:** `terio run ls -l` показывает файлы как таблицу, а `terio run echo hello` показывает plain text. Лог пишется.

## Фаза 2: Behavior Log + компилятор рецептов

- [ ] Анализ лога: группировка похожих запросов и команд.
- [ ] Предложение рецепта после N успешных выполнений.
- [ ] Формат Recipe: YAML (аргументы, шаги, прекондишены, риск).
- [ ] Валидация аргументов и прекондишенов.
- [ ] Выполнение рецепта: подстановка аргументов с экранированием.
- [ ] Повышение/понижение confidence на основе успехов/неудач.

**Критерий:** terio замечает, что пользователь трижды выполнил `ffmpeg -i track.flac ...`, и предлагает рецепт. Рецепт запускается с новыми аргументами.

## Фаза 3: Trust Engine + безопасность

- [ ] Risk taxonomy (read_only, local_write, destructive, network_*, credential, financial).
- [ ] Permission policy: always_ask / ask_once / allow_for_dir / allow_for_recipe / never.
- [ ] Confirmation UI перед опасными рецептами.
- [ ] Redaction secrets из лога.
- [ ] Shell quoting и защита от injection.
- [ ] Expandable trace: пользователь видит, какие команды реально будут выполнены.

**Критерий:** рецепт с risk `destructive` требует подтверждения. Secrets не пишутся в лог.

## Фаза 4: Agent integration

- [ ] Естественно-языковый ввод (`terio ask "..."`).
- [ ] LLM-провайдер: конфигурируемый (OpenAI, Anthropic, локальный).
- [ ] Агент генерирует shell-команды по запросу.
- [ ] Fallback: если рецепт не найден → агент.
- [ ] Fallback: если рецепт упал → агент с контекстом ошибки.
- [ ] Показ экономии: "этот запрос выполнен по рецепту, сэкономлено X токенов".

**Критерий:** `terio ask "split this flac/cue and name like last time"` — если рецепт есть, выполняется без LLM; если нет, агент генерирует команды.

## Фаза 5: Расширение рендеринга

- [ ] Timeline renderer (git log).
- [ ] Card renderer (статусы, предупреждения).
- [ ] Gallery renderer (файлы, превью).
- [ ] Progress renderer (длинные операции с шагами).
- [ ] Readable page renderer (новости, логи, документация).
- [ ] Авто-выбор renderer на основе типа вывода.

**Критерий:** `git log` показывается как commit timeline. `curl` новости — как читаемая страница.

## Фаза 6: Working environment

- [ ] `terio init` — настройка workspace (CWD, алиасы, провайдеры).
- [ ] `terio config` — управление политиками доверия.
- [ ] `terio recipe list` / `terio recipe edit`.
- [ ] `terio log` — просмотр истории.
- [ ] `terio stats` — экономия токенов и времени.

**Критерий:** пользователь может настроить terio под себя, не выходя из CLI.

## Фаза 7: Connectors (extended)

- [ ] GitHub: отображать issues/PR как рендеренные блоки.
- [ ] Media: управление плейлистами через shell + render.
- [ ] Download: поиск и загрузка отсутствующих файлов.
- [ ] Session export: поделиться блоком или историей.

> **Важно:** коннекторы не встраиваются в terio, а реализуются через shell-команды + рендеринг. terio не становится «медиаплеером» — он показывает результат работы медиа-команд.

## Фаза 8: Desktop / Commercial

- [ ] Упаковка: десктопный билд (Rust + webview).
- [ ] Freemium: лимит рецептов / agent-минут.
- [ ] Paid: история, расширенные политики, несколько профилей.
- [ ] Cost-savings dashboard.
- [ ] Marketplace рецептов (community).
