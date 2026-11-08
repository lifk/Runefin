# Engineering notes — architecture, decisions and traps

How Runefin is built, what was measured, and which mistakes were already paid for. The
user-facing inventory is [`features.md`](features.md); the contracts are
[`design.md`](design.md) and [`design-player.md`](design-player.md).

**Repositories**

- App: https://github.com/lifk/Runefin — `main` is the source of truth. The pre-rewrite
  development history lives only on a private archive remote and is never pushed to `origin`.
- Rune fork: https://github.com/lifk/rune — `runefin/player` carries the patches the app
  depends on; the default branch is `runefin/player`
- Upstream: https://github.com/anomalyco/rune (moves ~100 commits/day)

## Visual checks against a real server

`scripts/snapshots.sh` stays hermetic (fixtures, no network) so committed images are
reproducible. For a layout check against the user's actual library and artwork, use the live
snapshot path instead:

```sh
target/release/runefin --snap /tmp/home-live.png --live --size 1920x1080 --at 20 --wait 3
```

`--live` reads the stored session and the real image cache; `--wait` gives setup and the image
workers time to land before the frame is frozen. Use this whenever a report depends on real
posters (card text contrast, rail density, sidebar height).

For a full screen sweep without a scripted tap, `--live` also accepts a route and its data:

```sh
RUNEFIN_LIVE_ROUTE=seerr-requests target/release/runefin \
  --snap /tmp/requests-live.png --live --at 8 --wait 12 --size 1440x900 --scale 2
```

Routes: `home`, `library` (`RUNEFIN_LIVE_LIBRARY_ID/NAME/TYPE`), `detail`
(`RUNEFIN_LIVE_ITEM_ID`), `search` (`RUNEFIN_LIVE_QUERY`), `settings`, `remote`, `live-tv`,
`seerr-discover`, `seerr-requests`, `seerr-detail` (`RUNEFIN_LIVE_MEDIA_TYPE/TMDB_ID`).
`RUNEFIN_LIVE_SETTINGS_QUERY` filters Settings, and `RUNEFIN_LIVE_FRAME_MS=20` holds each virtual
frame long enough for real images to land. These are developer tools; `scripts/snapshots.sh` never
sets them.

## 1. State of play

