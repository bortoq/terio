use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "terio",
    author,
    version,
    about = "Интерфейс-агрегатор с AI-планированием"
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

    /// Управление настройками
    #[command(subcommand)]
    Config(ConfigCmd),
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Показать текущую конфигурацию
    Show,
    /// Установить значение: terio config set <key> <value>
    Set { key: String, value: String },
}
