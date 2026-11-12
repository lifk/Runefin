# What the ecosystem does outside the app — 2026-11-12

Question: which features does a Jellyfin user normally get from a *separate* tool, and which of
those can Runefin do inside itself, the way it now syncs subtitles (Bazarr/ffsubsync's job)?

Method: the Jellyfin 12.4 API surface the app already owns (`crates/jellyfin-api`), what
`docs/features.md` ships today, and the external tools in the surrounding ecosystem — Bazarr,
ffsubsync, the Intro Skipper / Media Segments plugins, SponsorBlock-for-Jellyfin, Tautulli and
the Playback Reporting plugin, Trakt's plugin and scripts, Jellyfin Media Cleaner, Kometa,
Filebot, jfa-go, and the server's own web client for SyncPlay.

The filter is the same one the subtitle sync passed: the tool exists because the *server* cannot
or will not do the work, and the work is within a client's reach — its own decoding, its own
cache, one item's timeline — rather than an admin's (file naming, acquisition, metadata writes,
user provisioning).

## The survey

| Outside the app | What it does there | In-app version | API / data it needs | Verdict |
|---|---|---|---|---|
| **Bazarr** | searches and downloads subtitles for a whole library on a schedule | **Fetch missing subtitles for a series, season or library**, in the reader's language, one item at a time, with the existing search and download | `has_subtitles` / `missing_only` on `ItemQuery`, `episodes_in`, `search_subtitles`, `download_subtitle` — all present | **Strong**: the app already has every call; only the loop and a progress row are missing |
| **ffsubsync / ffsubsync-web** | measures and applies a constant offset (and a rate) | shipped: **Sync to audio**; the *drift* fit is the half not done | the probe and correlation already exist; a third window and a line fit | Next increment, medium |
| **Intro Skipper / Media Segments plugins** | detects intros and credits from audio fingerprints across episodes, then the server skips them | **detect them locally**: decode the first minutes of two episodes of a series, correlate, store the common span, and offer it as a skip range (the app already skips segments) | the audio probe and correlation, plus a local per-series store | Ambitious but genuinely ours: the machinery is built; a wrong skip is the risk |
| **SponsorBlock for Jellyfin** | fetches community skip ranges by hashing the file | fetch ranges by a hash of the item's media, cache them, offer them like intro/credits segments | an external service keyed by file hash; no Jellyfin API | Possible, but depends on a service whose data is thin for private libraries |
| **Tautulli / Playback Reporting** | long-term viewing statistics per user, per library | a **stats screen**: hours, top series, streaks, per-month, computed from the app's own `history.jsonl` | local only | Cheap and pleasant; the app already writes the log |
| **Trakt plugin / scripts** | scrobbles plays to Trakt | scrobble from the player's own lifecycle | Trakt OAuth + API; a real external account | Medium; adds an external dependency to an app that has none |
| **Jellyfin Media Cleaner** | finds missing subtitles, missing artwork, duplicates and bad metadata | a **needs-attention view** per library: missing subtitles, no primary image, duplicate names, unaired episodes | `ItemQuery` filters already in the client | Cheap; the flags are already there (`missing_only`, `unaired_only`, `has_subtitles`) |
| **ReplayGain plugin / "loudness" scripts** | writes per-track gain into tags | measure a film's loudness with the audio probe once, store the gain locally, apply it to the player's volume | the probe, plus a loudness measure (RMS/EBU-lite) and a cache | Medium; genuinely useful at night, and the ears are already written |
| **jellyfin-mpv-shim / "open in player"** | hands a stream to a desktop player | the app already has an mpv handoff | — | Done |
| **SyncPlay (web client)** | watch together, over a WebSocket | polling the group's state | Jellyfin's SyncPlay needs a WebSocket for group timing | Excluded: the app has no WebSocket, by design |
| **Kometa / Plex-Meta-Manager** | builds collections and metadata from external lists | create collections from a selection (already shipped); metadata writes are server-side | collection endpoints only | Partial, as far as a client can go |
| **Filebot, jfa-go, sonarr/radarr** | renaming, users, acquisition | — | file system, admin, download clients | Out of scope for a client |

## What I would take, in order

1. **Fetch missing subtitles for a series or season** (Bazarr's core). The app has search,
   download, language and the episode list; the missing piece is a loop that skips items that
   already have a track and reports what it added. A series of 24 episodes at ~1 s per provider
   search is a minute of work on a worker, and it turns the one-item panel row into something a
   reader can leave running. Effort: S–M.
2. **A needs-attention view** (Media Cleaner). `missing_only`, `unaired_only` and `has_subtitles`
   are already query flags; the screen is a filtered grid with counts, and the app already has a
   "Needs attention" chip for missing/unaired in the library header. Extending it to subtitles and
   artwork is mostly wiring. Effort: S.
3. **A stats screen** (Tautulli). The history file already has one line per completed play; a
   screen that groups it by month, series and weekday is local, private, and needs no server
   plugin. Effort: S–M.
4. **Loudness normalisation** (ReplayGain). The probe is the expensive part and it exists; the
   gain is a cached number per item applied to the player's volume, with the existing
   downmix/dialogue/night modes beside it. Effort: M.
5. **Local intro detection** (Intro Skipper). The most interesting and the riskiest: two episodes,
   a cross-correlation over their first minutes, a stored span per series, and the existing skip
   machinery. Worth a prototype once the drift fit exists. Effort: M–L.

What I would not do: SyncPlay (needs the WebSocket the design excludes), Trakt (an external
account and API keys for a feature the local history already covers in spirit), and anything that
writes to the server's files or metadata.

## Reusable pieces

The subtitle sync left three things that make the neighbours cheap:

- `audio_probe` decodes any window of an item's direct stream at 8 kHz mono on a worker, with
  cancel and progress — the same primitive loudness and intro detection need.
- `subtitle_sync` has the mask/correlation/confidence machinery, including the lesson that a plain
  energy envelope is not enough: band-limit and take the modulation.
- The panel row, the `Latest` request, the toast and the online search panel show the shape a new
  one takes.