| Piece | State |
|---|---|
| M0 scaffold | done — workspace, `jellyfin-api` (21 tests), snapshot loop |
| M1 connect screen | done — probe + password sign-in, Quick Connect, LAN discovery, session persisted (0600), headless snapshots |
| Design | done — `docs/design.md` (browse app), `docs/design-player.md` (player: DMA-BUF, GStreamer, HDR) |
| Research | done — `docs/research/01..07`, cited; 05 is the Moonfin Linux desktop teardown, 06 the client parity audit (2026-11-04), 07 the feature/UX opportunity brief (2026-11-04) |
| Fork | live, patch slices 1-3 + NV12/P010 committed (`5221a35d`, on upstream `93d23f2f`) |
| M2 home / Continue Watching | done — shell, merged CW, recent rails, disk image cache, skeleton/empty/error states |
| M3 library | done — virtualized grid/list, sort/facets and direction, letter rail, paged fetch with one-page prefetch, back-restored scroll, watched badge on cards and rows |
| M4 detail | done — shared-element poster, seasons chips, episode rows, resume label, Start over, playlist picker, server "More like this" rail, Esc/back restores the library offset |
| M5 handoff | done — mpv spawn + IPC, now-playing, start/progress/stop reports, all exit paths |
| M5b in-app | zero-copy NV12/P010 done — custom DMA-BUF BaseSink into Rune, two-plane Y/UV import, PQ/HLG tone mapping, BGRA fallback, layered OSD, scrub with a 240×135 trickplay preview and chapter rail, held seek target so the bar does not snap, Rune-drawn subtitles with track/delay/size/background, ASS alignment and positions, online provider search/download, up-next strip with auto-advance, audio track menu, volume, fit/fill, keyboard, stats overlay, buffering indicator, idle/sleep inhibitor, volume persistence, collections rail, fullscreen (F11/OSD), MPRIS media controls, minimized player, CPU + mpv fallbacks; live 10-bit test stored 149,406 ms |
| M6 search | done — Ctrl+K/Ctrl+F, 150 ms `Latest` debounce, grouped Movies/Shows/Episodes/People rails |
| Playlists | done — sidebar section, ordered grid with its own endpoint, Play all/Shuffle, an add-to-playlist picker, a name prompt on detail, and entry move/removal plus whole-row drag reorder from the playlist page; create/add/move/read/remove/delete verified live (probe playlist removed again) |
| M6 cast / settings | done — `/Sessions` list, Pause/Resume/+30s/Stop, settings/cache/version/sign-out |
| Live TV | done — `/LiveTv/Channels` list with current program, guide grid, `/LiveStreams/Open` + `Close`, tuner URL in the in-app player, LIVE OSD with channel up/down (`PgUp`/`PgDn`), buffering indicator, local next-program reminders; live server played, zapped and closed (this provider only serves one of its four channels) |
| Phase 0 | done — Home contract (Recently Played, every-library Recently Added, See all, resume context/Remove), content-first sidebar, undo toasts, Settings sections |
| Phase 1 | done — segment bands with Ask/Always/Never, still watching, sleep timer, resume rewind + local checkpoints + conflict card, versions/pre-playback, queue controls, extras, theme media, pitch-corrected speed, downmix/delay/sound modes, diagnostics |
| Phase 1b | done — watchlist, likes, search hints, filter sheet (year, rating, language, subtitles), multi-select/bulk, Release/Runtime/Plays sorts, 4K/HDR/CC/5.1/Atmos/VERS/NEW badges, AD/SDH tags, reduced motion, settings search/reset/export, 5xx/transport retry, 400-entry poster cache cap; the mini player was already the persistent now-playing bar |
| Phase 2 (P4) | implemented and live-checked — PlaybackInfo with a device profile, direct-play-first selection, quality menu/restart, method + PlaySessionId reporting, Why? text. Live: direct play stays offered, a 2 Mbps cap returns a 951-byte HLS playlist, and the app decoded 72 frames in 8 s (1.35 s startup, debug) and seeked to 67.9 s (140 frames). HLS/tsdemux/souphttpsrc plugins present |
| Phase 2b (small items) | done — collection add/sort and 12.4 creation fix, playlist drag, live reminders and notify-send opt-in, text size 85–130 %, remappable player shortcuts, Live TV crash-safe restore; H2 discovery rail still deprioritised. Covers blocked (server 500 on image upload); frame screenshot blocked (no DMA-BUF readback) |
| Downloads | planned (Q9) — Settings storage path, queue, offline playback |
| Seerr companion | **done** — `seerr-api` crate (22 tests), Settings probe, Discover/search, film and season requests, Active/All request progress, Open in Jellyfin and Request missing seasons. Live Jellyseerr 3.5.1; Radarr/Sonarr sync enabled; the live duplicate probe returns 409. |
| Images | pass 2 done — search requests, mounted-row library fetches, 256-handle cap, retry, and every card in a home rail queued up front |
| UI/UX | pass 3 done — ambient home backdrop path, item logos, next-up and cast, shortcut sheet, layered player OSD, mini player, readable-text shadows over artwork, pointer cursors, 10 %/16 % hover/press washes, wheel scrolling in the library; see `docs/research/04-ui-ux-finishing-pass.md` and `docs/research/05-moonfin-linux-desktop.md` |
| Live server | verified 2026-11-05: Jellyfin **12.4.0**, 8 libraries, 14 resume rows, 12 next-up rows, 41 boxsets, 4 live channels; all three Rust live tests pass, including direct play → 2 Mbps HLS transcode and a collection create/add/remove/delete round trip |
| M5a player patch | **slices 1-3 done** — `push_dmabuf`, BGRA/NV12/P010 two-plane import, safe fd duplication, PQ/HLG tone mapping |

**M2 home.** `crates/runefin/src/home.rs` owns the server-side merge (one card per
series, newest first, entries untouched for 60 days hidden, finished items dropped) and the
bounded fetch (libraries + Resume + one Recently Added rail per video library; one rail failing
does not hide the screen). `state.rs` starts the request through `rune::Latest` after sign-in and never
from a headless test. `net.rs` has one image worker; `image.rs` keys bytes by the server's
immutable image tag and reads `$XDG_CACHE_HOME/runefin/images` before the network. `screens.rs`
draws the collapsible-ready sidebar, rails, cards, progress bars, and the skeleton/empty/error
states. Snapshots: `home`, `home-loading`, `home-empty`, `home-error`.

**M3 library.** `library.rs` owns the query (types per collection, sort, 100-item
pages, unwatched filter, `nameStartsWith` letter), the append/prefetch state, and the hash
`virtual_rows_version` uses to skip re-keying 20 000 rows. `state.rs` adds `Route::{Home,Library}`,
opens a library from the sidebar, prefetches one page after the first, and remembers each
library's viewport so reopening restores it through `Tx::scroll_to_y`. `screens.rs` draws the
virtualized grid and list, the Name/Added/Rating and All/Unwatched toolbar, the A–Z rail, and the
empty/error/loading states. Snapshots: `library`, `library-list`, `library-loading`,
`library-empty`, `library-error`. A 20 000-item fixture stays under the 16 ms frame budget in the
test.

