//! Creation of shortcuts (Windows .lnk) to the day directory
//! and registration of them in a local database (JSON).
//!
//! Shortcuts are created in the root directory with a
//! "flattened" name derived from the path template.

use std::{
    collections::HashMap,
    io::Write,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};

use super::{error::DaifoError, template::expand_template};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Character used to replace path separators (`/`, `\`) when
/// flattening the shortcut name.
pub const LINK_NAME_SEPARATOR: char = '-';

/// Extension added to the flattened name.
const LINK_EXTENSION: &str = ".lnk";

/// Default path for the file that stores the created links record.
const LINKS_DB_PATH: &str = "./.settings/printed_symlinks.json";

// ---------------------------------------------------------------------------
// Links database (JSON)
// ---------------------------------------------------------------------------

/// An entry in the created shortcuts record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkEntry {
    /// Path of the `.lnk` file.
    pub link_path: String,
    /// Path of the directory the shortcut points to.
    pub target_path: String,
    /// Date it was created for (ISO: `YYYY-MM-DD`).
    pub created_date: String,
}

/// Lightweight JSON database that records the created shortcuts.
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
    /// date is before today. Also deletes the corresponding `.lnk`
    /// file if it is still present.
    ///
    /// Returns the number of removed entries.
    pub fn cleanup_stale(&mut self, today: chrono::NaiveDate) -> usize {
        let mut removed = 0;
        let mut surviving = Vec::new();

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
// Name flattening
// ---------------------------------------------------------------------------

/// Converts a path with separators into a "flattened" name by replacing
/// each separator (`/` or `\`) with [`LINK_NAME_SEPARATOR`].
///
/// Consecutive separators are collapsed into a single replacement
/// character. The `.lnk` extension is appended to the result.
///
/// # Example
///
/// ```ignore
/// let name = flatten_link_name("./2025/01 enero/15");
/// assert_eq!(name, "2025-01 enero-15.lnk");
/// ```
pub fn flatten_link_name(raw: &str) -> String {
    // Remove "./" or ".\" prefix before processing
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

    // Remove a possible leading separator (e.g. "-2025..." → "2025...")
    let trimmed = result
        .strip_prefix(&format!("{LINK_NAME_SEPARATOR}"))
        .unwrap_or(&result);

    format!("{trimmed}{LINK_EXTENSION}")
}

/// Generates the flattened name of the shortcut from the path template
/// and the date.
pub fn make_link_name(
    template: &str,
    date: chrono::NaiveDate,
    month_names: &HashMap<String, String>,
) -> String {
    let expanded = expand_template(template, date, month_names);
    flatten_link_name(&expanded)
}

// ---------------------------------------------------------------------------
// Shortcut creation (.lnk) via PowerShell (without admin)
// ---------------------------------------------------------------------------

/// Creates a Windows shortcut (`.lnk`) that points to `target`.
///
/// Uses PowerShell with `WScript.Shell` to create the `.lnk` file
/// **without** requiring administrator permissions.
pub fn create_shell_link(
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
        .map_err(|e| DaifoError::Io(e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(DaifoError::SymlinkCreationFailed(stderr.trim().to_string()))
    }
}

// ---------------------------------------------------------------------------
// Orchestration
// ---------------------------------------------------------------------------

/// Creates the shortcut to the day directory (if the configuration allows
/// it) and registers it in the database.
///
/// Also copies the `.lnk` to each path listed in
/// `duplicate_daily_folder_link_to` (for example, the Desktop).
/// If a target directory does not exist, it is silently skipped.
///
/// Returns `Some(link_path)` with the path of the primary link if it was
/// created, or `None` if the configuration disables it or the link already
/// existed.
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

    // ── Primary link (in the root directory) ──────────────────────────
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

    // ── Duplicates in additional paths ───────────────────────────────────
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
                "Failed to copy shortcut to duplicate path"
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
