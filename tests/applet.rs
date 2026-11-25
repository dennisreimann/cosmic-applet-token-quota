use cosmic_applet_token_quota::applet::{
    configured_fetch_targets, fmt_updated, format_duration, merge_fetched, provider_panel_segments,
    provider_popup_rows,
};
use cosmic_applet_token_quota::cache::Snapshots;
use cosmic_applet_token_quota::config::{AppConfig, ProviderRow};
use cosmic_applet_token_quota::providers::ProviderKind;
use cosmic_applet_token_quota::quota::{percent_label, usage_level, UsageLevel};
use std::collections::HashMap;
use std::sync::Once;

static EN_LOCALE: Once = Once::new();

/// Deterministische Sprache für diesen Testprozess: EN (Quell-Locale),
/// unabhängig von der Desktop-Locale des Rechners. Muss vor dem ersten
/// Format-Funktionsaufruf laufen (Once); der Loader initialisiert beim
/// ersten Zugriff.
fn force_en() {
    EN_LOCALE.call_once(|| std::env::set_var("I18N_LANGUAGE", "en"));
}

fn psnap(
    five: f64,
    weekly: f64,
    monthly: f64,
) -> cosmic_applet_token_quota::quota::ProviderSnapshot {
    use cosmic_applet_token_quota::quota::{ProviderWindow, WindowKind};
    cosmic_applet_token_quota::quota::ProviderSnapshot {
        windows: vec![
            ProviderWindow {
                kind: WindowKind::FiveHours,
                percent_used: five,
                reset_unix: 1771073738,
            },
            ProviderWindow {
                kind: WindowKind::Weekly,
                percent_used: weekly,
                reset_unix: 1744137600,
            },
            ProviderWindow {
                kind: WindowKind::Monthly,
                percent_used: monthly,
                reset_unix: 1772615765,
            },
        ],
        fetched_at_unix: 100,
        level: None,
    }
}

fn psnap_zai(
    five: f64,
    weekly: Option<f64>,
    mcp: f64,
) -> cosmic_applet_token_quota::quota::ProviderSnapshot {
    use cosmic_applet_token_quota::quota::{ProviderWindow, WindowKind};
    let mut windows = vec![ProviderWindow {
        kind: WindowKind::FiveHours,
        percent_used: five,
        reset_unix: 1771073738,
    }];
    if let Some(w) = weekly {
        windows.push(ProviderWindow {
            kind: WindowKind::Weekly,
            percent_used: w,
            reset_unix: 1744137600,
        });
    }
    windows.push(ProviderWindow {
        kind: WindowKind::McpMonthly,
        percent_used: mcp,
        reset_unix: 1772615765,
    });
    cosmic_applet_token_quota::quota::ProviderSnapshot {
        windows,
        fetched_at_unix: 100,
        level: Some("pro".into()),
    }
}

fn seg_text(v: &[(String, UsageLevel)]) -> String {
    v.iter().map(|(t, _)| t.as_str()).collect()
}

#[test]
fn provider_panel_segments_opencode_three_windows() {
    let segs = provider_panel_segments(&psnap(16.0, 85.0, 38.0));
    assert_eq!(seg_text(&segs), "16% · 85% W · 38% M");
    assert_eq!(segs[0].1, UsageLevel::Normal);
    assert_eq!(segs[2].1, UsageLevel::Warning); // 85% >= 80
    assert_eq!(segs[4].1, UsageLevel::Normal);
    // 100% → Critical:
    let segs = provider_panel_segments(&psnap(16.0, 85.0, 100.0));
    assert_eq!(segs[4].1, UsageLevel::Critical);
}

#[test]
fn provider_panel_segments_zai_skips_mcp() {
    let segs = provider_panel_segments(&psnap_zai(5.0, Some(24.0), 12.0));
    assert_eq!(seg_text(&segs), "5% · 24% W");
}

#[test]
fn provider_panel_segments_zai_legacy_without_weekly() {
    let segs = provider_panel_segments(&psnap_zai(5.0, None, 12.0));
    assert_eq!(seg_text(&segs), "5%");
}

#[test]
fn provider_popup_rows_include_mcp_with_reset() {
    force_en();
    let rows = provider_popup_rows(&psnap_zai(5.0, Some(24.0), 12.0));
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].0, "5h");
    assert_eq!(rows[1].0, "W");
    assert_eq!(rows[2].0, "MCP · M"); // lokalisiertes MCP-Label
    assert!(rows.iter().all(|r| r.3.is_some()));
}

