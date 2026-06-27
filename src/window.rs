// Window model: core abstraction for Phase 0 terminal-like UI.
// Each result from terio is a Window. WindowManager manages the viewport.

use crate::types::{LogEntry, LogEvent, TerioPrediction};
use std::collections::VecDeque;

/// Тип окна.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowKind {
    /// Обычный текст (stdout, лог, ответ)
    Text(String),
    /// Подтверждение: prompt + команда, которая выполнится при y
    Confirm { prompt: String },
    /// Rich-медиа (Phase 2): url + mime-тип для рендеринга плеером/браузером
    Rich { url: String, mime: String },
}

/// Окно — базовый элемент отображения.
#[derive(Debug, Clone)]
pub struct Window {
    pub id: String,
    pub kind: WindowKind,
    pub created_at: String,
}

impl Window {
    /// Create a window from a TerioPrediction.
    pub fn from_tp(tp: &TerioPrediction, event_idx: usize, tp_idx: usize) -> Self {
        let id = format!(
            "{}-{:?}-{}",
            tp.interaction_id.as_deref().unwrap_or("none"),
            tp.kind,
            (event_idx << 16) | tp_idx
        );
        let content = tp_content(tp);
        Self {
            id,
            kind: WindowKind::Text(content),
            created_at: tp.ts.clone(),
        }
    }

    /// Legacy: create from raw LogEntry (for tests / backward compat).
    pub fn from_entry(entry: &LogEntry, idx: usize) -> Self {
        let id = format!(
            "{}-{:?}-{}",
            entry.interaction_id.as_deref().unwrap_or("none"),
            entry.kind,
            idx
        );
        let content = entry_content(entry);
        Self {
            id,
            kind: WindowKind::Text(content),
            created_at: entry.ts.clone(),
        }
    }
}

fn tp_content(tp: &TerioPrediction) -> String {
    let mut lines = Vec::new();
    if let Some(ref cmd) = tp.command {
        lines.push(format!("$ {}", cmd.display));
    } else if let Some(ref desc) = tp.description {
        lines.push(desc.clone());
    } else if let Some(ref summary) = tp.prompt_summary {
        lines.push(summary.clone());
    }
    if let Some(ref out) = tp.stdout_summary {
        if !out.is_empty() {
            lines.push(out.clone());
        }
    }
    if let Some(ref err) = tp.stderr_summary {
        if !err.is_empty() {
            lines.push(format!("[stderr]\n{}", err));
        }
    }
    lines.join("\n")
}

fn entry_content(entry: &LogEntry) -> String {
    let mut lines = Vec::new();

    // Description / command display
    if let Some(ref cmd) = entry.command {
        lines.push(format!("$ {}", cmd.display));
    } else if let Some(ref desc) = entry.description {
        lines.push(desc.clone());
    } else if let Some(ref summary) = entry.prompt_summary {
        lines.push(summary.clone());
    }

    // stdout
    if let Some(ref out) = entry.stdout_summary {
        if !out.is_empty() {
            lines.push(out.clone());
        }
    }

    // stderr
    if let Some(ref err) = entry.stderr_summary {
        if !err.is_empty() {
            lines.push(format!("[stderr]\n{}", err));
        }
    }

    // Exit code & status
    if let Some(exit) = entry.exit {
        let status = entry
            .status
            .as_ref()
            .map(|s| format!("{:?}", s))
            .unwrap_or_default();
        if exit != 0 {
            lines.push(format!("[exit {} {}]", exit, status));
        }
    }

    lines.join("\n")
}

