//! Data gathering for the settings window's About section (`get_about_info`).

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;
use sysinfo::{Disks, Pid, ProcessesToUpdate, System};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutInfo {
    pub version: String,
    pub bundle_id: String,
    /// `None` for a dev build (no `.app` ancestor in `current_exe()`) or if the walk itself fails
    /// (unreadable directory, race with a concurrent uninstall, ...) — best-effort.
    pub bundle_size_bytes: Option<u64>,
    pub platform: String,
    pub arch: String,
    pub process_memory_bytes: u64,
    pub system_memory_used_bytes: u64,
    pub system_memory_total_bytes: u64,
    pub disk_used_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
    pub uptime_secs: u64,
}

/// Walks up from an executable path to the `.app` bundle that contains it
/// (`.../notchtap.app/Contents/MacOS/notchtap` -> `.../notchtap.app`).
pub fn app_bundle_root(exe_path: &Path) -> Option<PathBuf> {
    exe_path
        .ancestors()
        .find(|candidate| candidate.extension().is_some_and(|ext| ext == "app"))
        .map(Path::to_path_buf)
}

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

pub fn macos_product_version() -> Option<String> {
    // absolute path, not a bare `sw_vers` looked up on `$PATH` — this process's `PATH` isn't
    // attacker-controlled in the way a setuid binary's would be.
    let output = std::process::Command::new("/usr/bin/sw_vers")
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

/// Assembles the full `AboutInfo` payload.
pub fn gather_about_info<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    started_at: Instant,
) -> AboutInfo {
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
    fn bundle_size_bytes_sums_nested_files() {
        let dir =
            std::env::temp_dir().join(format!("notchtap-about-test-{}", uuid::Uuid::new_v4()));
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
        let missing =
            std::env::temp_dir().join(format!("notchtap-about-missing-{}", uuid::Uuid::new_v4()));
        assert_eq!(bundle_size_bytes(&missing), None);
    }
}
