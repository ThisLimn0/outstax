//! Minimal `log` backend for the CLI (stderr) and desktop (rotating file); std only.
//! Line format: `2026-11-12T10:15:30.123Z INFO  outstax_client::session: message`.
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// `OUTSTAX_LOG=info|debug|trace` (or the old `INSTAX_LOG`) raises `default`; it never lowers it.
pub fn level(default: LevelFilter) -> LevelFilter {
    std::env::var("OUTSTAX_LOG")
        .or_else(|_| std::env::var("INSTAX_LOG"))
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .map_or(default, |env: LevelFilter| env.max(default))
}

/// Install the global logger. `file`: append there (rotating at `ROTATE_BYTES`), else stderr.
pub fn init(level: LevelFilter, file: Option<&Path>) -> io::Result<()> {
    let file = file.map(RotatingFile::open).transpose()?;
    let logger = Logger {
        level,
        file: file.map(Mutex::new),
    };
    log::set_logger(Box::leak(Box::new(logger))).map_err(|e| io::Error::other(e.to_string()))?;
    log::set_max_level(level);
    Ok(())
}

struct Logger {
    level: LevelFilter,
    file: Option<Mutex<RotatingFile>>,
}
impl Log for Logger {
    fn enabled(&self, m: &Metadata) -> bool {
        let target = m.target();
        // Our crates at the chosen level; btleplug joins at debug/trace; everything
        // else (eframe, winit, …) stays at warn so debug logs remain readable.
        let limit = if target.starts_with("outstax")
            || (self.level >= LevelFilter::Debug && target.starts_with("btleplug"))
        {
            self.level
        } else {
            LevelFilter::Warn
        };
        m.level() <= limit
    }
    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format_line(
            SystemTime::now(),
            record.level(),
            record.target(),
            record.args(),
        );
        match &self.file {
            Some(file) => {
                if let Ok(mut file) = file.lock() {
                    let _ = file.write_line(&line);
                }
            }
            None => eprint!("{line}"),
        }
    }
    fn flush(&self) {}
}

pub fn format_line(
    time: SystemTime,
    level: Level,
    target: &str,
    message: &std::fmt::Arguments,
) -> String {
    format!("{} {level:<5} {target}: {message}\n", utc_timestamp(time))
}

/// `YYYY-MM-DDTHH:MM:SS.mmmZ`.
pub fn utc_timestamp(time: SystemTime) -> String {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let secs = since.as_secs();
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let s = secs % 86_400;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z",
        s / 3600,
        s / 60 % 60,
        s % 60,
        since.subsec_millis()
    )
}

/// Howard Hinnant's civil_from_days: days since 1970-01-01 to (year, month, day).
pub(crate) fn civil_from_days(days: i64) -> (i64, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (
        yoe + era * 400 + i64::from(month <= 2),
        month as u8,
        day as u8,
    )
}

pub const ROTATE_BYTES: u64 = 5 << 20;

/// Append-only log file; past `limit` bytes it becomes `<name>.1` (one old file kept).
pub struct RotatingFile {
    path: PathBuf,
    file: File,
    size: u64,
    limit: u64,
}
impl RotatingFile {
    pub fn open(path: &Path) -> io::Result<Self> {
        Self::with_limit(path, ROTATE_BYTES)
    }
    pub fn with_limit(path: &Path, limit: u64) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let size = file.metadata()?.len();
        Ok(Self {
            path: path.to_owned(),
            file,
            size,
            limit,
        })
    }
    pub fn write_line(&mut self, line: &str) -> io::Result<()> {
        if self.size > 0 && self.size + line.len() as u64 > self.limit {
            let mut old = OsString::from(&self.path);
            old.push(".1");
            // std opens files with FILE_SHARE_DELETE, so the open handle may be renamed.
            std::fs::rename(&self.path, old)?;
            self.file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?;
            self.size = 0;
        }
        self.file.write_all(line.as_bytes())?;
        self.size += line.len() as u64;
        Ok(())
    }
}

/// Keep the newest `keep` files with `extension` in `dir` (names sort by time), delete the rest.
pub fn prune(dir: &Path, extension: &str, keep: usize) -> io::Result<()> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| p.extension().is_some_and(|e| e == extension))
        .collect();
    files.sort();
    let excess = files.len().saturating_sub(keep);
    for path in &files[..excess] {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("outstax-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn timestamp() {
        let t = UNIX_EPOCH + Duration::from_millis(1_709_251_199_123);
        assert_eq!(utc_timestamp(t), "2024-02-29T23:59:59.123Z");
        assert_eq!(utc_timestamp(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        let line = format_line(UNIX_EPOCH, Level::Warn, "outstax", &format_args!("hi"));
        assert_eq!(line, "1970-01-01T00:00:00.000Z WARN  outstax: hi\n");
    }

    #[test]
    fn rotation_keeps_one_old_file() {
        let dir = scratch("rotate");
        let path = dir.join("app.log");
        let mut log = RotatingFile::with_limit(&path, 25).unwrap();
        for line in [
            "first line 0123456\n",
            "second line 012345\n",
            "third line 0123456\n",
        ] {
            log.write_line(line).unwrap();
        }
        drop(log);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "third line 0123456\n"
        );
        let old = std::fs::read_to_string(dir.join("app.log.1")).unwrap();
        assert_eq!(old, "second line 012345\n");
        // Reopening continues from the existing size.
        let mut log = RotatingFile::with_limit(&path, 25).unwrap();
        log.write_line("x\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "third line 0123456\nx\n"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn prune_keeps_newest() {
        let dir = scratch("prune");
        for i in 0..25 {
            std::fs::write(
                dir.join(format!("2026-11-12T1000{i:02}.000Z-test.jsonl")),
                "",
            )
            .unwrap();
        }
        std::fs::write(dir.join("notes.txt"), "").unwrap();
        prune(&dir, "jsonl", 20).unwrap();
        let mut left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left.len(), 21);
        assert_eq!(left[0], "2026-11-12T100005.000Z-test.jsonl");
        assert_eq!(left[20], "notes.txt");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
