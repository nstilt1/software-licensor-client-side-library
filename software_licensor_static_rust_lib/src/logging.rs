// logging.rs
//
// Conditional compilation: this entire module's macro surface is only active
// when the `logging` feature flag is enabled.  The macros themselves are
// defined unconditionally so that call-sites always compile; when the feature
// is absent they expand to nothing (zero cost, no symbols emitted).

#[cfg(feature = "logging")]
pub mod inner {
    use std::fs::{self, OpenOptions};
    use std::io::{BufRead, BufReader, Write};
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    use once_cell::sync::Lazy;

    // ── constants ────────────────────────────────────────────────────────────

    const LOG_FILE_NAME: &str = "plugin.log";
    /// Maximum number of lines kept in the log file.
    const MAX_LOG_LINES: usize = 500;
    /// Lines kept after a rotation (we keep the newest half).
    const KEEP_LINES_AFTER_ROTATE: usize = MAX_LOG_LINES / 2;

    // ── global state ─────────────────────────────────────────────────────────

    /// Resolved path to the log file, set once by `log_init!`.
    pub(crate) static LOG_FILE_PATH: Lazy<Mutex<Option<PathBuf>>> =
        Lazy::new(|| Mutex::new(None));

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Returns an ISO-8601-like timestamp string: `2024-04-01 12:34:56.789`.
    fn timestamp() -> String {
        // We deliberately avoid `chrono` to keep the dependency surface small.
        // SystemTime gives us millisecond precision; we format it manually.
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let secs = now.as_secs();
        let millis = now.subsec_millis();

        let s = secs % 60;
        let m = (secs / 60) % 60;
        let h = (secs / 3600) % 24;
        // days since epoch → calendar date (Gregorian proleptic)
        let days = secs / 86_400;
        let (year, month, day) = days_to_ymd(days);

        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
            year, month, day, h, m, s, millis
        )
    }

    /// Converts days-since-Unix-epoch to `(year, month, day)`.
    fn days_to_ymd(days: u64) -> (u64, u64, u64) {
        // Algorithm: http://howardhinnant.github.io/date_algorithms.html
        let z = days + 719_468;
        let era = z / 146_097;
        let doe = z % 146_097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        (y, m, d)
    }

    /// Rotate the log file if it exceeds `MAX_LOG_LINES`.
    /// Keeps the newest `KEEP_LINES_AFTER_ROTATE` lines.
    fn rotate_if_needed(path: &PathBuf) {
        let file = match fs::File::open(path) {
            Ok(f) => f,
            Err(_) => return, // file doesn't exist yet – nothing to rotate
        };

        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();

        if lines.len() < MAX_LOG_LINES {
            return;
        }

        // Keep only the tail.
        let start = lines.len().saturating_sub(KEEP_LINES_AFTER_ROTATE);
        let trimmed = lines[start..].join("\n");

        // Overwrite the file with the trimmed content.
        if let Ok(mut f) = fs::File::create(path) {
            let _ = writeln!(f, "{}", trimmed);
        }
    }

    /// Core write function.  Exposed only to this module's macros.
    pub(crate) fn write_log(level: &str, message: &str) {
        let guard = match LOG_FILE_PATH.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let path = match &*guard {
            Some(p) => p.clone(),
            None => return, // not initialised
        };
        drop(guard); // release lock before doing I/O

        // Ensure the parent directory exists.
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }

        rotate_if_needed(&path);

        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(file, "[{}] [{}] {}", timestamp(), level, message);
        }
    }

    /// Initialises the logger by resolving the log-file path via
    /// `crate::file_io::get_log_file_path()` and storing it globally.
    /// Returns `Ok(PathBuf)` on success.
    pub(crate) fn init_logger() -> Result<PathBuf, crate::Error> {
        let log_dir = crate::file_io::get_log_file_path()?;
        let log_file = log_dir.join(LOG_FILE_NAME);

        let mut guard = LOG_FILE_PATH
            .lock()
            .map_err(|_| crate::Error::LockPoisoned)?;
        *guard = Some(log_file.clone());

        Ok(log_file)
    }
}

// ── public macros ─────────────────────────────────────────────────────────────
//
// Each macro is defined in two variants:
//   • When `logging` feature is ON  → format the message and call the writer.
//   • When `logging` feature is OFF → expand to nothing (no-op).

/// Initialise the logger.  Must be called before `log_error!` / `log_info!`.
///
/// # Example
/// ```ignore
/// log_init!("/path/to/plugin/data");
/// ```
#[macro_export]
#[cfg(feature = "logging")]
macro_rules! log_init {
    ($initialized_at_str:expr) => {{
        match $crate::logging::inner::init_logger() {
            Ok(_path) => {
                $crate::log_info!("logs initialized at {}", $initialized_at_str);
            }
            Err(e) => {
                // Can't use log_info here because the logger may not be ready.
                // Fall through silently – a JUCE plug-in must never panic.
                let _ = e;
            }
        }
    }};
}

#[macro_export]
#[cfg(not(feature = "logging"))]
macro_rules! log_init {
    ($initialized_at_str:expr) => {{}};
}

/// Log an informational message.
///
/// Accepts the same format string syntax as `println!`.
///
/// # Examples
/// ```ignore
/// log_info!("plugin loaded");
/// log_info!("sample rate: {}", sample_rate);
/// ```
#[macro_export]
#[cfg(feature = "logging")]
macro_rules! log_info {
    ($($arg:tt)*) => {{
        $crate::logging::inner::write_log("INFO", &::std::format!($($arg)*));
    }};
}

#[macro_export]
#[cfg(not(feature = "logging"))]
macro_rules! log_info {
    ($($arg:tt)*) => {{}};
}

/// Log an error message.
///
/// Accepts the same format string syntax as `println!`.
///
/// # Examples
/// ```ignore
/// log_error!("failed to open file");
/// log_error!("unexpected value: {:?}", val);
/// ```
#[macro_export]
#[cfg(feature = "logging")]
macro_rules! log_error {
    ($($arg:tt)*) => {{
        $crate::logging::inner::write_log("ERROR", &::std::format!($($arg)*));
    }};
}

#[macro_export]
#[cfg(not(feature = "logging"))]
macro_rules! log_error {
    ($($arg:tt)*) => {{}};
}