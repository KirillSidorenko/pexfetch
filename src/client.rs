use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Serialize;
use url::Url;

use crate::error::AppError;
use crate::models::{Photo, SearchResponse};

const DEFAULT_API_BASE: &str = "https://api.pexels.com";
const USER_AGENT_VALUE: &str = concat!("pexels-agent-cli/", env!("CARGO_PKG_VERSION"));
const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_DOWNLOAD_MAX_BYTES: u64 = 200 * 1024 * 1024; // 200 MiB

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

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub http_timeout: Duration,
    pub connect_timeout: Duration,
    pub download_max_bytes: u64,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            http_timeout: DEFAULT_HTTP_TIMEOUT,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            download_max_bytes: DEFAULT_DOWNLOAD_MAX_BYTES,
        }
    }
}

pub struct PexelsClient {
    api_base: String,
    api_key: String,
    http: Client,
    download_max_bytes: u64,
}

impl PexelsClient {
    pub fn new(
        api_key: String,
        api_base: Option<String>,
        config: ClientConfig,
    ) -> Result<Self, AppError> {
        let api_base = api_base.unwrap_or_else(|| DEFAULT_API_BASE.to_owned());
        validate_api_base(&api_base)?;
        let http = Client::builder()
            .connect_timeout(config.connect_timeout)
            .timeout(config.http_timeout)
            .build()?;
        Ok(Self {
            api_base,
            api_key,
            http,
            download_max_bytes: config.download_max_bytes,
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
        let mut buf = [0u8; 64 * 1024];
        let mut total: u64 = 0;
        loop {
            let n = response.read(&mut buf)?;
            if n == 0 {
                break;
            }
            total = total.saturating_add(n as u64);
            if total > self.download_max_bytes {
                drop(file);
                let _ = std::fs::remove_file(destination);
                return Err(AppError::message(format!(
                    "download exceeds limit: {total} bytes > max {max} bytes (set PEXELS_AGENT_DOWNLOAD_MAX_BYTES to raise)",
                    max = self.download_max_bytes
                )));
            }
            file.write_all(&buf[..n])?;
        }
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

fn validate_api_base(api_base: &str) -> Result<(), AppError> {
    let url = Url::parse(api_base).map_err(|error| {
        AppError::message(format!(
            "PEXELS_AGENT_API_BASE is not a valid URL ({error}): {api_base}"
        ))
    })?;
    match url.scheme() {
        "https" => Ok(()),
        "http" if is_loopback(&url) => Ok(()),
        "http" => Err(AppError::message(format!(
            "PEXELS_AGENT_API_BASE must use https:// (got {api_base}); http:// is only permitted for loopback hosts like 127.0.0.1, ::1, or localhost"
        ))),
        other => Err(AppError::message(format!(
            "PEXELS_AGENT_API_BASE scheme must be https (got {other}://)"
        ))),
    }
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(domain)) => domain == "localhost",
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}
