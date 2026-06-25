// Renderer selection: maps LogEntry → EntryRenderer based on view and display_profile.
// Extracted from app.rs per audit recommendation (P0.4).

use crate::types::{DisplayType, LogEntry, LogKind, RendererHint};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntryRenderer {
    Table,
    Timeline,
    Card,
    Readable,
    Chat,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceView {
    Auto,
    Table,
    Timeline,
    Cards,
    Readable,
    Chat,
}

pub fn renderer_for_entry(entry: &LogEntry, view: WorkspaceView) -> EntryRenderer {
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

pub fn workspace_title(view: WorkspaceView) -> &'static str {
    match view {
        WorkspaceView::Auto => "Auto Workspace",
        WorkspaceView::Table => "Table View",
        WorkspaceView::Timeline => "Timeline View",
        WorkspaceView::Cards => "Card View",
        WorkspaceView::Readable => "Readable View",
        WorkspaceView::Chat => "Chat View",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

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
    fn test_renderer_for_entry_table_view_always_table() {
        let entry = LogEntry::new_system_event("i1", "s1", "event");
        assert_eq!(
            renderer_for_entry(&entry, WorkspaceView::Table),
            EntryRenderer::Table
        );
    }

    #[test]
    fn test_renderer_for_entry_chat_view_always_chat() {
        let entry = LogEntry::new_system_event("i1", "s1", "event");
        assert_eq!(
            renderer_for_entry(&entry, WorkspaceView::Chat),
            EntryRenderer::Chat
        );
    }

    #[test]
    fn test_workspace_title_has_all_variants() {
        assert_eq!(workspace_title(WorkspaceView::Auto), "Auto Workspace");
        assert_eq!(workspace_title(WorkspaceView::Table), "Table View");
        assert_eq!(workspace_title(WorkspaceView::Chat), "Chat View");
    }
}
