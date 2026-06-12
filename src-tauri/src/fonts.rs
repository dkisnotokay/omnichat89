//! Список установленных шрифтов Windows.
//!
//! Читает имена шрифтов из реестра
//! `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts`,
//! срезает суффиксы формата («(TrueType)») и стилевые варианты
//! («Bold», «Italic»...), оставляя имена семейств для CSS font-family.

use std::collections::BTreeSet;
use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;

/// Стилевые суффиксы, которые срезаем с конца имени для получения семейства.
const STYLE_SUFFIXES: &[&str] = &[
    "Bold", "Italic", "Light", "Regular", "Medium", "Semibold", "SemiBold",
    "Semilight", "SemiLight", "Black", "Thin", "Heavy", "Condensed",
    "Oblique", "ExtraBold", "ExtraLight",
];

/// Получить отсортированный список семейств шрифтов, установленных в системе.
pub fn list_system_fonts() -> Result<Vec<String>, String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let fonts_key = hklm
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts")
        .map_err(|e| format!("Ошибка открытия реестра шрифтов: {}", e))?;

    let mut families = BTreeSet::new();

    for name in fonts_key.enum_values().filter_map(|v| v.ok().map(|(n, _)| n)) {
        // Срезаем "(TrueType)", "(OpenType)" и т.п.
        let mut family = match name.find(" (") {
            Some(idx) => name[..idx].to_string(),
            None => name,
        };

        // Срезаем стилевые суффиксы с конца ("Arial Bold Italic" → "Arial")
        loop {
            let mut stripped = false;
            for suffix in STYLE_SUFFIXES {
                if let Some(base) = family.strip_suffix(suffix) {
                    let base = base.trim_end();
                    if !base.is_empty() {
                        family = base.to_string();
                        stripped = true;
                        break;
                    }
                }
            }
            if !stripped {
                break;
            }
        }

        let family = family.trim();
        if !family.is_empty() && family.len() <= 80 {
            families.insert(family.to_string());
        }
    }

    Ok(families.into_iter().collect())
}
