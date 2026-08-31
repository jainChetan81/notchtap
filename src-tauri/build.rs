// See settings_commands.rs's own doc comment for the full rationale: single source of truth for
// the fifteen settings-window commands.
include!("src/settings_commands.rs");

fn main() {
    // tauri allows app-defined commands to EVERY window by default — this opt-in flips them to
    // deny-by-default so capabilities/settings.json can grant them to the settings window alone.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(SETTINGS_COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
