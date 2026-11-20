//! Diagnostic logging, global panic hooks, and Root Cause Analysis (RCA) reporting.
//!
//! Provides infrastructure for capturing crash dumps with backtraces,
//! structured startup error boxes with actionable remediation, and dual-sink tracing subscribers.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Delimiter line for console and diagnostic log formatting.
pub const BOX_DELIMITER: &str =
    "================================================================================";

/// Delimiter divider line.
pub const BOX_DIVIDER: &str =
    "--------------------------------------------------------------------------------";

/// Formats a panic hook event into a human-readable diagnostic report with backtrace.
pub fn format_panic_report(info: &std::panic::PanicHookInfo<'_>) -> String {
    let timestamp = chrono::Utc::now().to_rfc3339();

    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("unnamed");

    let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
        *s
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.as_str()
    } else {
        "Unknown panic payload (Box<dyn Any>)"
    };

    let location = if let Some(loc) = info.location() {
        format!("{}:{}:{}", loc.file(), loc.line(), loc.column())
    } else {
        "unknown location".to_string()
    };

    let mut backtrace = std::backtrace::Backtrace::capture();
    if backtrace.status() == std::backtrace::BacktraceStatus::Disabled {
        backtrace = std::backtrace::Backtrace::force_capture();
    }

    format!(
        "{BOX_DELIMITER}\n\
         [CRASH REPORT] Unhandled Panic\n\
         {BOX_DELIMITER}\n\
         Timestamp: {timestamp}\n\
         Thread:    {thread_name}\n\
         Location:  {location}\n\
         Payload:   {payload}\n\
         {BOX_DIVIDER}\n\
         Backtrace:\n\
         {backtrace}\n\
         {BOX_DELIMITER}\n\n"
    )
}

/// Installs a global panic hook that captures panic details and backtrace,
/// printing them to `stderr` and appending a structured dump to `<log_dir>/crash.log`.
pub fn setup_panic_hook(log_dir: &str) {
    let log_dir_path = PathBuf::from(log_dir);
    let _ = std::fs::create_dir_all(&log_dir_path);

    std::panic::set_hook(Box::new(move |info| {
        let report = format_panic_report(info);

        eprint!("{report}");

        let _ = std::fs::create_dir_all(&log_dir_path);
        let crash_file = log_dir_path.join("crash.log");
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(crash_file)
        {
            let _ = file.write_all(report.as_bytes());
            let _ = file.flush();
        }
    }));
}

/// Formats and records a structured Root Cause Analysis (RCA) and actionable remediation box
/// to `stderr` and `<log_dir>/startup_error.log`.
pub fn log_startup_error(log_dir: &str, title: &str, cause: &str, rca: &str, remediation: &str) {
    let timestamp = chrono::Utc::now().to_rfc3339();
    let report = format!(
        "{BOX_DELIMITER}\n\
         [FATAL STARTUP ERROR] {title}\n\
         {BOX_DELIMITER}\n\
         Timestamp: {timestamp}\n\
         Cause:     {cause}\n\n\
         [ROOT CAUSE ANALYSIS (RCA)]\n\
         {rca}\n\n\
         [ACTIONABLE REMEDIATION]\n\
         {remediation}\n\
         {BOX_DELIMITER}\n\n"
    );

    eprint!("{report}");

    let log_dir_path = Path::new(log_dir);
    let _ = std::fs::create_dir_all(log_dir_path);
    let error_file = log_dir_path.join("startup_error.log");
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(error_file)
    {
        let _ = file.write_all(report.as_bytes());
        let _ = file.flush();
    }
}

/// Pure helper evaluating whether ANSI escapes should be enabled based on TTY status and `NO_COLOR` value.
pub fn is_ansi_enabled(is_terminal: bool, no_color_val: Option<&str>) -> bool {
    let no_color_active = match no_color_val {
        Some(v) => !v.trim().is_empty(),
        None => false,
    };
    is_terminal && !no_color_active
}

/// Determines whether ANSI escape sequences should be enabled for stdout logging.
/// Returns true only when stdout is connected to an interactive terminal (`std::io::stdout().is_terminal()`)
/// and the `NO_COLOR` environment variable is unset or empty.
pub fn should_enable_ansi() -> bool {
    use std::io::IsTerminal;
    let is_terminal = std::io::stdout().is_terminal();
    let no_color = std::env::var("NO_COLOR").ok();
    is_ansi_enabled(is_terminal, no_color.as_deref())
}

