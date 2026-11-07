# Security policy

## Reporting a vulnerability

Please report security problems privately through GitHub's **Security → Report a vulnerability**
form on this repository (private vulnerability reporting). Do not open a public issue for
anything that involves credentials, a private server address, or a way to make the app leak either.

Include the version or commit, the distribution, what an attacker would gain and the smallest
reproduction you can. A first reply is a personal-effort best, usually within a week.

## What is in scope

- The app and the three workspace crates (`runefin`, `jellyfin-api`, `seerr-api`).
- The Flatpak packaging in `packaging/`.
- The pinned Rune fork, for changes made in the fork's `runefin/player` branch only; upstream Rune
  issues belong to [anomalyco/rune](https://github.com/anomalyco/rune).

## The security model

Runefin is a **client for one Jellyfin server and one user**, plus an optional Seerr instance. It
is not a server, a proxy, a multi-tenant system or a sync service. Anyone who can read the
config file can act as that user, so:

- The session is written to `~/.config/runefin/config.toml` with mode `0600` (inside a Flatpak,
  `~/.var/app/io.github.lifk.Runefin/config/runefin/config.toml`). The token is wiped from the
  heap on sign-out with `zeroize`.
- Error messages are redacted before they reach a screen or a log: any `api_key`/`token` query
  value is replaced with `<redacted>` (`jellyfin_api::client::redact`).
- Jellyfin's API takes the token in a header for API calls and in the stream URL for direct
  playback. Stream URLs therefore appear in process arguments when playback is handed to `mpv`,
  and can appear in a crash log or a trace. Treat a bug report, log or screenshot as potentially
  sensitive and redact the token before sharing it.
- `yt-dlp`, `ffmpeg` and `mpv` are optional external programs. When used, they run as your user
  and inherit the URLs they are given.

## Supported versions

Only the latest commit on `main` and the newest release receive fixes. There are no long-term
support branches.

## Before you post

Remove from any report: server URLs, tokens, API keys, library contents, user names and file
paths that identify you. The project never needs your real token to reproduce a bug.
