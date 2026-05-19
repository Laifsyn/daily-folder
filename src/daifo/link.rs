//! Creation of directory symbolic links to the day directory
//! and registration of them in a local database (JSON).
//!
//! Symlinks are created in the root directory with a
//! "flattened" name derived from the path template.
//!
//! On Windows, creating symbolic links typically requires
//! administrator privileges or Developer Mode to be enabled.
//! If the symlink cannot be created (e.g. due to missing
//! permissions), the operation is logged at debug level and
//! skipped gracefully so the rest of the execution can continue.

use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::{error::DaifoError, template::expand_template};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Character used to replace path separators (`/`, `\`) when
/// flattening the symlink name.
pub const LINK_NAME_SEPARATOR: char = '-';

/// Default path for the file that stores the created symlinks record.
const LINKS_DB_PATH: &str = "./.setting/printed_symlinks.json";

/// An entry in the created symlinks record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkEntry {
    /// Path of the symlink.
    pub link_path: String,
    /// Path of the directory the symlink points to.
    pub target_path: String,
    /// Date it was created for (ISO: `YYYY-MM-DD`).
    pub created_date: String,
}

/// Lightweight JSON database that records the created symlinks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LinksDatabase {
    pub links: Vec<LinkEntry>,
}

impl LinksDatabase {
    /// Loads the database from disk, or returns an empty one if the
    /// file does not exist or is corrupt.
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

    /// Persists the database to disk.
    pub fn save(&self) -> Result<(), DaifoError> {
        let path = Path::new(LINKS_DB_PATH);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| {
            DaifoError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("error serializing links database: {e}"),
            ))
        })?;
        let mut file = std::fs::File::create(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    /// Registers a new link.
    pub fn insert(&mut self, entry: LinkEntry) { self.links.push(entry); }

    /// Removes entries whose target directory no longer exists, or whose
    /// date is before today. Also deletes the corresponding symlink
    /// if it is still present.
    ///
    /// Returns the number of removed entries.
    pub fn cleanup_stale(&mut self, today: chrono::NaiveDate) -> usize {
        let mut removed = 0;
        let mut surviving = Vec::<LinkEntry>::new();

        for entry in self.links.drain(..) {
            let target = Path::new(&entry.target_path);

            // Condition 1: the target directory no longer exists
            let target_gone = !target.exists();

            // Condition 2: the entry is from a date before today
            let is_old = chrono::NaiveDate::parse_from_str(
                &entry.created_date,
                "%Y-%m-%d",
            )
            .map(|entry_date| entry_date < today)
            .unwrap_or(true);

            if target_gone || is_old {
                let link = Path::new(&entry.link_path);
                if link.exists() {
                    // Try to remove as a directory symlink first,
                    // then fall back to file removal for legacy .lnk
                    // shortcuts that may still exist.
                    if std::fs::remove_dir(link).is_err() {
                        let _ = std::fs::remove_file(link);
                    }
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
// Name flattening
// ---------------------------------------------------------------------------

/// Converts a path with separators into a "flattened" name by replacing
/// each separator (`/` or `\`) with [`LINK_NAME_SEPARATOR`].
///
/// Consecutive separators are collapsed into a single replacement
/// character.
///
/// # Example
///
/// ```ignore
/// let name = flatten_link_name("./2025/01 enero/15");
/// assert_eq!(name, "2025-01 enero-15");
/// ```
pub fn flatten_link_name(raw: &str) -> String {
    // Remove "./" or ".\" prefix before processing
    let raw = raw
        .strip_prefix("./")
        .or_else(|| raw.strip_prefix(".\\"))
        .unwrap_or(raw);

    let mut result = String::with_capacity(raw.len());
    let mut prev_was_sep = false;

    for ch in raw.chars() {
        if ch == '/' || ch == '\\' {
            if !prev_was_sep {
                result.push(LINK_NAME_SEPARATOR);
                prev_was_sep = true;
            }
        } else if ch == LINK_NAME_SEPARATOR {
            if !prev_was_sep {
                result.push(ch);
                prev_was_sep = true;
            }
        } else {
            result.push(ch);
            prev_was_sep = false;
        }
    }

    // Remove a possible leading separator (e.g. "-2025..." → "2025...")
    let trimmed: &str = result
        .strip_prefix(&format!("{LINK_NAME_SEPARATOR}"))
        .unwrap_or(&result);

    trimmed.to_string()
}

/// Generates the flattened name of the symlink from the path template
/// and the date.
pub fn make_link_name(
    template: &str,
    date: chrono::NaiveDate,
    month_names: &HashMap<String, String>,
) -> String {
    let expanded: String = expand_template(template, date, month_names);
    flatten_link_name(&expanded)
}

// ---------------------------------------------------------------------------
// Directory symlink creation
// ---------------------------------------------------------------------------

/// Creates a directory symbolic link that points to `target`.
///
/// Uses [`std::os::windows::fs::symlink_dir`] on Windows.
/// **Requires administrator privileges or Developer Mode** to be
/// enabled; if the operation fails for that reason, the caller should
/// log the error at debug level and continue.
///
/// Returns `Ok(())` on success, or `Err` with the I/O reason.
pub fn create_directory_symlink(
    target: &Path,
    link_path: &Path,
) -> Result<(), DaifoError> {
    let target_abs =
        std::path::absolute(target).unwrap_or_else(|_| target.to_path_buf());
    let link_abs = if link_path.is_absolute() {
        link_path.to_path_buf()
    } else {
        std::env::current_dir()?.join(link_path)
    };

    // If the link already exists, remove it first so we can replace it.
    if link_abs.exists() {
        // Try directory first (for existing symlinks), then file
        // (for legacy .lnk files that might occupy the name).
        if std::fs::remove_dir(&link_abs).is_err() {
            std::fs::remove_file(&link_abs)?;
        }
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(&target_abs, &link_abs)?;
    }
    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(&target_abs, &link_abs)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Orchestration
// ---------------------------------------------------------------------------

/// Creates the directory symlink to the day directory (if the
/// configuration allows it) and registers it in the database.
///
/// Also creates a symlink at each path listed in
/// `duplicate_daily_folder_link_to` (for example, the Desktop).
/// If a target directory does not exist, it is silently skipped.
///
/// If symlink creation fails (e.g. due to missing admin permissions),
/// the error is logged at **debug** level and execution continues.
///
/// Returns `Some(link_path)` with the path of the primary symlink if it
/// was created, or `None` if the configuration disables it or the symlink
/// already existed.
pub fn ensure_link_for_date(
    settings: &super::settings::Settings,
    date: chrono::NaiveDate,
    day_dir: &Path,
) -> Result<Option<PathBuf>, DaifoError> {
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

    // Primary symlink (in the root directory)
    if !link_path.exists() {
        match create_directory_symlink(day_dir, &link_path) {
            Ok(()) => {
                any_created = true;
            }
            Err(e) => {
                tracing::debug!(
                    error = %e,
                    link = %link_path.display(),
                    target = %day_dir.display(),
                    "Failed to create directory symlink (missing \
                     permissions?); skipping"
                );
            }
        }
    }
    if !db.links.iter().any(|e| e.link_path == link_path.to_string_lossy()) {
        db.insert(LinkEntry {
            link_path: link_path.to_string_lossy().to_string(),
            target_path: target_str.clone(),
            created_date: created_date.clone(),
        });
    }

    // Duplicates in additional paths
    for dup_root in &settings.duplicate_daily_folder_link_to {
        let dup_dir = Path::new(dup_root);
        if !dup_dir.is_dir() {
            continue;
        }

        let dup_path = dup_dir.join(&link_name);
        if dup_path.exists() {
            // If the link already exists, we assume it's correct and skip it.
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

        match create_directory_symlink(day_dir, &dup_path) {
            Ok(()) => {
                any_created = true;
            }
            Err(e) => {
                tracing::debug!(
                    error = %e,
                    link = %dup_path.display(),
                    target = %day_dir.display(),
                    "Failed to create duplicate directory symlink \
                     (missing permissions?); skipping"
                );
                continue;
            }
        }

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

/// Runs cleanup of stale entries in the links database.
pub fn cleanup_stale_links(
    today: chrono::NaiveDate,
) -> Result<usize, DaifoError> {
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
        assert_eq!(name, "2025-01 enero-15");
    }

    #[test]
    fn test_flatten_consecutive_separators() {
        let name = flatten_link_name("2026//06 junio");
        assert_eq!(name, "2026-06 junio");
    }

    #[test]
    fn test_flatten_backslashes() {
        let name = flatten_link_name(r"2026\06 junio\15");
        assert_eq!(name, "2026-06 junio-15");
    }

    #[test]
    fn test_flatten_mixed_separators() {
        let name = flatten_link_name("2026/06 junio\\15");
        assert_eq!(name, "2026-06 junio-15");
    }

    #[test]
    fn test_flatten_no_separators() {
        let name = flatten_link_name("2026");
        assert_eq!(name, "2026");
    }

    #[test]
    fn test_flatten_consecutive_dots_deduplicated() {
        let name = flatten_link_name("2026.. 06 June");
        assert_eq!(name, "2026. 06 June");
    }
}
