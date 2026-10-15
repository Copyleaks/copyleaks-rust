//! Opt-in retries of calls that are safe to repeat.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{Duration, SystemTime};

use reqwest::header::{HeaderMap, RETRY_AFTER};

/// Retries of calls that fail with HTTP 429, HTTP 503 or a connection error, set with
/// [`ClientBuilder::retry_policy`](crate::ClientBuilder::retry_policy). Without it, nothing is
/// retried.
///
/// Only the `GET` calls and the `PUT` submits (Plagiarism Checker, AI Image Detection by URL) are
/// retried; a submit whose earlier attempt reached the server fails with HTTP 409. `POST` and
/// `PATCH` calls (credit-spending checks, AI Video Detection, login, export, start, delete,
/// resend webhook) are never retried. The wait is the server's `Retry-After`, else a doubling
/// backoff with jitter.
///
/// ```
/// use std::time::Duration;
/// use copyleaks::{Client, RetryPolicy};
///
/// # fn run() -> Result<(), copyleaks::Error> {
/// let client = Client::builder()
///     .retry_policy(RetryPolicy::new(3).initial_backoff(Duration::from_secs(2)))
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RetryPolicy {
    /// How many times a call is retried after the first attempt. Default: 3.
    pub max_retries: u32,
    /// The wait before the first retry without `Retry-After`. Default: 1 second.
    pub initial_backoff: Duration,
    /// The longest wait between two attempts without `Retry-After`. Default: 30 seconds.
    pub max_backoff: Duration,
    /// The longest `Retry-After` the client waits for; a longer one fails the call at once, with
    /// the wait in the error's `retry_after`. Default: 60 seconds.
    pub max_retry_after: Duration,
}

impl RetryPolicy {
    /// A policy that retries up to `max_retries` times, with the default waits.
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
            max_retry_after: Duration::from_secs(60),
        }
    }

    /// Sets [`initial_backoff`](field@RetryPolicy::initial_backoff).
    #[must_use]
    pub fn initial_backoff(mut self, wait: Duration) -> Self {
        self.initial_backoff = wait;
        self
    }

    /// Sets [`max_backoff`](field@RetryPolicy::max_backoff).
    #[must_use]
    pub fn max_backoff(mut self, wait: Duration) -> Self {
        self.max_backoff = wait;
        self
    }

    /// Sets [`max_retry_after`](field@RetryPolicy::max_retry_after).
    #[must_use]
    pub fn max_retry_after(mut self, wait: Duration) -> Self {
        self.max_retry_after = wait;
        self
    }

    /// The wait before retry number `retry` (0 for the first retry) without `Retry-After`:
    /// the doubled backoff, capped, with jitter that keeps between half and all of it.
    pub(crate) fn backoff(&self, retry: u32) -> Duration {
        let doubled = self
            .initial_backoff
            .saturating_mul(2_u32.saturating_pow(retry.min(31)));
        let capped = doubled.min(self.max_backoff);
        let half = capped / 2;
        let nanos = u64::try_from(half.as_nanos()).unwrap_or(u64::MAX);
        let jitter = if nanos == 0 {
            0
        } else {
            random() % nanos.saturating_add(1)
        };
        half + Duration::from_nanos(jitter)
    }
}

impl Default for RetryPolicy {
    /// The same as `RetryPolicy::new(3)`.
    fn default() -> Self {
        Self::new(3)
    }
}

/// A random number from the standard library's randomly seeded hasher.
fn random() -> u64 {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u128(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos()),
    );
    hasher.finish()
}

/// Reads the `Retry-After` header: a number of seconds, or an HTTP date. A date in the past is a
/// wait of zero.
pub(crate) fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let date = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    let at = u64::try_from(date.timestamp()).ok()?;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(Duration::from_secs(at.saturating_sub(now)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    fn headers(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_str(value).expect("header"));
        headers
    }

    #[test]
    fn retry_after_seconds_and_dates() {
        assert_eq!(retry_after(&headers("7")), Some(Duration::from_secs(7)));
        assert_eq!(retry_after(&headers(" 0 ")), Some(Duration::ZERO));
        assert_eq!(
            retry_after(&headers("Sun, 06 Nov 1994 08:49:37 GMT")),
            Some(Duration::ZERO),
            "a past date is no wait"
        );
        let wait = retry_after(&headers("Fri, 01 Jan 2100 00:00:00 GMT")).expect("future date");
        assert!(wait > Duration::from_secs(365 * 24 * 3600));
        assert_eq!(retry_after(&headers("soon")), None);
        assert_eq!(retry_after(&headers("-5")), None);
        assert_eq!(retry_after(&HeaderMap::new()), None);
    }

    #[test]
    fn backoff_doubles_with_jitter_and_is_capped() {
        let policy = RetryPolicy::new(5)
            .initial_backoff(Duration::from_millis(100))
            .max_backoff(Duration::from_millis(350));
        for _ in 0..50 {
            let first = policy.backoff(0);
            assert!((50..=100).contains(&first.as_millis()), "{first:?}");
            let second = policy.backoff(1);
            assert!((100..=200).contains(&second.as_millis()), "{second:?}");
            let capped = policy.backoff(10);
            assert!((175..=350).contains(&capped.as_millis()), "{capped:?}");
        }
        assert_eq!(RetryPolicy::new(1).max_retries, 1);
        assert_eq!(RetryPolicy::default(), RetryPolicy::new(3));
        let zero = RetryPolicy::new(1).initial_backoff(Duration::ZERO);
        assert_eq!(zero.backoff(0), Duration::ZERO);
        assert!(RetryPolicy::new(1).backoff(u32::MAX) <= Duration::from_secs(30));
    }
}
