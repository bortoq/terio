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

    /// Откатить последнее snapshot-backed выполнение script execution
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
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Показать текущую конфигурацию
    Show,
    /// Установить значение: terio config set <key> <value>
    Set { key: String, value: String },
}
