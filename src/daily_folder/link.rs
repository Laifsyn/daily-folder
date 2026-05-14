//! Creación de accesos directos (.lnk de Windows) al directorio del día
//! y registro de los mismos en una base de datos local (JSON).
//!
//! Los accesos directos se crean en el directorio raíz con un nombre
//! "aplanado" derivado del template de ruta.

use std::{
    collections::HashMap,
    io::Write,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};

use super::{error::ImpresosError, template::expand_template};

// ---------------------------------------------------------------------------
// Constantes
// ---------------------------------------------------------------------------

/// Carácter usado para reemplazar los separadores de ruta (`/`, `\`) al
/// aplanar el nombre del acceso directo.
pub const LINK_NAME_SEPARATOR: char = '-';

/// Extensión que se añade al nombre aplanado.
const LINK_EXTENSION: &str = ".lnk";

/// Ruta por defecto del archivo que guarda el registro de enlaces creados.
const LINKS_DB_PATH: &str = "./.settings/impresos_links.json";

// ---------------------------------------------------------------------------
// Base de datos de enlaces (JSON)
// ---------------------------------------------------------------------------

/// Una entrada en el registro de accesos directos creados.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkEntry {
    /// Ruta del archivo `.lnk`.
    pub link_path: String,
    /// Ruta del directorio al que apunta el acceso directo.
    pub target_path: String,
    /// Fecha para la que se creó (ISO: `YYYY-MM-DD`).
    pub created_date: String,
}

/// Base de datos ligera en JSON que registra los accesos directos creados.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LinksDatabase {
    pub links: Vec<LinkEntry>,
}

impl LinksDatabase {
    /// Carga la base de datos desde disco, o devuelve una vacía si el
    /// archivo no existe o está corrupto.
    pub fn load() -> Self {
        let path = Path::new(LINKS_DB_PATH);
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    serde_json::from_str(&content).unwrap_or_default()
                }
                Err(_) => Self::default(),
            }
        } else {
            Self::default()
        }
    }

    /// Persiste la base de datos a disco.
    pub fn save(&self) -> Result<(), ImpresosError> {
        let path = Path::new(LINKS_DB_PATH);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| {
            ImpresosError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("error al serializar la base de datos de enlaces: {e}"),
            ))
        })?;
        let mut file = std::fs::File::create(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    /// Registra un nuevo enlace.
    pub fn insert(&mut self, entry: LinkEntry) { self.links.push(entry); }

    /// Elimina las entradas cuyo directorio destino ya no existe, o cuya
    /// fecha es anterior a hoy.  También elimina el archivo `.lnk`
    /// correspondiente si todavía está presente.
    ///
    /// Retorna el número de entradas eliminadas.
    pub fn cleanup_stale(&mut self, today: chrono::NaiveDate) -> usize {
        let mut removed = 0;
        let mut surviving = Vec::new();

        for entry in self.links.drain(..) {
            let target = Path::new(&entry.target_path);

            // Condición 1: el directorio destino ya no existe
            let target_gone = !target.exists();

            // Condición 2: la entrada es de una fecha anterior a hoy
            let is_old = chrono::NaiveDate::parse_from_str(
                &entry.created_date,
                "%Y-%m-%d",
            )
            .map(|entry_date| entry_date < today)
            .unwrap_or(true);

            if target_gone || is_old {
                let link = Path::new(&entry.link_path);
                if link.exists() {
                    let _ = std::fs::remove_file(link);
                }
                removed += 1;
            } else {
                surviving.push(entry);
            }
        }

        self.links = surviving;
        removed
    }
}

// ---------------------------------------------------------------------------
// Aplanado del nombre
// ---------------------------------------------------------------------------

/// Convierte una ruta con separadores en un nombre "aplanado" reemplazando
/// cada separador (`/` o `\`) por [`LINK_NAME_SEPARATOR`].
///
/// Los separadores consecutivos se contraen en un único carácter de
/// reemplazo.  Al resultado se le añade la extensión `.lnk`.
///
/// # Ejemplo
///
/// ```ignore
/// let name = flatten_link_name("./2025/01 enero/15");
/// assert_eq!(name, "2025-01 enero-15.lnk");
/// ```
pub fn flatten_link_name(raw: &str) -> String {
    // Quitar prefijo "./" o ".\" antes de procesar
    let raw = raw
        .strip_prefix("./")
        .or_else(|| raw.strip_prefix(".\\"))
        .unwrap_or(raw);

    let mut result = String::with_capacity(raw.len() + LINK_EXTENSION.len());
    let mut prev_was_sep = false;

    for ch in raw.chars() {
        if ch == '/' || ch == '\\' {
            if !prev_was_sep {
                result.push(LINK_NAME_SEPARATOR);
                prev_was_sep = true;
            }
        } else if ch == '.' {
            if !prev_was_sep {
                result.push(ch);
                prev_was_sep = true;
            }
        } else {
            result.push(ch);
            prev_was_sep = false;
        }
    }

    // Quitar un posible separador inicial (ej: "-2025..." → "2025...")
    let trimmed = result
        .strip_prefix(&format!("{LINK_NAME_SEPARATOR}"))
        .unwrap_or(&result);

    format!("{trimmed}{LINK_EXTENSION}")
}

/// Genera el nombre aplanado del acceso directo a partir del template de
/// ruta y la fecha.
pub fn make_link_name(
    template: &str,
    date: chrono::NaiveDate,
    month_names: &HashMap<String, String>,
) -> String {
    let expanded = expand_template(template, date, month_names);
    flatten_link_name(&expanded)
}

