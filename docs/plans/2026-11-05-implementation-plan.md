# Runefin implementation plan — 2026-11-05

Plan for the revised opportunity list in
[`../research/07-feature-opportunities-2026-11.md`](../research/07-feature-opportunities-2026-11.md).
The backlog was trimmed and merged on the same date; the decisions below are the product owner's,
not new research.

**Decisions applied**

- **Transcoding is unblocked.** The server does transcode (the official client uses it here).
  Policy: **direct play first**; transcode only when the reader picks a lower bitrate or the app
  cannot reproduce the codec/container/audio/subtitle. Never transcode silently.
- **Downloads are planned**, with a **Settings → Downloads storage path**, a queue, and offline
  playback of stored files.
- **Removed:** A5, H3, S4, V6, R6 (SyncPlay), C6 (multi-user), T7.
- **Merged:** H1+H10, H5+R2, P3+C1–C3, P5+X3+P13, P6+X1+X2, P9+Q6, Q4+Q9.
- **Adjusted:** H8 drops “Leaving”; T3 is MPRIS push/shuffle/loop (artwork already ships);
  A7 is the lowest priority.
- **One user, one window, one server.** No SyncPlay, no multi-user, no web view.

The work is ordered by value per week. Every phase ends with `scripts/check.sh`,
`scripts/snapshots.sh`, a design/doc amendment where the contract changed, and a commit someone
can review. No phase starts before the previous one’s “done when” is met, except the paired
Phase 1a/1b tracks.

---

## Step checklist

- [x] Phase 0 — Home contract, sidebar, undo, settings skeleton, doc truth
- [x] Phase 1 — player quality of life (segments, still watching, resume, versions, queue)
- [x] Phase 1b — browse/search/accessibility polish
- [x] Phase 2 — PlaybackInfo + direct-play-first transcoding (live HLS play/seek verified; report fields unit-checked)
- [ ] Phase 2b — collector/detail/discovery depth (implemented: collection add/sort, playlist drag, live reminders, reminder notifications, text size, shortcut remapping, Q5 for Live TV; still open: H2 discovery rail, end-of-episode polish, most detail depth. Blocked: covers — the server returns 500 to image upload; frame screenshot — the DMA-BUF sink has no readback)
- [ ] Phase 3 — downloads, offline, platform integration
- [ ] Phase 4 — only if the product changes (A7 last)
- [x] Phase 5 (optional, separate track) — Seerr companion: discover/request films and series and
  show download progress. **S0–S3 shipped**; S2's optional delete/undo and a full keyboard-only
  pass remain open. Discovery in
  [`../research/08-seerr-client.md`](../research/08-seerr-client.md); plan in
  [`2026-11-05-seerr-requests-plan.md`](2026-11-05-seerr-requests-plan.md). Not part of the
  download queue or the Jellyfin contract.

---

## Phase 0 — Truth and polish (about 1 week)

### H1 — Home contract (absorbs H10)

1. Rename the first rail **Continue Watching** (not “In progress”).
2. Order: Continue Watching → Next Up → one Recently Added rail per video library →
   **Recently Played** → Collections; suppress empty rails.
3. Add the Recently Played rail: `GET /Items?filters=IsPlayed&sortBy=DatePlayed&descending=true`
   with `fields=UserData`, deduplicated by series against Continue Watching.
4. Add “See all” to the first two rails (filtered grid), card context (`S2E4`, time left) and a
   Remove action on resume cards.
5. Keep the readable-text shadow, rail image prefetch and horizontal scroll behavior.

Acceptance: `home.png` and a new `home-contract.png` match the contract; the rail count is ≤ 6;
Recently Played is absent when empty; a resume card shows the episode and remaining time.
Tests: `home::merge_resume`, `rt.find("rail-recently-played-…")`, `rt.state.home_data`.
Docs: `design.md` §3 and `features.md` §3 already describe the target; they must match the code.

### S6 — Sidebar IA

