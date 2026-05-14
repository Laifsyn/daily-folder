//! Tipos de error estáticamente conocidos para la lógica de dominio.

use thiserror::Error;

/// Errores estáticamente conocidos para la lógica de dominio de impresos.
#[derive(Error, Debug)]
pub enum ImpresosError {
    /// Fallo al leer o escribir en el sistema de archivos.
    #[error("error de entrada/salida: {0}")]
    Io(#[from] std::io::Error),

    /// El archivo de configuración TOML no se pudo parsear.
    #[error("error al parsear el archivo de configuración: {0}")]
    SettingsParse(String),

    /// El template de ruta contiene marcadores no soportados.
    #[error("template de ruta inválido: {0}")]
    InvalidTemplate(String),

    /// Falta un nombre de mes en la tabla de configuración.
    #[error(
        "falta el nombre del mes para la clave '{0}' — agregalo en \
         [month_names]"
    )]
    MissingMonthName(String),

    /// No se pudo crear el directorio base.
    #[error("no se pudo crear el directorio: {0}")]
    DirectoryCreationFailed(String),

    /// Fallo al crear el acceso directo (.lnk).
    #[error("no se pudo crear el acceso directo: {0}")]
    LinkCreationFailed(String),
}
