use std::path::{Path, PathBuf};

use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Serialize;
use url::Url;

use crate::error::AppError;
use crate::models::{Photo, SearchResponse};

const DEFAULT_API_BASE: &str = "https://api.pexels.com";
const USER_AGENT_VALUE: &str = "pexels-agent-cli/0.1.0";

#[derive(Debug, Clone, Serialize)]
pub struct SearchRequest<'a> {
    pub query: &'a str,
    pub page: u64,
    pub per_page: u64,
    pub orientation: Option<&'a str>,
    pub size: Option<&'a str>,
    pub color: Option<&'a str>,
    pub locale: Option<&'a str>,
}

pub struct PexelsClient {
    api_base: String,
    api_key: String,
    http: Client,
}

impl PexelsClient {
    pub fn new(api_key: String, api_base: Option<String>) -> Result<Self, AppError> {
        let http = Client::builder().build()?;
        Ok(Self {
            api_base: api_base.unwrap_or_else(|| DEFAULT_API_BASE.to_owned()),
            api_key,
            http,
        })
    }

    pub fn search_photos(&self, request: &SearchRequest<'_>) -> Result<SearchResponse, AppError> {
        let endpoint = self.endpoint("/v1/search")?;
        let response = self
            .http
            .get(endpoint)
            .header(AUTHORIZATION, &self.api_key)
            .header(ACCEPT, "application/json")
            .header(USER_AGENT, USER_AGENT_VALUE)
            .query(request)
            .send()?
            .error_for_status()?;

        Ok(response.json()?)
    }

    pub fn get_photo(&self, photo_id: u64) -> Result<Photo, AppError> {
        let endpoint = self.endpoint(&format!("/v1/photos/{photo_id}"))?;
        let response = self
            .http
            .get(endpoint)
            .header(AUTHORIZATION, &self.api_key)
            .header(ACCEPT, "application/json")
            .header(USER_AGENT, USER_AGENT_VALUE)
            .send()?
            .error_for_status()?;

        Ok(response.json()?)
    }

    pub fn check_connection(&self) -> Result<(), AppError> {
        let endpoint = self.endpoint("/v1/search")?;
        self.http
            .get(endpoint)
            .header(AUTHORIZATION, &self.api_key)
            .header(ACCEPT, "application/json")
            .header(USER_AGENT, USER_AGENT_VALUE)
            .query(&[("query", "status"), ("page", "1"), ("per_page", "1")])
            .send()?
            .error_for_status()?;
        Ok(())
    }

    pub fn download_file(&self, source_url: &str, destination: &Path) -> Result<PathBuf, AppError> {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut response = self
            .http
            .get(source_url)
            .header(USER_AGENT, USER_AGENT_VALUE)
            .send()?
            .error_for_status()?;

        let mut file = std::fs::File::create(destination)?;
        response.copy_to(&mut file)?;
        Ok(destination.to_path_buf())
    }

    fn endpoint(&self, path: &str) -> Result<Url, AppError> {
        Ok(Url::parse(&format!(
            "{}/{}",
            self.api_base.trim_end_matches('/'),
            path.trim_start_matches('/')
        ))?)
    }
}
