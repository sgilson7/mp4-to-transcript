//! The reader-visible strings, read from `data/strings.json`. Every key the
//! screens use is a field here, so a missing or misspelt key fails when the
//! file is read, not when a screen happens to need it.

use serde::Deserialize;

/// Every string the page shows. Numbers are `{placeholders}` the core fills.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Strings {
    pub title: String,
    pub intro: String,
    pub privacy: String,
    pub file_label: String,
    pub file_accept: String,
    pub file_chosen: String,
    pub size_mb: String,
    pub speakers_heading: String,
    pub speakers_auto: String,
    pub speakers_note: String,
    pub make: String,
    pub download_note: String,
    pub ids_note: String,
    pub browser_heading: String,
    pub browser_cannot: String,
    pub loading_heading: String,
    pub loading_text: String,
    pub reading_heading: String,
    pub reading_text: String,
    pub voices_heading: String,
    pub voices_text: String,
    pub words_heading: String,
    pub words_text: String,
    pub progress_label: String,
    pub time_left: String,
    pub time_left_unknown: String,
    pub stop: String,
    pub done_heading: String,
    pub done_text: String,
    pub save_vtt: String,
    pub save_txt: String,
    pub again: String,
    pub speaker_label: String,
    pub file_name: String,
    pub default_name: String,
    pub empty_file: String,
    pub no_file: String,
    pub no_speech: String,
    pub decode_failed: String,
    pub models_failed: String,
    pub job_failed: String,
    pub stopped: String,
}

/// Why a strings file cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringsError {
    NotJson(String),
    Empty(String),
}

impl StringsError {
    /// The message the page shows. These two are about the tool's own files,
    /// for whoever maintains it, so they stay beside the rule in English.
    pub fn message(&self) -> String {
        match self {
            StringsError::NotJson(detail) => {
                format!("The strings file could not be read: {detail}")
            }
            StringsError::Empty(field) => format!("The strings file has an empty \"{field}\"."),
        }
    }
}

/// Parses and checks the strings file. A string that is only whitespace counts
/// as empty, because the page would draw nothing.
pub fn parse_strings(json: &str) -> Result<Strings, StringsError> {
    let strings: Strings = serde_json::from_str(json).map_err(|e| StringsError::NotJson(e.to_string()))?;
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| StringsError::NotJson(e.to_string()))?;
    if let Some(map) = value.as_object() {
        for (key, v) in map {
            if v.as_str().is_some_and(|s| s.trim().is_empty()) {
                return Err(StringsError::Empty(key.clone()));
            }
        }
    }
    Ok(strings)
}

/// Replaces each `{name}` in `template` with its value.
pub fn fill(template: &str, values: &[(&str, String)]) -> String {
    values.iter().fold(template.to_string(), |text, (name, value)| {
        text.replace(&format!("{{{name}}}"), value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn real() -> String {
        std::str::from_utf8(include_bytes!("../../../data/strings.json"))
            .unwrap()
            .to_string()
    }

    #[test]
    fn the_strings_file_is_complete() {
        let s = parse_strings(&real()).unwrap();
        assert!(s.speaker_label.contains("{n}"));
        assert!(s.file_name.contains("{name}"));
    }

    #[test]
    fn an_empty_string_is_refused_by_name() {
        let mut v: serde_json::Value = serde_json::from_str(&real()).unwrap();
        v["stop"] = serde_json::Value::String("  ".into());
        let e = parse_strings(&v.to_string()).unwrap_err();
        assert_eq!(e, StringsError::Empty("stop".into()));
        assert_eq!(e.message(), "The strings file has an empty \"stop\".");
    }

    #[test]
    fn an_unknown_or_missing_key_is_refused() {
        let mut v: serde_json::Value = serde_json::from_str(&real()).unwrap();
        v["extra"] = serde_json::Value::String("x".into());
        assert!(matches!(
            parse_strings(&v.to_string()),
            Err(StringsError::NotJson(_))
        ));
        let mut v: serde_json::Value = serde_json::from_str(&real()).unwrap();
        v.as_object_mut().unwrap().remove("stop");
        assert!(matches!(
            parse_strings(&v.to_string()),
            Err(StringsError::NotJson(_))
        ));
    }

    #[test]
    fn placeholders_are_filled_by_name() {
        assert_eq!(
            fill("{a} of {b}", &[("a", "1".into()), ("b", "2".into())]),
            "1 of 2"
        );
        assert_eq!(fill("{a}", &[]), "{a}");
    }
}
