//! Lenient deserializers for API responses and webhook payloads.
//!
//! The server sometimes sends `null` where a list is expected, numbers where a string is expected,
//! or `null` entries inside a list. These helpers accept those shapes, so one odd field does not
//! fail the whole response.

use serde::{Deserialize, Deserializer};

/// Reads a missing or `null` value as `T::default()`. Use with `#[serde(default)]`.
pub(crate) fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// Reads a list, skipping `null` entries. A missing or `null` list is empty.
pub(crate) fn vec_skip_nulls<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let items = Option::<Vec<Option<T>>>::deserialize(deserializer)?;
    Ok(items.unwrap_or_default().into_iter().flatten().collect())
}

/// Reads a string or a number as an optional string, for example a model version sent as `11`
/// or `"v11.0"`.
pub(crate) fn opt_string_or_number<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrNumber {
        String(String),
        Number(serde_json::Number),
    }

    Ok(
        Option::<StringOrNumber>::deserialize(deserializer)?.map(|value| match value {
            StringOrNumber::String(text) => text,
            StringOrNumber::Number(number) => number.to_string(),
        }),
    )
}
