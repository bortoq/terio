// Terminal-like Dioxus UI for terio.
// Phase 0: black screen, input at bottom, output as windows, no modes.

use crate::window::{WindowKind, WindowManager};
use dioxus::prelude::*;

use super::state::{
    append_live_entry, get_entries, is_completion_entry, parse_input, refresh_entries,
    refresh_undo_status, send_ui_command, take_live_stream, undo_summary_label, ActivityState,
    UiCommand,
};

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
    // Use LaunchBuilder with explicit close behavior (Fix 4)
    #[cfg(feature = "desktop")]
    {
        use dioxus::desktop::{Config, WindowCloseBehaviour};
        dioxus::LaunchBuilder::new()
            .with_cfg(Config::new().with_close_behaviour(WindowCloseBehaviour::LastWindowExitsApp))
            .launch(app);
    }
    #[cfg(not(feature = "desktop"))]
    dioxus::launch(app);
}

/// Старое API (для обратной совместимости).
pub fn run() {
    run_with_entries(vec![]);
}

fn app() -> Element {
    let mut refresh_tick = use_signal(|| 0_u64);
    let live_rx = use_signal(take_live_stream);

    let mut input_text = use_signal(String::new);
    let mut undo_status = use_signal(refresh_undo_status);
    let mut activity_state = use_signal(|| ActivityState::Idle);

    // Fix 1: store input element for reliable autofocus and click-to-refocus
    let mut input_data = use_signal(|| None::<std::rc::Rc<MountedData>>);

    // FocusOut persistence: храним индекс фокуса между рендерами
    let mut focus_signal = use_signal(|| None::<usize>);
    let mut prev_entry_count = use_signal(|| 0_usize);
    // Scroll offset: viewport scroll position for scroll command
    let mut scroll_offset = use_signal(|| 0_i32);

    // Подписка на live-стрим
    use_future(move || {
        let mut refresh_tick = refresh_tick;
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
                undo_status.set(refresh_undo_status());
                if completion {
                    activity_state.set(ActivityState::Idle);
                }
                refresh_tick += 1;
            }
        }
    });

    // Input routing: parse input, handle locally or send to backend
    let mut on_submit = move |_| {
        let val = input_text();
        let val = val.trim().to_string();
        if !val.is_empty() {
            let cmd = parse_input(&val);
            match &cmd {
                UiCommand::Help => {
                    let msg = concat!(
                        "terio commands:\n",
                        "  help              this help\n",
                        "  mode <quiet|normal|debug>  attention mode\n",
                        "  focus <up|down>   focus output window\n",
                        "  scroll <N>        scroll output\n",
                        "  repeat            repeat last request\n",
                        "  y / confirm y     confirm pending plan\n",
                        "  n / confirm n     decline\n",
                        "  undo              undo last operation\n",
                        "  redo              redo last undo\n",
                        "  <anything else>    send as LLM ask"
                    );
                    append_system_event_direct(msg);
                    undo_status.set(refresh_undo_status());
                    refresh_tick += 1;
                }
                UiCommand::Mode(m) => {
                    let mut config = crate::config::Config::load().unwrap_or_default();
                    let result = config.set("attention_mode", m);
                    if let Err(ref e) = result {
                        append_system_event_direct(&format!("mode error: {e}"));
                    } else {
                        let _ = config.save();
                        append_system_event_direct(&format!("attention mode: {m}"));
                    }
                    undo_status.set(refresh_undo_status());
                    refresh_tick += 1;
                }
                UiCommand::Focus(direction) => {
                    // Local focus handling (clamped on render)
                    let cur = focus_signal.read().unwrap_or(0);
                    let new_focus = match direction.as_str() {
                        "up" | "↑" => cur.saturating_sub(1),
                        "down" | "↓" => cur.saturating_add(1),
                        _ => focus_signal.read().unwrap_or(0),
                    };
                    focus_signal.set(Some(new_focus));
                    activity_state.set(ActivityState::Idle);
                    refresh_tick += 1;
                }
                UiCommand::Scroll(n) => {
                    // Local scroll offset (clamped on render)
                    let new_offset = scroll_offset.read().saturating_add(*n).max(0);
                    scroll_offset.set(new_offset);
                    activity_state.set(ActivityState::Idle);
                    refresh_tick += 1;
                }
                UiCommand::Repeat => {
                    // Repeat goes to backend (needs log access)
                    activity_state.set(ActivityState::Busy);
                    send_ui_command(cmd);
                }
                _ => {
                    // Ask, Confirm, Undo, Redo — send to backend
                    activity_state.set(ActivityState::Busy);
                    send_ui_command(cmd);
                }
            }
        }
        input_text.set(String::new());
    };

    let _ = refresh_tick(); // force re-render (Dioxus reactivity)

    // Формируем окна из записей лога с persistent focus
    let entries = get_entries();
    let entry_count = entries.len();
    let mut mgr = WindowManager::from_log(&entries);

    // Restore focus: если entry_count не изменился, храним фокус; если изменился — сброс в конец
    if entry_count == *prev_entry_count.read() {
        if let Some(focus) = *focus_signal.read() {
            if focus < mgr.windows.len() {
                mgr.focus_out = Some(focus);
            }
        }
    } else {
        // Новые записи — фокус на последнее окно (from_log default)
        focus_signal.set(mgr.focus_out);
    }
    prev_entry_count.set(entry_count);
    focus_signal.set(mgr.focus_out);

    // Добавляем окно-подтверждение, если есть pending confirmation
    let mut windows = mgr.windows.clone();
    if let Some(pending) = load_pending_confirmation_direct() {
        let id = format!("__confirm__{}", pending.plan_hash);
        // Проверяем, не добавлено ли уже окно
        if !windows.iter().any(|w| w.id == id) {
            windows.push_back(crate::window::Window {
                id,
                kind: crate::window::WindowKind::Confirm {
                    prompt: pending.plan_summary.summary,
                },
                created_at: chrono::Utc::now().to_rfc3339(),
            });
            focus_signal.set(Some(windows.len() - 1));
        }
    }

    rsx! {
        div {
            onclick: move |_| {
                if let Some(ref data) = *input_data.read() {
                    drop(data.set_focus(true));
                }
            },
            style: "
                display: flex;
                flex-direction: column;
                height: 100vh;
                font-family: 'Courier New', 'Consolas', monospace;
                background: #000000;
                color: #d4d4d4;
                font-size: 13px;
                overflow: hidden;
            ",
            // Main output area — scrollable, fills all available space
            div {
                style: "flex: 1; overflow-y: auto; padding: 8px 12px; display: flex; flex-direction: column; justify-content: flex-end;",
                div {
                    style: "display: flex; flex-direction: column; gap: 2px;",
                    for (i, win) in windows.iter().enumerate() {
                        div {
                            key: "{win.id}",
                            style: format!("
                                white-space: pre-wrap;
                                word-wrap: break-word;
                                padding: 4px 6px;
                                border-left: 3px solid {};
                                margin-bottom: 2px;
                            ",
                                if Some(i) == *focus_signal.read() { "#569cd6" } else { "transparent" }
                            ),
                            "{render_window_content(win)}"
                        }
                    }
                }
            }
            // Bottom bar — activity status + undo info
            div {
                style: "
                    display: flex;
                    justify-content: space-between;
                    font-size: 11px;
                    color: #555;
                    padding: 2px 12px;
                    border-top: 1px solid #222;
                ",
                div {
                    style: format!(
                        "color: {};",
                        if activity_state() == ActivityState::Busy { "#d7ba7d" } else { "#6a9955" }
                    ),
                    if activity_state() == ActivityState::Busy { "●" } else { "●" }
                }
                div { style: "color: #555;", "{undo_summary_label(&undo_status())}" }
            }
            // Input line — always at the bottom
            div {
                style: "
                    display: flex;
                    align-items: center;
                    padding: 6px 12px;
                    border-top: 1px solid #333;
                    background: #0a0a0a;
                ",
                div { style: "color: #6a9955; margin-right: 8px; font-weight: bold;", "$" }
                input {
                    style: "
                        flex: 1;
                        background: transparent;
                        border: none;
                        color: #d4d4d4;
                        font-family: 'Courier New', 'Consolas', monospace;
                        font-size: 13px;
                        outline: none;
                        caret-color: #d4d4d4;
                    ",
                    placeholder: "введите команду...",
                    onmounted: move |evt| {
                        let data = evt.data();
                        drop(data.set_focus(true));
                        input_data.set(Some(data));
                    },
                    value: "{input_text}",
                    oninput: move |evt: Event<FormData>| input_text.set(evt.value().clone()),
                    onkeydown: move |evt: Event<KeyboardData>| {
                        if evt.key() == dioxus::events::Key::Enter {
                            on_submit(());
                        }
                        // Ctrl+L — очистить (refresh)
                        if evt.key() == dioxus::events::Key::Character("l".into()) && evt.modifiers().contains(dioxus::events::Modifiers::CONTROL) {
                            refresh_entries();
                            undo_status.set(refresh_undo_status());
                            refresh_tick += 1;
                        }
                    },
                }
            }
        }
    }
}

/// Helper: directly append a system event to the in-memory log (for local commands like help/mode).
fn append_system_event_direct(description: &str) {
    use crate::types::LogEntry;
    let entry = LogEntry::new_system_event("ui", "ui", description);
    append_live_entry(entry);
}

/// Helper: check if there's a pending confirmation on disk.
fn load_pending_confirmation_direct() -> Option<crate::ask::PendingConfirmationState> {
    crate::ask::load_pending_confirmation().ok().flatten()
}

fn render_window_content(win: &crate::window::Window) -> String {
    match &win.kind {
        WindowKind::Text(content) => content.clone(),
        WindowKind::Confirm { prompt } => format!("[?] {} [y/N]", prompt),
        WindowKind::Rich { url, mime } => format!("[{}] {}", mime, url),
    }
}
