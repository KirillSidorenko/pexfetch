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

/// Stdout payload for `pexfetch auth status` / `auth login` /
/// `auth logout`.
#[derive(Debug, Serialize)]
pub struct AuthStatusPayload {
    pub config_path: String,
    pub configured: bool,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed: Option<bool>,
}

/// Stdout payload for `pexfetch status`.
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

/// Stdout payload for `pexfetch search`.
#[derive(Debug, Serialize)]
pub struct SearchPayload {
    pub next_page: Option<String>,
    pub page: u64,
    pub per_page: u64,
    pub photos: Vec<Photo>,
    pub query: String,
    pub total_results: u64,
}

/// Stdout payload for `pexfetch download` and `download-first`.
#[derive(Debug, Serialize)]
pub struct DownloadPayload {
    pub photo_id: u64,
    pub quality: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    pub saved_to: String,
    pub source_url: String,
}

/// Photographer/videographer attribution attached to each video.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VideoUser {
    pub id: u64,
    pub name: Option<String>,
    pub url: Option<String>,
}

/// One renderable variant of a video (specific resolution, fps, codec).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VideoFile {
    pub id: u64,
    pub quality: Option<String>,
    pub file_type: Option<String>,
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub fps: Option<f64>,
    pub link: String,
}

/// Still preview frame sampled at a given position in the video.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VideoPicture {
    pub id: u64,
    pub picture: String,
    pub nr: Option<u64>,
}

/// Single video as returned by the Pexels `/v1/videos` endpoints.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Video {
    pub id: u64,
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub url: Option<String>,
    pub image: Option<String>,
    pub duration: Option<u64>,
    pub user: Option<VideoUser>,
    #[serde(default)]
    pub video_files: Vec<VideoFile>,
    #[serde(default)]
    pub video_pictures: Vec<VideoPicture>,
}

/// Deserialized body of `GET /v1/videos/search` and
/// `GET /v1/videos/popular`.
#[derive(Debug, Deserialize)]
pub struct VideosSearchResponse {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub total_results: Option<u64>,
    #[serde(default)]
    pub videos: Vec<Video>,
    pub next_page: Option<String>,
    pub prev_page: Option<String>,
}

/// Stdout payload for `pexfetch videos search`.
#[derive(Debug, Serialize)]
pub struct VideoSearchPayload {
    pub next_page: Option<String>,
    pub prev_page: Option<String>,
    pub page: u64,
    pub per_page: u64,
    pub query: String,
    pub total_results: u64,
    pub videos: Vec<Video>,
}

/// Stdout payload for `pexfetch videos download` and
/// `pexfetch videos download-first`.
#[derive(Debug, Serialize)]
pub struct VideoDownloadPayload {
    pub video_id: u64,
    pub video_file_id: u64,
    pub quality: Option<String>,
    pub file_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    pub saved_to: String,
    pub source_url: String,
}
