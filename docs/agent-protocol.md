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
- `files` — только имена. Содержимое не отправляется (защита от prompt injection).
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
  "expected_output": "file listing with permissions, size, date, name",
  "cache_template": {
    "parameters": {
      "dir": {"source": "default", "value": "."},
      "flags": {"source": "default", "value": "-l"}
    },
    "preconditions": [
      {"binary_exists": "ls"}
    ],
    "artifacts": []
  }
}
```

**Правила:**
- `commands` — массив structured команд (command + argv).
- `risk` per command — рекомендательный. terio вычисляет финальный.
- `reason` — объяснение для confirmation UI.
- `cache_template` — опциональный. Если передан, terio валидирует и использует для Script Cache. Если нет — terio сохраняет фиксированный plan.

## Validation (terio проверяет выход модели)

1. JSON валиден, поля присутствуют.
2. `command` в allow list.
3. `argv` — массив строк, не пустой.
4. `risk` не ниже минимального для данной команды (см. Risk Rules ниже).
5. Shell injection: argv не конкатенируется, каждый аргумент отдельно.
6. `cache_template` (если есть): parameters валидны, preconditions выполнимы, risk совпадает с общим.

## Risk Rules (по command + argv)

Безопасность зависит не только от command, но и от argv:

| Команда | Опасные argv | Правильный risk |
|---------|-------------|-----------------|
| `cat` | `~/.ssh/id_rsa`, `.env`, токены | `credential_access` |
| `curl` | `-X POST`, `-d`, `--data` | `network_write` |
| `curl` | любой URL | `network_read` |
| `git push` | любой | `network_write` |
| `git clean`, `git reset --hard` | любой | `destructive` |
| `cp` | в `/etc/`, `/usr/` | `destructive` |
| `ffmpeg` | `-i http://...` | `network_read` |
| `rm`, `mv` | любой | `destructive` |
| `sudo` | любой | `destructive` (повышенный) |

Финальный risk = `max(model.risk, terio.computed_risk(command, argv))`.

Пример:
- Модель: `{"command": "rm", "argv": ["rm", "-rf", "/tmp/x"], "risk": "read_only"}`
- terio.computed_risk("rm", ["rm", "-rf", "/tmp/x"]) = `destructive`
- Финальный: `max("read_only", "destructive")` = `destructive`

## Allow List (MVP)

`ls`, `cat`, `mkdir`, `cp`, `mv`, `rm`, `ffmpeg`, `git`, `curl`, `wget`, `mpv`, `rsync`, `docker`, `gh`, `echo`, `printf`, `shasum`, `find`, `grep`, `awk`, `sort`, `uniq`, `head`, `tail`, `wc`, `date`, `pwd`, `which`.

Команды вне allow list требуют дополнительного подтверждения и блокируются для auto-run.

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
