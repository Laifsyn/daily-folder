//! Module containing functions to create the printed folder.
//!
//! ( Compatible with Windows 32 systems).
//!
//! # Domain Logic
//!
//! This module contains the core domain logic for the prints system.
//! All errors are statically known via [`PrintedError`].
//! File system operations are synchronous and must be executed inside
//! `spawn_blocking` by the application layer.

pub mod app;
pub mod error;
pub mod link;
pub mod ops;
pub mod settings;
mod template;

// Re-export public items so consumers don't need to know the internal
// module structure.
pub use error::DaifoError;
pub use link::{
    LINK_NAME_SEPARATOR, cleanup_stale_links, ensure_link_for_date,
};
pub use ops::{
    ensure_date_directory, generate_date_path, run_for_date,
    run_for_date_range, should_create_printed,
};
pub use settings::{Settings, load_or_create_settings};
