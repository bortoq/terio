# Architecture

## Core Idea

terio — агрегатор интерфейсов. В перспективе — любая программа, с которой можно обменяться действием и результатом (через CLI, API, логи или иной канал), может быть управляема из terio. В MVP — через CLI-инструменты, чьи команды можно безопасно спланировать, подтвердить, выполнить, отрендерить и закешировать.

В MVP это работает через CLI:

```
Пользователь → [Запрос] → Поиск в кеше скриптов → Найден? → Выполнить скрипт
                                                      ↓
                                                   Не найден?
                                                      ↓
                                              AI-модель строит план
                                                      ↓
                                              Показать план → Подтверждение
                                                      ↓
                                              Выполнить команды → Показать результат
                                                      ↓
                                              Сохранить как скрипт (structured chain)
```

## Компоненты

```
┌──────────────────────────────────────────────────┐
│              COMMAND SURFACE                      │
│   terio ask "..." / terio run -- <command>        │
├──────────────────────────────────────────────────┤
│              REQUEST MATCHER                      │
│   exact normalized match → script; иначе → agent  │
├──────────┬───────────────────┬───────────────────┤
│  AGENT   │  EXECUTION LAYER  │   RENDERER        │
│  (LLM)   │  (shell, process) │  (web blocks)     │
├──────────┴───────────────────┴───────────────────┤
│           SCRIPT CACHE                            │
│   normalized_request → { steps, risk, metadata }  │
│   (structured command chains, НЕ shell-скрипты)   │
├──────────────────────────────────────────────────┤
│           BEHAVIOR LOG + METRICS                  │
│   JSONL (agent_turn, command_run, script_run)     │
│   + счётчики: model_calls, cache_hits, errors    │
├──────────────────────────────────────────────────┤
│           STORAGE LAYER                           │
│   config, credentials (env/keychain), cache, log, │
│   metrics, trash (experimental undo)              │
└──────────────────────────────────────────────────┘
```

### 1. Command Surface
- Принимает: естественно-языковые запросы (`terio ask "..."`) и shell-команды (`terio run -- <command>`).
- **Инвариант:** каждый запрос проходит через Request Matcher.

### 2. Request Matcher
- Нормализует запрос (lowercase, стоп-слова, приведение).
- **MVP: только exact normalized match.**
- Fuzzy match: в будущем, только с подтверждением пользователя, никогда auto-run для local_write/destructive.
- **Ключевое:** единственное место, где terio решает, вызывать модель или нет.

### 3. Agent Layer (Built-in AI)
- Вызывается только когда Request Matcher не нашёл скрипт.
- Получает: запрос пользователя, CWD, список файлов (без содержимого).
- Возвращает: structured plan (см. docs/agent-protocol.md).
- Провайдер: локальный (ollama, llama.cpp) или удалённый (OpenAI, Anthropic).
- **Правило:** агент только планирует. Исполняет terio.
- **Правило:** агент не получает credentials (редэкция до отправки).
- **Правило:** risk от модели — рекомендательный. terio вычисляет финальный risk локально и использует более строгий.

### 4. Execution Layer
- Принимает structured command (command + argv).
- Запускает процесс, стримит stdout/stderr.
- Возвращает: exit code, stdout, stderr, duration.
- **Отмена:** Ctrl+C, таймаут, `terio cancel`.
- **Безопасность:** перед выполнением проверяется risk. Destructive/network_write → обязательное подтверждение.

### 5. Renderer
- Принимает: `ExecutionResult { stdout, stderr, exit_code, duration, command }`.
- Авто-определяет тип вывода: таблица, таймлайн, карточка, plain text.
- Показывает результат.

### 6. Script Cache
- Хранит **structured command chains** (не shell-скрипты).
- Ключ: нормализованный запрос.
- Формат (v1):
  ```json
  {
    "schema_version": 1,
    "script_id": "sha256-...",
    "normalized_request": "split flac cue album",
    "match_policy": "exact_normalized",
    "risk": "local_write",
    "parameters": {
      "flac_file": {"source": "glob_one", "pattern": "*.flac", "required": true},
      "cue_file": {"source": "glob_one", "pattern": "*.cue", "required": true},
      "output_dir": {"source": "default", "value": "./tracks"}
    },
    "preconditions": [
      {"binary_exists": "ffmpeg"},
      {"glob_one": "*.flac"},
      {"glob_one": "*.cue"}
    ],
    "steps": [
      {"command": "mkdir", "argv": ["-p", "${output_dir}"], "risk": "local_write"},
      {"command": "ffmpeg", "argv": ["-i", "${flac_file}", "-i", "${cue_file}", "-map", "0:0", "-c", "copy", "-f", "segment", "${output_dir}/track_%02d.flac"], "risk": "local_write"}
    ],
    "artifacts": [{"path_glob": "./${output_dir}/*.flac", "kind": "created_file"}],
    "success_count": 0,
    "trust_threshold": 3,
    "created_at": "2026-06-23T12:00:00Z",
    "last_used_at": "2026-06-23T12:00:00Z"
  }
  ```
