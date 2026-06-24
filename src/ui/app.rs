// Dioxus webview UI — primary interface for terio.
// Receives log entries from main via static (refreshable with F5).

use crate::ask::{clear_pending_confirmation, load_pending_confirmation};
use crate::config::Config;
use crate::log::LogReader;
use crate::trust::trust_level_str;
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
    trust: String,
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
                .or(entry.description.as_deref())
                .unwrap_or("—")
                .to_string();
            let exit = entry.exit.map(|e| e.to_string()).unwrap_or_default();
            let risk = risk_label(entry);
            let trust = trust_badge(entry);
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

fn run_terio_args(args: &[String]) {
    let _ = std::process::Command::new("terio")
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut c| c.wait());
}

fn app() -> Element {
    let mut refresh_tick = use_signal(|| 0_u64);
    // Read current entries
    let entries_guard = LOG_ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
    let rows = prepare_rows(&entries_guard);
    drop(entries_guard);

    let mut input_text = use_signal(String::new);
    let mut pending = use_signal(|| load_pending_confirmation().ok().flatten());
    let initial_config = Config::load().unwrap_or_default();
    let mut show_config = use_signal(|| initial_config.ui.show_config);
    let mut config_text = use_signal(|| initial_config.render_for_display());

    let on_submit = move |_| {
        let val = input_text();
        let val = val.trim().to_string();
        if !val.is_empty() {
            run_terio_args(&["ask".to_string(), val.clone()]);
            refresh_entries();
            pending.set(load_pending_confirmation().ok().flatten());
            config_text.set(Config::load().unwrap_or_default().render_for_display());
            refresh_tick += 1;
        }
        input_text.set(String::new());
    };

    let on_f5 = move |_| {
        refresh_entries();
        pending.set(load_pending_confirmation().ok().flatten());
        config_text.set(Config::load().unwrap_or_default().render_for_display());
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
                button {
                    onclick: move |_| {
                        let next = !show_config();
                        let mut config = Config::load().unwrap_or_default();
                        config.ui.show_config = next;
                        let _ = config.save();
                        show_config.set(next);
                        config_text.set(config.render_for_display());
                    },
                    style: "
                        background: #3c3c3c; border: 1px solid #555;
                        color: #d4d4d4; padding: 3px 10px;
                        border-radius: 3px; cursor: pointer; font-size: 12px;
                    ",
                    if show_config() { "Скрыть настройки" } else { "Настройки" }
                }
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
                                run_terio_args(&[
                                    "ask".to_string(),
                                    state.plan_summary.request.clone(),
                                    "--yes".to_string(),
                                ]);
                                let _ = clear_pending_confirmation();
                                refresh_entries();
                                pending.set(load_pending_confirmation().ok().flatten());
                                refresh_tick += 1;
                            },
                            style: "
                                background: #0e639c; border: none; color: white;
                                padding: 5px 12px; border-radius: 4px; cursor: pointer;
                            ",
                            "Accept"
                        }
                        button {
                            onclick: move |_| {
                                let _ = clear_pending_confirmation();
                                pending.set(None);
                                refresh_tick += 1;
                            },
                            style: "
                                background: #5a2d2d; border: none; color: white;
                                padding: 5px 12px; border-radius: 4px; cursor: pointer;
                            ",
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
                    div { style: "display: flex; gap: 8px; margin-bottom: 10px;",
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
            // Table header
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
                                grid-template-columns: 160px 100px 80px 1fr 60px 80px 90px;
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
                            div { style: "color: #9cdcfe; font-size: 11px;", "{row.trust}" }
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
}
