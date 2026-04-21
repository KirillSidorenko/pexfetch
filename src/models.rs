//! Wire-format types shared by the HTTP client and the JSON payloads
//! printed on stdout.
//!
//! The `*Response` types mirror the Pexels API and are `Deserialize`.
//! The `*Payload` types are what we actually emit to stdout; they are
//! `Serialize`-only and tuned for agent consumption (stable field
//! names, `skip_serializing_if` on optional fields).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Single photo as returned by the Pexels API. `src` holds the fixed
/// set of quality variants keyed by name ("original", "large2x", etc.)
/// but is typed as a map so new qualities the API adds deserialize
/// without a code change.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Photo {
    pub id: u64,
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub url: Option<String>,
    pub photographer: Option<String>,
    #[serde(default)]
    pub src: BTreeMap<String, String>,
}

/// Deserialized body of `GET /v1/search`.
#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub total_results: Option<u64>,
    #[serde(default)]
    pub photos: Vec<Photo>,
    pub next_page: Option<String>,
}

/// Stdout payload for `pexels-agent auth status` / `auth login` /
/// `auth logout`.
#[derive(Debug, Serialize)]
pub struct AuthStatusPayload {
    pub config_path: String,
    pub configured: bool,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed: Option<bool>,
}

/// Stdout payload for `pexels-agent status`.
#[derive(Debug, Serialize)]
pub struct StatusPayload {
    pub api_base: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_error: Option<String>,
    pub api_reachable: bool,
    pub config_path: String,
    pub configured: bool,
    pub source: String,
}

/// Stdout payload for `pexels-agent search`.
#[derive(Debug, Serialize)]
pub struct SearchPayload {
    pub next_page: Option<String>,
    pub page: u64,
    pub per_page: u64,
    pub photos: Vec<Photo>,
    pub query: String,
    pub total_results: u64,
}

/// Stdout payload for `pexels-agent download` and `download-first`.
#[derive(Debug, Serialize)]
pub struct DownloadPayload {
    pub photo_id: u64,
    pub quality: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    pub saved_to: String,
    pub source_url: String,
}
