//! The login token.

use std::fmt;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// A token is treated as expired this many seconds before its `.expires` time,
/// so that a call does not start with a token that runs out on the way.
const EXPIRY_MARGIN_SECS: i64 = 5 * 60;

/// A login token, returned by [`Client::login`](crate::Client::login). It is valid for 48 hours.
///
/// Treat it as a password; its `Debug` output hides the access token. It serializes to the login
/// endpoint's JSON, so you can cache it.
#[derive(Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AuthToken {
    /// The bearer token sent in the `Authorization` header.
    pub access_token: String,
    /// When the token was issued (`.issued`), as an ISO 8601 time.
    #[serde(rename = ".issued", default)]
    pub issued: String,
    /// When the token expires (`.expires`), as an ISO 8601 time.
    #[serde(rename = ".expires")]
    pub expires: String,
    /// `.expires` as Unix seconds, with the text it was parsed from, so that the check before
    /// each call does not parse the time again.
    #[serde(skip)]
    parsed_expiry: OnceLock<(String, Option<i64>)>,
}

impl AuthToken {
    /// Creates a token from its parts, for example from a token you cached after an earlier login.
    pub fn new(
        access_token: impl Into<String>,
        issued: impl Into<String>,
        expires: impl Into<String>,
    ) -> Self {
        Self {
            access_token: access_token.into(),
            issued: issued.into(),
            expires: expires.into(),
            parsed_expiry: OnceLock::new(),
        }
    }

    /// Checks locally, without a network call, that the token has an access token and does not
    /// expire within 5 minutes. Every authenticated call runs this check first.
    ///
    /// # Errors
    ///
    /// [`Error::TokenExpired`] when the token has expired, expires within 5 minutes, or has no `.expires`;
    /// [`Error::Validation`] when the access token is empty or `.expires` is not ISO 8601.
    pub fn verify(&self) -> Result<()> {
        if self.access_token.is_empty() {
            return Err(Error::Validation(
                "the login token has no access_token".to_owned(),
            ));
        }
        if self.expires.trim().is_empty() {
            return Err(Error::TokenExpired);
        }
        let expires = self.expiry().ok_or_else(|| {
            Error::Validation(format!(
                "the login token's .expires is not an ISO 8601 time: {:?}",
                self.expires
            ))
        })?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
            });
        if expires <= now.saturating_add(EXPIRY_MARGIN_SECS) {
            return Err(Error::TokenExpired);
        }
        Ok(())
    }
}

impl AuthToken {
    /// `.expires` as Unix seconds. Parsed once; parsed again only when `expires` was changed.
    fn expiry(&self) -> Option<i64> {
        if let Some((text, parsed)) = self.parsed_expiry.get() {
            if *text == self.expires {
                return *parsed;
            }
            return parse_timestamp(&self.expires);
        }
        let parsed = parse_timestamp(&self.expires);
        let _ = self.parsed_expiry.set((self.expires.clone(), parsed));
        parsed
    }
}

impl PartialEq for AuthToken {
    fn eq(&self, other: &Self) -> bool {
        self.access_token == other.access_token
            && self.issued == other.issued
            && self.expires == other.expires
    }
}

impl Eq for AuthToken {}

impl fmt::Debug for AuthToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthToken")
            .field("access_token", &"<redacted>")
            .field("issued", &self.issued)
            .field("expires", &self.expires)
            .finish()
    }
}

/// Parses an ISO 8601 time to Unix seconds. A time without an offset is read as UTC.
fn parse_timestamp(value: &str) -> Option<i64> {
    if let Ok(time) = chrono::DateTime::parse_from_rfc3339(value) {
        return Some(time.timestamp());
    }
    chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
        .ok()
        .map(|time| time.and_utc().timestamp())
}

#[cfg(test)]
mod tests {
    use super::{parse_timestamp, AuthToken};

    #[test]
    fn expiry_is_parsed_once_and_again_after_a_change() {
        let mut token = AuthToken::new("t", "", "2999-01-01T00:00:00Z");
        assert!(token.verify().is_ok());
        assert!(token.parsed_expiry.get().is_some(), "cached");
        assert!(token.verify().is_ok());
        token.expires = "2020-01-01T00:00:00Z".to_owned();
        assert!(matches!(token.verify(), Err(crate::Error::TokenExpired)));
        assert_eq!(token, AuthToken::new("t", "", "2020-01-01T00:00:00Z"));
    }

    #[test]
    fn parses_the_login_response_format() {
        // 7 fractional digits and an offset, as in the login docs.
        assert_eq!(
            parse_timestamp("2018-11-26T16:15:38.2431255+02:00"),
            Some(1_543_241_738)
        );
        assert_eq!(parse_timestamp("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_timestamp("1970-01-01T00:01:00"), Some(60));
        assert_eq!(parse_timestamp("1970-01-01T00:01:00.5"), Some(60));
        assert_eq!(parse_timestamp("not a time"), None);
        assert_eq!(parse_timestamp(""), None);
    }
}
