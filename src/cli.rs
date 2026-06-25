use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "terio",
    author,
    version,
    about = "Интегратор интерфейсов: терминал с LLM, кешем скриптов и песочницей"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Запрос на естественном языке
    Ask {
        /// Текст запроса
        request: String,
        /// Пропустить подтверждение для risk >= local_write
        #[arg(long)]
        yes: bool,
    },

    /// Выполнить shell-команду напрямую
    #[command(name = "run")]
    Run {
        /// Команда и аргументы (после --)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },

    /// Показать лог
    Log {
        /// Вывод в JSON в stdout
        #[arg(long)]
        json: bool,
    },

    /// Открыть UI (по умолчанию)
    Ui,

    /// Показать метрики и cost_counters
    Stats,

    /// Отменить текущую операцию
    Cancel,

    /// Выполнить ранее подтверждённый сохранённый план без повторного запроса к provider
    Confirm,

    /// Откатить последнее snapshot-backed выполнение
    Undo,

    /// Повторно применить последний undo snapshot
    Redo,

    /// Управление настройками
    #[command(subcommand)]
    Config(ConfigCmd),

    /// (Phase 7) Обучиться работе с программой через --help
    Learn {
        /// Имя программы (например, git, docker)
        program: String,
    },

    /// (Phase 7) Показать статус изученных программ
    Integrations,

    /// (Phase 7) Забыть изученную программу
    Forget {
        /// Имя программы
        program: String,
    },

    /// (Phase 7) Экспортировать окно для передачи другому экземпляру
    Share {
        /// Путь для сохранения JSON (по умолчанию stdout)
        output: Option<String>,
        /// Количество последних записей для экспорта
        #[arg(long, default_value = "50")]
        count: usize,
    },

    /// (Phase 7) Импортировать окно из другого экземпляра
    Receive {
        /// Путь к JSON-файлу или "-" для stdin
        input: String,
    },

    // --- Phase 0: Terminal commands ---
    /// Показать встроенную справку (Phase 0)
    Help,

    /// Переключить режим внимания: quiet | normal | debug (Phase 0)
    Mode {
        /// Режим: quiet (не отвлекать), normal (умеренно), debug (все шаги)
        mode: String,
    },

    /// Переключить окно вывода в UI: up | down (Phase 0)
    Focus {
        /// Направление: up, down
        direction: String,
    },

    /// Прокрутить окна в UI (Phase 0)
    Scroll {
        /// Количество строк (положительное = вниз, отрицательное = вверх)
        lines: i32,
    },

    /// Повторить последний запрос (Phase 0)
    Repeat,

    /// Управление скриптами (Phase 2)
    #[command(subcommand)]
    Script(ScriptCmd),

    /// Управление песочницей (Phase 1)
    #[command(subcommand)]
    Sandbox(SandboxCmd),
}

#[derive(Subcommand)]
pub enum ScriptCmd {
    /// Показать список установленных скриптов
    List,
    /// Установить скрипт из файла
    Install {
        /// Путь к .rhai или .toml файлу
        path: String,
    },
}

#[derive(Subcommand)]
pub enum SandboxCmd {
    /// Показать состояние песочницы
    Status,
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Показать текущую конфигурацию
    Show,
    /// Установить значение: terio config set <key> <value>
    Set { key: String, value: String },
}
