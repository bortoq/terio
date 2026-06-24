// Dioxus webview UI — primary interface for terio.
// Uses initial log snapshot plus optional live stream and in-process UI commands.

use crate::ask::{clear_pending_confirmation, load_pending_confirmation};
use crate::config::Config;
use crate::log::LogReader;
use crate::trust::trust_level_str;
use crate::types::{DisplayType, LogEntry, LogKind, RendererHint};
use crate::undo::UndoStatus;
use dioxus::prelude::*;
use std::sync::{mpsc::Sender, Mutex};
use tokio::sync::broadcast;

static LOG_ENTRIES: Mutex<Vec<LogEntry>> = Mutex::new(Vec::new());
static LIVE_STREAM: Mutex<Option<broadcast::Receiver<LogEntry>>> = Mutex::new(None);
static ACTION_SENDER: Mutex<Option<Sender<UiCommand>>> = Mutex::new(None);

#[derive(Debug, Clone)]
pub enum UiCommand {
    Ask(String),
    Confirm,
    Undo,
    Redo,
}

/// Запускает Dioxus-окно с переданными записями лога.
pub fn run_with_entries(entries: Vec<LogEntry>) {
    run_with_entries_and_runtime(entries, None, None);
}

pub fn run_with_entries_and_runtime(
    entries: Vec<LogEntry>,
    stream: Option<broadcast::Receiver<LogEntry>>,
    sender: Option<Sender<UiCommand>>,
) {
    if let Ok(mut guard) = LOG_ENTRIES.lock() {
        *guard = entries;
    }
    if let Ok(mut guard) = LIVE_STREAM.lock() {
        *guard = stream;
    }
    if let Ok(mut guard) = ACTION_SENDER.lock() {
        *guard = sender;
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
        "SystemEvent" => "#9cdcfe",
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum WorkspaceView {
    Auto,
    Table,
    Timeline,
    Cards,
    Readable,
    Chat,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EntryRenderer {
    Table,
    Timeline,
    Card,
    Readable,
    Chat,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActivityState {
    Idle,
    Busy,
}

#[derive(Clone)]
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
    trust: String,
    stdout: String,
    stderr: String,
}

impl RowData {
    fn detail_text(&self) -> String {
        let mut parts = vec![format!("[{}] {} {}", self.ts, self.kind, self.desc)];
        if !self.status.is_empty() {
            parts.push(format!("status: {}", self.status));
        }
        if !self.risk.is_empty() {
            parts.push(format!("risk: {}", self.risk));
        }
        if !self.trust.is_empty() {
            parts.push(format!("trust: {}", self.trust));
        }
        parts.join("\n")
    }
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
            let ts = truncate_safe(&entry.ts, 19);
            let desc = entry
                .command
                .as_ref()
                .map(|c| truncate_safe(&c.display, 120))
                .or_else(|| entry.description.clone())
                .or_else(|| entry.prompt_summary.clone())
                .unwrap_or_else(|| "—".to_string());
            let exit = entry.exit.map(|e| e.to_string()).unwrap_or_default();
            let risk = risk_label(entry);
            let trust = trust_badge(entry);
            let stdout = entry.stdout_summary.clone().unwrap_or_default();
            let stderr = entry.stderr_summary.clone().unwrap_or_default();
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
                trust,
                stdout,
                stderr,
            }
        })
        .collect()
}

