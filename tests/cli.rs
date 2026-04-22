use std::fs;

use assert_cmd::Command;
use httpmock::Method::GET;
use httpmock::MockServer;
use serde_json::{Value, json};
use tempfile::tempdir;

fn command_with_config(config_path: &std::path::Path) -> Command {
    let mut command = Command::cargo_bin("pexfetch").expect("binary exists");
    command.env("PEXFETCH_CONFIG_PATH", config_path);
    command
}

fn parse_stdout_json(command: &mut Command) -> Value {
    let output = command.assert().success().get_output().stdout.clone();
    serde_json::from_slice(&output).expect("stdout is valid json")
}

#[test]
fn root_help_mentions_status_and_auth_flows() {
    let mut command = Command::cargo_bin("pexfetch").expect("binary exists");
    let assert = command.arg("--help").assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("Authenticate, inspect config state, or remove saved credentials"));
    assert!(stdout.contains("Check configured auth and live API connectivity"));
    assert!(stdout.contains("Search Pexels photos and return JSON results"));
}

#[test]
fn status_help_mentions_api_connectivity_check() {
    let mut command = Command::cargo_bin("pexfetch").expect("binary exists");
    let assert = command.args(["status", "--help"]).assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("Check configured auth and live API connectivity"));
    assert!(stdout.contains("Usage: pexfetch status"));
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
fn search_times_out_when_api_is_slow() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/v1/search");
        then.status(200)
            .delay(std::time::Duration::from_secs(3))
            .json_body(json!({ "page": 1, "per_page": 15, "photos": [] }));
    });

    let mut command = command_with_config(&config_path);
    let assert = command
        .env("PEXELS_API_KEY", "test-key")
        .env("PEXFETCH_API_BASE", server.base_url())
        .env("PEXFETCH_HTTP_TIMEOUT_MS", "300")
        .args(["search", "--query", "slow"])
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone())
        .unwrap()
        .to_lowercase();

    assert!(
        stderr.contains("timed out") || stderr.contains("timeout"),
        "expected timeout error, got: {stderr:?}"
    );
}

#[test]
fn download_fails_when_body_exceeds_limit() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let output_dir = dir.path().join("downloads");
    let server = MockServer::start();
    let download_url = server.url("/files/big.jpeg");

    server.mock(|when, then| {
        when.method(GET).path("/v1/photos/1001");
        then.status(200).json_body(json!({
            "id": 1001,
            "width": 10,
            "height": 10,
            "url": "https://www.pexels.com/photo/1001/",
            "photographer": "Ada",
            "src": { "original": download_url }
        }));
    });

    server.mock(|when, then| {
        when.method(GET).path("/files/big.jpeg");
        then.status(200)
            .body("this body is way too big for the cap");
    });

    let mut command = command_with_config(&config_path);
    let assert = command
        .env("PEXELS_API_KEY", "test-key")
        .env("PEXFETCH_API_BASE", server.base_url())
        .env("PEXFETCH_DOWNLOAD_MAX_BYTES", "8")
        .args([
            "download",
            "--id",
            "1001",
            "--quality",
            "original",
            "--output-dir",
            output_dir.to_str().unwrap(),
        ])
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();

    assert!(
        stderr.contains("exceeds"),
        "expected size-limit error, got: {stderr:?}"
    );

    let big_file = output_dir.join("1001-original.jpeg");
    assert!(
        !big_file.exists(),
        "oversized download must not leave a partial file on disk"
    );
}

fn stderr_json(assert: &assert_cmd::assert::Assert) -> Value {
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    // The error payload is a single-line JSON object emitted last. Some
    // commands (e.g. `auth login`) print interactive prompts without
    // trailing newlines, so the JSON can be appended to the prompt line.
    // Scan lines in reverse; within each line take the tail starting at
    // the first '{' and try to parse.
    for line in stderr.lines().rev() {
        if let Some(brace) = line.find('{') {
            if let Ok(value) = serde_json::from_str::<Value>(line[brace..].trim_end()) {
                return value;
            }
        }
    }
    panic!("stderr has no JSON object, got: {stderr:?}")
}

#[test]
fn search_maps_http_401_to_unauthorized_error() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/v1/search");
        then.status(401).body("");
    });

    let mut command = command_with_config(&config_path);
    let assert = command
        .env("PEXELS_API_KEY", "bad-key")
        .env("PEXFETCH_API_BASE", server.base_url())
        .args(["search", "--query", "x"])
        .assert()
        .code(3);
    let payload = stderr_json(&assert);

    assert_eq!(payload["ok"], false);
    assert_eq!(payload["error"]["kind"], "unauthorized");
    assert!(
        payload["error"]["message"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("401")
    );
}

