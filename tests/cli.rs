use std::fs;

use assert_cmd::Command;
use httpmock::Method::GET;
use httpmock::MockServer;
use serde_json::{Value, json};
use tempfile::tempdir;

fn command_with_config(config_path: &std::path::Path) -> Command {
    let mut command = Command::cargo_bin("pexels-agent").expect("binary exists");
    command.env("PEXELS_AGENT_CONFIG_PATH", config_path);
    command
}

fn parse_stdout_json(command: &mut Command) -> Value {
    let output = command.assert().success().get_output().stdout.clone();
    serde_json::from_slice(&output).expect("stdout is valid json")
}

#[test]
fn root_help_mentions_status_and_auth_flows() {
    let mut command = Command::cargo_bin("pexels-agent").expect("binary exists");
    let assert = command.arg("--help").assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("Authenticate, inspect config state, or remove saved credentials"));
    assert!(stdout.contains("Check configured auth and live API connectivity"));
    assert!(stdout.contains("Search Pexels photos and return JSON results"));
}

#[test]
fn status_help_mentions_api_connectivity_check() {
    let mut command = Command::cargo_bin("pexels-agent").expect("binary exists");
    let assert = command.args(["status", "--help"]).assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("Check configured auth and live API connectivity"));
    assert!(stdout.contains("Usage: pexels-agent status"));
}

#[test]
fn auth_status_reports_missing_configuration() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");

    let payload = parse_stdout_json(command_with_config(&config_path).args(["auth", "status"]));

    assert_eq!(payload["configured"], false);
    assert_eq!(payload["source"], "none");
}

#[test]
fn search_rejects_non_https_api_base() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");

    let mut command = command_with_config(&config_path);
    let assert = command
        .env("PEXELS_API_KEY", "test-key")
        .env("PEXELS_AGENT_API_BASE", "http://evil.example.com")
        .args(["search", "--query", "mountains"])
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();

    assert!(
        stderr.contains("https"),
        "error must mention https, got: {stderr:?}"
    );
    assert!(
        stderr.contains("PEXELS_AGENT_API_BASE"),
        "error must name the env var, got: {stderr:?}"
    );
}

#[test]
fn auth_login_with_cli_flag_prints_security_warning() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");

    let mut command = command_with_config(&config_path);
    command.args(["auth", "login", "--api-key", "pexels-secret"]);
    let assert = command.assert().success();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();

    assert!(
        stderr.to_lowercase().contains("warning"),
        "expected stderr to contain a warning, got: {stderr:?}"
    );
    assert!(
        stderr.contains("--api-key"),
        "warning must name the --api-key flag, got: {stderr:?}"
    );
    assert!(
        stderr.contains("ps") || stderr.contains("shell history"),
        "warning must mention ps/shell-history leak, got: {stderr:?}"
    );
}

#[cfg(unix)]
#[test]
fn auth_login_writes_config_file_with_mode_0600() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");

    command_with_config(&config_path)
        .args(["auth", "login", "--api-key", "pexels-secret"])
        .assert()
        .success();

    let mode = fs::metadata(&config_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "expected mode 0600, got {mode:o}");
}

#[test]
fn auth_login_saves_key_to_config() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");

    let payload = parse_stdout_json(command_with_config(&config_path).args([
        "auth",
        "login",
        "--api-key",
        "pexels-secret",
    ]));

    assert_eq!(payload["configured"], true);
    assert_eq!(payload["source"], "config");

    let stored: Value = serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    assert_eq!(stored["api_key"], "pexels-secret");
}

#[test]
fn auth_login_without_api_key_prints_link_and_reads_token_from_stdin() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");

    let mut command = command_with_config(&config_path);
    command
        .args(["auth", "login"])
        .write_stdin("pexels-secret\n");

    let assert = command.assert().success();
    let stdout = assert.get_output().stdout.clone();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    let payload: Value = serde_json::from_slice(&stdout).unwrap();

    assert_eq!(payload["configured"], true);
    assert_eq!(payload["source"], "config");
    assert!(stderr.contains("https://www.pexels.com/api/key/"));
    assert!(stderr.contains("Pexels API key:"));

    let stored: Value = serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    assert_eq!(stored["api_key"], "pexels-secret");
}

#[test]
fn auth_logout_removes_saved_config() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    fs::write(
        &config_path,
        json!({ "api_key": "pexels-secret" }).to_string(),
    )
    .unwrap();

    let payload = parse_stdout_json(command_with_config(&config_path).args(["auth", "logout"]));

    assert_eq!(payload["configured"], false);
    assert_eq!(payload["removed"], true);
    assert!(!config_path.exists());
}

#[test]
fn status_reports_missing_api_key_without_failing() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");

    let payload = parse_stdout_json(command_with_config(&config_path).args(["status"]));

    assert_eq!(payload["configured"], false);
    assert_eq!(payload["source"], "none");
    assert_eq!(payload["api_reachable"], false);
    assert!(
        payload["api_error"]
            .as_str()
            .unwrap()
            .contains("PEXELS_API_KEY is not set")
    );
}

