# Architecture

## Core Idea

terio — агрегатор интерфейсов. В перспективе — любая программа, с которой можно обменяться действием и результатом (через CLI, API, логи или иной канал), может быть управляема из terio. В MVP — через CLI-инструменты, чьи команды можно безопасно спланировать, подтвердить, выполнить, отрендерить и закешировать.

terio считает, сколько ресурсов сэкономлено: избегнутых вызовов модели, сэкономленных токенов, переиспользованных команд, затраченного времени. Архитектурный принцип: учёт расходов встроен в каждую запись лога с самого начала.

**UI с первого коммита:** terio — это оконное приложение (одно окно, Dioxus webview). Основной путь взаимодействия — `terio ask` с UI. Shell-команды (`terio run --`) доступны для обратной совместимости.

В MVP это работает через CLI + UI:

```
Пользователь → [Запрос через UI/CLI] → Поиск в кеше скриптов → Найден? → Выполнить скрипт
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
│  (LLM)   │  (shell, process) │  (Dioxus webview) │
├──────────┴───────────────────┴───────────────────┤
│           SCRIPT CACHE                            │
│   normalized_request → { steps, risk, metadata }  │
│   (structured command chains, НЕ shell-скрипты)   │
├──────────────────────────────────────────────────┤
│           LOG (LogStore)                          │
│   LogWriter trait → JsonlLogWriter (MVP)          │
│   LogReader trait → JsonlLogReader (MVP)          │
│   LogEventStream (broadcast после записи в буфер) │
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

### 5. Renderer (Dioxus webview)
- **Основной UI terio — Dioxus webview.** Одно окно, в котором отображается лог взаимодействий.
- **Читает из лога, а не из ExecutionResult.** Renderer подписан на `LogEventStream` (in-memory broadcast channel) и получает новые записи **после** записи в буфер LogWriter.
- Для отображения истории загружает лог через `LogReader.recent()` (для MVP — линейное сканирование).
- Авто-определяет тип вывода на основе `display_profile`.
- `display_profile` — **только подсказка для UI, не security boundary**.

### 6. Script Cache
- Хранит **structured command chains** (не shell-скрипты).
- Ключ: нормализованный запрос.
- **Правило:** скрипт не выполняется, если не прошёл preconditions.
- **Правило:** после `trust_threshold` успехов и exact match → может auto-run (если риск <= local_write).

### 7. Log (LogStore)

Лог — центральный компонент, связывающий выполнение и отображение.

#### LogWriter trait (MVP — JsonlLogWriter)

```rust
pub trait LogWriter: Send + Sync {
    fn append(&self, entry: LogEntry) -> Result<()>;
    fn flush(&self) -> Result<()>;
}
```

Порядок append:
1. Validate — проверка на соответствие schema.
2. Redact — удаление секретов из всех полей.
3. Write — запись в `BufWriter` (буфер в памяти, без fsync).
4. Broadcast — отправка в LogEventStream (только после успешной записи в буфер).

`flush()` — fsync на диск. Между `append` и `flush` запись есть в буфере и stream, но не гарантирована на диске.

#### LogReader trait (MVP — JsonlLogReader)

```rust
pub trait LogReader: Send + Sync {
    fn recent(&self, n: usize) -> Result<Vec<LogEntry>>;
    fn by_session(&self, session_id: &str) -> Result<Vec<LogEntry>>;
    fn by_interaction(&self, interaction_id: &str) -> Result<Vec<LogEntry>>;
    fn stream(&self) -> Receiver<LogEntry>;
}
```

#### LogStore

```rust
struct LogStore { writer, reader, broadcaster }
```

### 8. Accounting

**MVP:**
- `cost_counters` — required поле в каждой записи лога с required nested полями.
- `fn aggregate(counters: &[CostCounters]) -> AggregatedCosts` — сумма по типам.
- `fn compute_attention_cost(counters: &CostCounters) -> f64` — заглушка, возвращает 0.0.
- `terio stats` показывает сумму cost_counters.

### 9. Trust Engine
- Risk levels: read_only, local_write, destructive, network_read, network_write, credential_access, financial.
- Политики: `always_ask`, `ask_once`, `allow`.
- **Auto-run** (MVP): только exact normalized match + risk <= local_write + success_count >= trust_threshold + все parameters resolved однозначно + preconditions пройдены + все output внутри CWD или разрешённой директории + нет destructive/network_write шагов + пользователь не отключал auto-run + предыдущий запуск был успешен в эквивалентном контексте.
- **Fuzzy match:** никогда не auto-run в MVP.

### 10. Identity

- `instance_id` — ULID, генерируется при первом запуске, хранится в `~/.terio/instance.json`. Не меняется.
- `session_id` — UUID на каждый запуск, пишется в каждую запись лога.

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
3. **Найден и trust >= threshold:** Script Cache → Execution Layer → LogStore.append (validate → redact → write → broadcast) → Renderer (читает из LogEventStream).
4. **Не найден или trust < threshold:** → Agent Layer (LLM) → план → подтверждение → Execution Layer → LogStore → Renderer.
5. Renderer получает `LogEntry` через `LogReader.stream()` и показывает.
6. Accounting собирает `cost_counters` из лога. `terio stats` агрегирует.

## Key Design Rule

Пользователь работает. terio запоминает. Модель вызывается только когда нужно впервые. Renderer читает из лога. Лог — единый источник истины для UI.

**Архитектурная готовность к будущему:**
- `LogWriter`/`LogReader` traits — смена формата лога без рефакторинга.
- `interaction_id` — группировка пар (запрос → ответ) для любых типов взаимодействия.
- `display_profile` — типизация записей лога с настраиваемой отображаемостью.
- `cost_counters` — сырые счётчики для будущей метрики.
- `instance_id` + `session_id` — идентификация для шэринга и возобновления.
- Dioxus webview — основа для будущей оконной системы.
