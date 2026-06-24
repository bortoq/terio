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
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Запрос на естественном языке
    Ask {
        /// Текст запроса
        request: String,
    },

    /// Выполнить shell-команду напрямую
    #[command(name = "run")]
    Run {
        /// Команда и аргументы (после --)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },

    /// Показать историю в UI
    Log {
        /// Вывод в JSON вместо UI
        #[arg(long)]
        json: bool,
    },

    /// Открыть UI (по умолчанию)
    Ui,

    /// Показать метрики и cost_counters
    Stats,

    /// Отменить текущую операцию
    Cancel,

    /// Настройки
    Config,
}

impl Cli {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
}
