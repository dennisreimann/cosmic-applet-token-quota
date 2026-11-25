//! Konfiguration des Applets (`config.rs`).
//!
//! Persistente Konfiguration: TOML unter
//! `~/.config/cosmic-applet-token-quota/config.toml` (0600). API-Keys
//! liegen AUSSCHLIESSLICH hier — kein auth.json-/env-Fallback
//! (Entscheidung 2026-11-25).

// ============================================================================
// NEU (Task 4): Anbieter-Konfiguration mit Panel-Grenze. Der alte Teil oben
// (auth.json/resolve_api_key/legacy cache path) wird in Task 7 entfernt.
// ============================================================================

impl Default for AppConfig {
    /// Default (Spec-Migration): OpenCode Go allein, über der Linie, ohne
    /// Key — ohne Config-Datei verhält sich das Applet wie das Alt-Projekt.
    fn default() -> Self {
        Self {
            providers: vec![ProviderRow {
                provider: crate::providers::ProviderKind::OpencodeGo,
                api_key: String::new(),
            }],
            panel_count: None,
        }
    }
}

/// Eine konfigurierte Anbieter-Zeile: Anbieter-Typ plus API-Key.
/// Der Key liegt AUSSCHLIESSLICH hier (0600) — kein auth.json/env-Fallback.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProviderRow {
    pub provider: crate::providers::ProviderKind,
    #[serde(default)]
    pub api_key: String,
}

/// Gesamte Applet-Konfiguration (`~/.config/cosmic-applet-token-quota/config.toml`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub providers: Vec<ProviderRow>,
    /// Wie viele Zeilen (in Reihenfolge) im Panel erscheinen. `None` = alle.
    #[serde(default)]
    pub panel_count: Option<usize>,
}

/// Instanz-Schlüssel einer Anbieter-Zeile: Anbieter-ID + Listenposition.
/// Nötig, weil derselbe Anbieter MEHRMALS konfiguriert sein darf (z. B.
/// mit verschiedenen Keys) — Cache und Fetch-Routing brauchen dann
/// zeileninstanz-spezifische Schlüssel ("opencode-go#0" etc.).
#[must_use]
pub fn row_id(index: usize, provider: crate::providers::ProviderKind) -> String {
    format!("{}#{}", provider.id(), index)
}

impl AppConfig {
    /// Die Zeilen, die im Panel erscheinen: die ersten `panel_count`
    /// (geklemmt auf die Länge); `None` = alle.
    #[must_use]
    pub fn panel_rows(&self) -> &[ProviderRow] {
        let n = match self.panel_count {
            Some(n) => n.min(self.providers.len()),
            None => self.providers.len(),
        };
        &self.providers[..n]
    }
}

/// Verschiebt eine Zeile von Index `from` nach `to` (Out-of-Bounds ignoriert).
/// Kreuzt der Weg die Panel-Grenze, wechselt GENAU diese Zeile die Gruppe —
/// die Grenze folgt, alle übrigen Zeilen bleiben in ihren Gruppen
/// (Semantik wie applet-homeassistant `move_entity_across`).
pub fn move_row_across(rows: &mut Vec<ProviderRow>, boundary: &mut usize, from: usize, to: usize) {
    if from >= rows.len() || to >= rows.len() || from == to {
        return;
    }
    let was_in_panel = from < *boundary;
    let moved = rows.remove(from);
    if was_in_panel {
        *boundary = boundary.saturating_sub(1);
    }
    rows.insert(to, moved);
    if to < *boundary {
        *boundary += 1;
    }
}

/// Entfernt die Zeile an `i` und rückt die Panel-Grenze mit (falls die
/// gelöschte Zeile über der Grenze lag); die Grenze wird auf die Listenlänge
/// geklemmt (Semantik wie applet-homeassistant `remove_row`).
pub fn remove_row(rows: &mut Vec<ProviderRow>, boundary: &mut usize, i: usize) {
    if i >= rows.len() {
        return;
    }
    rows.remove(i);
    if i < *boundary {
        *boundary -= 1;
    }
    *boundary = (*boundary).min(rows.len());
}

/// Pfad der persistenten Konfigurationsdatei.
#[must_use]
pub fn config_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("cosmic-applet-token-quota").join("config.toml"))
}

/// Lädt die Konfiguration vom Standardpfad; fehlt sie, gilt der Default
/// (OpenCode Go allein, ohne Key — wie das Alt-Verhalten).
#[must_use]
pub fn load() -> AppConfig {
    config_path().map_or_else(AppConfig::default, |p| load_from(&p))
}

/// Lädt aus einer Datei; fehlt sie oder ist ungültig → Default (keine Panik).
#[must_use]
pub fn load_from(path: &std::path::Path) -> AppConfig {
    match std::fs::read_to_string(path) {
        Ok(source) => toml::from_str::<AppConfig>(&source).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

/// Speichert am Standardpfad (0600).
pub fn save(cfg: &AppConfig) -> std::io::Result<()> {
    let path = config_path()
        .ok_or_else(|| std::io::Error::other("kein Config-Verzeichnis ermittelbar"))?;
    save_to(&path, cfg)
}

/// Speichert an einen expliziten Pfad (0600, atomar via tempfile — HA-Muster).
pub fn save_to(path: &std::path::Path, cfg: &AppConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = toml::to_string(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("no config directory"))?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::Builder::new()
        .prefix(".config-")
        .tempfile_in(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    use std::io::Write;
    file.write_all(body.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}
