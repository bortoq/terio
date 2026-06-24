// terio — интерфейс-агрегатор с AI-планированием и кешированием скриптов

pub mod cli;

#[cfg(feature = "desktop")]
pub mod ui;

// Заглушки для следующих фаз
pub mod accounting;
pub mod agent;
pub mod ask;
pub mod cache;
pub mod config;
pub mod identity;
pub mod matcher;
pub mod render;
pub mod run;
pub mod trust;

pub mod log;
