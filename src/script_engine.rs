// Phase 2: Script engine — Rhai + TOML overlay.
// Интерпретатор: rhai::Engine + TOML → Rhai-транслятор.
// API для скриптов: terio_execute(), confirm(), show(), config_get/set().

use anyhow::{bail, Context, Result};
use rhai::{Engine, Scope, AST};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Источник скрипта
#[derive(Debug, Clone)]
pub enum ScriptSource {
    /// Исходный код на Rhai
    Rhai(String),
    /// TOML-декларация (будет скомпилирована в Rhai)
    Toml(String),
}

/// Категория скрипта (определяет приоритет при разрешении конфликтов).
/// Приоритет (по возрастанию): Builtin < Learned < Core < User.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScriptKind {
    /// Встроенные скрипты (низший приоритет)
    Builtin,
    /// learned/: автоматически изученные
    Learned,
    /// core/: системные скрипты
    Core,
    /// user/: пользовательские (наивысший приоритет)
    User,
}

/// Зарегистрированный скрипт
#[derive(Debug, Clone)]
pub struct Script {
    pub id: String,
    pub triggers: Vec<String>,
    pub source: ScriptSource,
    pub kind: ScriptKind,
    /// Опциональное описание
    pub description: String,
}

/// Бекенд для API-функций, вызываемых из скриптов.
/// Позволяет переключать реализацию между CLI и UI.
pub trait ScriptApiBackend: Send + Sync {
    fn execute(&self, cmd: &str, args: &[String]) -> Result<String>;
    fn show(&self, text: &str) -> Result<()>;
    fn confirm(&self, prompt: &str) -> Result<bool>;
    fn config_get(&self, key: &str) -> Result<String>;
    fn config_set(&self, key: &str, value: &str) -> Result<()>;
}

// ---------------------------------------------------------------------------
// TOML → Rhai translator
// ---------------------------------------------------------------------------

/// Переводит TOML-декларацию скрипта в Rhai-код.
///
/// Поддерживаемые форматы:
/// ```toml
/// triggers = ["list files"]
/// [[steps]]
/// command = "ls"
/// args = ["-la"]
/// ```
/// или
/// ```toml
/// triggers = ["hello"]
/// [step]
/// command = "echo"
/// args = ["hello world"]
/// ```
pub fn toml_to_rhai(toml_str: &str) -> Result<String> {
    let value: toml::Value = toml::from_str(toml_str.trim()).context("TOML parse error")?;
    let table: &toml::map::Map<String, toml::Value> = match &value {
        toml::Value::Table(t) => t,
        _ => bail!("TOML script must be a table at root"),
    };

    match table.get("triggers") {
        Some(toml::Value::Array(arr)) => {
            if arr.is_empty() {
                bail!("triggers array is empty");
            }
            for v in arr {
                if !matches!(v, toml::Value::String(_)) {
                    bail!("trigger must be a string, got {:?}", v);
                }
            }
        }
        Some(other) => bail!("triggers must be an array, got {:?}", other),
        None => bail!("missing required field: triggers (array of strings)"),
    }

    if let Some(toml::Value::Array(arr)) = table.get("steps") {
        return build_rhai_from_steps(arr);
    }

    if let Some(toml::Value::Table(step_table)) = table.get("step") {
        let cmd = step_table
            .get("command")
            .and_then(|v| v.as_str())
            .with_context(|| "step missing 'command' string")?;
        let args = parse_args(step_table.get("args"))?;
        return Ok(build_rhai_single(cmd, &args));
    }

    if let Some(toml::Value::Table(steps_table)) = table.get("steps") {
        let cmd = steps_table
            .get("command")
            .and_then(|v| v.as_str())
            .with_context(|| "steps table missing 'command' string")?;
        let args = parse_args(steps_table.get("args"))?;
        return Ok(build_rhai_single(cmd, &args));
    }

    bail!("missing required field: steps (array of step tables) or step (single step table)")
}

fn parse_args(args_val: Option<&toml::Value>) -> Result<Vec<String>> {
    match args_val {
        None => Ok(Vec::new()),
        Some(toml::Value::Array(a)) => a
            .iter()
            .map(|v| match v {
                toml::Value::String(s) => Ok(s.clone()),
                _ => bail!("args must be strings, got {:?}", v),
            })
            .collect(),
        Some(other) => bail!("args must be an array, got {:?}", other),
    }
}

