// build.rs is a SEPARATE compilation from the crate — it runs before the
// crate exists as a build artifact, so it cannot `use` the crate's
// `settings_commands` module. It textually `include!`s that module's
// source file instead (at file/item scope — `include!`ing item
// declarations only works cleanly here, not spliced inside a function
// body), which brings `SETTINGS_COMMANDS` into scope for `fn main` below.
// See settings_commands.rs's own doc comment for the full rationale:
// single source of truth for the fourteen v5 settings-window commands,
// plus the parity tests there that check it against
// capabilities/settings.json and lib.rs's generate_handler![...].
include!("src/settings_commands.rs");

fn main() {
    // v5 (V5_TECHNICAL_SPEC.md §2): tauri allows app-defined commands to
    // EVERY window by default — this opt-in flips them to deny-by-default
    // so capabilities/settings.json can grant them to the settings window
    // alone, keeping the overlay (`main`) receive-only. never add a
    // #[tauri::command] to generate_handler without also listing it in
    // SETTINGS_COMMANDS above.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(SETTINGS_COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
