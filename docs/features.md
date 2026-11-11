# Runefin — implemented features

Inventory of what the app does today, as of **2026-11-07** on `main`. This is the user-facing
list; the contract is [`design.md`](design.md), the evidence against other clients is
[`research/06-parity-audit-2026-11.md`](research/06-parity-audit-2026-11.md), the forward-looking
opportunity brief is [`research/07-feature-opportunities-2026-11.md`](research/07-feature-opportunities-2026-11.md),
and the operational notes are [`engineering.md`](engineering.md). If this file and the code disagree, the code
wins until both are fixed in the same change.

Status words:

- **Done** — shipped, covered by tests and exercised against the live server.
- **Partial** — shipped with a named limitation.
- **Blocked** — the server or the host cannot do it here.
- **Out of scope** — a deliberate product decision (one server, one window, no web view).

The product is a **single-server, single-window Jellyfin client**. It browses the server, plays
video in the same window through GStreamer, and reports playback back to Jellyfin. It is not a
web wrapper and does not embed a second render stack.

---

## 1. Connect and session

| Feature | What it does |
|---|---|
| Server probe | `GET /System/Info/Public`; shows the server name/version, rejects a too-old server, and normalises `host:port` to `http://`. |
| Password sign-in | `POST /Users/AuthenticateByName`; the token, user and server are stored in `~/.config/runefin/config.toml` with mode `0600`. |
| Quick Connect | Initiate, poll, then exchange the secret for a token (the token is only on the third call). |
| LAN discovery | UDP 7359 `who is JellyfinServer?`; replies are listed by name and fill the server field. Broadcast can be blocked by the network; the live probe is diagnostic. |
| Session persistence | Reopens signed in; refuses to render a stale fixture while signed out; sign-out clears the session. |
| Error states | Unreachable server, bad credentials, expired token and Quick Connect failure each have their own message; the connect form keeps what was typed. |

## 2. Shell, navigation and window

- Collapsible sidebar: Home, one entry per server library in server order, Live TV, Playlists,
  **Discover** (only when Seerr is configured), then Search, Remote and Settings; server name and
  connection state; user chip with sign-out.
- One navigation stack of `(route, scroll offset, focused key)`; **Esc** pops and restores both.
- Shared-element morph from a card's poster into the detail page, and back.
- Window content size and positive screen position are remembered on close and requested on the
  next launch; the compositor may ignore placement on Wayland.
- **Fullscreen**: `F11` anywhere, or the player's `Fullscreen` / `Windowed` chip. Borderless
  fullscreen through winit; remembered across hide/show; panels and overlays ignore it.
- **Two interface modes**: Desktop (default) and **Big screen**, chosen in Settings → Interface,
  by `--big-screen`, or by `RUNEFIN_UI=big` for one launch; the file's `ui_mode` is the default.
  Big screen keeps the 1280×720 design height (1.5× at 1080p, 3× at 4K), a 64 pt safe gutter,
  fullscreen on launch and initial focus on each screen's first item. A mode change takes effect
  at restart and the Settings row says so.
- **Controller navigation** in both modes: D-pad/left stick moves, A activates, B goes back,
  LB/RB change focus group, Start opens the focused card's menu or the player's gear. The pad is
  read natively through `gilrs`; `RUNEFIN_GAMEPAD=0` disables it.
- Keyboard shortcut sheet (`?`): search, playback, subtitles, live channels, volume, menus,
  fullscreen and navigation. Player actions can be remapped in Settings → Keyboard; a duplicate
  binding is refused.
- **Text size** in Settings scales every type token (85 %, 100 %, 115 %, 130 %) without
  changing poster or card geometry.
- Toast for short confirmations ("Added to the playlist", "Image subtitles ready") with its own
  timer; no modal dialog for a one-line answer.

## 3. Home

- Continue Watching, one card per series, newest first; entries untouched for 60 days are
  hidden and finished items are dropped.
- **Next Up** streamed beside the first paint, so a slow answer never delays Home.
- One **Recently Added** rail for every video library, then a **Recently Played** rail. That
  rail is deduplicated by series and never repeats a Continue Watching card. Home has no rail
  cap; it scrolls.
