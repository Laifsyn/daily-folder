//! Statically known error types for the domain logic.

use thiserror::Error;

/// Statically known errors for the prints domain logic.
#[derive(Error, Debug)]
pub enum DaifoError {
    /// File system read/write failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to parse the TOML configuration file.
    #[error("error parsing configuration file: {0}")]
    SettingsParse(String),

    /// The path template contains unsupported markers.
    #[error("invalid path template: {0}")]
    InvalidTemplate(String),

    /// Missing month name in the configuration table.
    #[error("missing month name for key '{0}' — add it in [month_names]")]
    MissingMonthName(String),

    /// Failed to create the base directory.
    #[error("failed to create directory: {0}")]
    DirectoryCreationFailed(String),

    /// Failed to create the shortcut (.lnk).
    #[error("failed to create shortcut: {0}")]
    SymlinkCreationFailed(String),
}
