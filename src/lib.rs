// terio — интерфейс-агрегатор с AI-планированием и кешированием скриптов

pub mod cli;
pub mod types;

pub mod accounting;
pub mod identity;
pub mod run;

#[cfg(feature = "desktop")]
pub mod ui;

// Следующие фазы (заглушки)
pub mod agent;
pub mod ask;
pub mod cache;
pub mod config;
pub mod matcher;
pub mod render;
pub mod trust;

pub mod log;
