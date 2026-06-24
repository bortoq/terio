# Agent Protocol

Этот документ описывает **текущую baseline-реализацию** и отдельно отмечает целевой, но ещё не реализованный контракт.

## Сейчас реализовано

### Input в real provider

OpenAI provider сейчас отправляет модели:

- redacted `request`
- текущий `cwd`
- список top-level entries текущей директории

Содержимое файлов не отправляется.

### Output от provider

Сейчас terio ожидает только:

```json
{
  "summary": "List files using ls -la",
  "risk": "read_only",
  "commands": [
    {
      "command": "ls",
      "argv": ["ls", "-la"],
      "risk": "read_only",
      "reason": "Detailed listing"
    }
  ]
}
```

### Runtime validation

Перед execution terio дополнительно проверяет:

- `argv` не пустой
- `command == argv[0]`
- risk пересчитывается локально по `argv[0]` и аргументам
- unknown commands не auto-run
- path boundary и scope не обходятся через `--yes`

### Confirmation

Если plan требует подтверждения, terio:

1. сохраняет redacted preview в `~/.terio/pending-plan.json`
2. сохраняет exact execution state в `~/.terio/pending-exec.json`
3. выполняет подтверждённый plan только через `terio confirm`

Это значит, что UI/CLI confirmation больше не вызывает provider повторно для уже подтверждённого action.

## Ещё не реализовано

Следующие части остаются целевыми, но не реализованы полностью:

- `cache_template`
- strict JSON mode / schema-bound provider response
- usage-token accounting из ответа provider
- hash-bound approval of preview vs exact execution payload
- richer provider context beyond `cwd` и top-level entries

## Known Commands

Known command list задаётся локально в `run::is_known_command()`. Known не означает safe: final risk всё равно вычисляется локально.
