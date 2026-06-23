# Trust Model

## Risk Taxonomy

| Risk Level | Примеры | Default Policy |
|------------|---------|----------------|
| `read_only` | `ls`, `cat`, `git status`, `pwd` | Auto |
| `local_write` | `mkdir`, `cp`, `ffmpeg`, `touch` | Confirm (первый раз) / Auto (скрипт, 3+ успеха) |
| `destructive` | `rm`, `mv --overwrite`, `dd` | Always confirm |
| `network_read` | `curl`, `wget`, `git fetch` | Confirm (первый раз) / Auto (скрипт) |
| `network_write` | `git push`, `curl -X POST`, `scp` | Always confirm |
| `credential_access` | Чтение `~/.ssh/*`, `.env`, токенов | Always confirm |
| `financial` | Платежи, API billing | Always confirm |

## Agent Safety

- **Модель не исполняет команды.** Модель только предлагает план. Исполняет terio.
- **План показывается пользователю.** Только после подтверждения выполняется.
- **Secrets не отправляются модели.** Перед отправкой запроса в LLM terio сканирует и редэктит контекст (файлы, CWD, переменные).
- **Agent не имеет доступа к credentials.** Ни к токенам, ни к API-ключам.
- **Prompt injection:** модель может получить вредоносные данные из файлов в CWD. terio логирует промпт и ответ для аудита.

## Script Cache Safety

- Скрипты не выполняются, если прекондишены не пройдены (нет команды, нет файлов).
- Destructive/network_write скрипты всегда требуют подтверждения, даже если в кеше.
- После N успехов (default: 3) `local_write` скрипты могут авто-запускаться.

## Undo/Redo (Experimental)

- **По умолчанию выключен.** Включается в `terio config`.
- **Не гарантируется** для произвольных shell-команд.
- **Best-effort** для кешированных скриптов: snapshot изменённых файлов до выполнения.
- Два режима:
  - `sandbox` — изоляция через bubblewrap/overlay FS. Безопасно, но ресурсоёмко.
  - `warn` — только предупреждение. Быстро, но рискованно.

## Confirmation UI

```
 terio plan (2 steps)
  1. mkdir -p ./tracks                    local_write
  2. ffmpeg -i album.flac ...             local_write
  Risk: local_write
  Proceed? [Y/n] (show full trace)
```

Для destructive:

```
 terio plan (1 step)
  1. rm -rf ./old_project                 destructive
  ⚠ This action cannot be fully undone (undo is experimental).
  Are you sure? [y/N]
```

## Redaction

Перед отправкой в модель и перед записью в лог редэктятся:

- `Authorization: Bearer <...>`
- `api_key=`, `API_KEY=`
- `token=`, `TOKEN=`
- `password=`, `PASS=`
- `secret=`, `SECRET=`
- `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`
- `GITHUB_TOKEN`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`
- URL credentials: `https://user:****@host`
- Private key blocks: `-----BEGIN ... PRIVATE KEY-----`
- `--header`/`-H` с содержанием токена

Case-insensitive. Применяется ко всем полям: request, context, command, stdout, stderr.

## Behavior Log Redaction

Лог не содержит:
- сырые credentials (редэктируются до записи);
- полный stdout для секретных команд (если риск = credential_access, stdout не пишется);
- prompt целиком (только summary).