**M4 detail.** `detail.rs` loads one item, its seasons and the first season's
episodes; a season switch is a second small request. `screens/detail.rs` draws the backdrop,
poster and scrim, one primary action labelled `Resume 42:13`, the clamped/expandable overview,
season chips with Play season/Shuffle and episode rows.
The poster uses Rune's `El::matched`, so the card's poster flies into the detail page and back.
`main::on_key` makes Esc pop detail to its library (restoring the stored viewport) or a library to
home. Snapshots: `detail`, `detail-loading`, `detail-error`.

Since M4 the detail page also has cast and crew, server logos, the server's Next Up row, episode
rows that play, More like this, Start over, a playlist picker and the item menu.

**M5 handoff.** `handoff.rs` spawns mpv on a worker, waits for its JSON IPC socket,
subscribes to position/duration/pause/EOF, and hands the reader to `Worker::watch_playback`.
`state/playback_state.rs` owns the session: it reports `/Sessions/Playing` at start, progress every
ten seconds and on pause/seek, and the distinct stop payload with the final position on Stop, EOF,
mpv exit, Esc, or app close (`App::on_close`). A session generation makes late events from an old
mpv harmless. The stop report refreshes home and the detail item, so Continue Watching tells the
truth. `screens/now_playing.rs` is the now-playing panel. The in-app player now owns the common
path; mpv remains the fallback when the in-app pipeline cannot produce a frame. A fake-player test
drives the real socket/event path, so the handoff is tested without opening mpv or a window.

Known trade-offs: the stream URL carries `api_key` (the spec-clean header path is still to do).
The ignored live tests run against the server; no default test hits the network. The detail
primary button, episode rows and Next Up all start playback.

**Live server, checked 2026-11-02.** The session lives in
`~/.config/runefin/config.toml` (0600). `crates/jellyfin-api/tests/live.rs` is ignored by default
and mostly read-only; one test creates a temporary collection, adds/removes a member and deletes
it. Run it with:

```sh
JELLYFIN_URL=... JELLYFIN_USER_ID=... JELLYFIN_TOKEN=... \
  cargo test -p jellyfin-api --test live -- --ignored --nocapture
```

The optional Seerr companion has its own ignored live test:

```sh
SEERR_URL=... SEERR_API_KEY=... \
  cargo test -p seerr-api --test live -- --ignored --nocapture
```

Set `SEERR_DUPLICATE_MOVIE_ID` to an already-requested TMDB id to exercise the 409 path; it creates
nothing.

The app pipeline has its own codec smoke test, which starts a file and reads its clock without
sending playback reports:

```sh
RUNEFIN_PROBE_ITEM=<itemId> \
  cargo test -p runefin a_live_file_decodes_in_the_app_pipeline -- --ignored --nocapture
```

2026-11-03: HEVC Main 10 4K HDR, H.264 and MPEG-4 Part 2 in AVI all started on the DMA-BUF
P010 path; the probe waits for the clock to move and stops.

## Performance probe (2026-11-04)

The app against the live `jellyfin.example` (Jellyfin 10.11.2 at the time; the server is
now 12.4.0), HTTPS through the reverse proxy, median of 5–7 runs. Historical numbers; reproduce
with:

```sh
# API calls and payloads (read-only; opens/closes one tuner stream)
JELLYFIN_URL=... JELLYFIN_USER_ID=... JELLYFIN_TOKEN=... \
  cargo test -p jellyfin-api --test perf -- --ignored --nocapture

# app-side home/detail/search pipelines
cargo test -p runefin perf_live_pipeline -- --ignored --nocapture

# player: start-to-first-frame, seek, real frame count, 1×/1.25×
RUNEFIN_PROBE_ITEM=<itemId> RUNEFIN_PROBE_RATE=1 RUNEFIN_PROBE_SEEK=1 \
  cargo test -p runefin state::in_app_state::tests::a_live_file_decodes -- --ignored --nocapture
```

| Call | Median |
|---|---|
| libraries | 28 ms |
| resume (13 rows) | 49 ms |
| latest (12 rows) | 81 ms |
| next up (12 rows) | 451 ms |
| item detail (People+Chapters+Trickplay+MediaSources) | 81 ms |
| search | 76 ms |
| poster 360 | 29 ms / 78 KB |
| backdrop 1280 | skipped for the sample, no tag |
| trickplay tile 640 | 107 ms / 4.3 MB |
| subtitle SRT | 34 ms / 19 KB |
| live channels | 39 ms |
| live guide window | 27 ms |
| live open + close | 72 ms |
| collections (12 box sets) | 46 ms |

