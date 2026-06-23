# Trust Model

## Risk Taxonomy

| Risk Level | Примеры | Default Policy |
|------------|---------|----------------|
| `read_only` | `ls`, `cat`, `git status` | Auto |
| `local_write` | `mkdir`, `cp`, `ffmpeg` | Confirm / auto (exact cache, >=3 success) |
| `destructive` | `rm`, `mv --overwrite` | Always confirm |
| `network_read` | `curl`, `git fetch` | Confirm / auto (exact cache) |
| `network_write` | `git push`, `curl -X POST` | Always confirm |
| `credential_access` | токены, ключи | Always confirm |
| `financial` | покупки, API billing | Always confirm |

## Agent Safety

- **Модель только планирует.** Не исполняет.
- **План показывается.** Только после подтверждения — выполнение.
- **Secrets не отправляются модели.** Редэкция контекста до отправки.
- **Модель не получает содержимое файлов** — только имена.
- **Risk от модели — рекомендательный.** terio вычисляет финальный и использует более строгий.
- **Prompt injection:** содержимое файлов не отправляется. System prompt запрещает команды из файлов.

## Script Cache Safety

- Exact match только. Fuzzy match не в MVP.
- Скрипты с risk >= destructive всегда требуют подтверждения.
- Preconditions проверяются перед каждым запуском.
- Auto-run: exact match + risk <= local_write + success >= trust_threshold.

## Undo/Redo (Experimental)

- Off by default.
- Не гарантируется для произвольных shell.
- Best-effort для скриптов из кеша.
- Два режима: sandbox (bubblewrap) или warn.

## Redaction

Перед отправкой в модель и перед записью в лог:

- `Authorization: Bearer <...>`
- `api_key=`, `API_KEY=`, `token=`, `TOKEN=`
- `password=`, `PASS=`, `secret=`, `SECRET=`
- `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`
- `GITHUB_TOKEN`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`
- URL credentials: `https://user:****@host`
- Private keys: `-----BEGIN ... PRIVATE KEY-----`
- `--header`/`-H` с токеном

Case-insensitive. Из всех полей: request, context, command, stdout, stderr.

## Поведение при ошибке скрипта

Если скрипт из кеша упал:
1. terio показывает ошибку и exit code.
2. Спрашивает: "Call model with error context? [Y/n]".
3. Если да — модель получает запрос + ошибку и предлагает исправленный план.
