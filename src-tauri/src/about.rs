//! Data gathering for the settings window's About section
//! (`docs/V5_TECHNICAL_SPEC.md` §2, `get_about_info`). Everything
//! decision-shaped here is a pure or near-pure function (bundle-root
//! derivation, bundle size walk, `sw_vers` parsing) so it's unit-testable
//! without a live `tauri::AppHandle` — the command wrapper in
//! `settings.rs` (which owns [`crate::settings::ensure_settings_window`])
//! is the only part that needs one, and it stays a thin call into
//! [`gather_about_info`].

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;
use sysinfo::{Disks, Pid, ProcessesToUpdate, System};

/// Wire shape of `get_about_info` — camelCase to match the rest of the
/// settings IPC surface (`ConnectorHealthDto`'s own doc comment in
/// `settings.rs` calls this out as the established convention).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutInfo {
    pub version: String,
    pub bundle_id: String,
    /// `None` for a dev build (no `.app` ancestor in `current_exe()`) or
    /// if the walk itself fails (unreadable directory, race with a
    /// concurrent uninstall, ...) — best-effort, never fatal to the rest
    /// of the payload.
    pub bundle_size_bytes: Option<u64>,
    /// e.g. "macOS 14.5" — falls back to the bare "macOS" if `sw_vers`
    /// is unavailable or its output doesn't parse.
    pub platform: String,
    pub arch: String,
    pub process_memory_bytes: u64,
    pub system_memory_used_bytes: u64,
    pub system_memory_total_bytes: u64,
    /// `None` if no disk in the refreshed list mounts at `/` — shouldn't
    /// happen on a real macOS host, but the DTO stays honest rather than
    /// reporting a zeroed stat as if it were real.
    pub disk_used_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
    pub uptime_secs: u64,
}

/// Walks up from an executable path to the `.app` bundle that contains
/// it (`.../notchtap.app/Contents/MacOS/notchtap` -> `.../notchtap.app`).
/// `None` for a dev build — `target/debug/notchtap` (or `release`) has no
/// ancestor whose final path component ends in `.app`, which is exactly
/// the signal a bundled build vs. a bare `cargo build`/`cargo test`
/// binary gives us for free, no extra config needed.
pub fn app_bundle_root(exe_path: &Path) -> Option<PathBuf> {
    exe_path
        .ancestors()
        .find(|candidate| candidate.extension().is_some_and(|ext| ext == "app"))
        .map(Path::to_path_buf)
}

/// Best-effort recursive size of everything under `root`, in bytes.
/// Symlinks are skipped rather than followed (a bundle shouldn't contain
/// one pointing outside itself, but this avoids a cycle if it somehow
/// does); any read error along the way (permissions, a file vanishing
/// mid-walk) makes the whole call return `None` rather than an
/// under-counted partial size that would misleadingly look precise.
pub fn bundle_size_bytes(root: &Path) -> Option<u64> {
    fn walk(dir: &Path, total: &mut u64) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            } else if file_type.is_dir() {
                walk(&entry.path(), total)?;
            } else {
                *total += entry.metadata()?.len();
            }
        }
        Ok(())
    }

    let mut total = 0u64;
    walk(root, &mut total).ok()?;
    Some(total)
}

/// `sw_vers -productVersion` (e.g. "14.5") — same subprocess-shim shape
/// as `presentation.rs`'s `notchtap-detect` call (CLAUDE.md's rust-core
/// precedent for shelling out on macOS), but `sw_vers` ships with the OS
/// itself so there's no bundled-binary path to resolve. `None` on any
/// failure (missing binary, non-zero exit, empty/unparseable stdout) —
/// the caller falls back to a bare "macOS" label.
pub fn macos_product_version() -> Option<String> {
    let output = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

/// Assembles the full `AboutInfo` payload. `started_at` is the
/// `Instant` captured once at app boot (managed state, `lib.rs`'s
/// `.setup()`) — uptime is process uptime, not system uptime.
///
/// sysinfo refresh is targeted (plan spec: "process + memory, not
/// everything") — `System::new()` refreshes nothing on its own; this
/// explicitly refreshes only memory and the current process, and a
/// separate `Disks` list only for the root-mount stat, rather than
/// `System::new_all()`'s full CPU/network/every-process sweep.
pub fn gather_about_info<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    started_at: Instant,
) -> AboutInfo
where
    R: tauri::Runtime,
{
    let package_info = app.package_info();
    let version = package_info.version.to_string();
    let bundle_id = app.config().identifier.clone();

    let bundle_size_bytes = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(app_bundle_root)
        .as_deref()
        .and_then(bundle_size_bytes);

    let platform = macos_product_version()
        .map(|v| format!("macOS {v}"))
        .unwrap_or_else(|| "macOS".to_string());
    let arch = std::env::consts::ARCH.to_string();

    let pid = sysinfo::get_current_pid().ok();
    let mut sys = System::new();
    sys.refresh_memory();
    if let Some(pid) = pid {
        sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    }
    let process_memory_bytes = pid
        .and_then(|pid: Pid| sys.process(pid))
        .map(|p| p.memory())
        .unwrap_or(0);
    let system_memory_used_bytes = sys.used_memory();
    let system_memory_total_bytes = sys.total_memory();

    let disks = Disks::new_with_refreshed_list();
    let root_disk = disks
        .list()
        .iter()
        .find(|d| d.mount_point() == Path::new("/"));
    let disk_used_bytes = root_disk.map(|d| d.total_space().saturating_sub(d.available_space()));
    let disk_total_bytes = root_disk.map(|d| d.total_space());

    AboutInfo {
        version,
        bundle_id,
        bundle_size_bytes,
        platform,
        arch,
        process_memory_bytes,
        system_memory_used_bytes,
        system_memory_total_bytes,
        disk_used_bytes,
        disk_total_bytes,
        uptime_secs: started_at.elapsed().as_secs(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_bundle_root_finds_the_dot_app_ancestor() {
        let exe = Path::new("/Applications/notchtap.app/Contents/MacOS/notchtap");
        assert_eq!(
            app_bundle_root(exe),
            Some(PathBuf::from("/Applications/notchtap.app"))
        );
    }

    #[test]
    fn app_bundle_root_is_none_for_a_bare_dev_build_path() {
        let exe = Path::new("/Users/dev/mac-notification-nudge/target/debug/notchtap");
        assert_eq!(app_bundle_root(exe), None);
    }

    #[test]
    fn app_bundle_root_is_none_for_a_release_dev_build_path() {
        let exe = Path::new("/Users/dev/mac-notification-nudge/target/release/notchtap");
        assert_eq!(app_bundle_root(exe), None);
    }

    #[test]
    fn bundle_size_bytes_sums_nested_files() {
        let dir = std::env::temp_dir().join(format!("notchtap-about-test-{}", uuid::Uuid::new_v4()));
        let nested = dir.join("Contents/MacOS");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(dir.join("Contents/Info.plist"), b"12345").unwrap();
        std::fs::write(nested.join("notchtap"), b"1234567890").unwrap();

        let total = bundle_size_bytes(&dir);

        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(total, Some(15));
    }

    #[test]
    fn bundle_size_bytes_is_none_for_a_missing_root() {
        let missing = std::env::temp_dir().join(format!("notchtap-about-missing-{}", uuid::Uuid::new_v4()));
        assert_eq!(bundle_size_bytes(&missing), None);
    }
}
