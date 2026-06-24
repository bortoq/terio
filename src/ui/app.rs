// Dioxus webview UI — primary interface for terio.
// Receives log entries from main via static (refreshable with F5).

use crate::log::LogReader;
use crate::types::LogEntry;
use dioxus::prelude::*;
use std::sync::Mutex;

static LOG_ENTRIES: Mutex<Vec<LogEntry>> = Mutex::new(Vec::new());

/// Запускает Dioxus-окно с переданными записями лога.
pub fn run_with_entries(entries: Vec<LogEntry>) {
    if let Ok(mut guard) = LOG_ENTRIES.lock() {
        *guard = entries;
    }
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

/// Безопасное усечение строки по символам (не байтам).
fn truncate_safe(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn app() -> Element {
    // Read current entries
    let entries_guard = LOG_ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
    let rows = prepare_rows(&entries_guard);
    drop(entries_guard);

    let mut input_text = use_signal(String::new);

    let on_submit = move |_| {
        let val = input_text();
        let val = val.trim().to_string();
        if !val.is_empty() {
            let val2 = val.clone();
            _ = std::thread::spawn(move || {
                _ = std::process::Command::new("terio")
                    .arg("ask")
                    .arg(&val2)
                    .arg("--yes")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .and_then(|mut c| c.wait());
            });
        }
        input_text.set(String::new());
    };

    let on_f5 = move |_| {
        if let Ok(log_dir) = crate::log::writer::JsonlLogWriter::default_dir() {
            let reader = crate::log::reader::JsonlLogReader::new(&log_dir);
            if let Ok(fresh) = reader.recent(100) {
                if let Ok(mut guard) = LOG_ENTRIES.lock() {
                    *guard = fresh;
                }
            }
        }
    };

    let count = rows.len();

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
            // Header + input
            div {
                style: "
                    display: flex; align-items: center; gap: 12px;
                    padding: 8px 12px;
                    background: #2d2d2d;
                    border-bottom: 1px solid #333;
                ",
                div { style: "font-size: 18px; font-weight: bold; color: #569cd6;", "terio" }
                div { style: "font-size: 12px; color: #888;", "log · {count} записей" }
                // Input + ask button
                div {
                    style: "display: flex; flex: 1; margin-left: 12px;",
                    input {
                        style: "
                            flex: 1;
                            background: #3c3c3c;
                            border: 1px solid #555;
                            color: #d4d4d4;
                            padding: 4px 8px;
                            border-radius: 3px;
                            font-size: 13px;
                            outline: none;
                        ",
                        placeholder: "Введите запрос...",
                        value: "{input_text}",
                        oninput: move |evt: Event<FormData>| {
                            let val = evt.value().clone();
                            input_text.set(val);
                        },
                    }
                    button {
                        onclick: on_submit,
                        style: "
                            margin-left: 6px;
                            background: #0e639c;
                            border: none;
                            color: white;
                            padding: 4px 12px;
                            border-radius: 3px;
                            font-size: 13px;
                            cursor: pointer;
                        ",
                        "Ask"
                    }
                }
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
                                "{truncate_safe(&row.desc, 120)}"
                            }
                            div { style: "color: #888;", "{row.exit}" }
                            div { style: "color: #d7ba7d; font-size: 11px;", "{row.risk}" }
                        }
                    }
                }
            }
            // Footer with F5 button
            div {
                style: "
                    display: flex; justify-content: space-between; align-items: center;
                    font-size: 11px; color: #555;
                    padding: 4px 12px; border-top: 1px solid #333;
                ",
                button {
                    onclick: on_f5,
                    style: "
                        background: #3c3c3c; border: 1px solid #555;
                        color: #d4d4d4; padding: 2px 10px;
                        border-radius: 3px; cursor: pointer; font-size: 11px;
                    ",
                    "↻ Обновить (F5)"
                }
            }
        }
    }
}
