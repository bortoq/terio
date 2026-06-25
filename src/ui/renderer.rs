// Renderer: maps LogEntry → terminal text for display as windows.
// Phase 0: removed EntryRenderer/WorkspaceView modes, replaced with Window::from_entry.

use crate::types::LogEntry;

/// Форматировать запись лога как строку для терминального вывода.
/// Используется для рендеринга окна.
pub fn render_entry_as_text(entry: &LogEntry, _idx: usize) -> String {
    let mut lines = Vec::new();

    if let Some(ref cmd) = entry.command {
        lines.push(format!("$ {}", cmd.display));
    } else if let Some(ref desc) = entry.description {
        lines.push(desc.clone());
    } else if let Some(ref summary) = entry.prompt_summary {
        lines.push(summary.clone());
    }

    if let Some(ref out) = entry.stdout_summary {
        if !out.is_empty() {
            lines.push(out.clone());
        }
    }

    if let Some(ref err) = entry.stderr_summary {
        if !err.is_empty() {
            lines.push(format!("[stderr]\n{}", err));
        }
    }

    if let Some(exit) = entry.exit {
        if exit != 0 {
            let status = entry
                .status
                .as_ref()
                .map(|s| format!("{:?}", s))
                .unwrap_or_default();
            lines.push(format!("[exit {} {}]", exit, status));
        }
    }

    let content = lines.join("\n");
    if content.is_empty() {
        "(no output)".to_string()
    } else {
        content
    }
}

/// Разделитель между окнами.
pub const WINDOW_SEPARATOR: &str = "───";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn test_render_entry_command_run() {
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
            "",
            CostCounters::default(),
        );
        entry.stdout_summary = Some("hello".into());
        let text = render_entry_as_text(&entry, 0);
        assert!(text.contains("$ echo hello"));
        assert!(text.contains("hello"));
    }

    #[test]
    fn test_render_entry_system_event() {
        let entry = LogEntry::new_system_event("i1", "s1", "system message");
        let text = render_entry_as_text(&entry, 0);
        assert!(text.contains("system message"));
    }

    #[test]
    fn test_render_entry_empty_fallback() {
        let mut entry = LogEntry::new_system_event("i1", "s1", "");
        // new_system_event sets description to ""
        entry.description = None;
        let text = render_entry_as_text(&entry, 0);
        assert_eq!(text, "(no output)");
    }
}