/// Безопасное усечение строки по символам (не байтам).
fn truncate_safe(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn trust_badge(entry: &LogEntry) -> String {
    if entry.cache_hit == Some(true) {
        let level = entry
            .success_count_after
            .or(entry.success_count_before)
            .unwrap_or(0) as u32;
        return format!("auto {}", trust_level_str(level, 3));
    }

    if entry.model_called == Some(true) {
        return "confirm".to_string();
    }

    match entry.risk.as_ref() {
        Some(crate::types::RiskLevel::ReadOnly) => "manual".to_string(),
        Some(_) => "confirm".to_string(),
        None => String::new(),
    }
}

fn refresh_entries() {
    if let Ok(log_dir) = crate::log::writer::JsonlLogWriter::default_dir() {
        let reader = crate::log::reader::JsonlLogReader::new(&log_dir);
        if let Ok(fresh) = reader.recent(100) {
            if let Ok(mut guard) = LOG_ENTRIES.lock() {
                *guard = fresh;
            }
        }
    }
}

fn append_live_entry(entry: LogEntry) {
    if let Ok(mut guard) = LOG_ENTRIES.lock() {
        guard.push(entry);
        if guard.len() > 500 {
            let excess = guard.len() - 500;
            guard.drain(0..excess);
        }
    }
}

fn refresh_undo_status() -> UndoStatus {
    crate::undo::latest_status().unwrap_or_default()
}

fn undo_summary_label(status: &UndoStatus) -> String {
    status
        .summary
        .clone()
        .unwrap_or_else(|| "undo/redo unavailable".to_string())
}

fn send_ui_command(command: UiCommand) {
    if let Ok(guard) = ACTION_SENDER.lock() {
        if let Some(sender) = guard.as_ref() {
            let _ = sender.send(command);
        }
    }
}

fn is_completion_entry(entry: &LogEntry) -> bool {
    matches!(
        entry.kind,
        LogKind::AgentTurn | LogKind::CommandRun | LogKind::ScriptRun | LogKind::SystemEvent
    )
}

fn renderer_for_entry(entry: &LogEntry, view: WorkspaceView) -> EntryRenderer {
    match view {
        WorkspaceView::Table => return EntryRenderer::Table,
        WorkspaceView::Timeline => return EntryRenderer::Timeline,
        WorkspaceView::Cards => return EntryRenderer::Card,
        WorkspaceView::Readable => return EntryRenderer::Readable,
        WorkspaceView::Chat => return EntryRenderer::Chat,
        WorkspaceView::Auto => {}
    }

    match entry.display_profile.renderer_hint {
        RendererHint::Timeline => EntryRenderer::Timeline,
        RendererHint::Card => EntryRenderer::Card,
        RendererHint::Plain => EntryRenderer::Readable,
        RendererHint::Table => EntryRenderer::Table,
        RendererHint::Auto => match entry.display_profile.display_type {
            DisplayType::Table => EntryRenderer::Table,
            DisplayType::Summary => EntryRenderer::Card,
            DisplayType::Text => {
                if matches!(entry.kind, LogKind::AgentTurn | LogKind::SystemEvent) {
                    EntryRenderer::Chat
                } else {
                    EntryRenderer::Readable
                }
            }
            _ => match entry.kind {
                LogKind::AgentTurn | LogKind::SystemEvent => EntryRenderer::Chat,
                LogKind::ScriptRun => EntryRenderer::Timeline,
                LogKind::CommandRun => EntryRenderer::Card,
            },
        },
    }
}

fn workspace_title(view: WorkspaceView) -> &'static str {
    match view {
        WorkspaceView::Auto => "Auto Workspace",
        WorkspaceView::Table => "Table View",
        WorkspaceView::Timeline => "Timeline View",
        WorkspaceView::Cards => "Card View",
        WorkspaceView::Readable => "Readable View",
        WorkspaceView::Chat => "Chat View",
    }
}