fn rhai_esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn build_rhai_from_steps(steps: &[toml::Value]) -> Result<String> {
    let mut rhai = String::from("fn main() {\n");
    for (i, step_val) in steps.iter().enumerate() {
        let step_table = match step_val {
            toml::Value::Table(t) => t,
            other => bail!("steps[{}] is not a table: {:?}", i, other),
        };
        let cmd = step_table
            .get("command")
            .and_then(|v| v.as_str())
            .with_context(|| format!("steps[{}] missing 'command' string", i))?;
        let args: Vec<String> = match step_table.get("args") {
            Some(toml::Value::Array(a)) => a
                .iter()
                .map(|v| match v {
                    toml::Value::String(s) => Ok(s.clone()),
                    other => Err(anyhow::anyhow!(
                        "steps[{}] arg is not string: {:?}",
                        i,
                        other
                    )),
                })
                .collect::<Result<Vec<_>>>()?,
            None => Vec::new(),
            Some(other) => bail!("steps[{}] args is not array: {:?}", i, other),
        };
        let args_lit = args
            .iter()
            .map(|a| format!("\"{}\"", rhai_esc(a)))
            .collect::<Vec<_>>()
            .join(", ");
        rhai.push_str(&format!(
            "    let result = terio_execute(\"{}\", [{}]);\n",
            rhai_esc(cmd),
            args_lit,
        ));
        rhai.push_str("    terio_show(result);\n");
    }
    rhai.push_str("}\n");
    Ok(rhai)
}