- **See all** on Continue Watching, Next Up and Recently Played opens a full grid; a Continue
  Watching card has a hover **Remove** action with Undo.
- Collections rail so box sets are reachable without opening a library.
- **Compact Home hero** (Settings → Interface, off by default): one 180 pt card above Continue
  Watching with the first visible entry's own backdrop, `S2E04 · Overtime · 12 min left`, and
  Resume / Details. It takes the first focus while it is on, Up from any rail reaches it, and it
  skips hidden or empty entries rather than borrowing another title's art.
- Full-bleed ambient backdrop, item logo when the server has one, otherwise the title.
- Poster art is fetched for **every card in a rail** when Home loads, not just the first
  screenful; rails never uncover empty boxes until a click.
- Skeleton, empty, error and loading states with matching final geometry.
- Every text label that sits on artwork carries a soft shadow, so a bright backdrop never
  swallows the words.

## 4. Libraries

- Virtualized grid or list; only the visible rows are built, so a 20 000-item library stays
  under the frame budget.
- 100 items per fetch, one page prefetched; the wheel/trackpad scrolls vertically and reopening
  a library restores the previous offset.
- Sort by Name, Added or Rating on the toolbar, with ascending/descending direction; the Filters
  sheet adds Played, Release date, Runtime, Play count and Random.
- Filters: All / Unwatched / Favourites; a Genre menu; an A–Z letter rail when sorted by name.
- Grid/list toggle; card shows poster, watched badge and resume progress.
- Quick play puck on hover; it does **not** navigate. Right-click or **Menu / ⇧F10** opens the
  item menu (watch state, favourite, resume removal, add to playlist/collection, Play on…).
- Playlist editing: remove entries, move them up/down, or drag a poster to move it by whole rows.
- Collections open as a library; a box-set detail page lists its members and can sort them by
  server order, name, year or runtime. Items can be added to an existing collection from a card,
  the detail page or the bulk bar.
- Server-side shuffle and Play all for a library, season, collection or TV shuffle.
- Empty, loading and error states.

## 5. Search

- **Ctrl+K** searches the whole server; **Ctrl+F** searches the open library and shows its scope
  as a removable chip.
- 150 ms debounce, minimum two characters, every keystroke cancels the in-flight request.
- Results grouped Movies · Shows · Episodes · People, six each; **See all** expands a group into
  a grid in place and **Show less** collapses it.
- Idle suggestions/recents, empty and error states.
- A person result opens their filmography.

## 6. Detail, person and collections

- Full-bleed backdrop with a two-axis scrim; server `Logo` wordmark or the title, with the
  readable-text shadow.
- One metadata line, then IMDb / TMDb / Rotten Tomatoes scores (see §10).
- Technical line from `MediaSources` with a **Details** disclosure (codec, resolution, range,
  container, size).
- Primary action (**Play** / **Resume 42:13**), **Start over**, **Trailer**, **Favorite**,
  **Watched**, **Playlist**, **Collection**.
- Overview clamped/expandable.
- Series: season chips, virtualized episode rows, Play season / Shuffle, next-up episode with
  its own resume action.
- Extras rail grouped by server `ExtraType` (behind the scenes, deleted scenes, interviews, …); an extra plays in-app without reporting and outside the queue.
- Cast and crew rail with portraits; a cast card opens the person page.
- **More like this**: the server's `/Items/{id}/Similar` rail, loaded beside the page so a slow
  recommendation answer never delays the hero.
- Playlist picker and a **New playlist** prompt that starts with the current item.
- Availability aware: a missing stream disables playback with a reason.
- Back restores the card, the library offset and the detail page's own nested state.
- Person page: portrait, name, credit count and the same poster grid, so an actor is a browse
  path.

## 7. Playlists

- Sidebar section listing the server's playlists, in server order.
- Ordered grid with its own endpoint; Play all / Shuffle from the playlist page.
- Create a playlist with the current item; add an item from a card menu or a detail page.
- Drag a poster or use the up/down controls to reorder; remove entries.
- Live create/add/move/read/remove/delete was exercised against the server.

