use cosmic_applet_token_quota::cache::{load_snapshots, save_snapshots, Snapshots};
use cosmic_applet_token_quota::quota::{ProviderSnapshot, ProviderWindow, WindowKind};
use std::collections::HashMap;
use tempfile::tempdir;

fn snap(five: f64, weekly: f64) -> ProviderSnapshot {
    ProviderSnapshot {
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
        ],
        fetched_at_unix: 123,
        level: Some("pro".into()),
    }
}

#[test]
fn roundtrip_two_providers() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nested").join("cache.json");
    let mut snaps: Snapshots = HashMap::new();
    snaps.insert("opencode-go".into(), snap(16.0, 77.0));
    snaps.insert("zai-coding-plan".into(), snap(5.0, 24.0));
    save_snapshots(Some(&path), &snaps).unwrap();
    let loaded = load_snapshots(Some(&path));
    assert_eq!(loaded.len(), 2);
    let oc = &loaded["opencode-go"];
    assert_eq!(oc.percent_used_of(WindowKind::FiveHours), Some(16.0));
    assert_eq!(oc.percent_used_of(WindowKind::Weekly), Some(77.0));
    assert_eq!(oc.fetched_at_unix, 123);
    assert_eq!(loaded["zai-coding-plan"].level.as_deref(), Some("pro"));
}

#[test]
fn load_missing_path_returns_empty() {
    let dir = tempdir().unwrap();
    assert!(load_snapshots(Some(&dir.path().join("nope.json"))).is_empty());
    assert!(load_snapshots(None).is_empty());
}

#[test]
fn invalid_entry_is_skipped_others_survive() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("cache.json");
    let mut snaps: Snapshots = HashMap::new();
    snaps.insert("good".into(), snap(10.0, 20.0));
    snaps.insert("survivor".into(), snap(5.0, 24.0));
    save_snapshots(Some(&path), &snaps).unwrap();
    // "good" defekt machen (percent_used 150) — "survivor" bleibt gültig:
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("10.0", "150.0");
    std::fs::write(&path, text).unwrap();
    let loaded = load_snapshots(Some(&path));
    assert_eq!(loaded.len(), 1);
    assert!(loaded.contains_key("survivor"));
    assert!(!loaded.contains_key("good"));
}

#[test]
fn invalid_file_returns_empty() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("cache.json");
    std::fs::write(&path, "not json at all").unwrap();
    assert!(load_snapshots(Some(&path)).is_empty());
}
