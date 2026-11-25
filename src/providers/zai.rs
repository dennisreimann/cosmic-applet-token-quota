//! Z.ai Coding Plan (GLM) — Quota über den Monitor-Endpunkt.
//!
//! **Inoffiziell** (nicht im openapi.json der Docs; Spec 2026-11-25 mit
//! Community-Quellen verifiziert): `GET /api/monitor/usage/quota/limit`,
//! `Authorization: <key>` OHNE Bearer-Prefix. Antwort defensively parsen —
//! fehlende Fenster sind kein Fehler (Legacy-Pläne ohne Weekly).

use crate::api::{fetch_bytes_at, ApiError, AuthStyle};
use crate::providers::opencode::now_unix;
use crate::quota::{ProviderSnapshot, ProviderWindow, WindowKind};
use serde::Deserialize;
use std::fmt;

pub const QUOTA_URL: &str = "https://api.z.ai/api/monitor/usage/quota/limit";

#[derive(Debug)]
pub enum QuotaError {
    Json(serde_json::Error),
    Contract(String),
}

impl fmt::Display for QuotaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuotaError::Json(e) => write!(f, "invalid json: {e}"),
            QuotaError::Contract(m) => write!(f, "invalid Z.ai quota response: {m}"),
        }
    }
}
impl std::error::Error for QuotaError {}

#[derive(Deserialize)]
struct RawQuotaResponse {
    #[serde(default)]
    code: i64,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    success: bool,
    data: Option<RawData>,
}

#[derive(Deserialize)]
struct RawData {
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    limits: Vec<RawLimit>,
}

#[derive(Deserialize)]
struct RawLimit {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    unit: i64,
    #[serde(default)]
    number: Option<i64>,
    #[serde(default)]
    percentage: Option<f64>,
    #[serde(rename = "nextResetTime", default)]
    next_reset_time: Option<i64>,
}

/// Fenster-Rangfolge für die kanonische Sortierung.
fn rank(kind: WindowKind) -> u8 {
    match kind {
        WindowKind::FiveHours => 0,
        WindowKind::Weekly => 1,
        WindowKind::Monthly => 2,
        WindowKind::McpMonthly => 3,
    }
}

/// Unix-Millisekunden → Unix-Sekunden ( negatives → 0, defensiv).
fn ms_to_s(ms: i64) -> i64 {
    if ms <= 0 {
        0
    } else {
        ms / 1000
    }
}

/// Parsed eine `quota/limit`-Antwort.
///
/// Erkannte Fenster: `TOKENS_LIMIT unit=3 number=5` → FiveHours,
/// `TOKENS_LIMIT unit=6` → Weekly, `TIME_LIMIT` → McpMonthly (Monat).
/// Unbekannte Typen werden übersprungen; ein erkanntes Fenster OHNE
/// `percentage` ist ein Contract-Fehler. `success:false` immer.
pub fn parse_quota(raw: &[u8], now_unix: i64) -> Result<ProviderSnapshot, QuotaError> {
    let parsed: RawQuotaResponse = serde_json::from_slice(raw).map_err(QuotaError::Json)?;
    if !parsed.success {
        return Err(QuotaError::Contract(format!(
            "api error {}: {}",
            parsed.code, parsed.msg
        )));
    }
    let data = parsed
        .data
        .ok_or_else(|| QuotaError::Contract("data missing".to_string()))?;

    let mut windows = Vec::new();
    for limit in data.limits {
        let kind = match (limit.kind.as_str(), limit.unit, limit.number) {
            // API-Generationen: ältere Specs nennen die Token-Fenster
            // TOKENS_LIMIT, die Live-API (2026-11-25) CREDIT_LIMIT —
            // unit/number-Semantik ist identisch.
            ("TOKENS_LIMIT" | "CREDIT_LIMIT", 3, Some(5)) => WindowKind::FiveHours,
            ("TOKENS_LIMIT" | "CREDIT_LIMIT", 6, _) => WindowKind::Weekly,
            ("TIME_LIMIT", _, _) => WindowKind::McpMonthly,
            _ => continue, // unbekanntes Fenster: defensiv überspringen
        };
        let percentage = limit.percentage.ok_or_else(|| {
            QuotaError::Contract(format!(
                "{} (unit {}) percentage missing",
                limit.kind, limit.unit
            ))
        })?;
        if !percentage.is_finite() || !(0.0..=100.0).contains(&percentage) {
            return Err(QuotaError::Contract(format!(
                "{} percentage must be finite from 0 to 100",
                limit.kind
            )));
        }
        windows.push(ProviderWindow {
            kind,
            percent_used: percentage,
            reset_unix: ms_to_s(limit.next_reset_time.unwrap_or(0)),
        });
    }
    windows.sort_by_key(|w| rank(w.kind));

    Ok(ProviderSnapshot {
        windows,
        fetched_at_unix: now_unix,
        level: data.level,
    })
}

/// Fetcht die Coding-Plan-Quota an einer expliziten URL (testbar via httpmock).
pub async fn fetch_quota_at(
    client: &reqwest::Client,
    api_key: &str,
    url: &str,
) -> Result<ProviderSnapshot, ApiError> {
    let bytes = fetch_bytes_at(client, url, api_key, AuthStyle::Plain).await?;
    parse_quota(&bytes, now_unix()).map_err(|e| ApiError {
        message: e.to_string(),
        retryable: false,
    })
}

/// Fetcht die Coding-Plan-Quota vom offiziellen Endpunkt.
pub async fn fetch_quota(
    client: &reqwest::Client,
    api_key: &str,
) -> Result<ProviderSnapshot, ApiError> {
    fetch_quota_at(client, api_key, QUOTA_URL).await
}
