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

fn status_color(status: &str) -> &'static str {
    match status {
        "Success" => "#6a9955",
        "Failed" => "#f14c4c",
        "Cancelled" => "#d7ba7d",
        _ => "#888",
    }
}

fn kind_color(kind: &str) -> &'static str {
    match kind {
        "AgentTurn" => "#569cd6",
        "CommandRun" => "#ce9178",
        "ScriptRun" => "#6a9955",
        _ => "#888",
    }
}

fn risk_label(entry: &LogEntry) -> String {
    entry
        .risk
        .as_ref()
        .map(|r| format!("{:?}", r))
        .unwrap_or_default()
}

struct RowData {
    key: String,
    ts: String,
    kind: String,
    kind_color: &'static str,
    status: String,
    status_color: &'static str,
    desc: String,
    exit: String,
    risk: String,
}

fn prepare_rows(entries: &[LogEntry]) -> Vec<RowData> {
    entries
        .iter()
        .map(|entry| {
            let kind = format!("{:?}", entry.kind);
            let status = entry
                .status
                .as_ref()
                .map(|s| format!("{:?}", s))
                .unwrap_or_default();
            let ts = entry.ts[..19].to_string();
            let desc = entry
                .command
                .as_ref()
                .map(|c| &c.display[..std::cmp::min(120, c.display.len())])
                .unwrap_or("—")
                .to_string();
            let exit = entry.exit.map(|e| e.to_string()).unwrap_or_default();
            let risk = risk_label(entry);
            RowData {
                key: format!("{}-{}", entry.ts, entry.session_id),
                ts,
                kind_color: kind_color(&kind),
                status_color: status_color(&status),
                kind,
                status,
                desc,
                exit,
                risk,
            }
        })
        .collect()
}

fn app() -> Element {
    let entries = ENTRIES.get().map(|e| e.as_slice()).unwrap_or(&[]);
    let rows = prepare_rows(entries);

    rsx! {
        div {
            style: "
                display: flex;
                flex-direction: column;
                height: 100vh;
                font-family: 'Segoe UI', 'Courier New', monospace;
                background: #1e1e1e;
                color: #d4d4d4;
                font-size: 13px;
            ",
            // Header
            div {
                style: "
                    display: flex; align-items: center; gap: 12px;
                    padding: 8px 12px;
                    background: #2d2d2d;
                    border-bottom: 1px solid #333;
                ",
                div { style: "font-size: 18px; font-weight: bold; color: #569cd6;", "terio" }
                div { style: "font-size: 12px; color: #888;", "log · {entries.len()} записей" }
            }
            // Table header
            div {
                style: "
                    display: grid;
                    grid-template-columns: 160px 100px 80px 1fr 60px 80px;
                    gap: 0;
                    background: #252526;
                    padding: 6px 12px;
                    font-size: 11px;
                    text-transform: uppercase;
                    color: #888;
                    border-bottom: 1px solid #333;
                ",
                div { "Время" }
                div { "Тип" }
                div { "Статус" }
                div { "Команда / Описание" }
                div { "Код" }
                div { "Риск" }
            }
            // Table body
            div {
                style: "flex: 1; overflow-y: auto;",
                if rows.is_empty() {
                    div { style: "padding: 24px; color: #888; text-align: center;", "(лог пуст)" }
                } else {
                    for row in rows {
                        div {
                            key: "{row.key}",
                            style: "
                                display: grid;
                                grid-template-columns: 160px 100px 80px 1fr 60px 80px;
                                gap: 0;
                                padding: 4px 12px;
                                border-bottom: 1px solid #2d2d2d;
                                font-size: 13px;
                            ",
                            div { style: "color: #888;", "{row.ts}" }
                            div { style: "color: {row.kind_color};", "{row.kind}" }
                            div { style: "color: {row.status_color}; font-weight: bold;", "{row.status}" }
                            div {
                                style: "
                                    overflow: hidden;
                                    text-overflow: ellipsis;
                                    white-space: nowrap;
                                ",
                                "{row.desc}"
                            }
                            div { style: "color: #888;", "{row.exit}" }
                            div { style: "color: #d7ba7d; font-size: 11px;", "{row.risk}" }
                        }
                    }
                }
            }
            // Footer
            div {
                style: "
                    font-size: 11px;
                    color: #555;
                    padding: 4px 12px;
                    border-top: 1px solid #333;
                    text-align: right;
                ",
                "F5: обновить"
            }
        }
    }
}
