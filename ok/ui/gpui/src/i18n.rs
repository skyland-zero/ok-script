//! Translation layer that reuses the web frontend's generated catalogs.
//!
//! `i18n/catalogs.json` is a copy of `web_src/src/i18n/catalogs.json` (produced
//! by `scripts/generate_web_i18n.py`); `tests/test_web_i18n.py` and
//! `tests/test_gpui_parity.py` keep the two in sync.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

const CATALOGS_JSON: &str = include_str!("../i18n/catalogs.json");

pub const LOCALES: [&str; 6] = ["zh_CN", "zh_TW", "en_US", "es_ES", "ja_JP", "ko_KR"];

static CATALOGS: OnceLock<HashMap<String, HashMap<String, String>>> = OnceLock::new();
static LOCALE: RwLock<&'static str> = RwLock::new("en_US");

fn catalogs() -> &'static HashMap<String, HashMap<String, String>> {
    CATALOGS.get_or_init(|| {
        serde_json::from_str::<HashMap<String, HashMap<String, String>>>(CATALOGS_JSON)
            .unwrap_or_default()
    })
}

/// Mirror of the web frontend's `resolveLocale`.
pub fn resolve_locale(language: &str) -> &'static str {
    let language = language.trim();
    if language.is_empty() || language.eq_ignore_ascii_case("auto") {
        return system_locale();
    }
    let normalized = language.replace('-', "_");
    let lower = normalized.to_ascii_lowercase();
    if lower == "zh_tw" || lower == "zh_hk" || lower == "zh_hant" || lower.contains("hant") {
        return "zh_TW";
    }
    if lower.starts_with("zh") {
        return "zh_CN";
    }
    for locale in LOCALES {
        if locale.eq_ignore_ascii_case(&normalized) {
            return locale;
        }
    }
    let base = lower.split('_').next().unwrap_or_default();
    for locale in LOCALES {
        if locale
            .to_ascii_lowercase()
            .starts_with(&format!("{base}_"))
        {
            return locale;
        }
    }
    "en_US"
}

fn system_locale() -> &'static str {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                return resolve_locale(&value);
            }
        }
    }
    "en_US"
}

/// Select the active locale (accepts `Auto` or a locale key) and return it.
pub fn set_locale(language: &str) -> &'static str {
    let resolved = resolve_locale(language);
    if let Ok(mut current) = LOCALE.write() {
        *current = resolved;
    }
    // Keep gpui-component's own strings (dialog buttons, table sort hints, ...)
    // in the same language.
    let rust_i18n_locale = match resolved {
        "zh_CN" => "zh-CN".to_owned(),
        "zh_TW" => "zh-TW".to_owned(),
        other => other.replace('_', "-"),
    };
    rust_i18n::set_locale(&rust_i18n_locale);
    resolved
}

pub fn locale() -> &'static str {
    LOCALE.read().map(|value| *value).unwrap_or("en_US")
}

pub fn language_label(language: &str) -> String {
    match language {
        "Auto" => "Use system setting",
        "zh_CN" => "简体中文",
        "zh_TW" => "繁體中文",
        "en_US" => "English",
        "es_ES" => "Español",
        "ja_JP" => "日本語",
        "ko_KR" => "한국인",
        other => other,
    }
    .to_owned()
}

/// Translate a source string, falling back to the key itself.
pub fn t(key: &str) -> String {
    let locale = locale();
    if let Some(catalog) = catalogs().get(locale) {
        if let Some(value) = catalog.get(key) {
            return value.clone();
        }
    }
    key.to_owned()
}

/// Translate and interpolate `{name}` placeholders.
pub fn tv(key: &str, params: &[(&str, &str)]) -> String {
    let mut text = t(key);
    for (name, value) in params {
        let placeholder = format!("{{{name}}}");
        if text.contains(&placeholder) {
            text = text.replace(&placeholder, value);
        }
    }
    text
}

/// Interpolate a backend message that already carries `{name}` placeholders.
pub fn interpolate(text: &str, params: &serde_json::Value) -> String {
    let Some(object) = params.as_object() else {
        return text.to_owned();
    };
    let mut result = text.to_owned();
    for (name, value) in object {
        let placeholder = format!("{{{name}}}");
        if result.contains(&placeholder) {
            let replacement = match value {
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            result = result.replace(&placeholder, &replacement);
        }
    }
    result
}

/// Keys present in the catalog; used by the parity tests.
pub fn catalog_keys() -> Vec<String> {
    catalogs()
        .get("en_US")
        .map(|catalog| catalog.keys().cloned().collect())
        .unwrap_or_default()
}

/// `tr!("Capture")` / `tr!("Waiting for {task_name} task to be completed", task_name = name)`
#[macro_export]
macro_rules! tr {
    ($key:expr) => {
        $crate::i18n::t($key)
    };
    ($key:expr, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::tv($key, &[$((stringify!($name), $value)),+])
    };
}
