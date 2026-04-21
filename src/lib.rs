use std::env;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};
use rpassword::prompt_password_from_bufread;
use serde_json::{Value, json};
use url::Url;

mod auth;
mod client;
mod error;
mod models;

use auth::{config_path, load_stored_api_key, remove_stored_api_key, save_api_key};
use client::{ClientConfig, PexelsClient, SearchRequest};
pub use error::AppError;
use models::{AuthStatusPayload, DownloadPayload, Photo, SearchPayload, StatusPayload};

const PEXELS_API_KEY_URL: &str = "https://www.pexels.com/api/key/";

#[derive(Debug, Parser)]
#[command(name = "pexels-agent")]
#[command(about = "Search, authenticate, and download Pexels images from the terminal.")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Authenticate, inspect config state, or remove saved credentials")]
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    #[command(about = "Check configured auth and live API connectivity")]
    Status,
    #[command(about = "Search Pexels photos and return JSON results")]
    Search(SearchArgs),
    #[command(about = "Download a specific Pexels photo by id")]
    Download(DownloadArgs),
    #[command(about = "Search and download the first matching Pexels photo")]
    DownloadFirst(DownloadFirstArgs),
}

#[derive(Debug, Subcommand)]
enum AuthCommand {
    #[command(about = "Show where credentials come from without calling the API")]
    Status,
    #[command(about = "Save a Pexels API key from --api-key or interactive stdin")]
    Login {
        #[arg(long)]
        api_key: Option<String>,
    },
    #[command(about = "Remove the stored Pexels API key from the local config file")]
    Logout,
}

#[derive(Debug, Clone, Args)]
struct SearchArgs {
    #[arg(long)]
    query: String,
    #[arg(long, default_value_t = 1)]
    page: u64,
    #[arg(long = "per-page", default_value_t = 15)]
    per_page: u64,
    #[arg(long)]
    orientation: Option<String>,
    #[arg(long)]
    size: Option<String>,
    #[arg(long)]
    color: Option<String>,
    #[arg(long)]
    locale: Option<String>,
}

#[derive(Debug, Args)]
struct DownloadArgs {
    #[arg(long)]
    id: u64,
    #[arg(long, default_value = "original")]
    quality: String,
    #[arg(long = "output-dir")]
    output_dir: PathBuf,
}

#[derive(Debug, Args)]
struct DownloadFirstArgs {
    #[command(flatten)]
    search: SearchArgs,
    #[arg(long, default_value = "original")]
    quality: String,
    #[arg(long = "output-dir")]
    output_dir: PathBuf,
}

pub fn main_entry() -> i32 {
    let cli = Cli::parse();
    let stdin = io::stdin();
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut stdin_lock = stdin.lock();
    let mut stdout_lock = stdout.lock();
    let mut stderr_lock = stderr.lock();

    match run(cli, &mut stdin_lock, &mut stdout_lock, &mut stderr_lock) {
        Ok(()) => 0,
        Err(error) => {
            emit_error_json(&mut stderr_lock, &error);
            error.exit_code()
        }
    }
}

fn emit_error_json(stderr: &mut impl Write, error: &AppError) {
    let mut details = serde_json::Map::new();
    details.insert("kind".to_owned(), Value::String(error.kind().to_owned()));
    details.insert("message".to_owned(), Value::String(error.to_string()));
    match error {
        AppError::RateLimited {
            retry_after_secs,
            remaining,
            reset_at,
        } => {
            if let Some(v) = retry_after_secs {
                details.insert("retry_after_secs".to_owned(), json!(v));
            }
            if let Some(v) = remaining {
                details.insert("remaining".to_owned(), json!(v));
            }
            if let Some(v) = reset_at {
                details.insert("reset_at".to_owned(), json!(v));
            }
        }
        AppError::InvalidQuality { available, .. } => {
            details.insert("available_qualities".to_owned(), json!(available));
        }
        _ => {}
    }
    let payload = json!({ "ok": false, "error": Value::Object(details) });
    let _ = writeln!(stderr, "{payload}");
}

