# terio

**Агрегатор интерфейсов с AI-планированием, локальным логом и replay cache.**

terio принимает естественный запрос, строит structured plan команд, показывает риск и подтверждение, выполняет шаги и пишет всё в локальный JSONL-лог. Удачные повторяемые сценарии могут переиспользоваться из Script Cache без повторного вызова модели.

## Что уже есть

- `terio ask "<request>"` для mock provider и baseline OpenAI provider
- `terio run -- <command...>` для прямого shell execution
- JSONL log + `LogStore` + Dioxus desktop UI
- Trust layer: policy, scope/path validation, exact/fuzzy distinction, confirmation
- Pending confirmation с exact saved execution через `terio confirm`
- Script Cache для exact normalized replay
- Redaction для лога, preview pending state и cache admission checks
- `terio stats`, `terio log --json`, `terio config`, `terio cancel`

## Ограничения текущего прототипа

- OpenAI provider пока experimental: строгий JSON mode, `cache_template` и usage-token accounting ещё не реализованы
- API key и exact pending execution state хранятся локально на диске; на Unix файлы пишутся с правами `0600`
- Sensitive commands/arguments не попадают в cache replay files
- UI работает как desktop control panel, но long-running actions пока запускаются через дочерний процесс без live-stream

## Текущее состояние

Репозиторий уже содержит рабочий baseline для:

- mock agent
- cache
- trust/security
- OpenAI provider abstraction
- Dioxus UI с ask/pending/config
- CI: `fmt`, `clippy -D warnings`, `build`, `test`

Тестов в текущем дереве: `94`.

## Основные команды

```bash
terio ask "list files"
terio confirm
terio run -- echo hello
terio log --json
terio stats
terio config show
```

## Документы

- [MVP](docs/mvp.md)
- [Architecture](architecture.md)
- [Roadmap](roadmap.md)
- [Agent Protocol](docs/agent-protocol.md)
- [Trust Model](docs/trust-model.md)
- [Script Cache](docs/script-cache.md)
- [Behavior Log](docs/behavior-log.md)
