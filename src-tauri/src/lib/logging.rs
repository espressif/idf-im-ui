//! Centralized logging configuration using the fern crate.
//!
//! This module provides a unified logging setup for CLI, GUI, and offline installer
//! with support for different log levels for console vs file, module filtering,
//! and custom log paths.

use fern::Dispatch;
use log::LevelFilter;
use std::path::PathBuf;

use crate::get_log_directory;

/// Log pattern format: "{date} - {level} - {message}"
pub const LOG_PATTERN: &str = "%Y-%m-%d %H:%M:%S - %l - %m";

/// Common fern formatter for consistent log output.
pub fn formatter(out: fern::FormatCallback, message: &std::fmt::Arguments, record: &log::Record) {
    out.finish(format_args!(
        "{} - {} - {}",
        chrono::Local::now().format(LOG_PATTERN),
        record.level(),
        message
    ));
}

/// Setup a simple logger for a subprocess or library that just needs basic logging.
///
/// # Arguments
/// * `log_file_path` - Path to the log file
/// * `level` - Maximum log level to output
pub fn setup_simple(
    log_file_path: PathBuf,
    level: LevelFilter,
) {
    let _ = Dispatch::new()
        .format(formatter)
        .level(level)
        .chain(fern::log_file(log_file_path).unwrap_or_else(|_| {
            fern::log_file("eim_simple.log").unwrap_or_else(|_| panic!("Failed to create fallback log file"))
        }))
        .apply();
}
