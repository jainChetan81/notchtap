use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

pub fn init_logging() -> anyhow::Result<WorkerGuard> {
    let log_dir = log_dir()?;
    let appender = SizeRotatingAppender::new(&log_dir, "notchtap.log", 10 * 1024 * 1024, 3)?;
    let (non_blocking, guard) = tracing_appender::non_blocking(appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_filter(log_filter());

    tracing_subscriber::registry()
        .with(file_layer)
        .with(tracing_subscriber::fmt::layer().with_filter(log_filter()))
        .init();

    Ok(guard)
}

fn log_dir() -> anyhow::Result<PathBuf> {
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;
    let dir = home.join("Library").join("Logs").join("notchtap");
    fs::create_dir_all(&dir)?;
    // notchtap.log can carry sensitive notification content (titles/
    // bodies relayed through `/notify`) — same posture as `history.rs`'s
    // `history.jsonl` (0700 dir / 0600 file, see `SizeRotatingAppender`
    // below) rather than trusting the umask-derived default (0755).
    #[cfg(unix)]
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    Ok(dir)
}

/// Read the last `n` lines of the active log file (`{log_dir}/notchtap.log`;
/// rotated backups stay out of scope, plan 077). Full-file read plus a
/// tail-slice — the 10MB rotation cap already bounds the worst-case file
/// size, so a seek-from-end tail reader would be complexity without payoff
/// at this size. A file that doesn't exist yet (fresh install, nothing
/// logged) reads as an empty Vec, not an error.
pub fn read_recent_lines(n: usize) -> anyhow::Result<Vec<String>> {
    read_recent_lines_from(&log_dir()?.join("notchtap.log"), n)
}

fn read_recent_lines_from(path: &Path, n: usize) -> anyhow::Result<Vec<String>> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let lines: Vec<String> = contents.lines().map(str::to_string).collect();
    Ok(lines[lines.len().saturating_sub(n)..].to_vec())
}

fn log_filter() -> EnvFilter {
    if cfg!(debug_assertions) {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    }
}

struct SizeRotatingAppender {
    inner: Mutex<Inner>,
}

struct Inner {
    dir: PathBuf,
    filename: String,
    max_size: u64,
    max_files: usize,
    file: File,
    size: u64,
}

impl SizeRotatingAppender {
    fn new(
        dir: impl AsRef<Path>,
        filename: impl AsRef<str>,
        max_size: u64,
        max_files: usize,
    ) -> io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;
        // matches `log_dir`'s own 0700 (this constructor is also reached
        // directly by this module's tests, with their own temp dirs, not
        // just via `log_dir`'s call path).
        #[cfg(unix)]
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        let filename = filename.as_ref().to_string();
        let path = dir.join(&filename);
        let mut open_options = OpenOptions::new();
        open_options.create(true).append(true);
        #[cfg(unix)]
        open_options.mode(0o600);
        let file = open_options.open(&path)?;
        // `.mode()` on `OpenOptions` only governs the permissions a *new*
        // file is created with — a no-op against a file that already
        // existed (e.g. one written before this hardening landed, umask
        // 0644). Force 0600 unconditionally so a pre-existing permissive
        // file gets fixed rather than staying world-readable forever
        // (same reasoning as `history.rs`'s `HistoryStore::append`).
        #[cfg(unix)]
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        let size = file.metadata()?.len();
        Ok(Self {
            inner: Mutex::new(Inner {
                dir,
                filename,
                max_size,
                max_files,
                file,
                size,
            }),
        })
    }

    fn rotate_if_needed(&self, buf_len: usize) -> io::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if inner.size + buf_len as u64 > inner.max_size && inner.size > 0 {
            Self::rotate_locked(&mut inner)?;
        }
        Ok(())
    }

    fn rotate_locked(inner: &mut Inner) -> io::Result<()> {
        for i in (1..inner.max_files).rev() {
            let src = inner.dir.join(format!("{}.{}", inner.filename, i));
            let dst = inner.dir.join(format!("{}.{}", inner.filename, i + 1));
            if src.exists() {
                fs::rename(&src, &dst)?;
            }
        }

        let current = inner.dir.join(&inner.filename);
        let backup = inner.dir.join(format!("{}.{}", inner.filename, 1));
        fs::rename(&current, &backup)?;

        let mut open_options = OpenOptions::new();
        open_options.create(true).append(true);
        #[cfg(unix)]
        open_options.mode(0o600);
        inner.file = open_options.open(&current)?;
        #[cfg(unix)]
        inner
            .file
            .set_permissions(fs::Permissions::from_mode(0o600))?;
        inner.size = 0;
        Ok(())
    }
}

