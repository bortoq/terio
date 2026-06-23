# Use Cases

## Принцип

Все use cases работают одинаково: пользователь описывает задачу на естественном языке, terio исполняет и запоминает. Повторные запросы — без модели.

## FLAC/CUE Album Split

Пользователь: `terio ask "split this flac/cue album into tracks"`. Агент генерирует ffmpeg, пользователь подтверждает. После первого успеха — скрипт в кеше.

## Missing Episode

`terio ask "what episodes are missing from season 3 of this show?"`. Агент запускает `ls`/`find`, сравнивает, показывает таблицу. Потом: `terio ask "download episode 3x07"`. Агент генерирует curl/yt-dlp.

## Resume Playlist

`terio ask "play my last mpv playlist"`. Агент находит конфиг mpv, запускает `mpv --playlist=...`, terio показывает now-playing.

## GitHub Issues

`terio ask "show my open issues in bortoq/terio"`. Агент запускает `gh issue list`, terio рендерит карточки. В следующий раз — без модели.

## File Copy

`terio run -- rsync -av --progress /src /dst`. terio парсит вывод, показывает прогресс-бар.

## News Reading

`terio ask "latest AI news"`. Агент запускает `curl` к RSS, форматирует как читаемую страницу.

## Почему это работает

terio не требует заранее подключать «коннекторы» для GitHub, медиа или браузера. Всё, что нужно — это CLI-инструменты (`gh`, `mpv`, `curl`, `ls`, `ffmpeg`). Пользователь описывает задачу, модель строит цепочку, terio запоминает.
