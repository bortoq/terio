# Architecture

## Core Idea

terio — агрегатор интерфейсов. В перспективе — любая программа, с которой можно обменяться действием и результатом (через CLI, API, логи или иной канал), может быть управляема из terio. В MVP — через CLI-инструменты, чьи команды можно безопасно спланировать, подтвердить, выполнить, отрендерить и закешировать.

**Архитектурный принцип:** terio минимизирует совокупную стоимость эксплуатации вычислительной системы (пользователь + агент + ОС + программы). Учёт расходов и оптимизация — сквозная функция всех компонентов. В MVP учёт — сырые счётчики; формулы минимизации — в будущем.

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
│   interaction_id генерируется здесь               │
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
│           LOG (WRITER + READER)                   │
│   JSONL writer + in-memory event stream           │
│   LogWriter trait → JsonlLogWriter (MVP)          │
│   LogReader trait → JsonlLogReader (MVP)          │
│   Renderer подписан на LogEventStream             │
├──────────────────────────────────────────────────┤
│           ACCOUNTING                              │
│   cost_counters в каждой записи лога              │
│   aggregation + заглушка compute_attention_cost   │
├──────────────────────────────────────────────────┤
│           STORAGE + IDENTITY                      │
│   config, credentials (env/keychain), cache, log, │
│   metrics, trash (experimental undo)              │
│   instance_id + session_id                        │
└──────────────────────────────────────────────────┘
```

### 1. Command Surface
- Принимает: естественно-языковые запросы (`terio ask "..."`) и shell-команды (`terio run -- <command>`).
- **Генерирует `interaction_id` (UUID)** для каждого запроса пользователя. Все последующие записи в логе (agent_turn, command_run, script_run) получают этот interaction_id.
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
- **Читает из лога, а не из ExecutionResult.** Renderer подписан на `LogEventStream` (in-memory канал) и получает новые записи сразу после записи, до записи на диск.
- Для отображения истории загружает лог через `LogReader.recent()` (streaming, без загрузки всего файла в память).
- Авто-определяет тип вывода: таблица, таймлайн, карточка, plain text.
- Учитывает `display_profile` записи (см. Логирование): `type` (auto/text/table/media/hidden/summary), `renderer_hint`, `user_visible`.
- Принимает: `Vec<LogEntry>` из лога.

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

### 7. Log (Writer + Reader)

Лог — центральный компонент, связывающий выполнение и отображение.

#### LogWriter trait (MVP — JsonlLogWriter)

```rust
pub trait LogWriter: Send + Sync {
    fn append(&self, entry: LogEntry) -> Result<()>;
    fn flush(&self) -> Result<()>;
}
```

- `JsonlLogWriter` пишет в `~/.terio/log/terio-YYYY-MM.jsonl`.
- Перед записью на диск отправляет запись в `LogEventStream` (in-memory broadcast channel).
- Только append, никаких удалений/изменений.

#### LogReader trait (MVP — JsonlLogReader)

```rust
pub trait LogReader: Send + Sync {
    fn recent(&self, n: usize) -> Result<Vec<LogEntry>>;
    fn by_session(&self, session_id: &str) -> Result<Vec<LogEntry>>;
    fn by_interaction(&self, interaction_id: &str) -> Result<Vec<LogEntry>>;
    fn stream(&self) -> Receiver<LogEntry>;
}
```

- `JsonlLogReader` читает JSONL потоком (streaming deserialization), не загружая весь файл в память.
- `stream()` возвращает `tokio::sync::broadcast::Receiver<LogEntry>` — renderer подписывается один раз и получает новые записи без I/O.
- `by_interaction` — группировка записей по одному пользовательскому запросу (см. `interaction_id`).

#### LogEntry

```json
{
  "schema_version": 1,
  "instance_id": "01JAN3XINSTANCE001",
  "session_id": "uuid",
  "interaction_id": "uuid",
  "ts": "2026-06-23T10:00:00Z",
  "kind": "agent_turn|command_run|script_run|system_event",
  "display_profile": {
    "type": "auto|text|table|media|hidden|summary",
    "renderer_hint": "auto|table|plain|timeline|card",
    "user_visible": true,
    "summary_max_lines": 10
  },
  "cost_counters": {
    "observation_cost_hint": { "user_sec": 0.0 },
    "llm_cost": { "tokens": 0, "duration_ms": 0 },
    "execution_cost": { "duration_ms": 6, "commands_executed": 1, "bytes_read": 0, "bytes_written": 120 },
    "cache_cost": { "lookup_ms": 0, "hit": false },
    "storage_cost": { "bytes_written": 120, "bytes_read": 0 }
  },
  // ... поля специфичные для kind
}
```

**Ключевые поля:**
- `instance_id` — уникальный ID экземпляра terio (генерируется при первом запуске, хранится в `~/.terio/instance.json`).
- `session_id` — ID сессии (от запуска до закрытия). Позволяет отделить одну сессию работы от другой.
- `interaction_id` — ID одного пользовательского запроса. Группирует все записи, относящиеся к одному взаимодействию.
- `display_profile` — как показывать эту запись пользователю. Настраивается через `terio config`.
- `cost_counters` — сырые счётчики расходов. Используются Accounting для агрегации.

### 8. Accounting

Выделенный компонент для сбора и агрегации cost_counters.

**MVP:**
- `cost_counters` — поле в каждой записи лога (см. LogEntry).
- `fn aggregate(counters: &[CostCounters]) -> AggregatedCosts` — сумма по типам.
- `fn compute_attention_cost(counters: &CostCounters) -> f64` — заглушка, возвращает 0.0.
- `terio stats` показывает сумму cost_counters.

**Future:**
- Реальные веса для `compute_attention_cost`.
- `total_attention_cost` как единственная метрика для оптимизации.
- Выбор маршрута (cache vs model) на основе стоимости.

### 9. Trust Engine
- Risk levels: read_only, local_write, destructive, network_read, network_write, credential_access, financial.
- Политики: `always_ask`, `ask_once`, `allow`.
- **Auto-run** (MVP): только exact normalized match + risk <= local_write + success_count >= trust_threshold + все parameters resolved однозначно + preconditions пройдены + все output внутри CWD или разрешённой директории + нет destructive/network_write шагов + пользователь не отключал auto-run + предыдущий запуск был успешен в эквивалентном контексте.
- **Fuzzy match:** никогда не auto-run в MVP. Только предложить и спросить.
- **Model risk:** рекомендательный. terio вычисляет финальный risk по команде.

### 10. Undo/Redo (Experimental)
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

1. `terio ask "split this flac/cue album"` → Command Surface генерирует `interaction_id`.
2. → Request Matcher ищет exact normalized match в Script Cache.
3. **Найден и trust >= threshold:** Script Cache → Execution Layer → Log (Writer → in-memory канал + JSONL) → Renderer (читает из LogEventStream).
4. **Не найден или trust < threshold:** → Agent Layer (LLM) → план → подтверждение → Execution Layer → Log → Renderer.
5. Renderer получает `LogEntry` через `LogReader.stream()` и показывает.
6. Account собирает `cost_counters` из лога. `terio stats` агрегирует.

## Agent Protocol (MVP)

См. [docs/agent-protocol.md](docs/agent-protocol.md) — полный контракт.

Кратко:
- Вход: `{ request, cwd, files[], allowed_risks[] }` (secrets redacted).
- Выход: `{ summary, risk, commands: [{ command, argv, risk, reason }], cache_template? }`.
- terio проверяет: JSON валиден, команды в known_commands, risk не занижен.
- Финальный risk = max(model.risk, terio.computed_risk).

## Identity

### instance_id
- ULID, генерируется при первом запуске.
- Хранится в `~/.terio/instance.json`.
- Не меняется всё время жизни экземпляра.
- Используется в логе (`instance_id`), в будущем — для связи между экземплярами и шэринга.

### session_id
- UUID, генерируется при каждом запуске terio.
- Пишется в каждую запись лога.
- Позволяет отделить одну сессию от другой.
- При возобновлении работы: terio читает последнюю session_id из лога и показывает «доступна предыдущая сессия».

## Логирование

- **LogWriter trait:** `append(LogEntry)`. Реализация в MVP — `JsonlLogWriter`.
- **LogReader trait:** `recent(n)`, `by_session(id)`, `by_interaction(id)`, `stream()`.
- **Типизация записей:** каждая запись лога имеет `display_profile`:
  - `type: auto` — terio определяет сам (по умолчанию).
  - `type: text|table|media|hidden|summary` — принудительно.
  - `user_visible: true|false` — скрыть от показа.
  - `summary_max_lines` — для `type: summary`.
- Пользователь настраивает отображаемость каждого `kind` через `terio config`.
- Агрегация — по запросу (`terio stats`).
- Всё хранится в JSONL (MVP). При смене формата — новая реализация `LogWriter`/`LogReader`, остальной код не меняется.

## Stack

- **Язык:** Rust.
- **CLI:** clap.
- **UI:** Dioxus (webview). Терминальный fallback — ratatui (опционально).
- **Плееры:** внешние (`mpv`, `vlc`) или HTML5 `<video>` в webview.
- **Shell:** duct / std::process::Command.
- **Log:** serde_json + JSONL.
- **Script Cache:** JSON.
- **Agent:** HTTP client к LLM (openai, anthropic, ollama).
- **Accounting:** встроенный модуль, заглушка для формулы стоимости.

## Экономическая модель: разделение стоимости внимания

terio работает с вычислительной системой (в смысле проекта «address space»): пользователь + агент (ИИ) + ОС + программы. Каждый участник потребляет внимание, стоимость которого разная.

### Типы стоимости внимания

| Тип | Пример | Относительная стоимость | Счётчик |
|-----|--------|------------------------|---------|
| **Внимание пользователя** | чтение вывода, подтверждение, выбор | Высокая (70–90% общей стоимости) | `observation_cost_hint.user_sec` |
| **Внимание агента (LLM)** | планирование, анализ ошибок | Средняя (8–25%) | `llm_cost.tokens`, `llm_cost.duration_ms` |
| **Внимание ОС + программ** | выполнение команд, ввод-вывод | Низкая (0.5–2%) | `execution_cost.duration_ms` |
| **Кеш** | поиск в кеше | Очень низкая | `cache_cost.lookup_ms` |
| **Хранение** | запись/чтение лога | Пренебрежимо мала | `storage_cost.bytes_written/read` |

### Метрика стоимости (future)

```
total_attention_cost = 
  observation_cost_hint × w_user + 
  llm_cost × w_agent + 
  execution_cost × w_system + 
  cache_cost × w_cache + 
  storage_cost × w_storage
```

В MVP `compute_attention_cost` — заглушка, возвращает 0.0. Счётчики копятся в логе, готовые к использованию, когда появятся веса.

### Как это используется (future)

- Выбор маршрута: cache vs model на основе `total_attention_cost`.
- Предсказание: pre-execution, если ожидаемая стоимость низкая.
- Отчёт `terio cost` с реальными весами.

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

**Архитектурная готовность к будущему:**
- `LogWriter`/`LogReader` traits — смена формата лога без рефакторинга.
- `interaction_id` — группировка пар (запрос → ответ) для любых типов взаимодействия.
- `display_profile` — типизация записей лога с настраиваемой отображаемостью.
- `cost_counters` — сырые счётчики для будущей метрики `total_attention_cost`.
- `instance_id` + `session_id` — идентификация для шэринга и возобновления.
- Renderer читает из лога — лог как единый источник для UI.