fn build_rhai_single(cmd: &str, args: &[String]) -> String {
    let args_lit = args
        .iter()
        .map(|a| format!("\"{}\"", rhai_esc(a)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "fn main() {{\n    let result = terio_execute(\"{}\", [{}]);\n    terio_show(result);\n}}\n",
        rhai_esc(cmd),
        args_lit,
    )
}

/// Возвращает встроенные скрипты (binary-embedded core).
pub fn builtin_scripts() -> Vec<Script> {
    vec![
        Script {
            id: "help".into(),
            triggers: vec!["help".into(), "помощь".into(), "?".into()],
            source: ScriptSource::Rhai(BUILTIN_HELP.into()),
            kind: ScriptKind::Builtin,
            description: "Show built-in help".into(),
        },
        Script {
            id: "mode".into(),
            triggers: vec!["mode".into()],
            source: ScriptSource::Rhai(BUILTIN_MODE.into()),
            kind: ScriptKind::Builtin,
            description: "Set attention mode: quiet | normal | debug".into(),
        },
        Script {
            id: "focus".into(),
            triggers: vec![
                "focus up".into(),
                "focus down".into(),
                "focus ↑".into(),
                "focus ↓".into(),
            ],
            source: ScriptSource::Rhai(BUILTIN_FOCUS.into()),
            kind: ScriptKind::Builtin,
            description: "Switch output window focus (UI only)".into(),
        },
        Script {
            id: "scroll".into(),
            triggers: vec![
                "scroll 1".into(),
                "scroll -1".into(),
                "scroll 5".into(),
                "scroll -5".into(),
                "scroll 10".into(),
                "scroll -10".into(),
            ],
            source: ScriptSource::Rhai(BUILTIN_SCROLL.into()),
            kind: ScriptKind::Builtin,
            description: "Scroll output (UI only)".into(),
        },
        Script {
            id: "repeat".into(),
            triggers: vec!["repeat".into(), "повтори".into()],
            source: ScriptSource::Rhai(BUILTIN_REPEAT.into()),
            kind: ScriptKind::Builtin,
            description: "Repeat last LLM request".into(),
        },
        // Пример TOML-скрипта (демонстрирует TOML overlay)
        Script {
            id: "list_files_demo".into(),
            triggers: vec!["list files".into(), "ls".into()],
            source: ScriptSource::Toml(TOML_DEMO.into()),
            kind: ScriptKind::Builtin,
            description: "List files in current directory (TOML demo)".into(),
        },
    ]
}

const BUILTIN_HELP: &str = r##"
fn main() {
    let help_text = #"
terio — интегратор интерфейсов.

Использование: terio [КОМАНДА]

Команды:
  ask <запрос>       запрос на естественном языке
  run -- <команда>   выполнить shell-команду
  log [--json]       показать лог
  stats              метрики и cost_counters
  confirm            подтвердить ожидающий план
  undo               откатить последнюю операцию
  redo               повторить отменённую операцию
  cancel             отменить текущую операцию
  config show        показать настройки
  config set <k> <v> установить настройку
  learn <program>    обучить интеграцию
  integrations       статус интеграций
  forget <program>   забыть интеграцию
  mode <mode>        режим внимания
  focus <up|down>    переключить окно вывода (UI)
  scroll <N>         прокрутить вывод (UI)
  repeat             повторить последний запрос
  help               эта справка
  ui                 открыть UI (по умолчанию)
  script list        список скриптов
  script install <p> установить скрипт

Подробнее: https://github.com/bortoq/terio
"#;
    terio_show(help_text);
}
"##;

const BUILTIN_MODE: &str = r##"
fn main() {
    let args = terio_args();
    if args.len() != 1 {
        terio_show("Использование: mode <quiet|normal|debug>");
        return;
    }
    let mode = args[0];
    if mode != "quiet" && mode != "normal" && mode != "debug" {
        terio_show("Неизвестный режим. Используйте: quiet, normal, debug");
        return;
    }
    terio_config_set("attention_mode", mode);
    terio_show("attention mode: " + mode);
}
"##;

const BUILTIN_FOCUS: &str = r##"
fn main() {
    terio_show("Фокус работает в UI. Запустите terio без аргументов.");
}
"##;

const BUILTIN_SCROLL: &str = r##"
fn main() {
    terio_show("Скролл работает в UI. Запустите terio без аргументов.");
}
"##;

const BUILTIN_REPEAT: &str = r##"
fn main() {
    let result = terio_config_get("last_request");
    if result == "" {
        terio_show("Нет предыдущих запросов для повторения.");
        return;
    }
    terio_show("Повтор запроса: " + result);
    // В CLI режиме пользователь наберёт `terio ask "<result>"`
    // В UI режиме скрипт инициирует ask flow
    terio_show("Выполните: terio ask \"" + result + "\"");
}
"##;

const TOML_DEMO: &str = r##"
triggers = ["list files", "ls"]


[step]
command = "ls"
args = ["-la"]
"##;

// ---------------------------------------------------------------------------
// ScriptEngine
// ---------------------------------------------------------------------------

/// Основной движок скриптов. Загружает, компилирует и выполняет скрипты.
pub struct ScriptEngine {
    engine: Engine,
    scripts: Vec<Script>,
    /// Выходной буфер: накапливает вывод от terio_show() (shared with closures)
    output_buffer: Arc<Mutex<Vec<String>>>,
    /// Аргументы, переданные в текущий скрипт (часть запроса после команды)
    script_args: Vec<String>,
}

impl ScriptEngine {
    /// Создаёт новый ScriptEngine с заданным API-бекендом.
    pub fn new(backend: Arc<dyn ScriptApiBackend>) -> Self {
        let mut engine = Engine::new();

        // Настраиваем ограничения безопасности
        engine.set_max_operations(100_000);
        engine.set_max_string_size(1024 * 1024); // 1 MB

        // Shared output buffer, accessible from closures
        let shared_output: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        // Регистрируем API-функции терьо (через closure, захватывающий Arc<backend>)
        let b = Arc::clone(&backend);
        engine.register_fn("terio_execute", move |cmd: String, args: rhai::Array| {
            let str_args: Vec<String> = args.iter().map(|d| d.to_string()).collect();
            match b.execute(&cmd, &str_args) {
                Ok(out) => out,
                Err(e) => format!("error: {e}"),
            }
        });

        let b = Arc::clone(&backend);
        let out_clone = Arc::clone(&shared_output);
        engine.register_fn("terio_show", move |text: String| {
            let _ = b.show(&text);
            let mut out = out_clone.lock().unwrap();
            out.push(text);
        });

        let b = Arc::clone(&backend);
        engine.register_fn("terio_confirm", move |prompt: String| -> bool {
            b.confirm(&prompt).unwrap_or(false)
        });

        let b = Arc::clone(&backend);
        engine.register_fn("terio_config_get", move |key: String| -> String {
            b.config_get(&key).unwrap_or_default()
        });

        let b = Arc::clone(&backend);
        engine.register_fn("terio_config_set", move |key: String, value: String| {
            let _ = b.config_set(&key, &value);
        });

        // Регистрируем функцию для получения аргументов скрипта
        // Она будет переопределяться перед каждым выполнением через Scope
        engine.register_fn("terio_args", || -> Vec<String> {
            Vec::new() // stub, переопределяется в Scope
        });

        Self {
            engine,
            scripts: Vec::new(),
            output_buffer: Arc::clone(&shared_output),
            script_args: Vec::new(),
        }
    }

    /// Загрузить скрипты (встроенные + из директорий).
    pub fn load_all(&mut self, dirs: &ScriptDirs) -> Result<()> {
        // 1. Встроенные
        for script in builtin_scripts() {
            self.register(script)?;
        }

        // 2. core/
        self.load_from_dir(&dirs.core, ScriptKind::Core)?;

        // 3. user/ (переопределяет core)
        self.load_from_dir(&dirs.user, ScriptKind::User)?;

        // 4. learned/
        self.load_from_dir(&dirs.learned, ScriptKind::Learned)?;

        Ok(())
    }

    /// Загрузить скрипты из одной директории.
    fn load_from_dir(&mut self, dir: &Path, kind: ScriptKind) -> Result<()> {
        if !dir.exists() {
            std::fs::create_dir_all(dir)?;
            return Ok(());
        }
        let read_dir = match std::fs::read_dir(dir) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("terio: не удалось прочитать {:?}: {e}", dir);
                return Ok(());
            }
        };

        for entry in read_dir {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("rhai") {
                let source = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("terio: не удалось прочитать {:?}: {e}", path);
                        continue;
                    }
                };
                let id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let triggers = extract_triggers_from_rhai(&source);
                let description = extract_description_from_rhai(&source);
                let script = Script {
                    id,
                    triggers,
                    source: ScriptSource::Rhai(source),
                    kind,
                    description,
                };
                self.register(script)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                let source = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("terio: не удалось прочитать {:?}: {e}", path);
                        continue;
                    }
                };
                let id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                // Парсим TOML для извлечения triggers
                let triggers = extract_triggers_from_toml(&source).unwrap_or_default();
                let description = extract_description_from_toml(&source).unwrap_or_default();
                let script = Script {
                    id,
                    triggers,
                    source: ScriptSource::Toml(source),
                    kind,
                    description,
                };
                self.register(script)?;
            }
        }
        Ok(())
    }

    /// Зарегистрировать один скрипт (скомпилировать и добавить в список).
    pub fn register(&mut self, script: Script) -> Result<()> {
        // Компилируем исходник
        let rhai_source = match &script.source {
            ScriptSource::Rhai(src) => src.clone(),
            ScriptSource::Toml(toml_str) => toml_to_rhai(toml_str)?,
        };

        // Компилируем AST (проверяем синтаксис)
        let _ast: AST = self
            .engine
            .compile(&rhai_source)
            .with_context(|| format!("failed to compile script '{}'", script.id))?;

        // Remove lower-priority scripts that share triggers with the new one.
        // Priority: User > Core > Learned > Builtin.
        self.scripts.retain(|s| {
            if s.id == script.id {
                return false; // same id: override
            }
            // If new script has higher kind and shares a trigger, remove the old one.
            if script.kind > s.kind {
                let overlap: bool = s
                    .triggers
                    .iter()
                    .any(|st| script.triggers.iter().any(|nt| nt.eq_ignore_ascii_case(st)));
                if overlap {
                    return false;
                }
            }
            true
        });

        self.scripts.push(script);

        // Keep scripts sorted by kind descending so match_input finds highest priority first.
        self.scripts.sort_by_key(|b| std::cmp::Reverse(b.kind));

        Ok(())
    }

    /// Найти скрипт по вводу пользователя (сопоставление trigger).
    /// Возвращает (Script, remaining_args), где remaining_args — часть ввода после команды.
    /// Trigger matching — case-insensitive; аргументы сохраняют оригинальный регистр.
    pub fn match_input(&self, input: &str) -> Option<(&Script, Vec<String>)> {
        let input_trimmed = input.trim();
        let input_lower = input_trimmed.to_lowercase();

        // Scripts already sorted by kind desc (User > Core > Learned > Builtin).
        // First-found wins within same kind.
        for script in &self.scripts {
            for trigger in &script.triggers {
                let trigger_lower = trigger.to_lowercase();

                // 1) Exact match: full trigger equals full input
                if input_lower == trigger_lower {
                    return Some((script, Vec::new()));
                }

                // 2) Prefix match: input starts with trigger + space
                //    Remaining words = args (preserve original case)
                let prefix = format!("{} ", trigger_lower);
                if input_lower.starts_with(&prefix) {
                    let consumed = trigger_lower.split_whitespace().count();
                    let args: Vec<String> = input_trimmed
                        .split_whitespace()
                        .skip(consumed)
                        .map(|s| s.to_string())
                        .collect();
                    return Some((script, args));
                }
            }
        }

        None
    }

    /// Выполнить скрипт с заданными аргументами.
    /// Возвращает накопленный вывод (через terio::show).
    pub fn execute_script(&mut self, script: &Script, args: Vec<String>) -> Result<String> {
        let rhai_source = match &script.source {
            ScriptSource::Rhai(src) => src.clone(),
            ScriptSource::Toml(toml_str) => toml_to_rhai(toml_str)?,
        };

        // AST уже скомпилирован при register, но компилируем снова (безопаснее)
        let ast: AST = self.engine.compile(&rhai_source)?;

        // Создаём Scope и передаём аргументы
        let mut scope = Scope::new();
        let args_clone = args.clone();
        // Переопределяем terio_args()
        self.engine
            .register_fn("terio_args", move || -> Vec<String> { args_clone.clone() });

        // Очищаем буфер вывода
        self.output_buffer.lock().unwrap().clear();
        self.script_args = args;

        // Выполняем
        self.engine
            .call_fn::<()>(&mut scope, &ast, "main", ())
            .map_err(|e| anyhow::anyhow!("script '{}' error: {e}", script.id))?;

        let output = self.output_buffer.lock().unwrap().join("\n");
        Ok(output)
    }

    /// Выполнить скрипт по его ID.
    pub fn run_script_by_id(&mut self, id: &str, args: Vec<String>) -> Result<String> {
        let idx = self
            .scripts
            .iter()
            .position(|s| s.id == id)
            .with_context(|| format!("script '{}' not found", id))?;
        let script = self.scripts[idx].clone();
        self.execute_script(&script, args)
    }

    /// Добавить в буфер вывода (вызывается из API).
    pub fn push_output(&mut self, text: &str) {
        self.output_buffer.lock().unwrap().push(text.to_string());
    }

    /// Получить все зарегистрированные скрипты.
    pub fn scripts(&self) -> &[Script] {
        &self.scripts
    }

    /// Создать директории скриптов, если не существуют.
    pub fn ensure_dirs(dirs: &ScriptDirs) -> Result<()> {
        std::fs::create_dir_all(&dirs.core)?;
        std::fs::create_dir_all(&dirs.user)?;
        std::fs::create_dir_all(&dirs.learned)?;
        Ok(())
    }

    /// Установить скрипт из файла в user/ директорию.
    pub fn install_script(path: &Path, dirs: &ScriptDirs) -> Result<String> {
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("не удалось прочитать {:?}", path))?;

        // Определяем тип по расширению
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("rhai");
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("script")
            .to_string();

        // Проверяем корректность (Rhai или TOML)
        match ext {
            "rhai" => {
                Engine::new()
                    .compile(&source)
                    .with_context(|| format!("Rhai compilation error in {:?}", path))?;
            }
            "toml" => {
                toml_to_rhai(&source)?;
            }
            other => {
                bail!("неподдерживаемое расширение: .{other} (используйте .rhai или .toml)");
            }
        }

        // Копируем в user/ с тем же именем + расширение
        let dest = dirs.user.join(format!("{id}.{ext}"));
        std::fs::write(&dest, &source)
            .with_context(|| format!("не удалось записать {:?}", dest))?;

        Ok(id)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Пути для директорий скриптов.