#[test]
fn search_maps_http_429_to_rate_limited_error() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/v1/search");
        then.status(429)
            .header("x-ratelimit-limit", "200")
            .header("x-ratelimit-remaining", "0")
            .header("x-ratelimit-reset", "9999999999")
            .body("");
    });

    let mut command = command_with_config(&config_path);
    let assert = command
        .env("PEXELS_API_KEY", "test-key")
        .env("PEXFETCH_API_BASE", server.base_url())
        .args(["search", "--query", "x"])
        .assert()
        .code(6);
    let payload = stderr_json(&assert);

    assert_eq!(payload["error"]["kind"], "rate_limited");
    assert_eq!(payload["error"]["remaining"], 0);
    assert_eq!(payload["error"]["reset_at"], 9_999_999_999_u64);
}

#[test]
fn download_first_maps_empty_result_to_not_found() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let output_dir = dir.path().join("downloads");
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/v1/search");
        then.status(200)
            .json_body(json!({ "page": 1, "per_page": 15, "photos": [] }));
    });

    let mut command = command_with_config(&config_path);
    let assert = command
        .env("PEXELS_API_KEY", "test-key")
        .env("PEXFETCH_API_BASE", server.base_url())
        .args([
            "download-first",
            "--query",
            "nothing",
            "--output-dir",
            output_dir.to_str().unwrap(),
        ])
        .assert()
        .code(4);
    let payload = stderr_json(&assert);

    assert_eq!(payload["error"]["kind"], "not_found");
    assert!(
        payload["error"]["message"]
            .as_str()
            .unwrap()
            .contains("nothing")
    );
}

#[test]
fn download_rejects_unknown_quality_at_parse_time() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let output_dir = dir.path().join("downloads");

    command_with_config(&config_path)
        .env("PEXELS_API_KEY", "test-key")
        .args([
            "download",
            "--id",
            "1",
            "--quality",
            "bogus",
            "--output-dir",
            output_dir.to_str().unwrap(),
        ])
        .assert()
        .code(2);
}

#[test]
fn download_missing_quality_in_photo_emits_available_list() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let output_dir = dir.path().join("downloads");
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/v1/photos/1001");
        then.status(200).json_body(json!({
            "id": 1001,
            "width": 10,
            "height": 10,
            "url": "https://www.pexels.com/photo/1001/",
            "photographer": "Ada",
            "src": {
                "original": "https://example.test/1001.jpeg",
                "large2x": "https://example.test/1001-large2x.jpeg"
            }
        }));
    });

    let mut command = command_with_config(&config_path);
    let assert = command
        .env("PEXELS_API_KEY", "test-key")
        .env("PEXFETCH_API_BASE", server.base_url())
        .args([
            "download",
            "--id",
            "1001",
            "--quality",
            "medium",
            "--output-dir",
            output_dir.to_str().unwrap(),
        ])
        .assert()
        .code(4);
    let payload = stderr_json(&assert);

    assert_eq!(payload["error"]["kind"], "invalid_quality");
    let available: Vec<String> = payload["error"]["available_qualities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    assert!(available.contains(&"original".to_owned()));
    assert!(available.contains(&"large2x".to_owned()));
}

#[test]
fn missing_api_key_exits_with_auth_code() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");

    let mut command = command_with_config(&config_path);
    let assert = command.args(["search", "--query", "x"]).assert().code(3);
    let payload = stderr_json(&assert);
    assert_eq!(payload["error"]["kind"], "missing_credential");
}

#[test]
fn auth_status_gives_actionable_error_on_corrupt_config() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    fs::write(&config_path, "{not json").unwrap();

    let mut command = command_with_config(&config_path);
    let assert = command.args(["auth", "status"]).assert().code(1);
    let payload = stderr_json(&assert);
    let message = payload["error"]["message"].as_str().unwrap();

    assert!(
        message.contains(config_path.to_str().unwrap()),
        "error must name the corrupt path, got: {message:?}"
    );
    assert!(
        message.contains("auth logout"),
        "error must suggest `auth logout`, got: {message:?}"
    );
}

#[test]
fn search_reports_upstream_total_results() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/v1/search");
        then.status(200).json_body(json!({
            "page": 1,
            "per_page": 2,
            "total_results": 8000,
            "photos": [
                { "id": 1, "src": {} },
                { "id": 2, "src": {} }
            ]
        }));
    });

    let payload = parse_stdout_json(
        command_with_config(&config_path)
            .env("PEXELS_API_KEY", "test-key")
            .env("PEXFETCH_API_BASE", server.base_url())
            .args(["search", "--query", "x", "--per-page", "2"]),
    );

    assert_eq!(payload["total_results"], 8000);
}

