# Video support — implementation plan

Status: done (2026-04-23) — all five TDD cycles merged.

## Why

Photos only covered half the Pexels catalogue. Agents asking for
“drone footage of a sunset” need to download MP4s the same way they
download JPEGs today. Pexels exposes a parallel `/v1/videos/` API with
its own shape, so the work is additive rather than a rewrite.

## API reference (as of 2026-04-23)

Endpoints:

| Method | Path                             | Purpose                       |
| ------ | -------------------------------- | ----------------------------- |
| GET    | `/v1/videos/search`              | Search videos by query        |
| GET    | `/v1/videos/popular`             | Trending videos (v2, not now) |
| GET    | `/v1/videos/videos/:id`          | One video by id               |

Auth / rate limit / pagination are identical to `/v1/`. Same
`Authorization` header, same `X-Ratelimit-*` headers, same 429 handling.

`Video` object (relevant fields):

```
id, width, height, url, image, duration, user{id,name,url},
video_files: [{ id, quality: "hd"|"sd"|"hls", file_type, width, height, fps, link }],
video_pictures: [{ id, picture, nr }]
```

`videos/search` response wraps it in
`{ videos, url, page, per_page, total_results, prev_page, next_page }`.

## Command surface

```
pexfetch videos search      --query ... [--orientation L|P|S]
                                        [--size large|medium|small]
                                        [--locale xx-YY]
                                        [--per-page N] [--page N]

pexfetch videos download    --id N --output-dir ...
                            [--quality hd|sd|hls]        # default: hd
                            [--video-file-id M]          # escape hatch; overrides --quality

pexfetch videos download-first --query ... --output-dir ...
                               [same quality/filter flags]
```

No top-level `videos` variant added to existing `search` / `download` —
photo commands stay untouched, video commands live under their own
`videos` namespace. Zero breakage of scripts written against the photo
API.

Deferred to v2: `pexfetch videos popular`.

## Quality selection

`--quality` is a `ValueEnum { Hd, Sd, Hls }` (mirror of the photo
`Quality` enum). Default `hd`.

Auto-pick within a quality bucket: highest (`width * fps`) wins. Ties
go to the first match in the upstream order.

`--video-file-id` is an escape hatch: when set it bypasses `--quality`
entirely and picks the `video_files[]` entry with that exact `id`.

Error shapes:

- bucket empty → `AppError::InvalidQuality { quality: "sd",
  available: <the set of buckets that DO have files on this video> }`
  (reusing the existing variant and JSON payload shape)
- `--video-file-id M` not in `video_files[]` → `NotFound` with the id

## Types (new, all in `src/models.rs`)

```rust
pub struct Video           { id, width, height, url, image, duration,
                             user, video_files, video_pictures }
pub struct VideoUser       { id, name, url }
pub struct VideoFile       { id, quality, file_type, width, height,
                             fps, link }
pub struct VideoPicture    { id, picture, nr }
pub struct VideosSearchResponse
                           { page, per_page, total_results, videos,
                             next_page, prev_page, url }

pub struct VideoSearchPayload   { query, page, per_page, total_results,
                                  next_page, videos }
pub struct VideoDownloadPayload { video_id, quality, video_file_id,
                                  query: Option<String>, saved_to,
                                  source_url, file_type }
```

`PexelsClient` gains:

- `search_videos(&VideoSearchRequest) -> VideosSearchResponse`
- `get_video(id: u64) -> Video`

`download_file()` is reused as-is (just HTTP + byte-cap).

## File extension on disk

Derive from `video_files[].file_type` (e.g. `video/mp4 -> .mp4`).
Fallback to `.mp4` if `file_type` is missing or unrecognised. URL
extension is unreliable because Pexels serves via CDN with opaque
paths.

## TDD cycles

| # | Red test (tests/cli.rs)                                                     | Green                                                                    |
|---|-----------------------------------------------------------------------------|--------------------------------------------------------------------------|
| 1 | `videos_search_prints_machine_readable_json`                                | `Video*` types, `search_videos`, `videos search` subcommand              |
| 2 | `videos_download_by_id_saves_highest_quality_file`                          | `get_video`, `videos download`, quality picker, extension from file_type |
| 3 | `videos_download_with_explicit_file_id_overrides_quality`                   | `--video-file-id` flag plumbed through                                   |
| 4 | `videos_download_first_searches_then_downloads_first_match`                 | `videos download-first` subcommand                                       |
| 5 | `videos_download_unknown_quality_emits_available_list` + NotFound variants  | InvalidQuality construction, NotFound for bad file-id / 404 on video id  |

Each cycle lands as its own commit so `git log` stays readable.

## Not in scope

- `videos popular` (v2)
- Streaming HLS playback / m3u8 handling — we download the `.m3u8`
  manifest file if asked, nothing fancier.
- Thumbnail download from `video_pictures[]`.
- Updating the bundled skills (`skills/claude/pexels-images/SKILL.md`
  and `skills/codex/pexels-images/SKILL.md`) — follow-up once the
  command surface is stable and merged.

## Checklist before commit sequence is done

- [x] All 5 TDD cycles green (40 integration tests total, +7 new)
- [x] `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` all clean
- [x] README gains a "Videos" section under Commands
- [x] `docs/videos.md` (this file) flipped to Status: done

Follow-ups punted to a later PR:
- `pexfetch videos popular` (the only endpoint not yet wrapped)
- Updating `skills/claude/pexels-images/` and `skills/codex/pexels-images/`
  to teach the new `videos …` subtree