/// Базовая директория: ~/.terio/scripts/
pub fn default_script_dirs() -> Result<ScriptDirs> {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"))?;
    let base = PathBuf::from(home).join(".terio").join("scripts");
    Ok(ScriptDirs {
        core: base.join("core"),
        user: base.join("user"),
        learned: base.join("learned"),
    })
}

pub struct ScriptDirs {
    pub core: PathBuf,
    pub user: PathBuf,
    pub learned: PathBuf,
}

/// Извлечь triggers из Rhai-скрипта (ищем комментарий // triggers: ...)
fn extract_triggers_from_rhai(source: &str) -> Vec<String> {
    use regex::Regex;
    let re = Regex::new(r#"(?m)^//\s*triggers:\s*(.+)$"#).ok();
    match re {
        Some(re) => {
            if let Some(cap) = re.captures(source) {
                let line = cap.get(1).unwrap().as_str();
                line.split(',')
                    .map(|s| s.trim().trim_matches('"').trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            } else {
                Vec::new()
            }
        }
        None => Vec::new(),
    }
}

/// Извлечь описание из Rhai-скрипта (комментарий // description: ...)
fn extract_description_from_rhai(source: &str) -> String {
    use regex::Regex;
    let re = Regex::new(r#"(?m)^//\s*description:\s*(.+)$"#).ok();
    match re {
        Some(re) => {
            if let Some(cap) = re.captures(source) {
                cap.get(1).unwrap().as_str().trim().to_string()
            } else {
                String::new()
            }
        }
        None => String::new(),
    }
}

/// Извлечь triggers из TOML
fn extract_triggers_from_toml(source: &str) -> Result<Vec<String>> {
    let value: toml::Value = toml::from_str(source.trim())?;
    match value.get("triggers") {
        Some(toml::Value::Array(arr)) => arr
            .iter()
            .map(|v| match v {
                toml::Value::String(s) => Ok(s.clone()),
                _ => bail!("trigger must be string"),
            })
            .collect(),
        _ => Ok(Vec::new()),
    }
}

/// Извлечь описание из TOML
fn extract_description_from_toml(source: &str) -> Result<String> {
    let value: toml::Value = toml::from_str(source.trim())?;
    Ok(value
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string())
}

// ---------------------------------------------------------------------------
// CLI API backend (реализация для CLI-режима)
// ---------------------------------------------------------------------------

/// Реализация API для CLI-режима.
pub struct CliApiBackend;

impl ScriptApiBackend for CliApiBackend {
    fn execute(&self, cmd: &str, args: &[String]) -> Result<String> {
        let full_cmd: Vec<String> = std::iter::once(cmd.to_string())
            .chain(args.iter().cloned())
            .collect();
        // Basic risk check: warn user for dangerous commands
        let risk = crate::run::compute_risk(&full_cmd[0], &full_cmd[1..]);
        if matches!(
            risk,
            crate::types::RiskLevel::Destructive | crate::types::RiskLevel::NetworkWrite
        ) {
            eprintln!(
                "⚠️  WARNING: script is executing a risky command: {}",
                full_cmd.join(" ")
            );
            eprint!("Proceed? [y/N]: ");
            use std::io::Write;
            std::io::stderr().flush()?;
            let mut reply = String::new();
            std::io::stdin().read_line(&mut reply)?;
            if !reply.trim().eq_ignore_ascii_case("y") {
                bail!("script command cancelled by user");
            }
        }
        let result = crate::run::execute(&full_cmd)?;
        let mut output = result.stdout;
        if !result.stderr.is_empty() {
            output.push_str(&result.stderr);
        }
        Ok(output)
    }

    fn show(&self, text: &str) -> Result<()> {
        println!("{text}");
        Ok(())
    }

    fn confirm(&self, prompt: &str) -> Result<bool> {
        eprint!("{prompt} [y/N]: ");
        use std::io::Write;
        std::io::stderr().flush()?;
        let mut reply = String::new();
        std::io::stdin().read_line(&mut reply)?;
        Ok(reply.trim().eq_ignore_ascii_case("y") || reply.trim().eq_ignore_ascii_case("yes"))
    }

    fn config_get(&self, key: &str) -> Result<String> {
        let config = crate::config::Config::load().unwrap_or_default();
        // Простейший доступ по ключу
        let val = match key {
            "attention_mode" => format!("{:?}", config.attention_mode),
            "provider" => format!("{:?}", config.provider),
            _ => String::new(),
        };
        Ok(val)
    }

    fn config_set(&self, key: &str, value: &str) -> Result<()> {
        let mut config = crate::config::Config::load().unwrap_or_default();
        config.set(key, value)?;
        config.save()?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct TestApiBackend {
        output: Mutex<Vec<String>>,
    }

    impl TestApiBackend {
        fn new() -> Self {
            Self {
                output: Mutex::new(Vec::new()),
            }
        }
    }

    impl ScriptApiBackend for TestApiBackend {
        fn execute(&self, cmd: &str, _args: &[String]) -> Result<String> {
            Ok(format!("executed: {cmd}"))
        }
        fn show(&self, text: &str) -> Result<()> {
            self.output.lock().unwrap().push(text.to_string());
            Ok(())
        }
        fn confirm(&self, _prompt: &str) -> Result<bool> {
            Ok(true)
        }
        fn config_get(&self, key: &str) -> Result<String> {
            Ok(format!("config:{key}"))
        }
        fn config_set(&self, key: &str, value: &str) -> Result<()> {
            let _ = (key, value);
            Ok(())
        }
    }

    #[test]
    fn test_toml_to_rhai_basic() {
        let toml = r#"
triggers = ["list files", "ls"]
description = "List files"

[[steps]]
command = "ls"
args = ["-la", "/tmp"]
"#;
        let rhai = toml_to_rhai(toml).unwrap();
        assert!(rhai.contains("fn main()"));
        assert!(rhai.contains("terio_execute("));
        assert!(rhai.contains("\"ls\""));
        assert!(rhai.contains("\"-la\""));
        assert!(rhai.contains("\"/tmp\""));
    }

    #[test]
    fn test_toml_to_rhai_single_step() {
        // Single step table (без массива [[steps]])
        let toml = r#"
triggers = ["hello"]
[step]
command = "echo"
args = ["hello world"]
"#;
        let rhai = toml_to_rhai(toml).unwrap();
        assert!(rhai.contains("fn main()"));
        assert!(rhai.contains("\"echo\""));
        assert!(rhai.contains("\"hello world\""));
    }

    #[test]
    fn test_toml_missing_triggers() {
        let toml = r#"
description = "no triggers"
[[steps]]
command = "ls"
"#;
        assert!(toml_to_rhai(toml).is_err());
    }

    #[test]
    fn test_toml_empty_triggers() {
        let toml = r#"
triggers = []
[[steps]]
command = "ls"
"#;
        assert!(toml_to_rhai(toml).is_err());
    }

    #[test]
    fn test_toml_missing_steps() {
        let toml = r#"
triggers = ["test"]
"#;
        assert!(toml_to_rhai(toml).is_err());
    }

    #[test]
    fn test_engine_compile_builtins() {
        let backend = Arc::new(TestApiBackend::new());
        let mut engine = ScriptEngine::new(backend);
        for script in builtin_scripts() {
            engine.register(script).unwrap();
        }
        assert!(engine.scripts().len() >= 5);
    }

    #[test]
    fn test_match_exact_trigger() {
        let backend = Arc::new(TestApiBackend::new());
        let mut engine = ScriptEngine::new(backend);
        engine
            .register(Script {
                id: "test".into(),
                triggers: vec!["list files".into(), "ls".into()],
                source: ScriptSource::Rhai("fn main() {}".into()),
                kind: ScriptKind::User,
                description: String::new(),
            })
            .unwrap();

        let (matched, args) = engine.match_input("list files").unwrap();
        assert_eq!(matched.id, "test");
        assert!(args.is_empty());

        let (matched, args) = engine.match_input("ls").unwrap();
        assert_eq!(matched.id, "test");
        assert!(args.is_empty());
    }

    #[test]
    fn test_match_with_args() {
        let backend = Arc::new(TestApiBackend::new());
        let mut engine = ScriptEngine::new(backend);
        engine
            .register(Script {
                id: "mode".into(),
                triggers: vec!["mode".into()],
                source: ScriptSource::Rhai("fn main() {}".into()),
                kind: ScriptKind::Builtin,
                description: String::new(),
            })
            .unwrap();

        let (matched, args) = engine.match_input("mode quiet").unwrap();
        assert_eq!(matched.id, "mode");
        assert_eq!(args, vec!["quiet"]);

        let (matched, args) = engine.match_input("mode").unwrap();
        assert_eq!(matched.id, "mode");
        assert!(args.is_empty());
    }

    #[test]
    fn test_no_match() {
        let backend = Arc::new(TestApiBackend::new());
        let mut engine = ScriptEngine::new(backend);
        engine
            .register(Script {
                id: "test".into(),
                triggers: vec!["list files".into()],
                source: ScriptSource::Rhai("fn main() {}".into()),
                kind: ScriptKind::Builtin,
                description: String::new(),
            })
            .unwrap();

        assert!(engine.match_input("unknown command").is_none());
        assert!(engine.match_input("").is_none());
    }

    #[test]
    fn test_execute_rhai_script() {
        let backend = Arc::new(TestApiBackend::new());
        let mut engine = ScriptEngine::new(backend);
        let script = Script {
            id: "hello".into(),
            triggers: vec!["hello".into()],
            source: ScriptSource::Rhai(r#"fn main() { terio_show("Hello, World!"); }"#.into()),
            kind: ScriptKind::User,
            description: String::new(),
        };
        engine.register(script.clone()).unwrap();
        let output = engine.execute_script(&script, vec![]).unwrap();
        assert_eq!(output, "Hello, World!");
    }

    #[test]
    fn test_execute_toml_script() {
        let backend = Arc::new(TestApiBackend::new());
        let mut engine = ScriptEngine::new(backend);
        let toml = r#"
triggers = ["test"]

[[steps]]
command = "echo"
args = ["hi"]
"#;
        let script = Script {
            id: "test".into(),
            triggers: vec!["test".into()],
            source: ScriptSource::Toml(toml.into()),
            kind: ScriptKind::User,
            description: String::new(),
        };
        engine.register(script.clone()).unwrap();
        let output = engine.execute_script(&script, vec![]).unwrap();
        assert!(output.contains("executed: echo"));
    }

    #[test]
    fn test_match_case_insensitive() {
        let backend = Arc::new(TestApiBackend::new());
        let mut engine = ScriptEngine::new(backend);
        engine
            .register(Script {
                id: "help".into(),
                triggers: vec!["help".into(), "?".into()],
                source: ScriptSource::Rhai("fn main() {}".into()),
                kind: ScriptKind::Builtin,
                description: String::new(),
            })
            .unwrap();

        assert!(engine.match_input("HELP").is_some());
        assert!(engine.match_input("Help").is_some());
        assert!(engine.match_input("?").is_some());
    }

    #[test]
    fn test_user_override_core() {
        let backend = Arc::new(TestApiBackend::new());
        let mut engine = ScriptEngine::new(backend);
        // Core script
        engine
            .register(Script {
                id: "test".into(),
                triggers: vec!["test".into()],
                source: ScriptSource::Rhai(r#"fn main() { terio_show("core"); }"#.into()),
                kind: ScriptKind::Core,
                description: String::new(),
            })
            .unwrap();
        // User override
        engine
            .register(Script {
                id: "test".into(),
                triggers: vec!["test".into()],
                source: ScriptSource::Rhai(r#"fn main() { terio_show("user"); }"#.into()),
                kind: ScriptKind::User,
                description: String::new(),
            })
            .unwrap();

        // Должен быть только один (user переопределил core)
        let count = engine.scripts().iter().filter(|s| s.id == "test").count();
        assert_eq!(count, 1);
        let matched = engine.match_input("test").unwrap();
        assert_eq!(matched.0.kind, ScriptKind::User);
    }

    #[test]
    fn test_builtin_mode_script_compiles() {
        let backend = Arc::new(TestApiBackend::new());
        let mut engine = ScriptEngine::new(backend);
        let mode_script = builtin_scripts()
            .into_iter()
            .find(|s| s.id == "mode")
            .unwrap();
        engine.register(mode_script).unwrap();
    }
}
