# Trust Model

## Risk Taxonomy

| Risk Level | Примеры | Default Policy |
|------------|---------|----------------|
| `read_only` | `ls`, `cat`, `git status`, `pwd` | Auto |
| `local_write` | `mkdir`, `cp`, `mv`, `ffmpeg -i`, `touch` | Auto (undo available) |
| `destructive` | `rm -rf`, `mv --overwrite`, `dd`, `format` | Confirm; undo через trash |
| `network_read` | `curl url`, `wget`, `git fetch`, `apt install` | Auto (recipe: ask_once) |
| `network_write` | `git push`, `curl -X POST`, `scp` | Always confirm |
| `credential_access` | Чтение `~/.ssh/*`, `~/.env`, токенов | Always confirm, не логировать |
| `financial` | Платежи, покупки, API с billing | Always confirm |

## Undo Safety

Тривиально обратимые действия (`local_write`) выполняются без запроса пользователя, потому что terio гарантирует откат:

- **Создание/запись файлов:** записывается diff или snapshot. `terio undo` удаляет созданное / восстанавливает оригинал.
- **Удаление:** `rm` внутри terio заменяется на перемещение в `~/.terio/trash/<run_id>/`. `terio undo` восстанавливает из корзины.
- **Квота корзины:** `TERIO_TRASH_SIZE_MB` (default: 1024). При превышении — предупреждение, старые записи自动 удаляются.
- **Исключение:** network_write, credential_access, financial — undo не гарантируется, всегда запрос.

## Permission Policies

- `always_ask` — всегда спрашивать подтверждение.
- `ask_once` — спросить один раз для рецепта.
- `allow_in_dir` — разрешить для конкретной директории.
- `allow_for_recipe` — разрешить для конкретного рецепта.
- `never_allow` — никогда не разрешать автоматически.

## Confidence System

- Начальный confidence рецепта: `0.0`.
- Каждый успешный запуск: `+0.2` (до макс. 1.0).
- Каждая неудача: `-0.3`.
- Предложение рецепта: после 3 успешных выполнений похожих команд.
- Порог авто-запуска:
  - `read_only`: всегда auto.
  - `local_write`: `0.8` (4 успешных рецепта).
  - `network_read`: `0.95` (5 успешных).
  - `destructive` / `network_write` / `credential_access` / `financial`: auto только если пользователь явно изменил политику.

## Confirmation UI

Перед запуском рецепта с риском >= `destructive`:

```
Recipe: Bulk rename files
Risk: destructive (undo via trash)
Confidence: 0.8 (4 successful runs)

Commands:
  mv ./old_name.txt ./new_name.txt
  mv ./old_name2.txt ./new_name2.txt

Proceed? [Y/n] (show trace) (show undo)
```

## Trace

Пользователь может развернуть trace:

```
terio trace <run_id>
→ mkdir -p ./tracks
→ ffmpeg -i album.flac -i album.cue -map 0:0 -c copy ... ./tracks/
```

Trace показывает **реальные команды** с подставленными аргументами.

## Redaction

Следующие паттерны редэктятся из ВСЕХ полей лога (`request`, `command`, `stdout`, `stderr`, `error`):

- `Authorization: Bearer <...>`
- `api_key=`, `API_KEY=`, `api-key`
- `token=`, `TOKEN=`
- `password=`, `PASS=`, `passwd`
- `secret=`, `SECRET=`
- `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`
- `GITHUB_TOKEN`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`
- URL credentials: `https://user:pass@host` → `https://user:****@host`
- Private key blocks: `-----BEGIN ... PRIVATE KEY-----`
- Cookie/session patterns
- `--header`, `-H` с содержанием токена

Редакция case-insensitive.

## Sandbox Boundaries

- Рецепты с `shell_allowed: false` используют structured argv — command injection невозможен.
- Рецепты с `shell_allowed: true` (явно разрешённые) требуют повышенного confirmation.
- Аргументы рецепта экранируются перед подстановкой (shell quoting).
- terio не предотвращает `rm -rf /` вне рецептов — это ответственность пользователя.
- terio перехватывает `rm` внутри рецептов и заменяет на trash.

## Agent Safety

- Агент показывает сгенерированные команды перед выполнением.
- Пользователь может запретить выполнение.
- Агент не выполняет команды с risk > настраиваемого порога без явного разрешения.
- Агент логирует все свои действия (какие команды сгенерировал, почему).