## 8. In-app player

The player is a GStreamer pipeline in the same window, not an embedded browser and not a
separate window. `scripts/check.sh` builds it; the player's contract is
[`design-player.md`](design-player.md).

### Video path

- **Zero-copy DMA-BUF** on Linux: a custom `BaseSink` imports the decoder's `P010`/`NV12`/`BGRA`
  frames into Rune's Vulkan device. Tested at 4K HEVC HDR10 on AMD RADV/VCN: 256 ms to first
  frame, 24 fps, 232 MB RSS.
- **HDR tone mapping** for PQ/HLG to the SDR window, calibrated so reference white lands at 0.8
  and 1 000/4 000-nit highlights at 0.976/0.994. True HDR passthrough is not shipped (see §13).
- **CPU NV12 fallback** when VA-API or DMA-BUF negotiation fails; it copies frames through
  system memory and plays SDR reliably.
- **mpv fallback** if the in-app pipeline cannot produce a frame; the same start/progress/stop
  reporting runs.
- Direct play of the containers GStreamer supports (MKV/MP4/AVI, HEVC/H.264/AV1, 10-bit).
  Playback reporting is three calls with the right payloads: start, progress, stop (the stop
  payload carries the final position and no pause/seek fields).

### Timeline and OSD

- Minimal OSD that fades after idle and returns on pointer move or key: Back and Fullscreen in
  the top bar, three centred controls (back, play/pause, forward) and one timeline row with the
  clock, queue position, volume, mute and a settings sheet. Every other control is a row in that
  sheet, showing its current value; `Esc` closes a panel, then the sheet, then stops.
- Segment bands for intro/recap/credits/preview, with a time-range label, click-to-seek,
  **Ask / Always / Never** skipping and Undo. Ask shows a small bottom-right skip pill five
  seconds early, labelled by the segment kind (intro, credits, recap, preview);
  Always waits for the segment to start. Timeline with chapter ticks; hover shows a **240×135 trickplay preview** in a 300 pt panel
  with the chapter name and timestamp. Without server trickplay the same panel shows the time
  and says “No preview for this video” rather than a fake still.
- Resume starts 0/5/10/15 s before the server position (setting), and a local checkpoint under
  `~/.cache/runefin/positions.json` covers a crash; a newer local position wins.
- **Still watching?** pauses a long run after N episodes or minutes with a quiet Continue/Stop
  card and a 60 s countdown; a **Sleep timer** offers 10/20/30/45/60 minutes or end of episode and
  fades the last 20 s.
- Multiple media sources appear as a **Version** row on detail; the chosen source is used for
  playback and remembered for the session.
- **Watchlist** in the sidebar: the server favourite flag as one cross-library page, written by
  the same card/detail toggle.
- Search **hints** from `/Search/Hints` under the field: people, genres and studios the result
  rails do not show as rows; a chip fills the query.
- Playlist **Undo** for an added item, a removed entry and a move; the page updates before the
  server answers.
- **Like / Dislike** on the detail page and card menu (`UserItems/{id}/Rating`).
- Library **Filters** sheet: both facets, Name/Added/Rating/Played/Release/Runtime/Plays/Random,
  direction, genre chips, year, minimum rating, audio/subtitle language, “has subtitles” and
  “4K only”;
  **Select** for multi-select with Mark watched; **4K/HDR/CC/5.1/Atmos/VERS/NEW** text badges
  from the streams and `DateCreated`.
- **Playback negotiation**: `POST /Items/{id}/PlaybackInfo` with a device profile; direct play is
  the default, a bitrate cap or unsupported stream uses DirectStream/Transcode, and reports carry
  the method and server `PlaySessionId`. Live-verified on the home server: direct play stays
  direct, a 2 Mbps cap returns HLS, and the app decodes and seeks the transcode.
- **Quality menu** in the player's settings sheet (Auto, Original, 20/12/8/4/2 Mbps) with an explicit method label
  and a Why? line; changing it renegotiates at the current position. The chosen quality persists
  per item and as the last global choice.
