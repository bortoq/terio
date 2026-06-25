// Dioxus webview UI — primary interface for terio.
// Uses extracted state.rs and renderer.rs modules (audit P0.4 refactor).

use crate::ask::load_pending_confirmation;
use crate::config::Config;
use dioxus::prelude::*;

use super::renderer::{workspace_title, EntryRenderer, WorkspaceView};
use super::state::{
    append_live_entry, get_entries, is_completion_entry, prepare_rows, refresh_entries,
    refresh_undo_status, send_ui_command, take_live_stream, truncate_safe, undo_summary_label,
    ActivityState,
};

// Re-export for main.rs
pub use super::state::UiCommand;

/// Запускает Dioxus-окно с переданными записями лога.
pub fn run_with_entries(entries: Vec<crate::types::LogEntry>) {
    run_with_entries_and_runtime(entries, None, None);
}

pub fn run_with_entries_and_runtime(
    entries: Vec<crate::types::LogEntry>,
    stream: Option<tokio::sync::broadcast::Receiver<crate::types::LogEntry>>,
    sender: Option<std::sync::mpsc::Sender<UiCommand>>,
) {
    super::state::init_globals(entries, stream, sender);
    dioxus::launch(app);
}

/// Старый API без аргументов (для обратной совместимости, не используется).
pub fn run() {
    run_with_entries(vec![]);
}

