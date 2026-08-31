// NEVER add a `#[tauri::command]` to `generate_handler!` without adding it here too.
#[allow(dead_code)]
pub(crate) const SETTINGS_COMMANDS: &[&str] = &[
    "clear_history",
    "clear_queue",
    "get_about_info",
    "get_agent_health",
    "get_config",
    "get_default_config",
    "get_history",
    "get_queue",
    "get_recent_log_lines",
    "save_config_and_relaunch",
    "search_news_now",
    "send_agent_test_event",
    "send_test_notification",
    "set_appearance",
    "skip_current",
];

#[cfg(test)]
mod tests {
    use super::SETTINGS_COMMANDS;
    use std::collections::BTreeSet;

    #[test]
    fn canonical_list_has_the_documented_fifteen_commands() {
        assert_eq!(SETTINGS_COMMANDS.len(), 15);
        assert!(SETTINGS_COMMANDS.contains(&"get_history"));
        assert!(SETTINGS_COMMANDS.contains(&"clear_history"));
        assert!(SETTINGS_COMMANDS.contains(&"get_queue"));
        assert!(SETTINGS_COMMANDS.contains(&"clear_queue"));
        assert!(SETTINGS_COMMANDS.contains(&"skip_current"));
        assert!(SETTINGS_COMMANDS.contains(&"search_news_now"));
        assert!(SETTINGS_COMMANDS.contains(&"get_about_info"));
        assert!(SETTINGS_COMMANDS.contains(&"get_agent_health"));
        assert!(SETTINGS_COMMANDS.contains(&"send_agent_test_event"));
    }

    #[test]
    fn settings_json_permissions_match_exactly() {
        let raw = include_str!("../capabilities/settings.json");
        let doc: serde_json::Value =
            serde_json::from_str(raw).expect("capabilities/settings.json must parse as JSON");
        let permissions: BTreeSet<String> = doc["permissions"]
            .as_array()
            .expect("capabilities/settings.json must have a top-level \"permissions\" array")
            .iter()
            .map(|v| {
                v.as_str()
                    .expect("every permission entry must be a string")
                    .to_string()
            })
            .collect();

        let mut expected: BTreeSet<String> = SETTINGS_COMMANDS
            .iter()
            .map(|name| format!("allow-{}", name.replace('_', "-")))
            .collect();
        expected.insert("core:event:allow-listen".to_string());
        expected.insert("core:event:allow-unlisten".to_string());

        assert_eq!(
            permissions, expected,
            "capabilities/settings.json's full permissions array has drifted from \
             SETTINGS_COMMANDS (src/settings_commands.rs) plus the pinned core:event extras — \
             every entry must be either allow-<kebab-case> for a command in that canonical \
             list, or one of the two event-channel extras, and nothing else (a namespaced \
             plugin grant like shell:allow-execute must fail here)"
        );
    }

    #[test]
    fn generate_handler_registers_exactly_the_canonical_commands() {
        let lib_src = include_str!("lib.rs");
        let marker = "tauri::generate_handler![";

        assert_eq!(
            lib_src.matches(marker).count(),
            1,
            "lib.rs must contain exactly one tauri::generate_handler![...] invocation — a \
             second occurrence would register commands this parity test never inspects"
        );

        let start = lib_src
            .find(marker)
            .expect("lib.rs must have a tauri::generate_handler![...] invocation");
        let after_open = start + marker.len();
        let end = lib_src[after_open..]
            .find(']')
            .map(|i| i + after_open)
            .expect("generate_handler![...] must close with a ]");
        let body = &lib_src[after_open..end];

        let registered: BTreeSet<&str> = body
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.strip_prefix("settings::").unwrap_or_else(|| {
                    panic!(
                        "generate_handler![...] entry {s:?} is not a settings:: command — \
                         every entry in this block is expected to be one of the fifteen \
                         settings commands"
                    )
                })
            })
            .collect();

        let expected: BTreeSet<&str> = SETTINGS_COMMANDS.iter().copied().collect();

        assert_eq!(
            registered, expected,
            "lib.rs's generate_handler![...] has drifted from SETTINGS_COMMANDS \
             (src/settings_commands.rs) — add/remove the command in both places"
        );
    }
}