Content before utilities: Home → libraries (pinned first) → Playlists → Search/Remote/Settings.
Add groups, pinned libraries and counts only where they fit without a new navigation model.
Snapshot `home-collapsed.png`/`home.png`; tests for pinned order.

### S9 — Undo toasts

Favourite, watched, remove-from-CW and playlist changes return the previous state for 8 s. The
toast is a real action, not a message. Tests: `rt.state` before/after; snapshot `toast-undo.png`.

### S10 — Settings skeleton

Split the single column into Appearance, Playback, Subtitle/Audio, Storage/Downloads, Account.
No new settings yet; move existing rows without changing defaults.

### Doc truth

- `features.md`, `README.md`, `HANDOFF.md`, the parity audit and `design.md` must agree about
  Recently Played, transcoding (planned, direct-first) and downloads (planned).
- Do not claim a rail or a capability that the code does not have.

**Done when:** Home matches the contract, the sidebar puts content first, destructive actions
have Undo, Settings is sectioned, and the docs/code agree.

---

## Phase 1 — Player quality of life (3–4 weeks)

### P1 — Segment bands + auto-skip

- Draw `Intro`, `Recap`, `Outro`, `Preview` bands from the existing `media_segments` data on the
  timeline; hover label with the time range; clicking a band seeks to its start.
- Setting `Skip intro/recap/credits: Ask / Always / Never` (persist in config). `Always` seeks at
  the segment start and offers Undo for 8 s; `Ask` shows the existing button five seconds early.
- Existing `S` key stays; no segment data means no bands and no button.
- Tests: `rt.find("in-app-seek-segment-intro")`, fake clock, always-skip + undo. Snapshot
  `in-app-segments.png`.

### P2 — “Still watching?” + sleep timer

- Pause at the end of an episode after N episodes or M minutes of wall-clock playback with no
  input (default 3 / 90); quiet full-screen card with Continue/Stop; 60 s countdown.
- Sleep timer: 10/20/30/45/60 min or end of episode; OSD moon + remaining; volume fades over the
  last 20 s; cancel restores.
- Tests with a fake clock; stop sends the final position. Snapshots `in-app-still-watching.png`,
  `in-app-sleep-timer.png`.

### P3 — Resume continuity (absorbs C1–C3)

- Setting `Resume rewind: 0/5/10/15 s`; start earlier than the server position and show “Rewound
  10 s” briefly. The detail label still shows the true server position.
- Local checkpoint every 5 s in a capped JSON map under `~/.cache/runefin/positions.json`; after a
  crash offer local vs server position when they differ.
- **Continue on…** in the player menu: list other sessions with their position, send
  `POST /Sessions/{id}/Playing` with `playCommand=PlayNow&startPositionTicks=…`, stop locally only
  after a 2xx.
- Tests: checkpoint survives a simulated kill; transfer body; conflict prompt. Snapshot
  `in-app-transfer.png`.

### P5 — Versions + pre-playback summary (absorbs X3, P13)

- `MediaSources.len() > 1` or `MediaSourceCount > 1` adds a chevron to Play and a Version row:
  resolution/range/bitrate/name, labelled from server data only.
- Remember the last version per item; pass `mediaSourceId` to PlaybackInfo/stream.
- Pre-playback sheet: version, container, video/audio codec, HDR, subtitle tracks, size,
  direct-play/transcode decision (P4 supplies the last line).
- Tests: source selection sends the right id; single source shows no chevron. Snapshots
  `detail-versions.png`, `in-app-preplay.png`.

### P12 — Queue 2.0

- Ephemeral queue: Play next / Add to queue / reorder / remove; save queue as a playlist.
- Queue panel marks the current entry; EOS follows it; a single item still ends the run.
- Tests: add/next/reorder/EOS; snapshot `in-app-queue-panel.png`.

### P6 — Extras + theme media (absorbs X1, X2)

- Extras rail grouped by `ExtraType`; play with reporting off.
- Theme song on detail (speaker toggle), theme video muted behind the hero; both stop on
  navigation/playback; defaults off for audio, optional for video; reduced motion suppresses video.
