//! Módulo que contiene funciones para crear la carpeta de impresos.
//! Compatible con un sistema de Windows 32.
//!
//! # Domain Logic
//!
//! Este módulo contiene la lógica de dominio (core) para el sistema de
//! impresos. Todos los errores son estáticamente conocidos mediante
//! [`ImpresosError`]. Las operaciones de sistema de archivos son síncronas y
//! deben ser ejecutadas dentro de `spawn_blocking` por la capa de aplicación.

pub mod app;
pub mod error;
pub mod link;
pub mod ops;
pub mod settings;
mod template;

// Re-exportar los ítems públicos para que los consumidores no tengan que
// conocer la estructura interna de módulos.
pub use error::ImpresosError;
pub use link::{
    LINK_NAME_SEPARATOR, cleanup_stale_links, ensure_link_for_date,
};
pub use ops::{
    ensure_date_directory, generate_date_path, run_for_date,
    run_for_date_range, should_create_impresos,
};
pub use settings::{Settings, load_or_create_settings};