/// Configures a dual-sink non-blocking tracing subscriber logging to console (stdout)
/// and daily rolling file `<log_dir>/money-flow.log.<date>`.
///
/// Returns a `WorkerGuard` that must be held in scope to ensure log flushing on termination.
pub fn init_dual_sink_subscriber(
    log_dir: &str,
    log_level: &str,
    log_format: crate::config::LogFormat,
) -> tracing_appender::non_blocking::WorkerGuard {
    let _ = std::fs::create_dir_all(log_dir);
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));

    let enable_ansi = should_enable_ansi();

    let file_appender = tracing_appender::rolling::daily(log_dir, "money-flow.log");
    let (non_blocking_appender, guard) = tracing_appender::non_blocking(file_appender);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking_appender)
        .with_ansi(false);

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer);

    match log_format {
        crate::config::LogFormat::Pretty => {
            let stdout_layer = tracing_subscriber::fmt::layer()
                .pretty()
                .with_ansi(enable_ansi);

            let _ = subscriber.with(stdout_layer).try_init();
        }
        crate::config::LogFormat::Compact => {
            let stdout_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_ansi(enable_ansi);

            let _ = subscriber.with(stdout_layer).try_init();
        }
    }

    guard
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_startup_error_persistence_and_formatting() {
        let temp_dir =
            std::env::temp_dir().join(format!("mf_startup_test_{}", uuid::Uuid::new_v4()));
        let dir_str = temp_dir.to_str().unwrap();

        log_startup_error(
            dir_str,
            "Database Connection Failed",
            "sqlite3 open error: file locked",
            "Database is exclusively locked by another process.",
            "Terminate conflicting processes or change DATABASE_PATH.",
        );

        let log_file = temp_dir.join("startup_error.log");
        assert!(log_file.exists(), "startup_error.log must exist");

        let content = std::fs::read_to_string(&log_file).expect("Failed to read startup_error.log");
        assert!(content.contains("[FATAL STARTUP ERROR] Database Connection Failed"));
        assert!(content.contains("sqlite3 open error: file locked"));
        assert!(content.contains("[ROOT CAUSE ANALYSIS (RCA)]"));
        assert!(content.contains("Database is exclusively locked by another process."));
        assert!(content.contains("[ACTIONABLE REMEDIATION]"));
        assert!(content.contains("Terminate conflicting processes or change DATABASE_PATH."));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_setup_panic_hook_and_crash_log_capture() {
        let temp_dir = std::env::temp_dir().join(format!("mf_crash_test_{}", uuid::Uuid::new_v4()));
        let dir_str = temp_dir.to_str().unwrap();

        let prev_hook = std::panic::take_hook();
        setup_panic_hook(dir_str);

        // Run panic inside catch_unwind
        let panic_result = std::panic::catch_unwind(|| {
            panic!("Intentional diagnostic panic verification payload");
        });
        std::panic::set_hook(prev_hook);
        assert!(panic_result.is_err(), "catch_unwind must capture the panic");

        let crash_file = temp_dir.join("crash.log");
        assert!(crash_file.exists(), "crash.log must exist after panic");

        let content = std::fs::read_to_string(&crash_file).expect("Failed to read crash.log");
        assert!(content.contains("[CRASH REPORT] Unhandled Panic"));
        assert!(content.contains("Intentional diagnostic panic verification payload"));
        assert!(content.contains("Backtrace:"));
        assert!(!content.contains("disabled backtrace"));
        assert!(content.contains("Thread:"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_rolling_file_appender_log_dir_and_file_creation() {
        let temp_dir =
            std::env::temp_dir().join(format!("mf_rolling_test_{}", uuid::Uuid::new_v4()));
        let dir_str = temp_dir.to_str().unwrap();

        let appender = tracing_appender::rolling::daily(dir_str, "money-flow.log");
        let (mut non_blocking, _guard) = tracing_appender::non_blocking(appender);

        use std::io::Write;
        non_blocking
            .write_all(b"2026-11-20 INFO [test] Test rolling log message\n")
            .expect("Failed to write to non-blocking appender");
        non_blocking
            .flush()
            .expect("Failed to flush non-blocking appender");

        // Give worker thread a moment to flush to disk
        std::thread::sleep(std::time::Duration::from_millis(100));
        drop(_guard);

        assert!(temp_dir.exists(), "Log directory must exist");
        let entries = std::fs::read_dir(&temp_dir).expect("Failed to read temp log dir");
        let found_log = entries.filter_map(|e| e.ok()).any(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("money-flow.log")
        });

        assert!(
            found_log,
            "A money-flow.log file must have been created in the log directory"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_ansi_color_policy_matrix() {
        // TTY true, NO_COLOR unset -> true
        assert!(is_ansi_enabled(true, None));
        // TTY true, NO_COLOR empty or spaces -> true
        assert!(is_ansi_enabled(true, Some("")));
        assert!(is_ansi_enabled(true, Some("   ")));
        // TTY true, NO_COLOR set to value -> false
        assert!(!is_ansi_enabled(true, Some("1")));
        assert!(!is_ansi_enabled(true, Some("true")));
        assert!(!is_ansi_enabled(true, Some("0")));

        // TTY false (piped/redirected) -> always false
        assert!(!is_ansi_enabled(false, None));
        assert!(!is_ansi_enabled(false, Some("")));
        assert!(!is_ansi_enabled(false, Some("1")));
    }

    #[test]
    fn test_init_dual_sink_subscriber_with_formats() {
        let temp_dir =
            std::env::temp_dir().join(format!("mf_sink_format_test_{}", uuid::Uuid::new_v4()));
        let dir_str = temp_dir.to_str().unwrap();

        // Testing initialization with both Pretty and Compact does not panic
        let guard_pretty =
            init_dual_sink_subscriber(dir_str, "info", crate::config::LogFormat::Pretty);
        drop(guard_pretty);

        let guard_compact =
            init_dual_sink_subscriber(dir_str, "info", crate::config::LogFormat::Compact);
        drop(guard_compact);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