fn run(
    mut cli: Cli,
    stdin: &mut impl BufRead,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<(), AppError> {
    match &mut cli.command {
        Command::Auth { command } => run_auth(command, stdin, stdout, stderr),
        Command::Status => emit_json(stdout, &status_payload()?),
        Command::Search(args) => {
            let client = build_client()?;
            let response = client.search_photos(&search_request(args))?;
            emit_json(
                stdout,
                &search_payload(
                    args,
                    response.photos,
                    response.page,
                    response.per_page,
                    response.next_page,
                ),
            )
        }
        Command::Download(args) => {
            let client = build_client()?;
            let photo = client.get_photo(args.id)?;
            let source_url = quality_url(&photo, &args.quality)?;
            let destination =
                build_destination(&args.output_dir, photo.id, &args.quality, &source_url)?;
            client.download_file(&source_url, &destination)?;
            emit_json(
                stdout,
                &DownloadPayload {
                    photo_id: photo.id,
                    quality: args.quality.clone(),
                    query: None,
                    saved_to: destination.to_string_lossy().into_owned(),
                    source_url,
                },
            )
        }
        Command::DownloadFirst(args) => {
            let client = build_client()?;
            let response = client.search_photos(&search_request(&args.search))?;
            let photo = response.photos.into_iter().next().ok_or_else(|| {
                AppError::NotFound(format!("No photos found for query '{}'", args.search.query))
            })?;
            let source_url = quality_url(&photo, &args.quality)?;
            let destination =
                build_destination(&args.output_dir, photo.id, &args.quality, &source_url)?;
            client.download_file(&source_url, &destination)?;
            emit_json(
                stdout,
                &DownloadPayload {
                    photo_id: photo.id,
                    quality: args.quality.clone(),
                    query: Some(args.search.query.clone()),
                    saved_to: destination.to_string_lossy().into_owned(),
                    source_url,
                },
            )
        }
    }
}

fn run_auth(
    command: &mut AuthCommand,
    stdin: &mut impl BufRead,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<(), AppError> {
    match command {
        AuthCommand::Status => emit_json(stdout, &auth_status_payload(None)?),
        AuthCommand::Login { api_key } => {
            let api_key = match api_key.take() {
                Some(api_key) => {
                    writeln!(
                        stderr,
                        "warning: --api-key is visible in `ps` and shell history; prefer PEXELS_API_KEY env or interactive stdin"
                    )?;
                    api_key.trim().to_owned()
                }
                None => prompt_for_api_key(stdin, stderr)?,
            };
            let path = save_api_key(&api_key)?;
            emit_json(
                stdout,
                &AuthStatusPayload {
                    config_path: path.to_string_lossy().into_owned(),
                    configured: true,
                    source: "config".to_owned(),
                    removed: None,
                },
            )
        }
        AuthCommand::Logout => {
            let removed = remove_stored_api_key()?;
            emit_json(stdout, &auth_status_payload(Some(removed))?)
        }
    }
}

fn prompt_for_api_key(
    stdin: &mut impl BufRead,
    stderr: &mut impl Write,
) -> Result<String, AppError> {
    writeln!(stderr, "Open this page to get your Pexels API key:")?;
    writeln!(stderr, "{PEXELS_API_KEY_URL}")?;
    writeln!(stderr, "Paste the API key below and press Enter.")?;
    let api_key = prompt_password_from_bufread(stdin, stderr, "Pexels API key: ")
        .map_err(|error| AppError::message(error.to_string()))?;
    let api_key = api_key.trim().to_owned();
    if api_key.is_empty() {
        return Err(AppError::message("API key is required"));
    }
    Ok(api_key)
}

fn auth_status_payload(removed: Option<bool>) -> Result<AuthStatusPayload, AppError> {
    let auth_state = resolve_auth_state()?;

    Ok(AuthStatusPayload {
        config_path: auth_state.config_path.to_string_lossy().into_owned(),
        configured: auth_state.configured,
        source: auth_state.source,
        removed,
    })
}

fn build_client() -> Result<PexelsClient, AppError> {
    let auth_state = resolve_auth_state()?;
    let api_key = auth_state.api_key.ok_or_else(|| {
        AppError::MissingCredential(
            "PEXELS_API_KEY is not set and no stored config was found".to_owned(),
        )
    })?;
    let api_base = env::var("PEXELS_AGENT_API_BASE").ok();
    PexelsClient::new(api_key, api_base, client_config_from_env()?)
}

fn client_config_from_env() -> Result<ClientConfig, AppError> {
    let mut config = ClientConfig::default();
    if let Ok(raw) = env::var("PEXELS_AGENT_HTTP_TIMEOUT_MS") {
        let ms: u64 = raw.parse().map_err(|_| {
            AppError::message(format!(
                "PEXELS_AGENT_HTTP_TIMEOUT_MS must be a positive integer (got {raw})"
            ))
        })?;
        config.http_timeout = std::time::Duration::from_millis(ms);
    }
    if let Ok(raw) = env::var("PEXELS_AGENT_DOWNLOAD_MAX_BYTES") {
        let bytes: u64 = raw.parse().map_err(|_| {
            AppError::message(format!(
                "PEXELS_AGENT_DOWNLOAD_MAX_BYTES must be a positive integer (got {raw})"
            ))
        })?;
        config.download_max_bytes = bytes;
    }
    Ok(config)
}

fn status_payload() -> Result<StatusPayload, AppError> {
    let auth_state = resolve_auth_state()?;
    let api_base =
        env::var("PEXELS_AGENT_API_BASE").unwrap_or_else(|_| "https://api.pexels.com".to_owned());

    let (api_reachable, api_error) = match build_client() {
        Ok(client) => match client.check_connection() {
            Ok(()) => (true, None),
            Err(error) => (false, Some(error.to_string())),
        },
        Err(error) => (false, Some(error.to_string())),
    };

    Ok(StatusPayload {
        api_base,
        api_error,
        api_reachable,
        config_path: auth_state.config_path.to_string_lossy().into_owned(),
        configured: auth_state.configured,
        source: auth_state.source,
    })
}

struct AuthState {
    api_key: Option<String>,
    config_path: PathBuf,
    configured: bool,
    source: String,
}

fn resolve_auth_state() -> Result<AuthState, AppError> {
    let config_path = config_path()?;
    let env_key = env::var("PEXELS_API_KEY")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let stored_key = load_stored_api_key()?;

    let (api_key, configured, source) = if let Some(api_key) = env_key {
        (Some(api_key), true, "env".to_owned())
    } else if let Some(api_key) = stored_key {
        (Some(api_key), true, "config".to_owned())
    } else {
        (None, false, "none".to_owned())
    };

    Ok(AuthState {
        api_key,
        config_path,
        configured,
        source,
    })
}

fn search_request(args: &SearchArgs) -> SearchRequest<'_> {
    SearchRequest {
        query: &args.query,
        page: args.page,
        per_page: args.per_page,
        orientation: args.orientation.as_deref(),
        size: args.size.as_deref(),
        color: args.color.as_deref(),
        locale: args.locale.as_deref(),
    }
}

fn search_payload(
    args: &SearchArgs,
    photos: Vec<Photo>,
    page: Option<u64>,
    per_page: Option<u64>,
    next_page: Option<String>,
) -> SearchPayload {
    SearchPayload {
        next_page,
        page: page.unwrap_or(args.page),
        per_page: per_page.unwrap_or(args.per_page),
        total_results: photos.len(),
        photos,
        query: args.query.clone(),
    }
}

fn quality_url(photo: &Photo, quality: &str) -> Result<String, AppError> {
    if let Some(url) = photo.src.get(quality) {
        return Ok(url.clone());
    }
    Err(AppError::InvalidQuality {
        quality: quality.to_owned(),
        available: photo.src.keys().cloned().collect(),
    })
}

fn build_destination(
    output_dir: &Path,
    photo_id: u64,
    quality: &str,
    source_url: &str,
) -> Result<PathBuf, AppError> {
    let suffix = Url::parse(source_url)
        .ok()
        .and_then(|url| {
            Path::new(url.path())
                .extension()
                .map(|extension| format!(".{}", extension.to_string_lossy()))
        })
        .unwrap_or_else(|| ".jpeg".to_owned());

    Ok(output_dir.join(format!("{photo_id}-{quality}{suffix}")))
}

fn emit_json(stdout: &mut impl Write, payload: &impl serde::Serialize) -> Result<(), AppError> {
    serde_json::to_writer_pretty(&mut *stdout, payload)?;
    writeln!(stdout)?;
    Ok(())
}