Home parallelises Continue Watching and the Recently Added rails and streams both Next Up and
Collections beside them: **104 ms** to the first paint data (was **179 ms** before the shared
HTTP connection pool). Next Up arrives by **~490 ms** and Collections by **~21 ms**; Collections
is streamed rather than folded into the first paint because enumerating 41 box sets walks every
library on the server. Detail **54 ms**, search **66 ms**. The pool is one `ureq::Agent` in
`Client`, cloned with it: a screenful of posters pays one TLS handshake, not one per image.
Recommendations and cast on an open detail page are High priority, so a cold cache fills the
visible rail before the hidden home rails that queued behind it.

Provider trailers (release build, local GPU): first play **5.5 s** for a 34.8 MB 1080p
download + audio merge; a cached trailer reaches its first frame in **135 ms**. `yt-dlp` is
spawned only when the Trailer action is used, so app startup and browsing are unchanged.

Player, release build, 1280×800, 24 fps source:

| File | Sink | Start | Frames / 3 s | RSS | CPU |
|---|---|---|---|---|---|
| H.264 MP4 | DMA-BUF P010 | 237 ms | 72 (24.0 fps) | 184 MB | 0.27 s |
| HEVC 1080p | DMA-BUF P010 | 371 ms | 72 (24.0 fps) | 182 MB | 0.31 s |
| HEVC 4K HDR10 | DMA-BUF P010 PQ | 243 ms | 71 (23.7 fps) | 232 MB | 1.17 s |
| Live TV channel (1080i) | CPU | 1.51 s (open + first frame) | 151 (49.9 fps) | — | — |

Seek lands in **64–67 ms** on the 4K HDR file; 1.25× advances **3.6 s of media in 3 s** and
survives the seek. All three files now use the DMA-BUF sink; the earlier HEVC CPU fallback was
caused by playsink's default subtitleoverlay refusing `VAMemory P010`.
Release UI snapshot: **128 ms** process wall (GPU init included), **9.1 ms** first frame,
**137 MB** RSS. Binary **38 MB**. Every tested file uses the DMA-BUF sink, including 4K HDR. Subtitles are drawn by Rune, and
playbin's TEXT flag is cleared so its subtitleoverlay/ASS renderer cannot block the DMA path or
crash the pipeline.

The server was upgraded to **Jellyfin 12.4.0** on 2026-11-05. All three live tests pass:
8 libraries, 14 Continue Watching rows with `UserData`, 12 next-up rows, seasons/episodes,
collections, playlist order, subtitle tracks, chapter images and an image fetch. `PlaybackInfo`
still offers direct play and returns a fetchable HLS playlist at a 2 Mbps cap. The collection
test creates a temporary box set, adds two members, verifies them, removes one member and deletes
the collection again.

**M5 end-to-end, verified 2026-11-02.** The ignored
`a_live_item_reports_its_position_back_to_continue_watching` test played a 5.1-minute episode
(NCED 06) for 60 s: **mpv position 59,497 ms, server stored 59,497 ms**, and the item returned in
Continue Watching. mpv was muted for the test; the player window was real, not a fake. The two
1.5-minute NCOP specials that the first attempt marked played were reset to unplayed through
`DELETE /Users/{userId}/PlayedItems/{itemId}`.

Two lessons:

- Jellyfin's default `MinResumeDurationSeconds` is 300, so short specials are marked played by
  design even after a clean stop. Use an episode over five minutes and play past `MinResumePct`
  (default 5%); the test now plays 60 seconds.
- A 403 may be a proxy rule rather than a Jellyfin permission check, and some fail2ban/WAF
  setups ban the client IP on the first one. Do not probe admin paths from the app, and stop
  rather than retrying when a 403 appears.

**M6 search.** `search.rs` groups one `/Items?searchTerm=` answer into Movies,
Shows, Episodes and People; `state/search_state.rs` owns the focused field, the scoped query
(`Ctrl+F` in a library, `Ctrl+K` everywhere), and a 150 ms `Latest` debounce so every keystroke
cancels the previous request. `screens/search.rs` draws the field and grouped rails; cards open
detail and Esc returns to the query and results. The connect fields also gained their missing
`on_edit` accessors, so typing now actually reaches `State`.

