#[cfg(test)]
mod tests {
    use executors::logs::{
        ActionType, NormalizedEntry, NormalizedEntryType, TokenUsageInfo, ToolStatus,
    };

    use crate::routes::trajectory::{
        TokenUsageSummary, TrajectoryCompleteness, TrajectoryResponse, TrajectoryTotals,
        event_from_entry, update_totals_from_entry,
    };

    #[test]
    fn test_update_totals_from_entry_user_message() {
        let mut totals = TrajectoryTotals::default();

        let entry = NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::UserMessage,
            content: "test message".to_string(),
            metadata: None,
        };

        update_totals_from_entry(&entry, &mut totals);

        assert_eq!(*totals.entries_by_type.get("user_message").unwrap(), 1);
        assert_eq!(totals.tool_calls_by_status.len(), 0);
    }

    #[test]
    fn test_update_totals_from_entry_tool_use() {
        let mut totals = TrajectoryTotals::default();

        let entry = NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "bash".to_string(),
                action_type: ActionType::Tool {
                    tool_name: "bash".to_string(),
                    arguments: None,
                    result: None,
                },
                status: ToolStatus::Success,
            },
            content: "command executed".to_string(),
            metadata: None,
        };

        update_totals_from_entry(&entry, &mut totals);

        assert_eq!(*totals.entries_by_type.get("tool_use").unwrap(), 1);
        assert_eq!(*totals.tool_calls_by_status.get("success").unwrap(), 1);
    }

    #[test]
    fn test_update_totals_from_entry_token_usage() {
        let mut totals = TrajectoryTotals::default();

        let entry = NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::TokenUsageInfo(TokenUsageInfo {
                total_tokens: 1000,
                model_context_window: 128000,
            }),
            content: "".to_string(),
            metadata: None,
        };

        update_totals_from_entry(&entry, &mut totals);

        assert_eq!(*totals.entries_by_type.get("token_usage_info").unwrap(), 1);
        assert!(totals.last_token_usage.is_some());
        assert_eq!(totals.last_token_usage.as_ref().unwrap().total_tokens, 1000);
    }

    #[test]
    fn test_update_totals_multiple_entries() {
        let mut totals = TrajectoryTotals::default();

        // Add user message
        update_totals_from_entry(
            &NormalizedEntry {
                timestamp: None,
                entry_type: NormalizedEntryType::UserMessage,
                content: "msg1".to_string(),
                metadata: None,
            },
            &mut totals,
        );

        // Add another user message
        update_totals_from_entry(
            &NormalizedEntry {
                timestamp: None,
                entry_type: NormalizedEntryType::UserMessage,
                content: "msg2".to_string(),
                metadata: None,
            },
            &mut totals,
        );

        // Add assistant message
        update_totals_from_entry(
            &NormalizedEntry {
                timestamp: None,
                entry_type: NormalizedEntryType::AssistantMessage,
                content: "response".to_string(),
                metadata: None,
            },
            &mut totals,
        );

        assert_eq!(*totals.entries_by_type.get("user_message").unwrap(), 2);
        assert_eq!(*totals.entries_by_type.get("assistant_message").unwrap(), 1);
    }

    #[test]
    fn test_tool_status_serialization() {
        let totals = TrajectoryTotals {
            entries_by_type: vec![("tool_use".to_string(), 3)].into_iter().collect(),
            tool_calls_by_status: vec![("success".to_string(), 2), ("failed".to_string(), 1)]
                .into_iter()
                .collect(),
            last_token_usage: Some(TokenUsageSummary {
                total_tokens: 500,
                model_context_window: 128000,
            }),
        };

        let json = serde_json::to_string(&totals).unwrap();
        assert!(json.contains("tool_use"));
        assert!(json.contains("success"));
        assert!(json.contains("500"));
    }

    #[test]
    fn test_event_from_entry_tool_use() {
        let entry = NormalizedEntry {
            timestamp: Some("2026-08-11T17:33:20Z".to_string()),
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "bash".to_string(),
                action_type: ActionType::CommandRun {
                    command: "ls -la src/cli".to_string(),
                    result: None,
                    category: Default::default(),
                },
                status: ToolStatus::Success,
            },
            content: "listed files".to_string(),
            metadata: None,
        };

        let event = event_from_entry(3, &entry);
        assert_eq!(event.index, 3);
        assert_eq!(event.kind, "tool_use");
        assert_eq!(event.label, "ls -la src/cli");
        assert_eq!(event.status.as_deref(), Some("success"));
        assert_eq!(event.preview, "listed files");
    }

    #[test]
    fn test_trajectory_response_serialization() {
        let trajectory = TrajectoryResponse {
            session_id: uuid::Uuid::new_v4(),
            workspace_id: uuid::Uuid::new_v4(),
            session_name: Some("test session".to_string()),
            executor: Some("test_executor".to_string()),
            segments: vec![],
            completeness: TrajectoryCompleteness {
                total_processes: 0,
                with_logs: 0,
                dropped: 0,
                missing_logs: vec![],
            },
            totals: TrajectoryTotals::default(),
        };

        let json = serde_json::to_string(&trajectory).unwrap();
        assert!(json.contains("session_id"));
        assert!(json.contains("segments"));
        assert!(json.contains("completeness"));
        assert!(json.contains("totals"));
    }
}
