// terio — интегратор интерфейсов: терминал с LLM, кешем скриптов и песочницей

pub mod cli;
pub mod types;

pub mod accounting;
pub mod agent;
pub mod ask;
pub mod cache;
pub mod identity;
pub mod integration;
pub mod matcher;
pub mod provider;
pub mod redact;
pub mod run;
pub mod undo;
pub mod window;

#[cfg(feature = "desktop")]
pub mod ui;

pub mod config;
pub mod log;

// Следующие фазы (заглушки)
pub mod proactive;
pub mod registry;
pub mod render;
pub mod script_engine;
pub mod synonym;
pub mod trust;

#[cfg(test)]
pub mod test_support {
    use std::sync::{LazyLock, Mutex};

    pub static ENV_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
}