**M6 cast/remote and settings.** `jellyfin-api` gained `SessionInfo`,
`PlayerState`, `GET /Sessions`, `POST /Sessions/{id}/Playing/{command}` and the seek call.
`screens/remote.rs` filters out this device, lists the rest with now-playing and position, and
wires Pause/Resume, +30s and Stop; `screens/settings.rs` shows the server, user, device id,
version and poster-cache location, with Remote and Sign out. The live test now also calls
`/Sessions` and passed. Remote commands themselves need a second active session to exercise.

**Images, tightened.** Search results now queue their posters; library posters are requested only
for the virtual rows the viewport has mounted; home requests are capped at eight cards per rail.
Decoded handles are bounded to 256 with FIFO eviction (and evicted keys may be requested again),
failed fetches leave the retry door open, and returning to home/search re-queues what eviction may
have dropped. Settings now clears the poster cache on a worker. Blur-up placeholders are done: the image worker
bakes a 27×17 RGBA wash with the same `image` crate Rune decodes with, the card paints it under the
full poster, and a 120 ms beat plus the opacity spring crossfades over it. Image requests now use a
two-level queue: the current screen (Continue Watching, library rows, detail, search) is high
priority; Recently Added rails are low, so they cannot delay a poster the user is waiting on.

**In-app player, first slice.** `player.rs` builds a `playbin3` pipeline with a
The first in-app slice used `vapostproc`/`videoconvert` → NV12 `appsink`; the current path is the
DMA-BUF sink below. The CPU path remains the fallback and exposes the same clock, pause, seek and
stop surface. `state/in_app_state.rs` reports start/progress/stop the same
way as mpv and falls back to mpv automatically if GStreamer cannot start. `screens/in_app.rs` is
the video surface plus controls. The item used for the live check was Matroska, which needs
`gst-plugins-good` (`matroskademux`/`qtdemux`) — install `gst-plugins-good gst-libav` before
expecting MKV/MP4 to play in-app. With them installed, the ignored live test passed:
**29,876 ms played, 29,876 ms stored** in Continue Watching, with the item starting at its resume
position. The system plugin is the supported path; unpacking the Arch package to
`~/.local/opt/gst-good` plus `GST_PLUGIN_PATH` also works for a non-root check. The local clip
test checks NV12 surface validation, clock and a 2 s seek. The zero-copy slice uses the custom DMA-BUF `BaseSink` described above. Its NV12 path imports
the Y and UV planes as separate R8/R8G8 images using the vendored `wgpu-external-frame` extension,
so the existing two-plane shader samples them without a CPU copy; BGRA is still the fallback when
NV12 cannot be negotiated. Playbin's audio streams are exposed as an `Audio 1/2` control when a
file has more than one; Space toggles pause, ←/→ seek 10 s, and Esc stops. P010 imports R16/R16G16
planes and the shader converts BT.2020 PQ/HLG to linear, tone-maps with a Reinhard shoulder and
then writes SDR. The path badge names `P010 PQ`, `P010 HLG`, `P010`, `NV12` or `BGRA`. If a 4K HDR file
reaches PLAYING but produces no frame (VA-API's pre-colorspace refuses that 2160p P010
stream), the player treats startup as failed after six seconds and falls back to CPU, then mpv,
instead of showing a black frozen surface.

**In-app player, zero-copy slice.** `dmabuf_sink.rs` is a minimal GStreamer `BaseSink`: its pad
template accepts `video/x-raw(memory:DMABuf),format=DMA_DRM`; `propose_allocation` advertises
`GstVideoMeta` (the missing piece that makes `vapostproc` refuse `appsink`); `render` reads the
plane fd/offset/stride, duplicates the fd through Rune's safe helper, wraps the GStreamer buffer
in a lease, and calls `push_dmabuf`. `player.rs` picks the first AR24/XR24 modifier that
`vapostproc` advertises, caps the filter to it, and retries the CPU NV12 path if anything fails.
The local MKV test and the live server test both pass; the live run stored **59,734 ms** after
30 s with P010 selected. P010 imports R16/R16G16 and tone-maps PQ/HLG to SDR; quality calibration
against a real HDR remux is the remaining M5c work, not a missing path.

The next feature projects are **P4** (PlaybackInfo + direct-play-first transcoding) and **Q9**
(downloads/offline with a Settings path); music is still out of scope.

## 2. The patch: what exists and what is next

`runefin/player` at `5221a35d` carries slices 1-3, the main-window adapter fix, the NV12/P010
two-plane importer, and a vendored `wgpu-external-frame` with R8/R8G8/R16/R16G16 formats, rebased
onto upstream `dev` `93d23f2f` on 2026-11-04.

