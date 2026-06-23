# Trust Model

## Risk Taxonomy

| Risk Level | Примеры | Default Policy |
|------------|---------|----------------|
| `read_only` | `ls`, `cat`, `git status` | Auto |
| `local_write` | `mkdir`, `cp`, `ffmpeg` | Confirm / auto (exact cache, >=3 success) |
| `destructive` | `rm`, `mv --overwrite` | Always confirm |
| `network_read` | `curl`, `git fetch` | Confirm (agent) / ask_once (cached per domain) |
| `network_write` | `git push`, `curl -X POST` | Always confirm |
| `credential_access` | токены, ключи | Always confirm |
| `financial` | покупки, API billing | Always confirm |

## Auto-run policy (для скриптов из кеша)

Auto-run `local_write` / `network_read` только при ВСЕХ условиях:

- exact normalized match запроса;
- success_count >= trust_threshold;
- все parameters разрешились однозначно (glob_one дал ровно один файл);
- preconditions пройдены;
- все output остаются внутри CWD или разрешённой директории;
- скрипт не содержит destructive/network_write шагов;
- пользователь не отключал auto-run в конфиге;
- предыдущий запуск скрипта был успешен.

Fuzzy match — никогда не auto-run в MVP. Только предложить и запросить подтверждение.

## Agent Safety

- **Модель только планирует.** Не исполняет.
- **План показывается.** Только после подтверждения — выполнение.
- **Secrets не отправляются модели.** Редэкция контекста до отправки.
- **Модель не получает содержимое файлов** — только имена.
- **Risk от модели — рекомендательный.** terio вычисляет финальный по command+argv.
- **Prompt injection:** содержимое файлов не отправляется. System prompt запрещает команды из файлов.

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
