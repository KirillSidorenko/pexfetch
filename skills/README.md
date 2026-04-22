# Bundled agent skills

Two host-specific skill packages that drive the `pexfetch` CLI from
inside an AI coding agent:

| Host         | Path                                   | Format                         |
| ------------ | -------------------------------------- | ------------------------------ |
| Claude Code  | `claude/pexels-images/SKILL.md`        | frontmatter: `user-invocable`, `argument-hint`, `allowed-tools` |
| OpenAI Codex | `codex/pexels-images/SKILL.md` + `agents/openai.yaml` | frontmatter: `name` + `description`; YAML manifest for Codex agent interface |

## What these are

The bodies are ~95 % identical — same Rules / Auth / Search / Download /
Workflow sections. They only diverge in the frontmatter required by each
host's skill loader, and in a few wording tweaks to match each host's
conventions.

## When to update

Treat the two files as a mirrored pair. When you change one, re-read the
other and apply the same change. A future refactor could extract a shared
body and generate the host-specific wrappers; until then, keep the manual
sync tight.

## When to use

These copies exist so the repo can be cloned into a user's agent
configuration (for example by copying `skills/claude/pexels-images/` into
`~/.claude/skills/`). They are not built or packaged by `cargo`; they are
repo artifacts, not Rust source.
