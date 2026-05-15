//! Configuration: [`Settings`] struct, default values, and
//! loading/creation of the TOML file with comments.

use std::{collections::HashMap, path::Path};

use serde::{Deserialize, Serialize};

use super::error::DaifoError;

fn default_root_directory() -> String { "./".to_string() }

fn default_create_path() -> String { "./%Y/%m %B/%d".to_string() }

fn default_extensions() -> Vec<String> {
    vec!["pdf".to_string(), "png".to_string(), "_tf".to_string()]
}

const fn default_max_files() -> u8 { 5 }

/// Prints folder name, inside the daily folder.
fn default_done_folder() -> String { "printed".to_string() }

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
    /// Root directory where the date structure is created.
    /// Default: `"./"`.
    #[serde(default = "default_root_directory")]
    pub root_directory: String,

    /// Path template using Chrono format specifiers.
    /// Most common:
    /// - `%Y` → 4-digit year (e.g. `2025`)
    /// - `%m` → 2-digit month (e.g. `01`)
    /// - `%d` → 2-digit day (e.g. `15`)
    /// - `%B` → full month name, taken from [`month_names`]
    ///
    /// See: <https://docs.rs/chrono/latest/chrono/format/strftime/index.html>
    #[serde(default = "default_create_path")]
    pub create_path: String,

    /// Extensions that trigger the creation of the prints folder.
    #[serde(default = "default_extensions")]
    pub extension_trigger_printed: Vec<String>,

    /// Maximum number of files in a "day" directory before automatically
    /// creating the prints folder.
    #[serde(default = "default_max_files")]
    pub max_files_before_trigger: u8,

    /// Name of the prints folder to create inside the day directory.
    #[serde(default = "default_done_folder")]
    pub printed_folder_name: String,

    /// If `true`, creates a directory symbolic link to the day directory
    /// in the root directory.  The symlink name is a "flattened" version
    /// of the template where path separators are
    /// replaced by dashes.
    #[serde(default = "default_create_link")]
    pub create_link_to_daily_folder: bool,

    /// Additional directories where to create a copy of the symlink.
    /// Useful for placing the link on the Desktop or other locations.
    /// If a directory doesn't exist, it is simply skipped.
    #[serde(default = "default_duplicate_link_targets")]
    pub duplicate_daily_folder_link_to: Vec<String>,

    /// Optional table with month names. If not provided, the Spanish
    /// default is used (see [`default_month_names`]).
    #[serde(default = "default_month_names")]
    pub month_names: HashMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            root_directory: default_root_directory(),
            create_path: default_create_path(),
            extension_trigger_printed: default_extensions(),
            max_files_before_trigger: default_max_files(),
            printed_folder_name: default_done_folder(),
            create_link_to_daily_folder: default_create_link(),
            duplicate_daily_folder_link_to: default_duplicate_link_targets(),
            month_names: default_month_names(),
        }
    }
}

// ---------------------------------------------------------------------------
// TOML I/O with comments (toml_edit)
// ---------------------------------------------------------------------------

/// Builds the default TOML document with explanatory comments.
fn build_default_toml_document() -> toml_edit::DocumentMut {
    let mut doc = toml_edit::DocumentMut::new();

    // -- root_directory --
    let root_dir = default_root_directory();
    doc.insert("root_directory", toml_edit::value(root_dir));
    if let Some(mut key) = doc.key_mut("root_directory") {
        key.leaf_decor_mut().set_prefix(
            "# Root directory where the date structure is created.\n# Por \
             default is the application's location.\n# A real use case could \
             be another partition, e.g. `D:\\`\n",
        );
    }

    // -- create_path --
    let default_template = default_create_path();
    doc.insert("create_path", toml_edit::value(default_template));
    if let Some(mut key) = doc.key_mut("create_path") {
        key.leaf_decor_mut().set_prefix(
            "# Path template with Chrono specifiers.\n# %Y = 4-digit year\n# \
             %m = mes 2 year\n# %d = día 2 year\n# %B = month name (taken \
             from [month_names])\n",
        );
    }

    // -- extension_trigger_printed --
    let mut ext_array = toml_edit::Array::new();
    ext_array.extend(default_extensions().into_iter());

    doc.insert(
        "extension_trigger_printed",
        toml_edit::Item::Value(ext_array.into()),
    );
    if let Some(mut key) = doc.key_mut("extension_trigger_printed") {
        key.leaf_decor_mut().set_prefix(
            "# Extensions that trigger the creation of the daily folder's \
             printed\n# The list may grow in the future.\n",
        );
    }

    // -- max_files_before_trigger --
    doc.insert("max_files_before_trigger", (default_max_files() as i64).into());
    if let Some(mut key) = doc.key_mut("max_files_before_trigger") {
        key.leaf_decor_mut().set_prefix(
            "# Maximum number of files in a day directory before crear\n# \
             creating the prints folder.\n",
        );
    }

    // -- printed_folder_name --
    doc.insert("printed_folder_name", toml_edit::value("printed"));
    if let Some(mut key) = doc.key_mut("printed_folder_name") {
        key.leaf_decor_mut().set_prefix(
            "# Name of the prints folder to create inside the day día.\n",
        );
    }

    // -- create_link_to_daily_folder --
    doc.insert("create_link_to_daily_folder", toml_edit::value(true));
    if let Some(mut key) = doc.key_mut("create_link_to_daily_folder") {
        key.leaf_decor_mut().set_prefix(
            "# If true, creates a directory symbolic link to the day\n# in \
             the root directory. Requires admin or Developer Mode.\n",
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
            "# Additional directories where to create a copy of the \
             symlink.\n# Example: [\"C:\\Users\\anton\\Desktop\"]\n# Si un \
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
            "# Table of month names.\n# Optional: if the system locale works, \
             it can be left empty.\n# By default it is filled with Spanish \
             names for Windows 7 compatibility 7.\n",
        );
    }

    doc
}

/// Loads the configuration from a TOML file.
/// If the file doesn't exist, it creates it with default values (including
/// comentarios).
pub fn load_or_create_settings(path: &Path) -> Result<Settings, DaifoError> {
    if path.exists() {
        let content = std::fs::read_to_string(path).map_err(DaifoError::Io)?;
        let settings: Settings = toml::from_str(&content)
            .map_err(|e| DaifoError::SettingsParse(e.to_string()))?;
        Ok(settings)
    } else {
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let doc = build_default_toml_document();
        std::fs::write(path, doc.to_string())?;

        // Return the defaults
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
            assert!(names.contains_key(&key), "Missing month {}", key);
        }
    }
}
