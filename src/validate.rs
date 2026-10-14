//! Cheap client-side checks that match the server's rules, so a bad request fails fast
//! with [`Error::Validation`] instead of a round trip.
//!
//! Lengths are counted in UTF-16 code units, as the server (.NET `string.Length`) counts them.

use crate::{Error, Result};

/// The symbols a Plagiarism Checker submit scan id may contain, besides `a-z` and `0-9`.
const SUBMIT_SCAN_ID_SYMBOLS: &str = "!@$&=_()';:.,~-";

/// The symbols an export's scan id and export id may contain, besides `a-z` and `0-9`. The
/// Image Plagiarism Detector, AI Image Detection by URL and AI Video Detection accept the same.
const FULL_ID_SYMBOLS: &str = "!@$^&+%=_(){}<>'\";:/.,~`|\\-";

/// Checks the 3 to 36 character length that every endpoint enforces on scan and export ids.
/// The product endpoints (AI Text Detection, AI Image Detection, Grammar Checker and Text
/// Moderation) check only this.
pub(crate) fn id_length(id: &str, name: &str) -> Result<()> {
    length(id, name, 3, 36)
}

/// Checks a Plagiarism Checker submit scan id: 3 to 36 characters from `a-z`, `0-9` and
/// `!@$&=_()';:.,~-`, without `g`, one or more `o` or `0`, then `gle`.
pub(crate) fn submit_scan_id(id: &str) -> Result<()> {
    charset_id(id, "scan id", SUBMIT_SCAN_ID_SYMBOLS)
}

/// Checks an export's scan id or export id: 3 to 36 characters from `a-z`, `0-9` and
/// ``!@$^&+%=_(){}<>'";:/.,~`|\-``, without `g`, one or more `o` or `0`, then `gle`.
pub(crate) fn export_id(id: &str, name: &str) -> Result<()> {
    charset_id(id, name, FULL_ID_SYMBOLS)
}

/// Checks the scan id of the Image Plagiarism Detector, AI Image Detection by URL and AI Video
/// Detection: the same rules as [`export_id`].
pub(crate) fn scan_id(id: &str) -> Result<()> {
    charset_id(id, "scan id", FULL_ID_SYMBOLS)
}

/// Checks a course, assignment or institution id: at most 64 characters from `a-z`, `0-9` and
/// `!@$&=_()';:.,~-`, without `g`, one or more `o` or `0`, then `gle`. An empty id is accepted.
pub(crate) fn entity_id(id: &str, name: &str) -> Result<()> {
    if id.is_empty() {
        return Ok(());
    }
    length(id, name, 1, 64)?;
    charset(id, name, SUBMIT_SCAN_ID_SYMBOLS)
}

fn charset_id(id: &str, name: &str, symbols: &str) -> Result<()> {
    id_length(id, name)?;
    charset(id, name, symbols)
}

/// Checks that `id` has only `a-z`, `0-9` and `symbols`, and is not a reserved word.
fn charset(id: &str, name: &str, symbols: &str) -> Result<()> {
    let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || symbols.contains(c);
    if let Some(bad) = id.chars().find(|&c| !allowed(c)) {
        return Err(Error::Validation(format!(
            "{name} contains the unsupported character {bad:?}; allowed are a-z, 0-9 and {symbols}"
        )));
    }
    if is_reserved(id) {
        return Err(Error::Validation(format!(
            "{name} must not contain g, then one or more o or 0, then gle (for example google)"
        )));
    }
    Ok(())
}

/// True when `id` contains `g`, then one or more `o` or `0`, then `gle`.
fn is_reserved(id: &str) -> bool {
    let bytes = id.as_bytes();
    (0..bytes.len()).any(|start| {
        if bytes[start] != b'g' {
            return false;
        }
        let mut end = start + 1;
        while end < bytes.len() && matches!(bytes[end], b'o' | b'0') {
            end += 1;
        }
        end > start + 1 && bytes[end..].starts_with(b"gle")
    })
}