- **Правило:** скрипт не выполняется, если не прошёл preconditions.
- **Правило:** после `trust_threshold` успехов и exact match → может auto-run (если риск <= local_write).

### 7. Behavior Log + Metrics
- JSONL, schema v1.
- Три вида записей: `agent_turn`, `command_run`, `script_run`.
- Секреты редэктятся из всех полей.
- **Метрики (счётчики MVP):**
  - `model_calls` — каждый вызов модели.
  - `cache_hits` — каждый успешный cache hit.
  - `tokens_consumed` — токены на вызов модели.
  - `commands_executed` — количество выполненных команд.
  - `failure_count` + `failure_kind` — ошибки.
  - `duration_total` — общая длительность.
- Агрегация: по запросу (`terio stats`), из лога.

### 8. Trust Engine
- Risk levels: read_only, local_write, destructive, network_read, network_write, credential_access, financial.
- Политики: `always_ask`, `ask_once`, `allow`.
- **Auto-run** (MVP): только exact normalized match + risk <= local_write + success_count >= trust_threshold + все parameters resolved однозначно + preconditions пройдены + все output внутри CWD или разрешённой директории + нет destructive/network_write шагов + пользователь не отключал auto-run + предыдущий запуск был успешен в эквивалентном контексте.
- **Fuzzy match:** никогда не auto-run в MVP. Только предложить и спросить.
- **Model risk:** рекомендательный. terio вычисляет финальный risk по команде.

### 9. Undo/Redo (Experimental)
- **Не гарантируется.** Best-effort для кешированных скриптов.
- Два режима (в конфиге): sandbox (bubblewrap) или warn-only.
- По умолчанию: выключен.

## Risk Taxonomy (MVP)

| Risk Level | Примеры | Default Policy |
|------------|---------|----------------|
| `read_only` | `ls`, `cat`, `git status` | Auto |
| `local_write` | `mkdir`, `cp`, `ffmpeg` | Confirm / auto (exact cache match, >=3 success) |
| `destructive` | `rm`, `mv --overwrite` | Always confirm |
| `network_read` | `curl`, `git fetch` | Confirm (agent) / ask_once (cached per domain) |
| `network_write` | `git push`, `curl -X POST` | Always confirm |
| `credential_access` | токены, ключи | Always confirm, не логировать |
| `financial` | покупки, API billing | Always confirm |

## Data Flow (MVP)

1. `terio ask "split this flac/cue album"` → Command Surface → Request Matcher.
2. Matcher ищет exact normalized match в Script Cache. Найден и trust >= threshold → шаг 7.
3. Не найден или trust < threshold → Agent Layer (LLM).
4. Агент возвращает structured plan. terio проверяет risk и показывает план.
5. Пользователь подтверждает. Execution Layer выполняет.
6. Renderer показывает результат. Log пишет. Metrics обновляются.
7. Цепочка сохраняется в Script Cache (с параметрами и preconditions).
8. В следующий раз: exact match → скрипт выполняется без модели.

## Agent Protocol (MVP)

См. [docs/agent-protocol.md](docs/agent-protocol.md) — полный контракт.

Кратко:
- Вход: `{ request, cwd, files[], allowed_risks[] }` (secrets redacted).
- Выход: `{ summary, risk, commands: [{ command, argv, risk, reason }], cache_template? }`.
- terio проверяет: JSON валиден, команды в known_commands, risk не занижен.
- Финальный risk = max(model.risk, terio.computed_risk).

## Логирование

- Логируется **каждый ввод пользователя**: `terio ask "..."` и `terio run -- ...`.
- Логируется **каждый шаг выполнения**: команда, exit code, duration, stdout summary.
- Логируется **каждый вызов модели**: провайдер, токены, статус.
- **Секреты не логируются** — редэкция до записи.
- Всё хранится в JSONL. Агрегация — по запросу.

## Stack

- **Язык:** Rust.
- **CLI:** clap.
- **Shell:** duct / std::process::Command.
- **Renderer:** HTML (webview или браузер).
- **Log:** serde_json + JSONL.
- **Script Cache:** SQLite или JSON.
- **Agent:** HTTP client к LLM (openai, anthropic, ollama).

