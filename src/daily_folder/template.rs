//! Path template expansion with Chrono specifiers.

use std::collections::HashMap;

use chrono::Datelike;

/// Expands a path template using Chrono format specifiers.
///
/// The `%B` specifier (full month name) is resolved first against the
/// `month_names` table. If the key is present, it is substituted before
/// delegating the rest to [`chrono::NaiveDate::format`].
///
/// If it is **not** found in the table, `%B` is left unreplaced and Chrono
/// resolves it using the operating system locale (English on Windows 7
/// without regional settings). This acts as a fallback to avoid breaking
/// execution if the table is incomplete.
///
/// The remaining specifiers (`%Y`, `%m`, `%d`, etc.) are delegated directly
/// to [`chrono::NaiveDate::format`].
pub(super) fn expand_template(
    template: &str,
    date: chrono::NaiveDate,
    month_names: &HashMap<String, String>,
) -> String {
    let month_num = format!("{:02}", date.month());

    let month_name = month_names.get(&month_num);

    let mut preprocessed = template.to_string();

    // If we find the month name in the table, we use it.
    // This ensures the result is consistent regardless of the operating
    // system locale.
    if let Some(name) = month_name {
        preprocessed = preprocessed.replace("%B", name);
    }

    // Chrono handles the remaining standard specifiers.
    let result = date.format(&preprocessed).to_string();

    result
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::NaiveDate;

    use super::*;
    use crate::daily_folder::settings::default_month_names;

    #[test]
    fn test_expand_template_basic() {
        let month_names = default_month_names();
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        let result = expand_template("%Y/%m %B/%d", date, &month_names);
        assert_eq!(result, "2025/01 enero/15");
    }

    #[test]
    fn test_expand_template_december() {
        let month_names = default_month_names();
        let date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
        let result = expand_template("%Y/%m %B/%d", date, &month_names);
        assert_eq!(result, "2025/12 diciembre/31");
    }

    #[test]
    fn test_expand_template_missing_month_fallback_to_locale() {
        let mut month_names = HashMap::new();
        month_names.insert("01".into(), "enero".into());

        let date = NaiveDate::from_ymd_opt(2025, 2, 10).unwrap();
        let format = "%Y/%m %B/%d";
        // system locale fallback: if the month name is missing, it should fall
        // back to the system locale.
        let expected_formatted = date.format(format).to_string();
        let result = expand_template(format, date, &month_names);
        assert_eq!(
            expected_formatted, result,
            "when month name is missing, should fall back to system locale"
        );
    }
}
