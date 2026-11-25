//! Cache: zuletzt bekannte Snapshots je Anbieter
//! (`~/.cache/cosmic-applet-token-quota/cache.json`). Ermöglicht den Start
//! mit dem letzten Stand, ohne auf den ersten Fetch zu warten. Jeder
//! Eintrag wird beim Laden validiert (siehe `ProviderSnapshot`'s
//! `Deserialize`); defekte Einträge werden einzeln übersprungen.

use crate::quota::ProviderSnapshot;
use anyhow::Context;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Snapshots je Anbieter-ID.
pub type Snapshots = HashMap<String, ProviderSnapshot>;

#[derive(serde::Serialize, serde::Deserialize)]
struct CacheFile {
    #[serde(default)]
    providers: Snapshots,
}

/// Cache-Pfad (`~/.cache/cosmic-applet-token-quota/cache.json`).
#[must_use]
pub fn cache_path() -> Option<std::path::PathBuf> {
    dirs::cache_dir().map(|d| d.join("cosmic-applet-token-quota").join("cache.json"))
}

/// Lädt alle gültigen Einträge; fehlende/defekte Datei → leere Map.
/// Defekte EINZELNEinträge werden übersprungen (Rest bleibt erhalten).
#[must_use]
pub fn load_snapshots(path: Option<&Path>) -> Snapshots {
    let Some(path) = path else {
        return Snapshots::new();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return Snapshots::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        tracing::warn!("Cache unlesbar (kein JSON), wird ignoriert");
        return Snapshots::new();
    };
    let Some(entries) = value.get("providers").and_then(|p| p.as_object()) else {
        return Snapshots::new();
    };
    let mut snaps = Snapshots::new();
    for (id, entry) in entries {
        match serde_json::from_value::<ProviderSnapshot>(entry.clone()) {
            Ok(snap) => {
                snaps.insert(id.clone(), snap);
            }
            Err(error) => {
                tracing::warn!("Cache-Eintrag für {id} ungültig, übersprungen: {error}");
            }
        }
    }
    snaps
}

/// Speichert die Snapshots (Elternverzeichnisse werden angelegt).
pub fn save_snapshots(path: Option<&Path>, snaps: &Snapshots) -> anyhow::Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create cache dir")?;
    }
    let body = serde_json::to_vec_pretty(&CacheFile {
        providers: snaps.clone(),
    })
    .context("serialize cache")?;
    fs::write(path, body).context("write cache")?;
    Ok(())
}
