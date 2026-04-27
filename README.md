# pexfetch

Agent-friendly Rust CLI for Pexels image **and video** search and
downloads. Every command emits machine-readable JSON on stdout; errors
come back as JSON on stderr with a stable `kind` and a distinct exit
code, so an LLM or automation script can branch without regex.

## Install

Build and install the binary into Cargo's bin directory:

```bash
cargo install --path .
```

Or run straight from the repo during development:

```bash
cargo build
cargo run -- status
```

Requires Rust **1.86+** (edition 2024, transitive `icu_*` MSRV). The pinned toolchain lives in
[`rust-toolchain.toml`](./rust-toolchain.toml).

## Quickstart

```bash
pexfetch auth login                 # paste your API key from stdin
pexfetch status                     # check auth + live connectivity
pexfetch search --query mountains --per-page 3
pexfetch download --id 1001 --quality large2x --output-dir ./downloads
```

## Commands

```bash
pexfetch status
pexfetch auth status
pexfetch auth login --api-key your_api_key_here   # ⚠ see 'Secrets' below
pexfetch auth login                               # prompts on stdin
pexfetch auth logout

pexfetch search --query mountains --per-page 3
pexfetch download --id 1001 --quality large2x --output-dir ./downloads
pexfetch download-first --query mountains --output-dir ./downloads
```

### Videos

Videos live under their own subcommand tree because the Pexels API
exposes a separate `/v1/videos/` endpoint with different response shape
(array of `video_files` instead of photo's named `src.original`,
`src.large2x`, …).

```bash
pexfetch videos search --query "drone sunset" --per-page 5
pexfetch videos download --id 5000 --quality hd --output-dir ./clips
pexfetch videos download --id 5000 --video-file-id 90002 --output-dir ./clips
pexfetch videos download-first --query "drone sunset" --output-dir ./clips
```

`--quality` is one of `hd | sd | hls` (default `hd`). Within a bucket
the entry with the highest `width * fps` wins. `--video-file-id N`
bypasses `--quality` and picks that exact entry from
`video_files[]` — useful when a video has several variants at the
same quality. File extension on disk is derived from the upstream
`file_type` (`video/mp4` → `.mp4`, `video/webm` → `.webm`,
`video/quicktime` → `.mov`, HLS mime types → `.m3u8`).

## Auth & secrets

Resolution order for the API key:

1. `PEXELS_API_KEY` environment variable
2. Stored config file

Running `pexfetch auth login` without `--api-key` prints the official
API-key page, waits for you to paste the key into stdin, and saves it to
the config file. The file is written atomically with mode `0600` on Unix.

> ⚠ Passing `--api-key` as a flag leaks the key into `ps auxww`, shell
> history, and audit logs. Prefer `PEXELS_API_KEY` or the interactive
> prompt. The CLI prints a warning when it sees `--api-key`.

`pexfetch status` returns JSON with the auth source, configured
state, API base URL, and whether the API is reachable.

Default config file path:

```text
~/.config/pexfetch/config.json
```

## Configuration

All optional, read at command start:

| Env var                            | Default                  | Purpose                                                              |
| ---------------------------------- | ------------------------ | -------------------------------------------------------------------- |
| `PEXELS_API_KEY`                   | —                        | Overrides the stored config file.                                    |
| `PEXFETCH_CONFIG_PATH`         | `~/.config/pexfetch/config.json` (or `$XDG_CONFIG_HOME/pexfetch/config.json`) | Override where the credential is stored. Used by tests.              |
| `PEXFETCH_API_BASE`            | `https://api.pexels.com` | Point at a different base URL. **Must be `https://`**; `http://` is allowed only for loopback hosts (local mocks). |
| `PEXFETCH_HTTP_TIMEOUT_MS`     | `60000`                  | Total HTTP timeout (applies to both search/photo calls and downloads). |
| `PEXFETCH_DOWNLOAD_MAX_BYTES`  | `209715200` (200 MiB)    | Hard cap on download size. A partial file over the cap is deleted. |

## Error output

Errors print a single-line JSON object on stderr and the process exits
with a category-specific code. Successful commands still print JSON on
stdout as before.

```json
{"ok": false, "error": {"kind": "rate_limited", "message": "rate limited by Pexels (retry after 42s)", "retry_after_secs": 42, "remaining": 0, "reset_at": 1714000000}}
```

Exit-code map:

| Code | Kind                                                         | Meaning                                      |
| ---: | ------------------------------------------------------------ | -------------------------------------------- |
|    0 | —                                                            | Success.                                     |
|    1 | `error`, `io_error`, `json_error`, `url_error`               | Generic / filesystem / JSON / URL failure.   |
|    2 | — (from clap)                                                | Usage error (unknown flag, missing arg).     |
|    3 | `missing_credential`, `unauthorized`, `forbidden`            | Auth problem — set `PEXELS_API_KEY` or login.|
|    4 | `not_found`, `invalid_quality`                               | Resource or argument does not match.         |
|    5 | `http_error`                                                 | Network / connection failure.                |
|    6 | `rate_limited`                                               | 429 from Pexels. Payload exposes `retry_after_secs`. |

## Skills

Repository copies of the global agent skills that drive this CLI:

- Claude Code: [`skills/claude/pexels-images/`](./skills/claude/pexels-images/)
- Codex: [`skills/codex/pexels-images/`](./skills/codex/pexels-images/)

See [`skills/README.md`](./skills/README.md) for the sync policy between
the two host-specific copies.

## Contributing

Local checks before opening a PR:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs the same plus `cargo build --release --locked` and a
`rustsec/audit-check` pass against `Cargo.lock`.

Tests are integration-level and hit a local `httpmock` server; no real
network is required.

## License

Dual-licensed under **MIT** ([LICENSE-MIT](./LICENSE-MIT)) **OR
Apache-2.0** ([LICENSE-APACHE](./LICENSE-APACHE)) at your option.
