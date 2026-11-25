use cosmic_applet_token_quota::api::{fetch_bytes_at, AuthStyle};
use httpmock::prelude::*;

const VALID_BODY: &str = r#"{ "usage": {
    "rolling": { "status": "ok", "percent": 58,  "resetsAt": "2026-09-01T20:00:00+02:00" },
    "weekly":  { "status": "ok", "percent": 85,  "resetsAt": "2026-09-06T00:00:00Z" },
    "monthly": { "status": "ok", "percent": 20,  "resetsAt": "2026-10-01T00:00:00+00:00" }
} }"#;

#[tokio::test]
async fn bearer_style_sends_prefixed_header() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/b")
            .header("authorization", "Bearer k1"); // exakt, inkl. Prefix
        then.status(200).body(VALID_BODY);
    });
    let client = reqwest::Client::new();
    let url = format!("{}/b", server.base_url());
    fetch_bytes_at(&client, &url, "k1", AuthStyle::Bearer)
        .await
        .expect("ok");
    mock.assert_hits(1);
}

#[tokio::test]
async fn plain_style_sends_raw_header_without_bearer() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/p").header("authorization", "k2"); // exakt — ein zweiter "Bearer" darf nicht vorkommen
        then.status(200).body(VALID_BODY);
    });
    let client = reqwest::Client::new();
    let url = format!("{}/p", server.base_url());
    fetch_bytes_at(&client, &url, "k2", AuthStyle::Plain)
        .await
        .expect("ok");
    mock.assert_hits(1);
}

#[tokio::test]
async fn no_retry_on_401() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/x");
        then.status(401).body("unauthorized");
    });
    let client = reqwest::Client::new();
    let url = format!("{}/x", server.base_url());
    let err = fetch_bytes_at(&client, &url, "k", AuthStyle::Bearer)
        .await
        .unwrap_err();
    assert!(!err.retryable);
    assert!(err.message.contains("401"));
    mock.assert_hits(1);
}

#[test]
fn retry_decision_matrix() {
    use cosmic_applet_token_quota::api::is_retryable;
    use reqwest::StatusCode;
    assert!(is_retryable(StatusCode::REQUEST_TIMEOUT)); // 408
    assert!(is_retryable(StatusCode::TOO_EARLY)); // 425
    assert!(is_retryable(StatusCode::TOO_MANY_REQUESTS)); // 429
    assert!(is_retryable(StatusCode::BAD_GATEWAY)); // 502
    assert!(is_retryable(StatusCode::SERVICE_UNAVAILABLE)); // 503
    assert!(!is_retryable(StatusCode::UNAUTHORIZED)); // 401
    assert!(!is_retryable(StatusCode::FORBIDDEN)); // 403
    assert!(!is_retryable(StatusCode::NOT_FOUND)); // 404
    assert!(!is_retryable(StatusCode::OK));
}

#[test]
fn auth_header_value_builds_both_styles() {
    use cosmic_applet_token_quota::api::auth_header_value;
    assert_eq!(auth_header_value(AuthStyle::Bearer, "k"), "Bearer k");
    assert_eq!(auth_header_value(AuthStyle::Plain, "k"), "k");
}
