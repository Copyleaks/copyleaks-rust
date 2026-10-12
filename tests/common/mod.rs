//! Helpers shared by the integration tests.

#![allow(dead_code)]

use copyleaks::{AuthToken, Client, ClientBuilder};
use wiremock::{Match, MockServer, Request};

/// The access token of [`token`]. Fake, used only against the local mock server.
pub const TOKEN: &str = "test-access-token";

/// The `Authorization` header the client must send with [`token`].
pub const BEARER: &str = "Bearer test-access-token";

/// A token that expires far in the future.
pub fn token() -> AuthToken {
    AuthToken::new(TOKEN, "2026-01-01T00:00:00Z", "2999-01-01T00:00:00Z")
}

/// A builder pointed at the mock server, for both the API and the identity server.
pub fn builder(server: &MockServer) -> ClientBuilder {
    Client::builder()
        .api_base_url(server.uri())
        .identity_base_url(server.uri())
}

/// A client pointed at the mock server, without a tenant.
pub fn client(server: &MockServer) -> Client {
    builder(server).build().expect("client")
}

/// The 279-character sample text for AI Text Detection (at least 255 characters are required).
pub const LIONS: &str = "Lions are social animals, living in groups called prides, typically consisting of several females, their offspring, and a few males. Female lions are the primary hunters, working together to catch prey. Lions are known for their strength, teamwork, and complex social structures.";

/// Reads a fixture from `tests/fixtures`.
pub fn fixture(name: &str) -> serde_json::Value {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path).expect("fixture file");
    serde_json::from_str(&text).expect("fixture JSON")
}

/// Matches requests that do not have the header.
pub struct NoHeader(pub &'static str);

impl Match for NoHeader {
    fn matches(&self, request: &Request) -> bool {
        !request.headers.contains_key(self.0)
    }
}
