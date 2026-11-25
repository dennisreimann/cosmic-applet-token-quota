//! Anbieter-Registry: jeder Quota-Anbieter bringt Fetcher, Parser, Logo und
//! Anzeigename mit. Ausgabe ist immer das gemeinsame
//! [`crate::quota::ProviderSnapshot`].
//!
//! Start-Anbieter (Spec 2026-11-25): OpenCode Go und Z.ai Coding Plan.

pub mod opencode;
pub mod zai;

use serde::{Deserialize, Serialize};

/// Die unterstützten Quota-Anbieter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    OpencodeGo,
    ZaiCodingPlan,
}

impl ProviderKind {
    /// Stabile ID (Config-/Cache-Schlüssel).
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::OpencodeGo => "opencode-go",
            Self::ZaiCodingPlan => "zai-coding-plan",
        }
    }

    /// Anzeigename (Markenname, nicht übersetzt).
    #[must_use]
    pub fn display(self) -> &'static str {
        match self {
            Self::OpencodeGo => "OpenCode Go",
            Self::ZaiCodingPlan => "Z.ai Coding Plan",
        }
    }

    /// Logo als SVG-Bytes (symbolic, `currentColor`).
    #[must_use]
    pub fn logo_svg(self) -> &'static [u8] {
        match self {
            Self::OpencodeGo => include_bytes!("../../assets/o.svg"),
            Self::ZaiCodingPlan => include_bytes!("../../assets/z.svg"),
        }
    }
}

/// Fetcht die Quota des gegebenen Anbieters (eigener reqwest::Client je
/// Call — Pattern aus dem Alt-Projekt).
pub async fn fetch(
    client: &reqwest::Client,
    kind: ProviderKind,
    api_key: &str,
) -> Result<crate::quota::ProviderSnapshot, crate::api::ApiError> {
    match kind {
        ProviderKind::OpencodeGo => opencode::fetch_quota(client, api_key).await,
        ProviderKind::ZaiCodingPlan => zai::fetch_quota(client, api_key).await,
    }
}

/// ID-String → Kind (für Config-Toleranz bei fremden Dateien).
#[must_use]
pub fn provider_kind_from_id(id: &str) -> Option<ProviderKind> {
    match id {
        "opencode-go" => Some(ProviderKind::OpencodeGo),
        "zai-coding-plan" => Some(ProviderKind::ZaiCodingPlan),
        _ => None,
    }
}
