# Use Cases

## Принцип

Все use cases работают одинаково: пользователь описывает задачу на естественном языке, terio исполняет и запоминает. Любая программа с CLI или API — из одной точки.

## FLAC/CUE Album Split

`terio ask "split this flac/cue album into tracks"`. Агент генерирует ffmpeg, пользователь подтверждает. После первого успеха — скрипт в кеше. В следующий раз — без модели.

## Missing Episode

`terio ask "what episodes are missing from season 3?"`. Агент запускает `ls`/`find`, сравнивает, показывает таблицу. `terio ask "download episode 3x07"` — агент генерирует curl/yt-dlp.

## Resume Playlist

`terio ask "play my last mpv playlist"`. Агент находит конфиг mpv, запускает `mpv --playlist=...`, terio показывает now-playing.

## GitHub Issues

`terio ask "show my open issues in bortoq/terio"`. Агент запускает `gh issue list`, terio рендерит карточки. В следующий раз — без модели.

## File Copy

`terio run -- rsync -av --progress /src /dst`. terio парсит вывод, показывает прогресс-бар.

## News

`terio ask "latest AI news"`. Агент запускает `curl` к RSS, форматирует как читаемую страницу.

## Почему это работает

terio — агрегатор интерфейсов. Неважно, как программа предоставляет интерфейс: CLI, API, логи. Если есть возможность передать действие и получить результат, terio умеет интегрировать. В MVP — через CLI. В будущем — через любые каналы.
