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
}
