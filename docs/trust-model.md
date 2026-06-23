# Trust Model

## Risk Taxonomy

| Risk Level | Examples | Default Policy |
|------------|---------|----------------|
| `read_only` | `ls`, `cat`, `git status`, `pwd` | Auto — без подтверждения |
| `local_write` | `mkdir`, `cp`, `mv`, `ffmpeg -i`, `touch` | Confirm per recipe при первом запуске |
| `destructive` | `rm -rf`, `mv --overwrite`, `dd`, `format` | Всегда запрос |
| `network_read` | `curl`, `wget`, `git fetch`, `apt install` | Auto (в рецепте — confirm) |
| `network_write` | `git push`, `curl -X POST`, `scp` | Всегда запрос |
| `credential_access` | Чтение `~/.ssh/*`, `~/.env`, токенов | Всегда запрос, не логировать |
| `financial` | Платежи, покупки, API с billing | Всегда запрос |

## Permission Policies

Пользователь может настроить политику для каждого risk level:

- `always_ask` — всегда спрашивать.
- `ask_once` — спросить один раз для рецепта, запомнить решение.
- `allow_in_dir` — разрешить в указанной директории.
- `allow_for_recipe` — разрешить для конкретного рецепта.
- `never_allow` — никогда не разрешать автоматически.

## Confidence System

- Начальный confidence рецепта: `0.0`.
- Каждый успешный запуск: `+0.2` (до макс. 1.0).
- Каждая неудача: `-0.3`.
- Порог предложения рецепта: `0.0` (после 3 успешных выполнений без рецепта → предложить).
- Порог авто-запуска: `0.8` (для `local_write`), `0.95` (для `network_read`).
- `read_only` всегда auto.

## Confirmation UI (MVP)

Перед запуском рецепта с risk >= `local_write`:

```
Recipe: Split FLAC/CUE album
Risk: local_write
Confidence: 0.8 (4 successful runs)

Arguments:
  flac_file: /home/user/album.flac
  cue_file:  /home/user/album.cue
  output:    ./tracks

Commands:
  mkdir -p ./tracks
  ffmpeg -i /home/user/album.flac ... 

Proceed? [Y/n] (show trace)
```

## Trace

Пользователь может в любой момент развернуть trace и увидеть реальные команды с подставленными аргументами:

```
terio trace split_flac_cue
→ mkdir -p /home/user/music/tracks
→ ffmpeg -i /home/user/music/album.flac -f segment ...
```

## Redaction

Следующие паттерны редэктятся из лога:
- `Authorization: Bearer *`
- `api_key=*`
- `token=*`
- `password=*`
- `secret=*`
- Файлы `.env`, `.netrc`, `~/.ssh/*`

## Sandbox Boundaries

- Рецепты не могут содержать произвольный shell code — только параметризованные шаги.
- Аргументы экранируются через `shell_escape` перед подстановкой.
- Команды вне рецепта выполняются как обычный shell (пользователь явно их ввёл).
- terio не предотвращает `rm -rf /` — это ответственность пользователя.
