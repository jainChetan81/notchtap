use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

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
    Ok(dir)
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
        let filename = filename.as_ref().to_string();
        let path = dir.join(&filename);
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
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

        inner.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&current)?;
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
