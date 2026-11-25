// Task-4-Tests (Legacy auth.json/env-Tests entfallen mit dem Swap)
use cosmic_applet_token_quota::config::{
    load_from, move_row_across, remove_row, save_to, AppConfig, ProviderRow,
};
use cosmic_applet_token_quota::providers::ProviderKind;

// === Task 4: Provider-Config mit Grenzwerte-Helfern ===
fn row(kind: ProviderKind, key: &str) -> ProviderRow {
    ProviderRow {
        provider: kind,
        api_key: key.to_string(),
    }
}

#[test]
fn provider_kind_serde_roundtrip() {
    let raw = "provider = \"zai-coding-plan\"\napi_key = \"sk-test\"\n";
    let parsed: ProviderRow = toml::from_str(raw).expect("toml");
    assert_eq!(parsed.provider, ProviderKind::ZaiCodingPlan);
    assert_eq!(parsed.api_key, "sk-test");
    let back = toml::to_string(&parsed).expect("serialize");
    let reparsed: ProviderRow = toml::from_str(&back).expect("roundtrip");
    assert_eq!(reparsed, parsed);
    let opencode: ProviderRow =
        toml::from_str("provider = \"opencode-go\"\napi_key = \"\"").unwrap();
    assert_eq!(opencode.provider, ProviderKind::OpencodeGo);
}

#[test]
fn move_row_across_boundary_swap() {
    let mut rows = vec![
        row(ProviderKind::OpencodeGo, "a"),
        row(ProviderKind::ZaiCodingPlan, "b"),
        row(ProviderKind::OpencodeGo, "c"),
    ];
    let mut boundary = 1usize; // nur a im Panel
                               // Panel-Zeile a (0) ans Popup-Ende (2): a verlässt das Panel — es war
                               // EINZIGES Mitglied, die Linie fällt auf 0 (b,c bleiben Popup).
    move_row_across(&mut rows, &mut boundary, 0, 2);
    assert_eq!(boundary, 0);
    // Rückweg an die Listenspitze: a wird erste POPUP-Zeile (die Linie sitzt
    // über ihr) — Gruppenmitgliedschaft wechselt nicht.
    let moved_id = rows[2].api_key.clone();
    move_row_across(&mut rows, &mut boundary, 2, 0);
    assert_eq!(rows[0].api_key, moved_id);
    assert_eq!(boundary, 0);
    // Interner Tausch UNTERHALB der Linie: boundary bleibt.
    move_row_across(&mut rows, &mut boundary, 1, 2);
    assert_eq!(boundary, 0);
}

#[test]
fn move_row_across_joins_and_leaves_panel() {
    // Spiegelbildliche Richtung zu HAs keeps_other_members-Test:
    // Popup-Zeile an die Panel-Spitze → sie kommt ins Panel, die bisherigen
    // Panel-Mitglieder bleiben drin.
    let mut rows = vec![
        row(ProviderKind::OpencodeGo, "a"),
        row(ProviderKind::OpencodeGo, "b"),
        row(ProviderKind::ZaiCodingPlan, "c"),
        row(ProviderKind::ZaiCodingPlan, "d"),
    ];
    let mut boundary = 2usize; // a,b im Panel
    move_row_across(&mut rows, &mut boundary, 2, 0);
    assert_eq!(boundary, 3); // c kam dazu
    assert_eq!(rows[0].api_key, "c");
    assert_eq!(rows[1].api_key, "a");
    assert_eq!(rows[2].api_key, "b");
    // Rückweg: c ans Ende — verlässt das Panel, a,b bleiben.
    move_row_across(&mut rows, &mut boundary, 0, 3);
    assert_eq!(boundary, 2);
    assert_eq!(rows[3].api_key, "c");
}

#[test]
fn move_row_across_ignores_out_of_bounds() {
    let mut rows = vec![row(ProviderKind::OpencodeGo, "a")];
    let mut boundary = 1usize;
    move_row_across(&mut rows, &mut boundary, 0, 5);
    move_row_across(&mut rows, &mut boundary, 5, 0);
    assert_eq!(boundary, 1);
    assert_eq!(rows.len(), 1);
}

#[test]
fn remove_row_shifts_boundary() {
    let mut rows = vec![
        row(ProviderKind::OpencodeGo, "a"),
        row(ProviderKind::OpencodeGo, "b"),
        row(ProviderKind::ZaiCodingPlan, "c"),
    ];
    let mut boundary = 2usize;
    remove_row(&mut rows, &mut boundary, 0); // über der Linie löschen → Linie rückt mit
    assert_eq!(boundary, 1);
    assert_eq!(rows.len(), 2);
    remove_row(&mut rows, &mut boundary, 1); // unter der Linie löschen → Linie bleibt (auf Länge geklemmt)
    assert_eq!(boundary, 1);
    remove_row(&mut rows, &mut boundary, 0);
    remove_row(&mut rows, &mut boundary, 0);
    assert!(rows.is_empty());
    assert_eq!(boundary, 0);
}

#[test]
fn save_to_sets_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub").join("config.toml");
    save_to(&path, &AppConfig::default()).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn load_from_invalid_toml_returns_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "not [valid toml").unwrap();
    let cfg = load_from(&path);
    assert_eq!(cfg, AppConfig::default());
}

#[test]
fn default_config_has_opencode_row() {
    let cfg = AppConfig::default();
    assert_eq!(cfg.providers.len(), 1);
    assert_eq!(cfg.providers[0].provider, ProviderKind::OpencodeGo);
    assert_eq!(cfg.providers[0].api_key, "");
    assert_eq!(cfg.panel_count, None);
    assert_eq!(cfg.panel_rows().len(), 1);
}

#[test]
fn panel_rows_clamps_boundary() {
    let cfg = AppConfig {
        providers: vec![
            row(ProviderKind::OpencodeGo, "a"),
            row(ProviderKind::ZaiCodingPlan, "b"),
        ],
        panel_count: Some(5),
    };
    assert_eq!(cfg.panel_rows().len(), 2);
    let cfg_none = AppConfig {
        providers: cfg.providers.clone(),
        panel_count: None,
    };
    assert_eq!(cfg_none.panel_rows().len(), 2);
}
