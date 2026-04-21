//! Structured error type used by every command in the crate.
//!
//! Variants are chosen so that automation driving the CLI can branch on
//! [`AppError::kind`] without parsing prose, and so that the shell exit
//! code ([`AppError::exit_code`]) distinguishes recoverable from fatal
//! failures. The `From<reqwest::Error>` impl also detects timeouts and
//! connection errors and lifts them to a readable `Message` string.

use std::io;

use thiserror::Error;

/// Every failure mode surfaced by the library.
///
/// See [`AppError::kind`] for the snake_case identifier emitted in JSON
/// error payloads and [`AppError::exit_code`] for the mapping from
/// variants to process exit codes.
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
    /// Build an ad-hoc error with a free-form message. Prefer a dedicated
    /// variant where one exists so the `kind` field stays useful.
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    /// Stable snake_case identifier for automation. Included as
    /// `error.kind` in the JSON error payload written to stderr.
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

    /// Process exit code for this error. See the README for the
    /// category-to-code table.
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
