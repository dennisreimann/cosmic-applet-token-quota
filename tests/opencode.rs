use cosmic_applet_token_quota::providers::opencode::{parse_status, QuotaError};
use cosmic_applet_token_quota::quota::WindowKind;

const VALID: &str = r#"{
  "usage": {
    "rolling": { "status": "ok", "percent": 58,  "resetsAt": "2026-09-01T20:00:00+02:00" },
    "weekly":  { "status": "ok", "percent": 85,  "resetsAt": "2026-09-06T00:00:00Z" },
    "monthly": { "status": "ok", "percent": 20,  "resetsAt": "2026-10-01T00:00:00+00:00" }
  }
}"#;

#[test]
fn parses_valid_response() {
    let snap = parse_status(VALID.as_bytes(), 1000).expect("valid");
    assert_eq!(snap.fetched_at_unix, 1000);
    assert_eq!(snap.level, None);
    // API-percent = VERBRAUCHT (AGENTS.md-Vertrag): 58 → 58% used.
    assert_eq!(snap.percent_used_of(WindowKind::FiveHours), Some(58.0));
    assert_eq!(snap.percent_used_of(WindowKind::Weekly), Some(85.0));
    assert_eq!(snap.percent_used_of(WindowKind::Monthly), Some(20.0));
    let expected = chrono::DateTime::parse_from_rfc3339("2026-09-01T20:00:00+02:00")
        .unwrap()
        .timestamp();
    assert_eq!(snap.reset_unix_of(WindowKind::FiveHours), Some(expected));
}

#[test]
fn missing_window_or_usage_is_error() {
    // usage fehlt ganz
    let raw2 = br#"{"weekly": {}}"#;
    assert!(parse_status(raw2, 0).is_err());
    // weekly fehlt innerhalb von usage
    let raw = br#"{"usage": { "rolling": { "status":"ok","percent":10,"resetsAt":"2026-09-01T20:00:00+02:00" } } }"#;
    assert!(parse_status(raw, 0).is_err());
}

#[test]
fn bad_status_is_contract_error() {
    let raw = br#"{"usage": {
        "rolling": { "status":"error" },
        "weekly":  { "status":"ok","percent":10,"resetsAt":"2026-09-01T20:00:00+02:00" },
        "monthly": { "status":"ok","percent":10,"resetsAt":"2026-09-01T20:00:00+02:00" }
    } }"#;
    assert!(matches!(parse_status(raw, 0), Err(QuotaError::Contract(_))));
}

#[test]
fn rate_limited_window_is_valid_with_zero_remaining() {
    let raw = br#"{"usage": {
        "rolling": { "status":"ok","percent":0,"resetsAt":"2026-10-23T20:10:41.862Z" },
        "weekly":  { "status":"rate-limited","percent":100,"resetsAt":"2026-10-26T00:00:00.000Z" },
        "monthly": { "status":"ok","percent":86,"resetsAt":"2026-10-30T11:50:44.000Z" }
    } }"#;
    let snap = parse_status(raw, 0).expect("rate-limited window must parse");
    // rate-limited = 100% verbraucht; rolling mit percent 0 = unbenutzt;
    // monthly percent 86 = 86% verbraucht (API-percent ist CONSUMED).
    assert_eq!(snap.percent_used_of(WindowKind::Weekly), Some(100.0));
    assert_eq!(snap.percent_used_of(WindowKind::FiveHours), Some(0.0));
    assert_eq!(snap.percent_used_of(WindowKind::Monthly), Some(86.0));
}

#[test]
fn rate_limited_window_without_percent_defaults_to_zero_remaining() {
    let raw = br#"{"usage": {
        "rolling": { "status":"ok","percent":0,"resetsAt":"2026-10-23T20:10:41.862Z" },
        "weekly":  { "status":"rate-limited","resetsAt":"2026-10-26T00:00:00.000Z" },
        "monthly": { "status":"ok","percent":86,"resetsAt":"2026-10-30T11:50:44.000Z" }
    } }"#;
    let snap = parse_status(raw, 0).expect("rate-limited without percent must parse");
    assert_eq!(snap.percent_used_of(WindowKind::Weekly), Some(100.0));
}

#[test]
fn percent_out_of_range_is_contract_error() {
    let raw = br#"{"usage": {
        "rolling": { "status":"ok","percent":150,"resetsAt":"2026-09-01T20:00:00+02:00" },
        "weekly":  { "status":"ok","percent":10,"resetsAt":"2026-09-01T20:00:00+02:00" },
        "monthly": { "status":"ok","percent":10,"resetsAt":"2026-09-01T20:00:00+02:00" }
    } }"#;
    assert!(matches!(parse_status(raw, 0), Err(QuotaError::Contract(_))));
}

#[test]
fn invalid_json_is_json_error() {
    assert!(matches!(
        parse_status(b"not json", 0),
        Err(QuotaError::Json(_))
    ));
}

// === Portiert aus tests/api.rs (Task 2): HTTP-Verhalten an fetch_quota_at ===
use cosmic_applet_token_quota::providers::opencode::fetch_quota_at;
use httpmock::prelude::*;

const VALID_BODY: &str = r#"{
  "usage": {
    "rolling": { "status": "ok", "percent": 58,  "resetsAt": "2026-09-01T20:00:00+02:00" },
    "weekly":  { "status": "ok", "percent": 85,  "resetsAt": "2026-09-06T00:00:00Z" },
    "monthly": { "status": "ok", "percent": 20,  "resetsAt": "2026-10-01T00:00:00+00:00" }
  }
}"#;

#[tokio::test]
async fn sends_auth_header_and_parses() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/usage")
            .header("authorization", "Bearer test-key");
        then.status(200).body(VALID_BODY);
    });
    let client = reqwest::Client::new();
    let url = format!("{}/usage", server.base_url());
    let snap = fetch_quota_at(&client, "test-key", &url).await.expect("ok");
    assert_eq!(snap.percent_used_of(WindowKind::FiveHours), Some(58.0));
    mock.assert_hits(1);
}

#[tokio::test]
async fn handles_401_without_retry() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/usage");
        then.status(401).body("unauthorized");
    });
    let client = reqwest::Client::new();
    let url = format!("{}/usage", server.base_url());
    let err = fetch_quota_at(&client, "bad", &url).await.unwrap_err();
    assert!(!err.retryable);
    assert!(err.message.contains("401"));
    mock.assert_hits(1);
}

#[tokio::test]
async fn retries_once_on_500() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/usage");
        then.status(500).body("boom");
    });
    let client = reqwest::Client::new();
    let url = format!("{}/usage", server.base_url());
    let err = fetch_quota_at(&client, "k", &url).await.unwrap_err();
    assert!(err.retryable);
    // 1 Versuch + 1 Retry = 2 Requests
    mock.assert_hits(2);
}
