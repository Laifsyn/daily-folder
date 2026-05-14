//! Configuración: struct [`Settings`], valores por defecto, y
//! carga/creación del archivo TOML con comentarios.

use std::{collections::HashMap, path::Path};

use serde::{Deserialize, Serialize};

use super::error::ImpresosError;

fn default_root_directory() -> String { "./".to_string() }

fn default_create_path() -> String { "./%Y/%m %B/%d".to_string() }

fn default_extensions() -> Vec<String> {
    vec!["pdf".to_string(), "png".to_string(), "_tf".to_string()]
}

const fn default_max_files() -> u8 { 5 }

/// Done's folder name, inside the daily folder.
fn default_done_folder() -> String { "impreso".to_string() }

const fn default_create_link() -> bool { true }

fn default_duplicate_link_targets() -> Vec<String> { Vec::new() }

pub fn default_month_names() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("01".into(), "enero".into());
    m.insert("02".into(), "febrero".into());
    m.insert("03".into(), "marzo".into());
    m.insert("04".into(), "abril".into());
    m.insert("05".into(), "mayo".into());
    m.insert("06".into(), "junio".into());
    m.insert("07".into(), "julio".into());
    m.insert("08".into(), "agosto".into());
    m.insert("09".into(), "septiembre".into());
    m.insert("10".into(), "octubre".into());
    m.insert("11".into(), "noviembre".into());
    m.insert("12".into(), "diciembre".into());
    m
}

/// App's settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Settings {
    /// Directorio raíz donde se crea la estructura de fechas.
    /// Por defecto: `"./"`.
    #[serde(default = "default_root_directory")]
    pub root_directory: String,

    /// Template de ruta usando los especificadores de formato de Chrono.
    /// Los más comunes:
    /// - `%Y` → año con 4 dígitos (ej. `2025`)
    /// - `%m` → mes con 2 dígitos (ej. `01`)
    /// - `%d` → día con 2 dígitos (ej. `15`)
    /// - `%B` → nombre completo del mes, tomado de [`month_names`]
    ///
    /// Ver: <https://docs.rs/chrono/latest/chrono/format/strftime/index.html>
    #[serde(default = "default_create_path")]
    pub create_path: String,

    /// Extensiones que disparan la creación de la carpeta de impresos.
    #[serde(default = "default_extensions")]
    pub extension_trigger_impresos: Vec<String>,

    /// Número máximo de archivos en un directorio "día" antes de crear la
    /// carpeta de impresos automáticamente.
    #[serde(default = "default_max_files")]
    pub max_files_before_trigger: u8,

    /// Nombre de la carpeta de impresos a crear dentro del directorio día.
    #[serde(default = "default_done_folder")]
    pub impresos_folder_name: String,

    /// Si es `true`, crea un acceso directo (.lnk) al directorio del día
    /// en el directorio raíz.  El nombre del acceso directo es una versión
    /// "aplanada" del template donde los separadores de ruta se
    /// reemplazan por puntos.
    #[serde(default = "default_create_link")]
    pub create_link_to_daily_folder: bool,

    /// Directorios adicionales donde copiar el acceso directo creado.
    /// Útil para duplicar el enlace al Escritorio u otras ubicaciones.
    /// Si un directorio no existe, simplemente se omite.
    #[serde(default = "default_duplicate_link_targets")]
    pub duplicate_daily_folder_link_to: Vec<String>,

    /// Tabla opcional con nombres de meses. Si no se proporciona, se usa el
    /// default en español (ver [`default_month_names`]).
    #[serde(default = "default_month_names")]
    pub month_names: HashMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            root_directory: default_root_directory(),
            create_path: default_create_path(),
            extension_trigger_impresos: default_extensions(),
            max_files_before_trigger: default_max_files(),
            impresos_folder_name: default_done_folder(),
            create_link_to_daily_folder: default_create_link(),
            duplicate_daily_folder_link_to: default_duplicate_link_targets(),
            month_names: default_month_names(),
        }
    }
}

// ---------------------------------------------------------------------------
// TOML I/O con comentarios (toml_edit)
// ---------------------------------------------------------------------------

