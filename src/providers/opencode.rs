//! OpenCode Go — Datenquelle und Parser.
//!
//! API: `GET https://opencode.ai/zen/go/v1/usage` mit
//! `Authorization: Bearer <key>` und `Accept: application/json`.
//! Ausgabe: gemeinsames [`crate::quota::ProviderSnapshot`] —
//! API-`percent` ist der VERBRAUCHTE Anteil (AGENTS.md-Vertrag) und wird
//! direkt zu `percent_used`; `resetsAt` (RFC3339) → Unix-Sekunden.

use crate::api::{fetch_bytes_at, AuthStyle};
use crate::quota::{ProviderSnapshot, ProviderWindow, WindowKind};
use serde::Deserialize;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

pub const USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";

pub async fn fetch_quota(
    client: &reqwest::Client,
    api_key: &str,
) -> Result<ProviderSnapshot, ApiError> {
    fetch_quota_at(client, api_key, USAGE_URL).await
}

pub async fn fetch_quota_at(
    client: &reqwest::Client,
    api_key: &str,
    url: &str,
) -> Result<ProviderSnapshot, ApiError> {
    let bytes = fetch_bytes_at(client, url, api_key, AuthStyle::Bearer).await?;
    parse_status(&bytes, now_unix()).map_err(|e| ApiError {
        message: e.to_string(),
        retryable: false,
    })
}

#[derive(Debug)]
pub enum QuotaError {
    Json(serde_json::Error),
    Contract(String),
}

impl fmt::Display for QuotaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuotaError::Json(e) => write!(f, "invalid json: {e}"),
            QuotaError::Contract(m) => write!(f, "invalid OpenCode Go API response: {m}"),
        }
    }
}
impl std::error::Error for QuotaError {}

#[derive(Deserialize)]
struct RawResponse {
    usage: RawUsage,
}
#[derive(Deserialize)]
struct RawUsage {
    rolling: RawWindow,
    weekly: RawWindow,
    monthly: RawWindow,
}
#[derive(Deserialize)]
struct RawWindow {
    status: String,
    percent: Option<f64>,
    #[serde(rename = "resetsAt")]
    resets_at: Option<String>,
}

fn normalize(name: &str, w: RawWindow) -> Result<ProviderWindow, QuotaError> {
    let percent = match w.status.as_str() {
        "ok" => Some(
            w.percent
                .ok_or_else(|| QuotaError::Contract(format!("{name} percent missing")))?,
        ),
        "rate-limited" => Some(w.percent.unwrap_or(100.0)),
        _ => None,
    }
    .ok_or_else(|| QuotaError::Contract(format!("{name} status is not ok or rate-limited")))?;
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return Err(QuotaError::Contract(format!(
            "{name} percent must be finite from 0 to 100"
        )));
    }
    let resets_at = w
        .resets_at
        .ok_or_else(|| QuotaError::Contract(format!("{name} resetsAt missing")))?;
    let reset_unix = chrono::DateTime::parse_from_rfc3339(&resets_at)
        .map_err(|_| QuotaError::Contract(format!("{name} resetsAt must be valid RFC3339")))?
        .timestamp();
    Ok(ProviderWindow {
        kind: match name {
            "rolling" => WindowKind::FiveHours,
            "weekly" => WindowKind::Weekly,
            _ => WindowKind::Monthly,
        },
        percent_used: percent,
        reset_unix,
    })
}

/// Parsed die `/usage`-Antwort in einen normalisierten Snapshot
/// (kanonische Reihenfolge FiveHours, Weekly, Monthly).
pub fn parse_status(raw: &[u8], now_unix: i64) -> Result<ProviderSnapshot, QuotaError> {
    let parsed: RawResponse = serde_json::from_slice(raw).map_err(QuotaError::Json)?;
    let windows = vec![
        normalize("rolling", parsed.usage.rolling)?,
        normalize("weekly", parsed.usage.weekly)?,
        normalize("monthly", parsed.usage.monthly)?,
    ];
    Ok(ProviderSnapshot {
        windows,
        fetched_at_unix: now_unix,
        level: None,
    })
}

/// Unix-Sekunden "jetzt" (für Tests und Cache-Platzierung getrennt hältbar).
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub use crate::api::ApiError;