- **Pre-playback** names the decision and reason, and lets the quality be changed before play.
- **Frame step** with `,` / `.`, **A–B loop** with a coloured range on the timeline, and a
  **Go to** time panel accepting `mm:ss` or `h:mm:ss`, both reached from the settings sheet.
- **Frame screenshot is not exposed yet**: the DMA-BUF sink keeps frames on the GPU and has no
  CPU readback; capturing one needs a tee/readback branch in the player, not a UI action.
- Library **type-ahead** search (debounced), the header count now says how many filters are
  active, and empty-state copy still explains the active filter.
- **Light theme and high contrast**, a system tray icon with play/pause/next/stop, and a compact
  Home hero option.
- **Guide window** switches between 3 hours and a full day; channel favourites filter both.
- **Copy link** on the item menu, and **MPRIS shuffle/loop** properties now have real
  setters (loop maps to the A–B range).
- **Live channel favourites** filter the list and the guide; the set persists in the config.
- **Crash-safe navigation**: the last library or detail page is remembered in the config and
  reopened after the first Home load with a short “Returned to where you left off.” toast.
- **Local watch history and stats**: one JSONL line per completed play under
  `~/.cache/runefin/history.jsonl`; Settings shows sessions, minutes and the last five, with a
  clear action and a Letterboxd CSV export. No server call and no database.
- **Saved views**: name the current sort/filters and re-apply them later; local config.
- **Bulk playlist adds**: a multi-select can go straight into the playlist picker.
- **Collection management**: create a collection from a multi-select, add items to an existing
  collection from a card, detail page or bulk bar, remove a member, and sort a collection by
  server order, name, year or runtime without rewriting the server's order.
- **Playlist reorder**: Edit on a playlist page exposes a drag handle on each poster as well as
  the up/down controls; a drop moves the entry by whole rows.
- **Covers are blocked on the home server**: Jellyfin's
  `POST /Items/{id}/Images/Primary` answers 500 for JPEG, PNG, raw and multipart uploads on this
  server, so collection/playlist cover actions are not exposed. No upload path ships until the
  server accepts one.
- Narrow windows get two library columns instead of clipping a third, the detail hero stacks its
  poster above the text, and the filter sheet scrolls instead of running past the window.
- **Images fetch in parallel** (four workers, high before low), library columns follow the window
  width up to twelve, and capability badges cap at three plus a `+N` count.
- **Richer list view**: year, rating, runtime, date added, play count, file size and 4K/HDR/CC
  facts on one row.
- **Detail metadata** adds original title, release date, series status, studio and tags when the
  server sends them; episode rows show their air date, and the season header can mark the whole
  season watched.
- **Recently Added** for every video library, and **Part of …** collection links on detail.
- **Not interested / Why this?** on a Home card hides it locally (persisted, with Undo) or names
  the rail's actual reason.
- **Queue 2.0**: Play next, Add to queue, reorder up/down, remove, and **Save as playlist** from the queue panel.
- Resting timeline is a 4 pt rule; under the pointer it doubles to 8 pt, brightens the unplayed
  track and grows a white knob. Drag to scrub; the preview follows the pointer.
- **Hold the seek target** after a flushing seek until the player's clock reaches it (or four
  seconds), so the bar no longer snaps back to the old second.
- Chapter list with server stills, names, start times and the current chapter marked; clicking
  a row seeks and keeps the list open.
- Skip intro / credits / recap when the server publishes segments.
- Play/pause, ±10 s, volume, mute, pitch-corrected playback speed (0.5×–2×), Fit/Fill,
  previous/next episode.
- **End-of-episode card** in the last 30 seconds: the episode shrinks to a corner over a
  dimmed next still, with runtime, overview, countdown, Play now, Watch credits and Dismiss;
  the card takes focus. Auto-roll at EOS unless dismissed. A run's queue is shown by a
  `3 of 12` chip that opens the queue panel.
- Buffering percentage; it wakes above the OSD and never flashes on a healthy start.
- Diagnostics overlay: sink path, codec/resolution/range, active audio/subtitle, speed, position,
  frame count, cache/queue counts, server and last error.
