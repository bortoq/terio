// terio — интерфейс-агрегатор с AI-планированием и кешированием скриптов

pub mod cli;
pub mod types;

pub mod accounting;
pub mod agent;
pub mod ask;
pub mod cache;
pub mod identity;
pub mod matcher;
pub mod redact;
pub mod run;

#[cfg(feature = "desktop")]
pub mod ui;

// Следующие фазы (заглушки)
pub mod config;
pub mod render;
pub mod trust;

pub mod log;