- Tests: mocked `SpecialFeatures`/`ThemeMedia`; no reporting. Snapshots `detail-extras.png`,
  `detail-theme.png`.

### P7 — Audio intelligence

Split into two commits:
1. **Cheap routing**: pitch-corrected speed (`scaletempo`), downmix to stereo, audio delay
   (±100 ms), surfaced in the audio menu and stats.
2. **DSP**: dialogue boost and night mode (`audiodynamic`/compressor) with tuning; show the active
   mode in stats.
Tests: pipeline elements present, settings persist, mute/volume unaffected; snapshot
`in-app-audio-modes.png`.

### P9 — Diagnostics (absorbs Q6)

One panel: sink path, codec/range, frame count, GStreamer state, last error, server latency, cache
usage, playback method. Player failures link to it. No new dependency. Tests: state snapshot;
snapshot `in-app-stats.png` (extend).

### Phase 1b — Browse, search, accessibility

- **H5/R2** watchlist: decide Favorites vs a user playlist on the live server; one toggle, sidebar
  entry, home rail; verify sharing/privacy. No “private” claim.
- **R1** likes/dislikes, **S9** undo already in Phase 0.
- **L1** filter sheet, **L3** multi-select/bulk, **L4** more sorts, **L7** capability badges
  (fetch stream fields for visible cards only).
- **S2** search hints, **S5** result actions (go to series/trailer), **S8** persistent now-playing
  bar, **S10** settings IA.
- **Q1** stale-while-revalidate, **Q3** cache limits, **Q7** retry/backoff, **Q8** frame tests.
- **A1** reduced motion, **A4** AD/SDH badges, **A8/A9** badge/focus audit.
- Tests and snapshots per item; no new dependency.

**Done when:** resume/skip/still-watching/sleep/version/queue work with tests; a viewer can filter to
4K + English subs, add to a watchlist, undo a change, and the focus/contrast pass is documented.

---

## Phase 2 — PlaybackInfo and direct-play-first transcoding (P4)

This is the largest capability change and the first task of Phase 2. The server supports
transcoding; the client currently only builds `Videos/{id}/stream?static=true`. The rule is
**direct play unless there is a reason**.

### Client and wire types

1. Add `Client::playback_info(item_id, request)`:
   - POST `/Items/{id}/PlaybackInfo` with `UserId`, `StartTimeTicks`, `MaxStreamingBitrate`,
     `MediaSourceId`, `AudioStreamIndex`, `SubtitleStreamIndex`, `MaxAudioChannels`,
     `EnableDirectPlay`, `EnableDirectStream`, `EnableTranscoding`,
     `AllowVideoStreamCopy`, `AllowAudioStreamCopy`, and a `DeviceProfile`.
   - Parse `MediaSources[]`: `Id`, `SupportsDirectPlay`, `SupportsDirectStream`,
     `SupportsTranscoding`, `DirectStreamUrl`, `TranscodingUrl`, `TranscodingSubProtocol`,
     `TranscodingContainer`, `PlaySessionId`, `MediaStreams`.
2. Device profile from the actual GStreamer capabilities:
   - Direct play: containers and codecs the installed plugins can demux/decode (Matroska/MP4/AVI;
     H.264/HEVC/AV1/VP9/MPEG-4; audio AAC/AC3/EAC3/DTS/TrueHD/FLAC/Opus/Vorbis/MP3), 10-bit and
     HDR as supported by the sink.
   - Direct stream: video copy with audio transcode, or the reverse, when the server marks it.
   - Transcode: HLS/TS (or progressive) with H.264/AAC as the safe baseline.
   - Subtitles: `subrip`/`ass` direct; bitmap burn-in when needed; text conversion otherwise.
3. Build the playback request in one place (`play_in_app_media` path):
   - Default: no bitrate cap, direct play/direct stream enabled, transcode enabled as fallback.
   - If the source has a codec/container the profile cannot handle, let the server transcode.
   - If the reader chooses a quality below the source bitrate, cap `MaxStreamingBitrate` and use
     the returned `TranscodingUrl`.
   - If the server says only transcode, accept it and label it.