- Idle/sleep inhibitor while playing; volume is remembered.
- Mini player while browsing: poster, title, elapsed/remaining, play/pause, stop and reopen.
- **MPRIS2** (see §11).

### Audio and text

- Audio track menu from `MediaSources`, selected by playbin's own `current-audio`; a preferred
  language applies once, manual choices are never overridden.
- Text subtitles: SRT, VTT and ASS, with inline bold/italic/colour, `<font color>`, VTT colour
  classes, `{\anN}` alignment and `\pos`.
- Full appearance: Sans/Serif/Mono, six colours, Off/Thin/Thick outline (glyph runs, not a
  rectangle), Normal/Bold/Italic, Bottom/Middle/Top, text opacity and plate opacity.
- Delay (±100 ms) and size (±10%); default keys `c` cycles tracks, `[`/`]` delay, `-`/`=` size
  (`c` can be remapped in Settings → Keyboard).
- **PGS and VobSub** bitmap subtitles extracted with ffmpeg into the subtitle cache and decoded
  with `libbitsub-core`; **DVB** is transcoded by ffmpeg to DVD subtitle packets and read by
  the same parser. All are normal selectable rows.
- Online subtitle search and download into the player, then selection.
- Server-hosted trailers play in-app; YouTube trailers download once through optional `yt-dlp`
  into `~/.cache/runefin/trailers` and play in-app with reporting disabled. Without `yt-dlp`
  the action opens the browser.

## 9. Live TV

- Channel list (`/LiveTv/Channels`) with the current program and the next one when the guide
  has it.
- Guide mode: 15-minute slices over three hours, one row per channel, Now/+30m markers, the
  current program tinted; clicking a row tunes it.
- `/LiveStreams/Open` and `/LiveStreams/Close` on every path; the tuner is closed before the
  next channel opens.
- In the player: LIVE marker, program name, no seek/remaining, `PgUp`/`PgDn` and the OSD's
  `⏮ CH`/`CH ⏭` zapping, wrapping at the ends.
- Recordings open as a library.
- **Program reminders**: the bell beside a channel's next program stores a local reminder and
  shows an in-app toast when it starts; Settings can add a desktop notification through
  `notify-send`. No server timer is involved.
- **Recording timers are blocked** by this Xtream provider (HTTP 500 on `/LiveTv/Timers`).

## 10. Scores and metadata

- TMDb is Jellyfin's `CommunityRating`; Rotten Tomatoes is `CriticRating`. Both come with the
  item.
- IMDb is not in Jellyfin's schema: the first page with an IMDb id downloads IMDb's public
  `title.ratings.tsv.gz` once, builds a 10 MB local index under `~/.cache/runefin` and answers
  with a binary search. It is reused for 30 days. An optional OMDb key in Settings is the
  fallback when that download fails.
- Missing scores leave a compact link/empty badge rather than a zero.

## 11. Desktop integration

- **Fullscreen** from `F11` or the OSD chip (§2).
- **MPRIS2** on the session bus: identity, PlaybackStatus, title/series/album, length,
  position, volume, rate, seek, next/previous, and the root `Fullscreen` property. Commands
  from the desktop are queued and applied on the UI thread; the art URL is the cached poster's
  `file://` path, never a token-bearing server URL.
- The service runs only with a session bus; headless renders and a bare TTY simply have no
  MPRIS. MPRIS changes are read on demand rather than pushed as `PropertiesChanged`.
- Keyboard-first and mouse-first both work: one focus ring, roving focus, Tab between regions,
  Esc back, spatial arrows in grids.
- **Flatpak for SteamOS**: the package installs without root, exports Desktop and Big Screen
  entries, exposes `/dev/input` for the native pad, and uses the Freedesktop 26.08 GStreamer/
  VA-API stack. On a Steam Machine the config lives at
  `~/.var/app/io.github.lifk.Runefin/config/runefin/config.toml`.

## 12. Configuration and storage