// ---------------------------------------------------------------------------
// Creación del acceso directo (.lnk) vía PowerShell (sin admin)
// ---------------------------------------------------------------------------

/// Crea un acceso directo de Windows (`.lnk`) que apunta a `target`.
///
/// Usa PowerShell con `WScript.Shell` para crear el archivo `.lnk` **sin**
/// requerir permisos de administrador.
pub fn create_shell_link(
    target: &Path,
    link_path: &Path,
) -> Result<(), ImpresosError> {
    let target_abs =
        std::path::absolute(target).unwrap_or_else(|_| target.to_path_buf());
    let link_abs = if link_path.is_absolute() {
        link_path.to_path_buf()
    } else {
        std::env::current_dir()?.join(link_path)
    };

    let ps_script = format!(
        r#"
$WScriptShell = New-Object -ComObject WScript.Shell
$Shortcut = $WScriptShell.CreateShortcut('{link}')
$Shortcut.TargetPath = '{target}'
$Shortcut.Save()
"#,
        link = link_abs.to_str().unwrap_or("").replace('\'', "''"),
        target = target_abs.to_str().unwrap_or("").replace('\'', "''"),
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()
        .map_err(|e| ImpresosError::Io(e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(ImpresosError::LinkCreationFailed(stderr.trim().to_string()))
    }
}

// ---------------------------------------------------------------------------
// Orquestación
// ---------------------------------------------------------------------------

/// Crea el acceso directo al directorio del día (si la configuración lo
/// permite) y lo registra en la base de datos.
///
/// También copia el `.lnk` a cada ruta listada en
/// `duplicate_daily_folder_link_to` (por ejemplo, el Escritorio).
/// Si un directorio destino no existe, se omite sin error.
///
/// Retorna `Some(link_path)` con la ruta del enlace primario si se creó,
/// o `None` si la configuración lo deshabilita o el enlace ya existía.
pub fn ensure_link_for_date(
    settings: &super::settings::Settings,
    date: chrono::NaiveDate,
    day_dir: &Path,
) -> Result<Option<PathBuf>, ImpresosError> {
    if !settings.create_link_to_daily_folder {
        return Ok(None);
    }

    let link_name =
        make_link_name(&settings.create_path, date, &settings.month_names);
    let link_path = PathBuf::from(&settings.root_directory).join(&link_name);

    let created_date = date.format("%Y-%m-%d").to_string();
    let target_str = day_dir.to_string_lossy().to_string();

    let mut db = LinksDatabase::load();
    let mut any_created = false;

    // ── Enlace primario (en el directorio raíz) ────────────────────────
    if !link_path.exists() {
        create_shell_link(day_dir, &link_path)?;
        any_created = true;
    }
    if !db.links.iter().any(|e| e.link_path == link_path.to_string_lossy()) {
        db.insert(LinkEntry {
            link_path: link_path.to_string_lossy().to_string(),
            target_path: target_str.clone(),
            created_date: created_date.clone(),
        });
    }

    // ── Duplicados en rutas adicionales ─────────────────────────────────
    for dup_root in &settings.duplicate_daily_folder_link_to {
        let dup_dir = Path::new(dup_root);
        if !dup_dir.is_dir() {
            continue;
        }

        let dup_path = dup_dir.join(&link_name);
        if dup_path.exists() {
            if !db
                .links
                .iter()
                .any(|e| e.link_path == dup_path.to_string_lossy())
            {
                db.insert(LinkEntry {
                    link_path: dup_path.to_string_lossy().to_string(),
                    target_path: target_str.clone(),
                    created_date: created_date.clone(),
                });
            }
            continue;
        }

        if let Err(e) = std::fs::copy(&link_path, &dup_path) {
            tracing::warn!(
                error = %e,
                src = %link_path.display(),
                dst = %dup_path.display(),
                "No se pudo copiar el acceso directo a la ruta duplicada"
            );
            continue;
        }
        any_created = true;

        db.insert(LinkEntry {
            link_path: dup_path.to_string_lossy().to_string(),
            target_path: target_str.clone(),
            created_date: created_date.clone(),
        });
    }

    if any_created {
        db.save()?;
    }

    Ok(Some(link_path))
}

/// Ejecuta la limpieza de entradas obsoletas en la base de datos de
/// enlaces.
pub fn cleanup_stale_links(
    today: chrono::NaiveDate,
) -> Result<usize, ImpresosError> {
    let mut db = LinksDatabase::load();
    let removed = db.cleanup_stale(today);
    if removed > 0 {
        db.save()?;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flatten_basic() {
        let name = flatten_link_name("./2025/01 enero/15");
        assert_eq!(name, "2025-01 enero-15.lnk");
    }

    #[test]
    fn test_flatten_consecutive_separators() {
        let name = flatten_link_name("2026//06 junio");
        assert_eq!(name, "2026-06 junio.lnk");
    }

    #[test]
    fn test_flatten_backslashes() {
        let name = flatten_link_name(r"2026\06 junio\15");
        assert_eq!(name, "2026-06 junio-15.lnk");
    }

    #[test]
    fn test_flatten_mixed_separators() {
        let name = flatten_link_name("2026/06 junio\\15");
        assert_eq!(name, "2026-06 junio-15.lnk");
    }

    #[test]
    fn test_flatten_no_separators() {
        let name = flatten_link_name("2026");
        assert_eq!(name, "2026.lnk");
    }

    #[test]
    fn test_flatten_consecutive_dots_deduplicated() {
        let name = flatten_link_name("2026.. 06 June");
        assert_eq!(name, "2026. 06 June.lnk");
    }
}