fn app() -> Element {
    let mut refresh_tick = use_signal(|| 0_u64);
    let live_rx = use_signal(|| take_live_stream());

    let entries = get_entries();
    let rows = prepare_rows(&entries);

    let mut input_text = use_signal(String::new);
    let mut pending = use_signal(|| load_pending_confirmation().ok().flatten());
    let selected = use_signal(|| None::<String>);
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
            // Header
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
                button { onclick: move |_| active_view.set(WorkspaceView::Auto), style: "background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;", "Auto" }
                button { onclick: move |_| active_view.set(WorkspaceView::Table), style: "background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;", "Table" }
                button { onclick: move |_| active_view.set(WorkspaceView::Timeline), style: "background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;", "Timeline" }
                button { onclick: move |_| active_view.set(WorkspaceView::Cards), style: "background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;", "Cards" }
                button { onclick: move |_| active_view.set(WorkspaceView::Readable), style: "background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;", "Readable" }
                button { onclick: move |_| active_view.set(WorkspaceView::Chat), style: "background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;", "Chat" }
                button {
                    onclick: move |_| { activity_state.set(ActivityState::Idle); refresh_tick += 1; },
                    style: "background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 3px 10px; border-radius: 3px; cursor: pointer; font-size: 12px;",
                    "Integrations"
                }
                div {
                    style: "display: flex; flex: 1; min-width: 280px;",
                    input {
                        style: "flex: 1; background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 4px 8px; border-radius: 3px; font-size: 13px; outline: none;",
                        placeholder: "Введите запрос...",
                        value: "{input_text}",
                        oninput: move |evt: Event<FormData>| input_text.set(evt.value().clone()),
                    }
                    button {
                        onclick: on_submit,
                        style: "margin-left: 6px; background: #0e639c; border: none; color: white; padding: 4px 12px; border-radius: 3px; font-size: 13px; cursor: pointer;",
                        "Ask"
                    }
                }
            }
            // Workspace title bar
            div {
                style: "display: flex; justify-content: space-between; align-items: center; padding: 8px 12px; border-bottom: 1px solid #333; background: #202020; font-size: 12px; color: #9aa0a6;",
                div { "{workspace_title(active_view())}" }
                div { "Hybrid mode: snapshot + live broadcast" }
            }
            // Pending confirmation
            if let Some(state) = pending() {
                div {
                    style: "margin: 12px; padding: 12px; border: 1px solid #664d00; background: linear-gradient(180deg, #3b2f12 0%, #2a2417 100%); border-radius: 6px;",
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
                            onclick: move |_| { activity_state.set(ActivityState::Busy); send_ui_command(UiCommand::Confirm); },
                            style: "background: #0e639c; border: none; color: white; padding: 5px 12px; border-radius: 4px; cursor: pointer;",
                            "Accept"
                        }
                        button {
                            onclick: move |_| {
                                let _ = crate::ask::clear_pending_confirmation();
                                pending.set(None);
                                refresh_tick += 1;
                            },
                            style: "background: #5a2d2d; border: none; color: white; padding: 5px 12px; border-radius: 4px; cursor: pointer;",
                            "Decline"
                        }
                    }
                }
            }
            // Config panel
            if show_config() {
                div {
                    style: "margin: 0 12px 12px 12px; padding: 12px; border: 1px solid #333; background: #252526; border-radius: 6px;",
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
                    pre { style: "margin: 0; padding: 10px; background: #1e1e1e; border: 1px solid #333; color: #cfcfcf; border-radius: 4px; white-space: pre-wrap;", "{config_text}" }
                }
            }
            // Main content
            div {
                style: "display: flex; flex: 1; min-height: 0;",
                div {
                    style: "flex: 1; overflow-y: auto;",
                    if rows.is_empty() {
                        div { style: "padding: 24px; color: #888; text-align: center;", "(лог пуст)" }
                    } else {
                        match active_view() {
                            WorkspaceView::Table => rsx! { table_renderer { rows: rows.clone(), selected: selected } },
                            WorkspaceView::Timeline => rsx! { timeline_renderer { rows: rows.clone(), selected: selected } },
                            WorkspaceView::Cards => rsx! { cards_renderer { rows: rows.clone(), selected: selected } },
                            WorkspaceView::Readable => rsx! { readable_renderer { rows: rows.clone(), selected: selected } },
                            WorkspaceView::Chat => rsx! { chat_renderer { rows: rows.clone(), selected: selected } },
                            WorkspaceView::Auto => rsx! { auto_renderer { entries: entries.clone(), rows: rows.clone(), selected: selected } },
                        }
                    }
                }
                if let Some(sel_key) = selected() {
                    details_renderer { rows: rows.clone(), row_key: sel_key }
                }
            }
            // Footer
            div {
                style: "display: flex; justify-content: space-between; align-items: center; font-size: 11px; color: #555; padding: 4px 12px; border-top: 1px solid #333;",
                button {
                    onclick: on_f5,
                    style: "background: #3c3c3c; border: 1px solid #555; color: #d4d4d4; padding: 2px 10px; border-radius: 3px; cursor: pointer; font-size: 11px;",
                    "↻ Refresh Snapshot"
                }
                div { "Live stream for in-process actions; refresh for external changes" }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Renderer components (Dioxus component wrappers with #[component])
// These accept props and return Element for use in rsx!
// ---------------------------------------------------------------------------

#[component]
fn table_renderer(
    rows: Vec<super::state::RowData>,
    mut selected: Signal<Option<String>>,
) -> Element {
    rsx! {
        div {
            div {
                style: "display: grid; grid-template-columns: 160px 100px 80px 1fr 60px 80px 90px; gap: 0; background: #252526; padding: 6px 12px; font-size: 11px; text-transform: uppercase; color: #888; border-bottom: 1px solid #333;",
                div { "Время" } div { "Тип" } div { "Статус" } div { "Команда / Описание" } div { "Код" } div { "Риск" } div { "Trust" }
            }
            for row in rows.iter() {
                div {
                    key: "{row.key}",
                    onclick: {
                        let key = row.key.clone();
                        move |_| { selected.set(Some(key.clone())); }
                    },
                    style: "display: grid; grid-template-columns: 160px 100px 80px 1fr 60px 80px 90px; gap: 0; padding: 4px 12px; border-bottom: 1px solid #2d2d2d; font-size: 13px;",
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
    }
}

#[component]
fn timeline_renderer(
    rows: Vec<super::state::RowData>,
    mut selected: Signal<Option<String>>,
) -> Element {
    rsx! {
        div { style: "padding: 12px;",
            for row in rows.iter() {
                div {
                    key: "{row.key}",
                    onclick: { let key = row.key.clone(); move |_| { selected.set(Some(key.clone())); } },
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
    }
}

#[component]
fn cards_renderer(
    rows: Vec<super::state::RowData>,
    mut selected: Signal<Option<String>>,
) -> Element {
    rsx! {
        div { style: "padding: 12px; display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 12px;",
            for row in rows.iter() {
                div {
                    key: "{row.key}",
                    onclick: { let key = row.key.clone(); move |_| { selected.set(Some(key.clone())); } },
                    style: "padding: 12px; border: 1px solid #333; border-radius: 8px; background: #252526;",
                    div { style: "font-size: 11px; color: #888;", "{row.ts}" }
                    div { style: "margin-top: 6px; color: {row.kind_color}; font-weight: 700;", "{row.kind}" }
                    div { style: "margin-top: 6px; color: #d4d4d4;", "{row.desc}" }
                    div { style: "margin-top: 8px; color: {row.status_color};", "{row.status}" }
                    div { style: "margin-top: 4px; color: #d7ba7d; font-size: 11px;", "risk={row.risk}" }
                }
            }
        }
    }
}

#[component]
fn readable_renderer(
    rows: Vec<super::state::RowData>,
    mut selected: Signal<Option<String>>,
) -> Element {
    rsx! {
        div { style: "padding: 18px; max-width: 920px; margin: 0 auto; line-height: 1.6;",
            for row in rows.iter() {
                div {
                    key: "{row.key}",
                    onclick: { let key = row.key.clone(); move |_| { selected.set(Some(key.clone())); } },
                    style: "padding: 12px 0; border-bottom: 1px solid #2d2d2d;",
                    div { style: "font-size: 11px; color: #888;", "{row.ts} · {row.kind}" }
                    div { style: "margin-top: 6px; color: #d4d4d4;", "{row.desc}" }
                    if !row.stdout.is_empty() {
                        div { style: "margin-top: 6px; color: #9cdcfe; white-space: pre-wrap;", "{row.stdout}" }
                    }
                }
            }
        }
    }
}

#[component]
fn chat_renderer(
    rows: Vec<super::state::RowData>,
    mut selected: Signal<Option<String>>,
) -> Element {
    rsx! {
        div { style: "padding: 18px; max-width: 920px; margin: 0 auto;",
            for (idx, row) in rows.iter().enumerate() {
                div {
                    key: "{row.key}",
                    onclick: { let key = row.key.clone(); move |_| { selected.set(Some(key.clone())); } },
                    style: format!("display: flex; justify-content: {}; margin: 10px 0;",
                        if idx % 2 == 0 { "flex-start" } else { "flex-end" }
                    ),
                    div {
                        style: format!("max-width: 78%; padding: 12px; border-radius: 12px; background: {}; border: 1px solid #333;",
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
    }
}

#[component]
fn auto_renderer(
    entries: Vec<crate::types::LogEntry>,
    rows: Vec<super::state::RowData>,
    mut selected: Signal<Option<String>>,
) -> Element {
    rsx! {
        div { style: "padding: 12px;",
            for (entry, row) in entries.iter().zip(rows.iter()) {
                match super::renderer::renderer_for_entry(entry, WorkspaceView::Auto) {
                    EntryRenderer::Table => rsx! {
                        div {
                            key: "{row.key}",
                            onclick: { let key = row.key.clone(); move |_| { selected.set(Some(key.clone())); } },
                            style: "display: grid; grid-template-columns: 160px 100px 80px 1fr; gap: 0; padding: 8px 0; border-bottom: 1px solid #2d2d2d;",
                            div { style: "color: #888;", "{row.ts}" } div { style: "color: {row.kind_color};", "{row.kind}" }
                            div { style: "color: {row.status_color};", "{row.status}" } div { "{row.desc}" }
                        }
                    },
                    EntryRenderer::Timeline => rsx! {
                        div {
                            key: "{row.key}",
                            onclick: { let key = row.key.clone(); move |_| { selected.set(Some(key.clone())); } },
                            style: "display: flex; gap: 12px; padding: 10px 0; border-left: 2px solid #333; margin-left: 10px; padding-left: 14px;",
                            div { style: "width: 120px; color: #888; flex-shrink: 0;", "{row.ts}" }
                            div { div { style: "color: {row.kind_color}; font-weight: 600;", "{row.kind}" } div { style: "margin-top: 4px;", "{row.desc}" } }
                        }
                    },
                    EntryRenderer::Card => rsx! {
                        div {
                            key: "{row.key}",
                            onclick: { let key = row.key.clone(); move |_| { selected.set(Some(key.clone())); } },
                            style: "padding: 12px; border: 1px solid #333; border-radius: 8px; background: #252526; margin-bottom: 10px;",
                            div { style: "font-size: 11px; color: #888;", "{row.ts}" } div { style: "margin-top: 6px; color: {row.kind_color};", "{row.kind}" }
                            div { style: "margin-top: 6px;", "{row.desc}" }
                        }
                    },
                    EntryRenderer::Readable => rsx! {
                        div {
                            key: "{row.key}",
                            onclick: { let key = row.key.clone(); move |_| { selected.set(Some(key.clone())); } },
                            style: "padding: 12px 0; border-bottom: 1px solid #2d2d2d;",
                            div { style: "font-size: 11px; color: #888;", "{row.ts} · {row.kind}" } div { style: "margin-top: 6px; white-space: pre-wrap;", "{row.desc}" }
                        }
                    },
                    EntryRenderer::Chat => rsx! {
                        div {
                            key: "{row.key}",
                            onclick: { let key = row.key.clone(); move |_| { selected.set(Some(key.clone())); } },
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
    }
}

#[component]
fn details_renderer(
    rows: Vec<super::state::RowData>,
    row_key: String,
) -> Element {
    let row = rows.iter().find(|r| r.key == row_key);
    rsx! {
        if let Some(row) = row {
            div {
                style: "width: 32%; min-width: 320px; border-left: 1px solid #333; background: #252526; padding: 12px; overflow-y: auto;",
                div { style: "font-size: 12px; color: #888; text-transform: uppercase;", "Details Pane" }
                pre { style: "margin: 8px 0 0 0; padding: 10px; border-radius: 4px; border: 1px solid #333; background: #1e1e1e; color: #d4d4d4; white-space: pre-wrap; overflow-x: auto;", "{row.detail_text()}" }
                div { style: "margin-top: 10px; color: #888;", "stdout" }
                pre { style: "margin: 4px 0 0 0; padding: 10px; border-radius: 4px; border: 1px solid #333; background: #1e1e1e; color: #d4d4d4; white-space: pre-wrap; overflow-x: auto;", if row.stdout.is_empty() { "—" } else { "{row.stdout}" } }
                div { style: "margin-top: 10px; color: #888;", "stderr" }
                pre { style: "margin: 4px 0 0 0; padding: 10px; border-radius: 4px; border: 1px solid #333; background: #1e1e1e; color: #d4d4d4; white-space: pre-wrap; overflow-x: auto;", if row.stderr.is_empty() { "—" } else { "{row.stderr}" } }
            }
        }
    }
}