- `~/.config/runefin/config.toml` (0600): server, token, user, device id, subtitle/audio
  preferences, text size, player shortcuts, desktop-notification opt-in, live reminders,
  optional Seerr URL/key/user id, volume, window geometry.
- `~/.cache/runefin/images`: poster/backdrop cache, keyed by the server's immutable image tag.
- `~/.cache/runefin/subtitles`: extracted PGS/VobSub/DVB tracks.
- `~/.cache/runefin/trailers`: downloaded YouTube trailers.
- `~/.cache/runefin/imdb-ratings.bin`: IMDb index.
- Settings: Playback, Subtitles & audio and Keyboard cards, plus metadata, storage, history and
  account. Includes settings search, reset, export, text size (85–130 %), remappable player
  shortcuts, desktop-notification opt-in, OMDb fallback key, clear poster cache, remote control
  shortcut and sign-out. A **Requests (Seerr)** card holds the optional companion's address,
  masked admin key, user id, Test and Clear; S0 probes the connection only.
- Audio delay (±100 ms, 25 ms steps, applied with `identity ts-offset`); pitch-corrected speed;
  optional downmix; Dialogue boost and Night mode (`audiodynamic`).
- Theme songs play beside detail; a muted theme video can sit behind the hero. Both are off by
  default, stop on navigation or playback, and reduced motion refuses the video.
- Pre-playback sheet names version, container, codecs, HDR, subtitle count, size and the
  direct-play decision; the Version row and chevron choose a source before it starts.
- A newer local checkpoint after a crash offers "This device / Server" instead of silently
  choosing; "Continue on…" sends the current position to another session and stops locally only
  after the server accepts.

## 13. Requests and discovery (Seerr)

The optional Seerr/Jellyseerr companion is a second service, not a second media server. Settings
holds the address, the masked admin key and an optional user id; **Test** proves `/status` and
`/auth/me`.

- Sidebar **Discover** appears only when a URL and key are configured.
- **Discover** has two modes. **Explore** shows trending, films and series rows; **Browse** is a
  filtered grid: films or series, a genre, a studio (films) or network (series), a decade, and
  Popular / Top rated / Newest, with **Load more**. Genres come from `/genres/{movie,tv}`; the
  studio and network chips are a curated TMDB-id list. Search with the standard debounce sits
  above both, and a card opens the Seerr detail.
- **Request**: a film one click; a series season picker (All or individual; specials hidden when
  the server disables them). Duplicate, quota and permission answers come from Seerr's own
  message.
- **Requests**: **Active** (pending + processing + partial) by default, with an **All** chip.
  Titles and posters come from a cached `/movie` or `/tv` summary because `/request` has neither.
  Progress is byte-accurate when Radarr/Sonarr sync is on; a completed pack is counted once by
  `downloadId`; without queue rows the row says `Waiting for a download`.
- A request whose title is on the server opens the Jellyfin detail even when Seerr still calls it
  Processing: the app resolves the Jellyfin id by TMDB id (title search + `ProviderIds.Tmdb`) when
  Seerr has not linked it, so a partially downloaded series has a watch button. A partial series
  also offers **Request missing seasons**. A Jellyfin series offers the same from its TMDb id.
- The Seerr detail adds genres, a tagline and a **Cast** rail. A face opens the person page.
- **The main search covers both catalogues**: the same query also goes to Seerr, and an **On
  Seerr** section follows the library results with availability chips; a card opens the Seerr
  detail. A scoped `Ctrl+F` search still sends the unscoped query to Seerr, because the scope is a
  Jellyfin library.
- **Person pages join both catalogues.** The library titles come from the server's `personIds`
  credits; the same person's TMDB credits come from Seerr, minus anything already on the server,
  newest first. The header carries the biography, birthday and place of birth. A person opened
  from a Seerr cast is matched to the server's person by TMDB id (name search, then
  `ProviderIds.Tmdb`), so the library section appears there too.
- Polling is 5 s and only while the Requests tab is visible; leaving stops it. No background
  poll, no web view, no async runtime.

## 14. Limits and deliberate non-features

