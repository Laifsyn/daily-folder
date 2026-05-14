//! Expansión de templates de ruta con especificadores de Chrono.

use std::collections::HashMap;

use chrono::Datelike;

/// Expande un template de ruta usando los especificadores de formato de
/// Chrono.
///
/// El especificador `%B` (nombre completo del mes) se resuelve primero
/// contra la tabla `month_names`. Si la clave está presente, se sustituye
/// antes de delegar el resto a [`chrono::NaiveDate::format`].
///
/// Si **no** se encuentra en la tabla, `%B` se deja sin reemplazar y Chrono
/// lo resuelve usando la localización del sistema operativo (inglés en
/// Windows 7 sin configuración regional). Esto actúa como fallback para no
/// romper la ejecución si la tabla está incompleta.
///
/// El resto de especificadores (`%Y`, `%m`, `%d`, etc.) se delegan
/// directamente a [`chrono::NaiveDate::format`].
pub(super) fn expand_template(
    template: &str,
    date: chrono::NaiveDate,
    month_names: &HashMap<String, String>,
) -> String {
    let month_num = format!("{:02}", date.month());

    let month_name = month_names.get(&month_num);

    let mut preprocessed = template.to_string();

    // Si encontramos el nombre del mes en la tabla, lo usamos.
    // Esto garantiza que el resultado sea consistente independientemente
    // de la localización del sistema operativo.
    if let Some(name) = month_name {
        preprocessed = preprocessed.replace("%B", name);
    }

    // Chrono se encarga del resto de especificadores estándar.
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
