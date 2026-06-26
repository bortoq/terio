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

    /// Управление синонимами (Phase 3)
    #[command(subcommand)]
    Alias(AliasCmd),

    /// Управление песочницей (Phase 1)
    #[command(subcommand)]
    Sandbox(SandboxCmd),

    /// Отчёт по затратам (Phase 5)
    Cost,
    /// Управление реестром скриптов сообщества (Phase 6)
    #[command(subcommand)]
    Registry(RegistryCmd),
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
    /// Запустить скрипт по ID с аргументами
    Run {
        /// ID скрипта
        id: String,
        /// Аргументы скрипта (после --)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Экспортировать скрипт(ы) в JSON (Phase 6)
    Export {
        /// ID скрипта (или "all" для всех)
        id: String,
        /// Путь для сохранения (по умолчанию stdout)
        output: Option<String>,
    },
    /// Импортировать скрипт(ы) из JSON (Phase 6)
    Import {
        /// Путь к JSON-файлу или "-" для stdin
        input: String,
    },
}

#[derive(Subcommand)]
pub enum AliasCmd {
    /// Показать список синонимов
    List,
    /// Удалить синоним
    Remove {
        /// Запрос для удаления
        query: String,
    },
}

#[derive(Subcommand)]
pub enum SandboxCmd {
    /// Показать состояние песочницы
    Status,
}

#[derive(Subcommand)]
pub enum RegistryCmd {
    /// Поиск скриптов в реестре сообщества
    Search {
        /// Поисковый запрос
        query: String,
    },
    /// Установить скрипт из реестра сообщества
    Install {
        /// ID скрипта в реестре
        id: String,
    },
    /// Опубликовать скрипт в реестр сообщества
    Publish {
        /// ID локального скрипта
        id: String,
        /// API-ключ реестра (или из config)
        #[arg(long)]
        api_key: Option<String>,
    },
    /// Показать метаданные скрипта из реестра (без установки)
    Inspect {
        /// ID скрипта в реестре
        id: String,
    },
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Показать текущую конфигурацию
    Show,
    /// Установить значение: terio config set <key> <value>
    Set { key: String, value: String },
}
