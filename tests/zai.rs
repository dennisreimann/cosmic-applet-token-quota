use cosmic_applet_token_quota::providers::zai::{fetch_quota, fetch_quota_at, parse_quota};
use cosmic_applet_token_quota::quota::WindowKind;
use httpmock::prelude::*;

const FULL: &str = r#"{
  "code": 200,
  "msg": "Success",
  "success": true,
  "data": {
    "level": "lite",
    "limits": [
      { "type": "TIME_LIMIT", "unit": 5, "number": 1, "usage": 100,
        "currentValue": 28, "remaining": 72, "percentage": 28,
        "nextResetTime": 1772615765983,
        "usageDetails": [ { "modelCode": "search-prime", "usage": 67 } ] },
      { "type": "TOKENS_LIMIT", "unit": 3, "number": 5,
        "percentage": 1, "nextResetTime": 1771073738808 },
      { "type": "TOKENS_LIMIT", "unit": 6, "usage": 500000,
        "currentValue": 120000, "percentage": 24, "nextResetTime": 1744137600000 }
    ]
  }
}"#;

const AUTH_FAIL: &str = r#"{ "code": 1000, "msg": "身份验证失败。", "success": false }"#;

#[test]
fn parses_full_quota_response() {
    let snap = parse_quota(FULL.as_bytes(), 1000).expect("valid");
    assert_eq!(snap.fetched_at_unix, 1000);
    assert_eq!(snap.level.as_deref(), Some("lite"));
    let kinds: Vec<_> = snap.windows.iter().map(|w| w.kind).collect();
    assert_eq!(
        kinds,
        [
            WindowKind::FiveHours,
            WindowKind::Weekly,
            WindowKind::McpMonthly
        ]
    );
    let five = &snap.windows[0];
    assert_eq!(five.percent_used, 1.0);
    assert_eq!(five.reset_unix, 1771073738); // ms → s
    assert_eq!(snap.windows[1].percent_used, 24.0);
    assert_eq!(snap.windows[1].reset_unix, 1744137600);
    assert_eq!(snap.windows[2].percent_used, 28.0);
    assert_eq!(snap.windows[2].reset_unix, 1772615765); // 1772615765983 ms
}

#[test]
fn legacy_plan_without_weekly_window() {
    let raw = r#"{
        "code": 200, "success": true,
        "data": { "level": "lite", "limits": [
            { "type": "TOKENS_LIMIT", "unit": 3, "number": 5,
              "percentage": 5, "nextResetTime": 1771073738808 },
            { "type": "TIME_LIMIT", "unit": 5, "number": 1,
              "percentage": 12, "nextResetTime": 1772615765983 }
        ] }
    }"#;
    let snap = parse_quota(raw.as_bytes(), 7).expect("legacy plans lack weekly — kein Fehler");
    let kinds: Vec<_> = snap.windows.iter().map(|w| w.kind).collect();
    assert_eq!(kinds, [WindowKind::FiveHours, WindowKind::McpMonthly]);
}

#[test]
fn unknown_window_types_are_skipped() {
    let raw = r#"{
        "code": 200, "success": true,
        "data": { "level": null, "limits": [
            { "type": "SOMETHING_NEW", "unit": 9, "percentage": 50 },
            { "type": "TOKENS_LIMIT", "unit": 3, "number": 5,
              "percentage": 2, "nextResetTime": 1000 }
        ] }
    }"#;
    let snap = parse_quota(raw.as_bytes(), 0).expect("unbekannte Fenster ignorieren");
    assert_eq!(snap.windows.len(), 1);
    assert_eq!(snap.windows[0].kind, WindowKind::FiveHours);
    assert_eq!(snap.level, None);
}

#[test]
fn recognized_window_without_percentage_is_contract_error() {
    let raw = r#"{
        "code": 200, "success": true,
        "data": { "level": null, "limits": [
            { "type": "TOKENS_LIMIT", "unit": 3, "number": 5,
              "nextResetTime": 1000 }
        ] }
    }"#;
    assert!(parse_quota(raw.as_bytes(), 0).is_err());
}

#[test]
fn success_false_is_contract_error() {
    assert!(parse_quota(AUTH_FAIL.as_bytes(), 0).is_err());
}

#[tokio::test]
async fn plain_auth_header_no_bearer() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/q").header("authorization", "zkey"); // exakt — kein "Bearer " Prefix
        then.status(200).body(FULL);
    });
    let client = reqwest::Client::new();
    let url = format!("{}/q", server.base_url());
    let snap = fetch_quota_at(&client, "zkey", &url).await.expect("ok");
    assert_eq!(snap.level.as_deref(), Some("lite"));
    mock.assert_hits(1);
    // fetch_quota nutzt dieselbe Logik mit QUOTA_URL (ohne Netz-Call geprüft).
    let _ = fetch_quota;
}

#[tokio::test]
async fn auth_failure_is_api_error() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/q");
        then.status(200).body(AUTH_FAIL); // HTTP 200, aber success:false
    });
    let client = reqwest::Client::new();
    let url = format!("{}/q", server.base_url());
    let err = fetch_quota_at(&client, "bad", &url).await.unwrap_err();
    assert!(!err.retryable);
    assert!(err.message.contains("1000"));
    mock.assert_hits(1);
}

// === Live-Response (2026-11-25, echter Lite-Plan): Z.ai nennt die
// Token-Fenster inzwischen CREDIT_LIMIT statt TOKENS_LIMIT — gleiche
// unit/number-Semantik (3/5 = 5h, 6/1 = Weekly). ===
#[test]
fn parses_live_credit_limit_response() {
    let raw = r#"{
        "code": 200,
        "msg": "Operation successful",
        "data": {
            "limits": [
                { "type": "CREDIT_LIMIT", "unit": 3, "number": 5,
                  "usage": 2000, "currentValue": 15, "remaining": 1984,
                  "percentage": 1, "nextResetTime": 1795644712926 },
                { "type": "CREDIT_LIMIT", "unit": 6, "number": 1,
                  "usage": 10000, "currentValue": 1934, "remaining": 8065,
                  "percentage": 19, "nextResetTime": 1796192730981 }
            ],
            "level": "lite"
        },
        "success": true
    }"#;
    let snap = parse_quota(raw.as_bytes(), 0).expect("CREDIT_LIMIT muss erkannt werden");
    let kinds: Vec<_> = snap.windows.iter().map(|w| w.kind).collect();
    assert_eq!(kinds, [WindowKind::FiveHours, WindowKind::Weekly]);
    assert_eq!(snap.windows[0].percent_used, 1.0);
    assert_eq!(snap.windows[1].percent_used, 19.0);
    assert_eq!(snap.level.as_deref(), Some("lite"));
}
