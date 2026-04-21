use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    #[serde(default)]
    pub photos: Vec<Photo>,
    pub next_page: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthStatusPayload {
    pub config_path: String,
    pub configured: bool,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed: Option<bool>,
}

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

#[derive(Debug, Serialize)]
pub struct SearchPayload {
    pub next_page: Option<String>,
    pub page: u64,
    pub per_page: u64,
    pub photos: Vec<Photo>,
    pub query: String,
    pub total_results: usize,
}

#[derive(Debug, Serialize)]
pub struct DownloadPayload {
    pub photo_id: u64,
    pub quality: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    pub saved_to: String,
    pub source_url: String,
}
