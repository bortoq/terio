// Dioxus webview UI — primary interface for terio.
// Receives log entries from main via OnceLock (no filesystem access).

use crate::types::LogEntry;
use dioxus::prelude::*;
use std::sync::OnceLock;

static ENTRIES: OnceLock<Vec<LogEntry>> = OnceLock::new();

/// Запускает Dioxus-окно с переданными записями лога.
pub fn run_with_entries(entries: Vec<LogEntry>) {
    ENTRIES.set(entries).ok();
    dioxus::launch(app);
}

/// Старый API без аргументов (для обратной совместимости, не используется).
pub fn run() {
    run_with_entries(vec![]);
}

fn app() -> Element {
    let entries = ENTRIES.get().map(|e| e.as_slice()).unwrap_or(&[]);

    let log_content: Element = if entries.is_empty() {
        rsx! { span { "(лог пуст)" } }
    } else {
        let rendered: String = entries
            .iter()
            .map(|entry| {
                let ts = &entry.ts[..19];
                let status = entry
                    .status
                    .as_ref()
                    .map(|s| format!("{:?}", s))
                    .unwrap_or_default();
                let desc = entry
                    .command
                    .as_ref()
                    .map(|c| &c.display[..std::cmp::min(80, c.display.len())])
                    .unwrap_or("—");
                format!("  {ts} [{status}] {desc}\n")
            })
            .collect();
        rsx! { pre { "{rendered}" } }
    };

    rsx! {
        div {
            style: "
                display: flex;
                flex-direction: column;
                height: 100vh;
                font-family: 'Courier New', monospace;
                background: #1e1e1e;
                color: #d4d4d4;
                padding: 8px;
            ",
            div {
                style: "
                    font-size: 18px;
                    font-weight: bold;
                    color: #569cd6;
                    padding: 8px 0;
                    border-bottom: 1px solid #333;
                ",
                "terio"
            }
            div {
                style: "
                    flex: 1;
                    overflow-y: auto;
                    padding: 8px 0;
                    font-size: 14px;
                    white-space: pre-wrap;
                ",
                {log_content}
            }
            div {
                style: "
                    font-size: 12px;
                    color: #888;
                    padding: 4px 0;
                    border-top: 1px solid #333;
                ",
                "Всего записей: {entries.len()}"
            }
        }
    }
}
