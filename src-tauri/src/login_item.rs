use smappservice_rs::{AppService, ServiceType};

/// Registers the app as a login item via SMAppService (macOS 13+).
///
/// Only meaningful when running as a bundled .app — `tauri dev` runs an
/// unbundled binary, where registration is skipped with a log line
/// instead of erroring (spec §6).
pub fn register() {
    if !running_as_bundle() {
        tracing::info!("login item registration skipped (not running as a bundled .app)");
        return;
    }

    let service = AppService::new(ServiceType::MainApp);
    match service.register() {
        Ok(()) => tracing::info!("registered as login item via SMAppService"),
        Err(e) => tracing::warn!("login item registration failed: {e}"),
    }
}

fn running_as_bundle() -> bool {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false)
}