4. Play `DirectStreamUrl`/`TranscodingUrl` through the same player. Use the system
   `hlsdemux`/`tsdemux` through playbin; no new Cargo dependency. Keep the custom sink and HDR
   tone mapping. Handle HLS seek by position; a transcode seek may restart the segment.
5. Report `PlaySessionId` and the method (`DirectPlay`/`DirectStream`/`Transcode`) on start,
   progress and stop; keep the existing stop payload rules.

### UI

- Pre-playback sheet: version, container, video/audio, HDR, subtitles, size, and a plain-language
  decision: `Direct play · 4K HDR10 · 31 Mbps` / `Transcode · 1080p · 8 Mbps`.
- Quality menu: **Auto** (direct-first), **Original**, 20/12/8/4/2 Mbps, 1080p/720p/480p caps.
  Remember the last choice per network (LAN/remote) and per item.
- A **Why?** panel lists actual reasons (codec, container, bitrate, audio, subtitle burn-in,
  device profile). Never show a transcode label when the file is direct-playing.
- On a server that cannot transcode or a failed PlaybackInfo, fall back to the current direct
  stream and say so; never block playback behind the menu.

### Tests and evidence

- `jellyfin-api`: request-body/response fixtures for direct, direct-stream, audio-only and
  transcode; URL selection.
- `runefin`: mocked PlaybackInfo selecting direct vs transcode; a bitrate choice sends the cap; a
  failed PlaybackInfo falls back to direct; reported method/session id.
- Ignored live test on the real server: direct play stays direct; forcing 2 Mbps returns a
  transcoding URL and plays.
- Snapshots: `detail-preplay.png`, `in-app-quality.png`, `in-app-why.png`.
- Performance: first frame direct still under the current budget; record transcode startup and
  seek latency.

### Done when

A LAN file still direct-plays with no extra server work; a lower bitrate or an unsupported codec
transcodes, plays, seeks and reports correctly; the label tells the truth; direct play remains the
default after restart.

**Risk:** device-profile correctness, HLS startup latency, seek in a transcode, subtitle burn-in,
audio-only transcode. Test each live before claiming the menu works.

---

## Phase 2b — Collector and discovery depth

After P4: P6 if not already done, P8 frame step/A–B loop/go-to-time (screenshot is blocked on a
sink readback branch), P11 end-of-episode
polish, H2 honest discovery, H4 removed (Coming up rail dropped in favour of every-library Recently Added), H6 collections depth, H7 optional hero, H8 badges,
H9 why/not-interested, L2 saved views, L5 list view, L6 type-ahead, L9 collection management,
L10 playlist 2.0, L11 summary header, L12 needs attention, X4–X10 detail depth, C4/C5 local history
and stats, Q5 crash-safe navigation, T1 tray, T2 notifications, T3 MPRIS push/shuffle/loop, A2/A3/A6
themes and shortcuts, V1–V4 live depth, R3/R4/R7 optional memory/sharing.

Keep the collector work behind small commits; each new detail field or badge gets a mocked API
fixture and a snapshot.

---

## Phase 3 — Downloads and offline (Q9 + Q4)

### Storage and settings

- `Settings → Downloads`: a **path** field (default `~/.cache/runefin/downloads`), a storage budget
  (default e.g. 20 GB), and “delete after watched”. Validate the path exists and is writable;
  show the resolved path and free space. The path is stored in `config.toml` (0600).
- Downloads live under `<path>/<safe-item-id>/` with a JSON manifest (item id, source id, size,
  checksum, state, created, watched). Never use raw item ids in paths without sanitising.
- If the path is unavailable at startup, the Downloads page explains it and playback/downloads
  stay disabled; the rest of the app works.

### Worker and queue

- One download worker (never blocks the UI): `GET /Items/{id}/Download` (or the PlaybackInfo-selected
  direct URL), temp `.part`, HTTP Range resume when the server supports it, checksum/size check,
  atomic rename, manifest update.