#[test]
fn provider_popup_rows_no_reset_at_zero() {
    force_en();
    let rows = provider_popup_rows(&psnap_zai(0.0, Some(24.0), 12.0));
    assert_eq!(rows[0].3, None); // 5h bei 0% ohne Countdown
    assert!(rows[1].3.is_some());
}

#[test]
fn configured_fetch_targets_filters_empty_keys() {
    let cfg = AppConfig {
        providers: vec![
            ProviderRow {
                provider: ProviderKind::OpencodeGo,
                api_key: "k1".into(),
            },
            ProviderRow {
                provider: ProviderKind::ZaiCodingPlan,
                api_key: "".into(),
            },
            ProviderRow {
                provider: ProviderKind::ZaiCodingPlan,
                api_key: "  ".into(),
            },
        ],
        panel_count: None,
    };
    let targets = configured_fetch_targets(&cfg);
    assert_eq!(
        targets,
        vec![(
            ProviderKind::OpencodeGo,
            "k1".to_string(),
            "opencode-go#0".to_string()
        )]
    );
    assert!(configured_fetch_targets(&AppConfig {
        providers: vec![ProviderRow {
            provider: ProviderKind::OpencodeGo,
            api_key: String::new()
        }],
        panel_count: None,
    })
    .is_empty());
}

#[test]
fn merge_fetched_ok_replaces_and_clears_error() {
    let mut snaps: Snapshots = HashMap::new();
    snaps.insert("p1".into(), psnap(1.0, 2.0, 3.0));
    let mut errors = HashMap::new();
    errors.insert("p1".to_string(), "alt".to_string());
    let fresh = psnap(50.0, 60.0, 70.0);
    merge_fetched(&mut snaps, &mut errors, "p1", &Ok(fresh));
    assert_eq!(
        snaps["p1"].percent_used_of(cosmic_applet_token_quota::quota::WindowKind::FiveHours),
        Some(50.0)
    );
    assert!(!errors.contains_key("p1"));
}

#[test]
fn merge_fetched_err_keeps_snapshot_and_sets_error() {
    let mut snaps: Snapshots = HashMap::new();
    snaps.insert("p1".into(), psnap(1.0, 2.0, 3.0));
    let mut errors: HashMap<String, String> = HashMap::new();
    merge_fetched(&mut snaps, &mut errors, "p1", &Err("boom".into()));
    assert_eq!(
        snaps["p1"].percent_used_of(cosmic_applet_token_quota::quota::WindowKind::FiveHours),
        Some(1.0)
    );
    assert_eq!(errors.get("p1").map(String::as_str), Some("boom"));
    // Err auf unbekannten Anbieter: kein Snapshot erscheint
    merge_fetched(&mut snaps, &mut errors, "neu", &Err("x".into()));
    assert!(!snaps.contains_key("neu"));
    assert_eq!(errors.get("neu").map(String::as_str), Some("x"));
}

#[test]
fn format_duration_english_plural() {
    force_en();
    assert_eq!(format_duration(3600), "1 hour");
    assert_eq!(format_duration(3660), "1 hour 1 minute");
    assert_eq!(format_duration(5400), "1 hour 30 minutes");
    assert_eq!(format_duration(90000), "1 day 1 hour");
    assert_eq!(format_duration(93600), "1 day 2 hours");
    assert_eq!(format_duration(172800), "2 days");
    assert_eq!(format_duration(120), "2 minutes");
    assert_eq!(format_duration(59), "59 seconds");
}

#[test]
fn updated_formats_local_time() {
    force_en();
    let unix = chrono::TimeZone::timestamp_opt(&chrono::Local, 1788278419, 0)
        .unwrap()
        .timestamp();
    let expected = chrono::DateTime::from_timestamp(1788278419, 0)
        .unwrap()
        .with_timezone(&chrono::Local)
        .format("%H:%M:%S")
        .to_string();
    assert_eq!(fmt_updated(1788278419), expected);
    let _ = unix;
}

#[test]
fn usage_level_direct_on_used_values() {
    force_en();
    assert_eq!(usage_level(79.4), UsageLevel::Normal);
    assert_eq!(usage_level(85.0), UsageLevel::Warning);
    assert_eq!(usage_level(100.0), UsageLevel::Critical);
}

#[test]
fn percent_label_rounds_shared_impl() {
    force_en();
    assert_eq!(percent_label(15.6), "16%");
}

// === Task 8: Settings-Spawn-Pfad ===
#[test]
fn settings_binary_path_points_at_sibling() {
    let path = cosmic_applet_token_quota::applet::settings_binary_path().expect("path");
    let name = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .expect("utf8");
    assert_eq!(name, "cosmic-applet-token-quota-settings");
}
