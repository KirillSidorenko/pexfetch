---
name: pexels-images
description: Use when the task is to search, shortlist, or download stock and reference images from Pexels through the installed `pexfetch` CLI. Trigger for requests to find Pexels photos by query, color, orientation, or size; download a chosen image or the first good match; or authenticate the local Pexels CLI and save the API key.
---

# Pexels Images

Use `pexfetch` only. Do not scrape Pexels HTML or call the raw API directly when the CLI can do the work.

## Rules

- Check `command -v pexfetch` before relying on the CLI. If it is missing, stop and tell the user the local Pexels CLI is not installed.
- Keep machine-readable JSON intact; parse it instead of screen-scraping terminal output.
- Do not print or persist the API key anywhere except the CLI's built-in auth store.
- Do not batch-download broad result sets unless the user explicitly asks.

## Auth

- Run `pexfetch status` before the first Pexels action if auth or API health is unknown.
- Use `pexfetch auth status` only when you need config-source details without touching the API.
- If auth is missing, run `pexfetch auth login`.
- The login flow prints the official API key page, waits for the pasted key, and stores it in `~/.config/pexfetch/config.json`.
- `PEXELS_API_KEY` overrides the stored config.
- `pexfetch status` returns JSON with `configured`, `source`, `api_base`, `api_reachable`, and `api_error`.

## Search

- Basic search:
  `pexfetch search --query "brutalist living room" --per-page 5`
- Add filters as needed:
  `--orientation landscape|portrait|square`
  `--size large|medium|small`
  `--color blue|green|brown|...`
  `--locale en-US`
- Search returns JSON. Inspect `photos[].id`, `photos[].photographer`, `photos[].url`, and `photos[].src`.
- Prefer `jq` for deterministic selection:
  `pexfetch search --query "brutalist living room" --per-page 5 | jq '.photos[] | {id, photographer, url, original: .src.original}'`

## Download

- Download a selected image by photo ID:
  `pexfetch download --id 123456 --quality original --output-dir /absolute/output/dir`
- Download the first acceptable match only when the user explicitly wants a quick default:
  `pexfetch download-first --query "brutalist living room" --quality original --output-dir /absolute/output/dir`
- Prefer `original` unless the user asks for a smaller or faster file.
- After download, return the saved path and the selected photo metadata that matters for the task.

## Workflow

1. Confirm auth and API reachability with `pexfetch status` if needed.
2. Search with the user's query and any requested filters.
3. Let the user choose an image when multiple distinct options matter.
4. Use `download-first` only when the user wants an automatic best-first default.
5. Download to a user-specified directory. If none is given, create a temporary directory with `mktemp -d` and report the exact saved path.
6. Verify the file exists with `ls -l` or `file` when the saved artifact matters.