- Queue: pause/cancel/resume/retry; one or two concurrent downloads; persist the queue so it
  survives restart; budget enforcement before starting.
- Optional “download next 3 unwatched episodes” after a season is opened.
- Reports queue locally and flush on reconnect; a failed report never blocks playback.

### Offline experience

- A Downloads page with progress, size, path, pause/cancel/delete and “play”.
- When the server is unreachable: play downloaded files from `file://` through the same pipeline;
  show downloaded items/pages; disable non-downloaded playback with a reason.
- Cached Home/Library/Detail pages stay readable (Q4); no local database is introduced — the
  existing in-memory/disk cache is enough for the last rendered pages.
- Delete after watched optional; removing a download removes the manifest and files atomically.

### Tests and evidence

- `runefin`: path validation, queue persistence, range resume with a fake worker, budget refusal,
  offline playback selects a downloaded file, report flush.
- No network in tests; use a local temp directory and a fake HTTP server/client at the boundary.
- Ignored live test: download one short item to a temp path, verify size/checksum, play offline.
- Snapshots: `settings-downloads.png`, `downloads.png`, `downloads-offline.png`.

### Done when

A queued download survives restart; pause/resume works; a downloaded episode plays with the server
off; the chosen path is visible in Settings; deleting the download frees the file and entry.

**Risk:** storage/cleanup, path permissions, DRM-free local files, and the size of the feature.
Do not start before P4/P1/P2; it is the largest item in the backlog.

---

## Phase 4 — Later / not scheduled

- **A7** i18n + RTL: lowest priority; only after the product stabilises.
- **Never** (unless the product changes): SyncPlay, music/books/photos, multi-server aggregation,
  web view, social feed, storefront.
- **Not scheduled:** V6 recording timers (provider 500), T7 black out monitors, A5 (enable Rune
  accessibility instead), H3, S4, C6 multi-user, R6 SyncPlay.

### Phase 5 — optional Seerr companion (separate track)

The user asked for the ability to discover and request films/series through **Seerr** (the merged
Overseerr/Jellyseerr) and to see their download progress. This is not a Jellyfin feature and not
part of Q9: Seerr is an optional companion service with its own crate, credential and page. It does
not add a second media server, a merged catalogue, a web view or an async runtime.

- Discovery: [`../research/08-seerr-client.md`](../research/08-seerr-client.md).
- Plan: [`2026-11-05-seerr-requests-plan.md`](2026-11-05-seerr-requests-plan.md).
- Gate: SD1–SD11. With no Seerr configured, the app and every snapshot stay as they are.
- Recommended order: S0.0 live capture → S0 crate/settings → S1 read-only Requests → S2 Discover
  and request → S3 Jellyfin detail integration.

Do not start it before the decisions are answered; do not fold it into Phase 3's download queue.

---

## Commit and verification strategy

- One commit per opportunity or small cluster; a design change amends `design.md` /
  `design-player.md` in the same commit.
- Every pixel change ships `scripts/snapshots.sh` PNGs; every API shape gets a captured fixture.
- `scripts/check.sh` before every commit: fmt, clippy `-D warnings`, 42 API + 269 app tests, release.
- Live tests stay ignored and opt-in; no test hits the network.
- The first release that includes P4 must state direct-play-first in the release note; the first
  with Q9 must state the default download path and budget.

## Measures

- Direct-play share, transcode startup and seek latency.
- Resume accuracy and checkpoint recovery.
- Segment skip use and undo recovery.
- Filter/search time to result; frame budget on dense screens.
- Download throughput, disk usage, offline playback success.

## Open questions

1. Collector vs casual: versions/extras/filters vs discovery/stats.
2. Theme: is light/high-contrast a real request?
3. TV/Steam Deck: is a 10-foot mode a target?
4. Trakt/Letterboxd: external memory or local history only?
5. Server version: when does the live server move to 12.x?
6. Downloads scope: movies, next episodes, or the whole queue first? (Storage path and budget are
   decided.)