/// Construye el documento TOML por defecto con comentarios explicativos.
fn build_default_toml_document() -> toml_edit::DocumentMut {
    let mut doc = toml_edit::DocumentMut::new();

    // -- root_directory --
    let root_dir = default_root_directory();
    doc.insert("root_directory", toml_edit::value(root_dir));
    if let Some(mut key) = doc.key_mut("root_directory") {
        key.leaf_decor_mut().set_prefix(
            "# Directorio raíz donde se crea la estructura de fechas.\n# Por \
             defecto es la ubicación de la aplicación.\n# Caso de uso real \
             podría ser otra partición, ej. `D:\\`\n",
        );
    }

    // -- create_path --
    let default_template = default_create_path();
    doc.insert("create_path", toml_edit::value(default_template));
    if let Some(mut key) = doc.key_mut("create_path") {
        key.leaf_decor_mut().set_prefix(
            "# Template de ruta con especificadores de Chrono.\n# %Y = año 4 \
             dígitos\n# %m = mes 2 dígitos\n# %d = día 2 dígitos\n# %B = \
             nombre del mes (tomado de [month_names])\n",
        );
    }

    // -- extension_trigger_impresos --
    let mut ext_array = toml_edit::Array::new();
    ext_array.extend(default_extensions().into_iter());

    doc.insert(
        "extension_trigger_impresos",
        toml_edit::Item::Value(ext_array.into()),
    );
    if let Some(mut key) = doc.key_mut("extension_trigger_impresos") {
        key.leaf_decor_mut().set_prefix(
            "# Extensiones que disparan la creación de la carpeta de \
             impresos\n# La lista puede crecer en el futuro.\n",
        );
    }

    // -- max_files_before_trigger --
    doc.insert("max_files_before_trigger", (default_max_files() as i64).into());
    if let Some(mut key) = doc.key_mut("max_files_before_trigger") {
        key.leaf_decor_mut().set_prefix(
            "# Número máximo de archivos en un directorio día antes de \
             crear\n# automáticamente la carpeta de impresos.\n",
        );
    }

    // -- impresos_folder_name --
    doc.insert("impresos_folder_name", toml_edit::value("impresos"));
    if let Some(mut key) = doc.key_mut("impresos_folder_name") {
        key.leaf_decor_mut().set_prefix(
            "# Nombre de la carpeta de impresos a crear dentro del directorio \
             día.\n",
        );
    }

    // -- create_link_to_daily_folder --
    doc.insert("create_link_to_daily_folder", toml_edit::value(true));
    if let Some(mut key) = doc.key_mut("create_link_to_daily_folder") {
        key.leaf_decor_mut().set_prefix(
            "# Si es true, crea un acceso directo (.lnk) al directorio del \
             día\n# en el directorio raíz.  No requiere permisos de \
             administrador.\n",
        );
    }

    // -- duplicate_daily_folder_link_to --
    let dup_array = toml_edit::Array::new();
    doc.insert(
        "duplicate_daily_folder_link_to",
        toml_edit::Item::Value(dup_array.into()),
    );
    if let Some(mut key) = doc.key_mut("duplicate_daily_folder_link_to") {
        key.leaf_decor_mut().set_prefix(
            "# Directorios adicionales donde copiar el acceso directo \
             (.lnk).\n# Ejemplo: [\"C:\\Users\\anton\\Desktop\"]\n# Si un \
             directorio no existe, se omite sin error.\n",
        );
    }

    // -- [month_names] --
    let mut month_table = toml_edit::Table::new();
    let default_names = default_month_names();
    let mut sorted_keys: Vec<&String> = default_names.keys().collect();
    sorted_keys.sort();
    for key in sorted_keys {
        month_table.insert(key, toml_edit::value(default_names[key].as_str()));
    }
    month_table.set_implicit(true);
    doc.insert("month_names", toml_edit::Item::Table(month_table));
    if let Some(mut key) = doc.key_mut("month_names") {
        key.leaf_decor_mut().set_prefix(
            "# Tabla de nombres de meses.\n# Opcional: si la localización del \
             sistema funciona, se puede dejar vacía.\n# Por defecto se \
             rellena con nombres en español para compatibilidad con Windows \
             7.\n",
        );
    }

    doc
}

/// Carga la configuración desde un archivo TOML.
/// Si el archivo no existe, lo crea con valores por defecto (incluyendo
/// comentarios).
pub fn load_or_create_settings(path: &Path) -> Result<Settings, ImpresosError> {
    if path.exists() {
        let content =
            std::fs::read_to_string(path).map_err(ImpresosError::Io)?;
        let settings: Settings = toml::from_str(&content)
            .map_err(|e| ImpresosError::SettingsParse(e.to_string()))?;
        Ok(settings)
    } else {
        // Crear directorio padre si no existe
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let doc = build_default_toml_document();
        std::fs::write(path, doc.to_string())?;

        // Devolver los defaults
        Ok(Settings::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_month_names_complete() {
        let names = default_month_names();
        for i in 1..=12 {
            let key = format!("{:02}", i);
            assert!(names.contains_key(&key), "Falta el mes {}", key);
        }
    }
}