#[test]
fn status_checks_api_connectivity() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let server = MockServer::start();

    let status_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/search")
            .header("authorization", "test-key")
            .query_param("query", "status")
            .query_param("page", "1")
            .query_param("per_page", "1");
        then.status(200).json_body(json!({
            "page": 1,
            "per_page": 1,
            "photos": [],
            "next_page": null
        }));
    });

    let payload = parse_stdout_json(
        command_with_config(&config_path)
            .env("PEXELS_API_KEY", "test-key")
            .env("PEXELS_AGENT_API_BASE", server.base_url())
            .args(["status"]),
    );

    status_mock.assert();
    assert_eq!(payload["configured"], true);
    assert_eq!(payload["source"], "env");
    assert_eq!(payload["api_reachable"], true);
    assert_eq!(payload["api_error"], Value::Null);
    assert_eq!(payload["api_base"], server.base_url());
}

#[test]
fn search_prints_machine_readable_json() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let server = MockServer::start();

    let search_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/search")
            .header("authorization", "test-key")
            .query_param("query", "mountains")
            .query_param("page", "1")
            .query_param("per_page", "2")
            .query_param("orientation", "landscape")
            .query_param("size", "large")
            .query_param("color", "blue");
        then.status(200).json_body(json!({
            "page": 1,
            "per_page": 2,
            "photos": [
                {
                    "id": 1001,
                    "width": 4000,
                    "height": 3000,
                    "url": "https://www.pexels.com/photo/mountain-1001/",
                    "photographer": "Ada",
                    "src": {
                        "original": "https://images.pexels.com/photos/1001/original.jpeg",
                        "large2x": "https://images.pexels.com/photos/1001/large2x.jpeg"
                    }
                },
                {
                    "id": 1002,
                    "width": 3000,
                    "height": 2000,
                    "url": "https://www.pexels.com/photo/lake-1002/",
                    "photographer": "Linus",
                    "src": {
                        "original": "https://images.pexels.com/photos/1002/original.jpeg",
                        "large2x": "https://images.pexels.com/photos/1002/large2x.jpeg"
                    }
                }
            ],
            "next_page": "https://api.pexels.com/v1/search?page=2"
        }));
    });

    let payload = parse_stdout_json(
        command_with_config(&config_path)
            .env("PEXELS_API_KEY", "test-key")
            .env("PEXELS_AGENT_API_BASE", server.base_url())
            .args([
                "search",
                "--query",
                "mountains",
                "--orientation",
                "landscape",
                "--size",
                "large",
                "--color",
                "blue",
                "--per-page",
                "2",
            ]),
    );

    search_mock.assert();
    assert_eq!(payload["query"], "mountains");
    assert_eq!(payload["total_results"], 2);
    assert_eq!(payload["photos"][0]["id"], 1001);
}

#[test]
fn download_by_id_fetches_photo_and_saves_selected_quality() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let output_dir = dir.path().join("downloads");
    let server = MockServer::start();
    let download_url = server.url("/files/1001-large2x.jpeg");

    let photo_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/photos/1001")
            .header("authorization", "test-key");
        then.status(200).json_body(json!({
            "id": 1001,
            "width": 4000,
            "height": 3000,
            "url": "https://www.pexels.com/photo/mountain-1001/",
            "photographer": "Ada",
            "src": {
                "original": server.url("/files/1001-original.jpeg"),
                "large2x": download_url
            }
        }));
    });

    let file_mock = server.mock(|when, then| {
        when.method(GET).path("/files/1001-large2x.jpeg");
        then.status(200).body("image-bytes");
    });

    let payload = parse_stdout_json(
        command_with_config(&config_path)
            .env("PEXELS_API_KEY", "test-key")
            .env("PEXELS_AGENT_API_BASE", server.base_url())
            .args([
                "download",
                "--id",
                "1001",
                "--quality",
                "large2x",
                "--output-dir",
                output_dir.to_str().unwrap(),
            ]),
    );

    photo_mock.assert();
    file_mock.assert();
    assert_eq!(payload["photo_id"], 1001);
    let saved_to = payload["saved_to"].as_str().unwrap();
    assert!(saved_to.ends_with("1001-large2x.jpeg"));
    assert_eq!(fs::read(saved_to).unwrap(), b"image-bytes");
}

#[test]
fn download_first_searches_then_downloads_first_match() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let output_dir = dir.path().join("downloads");
    let server = MockServer::start();
    let download_url = server.url("/files/1001-original.jpeg");

    let search_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/search")
            .header("authorization", "test-key")
            .query_param("query", "mountains")
            .query_param("page", "1")
            .query_param("per_page", "15");
        then.status(200).json_body(json!({
            "page": 1,
            "per_page": 15,
            "photos": [
                {
                    "id": 1001,
                    "width": 4000,
                    "height": 3000,
                    "url": "https://www.pexels.com/photo/mountain-1001/",
                    "photographer": "Ada",
                    "src": {
                        "original": download_url,
                        "large2x": server.url("/files/1001-large2x.jpeg")
                    }
                }
            ]
        }));
    });

    let file_mock = server.mock(|when, then| {
        when.method(GET).path("/files/1001-original.jpeg");
        then.status(200).body("first-image");
    });

    let payload = parse_stdout_json(
        command_with_config(&config_path)
            .env("PEXELS_API_KEY", "test-key")
            .env("PEXELS_AGENT_API_BASE", server.base_url())
            .args([
                "download-first",
                "--query",
                "mountains",
                "--quality",
                "original",
                "--output-dir",
                output_dir.to_str().unwrap(),
            ]),
    );

    search_mock.assert();
    file_mock.assert();
    assert_eq!(payload["photo_id"], 1001);
    assert_eq!(payload["query"], "mountains");
    let saved_to = payload["saved_to"].as_str().unwrap();
    assert_eq!(fs::read(saved_to).unwrap(), b"first-image");
}
