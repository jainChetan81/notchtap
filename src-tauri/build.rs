fn main() {
    // v5 (V5_TECHNICAL_SPEC.md §2): tauri allows app-defined commands to
    // EVERY window by default — this opt-in flips them to deny-by-default
    // so capabilities/settings.json can grant them to the settings window
    // alone, keeping the overlay (`main`) receive-only. never add a
    // #[tauri::command] to generate_handler without also listing it here.
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_config",
            "get_secret_status",
            "save_config_and_relaunch",
            "set_secret",
            "send_test_notification",
            "set_appearance",
        ]),
    ))
    .expect("failed to run tauri-build");
}
