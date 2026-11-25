//! Gemeinsame HTTP-Schicht für alle Anbieter-Fetcher: 10-s-Timeout, genau
//! ein Retry bei 408/425/429/5xx, `Accept: application/json`, Auth-Stil
//! wählbar (OpenCode `Bearer`, Z.ai plain — Spec 2026-11-25).

use reqwest::StatusCode;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStyle {
    /// `Authorization: Bearer <key>`
    Bearer,
    /// `Authorization: <key>` (roh, ohne Prefix — Z.ai-Monitor-Endpunkt)
    Plain,
}

#[must_use]
pub fn auth_header_value(auth: AuthStyle, api_key: &str) -> String {
    match auth {
        AuthStyle::Bearer => format!("Bearer {api_key}"),
        AuthStyle::Plain => api_key.to_string(),
    }
}

#[derive(Debug)]
pub struct ApiError {
    pub message: String,
    pub retryable: bool,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "api: {}", self.message)
    }
}

impl std::error::Error for ApiError {}

/// Retry bei 408/425/429 und allen 5xx (Vertrag aus dem Alt-Projekt).
#[must_use]
pub fn is_retryable(code: StatusCode) -> bool {
    matches!(
        code,
        StatusCode::REQUEST_TIMEOUT | StatusCode::TOO_EARLY | StatusCode::TOO_MANY_REQUESTS
    ) || code.is_server_error()
}

pub const TIMEOUT_MS: u64 = 10_000;
pub const MAX_ATTEMPTS: usize = 2;

/// GET mit Auth-Header; gibt den Body bei Erfolg zurück.
///
/// Fehler sind retryable bei Netzwerk-/Timeout-Problemen und den
/// retryablen Statuscodes; sonst final.
pub async fn fetch_bytes_at(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    auth: AuthStyle,
) -> Result<bytes::Bytes, ApiError> {
    let mut attempt = 0;
    loop {
        let resp = client
            .get(url)
            .header(
                reqwest::header::AUTHORIZATION,
                auth_header_value(auth, api_key),
            )
            .header(reqwest::header::ACCEPT, "application/json")
            .timeout(std::time::Duration::from_millis(TIMEOUT_MS))
            .send()
            .await
            .map_err(|e| ApiError {
                message: e.to_string(),
                retryable: true,
            })?;

        if resp.status().is_success() {
            return resp.bytes().await.map_err(|e| ApiError {
                message: e.to_string(),
                retryable: true,
            });
        }

        let retryable = is_retryable(resp.status());
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let msg = format!("status {}: {}", status.as_u16(), text);
        if retryable && attempt + 1 < MAX_ATTEMPTS {
            attempt += 1;
            continue;
        }
        return Err(ApiError {
            message: msg,
            retryable,
        });
    }
}

/// Alias mit Argumentreihenfolge aus dem Plan (`fetch_bytes`).
pub async fn fetch_bytes(
    client: &reqwest::Client,
    api_key: &str,
    auth: AuthStyle,
    url: &str,
) -> Result<bytes::Bytes, ApiError> {
    fetch_bytes_at(client, url, api_key, auth).await
}