/// Checks that a required value is not empty.
pub(crate) fn required(value: &str, name: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::Validation(format!("{name} is required")));
    }
    Ok(())
}

/// Checks that a value has `min..=max` characters, counted in UTF-16 code units.
pub(crate) fn length(value: &str, name: &str, min: usize, max: usize) -> Result<()> {
    let len = value.encode_utf16().count();
    if !(min..=max).contains(&len) {
        return Err(Error::Validation(format!(
            "{name} must be {min} to {max} characters long, got {len}"
        )));
    }
    Ok(())
}

/// Checks that a number is in `min..=max`.
pub(crate) fn range(value: u32, name: &str, min: u32, max: u32) -> Result<()> {
    if !(min..=max).contains(&value) {
        return Err(Error::Validation(format!(
            "{name} must be between {min} and {max}, got {value}"
        )));
    }
    Ok(())
}

/// Checks that a value is one of `allowed`, compared case-sensitively as the server compares it.
pub(crate) fn one_of(value: &str, name: &str, allowed: &[&str]) -> Result<()> {
    if !allowed.contains(&value) {
        return Err(Error::Validation(format!(
            "{name} must be one of {}, got {value:?}",
            allowed.join(", ")
        )));
    }
    Ok(())
}

/// Checks that a list has `min..=max` items.
pub(crate) fn count(len: usize, name: &str, min: usize, max: usize) -> Result<()> {
    if !(min..=max).contains(&len) {
        return Err(Error::Validation(format!(
            "{name} must have {min} to {max} items, got {len}"
        )));
    }
    Ok(())
}

/// Checks that a weight is in `0.0..=1.0`.
pub(crate) fn weight(value: f64, name: &str) -> Result<()> {
    if !(0.0..=1.0).contains(&value) {
        return Err(Error::Validation(format!(
            "{name} must be between 0.0 and 1.0, got {value}"
        )));
    }
    Ok(())
}

/// Checks that a list has at most `max` items, each `min_len..=max_len` characters long.
pub(crate) fn string_list(
    items: &[String],
    name: &str,
    max: usize,
    min_len: usize,
    max_len: usize,
) -> Result<()> {
    count(items.len(), name, 0, max)?;
    for item in items {
        length(item, &format!("{name}[]"), min_len, max_len)?;
    }
    Ok(())
}

/// Checks an HTTP method the server accepts for fetching a URL: `GET`, `POST` or `PUT`, in any
/// case.
pub(crate) fn verb(value: &str, name: &str) -> Result<()> {
    if !["GET", "POST", "PUT"]
        .iter()
        .any(|verb| value.eq_ignore_ascii_case(verb))
    {
        return Err(Error::Validation(format!(
            "{name} must be GET, POST or PUT, got {value:?}"
        )));
    }
    Ok(())
}

/// Checks that a file name ends with one of `extensions` (lowercase, with the dot), ignoring case.
pub(crate) fn extension(filename: &str, extensions: &[&str]) -> Result<()> {
    let name = filename.to_ascii_lowercase();
    if !extensions.iter().any(|extension| name.ends_with(extension)) {
        return Err(Error::Validation(format!(
            "filename must end with one of {}",
            extensions.join(", ")
        )));
    }
    Ok(())
}