/// Управление окнами: список, фокус, скролл.
pub struct WindowManager {
    pub windows: VecDeque<Window>,
    /// Индекс окна вывода, находящегося в фокусе (FocusOut).
    pub focus_out: Option<usize>,
    /// Максимальное количество окон в видимой области.
    pub max_visible: usize,
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            windows: VecDeque::new(),
            focus_out: None,
            max_visible: 50,
        }
    }

    /// Восстановить окна из событий лога (Phase 7).
    pub fn from_log(entries: &[LogEvent]) -> Self {
        let windows: VecDeque<Window> = entries
            .iter()
            .enumerate()
            .flat_map(|(ei, event)| {
                event
                    .terio_predictions
                    .iter()
                    .enumerate()
                    .map(move |(ti, tp)| Window::from_tp(tp, ei, ti))
            })
            .collect();
        let count = windows.len();
        Self {
            windows,
            focus_out: if count > 0 { Some(count - 1) } else { None },
            max_visible: 50,
        }
    }

    /// Добавить новое окно из события лога (в конец).
    pub fn push(&mut self, event: &LogEvent) {
        for (ti, tp) in event.terio_predictions.iter().enumerate() {
            let _idx = self.windows.len();
            let window = Window::from_tp(tp, self.windows.len(), ti);
            self.windows.push_back(window);
        }
        // Ограничение размера
        if self.windows.len() > self.max_visible {
            self.windows.pop_front();
        }
        // Фокус на новое окно
        self.focus_out = Some(self.windows.len().saturating_sub(1));
    }

    /// Переключить FocusOut вверх/вниз.
    pub fn focus_move(&mut self, direction: &str) {
        let len = self.windows.len();
        if len == 0 {
            return;
        }
        let current = self.focus_out.unwrap_or(len - 1);
        self.focus_out = Some(match direction {
            "up" | "↑" => current.saturating_sub(1),
            "down" | "↓" => (current + 1).min(len - 1),
            _ => current,
        });
    }

    /// Получить окно в фокусе.
    pub fn focused_window(&self) -> Option<&Window> {
        self.focus_out.and_then(|i| self.windows.get(i))
    }

    /// Отрендерить все окна как текст (для терминального вывода).
    pub fn render_all(&self, _max_lines: usize) -> Vec<String> {
        self.windows
            .iter()
            .map(|w| match &w.kind {
                WindowKind::Text(content) => content.clone(),
                WindowKind::Confirm { prompt } => format!("[confirm] {}", prompt),
                WindowKind::Rich { url, mime } => format!("[{}] {}", mime, url),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn test_window_manager_empty() {
        let mgr = WindowManager::new();
        assert!(mgr.windows.is_empty());
        assert!(mgr.focused_window().is_none());
    }

    #[test]
    fn test_window_manager_push_and_focus() {
        use crate::types::LogEvent;
        let mut mgr = WindowManager::new();
        let entry = LogEntry::new_system_event("i1", "s1", "hello");
        let event = LogEvent::from_entry(&entry);
        mgr.push(&event);
        assert_eq!(mgr.windows.len(), 1);
        assert!(mgr.focused_window().is_some());
    }

    #[test]
    fn test_window_from_entry_system_event() {
        let entry = LogEntry::new_system_event("i1", "s1", "test event");
        let win = Window::from_entry(&entry, 0);
        assert!(win.id.contains("SystemEvent-0"));
        match &win.kind {
            WindowKind::Text(content) => assert!(content.contains("test event")),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_window_from_entry_command_run() {
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
        let win = Window::from_entry(&entry, 0);
        match &win.kind {
            WindowKind::Text(content) => {
                assert!(content.contains("$ echo hello"));
                assert!(content.contains("hello"));
            }
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_focus_move_up_down() {
        use crate::types::LogEvent;
        let mut mgr = WindowManager::new();
        mgr.push(&LogEvent::from_entry(&LogEntry::new_system_event(
            "i1", "s1", "a",
        )));
        mgr.push(&LogEvent::from_entry(&LogEntry::new_system_event(
            "i1", "s1", "b",
        )));
        mgr.push(&LogEvent::from_entry(&LogEntry::new_system_event(
            "i1", "s1", "c",
        )));
        assert_eq!(mgr.focus_out, Some(2)); // focus on last

        mgr.focus_move("up");
        assert_eq!(mgr.focus_out, Some(1));

        mgr.focus_move("up");
        assert_eq!(mgr.focus_out, Some(0));

        mgr.focus_move("up"); // clamp at 0
        assert_eq!(mgr.focus_out, Some(0));

        mgr.focus_move("down");
        assert_eq!(mgr.focus_out, Some(1));
    }

    #[test]
    fn test_window_manager_from_log() {
        use crate::types::LogEvent;
        let entries = vec![
            LogEntry::new_system_event("i1", "s1", "first"),
            LogEntry::new_system_event("i1", "s1", "second"),
        ];
        let events: Vec<LogEvent> = entries.iter().map(|e| LogEvent::from_entry(e)).collect();
        let mgr = WindowManager::from_log(&events);
        assert_eq!(mgr.windows.len(), 2);
        assert_eq!(mgr.focus_out, Some(1));
    }
}
