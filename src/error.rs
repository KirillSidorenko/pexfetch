use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("{0}")]
    MissingCredential(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error(
        "rate limited by Pexels{}",
        retry_after_secs.map(|s| format!(" (retry after {s}s)")).unwrap_or_default()
    )]
    RateLimited {
        retry_after_secs: Option<u64>,
        remaining: Option<u64>,
        reset_at: Option<u64>,
    },
    #[error("unknown quality '{quality}' (available: {})", available.join(", "))]
    InvalidQuality {
        quality: String,
        available: Vec<String>,
    },
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Http(reqwest::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Url(#[from] url::ParseError),
}

impl AppError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Message(_) => "error",
            Self::MissingCredential(_) => "missing_credential",
            Self::Unauthorized(_) => "unauthorized",
            Self::Forbidden(_) => "forbidden",
            Self::NotFound(_) => "not_found",
            Self::RateLimited { .. } => "rate_limited",
            Self::InvalidQuality { .. } => "invalid_quality",
            Self::Io(_) => "io_error",
            Self::Http(_) => "http_error",
            Self::Json(_) => "json_error",
            Self::Url(_) => "url_error",
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::MissingCredential(_) | Self::Unauthorized(_) | Self::Forbidden(_) => 3,
            Self::NotFound(_) | Self::InvalidQuality { .. } => 4,
            Self::Http(_) => 5,
            Self::RateLimited { .. } => 6,
            Self::Message(_) | Self::Io(_) | Self::Json(_) | Self::Url(_) => 1,
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::Message(format!("HTTP request timed out: {error}"))
        } else if error.is_connect() {
            Self::Message(format!("HTTP connection error: {error}"))
        } else {
            Self::Http(error)
        }
    }
}
