//! Gemeinsames Quota-Modell für alle Anbieter (Task 2).
//!
//! Jeder Anbieter normalisiert seine API-Antwort auf einen
//! [`ProviderSnapshot`] mit kanonisch geordneten [`ProviderWindow`]s.
//! `percent_used` ist der VERBRAUCHTE Anteil (0–100).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowKind {
    FiveHours,
    Weekly,
    Monthly,
    McpMonthly,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderWindow {
    pub kind: WindowKind,
    pub percent_used: f64,
    pub reset_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderSnapshot {
    /// Kanonische Reihenfolge: FiveHours, Weekly, Monthly, McpMonthly.
    pub windows: Vec<ProviderWindow>,
    pub fetched_at_unix: i64,
    /// Nur Z.ai („lite"/„pro"/„max" roh); `None` bei OpenCode.
    pub level: Option<String>,
}

impl<'de> Deserialize<'de> for ProviderSnapshot {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct RawWindow {
            kind: WindowKind,
            percent_used: f64,
            reset_unix: i64,
        }
        struct Validating(Vec<RawWindow>);

        impl<'de> Deserialize<'de> for Validating {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let windows = Vec::<RawWindow>::deserialize(deserializer)?;
                for w in &windows {
                    if !w.percent_used.is_finite() || !(0.0..=100.0).contains(&w.percent_used) {
                        return Err(serde::de::Error::custom(
                            "cached percent_used is out of range",
                        ));
                    }
                    if w.reset_unix < 0 {
                        return Err(serde::de::Error::custom("cached reset_unix is negative"));
                    }
                }
                Ok(Validating(windows))
            }
        }

        #[derive(Deserialize)]
        struct RawSnapshot {
            windows: Validating,
            fetched_at_unix: i64,
            #[serde(default)]
            level: Option<String>,
        }

        let raw = RawSnapshot::deserialize(deserializer)?;
        Ok(ProviderSnapshot {
            windows: raw
                .windows
                .0
                .into_iter()
                .map(|w| ProviderWindow {
                    kind: w.kind,
                    percent_used: w.percent_used,
                    reset_unix: w.reset_unix,
                })
                .collect(),
            fetched_at_unix: raw.fetched_at_unix,
            level: raw.level,
        })
    }
}

impl ProviderSnapshot {
    /// Verbrauchter Anteil eines Fensters; `None`, wenn das Fenster fehlt.
    #[must_use]
    pub fn percent_used_of(&self, kind: WindowKind) -> Option<f64> {
        self.windows
            .iter()
            .find(|w| w.kind == kind)
            .map(|w| w.percent_used)
    }

    /// Reset-Zeit eines Fensters (Unix-Sekunden); `None`, wenn fehlt.
    #[must_use]
    pub fn reset_unix_of(&self, kind: WindowKind) -> Option<i64> {
        self.windows
            .iter()
            .find(|w| w.kind == kind)
            .map(|w| w.reset_unix)
    }
}

/// Farbstufe des Verbrauchs, abgeleitet vom ANGEZEIGTEN (gerundeten) Wert:
/// < 80 % normal, ab 80 % gelb (Warning), bei 100 % rot (Critical).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageLevel {
    Normal,
    Warning,
    Critical,
}

#[must_use]
pub fn usage_level(percent_used: f64) -> UsageLevel {
    match percent_used.round() as i64 {
        100.. => UsageLevel::Critical,
        80.. => UsageLevel::Warning,
        _ => UsageLevel::Normal,
    }
}

/// Panel-Buchstabe eines Fensters: Weekly → `W`, Monthly → `M`, sonst keiner
/// (5h ist der ungelabelte erste Wert, McpMonthly erscheint nie im Panel).
#[must_use]
pub fn panel_letter(kind: WindowKind) -> Option<&'static str> {
    match kind {
        WindowKind::Weekly => Some("W"),
        WindowKind::Monthly => Some("M"),
        WindowKind::FiveHours | WindowKind::McpMonthly => None,
    }
}