| Limit | Why |
|---|---|
| **True HDR passthrough** | HDR10/HLG is tone-mapped to the 8-bit sRGB window. Passing PQ through unchanged needs an HDR swapchain and color space in Rune (M5d), not a shader flag. |
| **NVIDIA zero-copy** | The DMA-BUF path is tuned and tested on AMD RADV/VCN. NVIDIA would need `nvidia-vaapi-driver` or an NVDEC branch; without it the player falls back to CPU (SDR) and HDR can look wrong. |
| **DVB live proof** | No DVB track exists on this server. The path was validated with a DVB MPEG-TS generated from an on-server DVD track (720×480 bitmap). |
| **MPRIS signals** | Properties and commands work; no `PropertiesChanged` push, so a client that caches without polling refreshes when it next asks. |
| **Recording timers** | The Xtream provider answers HTTP 500. |
| **Image covers** | Jellyfin's `POST /Items/{id}/Images/Primary` answers 500 for JPEG, PNG, raw and multipart uploads on the home server. Collection/playlist cover actions are not exposed until it accepts an upload. |
| **Frame screenshot** | The DMA-BUF sink keeps the decoded frame on the GPU and exposes no CPU readback. A screenshot needs a tee/readback branch in the player, so there is no UI action yet. |
| **Trakt scrobbling** | Needs an account and OAuth credentials; the local watch history and Letterboxd CSV export work without them. |
| **Desktop notifications** | Only live program reminders use `notify-send`. Next-episode, download and error notifications are not implemented. |
| **Music, photos, books** | No such libraries on the server; not the product. |
| **SyncPlay** | Needs a WebSocket client; the design excludes that dependency. |
| **Downloads/offline** | Planned (Q9): a Settings-chosen storage path, a download queue and offline playback. Not implemented yet. |
| **Seerr companion** | Optional. Without a URL/key the app is unchanged. Progress needs **Sync Enabled** on Seerr's Radarr/Sonarr entries; 4K requests, Seerr administration, issues and deletion are out of scope. |
| **Multi-server aggregation** | Product is one server, one window. |

---

## Controls

| Key | Action |
|---|---|
| `?` | Shortcut sheet |
| `Ctrl+K` / `Ctrl+F` | Search the server / search this library |
| `Esc` | Back, close a panel, stop playback |
| `Space` | Play/pause |
| `←` / `→` | Seek 10 s |
| `↑` / `↓` | Volume |
| `M` | Mute (remappable) |
| `C` | Cycle subtitle tracks (remappable) |
| `S` | Skip intro/credits (remappable) |
| `[` / `]` | Subtitle delay |
| `-` / `=` | Subtitle size |
| `>` / `<` | Playback speed |
| `N` / `P` | Next/previous episode (remappable) |
| `PgUp` / `PgDn` | Live channel up/down |
| `F` | Fit/fill (remappable) |
| `F11` | Fullscreen (remappable) |
| `Menu` / `⇧F10` | Poster item menu |

Controller (standard gamepad): D-pad/left stick = arrows; A = Space; B/Select = Esc;
LB/RB = Shift+Tab/Tab; Start = Menu. Text entry uses Steam's on-screen keyboard on the Steam
Machine, or the desktop keyboard. HDMI-CEC TV remotes arrive as keyboard input on SteamOS 3.8.1+
(`cecd`); `packaging/flatpak/cecd.toml` maps Select to Space and the transport buttons to the
keys the player already uses.

## Verification

```sh
cargo test -p jellyfin-api        # 45 API tests, no network
cargo test -p seerr-api           # 25 Seerr API tests, no network
cargo test -p runefin             # 347 app tests (22 ignored), headless
scripts/check.sh                  # fmt, clippy, all three suites, release build
scripts/snapshots.sh              # the committed views, in place
JELLYFIN_URL=... JELLYFIN_USER_ID=... JELLYFIN_TOKEN=... \
  cargo test -p jellyfin-api --test live -- --ignored --nocapture
target/release/runefin            # the signed-in app
```

Personal live captures (`--live --snap`) are for debugging and are never committed; the
committed captures are the hermetic set in [`snapshots/`](snapshots/README.md).
