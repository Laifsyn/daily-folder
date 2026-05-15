//! Domain operations: file traversal, condition checking,
//! and creation of the "printed" folder.

use std::path::{Path, PathBuf};

use super::{
    error::DaifoError, link::ensure_link_for_date, settings::Settings,
    template::expand_template,
};

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Counts how many files a directory contains (non-recursive).
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

/// Determines whether a directory contains at least one file whose extension
/// matches the trigger extension list.
fn has_trigger_extension(
    dir: &Path,
    extensions: &[&str],
) -> Result<bool, std::io::Error> {
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str())
            {
                let ext_lower = ext.to_lowercase();
                for trigger in extensions {
                    let trigger_clean =
                        trigger.trim_start_matches('.').to_lowercase();
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
// Public API
// ---------------------------------------------------------------------------

/// Checks whether the prints folder should be created inside `dir`.
///
/// Returns `true` if:
/// - The directory contains more than `max_files` files, **or**
/// - It contains at least one file with an extension from the `extensions`
///   list.
pub fn should_create_printed(
    dir: &Path,
    max_files: usize,
    extensions: &[&str],
) -> Result<bool, DaifoError> {
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

/// Generates the nested directory path from the template and the date.
pub fn generate_date_path(
    settings: &Settings,
    date: chrono::NaiveDate,
) -> Result<PathBuf, DaifoError> {
    let relative =
        expand_template(&settings.create_path, date, &settings.month_names);
    let root = PathBuf::from(&settings.root_directory);
    Ok(root.join(relative))
}

/// Recursively creates intermediate directories and returns the full path
/// to the "day" directory (leaf).
pub fn ensure_date_directory(
    settings: &Settings,
    date: chrono::NaiveDate,
) -> Result<PathBuf, DaifoError> {
    let full_path = generate_date_path(settings, date)?;
    std::fs::create_dir_all(&full_path)?;
    Ok(full_path)
}

/// Runs the full logic for a given date:
/// 1. Creates (or ensures) the directory structure according to the template.
/// 2. Creates the shortcut (.lnk) in the root directory if the configuration
///    enables it.
/// 3. Checks whether the prints folder should be created.
/// 4. If applicable, creates it.
///
/// Returns `Some(path)` with the path to the prints folder if it was created,
/// or `None` if the conditions were not met.
pub fn run_for_date(
    settings: &Settings,
    date: chrono::NaiveDate,
) -> Result<Option<PathBuf>, DaifoError> {
    let day_dir = ensure_date_directory(settings, date)?;

    // Create shortcut to the day directory (if configured)
    if settings.create_link_to_daily_folder {
        if let Err(e) = ensure_link_for_date(settings, date, &day_dir) {
            // We don't want a .lnk creation failure to stop the entire
            // process. We log it and continue.
            tracing::warn!(
                error = %e,
                date = %date.format("%Y-%m-%d"),
                "Failed to create shortcut to the day directory"
            );
        }
    }

    let trigger_extensions: Vec<&str> =
        settings.extension_trigger_printed.iter().map(|s| s.as_str()).collect();
    if should_create_printed(
        &day_dir,
        settings.max_files_before_trigger.into(),
        &trigger_extensions,
    )? {
        let printed_dir = day_dir.join(&settings.printed_folder_name);
        std::fs::create_dir_all(&printed_dir)?;
        Ok(Some(printed_dir))
    } else {
        Ok(None)
    }
}

/// Iterates over a date range and runs [`run_for_date`] for each day.
///
/// Useful for processing multiple dates (e.g. an entire month).
pub fn run_for_date_range(
    settings: &Settings,
    start: chrono::NaiveDate,
    end: chrono::NaiveDate,
) -> Result<Vec<Option<PathBuf>>, DaifoError> {
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
    use crate::daifo::settings::Settings;

    #[test]
    fn test_should_create_printed_by_count() {
        let dir = std::env::temp_dir().join("test_printed_count");
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..60 {
            std::fs::write(dir.join(format!("file_{}.txt", i)), "test")
                .unwrap();
        }
        let result = should_create_printed(&dir, 50, &[]).unwrap();
        assert!(result, "Should trigger by file count");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_should_create_printed_by_extension() {
        let dir = std::env::temp_dir().join("test_printed_ext");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("documento.pdf"), "test").unwrap();
        let extensions = vec!["pdf", "png"];
        let result = should_create_printed(&dir, 100, &extensions).unwrap();
        assert!(result, "Should trigger by .pdf extension");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_should_not_create_printed() {
        let dir = std::env::temp_dir().join("test_printed_none");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("documento.txt"), "test").unwrap();
        let extensions = vec!["pdf"];
        let result = should_create_printed(&dir, 100, &extensions).unwrap();
        assert!(!result, "Should not trigger");
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