/// Erscheint dieses Fenster im Panel? MCP-Monatsquote nur im Popup.
#[must_use]
pub fn in_panel(kind: WindowKind) -> bool {
    kind != WindowKind::McpMonthly
}

/// Prozentanzeige gerundet: `16%`.
#[must_use]
pub fn percent_label(p: f64) -> String {
    format!("{}%", p.round())
}

#[cfg(test)]
mod common_model_tests {
    use super::*;

    #[test]
    fn usage_level_thresholds_round_before_compare() {
        assert_eq!(usage_level(79.4), UsageLevel::Normal);
        assert_eq!(usage_level(79.5), UsageLevel::Warning); // rundet auf 80
        assert_eq!(usage_level(99.6), UsageLevel::Critical); // rundet auf 100
        assert_eq!(usage_level(100.0), UsageLevel::Critical);
        assert_eq!(usage_level(0.0), UsageLevel::Normal);
    }

    #[test]
    fn panel_letter_maps_weekly_and_monthly_only() {
        assert_eq!(panel_letter(WindowKind::Weekly), Some("W"));
        assert_eq!(panel_letter(WindowKind::Monthly), Some("M"));
        assert_eq!(panel_letter(WindowKind::FiveHours), None);
        assert_eq!(panel_letter(WindowKind::McpMonthly), None);
    }

    #[test]
    fn in_panel_excludes_mcp_monthly() {
        assert!(in_panel(WindowKind::FiveHours));
        assert!(in_panel(WindowKind::Weekly));
        assert!(in_panel(WindowKind::Monthly));
        assert!(!in_panel(WindowKind::McpMonthly));
    }

    #[test]
    fn percent_label_rounds() {
        assert_eq!(percent_label(15.6), "16%");
        assert_eq!(percent_label(0.0), "0%");
    }

    const VALID_SNAPSHOT: &str = r#"{
        "windows": [
            { "kind": "FiveHours", "percent_used": 1.0, "reset_unix": 1771073738 },
            { "kind": "Weekly", "percent_used": 24.0, "reset_unix": 1744137600 }
        ],
        "fetched_at_unix": 100,
        "level": "lite"
    }"#;

    #[test]
    fn snapshot_deserializes_valid_payload() {
        let snap: ProviderSnapshot = serde_json::from_str(VALID_SNAPSHOT).expect("valid");
        assert_eq!(snap.windows.len(), 2);
        assert_eq!(snap.windows[0].kind, WindowKind::FiveHours);
        assert_eq!(snap.level.as_deref(), Some("lite"));
    }

    #[test]
    fn snapshot_rejects_percent_out_of_range() {
        let raw = r#"{ "windows": [ { "kind": "Weekly", "percent_used": 150.0, "reset_unix": 1 } ],
                       "fetched_at_unix": 100, "level": null }"#;
        assert!(serde_json::from_str::<ProviderSnapshot>(raw).is_err());
        let raw_neg = r#"{ "windows": [ { "kind": "Weekly", "percent_used": -1.0, "reset_unix": 1 } ],
                          "fetched_at_unix": 100, "level": null }"#;
        assert!(serde_json::from_str::<ProviderSnapshot>(raw_neg).is_err());
    }

    #[test]
    fn snapshot_rejects_negative_reset_unix() {
        let raw = r#"{ "windows": [ { "kind": "Weekly", "percent_used": 10.0, "reset_unix": -5 } ],
                       "fetched_at_unix": 100, "level": null }"#;
        assert!(serde_json::from_str::<ProviderSnapshot>(raw).is_err());
    }

    #[test]
    fn accessors_find_windows_by_kind() {
        let snap: ProviderSnapshot = serde_json::from_str(VALID_SNAPSHOT).unwrap();
        assert_eq!(snap.percent_used_of(WindowKind::Weekly), Some(24.0));
        assert_eq!(snap.reset_unix_of(WindowKind::Weekly), Some(1744137600));
        assert_eq!(snap.percent_used_of(WindowKind::Monthly), None);
    }
}
