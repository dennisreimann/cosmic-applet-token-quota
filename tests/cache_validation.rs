#[test]
fn cache_snapshot_with_invalid_percent_is_rejected() {
    // Datei mit percent_used > 100 bzw. negativem reset_unix:
    let bad_percent = r#"{ "windows": [ { "kind": "Weekly", "percent_used": 150.0, "reset_unix": 1 } ],
        "fetched_at_unix": 1788278419, "level": null }"#;
    assert!(
        serde_json::from_str::<cosmic_applet_token_quota::quota::ProviderSnapshot>(bad_percent)
            .is_err()
    );
    let bad_reset = r#"{ "windows": [ { "kind": "Weekly", "percent_used": 10.0, "reset_unix": -1 } ],
        "fetched_at_unix": 1788278419, "level": null }"#;
    assert!(
        serde_json::from_str::<cosmic_applet_token_quota::quota::ProviderSnapshot>(bad_reset)
            .is_err()
    );
}
