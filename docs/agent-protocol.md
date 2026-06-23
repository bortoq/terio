# Agent Protocol (MVP)

## Purpose

Формализовать контракт между terio и AI-моделью: вход, выход, валидация.

## Input (от terio к модели)

```json
{
  "request": "list files in current directory",
  "cwd": "/home/user/projects/terio",
  "files": ["README.md", "architecture.md", "src/", "docs/"],
  "allowed_risks": ["read_only", "local_write"],
  "os": "linux",
  "shell": "bash"
}
```

**Правила:**
- `files` — только имена. Содержимое файлов не отправляется (защита от prompt injection).
- `allowed_risks` — terio сообщает модели, какие риски допустимы. Модель не может их расширить.
- Secrets/Credentials никогда не попадают в input.

## Output (от модели к terio)

```json
{
  "summary": "List files using ls with details",
  "risk": "read_only",
  "commands": [
    {
      "command": "ls",
      "argv": ["ls", "-l"],
      "risk": "read_only",
      "reason": "Shows detailed file listing suitable for table rendering"
    }
  ],
  "expected_output": "file listing with permissions, size, date, name"
}
```

**Правила:**
- `commands` — массив structured команд (command + argv).
- `risk` per command — рекомендательный. terio вычисляет финальный.
- `reason` — объяснение для confirmation UI.

## Validation (terio проверяет выход модели)

1. JSON валиден, поля присутствуют.
2. `command` в allow list (безопасные команды: ls, cat, mkdir, cp, ffmpeg, git, curl...).
3. `argv` — массив строк, не пустой.
4. `risk` не ниже минимального для данной команды (если модель сказала `read_only`, а команда `rm` — terio повышает до `destructive`).
5. Нет shell injection (каждый argv — отдельный аргумент, без конкатенации).

## Risk Re-computation

Финальный risk = `max(model.risk, terio.computed_risk(command, argv))`.

Пример:
- Модель: `{"command": "rm", "argv": ["rm", "-rf", "/tmp/x"], "risk": "read_only"}`
- terio.computed_risk("rm", ["rm", "-rf", "/tmp/x"]) = `destructive`
- Финальный: `max("read_only", "destructive")` = `destructive`

## Confirmation UI

Для каждого запроса:

```
 terio plan
  1. ls -l  (read_only)
  Risk: read_only
  Proceed? [Y/n]
```

Для destructive:

```
 terio plan
  1. rm -rf /tmp/x  (destructive)
  ⚠ This action is destructive.
  Are you sure? [y/N]
```