- `crates/rune/src/native_surface/dmabuf.rs` — `Plane` (owned fd, offset, stride; `from_raw_fd`
  duplicates a producer fd), `DmabufFrame` (size, `Format`, `ColorInfo`, DRM modifier, planes,
  producer lease), `DmabufFrame::new()` validation (plane count, 420 even edges, stride ≥ row
  width), `from_fourcc()` for `XR24`/`AR24`/`NV12`/`P010`, `to_external()` (packed BGRA only),
  and the `Importer` (two planes for NV12/P010).
- `native_surface/mod.rs` — `pub mod dmabuf` (Linux) and `NativeSurface::push_dmabuf`.
- `native_surface/pipeline.rs` — `Source::Dmabuf` branch; imports into a wgpu-owned texture and
  releases the producer after `on_submitted_work_done`.
- `render.rs` / `render/upload.rs` / `desktop/window.rs` — the window `Renderer` remembers its
  adapter (the importer needs the backend), opts into `VULKAN_EXTERNAL_MEMORY_DMA_BUF`, and hands
  the adapter to `SurfaceRenderer`. Headless renderers keep no adapter.

**What slice 2 decided.** `wgpu-external-frame` 0.4 imports **only packed 32-bit planes**, and its
`DmaBufImporter` needs the adapter. Two consequences:

1. **BGRA now, NV12 next.** The current path takes `XR24`/`AR24` through the crate, which copies
   on the GPU into a texture wgpu owns (one blit, no CPU copy). `push_dmabuf` still accepts an NV12
   frame from slice 1, but import returns an error naming the multi-plane path rather than sampling
   it wrong. For M5b, have `vapostproc` export `BGRA`; ask for `NV12` again when slice 3 adds the
   two-image / multi-plane import. The shader's NV12 branch is unchanged and still serves CPU
   frames.
2. **Adapter plumbing is part of the patch.** `Renderer::new_with_adapter` is used by the window;
   `Renderer::new` stays adapter-less for tests and overlays. Do not "simplify" this away: the
   crate picks Vulkan vs EGL from the adapter, and guessing from the device panics on the wrong
   backend.

**Slice 3 — direct import + the shader.** The shader already samples imported BGRA and CPU NV12.
The remaining work is:

1. A multi-plane NV12/P010 import (one image per plane, explicit DRM modifier) that samples the
   producer's texture directly, removing the blit. This is where the `VK_EXT_image_drm_format_modifier`
   acquire/release barrier lives; keep the lease drop on `on_submitted_work_done` either way.
2. YCbCr→RGB from `ColorInfo` (never assume BT.709), then tone mapping (PQ/BT.2020 → SDR) in M5c.
   This now ships as the baseline Reinhard shoulder; calibration remains.
   Keep a second output path for HDR passthrough so M5d is a flag, not a rewrite.

**Scope:** the patch stays in `native_surface`, one shader seam and the window adapter; upstream it
once the seam stabilizes.

**App pin:** `crates/runefin/Cargo.toml` now points at the fork
`5221a35da1b25b1ac59678aa9164b02ef2cbea9d`. The fork adds `Plane::from_raw_fd` so the
app can hand GStreamer's descriptor to Rune without an unsafe app-side call. The DMA path is
app-side (`dmabuf_sink.rs`), uses a `BaseSink` rather than `appsink`, and falls back to the CPU
NV12 path if VA-API or DMA-BUF negotiation fails. Future Rune bumps stay standalone commits, per
`AGENTS.md` rule 6.

