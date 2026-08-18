// Single source of truth for the settings-window command triple
// (CLAUDE.md's "ipc & security" section): build.rs's
// `AppManifest::commands` allowlist, `lib.rs`'s `generate_handler!`
// registration, and `capabilities/settings.json`'s `allow-<kebab-name>`
// permission list must all name exactly these fifteen commands. NEVER
// add a `#[tauri::command]` to `generate_handler!` without adding it
// here too — the failure mode is FAIL-OPEN: a command missing from this
// list silently becomes callable from the overlay (`main`) window as
// well, breaking the receive-only guarantee that's the whole point of
// the split.
//
// `build.rs` is a SEPARATE compilation from this crate — it runs before
// the crate even exists as a build artifact, so it cannot `use` this
// module. It textually `include!`s this file into its own `fn main()`
// body instead, which is why:
// - this file is written with plain `//` comments throughout, never
// `//!`/`///` doc comments — those lex to `#[doc = "..."]` attribute
// tokens, and their legality is position-sensitive in a way that a
// spliced-into-a-function-body doc comment on a local const risks
// tripping over. Plain comments are inert whitespace in any context.
// - the `#[cfg(test)] mod tests` block below is dead weight from
// build.rs's point of view, not a problem: cargo never compiles
// build.rs with `--cfg test`, so the block is stripped before
// anything in it (including its `serde_json` use, which is a normal
// dependency but NOT a build-dependency) is ever checked.
//
// Snake_case, exactly as each function is named in `settings.rs` and
// listed in `lib.rs`'s `generate_handler!`. `capabilities/settings.json`
// encodes them as tauri's auto-derived `allow-<kebab-case>` permission
// strings — see `settings_json_permissions_match_exactly` below for that
// translation.
//
// `get_about_info`'s own data gathering (bundle path derivation, the
// recursive size walk, the `sw_vers` shell-out, sysinfo reads) lives in
// `about.rs`, not here or in `settings.rs` — this file only needs the
// command's snake_case name for the parity triple.
//
// `#[allow(dead_code)]`: this crate's own non-test compilation never
// reads SETTINGS_COMMANDS (only this file's #[cfg(test)] tests do) — the
// OTHER consumer, build.rs, reaches it via `include!` into a wholly
// separate compilation the crate's own dead-code analysis can't see.
// Same targeted-allow shape as `SlotState`'s `large_enum_variant` allow
// in event.rs and `Engine::new`'s `too_many_arguments` allow in
// engine.rs.
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

    // Pins the count/a couple of members so an accidental edit to the
    // array literal itself (typo, duplicate, stray removal) doesn't slip
    // by unnoticed alongside the two parity checks below.
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

    // Parity guard #1: capabilities/settings.json's FULL permissions
    // array must be exactly the command permissions derived from
    // SETTINGS_COMMANDS plus the two pinned event extras below, nothing
    // missing and nothing extra. Comparing the WHOLE set — never a subset
    // filtered on some prefix — is load-bearing: a namespaced plugin
    // grant like "shell:allow-execute" doesn't start with "allow-", so a
    // filtered comparison would let it widen the window's real capability
    // grant unaudited. Whole-set equality fails it loudly as "extra".
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
        // Not part of the command triple, but losing either of these
        // silently breaks the settings window in a different, quieter
        // way (no event delivery) — pinned here too since this test
        // already owns settings.json's full permission list.
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

    // Parity guard #2: lib.rs's `generate_handler![...]` must register
    // `settings::<name>` for exactly the names in SETTINGS_COMMANDS.
    // `tauri::generate_handler!` is a proc macro fed literal idents at
    // compile time — it cannot consume a runtime `&[&str]` — so this is a
    // source-parse check on lib.rs's own text, the documented fallback
    // for that exact constraint.
    #[test]
    fn generate_handler_registers_exactly_the_canonical_commands() {
        let lib_src = include_str!("lib.rs");
        let marker = "tauri::generate_handler![";

        // this test's own `.find(marker)` below only ever
        // parses the FIRST occurrence — a second, accidental
        // `generate_handler![...]` invocation (e.g. a copy-pasted
        // registration for a future window) could register an
        // unauthorized command in a SECOND block this test would never
        // even look at, passing clean. Pinning the occurrence count to
        // exactly one closes that blind spot before the parse below ever
        // runs.
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
