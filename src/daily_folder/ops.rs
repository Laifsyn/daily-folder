//! Operaciones de dominio: recorrido de archivos, verificación de
//! condiciones y creación de la carpeta "impresos".

use std::path::{Path, PathBuf};

use super::{
    error::ImpresosError, link::ensure_link_for_date, settings::Settings, template::expand_template,
};

// ---------------------------------------------------------------------------
// Helpers privados
// ---------------------------------------------------------------------------

/// Cuenta cuántos archivos contiene un directorio (no recursivo).
fn count_files_in_dir(dir: &Path) -> Result<usize, std::io::Error> {
    let mut count = 0;
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            count += 1;
        }
    }
    Ok(count)
}

/// Determina si un directorio contiene al menos un archivo cuya extensión
/// coincide con la lista de extensiones disparadoras.
fn has_trigger_extension(dir: &Path, extensions: &[&str]) -> Result<bool, std::io::Error> {
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                for trigger in extensions {
                    let trigger_clean = trigger.trim_start_matches('.').to_lowercase();
                    if ext_lower == trigger_clean {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

// ---------------------------------------------------------------------------
// API pública
// ---------------------------------------------------------------------------

/// Verifica si se debe crear la carpeta de impresos dentro de `dir`.
///
/// Devuelve `true` si:
/// - El directorio contiene más de `max_files` archivos, **o**
/// - Contiene al menos un archivo con una extensión de la lista `extensions`.
pub fn should_create_impresos(
    dir: &Path,
    max_files: usize,
    extensions: &[&str],
) -> Result<bool, ImpresosError> {
    if !dir.is_dir() {
        return Ok(false);
    }

    let file_count = count_files_in_dir(dir)?;
    if file_count > max_files {
        return Ok(true);
    }

    if has_trigger_extension(dir, extensions)? {
        return Ok(true);
    }

    Ok(false)
}

/// Genera la ruta de directorio anidado a partir del template y la fecha.
pub fn generate_date_path(
    settings: &Settings,
    date: chrono::NaiveDate,
) -> Result<PathBuf, ImpresosError> {
    let relative = expand_template(&settings.create_path, date, &settings.month_names);
    let root = PathBuf::from(&settings.root_directory);
    Ok(root.join(relative))
}

/// Crea recursivamente los directorios intermedios y devuelve la ruta completa
/// al directorio "día" (hoja).
pub fn ensure_date_directory(
    settings: &Settings,
    date: chrono::NaiveDate,
) -> Result<PathBuf, ImpresosError> {
    let full_path = generate_date_path(settings, date)?;
    std::fs::create_dir_all(&full_path)?;
    Ok(full_path)
}

/// Ejecuta la lógica completa para una fecha dada:
/// 1. Crea (o asegura) la estructura de directorios según el template.
/// 2. Crea el acceso directo (.lnk) en el directorio raíz si la configuración
///    lo habilita.
/// 3. Verifica si se debe crear la carpeta de impresos.
/// 4. Si corresponde, la crea.
///
/// Retorna `Some(path)` con la ruta a la carpeta de impresos si fue creada,
/// o `None` si no se cumplieron las condiciones.
pub fn run_for_date(
    settings: &Settings,
    date: chrono::NaiveDate,
) -> Result<Option<PathBuf>, ImpresosError> {
    let day_dir = ensure_date_directory(settings, date)?;

    // Crear acceso directo al directorio del día (si está configurado)
    if settings.create_link_to_daily_folder {
        if let Err(e) = ensure_link_for_date(settings, date, &day_dir) {
            // No queremos que un fallo al crear el .lnk detenga todo el
            // proceso.  Lo registramos y continuamos.
            tracing::warn!(
                error = %e,
                date = %date.format("%Y-%m-%d"),
                "No se pudo crear el acceso directo al directorio del día"
            );
        }
    }

    let trigger_extensions: Vec<&str> = settings
        .extension_trigger_impresos
        .iter()
        .map(|s| s.as_str())
        .collect();
    if should_create_impresos(
        &day_dir,
        settings.max_files_before_trigger.into(),
        &trigger_extensions,
    )? {
        let impresos_dir = day_dir.join(&settings.impresos_folder_name);
        std::fs::create_dir_all(&impresos_dir)?;
        Ok(Some(impresos_dir))
    } else {
        Ok(None)
    }
}

/// Itera sobre un rango de fechas y ejecuta [`run_for_date`] para cada día.
///
/// Útil para procesar múltiples fechas (ej. un mes entero).
pub fn run_for_date_range(
    settings: &Settings,
    start: chrono::NaiveDate,
    end: chrono::NaiveDate,
) -> Result<Vec<Option<PathBuf>>, ImpresosError> {
    let mut results = Vec::new();
    let mut current = start;
    while current <= end {
        results.push(run_for_date(settings, current)?);
        current = current.succ_opt().unwrap_or(current);
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daily_folder::settings::Settings;

    #[test]
    fn test_should_create_impresos_by_count() {
        let dir = std::env::temp_dir().join("test_impresos_count");
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..60 {
            std::fs::write(dir.join(format!("file_{}.txt", i)), "test").unwrap();
        }
        let result = should_create_impresos(&dir, 50, &[]).unwrap();
        assert!(result, "Debería dispararse por cantidad de archivos");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_should_create_impresos_by_extension() {
        let dir = std::env::temp_dir().join("test_impresos_ext");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("documento.pdf"), "test").unwrap();
        let extensions = vec!["pdf", "png"];
        let result = should_create_impresos(&dir, 100, &extensions).unwrap();
        assert!(result, "Debería dispararse por extensión .pdf");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_should_not_create_impresos() {
        let dir = std::env::temp_dir().join("test_impresos_none");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("documento.txt"), "test").unwrap();
        let extensions = vec!["pdf"];
        let result = should_create_impresos(&dir, 100, &extensions).unwrap();
        assert!(!result, "No debería dispararse");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_generate_date_path() {
        let settings = Settings::default();
        let date = chrono::NaiveDate::from_ymd_opt(2025, 3, 7).unwrap();
        let path = generate_date_path(&settings, date).unwrap();
        assert!(path.starts_with("./"));
        assert!(path.to_str().unwrap().contains("2025"));
        assert!(path.to_str().unwrap().contains("03 marzo"));
        assert!(path.to_str().unwrap().contains("07"));
    }
}
