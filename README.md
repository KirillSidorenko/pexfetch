# pexels-agent-cli

Agent-friendly Rust CLI for Pexels image search and downloads.

## Build

```bash
cargo build
```

Run directly from the repo:

```bash
cargo run -- auth status
cargo run -- status
cargo run -- search --query mountains
```

Install the binary into Cargo's bin directory:

```bash
cargo install --path .
```

## Commands

```bash
pexels-agent status
pexels-agent auth status
pexels-agent auth login --api-key your_api_key_here
pexels-agent auth login
pexels-agent auth logout

pexels-agent search --query mountains --per-page 3
pexels-agent download --id 1001 --quality large2x --output-dir ./downloads
pexels-agent download-first --query mountains --output-dir ./downloads
```

## Auth

The CLI resolves the API key in this order:

1. `PEXELS_API_KEY`
2. Stored config file

If you run `pexels-agent auth login` without `--api-key`, the CLI prints the official Pexels API key page, waits for you to paste the key into stdin, and then saves it to the config file.

Use `pexels-agent status` to verify both credential resolution and live API connectivity. It returns JSON with the auth source, configured state, API base URL, and whether the API is reachable.

The default config file path is:

```bash
~/.config/pexels-agent/config.json
```

You can override it for scripts or tests with:

```bash
PEXELS_AGENT_CONFIG_PATH=/custom/path/config.json
```

## Skills

Repository copies of the global skills live here:

- Codex: `skills/codex/pexels-images/`
- Claude Code: `skills/claude/pexels-images/`