## Экономическая модель: разделение стоимости внимания

terio работает с вычислительной системой (в смысле проекта «address space»): пользователь + агент (ИИ) + ОС + программы. Каждый участник потребляет внимание, стоимость которого разная.

### Типы стоимости внимания

| Тип | Пример | Относительная стоимость | Счётчик |
|-----|--------|------------------------|---------|
| **Внимание пользователя** | чтение вывода, подтверждение, выбор | Высокая (70–90% общей стоимости) | `attention_cost_user` |
| **Внимание агента (LLM)** | планирование, анализ ошибок | Средняя (8–25%) | `attention_cost_agent` |
| **Внимание ОС + программ** | выполнение команд, ввод-вывод | Низкая (0.5–2%) | `attention_cost_system` |

### Метрика стоимости

Каждый тип можно выразить в единицах времени пользователя (`user-seconds`). Тогда:

```
total_attention_cost = 
  t_user × c_user + 
  t_agent × c_agent + 
  t_system × c_system
```

где:
- `t_user` — время, потраченное пользователем на ввод/чтение/выбор;
- `c_user = 1.0` (нормировка — стоимость одной секунды пользователя);
- `c_agent = 0.05–0.3` (стоимость секунды агента, зависит от провайдера);
- `c_system = 0.001` (стоимость секунды ОС/программ — пренебрежимо мала).

### Как это используется

- **MVP:** раздельные счётчики в `terio stats`: `user_attention_sec`, `agent_attention_sec`, `system_attention_sec`.
- **Phase 3+:** оптимизация маршрута запроса. Если cache hit снижает `total_attention_cost` — terio предпочитает кеш. Если модель быстрее справится с复杂的 запросом — terio может предложить модель.
- **Цель:** минимизировать эксплуатационную стоимость вычислительной системы (пользователь + агент + ОС + программы), которую пользователь использует для выполнения своих запросов.

Пример:
```
Запрос: "list files"
  Первый раз (agent):  user=5s, agent=2s, system=0.1s → cost = 5×1.0 + 2×0.15 + 0.1×0.001 = 5.30
  Повторный (cache):   user=1s, agent=0s, system=0.1s → cost = 1×1.0 + 0 + 0.1×0.001 = 1.0001
  Экономия: ~81%
```

### Раздельные счётчики в логе

Каждая запись `command_run` и `script_run` содержит:
```json
{
  "kind": "script_run",
  "attention_cost": {
    "user_sec": 1.2,
    "agent_sec": 0.0,
    "system_sec": 0.05
  }
}
```

Это позволяет агрегировать `terio stats` по типам внимания и видеть, сколько каждого ресурса потребляет terio.

## Автоматическая интеграция программ (Vision)

В будущем terio сможет самостоятельно интегрировать новые программы. Процесс:

### 1. Идентификация программы
Пользователь говорит: "научись работать с yt-dlp". Агент находит yt-dlp в системе (`which`), проверяет версию (`--version`).

### 2. Проверка доступности интерфейса
Агент читает man-страницу, `--help`, вики или документацию (если чтение документации слишком дорого — агент summarises в несколько запросов). Если документация недоступна или слишком объёмна — агент пробует интерактивно: "yt-dlp --help" → анализирует флаги.

### 3. Написание интеграционного скрипта
Агент пишет structured command chain (не shell-скрипт) для terio:
- параметры (glob_one, default, url);
- preconditions (binary_exists);
- шаги (command + argv);
- risk classification по командам.

Пользователь не участвует — агент делает это автоматически.

### 4. Проверка (тестирование) скрипта
terio прогоняет тесты с минимальным влиянием:
- `--dry-run` или `--version` для проверки что команда вызывается корректно;
- проверка передачи данных между terio и программой (stdout/stderr/exit code);
- если тесты успешны — скрипт сохраняется в Script Cache как интеграционный.

### 5. Готово
Программа считается интегрированной: все будущие запросы к ней кешируются и выполняются без модели.

**Правило:** интеграционный скрипт не выполняется, пока не пройдёт тесты.
**Правило:** пользователь может отклонить интеграцию, если тесты выглядят подозрительно.

Это встроено в фазы Roadmap 5+ (ленивые интеграции).

## Key Design Rule

Пользователь работает. terio запоминает. Модель вызывается только когда нужно впервые. В перспективе — любая программа с отсоединяемым интерфейсом может быть управляема из terio. В MVP — через CLI-инструменты, чьи команды можно безопасно спланировать, подтвердить, выполнить, отрендерить и закешировать.
