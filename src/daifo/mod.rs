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

pub use link::cleanup_stale_links;
pub use ops::{run_for_date, run_for_date_range};