**Bump, 2026-11-04.** `scripts/upstream-sync.sh` rebased the six `native_surface` commits from
upstream base `c0d14d1` onto `93d23f2f` (55 upstream commits). The only conflict was `Cargo.lock`
(`libloading 0.8.9` for Rune vs the importer's 0.9); both stay. After the rebase the app builds,
`scripts/check.sh` is green (**42 API + 259 app tests**, 21 ignored, release), and all **99 snapshots are
byte-identical**. Rune's own lib suite is 1468 passed / 6 failed, and the same six fail on `dev`
(two video stubs that only run on macOS, a keycap modifier glyph, an edit shortcut, a layer
clip/fade comparison and a font-link box); the patch adds no failures. The bump also caught a
stale guard: `to_external` could take an NV12/P010 frame as packed BGRA, so it is now BGRA-only
and the two old refusal tests assert the current behaviour.

**Jellyfin 12.4 image upload is broken on this server.** `POST /Items/{id}/Images/Primary`
returns 500 for JPEG, PNG and multipart bodies, so collection/playlist covers stay out of the
UI. `POST /Collections` itself now takes query parameters (below).

**Jellyfin 12.4 changed collection creation.** `POST /Collections` now takes `name` and `ids`
as query parameters and returns `CollectionCreationResult`; the old JSON body answers 500.
`add_to_collection` uses `POST /Collections/{id}/Items?ids=…` and is covered by the new
ignored live test.

## 3. How to verify on a machine with a GPU

```sh
# the fork
git clone https://github.com/lifk/rune.git && cd rune
git switch runefin/player
cargo test -p rune --lib --no-default-features native_surface::dmabuf
cargo check -p rune --no-default-features --all-targets

# the app
git clone https://github.com/lifk/Runefin.git && cd Runefin
scripts/check.sh          # fmt, clippy, every test suite, release build
scripts/snapshots.sh      # every committed capture; see docs/snapshots/README.md
```

System packages: `libxkbcommon-dev libwayland-dev libvulkan1 mesa-vulkan-drivers`, plus
`libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev gstreamer1.0-vaapi gst-plugins-good
gst-libav` for the in-app player. `libudev-dev` is needed to compile the controller listener.

## 4. Traps worth not rediscovering

1. **Rune's own lib tests do not compile on Linux with default features**: `metal-backend` is in
   `crates/rune`'s defaults, and `native_surface/tests.rs`'s `on_each_backend!` then references
   `crate::gpu::metal`, which is `cfg(target_vendor = "apple")`. Always test with
   `--no-default-features` on Linux. The app crate already avoids it by listing features explicitly.
2. **Debug info is huge**: a full `debug = 2` build of Rune + wgpu is ~6 GB of artifacts. The app
   sets `[profile.dev] debug = 0`; for the fork, use `CARGO_PROFILE_DEV_DEBUG=0`.
3. **Linking needs `libxkbcommon.so`**, which some runtime packages ship only as `.so.0`. A symlink
   shim works: `ln -s /usr/lib/x86_64-linux-gnu/libxkbcommon.so.0 ~/.local/libshim/libxkbcommon.so`
   then `RUSTFLAGS="-L $HOME/.local/libshim"`. Installable dev packages make this unnecessary.
4. **Snapshots are ICD-specific.** Committed images are **AMD/RADV-rendered**; a lavapipe re-render
   will churn the antialiasing even when the UI did not change. Pick one ICD and say which produced
   the new images in the commit. Do not mix: re-render every scene when switching.
5. **The window's wake lane connects after `setup`.** A home answer that lands during
   `setup` can be lost and leave the skeleton up until the user moves the mouse. The app starts
   the first request in `setup` but keeps a short `AppHandle::wake` heartbeat alive for the first
   three seconds; later answers use the setup waker. Do not remove this without a live-window test.
6. **Prefer the typed client to ad-hoc probes.** Some reverse proxies in front of a Jellyfin server
   ban an IP on repeated or admin-path requests; the `jellyfin-api` client sends one request at a
   time and never touches `/System/Configuration`. If a 403 appears, stop rather than retrying.

## 5. Decisions already made (do not re-litigate)

- **D1/D3 superseded**: no mpv subprocess on Wayland; in-app playback via DMA-BUF. mpv stays as a
  settings-level fallback only.
- **Fork now, upstream later.** `runefin/player` is rebased weekly onto `dev`; when upstream lands
  the import (issue #23) or PR #50 becomes a GPU path, delete the branch and repoint `Cargo.toml`.
- **Targets**: laptop = AMD Strix Halo (RDNA 3.5, 40 CU), Steam Machine = RDNA 3 — one Mesa
  `radeonsi` + VCN path, no NVIDIA branch in v1.
- **HDR**: 4K 10-bit decode + tone-mapped SDR is the deliverable; HDR *output* waits on wgpu
  ([#10545](https://github.com/gfx-rs/wgpu/issues/10545)) and `wp_color_management_v1`. Samsung TV
  is the M5d case.
- **Rune rev is pinned to a commit** of `runefin/player` (currently `5221a35d` on the fork; the
  app points at that same commit), never to a branch.
- **Seerr is an optional companion**, not a second media server (SD10). It uses an admin API key
  in the 0600 config; with no URL/key the app is unchanged. S0 ships the crate and the Settings
  probe; the Discover/Requests page is S1/S2.

## 6. Final state, 2026-11-05

`scripts/snapshots.sh` renders 113 scenes to **121 captures**; the app suite is **304 tests**
(21 ignored), the Jellyfin API suite **42** and the Seerr API suite **22**.

The browse and playback surfaces are feature-complete against
`docs/research/06-parity-audit-2026-11.md`: connect with password, Quick Connect and LAN discovery;
Home rails with streamed Next Up, Collections and a watched badge; libraries with virtualized
grids, sort/direction, filters, genres, letter rail, server-side episode shuffle and a hover play
button; search; detail with seasons, episodes, cast, recommendations, Start over, a playlist
picker and the item menu; detail with **IMDb/TMDb/Rotten Tomatoes scores** and **trailers**
(local in-app; YouTube in-app through optional yt-dlp, browser fallback); player with zero-copy
DMA-BUF 4K HDR, text subtitles
(SRT/VTT/ASS with inline bold/italic/colour and `\an` alignment), **PGS/VobSub bitmap
subtitles**, full text appearance (face, colour, outline, weight, position, text and plate
opacity), online subtitle download, audio tracks, chapters with stills, trickplay, skip intro,
queue, stats, buffering, idle inhibit, fullscreen, MPRIS and remembered volume; playlists with
create/add/order/remove/drag-reorder; watched/favourite/resume actions; remote control; casting
to another session; live TV with guide, zapping, now/next, recordings browsing and local program
reminders; text size and remappable player shortcuts. Collections can be created, added to,
sorted and have members removed; Home has one Recently Added rail per video library.

Known limits, with the reason:

- image subtitles: PGS and VobSub work through an `ffmpeg` extraction into the subtitle cache
  plus `libbitsub-core` RGBA decoding; DVB is transcoded by ffmpeg to DVD subtitle packets and
  read by the same parser. The first selection on a large file can take a few seconds, and an
  unknown bitmap codec stays a listed row with the online text-search action;
- provider trailers: with `yt-dlp` (and ffmpeg for the merge) installed, a YouTube URL downloads
  once into `~/.cache/runefin/trailers` and plays in the in-app player without reporting; without
  `yt-dlp` the action opens the browser. Both paths were exercised: yt-dlp resolved the live
  trailer and the tree showed the in-app player with a 2:07 duration;
- server-hosted trailers play in-app;
- personal screenshots: `runefin --snap out.png --live --at 30 --wait 3` renders the signed-in
  session instead of the hermetic fixture; `scripts/snapshots.sh` never passes `--live`;
- IMDb scores need one download of IMDb's public dataset (8.7 MB) on first use; TMDb and RT
  render immediately and an IMDb link stands in until the index is ready. The 10 MB index is
  reused for 30 days and an optional OMDb key is the fallback if the download fails;
- transcoding: shipped direct-play-first (P4); a chosen bitrate cap or an unsupported stream
  uses DirectStream/Transcode, and direct play remains the default;
- image covers: `POST /Items/{id}/Images/Primary` returns 500 on Jellyfin 12.4 for JPEG, PNG, raw
  and multipart bodies, so collection/playlist cover actions are not exposed;
- frame screenshot: the DMA-BUF sink has no CPU readback; a screenshot needs a tee/readback
  branch in the player, so there is no UI action yet;
- Trakt: needs an account and OAuth credentials; local history and Letterboxd export do not;
- HDR passthrough: the window is 8-bit sRGB, so HDR10/HLG is tone-mapped; passing PQ through
  unchanged needs an HDR swapchain and color space in Rune (M5d);
- recording timers: the Xtream provider answers HTTP 500 on `POST /LiveTv/Timers`;
- ASS transforms: the authored outline, shadow, rotation and scale render; non-uniform
  `\fscx`/`\fscy` keeps its area because Rune scales uniformly, and 3D `\frz` axes are ignored;
- fullscreen/MPRIS: shipped (F11/OSD and a zbus service); MPRIS changes are polled rather than
  pushed as `PropertiesChanged`;
- downloads/offline: planned (Q9) with a Settings storage path, not implemented yet;
- music, photos, SyncPlay: not this product (no libraries, or a WebSocket dependency the design
  excludes);
- casting playback needs a second device to observe; the server answers 204 to the command.

## 7. Also worth knowing

- PR #50 (`Linux Player: GStreamer, libmpv, libVLC`) is **open, 0 reviews, `mergeable: false`**, and
  its frames are CPU BGRA — do not adopt it for the 4K path. Its GStreamer plumbing is still a
  useful reference for pipeline setup.
- The Jellyfin API contract is verified against the 12.4.0 OpenAPI and pinned by tests
  (`design.md` §12): `/Items` needs `fields=UserData`; `/UserItems/Resume` exists,
  `/Users/{userId}/Items/Resume` does not.
- `scripts/upstream-sync.sh` in the app repo does fetch → ff `dev` → rebase → diff review →
  `cargo check`. Run it before any Rune work, and after a week away.
