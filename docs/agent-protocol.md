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

Опционально provider может вернуть `cache_template`, но текущий runtime использует его как metadata для cache persistence и validation; execution всё равно идёт по `commands`.

### Runtime validation

Перед execution terio дополнительно проверяет:

- JSON parse через `serde` + shape validation для обязательных полей provider response
- `summary` не пустой
- `commands` не пустой
- `argv` не пустой
- `command == argv[0]`
- пустые элементы `argv` запрещены
- `cache_template.steps`, если присутствуют, тоже валидируются по тем же инвариантам
- risk пересчитывается локально по `argv[0]` и аргументам
- unknown commands не auto-run
- path boundary и scope не обходятся через `--yes`

### Confirmation

Если plan требует подтверждения, terio:

1. сохраняет redacted preview в `~/.terio/pending-plan.json`
2. сохраняет exact execution state в `~/.terio/pending-exec.json`
3. выполняет подтверждённый plan только через `terio confirm`
4. сверяет `plan_hash` preview и exact payload перед execution

Это значит, что UI/CLI confirmation больше не вызывает provider повторно для уже подтверждённого action.

## Ограничения

- Pending execution payload хранится локально в plaintext; на Unix файл ограничен `0600`, но это не заменяет disk encryption и host hardening
- Внешний JSON schema validator пока не подключён; контракт обеспечивается `response_format`, `serde` deserialization и runtime checks
- Provider context по-прежнему ограничен `request`, `cwd` и top-level entries

## Known Commands

Known command list задаётся локально в `run::is_known_command()`. Known не означает safe: final risk всё равно вычисляется локально.