fn app() -> Element {
    let mut refresh_tick = use_signal(|| 0_u64);
    let live_rx = use_signal(|| LIVE_STREAM.lock().ok().and_then(|mut guard| guard.take()));

    let entries = LOG_ENTRIES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let rows = prepare_rows(&entries);

    let mut input_text = use_signal(String::new);
    let mut pending = use_signal(|| load_pending_confirmation().ok().flatten());
    let mut selected = use_signal(|| None::<String>);
    let mut undo_status = use_signal(refresh_undo_status);
    let mut active_view = use_signal(|| WorkspaceView::Auto);
    let mut activity_state = use_signal(|| ActivityState::Idle);
    let initial_config = Config::load().unwrap_or_default();
    let mut show_config = use_signal(|| initial_config.ui.show_config);
    let mut config_text = use_signal(|| initial_config.render_for_display());

    use_future(move || {
        let mut refresh_tick = refresh_tick;
        let mut pending = pending;
        let mut undo_status = undo_status;
        let mut activity_state = activity_state;
        let mut live_rx = live_rx;
        async move {
            let Some(mut rx) = live_rx.write().take() else {
                return;
            };
            while let Ok(entry) = rx.recv().await {
                let completion = is_completion_entry(&entry);
                append_live_entry(entry);
                pending.set(load_pending_confirmation().ok().flatten());
                undo_status.set(refresh_undo_status());
                if completion {
                    activity_state.set(ActivityState::Idle);
                }
                refresh_tick += 1;
            }
        }
    });

    let on_submit = move |_| {
        let val = input_text();
        let val = val.trim().to_string();
        if !val.is_empty() {
            activity_state.set(ActivityState::Busy);
            send_ui_command(UiCommand::Ask(val.clone()));
        }
        input_text.set(String::new());
    };

    let on_f5 = move |_| {
        refresh_entries();
        pending.set(load_pending_confirmation().ok().flatten());
        undo_status.set(refresh_undo_status());
        config_text.set(Config::load().unwrap_or_default().render_for_display());
        activity_state.set(ActivityState::Idle);
        refresh_tick += 1;
    };

    let count = rows.len();
    let _ = refresh_tick();

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
            div {
                style: "
                    display: flex; align-items: center; gap: 12px;
                    padding: 8px 12px;
                    background: #2d2d2d;
                    border-bottom: 1px solid #333;
                    flex-wrap: wrap;
                ",
                div { style: "font-size: 18px; font-weight: bold; color: #569cd6;", "terio" }
                div { style: "font-size: 12px; color: #888;", "log · {count} записей" }
                div { style: "font-size: 12px; color: #888;", "{undo_summary_label(&undo_status())}" }
                div {
                    style: format!(
                        "font-size: 12px; color: {};",
                        if activity_state() == ActivityState::Busy { "#d7ba7d" } else { "#6a9955" }
                    ),
                    if activity_state() == ActivityState::Busy { "● Running" } else { "● Idle" }
                }
                button {
                    onclick: move |_| {
                        if undo_status().can_undo {
                            activity_state.set(ActivityState::Busy);
                            send_ui_command(UiCommand::Undo);
                        }
                    },
                    style: format!(
                        "background: {}; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;",
                        if undo_status().can_undo { "#3c3c3c" } else { "#2a2a2a" }
                    ),
                    "Undo"
                }
                button {
                    onclick: move |_| {
                        if undo_status().can_redo {
                            activity_state.set(ActivityState::Busy);
                            send_ui_command(UiCommand::Redo);
                        }
                    },
                    style: format!(
                        "background: {}; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;",
                        if undo_status().can_redo { "#3c3c3c" } else { "#2a2a2a" }
                    ),
                    "Redo"
                }
                button {
                    onclick: move |_| {
                        let next = !show_config();
                        let mut config = Config::load().unwrap_or_default();
                        config.ui.show_config = next;
                        let _ = config.save();
                        show_config.set(next);
                        config_text.set(config.render_for_display());
                    },
                    style: "background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;",
                    if show_config() { "Скрыть настройки" } else { "Настройки" }
                }
                button {
                    onclick: move |_| active_view.set(WorkspaceView::Auto),
                    style: "background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;",
                    "Auto"
                }
                button {
                    onclick: move |_| active_view.set(WorkspaceView::Table),
                    style: "background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;",
                    "Table"
                }
                button {
                    onclick: move |_| active_view.set(WorkspaceView::Timeline),
                    style: "background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;",
                    "Timeline"
                }
                button {
                    onclick: move |_| active_view.set(WorkspaceView::Cards),
                    style: "background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;",
                    "Cards"
                }
                button {
                    onclick: move |_| active_view.set(WorkspaceView::Readable),
                    style: "background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;",
                    "Readable"
                }
                button {
                    onclick: move |_| active_view.set(WorkspaceView::Chat),
                    style: "background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;",
                    "Chat"
                }
                div {
                    style: "display: flex; flex: 1; min-width: 280px;",
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
                        oninput: move |evt: Event<FormData>| input_text.set(evt.value().clone()),
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
            div {
                style: "
                    display: flex; justify-content: space-between; align-items: center;
                    padding: 8px 12px;
                    border-bottom: 1px solid #333;
                    background: #202020;
                    font-size: 12px;
                    color: #9aa0a6;
                ",
                div { "{workspace_title(active_view())}" }
                div { "Hybrid mode: snapshot + live broadcast" }
            }
            if let Some(state) = pending() {
                div {
                    style: "
                        margin: 12px;
                        padding: 12px;
                        border: 1px solid #664d00;
                        background: linear-gradient(180deg, #3b2f12 0%, #2a2417 100%);
                        border-radius: 6px;
                    ",
                    div { style: "font-size: 12px; color: #d7ba7d; text-transform: uppercase;", "Pending Confirmation" }
                    div { style: "font-size: 15px; color: #f3d98b; margin-top: 4px;", "{state.plan_summary.summary}" }
                    div { style: "font-size: 12px; color: #d4d4d4; margin-top: 6px;", "Risk: {state.plan_summary.risk:?}" }
                    if let Some(trust) = &state.plan_summary.trust {
                        div { style: "font-size: 12px; color: #d4d4d4; margin-top: 4px;", "Trust: {trust.trust_label} · {trust.reason}" }
                    }
                    div {
                        style: "margin-top: 8px; font-size: 12px; color: #c8c8c8;",
                        for cmd in &state.plan_summary.commands {
                            div { "• {cmd.argv.join(\" \")}" }
                        }
                    }
                    div {
                        style: "display: flex; gap: 8px; margin-top: 10px;",
                        button {
                            onclick: move |_| {
                                activity_state.set(ActivityState::Busy);
                                send_ui_command(UiCommand::Confirm);
                            },
                            style: "background: #0e639c; border: none; color: white; padding: 5px 12px; border-radius: 4px; cursor: pointer;",
                            "Accept"
                        }
                        button {
                            onclick: move |_| {
                                let _ = clear_pending_confirmation();
                                pending.set(None);
                                refresh_tick += 1;
                            },
                            style: "background: #5a2d2d; border: none; color: white; padding: 5px 12px; border-radius: 4px; cursor: pointer;",
                            "Decline"
                        }
                    }
                }
            }
            if show_config() {
                div {
                    style: "
                        margin: 0 12px 12px 12px;
                        padding: 12px;
                        border: 1px solid #333;
                        background: #252526;
                        border-radius: 6px;
                    ",
                    div { style: "font-size: 12px; color: #888; text-transform: uppercase; margin-bottom: 8px;", "Trust Settings" }
                    div {
                        style: "display: flex; gap: 8px; margin-bottom: 10px;",
                        button {
                            onclick: move |_| {
                                let mut config = Config::load().unwrap_or_default();
                                config.default_trust_policy = crate::trust::TrustPolicy::AlwaysAsk;
                                config.ui.last_selected_policy = Some("always_ask".to_string());
                                let _ = config.save();
                                config_text.set(config.render_for_display());
                                refresh_tick += 1;
                            },
                            style: "background: #4b2c2c; border: none; color: white; padding: 4px 10px; border-radius: 4px; cursor: pointer;",
                            "always_ask"
                        }
                        button {
                            onclick: move |_| {
                                let mut config = Config::load().unwrap_or_default();
                                config.default_trust_policy = crate::trust::TrustPolicy::AskOnce;
                                config.ui.last_selected_policy = Some("ask_once".to_string());
                                let _ = config.save();
                                config_text.set(config.render_for_display());
                                refresh_tick += 1;
                            },
                            style: "background: #5a4a20; border: none; color: white; padding: 4px 10px; border-radius: 4px; cursor: pointer;",
                            "ask_once"
                        }
                        button {
                            onclick: move |_| {
                                let mut config = Config::load().unwrap_or_default();
                                config.default_trust_policy = crate::trust::TrustPolicy::Allow;
                                config.ui.last_selected_policy = Some("allow".to_string());
                                let _ = config.save();
                                config_text.set(config.render_for_display());
                                refresh_tick += 1;
                            },
                            style: "background: #1f4d2b; border: none; color: white; padding: 4px 10px; border-radius: 4px; cursor: pointer;",
                            "allow"
                        }
                    }
                    pre {
                        style: "
                            margin: 0;
                            padding: 10px;
                            background: #1e1e1e;
                            border: 1px solid #333;
                            color: #cfcfcf;
                            border-radius: 4px;
                            white-space: pre-wrap;
                        ",
                        "{config_text}"
                    }
                }
            }
            div {
                style: "display: flex; flex: 1; min-height: 0;",
                div {
                    style: "flex: 1; overflow-y: auto;",
                    if rows.is_empty() {
                        div { style: "padding: 24px; color: #888; text-align: center;", "(лог пуст)" }
                    } else {
                        match active_view() {
                            WorkspaceView::Table => rsx! {
                                div {
                                    div {
                                        style: "
                                            display: grid;
                                            grid-template-columns: 160px 100px 80px 1fr 60px 80px 90px;
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
                                        div { "Trust" }
                                    }
                                    for row in rows.iter() {
                                        div {
                                            key: "{row.key}",
                                            onclick: {
                                                let key = row.key.clone();
                                                move |_| selected.set(Some(key.clone()))
                                            },
                                            style: "
                                                display: grid;
                                                grid-template-columns: 160px 100px 80px 1fr 60px 80px 90px;
                                                gap: 0;
                                                padding: 4px 12px;
                                                border-bottom: 1px solid #2d2d2d;
                                                font-size: 13px;
                                            ",
                                            div { style: "color: #888;", "{row.ts}" }
                                            div { style: "color: {row.kind_color};", "{row.kind}" }
                                            div { style: "color: {row.status_color}; font-weight: bold;", "{row.status}" }
                                            div { style: "overflow: hidden; text-overflow: ellipsis; white-space: nowrap;", "{truncate_safe(&row.desc, 120)}" }
                                            div { style: "color: #888;", "{row.exit}" }
                                            div { style: "color: #d7ba7d; font-size: 11px;", "{row.risk}" }
                                            div { style: "color: #9cdcfe; font-size: 11px;", "{row.trust}" }
                                        }
                                    }
                                }
                            },
                            WorkspaceView::Timeline => rsx! {
                                div {
                                    style: "padding: 12px;",
                                    for row in rows.iter() {
                                        div {
                                            key: "{row.key}",
                                            onclick: {
                                                let key = row.key.clone();
                                                move |_| selected.set(Some(key.clone()))
                                            },
                                            style: "display: flex; gap: 12px; padding: 10px 0; border-left: 2px solid #333; margin-left: 10px; padding-left: 14px;",
                                            div { style: "width: 120px; color: #888; flex-shrink: 0;", "{row.ts}" }
                                            div {
                                                style: "flex: 1;",
                                                div { style: "color: {row.kind_color}; font-weight: 600;", "{row.kind} · {row.status}" }
                                                div { style: "margin-top: 4px; color: #d4d4d4;", "{row.desc}" }
                                                div { style: "margin-top: 4px; color: #888; font-size: 11px;", "risk={row.risk} trust={row.trust}" }
                                            }
                                        }
                                    }
                                }
                            },
                            WorkspaceView::Cards => rsx! {
                                div {
                                    style: "padding: 12px; display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 12px;",
                                    for row in rows.iter() {
                                        div {
                                            key: "{row.key}",
                                            onclick: {
                                                let key = row.key.clone();
                                                move |_| selected.set(Some(key.clone()))
                                            },
                                            style: "padding: 12px; border: 1px solid #333; border-radius: 8px; background: #252526;",
                                            div { style: "font-size: 11px; color: #888;", "{row.ts}" }
                                            div { style: "margin-top: 6px; color: {row.kind_color}; font-weight: 700;", "{row.kind}" }
                                            div { style: "margin-top: 6px; color: #d4d4d4;", "{row.desc}" }
                                            div { style: "margin-top: 8px; color: {row.status_color};", "{row.status}" }
                                            div { style: "margin-top: 4px; color: #d7ba7d; font-size: 11px;", "risk={row.risk}" }
                                        }
                                    }
                                }
                            },
                            WorkspaceView::Readable => rsx! {
                                div {
                                    style: "padding: 18px; max-width: 920px; margin: 0 auto; line-height: 1.6;",
                                    for row in rows.iter() {
                                        div {
                                            key: "{row.key}",
                                            onclick: {
                                                let key = row.key.clone();
                                                move |_| selected.set(Some(key.clone()))
                                            },
                                            style: "padding: 12px 0; border-bottom: 1px solid #2d2d2d;",
                                            div { style: "font-size: 11px; color: #888;", "{row.ts} · {row.kind}" }
                                            div { style: "margin-top: 6px; color: #d4d4d4;", "{row.desc}" }
                                            if !row.stdout.is_empty() {
                                                div { style: "margin-top: 6px; color: #9cdcfe; white-space: pre-wrap;", "{row.stdout}" }
                                            }
                                        }
                                    }
                                }
                            },
                            WorkspaceView::Chat => rsx! {
                                div {
                                    style: "padding: 18px; max-width: 920px; margin: 0 auto;",
                                    for (idx, row) in rows.iter().enumerate() {
                                        div {
                                            key: "{row.key}",
                                            onclick: {
                                                let key = row.key.clone();
                                                move |_| selected.set(Some(key.clone()))
                                            },
                                            style: format!(
                                                "display: flex; justify-content: {}; margin: 10px 0;",
                                                if idx % 2 == 0 { "flex-start" } else { "flex-end" }
                                            ),
                                            div {
                                                style: format!(
                                                    "max-width: 78%; padding: 12px; border-radius: 12px; background: {}; border: 1px solid #333;",
                                                    if row.kind == "AgentTurn" || row.kind == "SystemEvent" { "#203040" } else { "#2a2a2a" }
                                                ),
                                                div { style: "font-size: 11px; color: #888;", "{row.kind} · {row.ts}" }
                                                div { style: "margin-top: 6px; color: #d4d4d4; white-space: pre-wrap;", "{row.desc}" }
                                                if !row.stdout.is_empty() {
                                                    div { style: "margin-top: 8px; color: #9cdcfe; white-space: pre-wrap;", "{row.stdout}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            WorkspaceView::Auto => rsx! {
                                div {
                                    style: "padding: 12px;",
                                    for (entry, row) in entries.iter().zip(rows.iter()) {
                                        match renderer_for_entry(entry, WorkspaceView::Auto) {
                                            EntryRenderer::Table => rsx! {
                                                div {
                                                    key: "{row.key}",
                                                    onclick: {
                                                        let key = row.key.clone();
                                                        move |_| selected.set(Some(key.clone()))
                                                    },
                                                    style: "display: grid; grid-template-columns: 160px 100px 80px 1fr; gap: 0; padding: 8px 0; border-bottom: 1px solid #2d2d2d;",
                                                    div { style: "color: #888;", "{row.ts}" }
                                                    div { style: "color: {row.kind_color};", "{row.kind}" }
                                                    div { style: "color: {row.status_color};", "{row.status}" }
                                                    div { "{row.desc}" }
                                                }
                                            },
                                            EntryRenderer::Timeline => rsx! {
                                                div {
                                                    key: "{row.key}",
                                                    onclick: {
                                                        let key = row.key.clone();
                                                        move |_| selected.set(Some(key.clone()))
                                                    },
                                                    style: "display: flex; gap: 12px; padding: 10px 0; border-left: 2px solid #333; margin-left: 10px; padding-left: 14px;",
                                                    div { style: "width: 120px; color: #888; flex-shrink: 0;", "{row.ts}" }
                                                    div {
                                                        div { style: "color: {row.kind_color}; font-weight: 600;", "{row.kind}" }
                                                        div { style: "margin-top: 4px;", "{row.desc}" }
                                                    }
                                                }
                                            },
                                            EntryRenderer::Card => rsx! {
                                                div {
                                                    key: "{row.key}",
                                                    onclick: {
                                                        let key = row.key.clone();
                                                        move |_| selected.set(Some(key.clone()))
                                                    },
                                                    style: "padding: 12px; border: 1px solid #333; border-radius: 8px; background: #252526; margin-bottom: 10px;",
                                                    div { style: "font-size: 11px; color: #888;", "{row.ts}" }
                                                    div { style: "margin-top: 6px; color: {row.kind_color};", "{row.kind}" }
                                                    div { style: "margin-top: 6px;", "{row.desc}" }
                                                }
                                            },
                                            EntryRenderer::Readable => rsx! {
                                                div {
                                                    key: "{row.key}",
                                                    onclick: {
                                                        let key = row.key.clone();
                                                        move |_| selected.set(Some(key.clone()))
                                                    },
                                                    style: "padding: 12px 0; border-bottom: 1px solid #2d2d2d;",
                                                    div { style: "font-size: 11px; color: #888;", "{row.ts} · {row.kind}" }
                                                    div { style: "margin-top: 6px; white-space: pre-wrap;", "{row.desc}" }
                                                }
                                            },
                                            EntryRenderer::Chat => rsx! {
                                                div {
                                                    key: "{row.key}",
                                                    onclick: {
                                                        let key = row.key.clone();
                                                        move |_| selected.set(Some(key.clone()))
                                                    },
                                                    style: "display: flex; justify-content: flex-start; margin: 10px 0;",
                                                    div {
                                                        style: "max-width: 78%; padding: 12px; border-radius: 12px; background: #203040; border: 1px solid #333;",
                                                        div { style: "font-size: 11px; color: #888;", "{row.kind} · {row.ts}" }
                                                        div { style: "margin-top: 6px; white-space: pre-wrap;", "{row.desc}" }
                                                    }
                                                }
                                            },
                                        }
                                    }
                                }
                            },
                        }
                    }
                }
                if let Some(key) = selected() {
                    if let Some(row) = rows.iter().find(|r| r.key == key) {
                        div {
                            style: "
                                width: 32%;
                                min-width: 320px;
                                border-left: 1px solid #333;
                                background: #252526;
                                padding: 12px;
                                overflow-y: auto;
                            ",
                            div { style: "font-size: 12px; color: #888; text-transform: uppercase;", "Details Pane" }
                            pre {
                                style: "
                                    margin: 8px 0 0 0;
                                    padding: 10px;
                                    border-radius: 4px;
                                    border: 1px solid #333;
                                    background: #1e1e1e;
                                    color: #d4d4d4;
                                    white-space: pre-wrap;
                                    overflow-x: auto;
                                ",
                                "{row.detail_text()}"
                            }
                            div { style: "margin-top: 10px; color: #888;", "stdout" }
                            pre {
                                style: "
                                    margin: 4px 0 0 0;
                                    padding: 10px;
                                    border-radius: 4px;
                                    border: 1px solid #333;
                                    background: #1e1e1e;
                                    color: #d4d4d4;
                                    white-space: pre-wrap;
                                    overflow-x: auto;
                                ",
                                if row.stdout.is_empty() { "—" } else { "{row.stdout}" }
                            }
                            div { style: "margin-top: 10px; color: #888;", "stderr" }
                            pre {
                                style: "
                                    margin: 4px 0 0 0;
                                    padding: 10px;
                                    border-radius: 4px;
                                    border: 1px solid #333;
                                    background: #1e1e1e;
                                    color: #d4d4d4;
                                    white-space: pre-wrap;
                                    overflow-x: auto;
                                ",
                                if row.stderr.is_empty() { "—" } else { "{row.stderr}" }
                            }
                        }
                    }
                }
            }
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
                    "↻ Refresh Snapshot"
                }
                div { "Live stream for in-process actions; refresh for external changes" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CostCounters, DisplayProfile, LogKind, LogStatus};

    #[test]
    fn test_trust_badge_for_cache_hit_uses_threshold_label() {
        let entry = LogEntry {
            schema_version: 1,
            instance_id: "i1".into(),
            session_id: "s1".into(),
            ts: "2026-06-24T00:00:00Z".into(),
            interaction_id: None,
            parent_interaction_id: None,
            kind: LogKind::ScriptRun,
            display_profile: DisplayProfile::default(),
            cost_counters: CostCounters::default(),
            request: None,
            cwd: None,
            risk: Some(crate::types::RiskLevel::ReadOnly),
            status: Some(LogStatus::Success),
            failure_kind: None,
            prompt_summary: None,
            plan: None,
            model_provider: None,
            model_name: None,
            duration_ms: None,
            tokens_used: None,
            command: None,
            exit: None,
            stdout_summary: None,
            stderr_summary: None,
            script_id: None,
            cache_hit: Some(true),
            model_called: Some(false),
            tokens_saved_estimate: None,
            success_count_before: Some(2),
            success_count_after: Some(3),
            steps: None,
            description: None,
        };

        assert_eq!(trust_badge(&entry), "auto ✓ 3/3");
    }

    #[test]
    fn test_prepare_rows_includes_stdout_and_stderr_details() {
        let mut entry = LogEntry::new_command_run(
            "i1",
            "s1",
            Some("int1".into()),
            "echo hello",
            "/tmp",
            &["echo".into(), "hello".into()],
            0,
            std::time::Duration::from_millis(1),
            "hello",
            "warn",
            CostCounters::default(),
        );
        entry.stdout_summary = Some("hello".into());
        entry.stderr_summary = Some("warn".into());
        let rows = prepare_rows(&[entry]);
        assert_eq!(rows[0].stdout, "hello");
        assert_eq!(rows[0].stderr, "warn");
    }

    #[test]
    fn test_undo_summary_label_prefers_summary() {
        let label = undo_summary_label(&UndoStatus {
            can_undo: true,
            can_redo: false,
            summary: Some("Create file".into()),
        });
        assert_eq!(label, "Create file");
    }

    #[test]
    fn test_undo_summary_label_has_fallback() {
        assert_eq!(
            undo_summary_label(&UndoStatus::default()),
            "undo/redo unavailable"
        );
    }

    #[test]
    fn test_renderer_for_entry_prefers_timeline_hint() {
        let mut entry = LogEntry::new_system_event("i1", "s1", "event");
        entry.display_profile.renderer_hint = RendererHint::Timeline;
        assert_eq!(
            renderer_for_entry(&entry, WorkspaceView::Auto),
            EntryRenderer::Timeline
        );
    }

    #[test]
    fn test_renderer_for_entry_maps_agent_turn_to_chat() {
        let entry = LogEntry {
            schema_version: 1,
            instance_id: "i1".into(),
            session_id: "s1".into(),
            ts: "2026-06-24T00:00:00Z".into(),
            interaction_id: None,
            parent_interaction_id: None,
            kind: LogKind::AgentTurn,
            display_profile: DisplayProfile::default(),
            cost_counters: CostCounters::default(),
            request: None,
            cwd: None,
            risk: None,
            status: Some(LogStatus::Success),
            failure_kind: None,
            prompt_summary: None,
            plan: None,
            model_provider: None,
            model_name: None,
            duration_ms: None,
            tokens_used: None,
            command: None,
            exit: None,
            stdout_summary: None,
            stderr_summary: None,
            script_id: None,
            cache_hit: None,
            model_called: Some(true),
            tokens_saved_estimate: None,
            success_count_before: None,
            success_count_after: None,
            steps: None,
            description: Some("agent".into()),
        };
        assert_eq!(
            renderer_for_entry(&entry, WorkspaceView::Auto),
            EntryRenderer::Chat
        );
    }

    #[test]
    fn test_is_completion_entry_for_system_event() {
        let entry = LogEntry::new_system_event("i1", "s1", "done");
        assert!(is_completion_entry(&entry));
    }
}