impl Write for SizeRotatingAppender {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.rotate_if_needed(buf.len())?;
        let mut inner = self.inner.lock().unwrap();
        let written = inner.file.write(buf)?;
        inner.size += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.lock().unwrap().file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    // a fresh, unique dir per test is mandatory, not hygiene: `new()`
    // seeds `size` from any pre-existing file's length, which would
    // silently shift the threshold arithmetic.
    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("notchtap-logtest-{}", Uuid::new_v4()))
    }

    // --- L-sec2: log dir/file must not be world-readable ---

    #[cfg(unix)]
    #[test]
    fn new_log_dir_and_file_are_0700_and_0600() {
        let dir = temp_dir();
        let app = SizeRotatingAppender::new(&dir, "notchtap.log", 100, 3).unwrap();
        drop(app);

        let dir_mode = fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700, "log dir must not be world-readable");

        let file_mode = fs::metadata(dir.join("notchtap.log"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600, "log file must not be world-readable");
    }

    #[cfg(unix)]
    #[test]
    fn preexisting_permissive_log_file_is_fixed_to_0600_on_open() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notchtap.log");
        // simulate a file written before this hardening landed (umask 0644)
        fs::write(&path, "").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        let _app = SizeRotatingAppender::new(&dir, "notchtap.log", 100, 3).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn rotated_log_file_is_still_0600() {
        let dir = temp_dir();
        let mut app = SizeRotatingAppender::new(&dir, "notchtap.log", 100, 3).unwrap();

        app.write_all(&[b'a'; 60]).unwrap();
        app.write_all(&[b'b'; 60]).unwrap(); // triggers a rotation
        app.flush().unwrap();

        let mode = fs::metadata(dir.join("notchtap.log"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "the post-rotation file must still be 0600");
    }

    #[test]
    fn no_rotation_below_threshold() {
        let dir = temp_dir();
        let mut app = SizeRotatingAppender::new(&dir, "notchtap.log", 100, 3).unwrap();

        app.write_all(&[b'a'; 50]).unwrap();
        app.write_all(&[b'b'; 50]).unwrap();
        app.flush().unwrap();

        // 100 total bytes is not `> 100` — no rotation, single file.
        assert_eq!(fs::read(dir.join("notchtap.log")).unwrap().len(), 100);
        assert!(!dir.join("notchtap.log.1").exists());
    }

    #[test]
    fn rotation_at_threshold_creates_backup_and_resets() {
        let dir = temp_dir();
        let mut app = SizeRotatingAppender::new(&dir, "notchtap.log", 100, 3).unwrap();

        app.write_all(&[b'a'; 60]).unwrap();
        // 60 + 60 > 100 with size 60 > 0: rotation happens before this
        // write, so the live file restarts with only the second write.
        app.write_all(&[b'b'; 60]).unwrap();
        app.flush().unwrap();

        assert_eq!(fs::read(dir.join("notchtap.log")).unwrap(), vec![b'b'; 60]);
        assert_eq!(
            fs::read(dir.join("notchtap.log.1")).unwrap(),
            vec![b'a'; 60]
        );
        assert!(!dir.join("notchtap.log.2").exists());
    }

    #[test]
    fn cascade_caps_at_max_files() {
        let dir = temp_dir();
        let mut app = SizeRotatingAppender::new(&dir, "notchtap.log", 100, 3).unwrap();

        // five 60-byte writes → four rotations.
        for fill in *b"12345" {
            app.write_all(&[fill; 60]).unwrap();
        }
        app.flush().unwrap();

        // rotate_locked's loop (i = 2, then 1) only ever renames up to
        // .3, so retention is current + exactly 3 backups: the oldest
        // ('1') is overwritten by the rename onto .3 and no .4 exists.
        assert_eq!(fs::read(dir.join("notchtap.log")).unwrap(), vec![b'5'; 60]);
        assert_eq!(
            fs::read(dir.join("notchtap.log.1")).unwrap(),
            vec![b'4'; 60]
        );
        assert_eq!(
            fs::read(dir.join("notchtap.log.2")).unwrap(),
            vec![b'3'; 60]
        );
        assert_eq!(
            fs::read(dir.join("notchtap.log.3")).unwrap(),
            vec![b'2'; 60]
        );
        assert!(!dir.join("notchtap.log.4").exists());
    }

    #[test]
    fn empty_current_file_never_rotates() {
        let dir = temp_dir();
        let mut app = SizeRotatingAppender::new(&dir, "notchtap.log", 100, 3).unwrap();

        // size is 0 going in, so the `inner.size > 0` guard skips
        // rotation even though this single write exceeds max_size — the
        // oversized line lands whole in the current file.
        app.write_all(&[b'x'; 150]).unwrap();
        app.flush().unwrap();

        assert_eq!(fs::read(dir.join("notchtap.log")).unwrap().len(), 150);
        assert!(!dir.join("notchtap.log.1").exists());
    }

    #[test]
    fn read_recent_lines_empty_file_returns_empty_vec() {
        let dir = temp_dir();
        let path = dir.join("notchtap.log");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, "").unwrap();

        assert_eq!(
            read_recent_lines_from(&path, 200).unwrap(),
            Vec::<String>::new()
        );
        // a missing file (fresh install, nothing logged yet) reads the
        // same way — empty, not an error.
        fs::remove_file(&path).unwrap();
        assert_eq!(
            read_recent_lines_from(&path, 200).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn read_recent_lines_fewer_lines_than_n_returns_all() {
        let dir = temp_dir();
        let path = dir.join("notchtap.log");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, "one\ntwo\nthree\n").unwrap();

        assert_eq!(
            read_recent_lines_from(&path, 200).unwrap(),
            vec!["one", "two", "three"]
        );
    }

    #[test]
    fn read_recent_lines_more_lines_than_n_returns_only_last_n() {
        let dir = temp_dir();
        let path = dir.join("notchtap.log");
        fs::create_dir_all(&dir).unwrap();
        let all: Vec<String> = (1..=10).map(|i| format!("line {i}")).collect();
        fs::write(&path, all.join("\n")).unwrap();

        assert_eq!(
            read_recent_lines_from(&path, 3).unwrap(),
            vec!["line 8", "line 9", "line 10"]
        );
    }
}