#[test]
fn search_maps_http_403_to_forbidden() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/v1/search");
        then.status(403).body("");
    });

    let mut command = command_with_config(&config_path);
    let assert = command
        .env("PEXELS_API_KEY", "no-scope")
        .env("PEXFETCH_API_BASE", server.base_url())
        .args(["search", "--query", "x"])
        .assert()
        .code(3);
    let payload = stderr_json(&assert);

    assert_eq!(payload["error"]["kind"], "forbidden");
}

#[test]
fn search_surfaces_malformed_json_response() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/v1/search");
        then.status(200).body("not json at all");
    });

    let mut command = command_with_config(&config_path);
    let assert = command
        .env("PEXELS_API_KEY", "test-key")
        .env("PEXFETCH_API_BASE", server.base_url())
        .args(["search", "--query", "x"])
        .assert()
        .code(5);
    let payload = stderr_json(&assert);

    assert_eq!(payload["error"]["kind"], "http_error");
}

#[test]
fn env_api_key_wins_over_stored_config() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    fs::write(&config_path, json!({ "api_key": "stored-key" }).to_string()).unwrap();

    let payload = parse_stdout_json(
        command_with_config(&config_path)
            .env("PEXELS_API_KEY", "env-key")
            .args(["auth", "status"]),
    );

    assert_eq!(payload["configured"], true);
    assert_eq!(payload["source"], "env");
}

#[test]
fn xdg_config_home_is_used_when_config_path_unset() {
    let xdg = tempdir().unwrap();
    let home = tempdir().unwrap();

    let mut command = Command::cargo_bin("pexfetch").expect("binary exists");
    command.env_remove("PEXFETCH_CONFIG_PATH");
    command.env("XDG_CONFIG_HOME", xdg.path());
    command.env("HOME", home.path());
    command
        .args(["auth", "login", "--api-key", "xdg-key"])
        .assert()
        .success();

    let expected = xdg.path().join("pexfetch").join("config.json");
    assert!(
        expected.exists(),
        "config must land at {}",
        expected.display()
    );
    let stored: Value = serde_json::from_str(&fs::read_to_string(&expected).unwrap()).unwrap();
    assert_eq!(stored["api_key"], "xdg-key");
}

#[test]
fn auth_login_empty_stdin_fails_with_missing_credential() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");

    let mut command = command_with_config(&config_path);
    command.args(["auth", "login"]).write_stdin("\n");
    let assert = command.assert().code(3);
    let payload = stderr_json(&assert);

    assert_eq!(payload["error"]["kind"], "missing_credential");
}

#[test]
fn auth_logout_without_saved_config_reports_removed_false() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");

    let payload = parse_stdout_json(command_with_config(&config_path).args(["auth", "logout"]));

    assert_eq!(payload["configured"], false);
    assert_eq!(payload["removed"], false);
}

#[test]
fn auth_status_treats_whitespace_config_key_as_unconfigured() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    fs::write(&config_path, json!({ "api_key": "   " }).to_string()).unwrap();

    let payload = parse_stdout_json(command_with_config(&config_path).args(["auth", "status"]));

    assert_eq!(payload["configured"], false);
    assert_eq!(payload["source"], "none");
}

#[test]
fn auth_login_overwrites_existing_config() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");

    command_with_config(&config_path)
        .args(["auth", "login", "--api-key", "first"])
        .assert()
        .success();
    command_with_config(&config_path)
        .args(["auth", "login", "--api-key", "second"])
        .assert()
        .success();

    let stored: Value = serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    assert_eq!(stored["api_key"], "second");
}

#[test]
fn search_rejects_non_https_api_base() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");

    let mut command = command_with_config(&config_path);
    let assert = command
        .env("PEXELS_API_KEY", "test-key")
        .env("PEXFETCH_API_BASE", "http://evil.example.com")
        .args(["search", "--query", "mountains"])
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();

    assert!(
        stderr.contains("https"),
        "error must mention https, got: {stderr:?}"
    );
    assert!(
        stderr.contains("PEXFETCH_API_BASE"),
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
            .env("PEXFETCH_API_BASE", server.base_url())
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
            .env("PEXFETCH_API_BASE", server.base_url())
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
            .env("PEXFETCH_API_BASE", server.base_url())
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
            .env("PEXFETCH_API_BASE", server.base_url())
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