/// Checks a `dd-MM-yyyy` date, the format of the usage history endpoint.
pub(crate) fn date(value: &str, name: &str) -> Result<()> {
    let bytes = value.as_bytes();
    let valid = bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, b)| {
            if i == 2 || i == 5 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    if !valid {
        return Err(Error::Validation(format!(
            "{name} must be a dd-MM-yyyy date, got {value:?}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_scan_id_rules() {
        assert!(submit_scan_id("abc").is_ok());
        assert!(submit_scan_id("my-scan_1!@$&=()';:.,~").is_ok());
        assert!(submit_scan_id(&"a".repeat(36)).is_ok());
        assert!(submit_scan_id("ab").is_err());
        assert!(submit_scan_id(&"a".repeat(37)).is_err());
        assert!(submit_scan_id("Abc").is_err());
        assert!(submit_scan_id("a b c").is_err());
        assert!(submit_scan_id("abc/def").is_err());
        assert!(submit_scan_id("abc^def").is_err());
    }

    #[test]
    fn export_id_rules() {
        assert!(export_id("abc/def^{x}|`\\%+<>\"", "export id").is_ok());
        assert!(export_id("Abc", "export id").is_err());
        assert!(export_id("a b", "export id").is_err());
        assert!(export_id("ab", "export id").is_err());
        assert!(export_id("my-google-export", "export id").is_err());
    }

    #[test]
    fn product_ids_check_only_the_length() {
        assert!(id_length("Scan 1 #ä", "scan id").is_ok());
        assert!(id_length("google", "scan id").is_ok());
        assert!(id_length("ab", "scan id").is_err());
        assert!(id_length(&"a".repeat(37), "scan id").is_err());
        // An emoji is 2 UTF-16 code units.
        assert!(id_length("😀a", "scan id").is_ok());
        assert!(id_length(&"😀".repeat(18), "scan id").is_ok());
        assert!(id_length(&format!("{}a", "😀".repeat(18)), "scan id").is_err());
    }

    #[test]
    fn reserved_word() {
        for id in [
            "google",
            "gogle",
            "g00gle",
            "go0ogle",
            "my-google-scan",
            "xggoogle",
        ] {
            assert!(is_reserved(id), "{id} should be reserved");
            assert!(
                submit_scan_id(id).is_err(),
                "{id} should be rejected for plagiarism"
            );
            assert!(
                id_length(id, "scan id").is_ok(),
                "{id} is fine for other products"
            );
        }
        for id in ["ggle", "goggle", "gooble", "goo-gle", "gle", "go", "g0"] {
            assert!(!is_reserved(id), "{id} should not be reserved");
        }
    }

    #[test]
    fn lengths_count_utf16_code_units() {
        assert!(length("😀", "text", 2, 2).is_ok());
        assert!(length("é", "text", 1, 1).is_ok());
        assert!(length("😀", "text", 1, 1).is_err());
    }

    #[test]
    fn one_of_is_case_sensitive() {
        assert!(one_of("v1", "reportVersion", &["v1", "latest"]).is_ok());
        assert!(one_of("V1", "reportVersion", &["v1", "latest"]).is_err());
        assert!(one_of("", "reportVersion", &["v1", "latest"]).is_err());
        assert!(one_of(" v1", "reportVersion", &["v1", "latest"]).is_err());
    }

    #[test]
    fn entity_ids() {
        assert!(entity_id("", "course.id").is_ok());
        assert!(entity_id("cs-101", "course.id").is_ok());
        assert!(entity_id(&"a".repeat(64), "course.id").is_ok());
        assert!(entity_id(&"a".repeat(65), "course.id").is_err());
        assert!(entity_id("CS-101", "course.id").is_err());
        assert!(entity_id("my-google-course", "course.id").is_err());
    }

    #[test]
    fn verbs_ignore_case() {
        for value in ["GET", "post", "Put"] {
            assert!(verb(value, "verb").is_ok(), "{value}");
        }
        for value in ["", "DELETE", "PATCH", "GET "] {
            assert!(verb(value, "verb").is_err(), "{value}");
        }
    }

    #[test]
    fn string_lists() {
        let items = |n: usize, len: usize| vec!["a".repeat(len); n];
        assert!(string_list(&items(10, 256), "domains", 10, 1, 256).is_ok());
        assert!(string_list(&items(11, 1), "domains", 10, 1, 256).is_err());
        assert!(string_list(&items(1, 257), "domains", 10, 1, 256).is_err());
        assert!(string_list(&items(1, 0), "domains", 10, 1, 256).is_err());
        assert!(string_list(&[], "domains", 10, 1, 256).is_ok());
    }

    #[test]
    fn dates() {
        assert!(date("01-01-2020", "start").is_ok());
        assert!(date("1-1-2020", "start").is_err());
        assert!(date("2020-01-01", "start").is_err());
        assert!(date("01/01/2020", "start").is_err());
    }
}
