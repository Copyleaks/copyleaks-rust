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

/// A client pointed at the mock server.
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

/// A part of a `multipart/form-data` body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormPart {
    /// The field name.
    pub name: String,
    /// The file name of a file part.
    pub filename: Option<String>,
    /// The content type of a file part.
    pub content_type: Option<String>,
    /// The content.
    pub data: Vec<u8>,
}

impl FormPart {
    /// A text field.
    pub fn text(name: &str, value: &str) -> Self {
        Self {
            name: name.to_owned(),
            filename: None,
            content_type: None,
            data: value.as_bytes().to_vec(),
        }
    }

    /// A file field.
    pub fn file(name: &str, filename: &str, content_type: &str, data: &[u8]) -> Self {
        Self {
            name: name.to_owned(),
            filename: Some(filename.to_owned()),
            content_type: Some(content_type.to_owned()),
            data: data.to_vec(),
        }
    }
}

/// Parses the `multipart/form-data` body of a request, checking the framing.
pub fn multipart(request: &Request) -> Vec<FormPart> {
    let content_type = request
        .headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .expect("content-type header");
    let boundary = content_type
        .strip_prefix("multipart/form-data; boundary=")
        .unwrap_or_else(|| panic!("not a multipart content type: {content_type}"));
    let delimiter = format!("--{boundary}").into_bytes();
    let body = &request.body;
    let starts: Vec<usize> = (0..body.len())
        .filter(|&i| body[i..].starts_with(&delimiter))
        .collect();
    assert!(starts.len() >= 2, "no parts");
    let last = starts[starts.len() - 1];
    assert_eq!(
        &body[last + delimiter.len()..],
        b"--\r\n",
        "closing delimiter"
    );
    starts
        .windows(2)
        .map(|pair| {
            let section = &body[pair[0] + delimiter.len()..pair[1]];
            let section = section.strip_prefix(b"\r\n").expect("CRLF after delimiter");
            let section = section
                .strip_suffix(b"\r\n")
                .expect("CRLF before delimiter");
            let split = (0..section.len())
                .find(|&i| section[i..].starts_with(b"\r\n\r\n"))
                .expect("part headers");
            let head = std::str::from_utf8(&section[..split]).expect("UTF-8 headers");
            let mut name = None;
            let mut filename = None;
            let mut content_type = None;
            for line in head.split("\r\n") {
                let (key, value) = line.split_once(": ").expect("header line");
                match key.to_ascii_lowercase().as_str() {
                    "content-disposition" => {
                        for item in value.split("; ").skip(1) {
                            let (key, value) = item.split_once('=').expect("attribute");
                            let value = value.trim_matches('"').to_owned();
                            match key {
                                "name" => name = Some(value),
                                "filename" => filename = Some(value),
                                other => panic!("unexpected attribute {other}"),
                            }
                        }
                    }
                    "content-type" => content_type = Some(value.to_owned()),
                    other => panic!("unexpected part header {other}"),
                }
            }
            FormPart {
                name: name.expect("part name"),
                filename,
                content_type,
                data: section[split + 4..].to_vec(),
            }
        })
        .collect()
}

/// Compresses `data` with gzip.
pub fn gzip(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).expect("gzip");
    encoder.finish().expect("gzip")
}

/// The only request the server received.
pub async fn only_request(server: &MockServer) -> Request {
    let mut requests = server.received_requests().await.expect("recording is on");
    assert_eq!(requests.len(), 1, "exactly one request");
    requests.remove(0)
}
