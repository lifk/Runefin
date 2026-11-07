# Runefin — feature and UX opportunities, 2026-11

**Status:** research brief. No code changed. This document is evidence for the next product
conversation, not a contract; anything adopted here still has to amend `docs/design.md` (or
`docs/design-player.md`) in the same change, per `AGENTS.md`.

> **Post-audit note (2026-11-05).** Some findings below are now history: the Home contract has no
> six-rail cap and no Coming up rail, every video library gets a Recently Added rail, and H1, H4,
> H6, L9 (except covers), L10 (except covers), A2, A3, A6, V4 and Q5 are shipped. Use
> [`../features.md`](../features.md) for the current inventory; the opportunity notes stay as
> written so the reasoning is auditable.

**Scope.** What Runefin could add next, and where the existing UX leaks value. It starts from the
shipped inventory in [`features.md`](../features.md), the contracts in [`design.md`](../design.md)
and [`design-player.md`](../design-player.md), the engineering notes in
[`engineering.md`](../engineering.md), and the existing research in `01`–`06`. It then compares a wider set
of media apps — streaming services, media centres, desktop players, trackers and companion
services — and turns the comparison into a prioritised opportunity list.

**Method.**

1. Read the docs, the screen code (`crates/runefin/src/screens*`, `state.rs`, `player.rs`,
   `home.rs`, `library.rs`, `detail.rs`, `net.rs`) and the `jellyfin-api` wire types.
2. Inspected the Jellyfin **12.4.0** OpenAPI document directly (1.9 MB, 12.4.0) for the endpoints
   and fields behind each idea, rather than relying on blog posts. The live server was upgraded
   to **12.4.0** on 2026-11-05; anything introduced in 12.0 is marked **12.0+** and was
   feature-detected, not assumed.
3. Surveyed more than 20 apps and services (below), with primary sources where they exist.
4. Scored each opportunity on viewer value, effort, risk and dependencies, then grouped it into a
   phase. Effort is deliberately blunt: **S** ≤ 2 days, **M** 3–7 days, **L** 2–4 weeks,
   **XL** 1–2 months, for one implementer who also writes the tests and snapshots.

**Constraints that shaped the recommendations.**

- One server, one window, one user (today); no web view, no async runtime, no second render stack.
- Rune draws everything; the player is GStreamer + DMA-BUF; the Rune rev is pinned.
- Every interactive element needs a stable `.key(..)`; any pixel change ships a snapshot; no
  network in tests; no new dependency without a line in `docs/design.md`.
- The product is a **browse + play** client. Recommendations below that turn it into a different
  product (music library, book reader, multi-server aggregator) are listed as non-goals.

**Short version.**

1. **Fix the truth first.** The Home contract and the code disagree (label, rail order, cap, missing
   Recently Played), and Settings has outgrown one column. These are Phase 0, hours not weeks.
2. **The best return is player quality of life:** segment bands + auto-skip, “Still watching?”,
   sleep timer, resume continuity (rewind/checkpoint/transfer), versions/editions, extras/theme
   media, dialogue boost/night mode.
3. **Browse needs the server’s real filter set**, saved views and multi-select; the UI currently
   exposes a fraction of what `/Items` and `/Items/Filters2` can do.
4. **Search should become a command surface.** `Ctrl+K` for items, people, genres, studios and
   commands; `/Search/Hints` is already in the API.
5. **Continuity and memory:** a watchlist built from Jellyfin favorites or a user playlist, likes,
   undo, local history/stats, optional Trakt. Keep the server the source of truth.
6. **The biggest capability gap is PlaybackInfo/transcoding.** Direct play is the default and
   should stay that way; transcode only when the reader chooses a lower bitrate or the app cannot
   reproduce a codec/container/audio/subtitle. The server supports it, so this is Phase 2.
7. **Desktop and accessibility are the differentiators:** tray, notifications, deep links,
   always-on-top/compact, MPRIS push, reduced motion, light/high-contrast, text scaling, audio
   description.
8. **Do not build** SyncPlay, music/books/photos, multi-server, fake AI recommendations, autoplay
   previews, or a web view. Each has a named reason and evidence in §7.
9. **Downloads are planned**, not out of scope: a Settings-chosen storage path, a download queue
   and offline playback of the stored files.

> **Revision 2026-11-05 — decisions applied.** This revision reflects the project owner's
> decisions, not a new survey. **Removed:** A5 (screen-reader investigation — Rune has an
> accessibility tree; the app currently builds with that feature off), H3 (“Surprise me”), S4
> (recent searches / idle suggestions / zero-result recovery, all shipped), V6 (recording timers,
> blocked by the provider), R6 (SyncPlay), C6 (multi-user) and T7 (black out other monitors).
> **Merged:** H1 + H10; H5 + R2; P3 + C1–C3; P5 + X3 + P13; P6 + X1 + X2; P9 + Q6; Q4 + Q9.
> **Changed:** H8 drops “Leaving” (no server data); T3 is MPRIS push + shuffle/loop only
> (artwork ships); P4 is unblocked and follows a **direct-play-first** policy (transcode only for
> a chosen bitrate or unsupported codecs/container/audio/subtitle); Q9 is downloads + offline
> browse with a Settings path; A7 is the lowest priority. The catalogue in §3 and the roadmap in
> §6 already reflect this revision. A5’s real first step is enabling Rune’s `accessibility`
> feature, which the app currently omits. The implementation plan is
> [`../plans/2026-11-05-implementation-plan.md`](../plans/2026-11-05-implementation-plan.md).

---

## 1. Baseline, and the places where the docs and the code disagree

The shipped surface is genuinely broad for a one-window client: connect (password, Quick Connect,
LAN discovery), home rails, virtualised libraries with sort/filter/letter rail, search, detail with
seasons/episodes/cast/similar, playlists, an in-app GStreamer player with zero-copy DMA-BUF, HDR
tone mapping, text/bitmap subtitles and full appearance, live TV with a guide, MPRIS, remote
control and a mini player. [`features.md`](../features.md) is the inventory; this section only
records the gaps that matter for the next pass.

### 1.1 Contract vs code

| # | What the contract says | What the code does | Why it matters |
|---|---|---|---|
| 1 | `design.md` §3: Home is **Continue Watching → Next Up → Recently Added → Recently Played → Collections**, at most six rails. | At audit time `screens.rs::rails` drew **In progress → Next up → Collections → up to four Recently Added**. **Resolved 2026-11-05**: label, order, every-library rails and the missing Recently Played rail shipped; there is no cap. | The most important rail changes position; the label is weaker; the cap can be exceeded (1 + 1 + 1 + 4 = 7). |
| 2 | `design.md` §3 calls for a **Recently Played** rail. | `home.rs` has no `recently_played` field and no loader; `HomeData` is `resume`, `next_up`, `collections`, `latest`. | The contract is ahead of the code. Build it (H1) or amend the contract; do not claim it in `features.md` until it exists. |
| 3 | `design.md` calls the first rail **Continue Watching**. | The UI says **In progress**. | “Continue Watching” is the industry-standard label and matches the server endpoint; “In progress” reads like a status, not an action. |
| 4 | `research/02` recommends a split Settings IA (Appearance, Playback, Network, Cache, Shortcuts, About). | `screens/settings.rs` is one long column of steppers and inputs. | Subtitle settings alone are ten rows; playback and cache are below the fold. It is now the densest screen in the app. |
| 5 | `design.md` §3: libraries are pinned first in the sidebar. | The sidebar order is Home, Search, Remote, Settings, then Libraries, then Playlists. | The user’s actual content is below three utilities; the first thing a new user sees is not their library. |
| 6 | `design.md` D4: no local catalogue database in v1. | No local store exists. | Any “history/stats/watchlist” idea must use a small append-only file or the server, not a DB, unless D4 is deliberately reopened. |

These are small fixes with outsized perceived quality. They belong in Phase 0 because they cost
hours and remove contradictions a reviewer will otherwise keep re-discovering.

### 1.2 What the player already gets right

Worth protecting while adding features: one clock (GStreamer), held seek target, chapter ticks,
trickplay preview with chapter name, segment skip, audio track preference applied once, subtitle
appearance set, PGS/VobSub/DVB, online subtitle search, up-next strip with auto-roll, queue from
Play all, stats overlay, buffering chip, idle inhibitor, volume persistence, mini player, MPRIS,
fullscreen, and explicit fallbacks (DMA-BUF → CPU → mpv). Most of the opportunities below are
**quality-of-life on top of a working player**, not a new player.

---

## 2. What the landscape does

The comparison set is deliberately wider than Jellyfin clients. Streaming services are good at
continuity and discovery; media centres are good at library depth; desktop players are good at
control; trackers are good at memory and identity. Runefin is a desktop media centre, so it can
borrow from all four without becoming any of them.

| App | What it is | Pattern worth taking | What not to copy |
|---|---|---|---|
| **Netflix** | Streaming service | Continue Watching as the home screen; autoplay next episode **with a profile toggle**; “Play Something” shuffle; Smart Downloads; “Still watching?”; skip intro; next-episode card; profiles. | Autoplay **previews** on by default; endless personalised rows; artwork personalisation that needs behavioural data Runefin does not have. |
| **Disney+** | Streaming service | Profiles, downloads, “Continue Watching”, strong franchise/collection pages, 4K/HDR badges. | GroupWatch: launched 2020, removed globally on 2023-09-18; co-watching is expensive and was not retained. |
| **Prime Video** | Streaming service | **X-Ray**: in-player cast, scene context, music, trivia and bonus content; “Watch Party” in some regions; rentals. | X-Ray’s editorial trivia is a content team, not a client feature. Take the *shape* (in-player context panel) only. |
| **Max / HBO** | Streaming service | 4K/HDR10/Dolby Vision badges, downloads, profiles, “Continue Watching”, watchlist. | Heavy brand chrome and autoplay trailers. |
| **Apple TV app** | Cross-service hub | **Up Next** as a single queue; SharePlay; Picture in Picture; **Enhance Dialogue**; skipping back turns subtitles on for 10 s; playback speed; profiles and content restrictions; accessibility settings. | Cross-service aggregation (not applicable to one server). |
| **YouTube** | Video platform | Chapters; fine-grained speed (0.05×); **sleep timer**; queue; miniplayer; double-tap seek; “Watch later”; history. | Infinite recommendations and autoplay as the default posture. |
| **Plex** | Media server + clients | Continue Watching; Watchlist; Discover; Skip Intro & Credits; **Play Version**; **Rewind on resume**; **Pass Out Protection**; trailers/cinema intros; collections; downloads; parental controls; UI themes. | **Watch Together** was removed from most apps in Feb 2025 (web kept “for the foreseeable future”); configurable home rows are an admin project. |
| **Emby** | Media server + clients | Configurable home sections; Cinema Intros; **theme songs and theme videos**; parental controls; downloads; intro skipping; extras. | Cinema Intros (trailers before the feature) is a preference, not a default; the config surface is enormous. |
| **Jellyfin Web 12** | Reference web client | **Multiple versions for episodes** (12.0); books/comics (12.0); configurable recommendation source (12.0); plugin-provided search (12.0); Modern layout; **audio/subtitle-language filters and studio search** (12.0); folders marked played; Upcoming view; frame-by-frame (`,`/`.`); chapter names in the scrub bubble; “still watching”. | The web client’s technical debt and layout churn; a browser-first interaction model. |
| **Jellyfin Android TV 0.19** | TV client | Voice search; **“Are you still watching?”** with a configurable timeout/episode count; **media capability report** for playback troubleshooting; VobSub without transcoding; better HDR capability detection. | Voice search needs a platform service; the TV focus model is not desktop. |
| **Findroid / Streamyfin / Swiftfin** | Mobile Jellyfin clients | Downloads and offline playback; long-press menus; gesture controls; “skip intro” parity; native player fallbacks. | Mobile-only gestures; per-platform player engines; UI that scales with library count. |
| **Infuse** | Apple media player | **Smart Filters**; **Hide Spoilers** (blur descriptions/thumbnails of unwatched episodes); **Auto-select first unwatched**; Trakt sync; gesture controls; 8 zoom options; deinterlacing; AI upscaling; black out other monitors; short video specs on the pre-playback screen; continuous playback. | AI upscaling and Dolby Vision processing are platform/vendor features; “black out other monitors” is a macOS-specific nicety. |
| **Kodi 22 “Piers”** | Media centre | **Video Versions presentation**; HDR overlays and UHD PGS; deep links; PVR; gamepad; library widgets. | Add-on ecosystem and skinning; the settings tree is a power-user tool. |
| **Stremio** | Add-on streaming hub | Reorderable catalogue rows; addons supply catalogues/metadata/subtitles; “Continue Watching” as a catalogue. | Addon supply chain and piracy-adjacent catalogues; no local library model. |
| **VLC / mpv / IINA** | Desktop players | A–B loop; frame step; screenshot; audio/subtitle delay; zoom/aspect; deinterlace; always-on-top/PiP; precise keyboard control. | Bare-player UI with no library context; configuration by text file. |
| **Spotify** | Music service | Queue as a first-class object; “continue listening”; podcast sleep timer; crossfade; offline downloads; “liked songs”; year-in-review stats. | Music-specific audio pipeline and social features. |
| **Letterboxd** | Film diary / social | Diary entries, ratings, reviews, lists, watchlist, year-in-review stats. | Public social graph, reviews, and a separate identity; Runefin should keep memory local. |
| **Trakt** | Watch tracking | Scrobbling plays, ratings, watchlist and history across apps; one-way from a media server, two-way only for local libraries. | OAuth/account dependency; never make it required. |
| **TheIntroDB / Intro Skipper** | Community segment data | Intro/recap/credits timestamps from a community DB; no-auth reads; works when the server has no segments. | Third-party availability and privacy; make it opt-in and cache it. |
| **SyncPlay / Watch Together** | Co-watching | Server groups; play/pause/seek sync; buffer-aware pause. | Needs a WebSocket client; Plex and Disney both retreated from the feature; low retention for the cost. |
| **Moonfin** | Linux/Flutter Jellyfin client | Measured design language: near-black palette, floating top pill, rails that expand over the library, art-first cards, dense type, short motion, player-first. | Style marketplace, full-height featured bar at 1280×800, auto-playing trailers, top navigation. |

### 2.1 What the survey says in one page

1. **The player is the product, but the *gaps between plays* are where clients lose people.**
   Resume accuracy, next-episode flow, “still watching?”, sleep timer and skip intro are cheap and
   disproportionately visible.
2. **Continuity is the core loop.** Continue Watching is the home screen; the best clients make
   resume a one-click action with a truthful position, a rewind option and a clear next step.
3. **Discovery must be honest.** Runefin has no behavioural data and should not pretend to have
   it. “Because you watched X” from a real seed, genre/decade/studio rows, “Surprise me” and
   server Instant Mix are honest; “Recommended for you” with no model is not.
4. **Collectors need depth, not more rows.** Versions/editions, extras, theme media, filters by
   resolution/HDR/audio/subtitle language, and collection management are what make a local library
   feel owned.
5. **Desktop is the differentiator.** Tray, notifications, deep links, always-on-top/compact mode,
   MPRIS push, gamepad/Steam Deck and window memory are things a web client cannot do well.
6. **Accessibility is a feature.** Captions, audio description, dialogue boost, reduced motion,
   text scaling, focus visibility and high contrast are not a compliance pass; they change who can
   use the app.
7. **Co-watching is a trap for this product.** It needs a WebSocket dependency, a group state
   machine and a second device to test; Plex removed it from most apps and Disney removed GroupWatch.
   Defer it explicitly.
8. **Do not copy the dark patterns.** Autoplay previews with sound, configurable rows in v1,
   infinite feeds, fake personalisation and “are you still watching?” used as an engagement trick
   all cost more than they return.

### 2.2 Failure modes worth designing around

The survey is not only a list of features to steal; the repeated failures are just as useful.

- **Jellyfin Web’s home rows loading one by one** after 10.11 (reported by users; see
  `01-jellyfin-clients.md`) is what streaming Next Up/Collections was designed to avoid. Keep the
  first paint independent of slow rails; never let a secondary query gate the screen.
- **Plex Watch Together and Disney GroupWatch were removed.** A feature that needs a second
  device, a WebSocket and a group state machine is expensive to keep alive; if it cannot be
  maintained, do not ship it.
- **Swiftfin loses sessions when platform storage is wiped** (research `01`). A desktop config file
  is simpler, but the recovery path still needs a test: reinstall, missing file, unreadable file,
  expired token.
- **Streamyfin’s UI reportedly “crumbled” at the 17th library** (research `01`). Runefin already
  takes up to 8 libraries in the sidebar; make the list scroll/virtualise, allow pinning, and never
  fan out a request per library on Home without a cap.
- **Jellyfin Desktop’s rendering tax** is the reason this project exists. Every “just embed a web
  view for X” idea reintroduces it.
- **Infuse hides multiple versions behind a long-press** and users could not find it (Firecore
  community). Make the version choice visible when it exists; a chevron on Play is cheaper than a
  support thread.
- **Kodi’s settings tree** is a power-user tool; Runefin’s one-column settings is already becoming
  the same problem. Split it before it grows.
- **Stremio’s addon supply chain** shows the cost of extensibility without curation. Runefin does
  not need a plugin marketplace; keep the surface closed and the data server-owned.
- **Netflix’s autoplay previews** are a profile toggle because they annoy people. If Runefin ever
  previews on hover, it is muted, opt-in and delayed.

---

## 3. Opportunity catalogue

The catalogue is grouped by the part of the product it changes. The table is the index; the
subsections give the UX and the API for the items worth designing first.

**Legend.** Value is viewer value, not engineering novelty. Effort: S/M/L/XL as above. Phase:
0 = truth/polish, 1 = viewer QoL, 2 = collector/discovery, 3 = platform/scale, 4 = later/never.

### 3.1 Playback and the player

| ID | Opportunity | Why it matters | Effort | Phase |
|---|---|---|---|---|
| P1 | Segment-aware timeline + auto-skip intro/recap/credits | The skip button exists; the timeline does not show where the segments are, and skipping is manual. | S | 1 |
| P2 | “Still watching?” + sleep timer | Prevents a whole series being marked watched overnight; the most requested TV-client QoL feature. | S | 1 |
| P3 | Resume continuity: rewind, local checkpoint, transfer, conflict prompt | Resume is the core loop; a crash or a second device should not lose the thread. **Absorbs C1–C3.** | S–M | 1 |
| P4 | PlaybackInfo + quality/transcode menu | Direct play first; transcode only for a chosen bitrate or an unsupported codec/container/audio/subtitle. The server supports transcoding, so this is no longer blocked. | L | 2 |
| P5 | Versions/editions + version selector + pre-playback summary | The client currently picks one source silently; a collector needs to see and choose. **Absorbs X3 and P13.** | S–M | 1 |
| P6 | Extras (special features) + theme media | The library already has behind-the-scenes, deleted scenes, theme songs and theme videos. **Absorbs X1 and X2.** | M | 1–2 |
| P7 | Audio intelligence: dialogue boost, night mode, downmix, pitch-corrected speed, audio delay | Speed and subtitles are strong; audio accessibility and late-night listening are not. Split the DSP parts from the cheap routing parts. | M | 1–2 |
| P8 | Frame step, A–B loop, screenshot, go-to-time | Power-user controls that make the player feel complete; screenshot needs a frame path, not a window grab. | M | 2 |
| P9 | Diagnostics: player capability report + app diagnostics | When a file fails the user needs a reason; latency, cache, GStreamer and last errors belong in the same panel. **Absorbs Q6.** | M | 1 |
| P10 | Compact/always-on-top mode, true PiP investigation | Desktop multitasking; Wayland has no standard PiP protocol, so a small always-on-top window is the honest path. | M–L | 3 |
| P11 | End-of-episode polish: shrink to corner, “Watch credits”, next-episode card | The last 30 s decide whether the next episode starts; the current strip is functional but not cinematic. | M | 2 |
| P12 | Queue 2.0: play next / add to queue / reorder / save as playlist | A queue exists only as the result of Play all; users expect an ephemeral queue. | M | 1 |

### 3.2 Continuity and session

| ID | Opportunity | Why it matters | Effort | Phase |
|---|---|---|---|---|
| C4 | Watch history / diary (local, append-only) | Letterboxd-style memory without a database; useful for stats and “what did I watch?”. | M | 2 |
| C5 | Year-in-review / stats | A local, private summary of time watched, top genres/actors/directors, completion rate. | M | 2 |

### 3.3 Home and discovery

| ID | Opportunity | Why it matters | Effort | Phase |
|---|---|---|---|---|
| H1 | Fix the Home contract: label, order, six-rail cap, Recently Played — **shipped 2026-11-05** | The first rail is mislabelled and out of order; a documented rail is missing. **Absorbs H10.** | S | 0 |
| H2 | “Because you watched X” / genre / decade / studio rows | Honest local discovery from a real seed; no fake ML. | M | 2 |
| H4 | Upcoming / calendar (`/Shows/Upcoming`) | “When is the next episode?” is a real question for a weekly series. | S–M | 2 |
| H5 | Watchlist / My List using Jellyfin Favorites or a user playlist | A cross-client list of intent; Jellyfin has no native watchlist in 12.4, so use an existing server object and verify sharing. **Absorbs R2.** | M | 1 |
| H6 | Collections depth: progress, “Continue collection”, create/add | Collections exist but are mostly a browse target; make them a playback unit. | M | 2 |
| H7 | Hero billboard (optional, compact) | A cinematic entry point; must not repeat the 1280×800 full-height mistake. | M | 3 |
| H8 | “New” / “In collection” badges | Decision support on the card; `DateCreated` and box-set membership are available. | S | 1 |
| H9 | “Why this?” + “Not interested” | Transparency and control for recommendation rows; local hide list. | M | 2 |

### 3.4 Library and organisation

| ID | Opportunity | Why it matters | Effort | Phase |
|---|---|---|---|---|
| L1 | Full filter sheet (genre, year, rating, studio, tag, audio/subtitle language, 4K/HDR, missing subs, series status) | The server exposes far more filters than the UI; this is the single biggest browse upgrade. | M | 1 |
| L2 | Saved views / smart lists | “Unwatched 4K horror under 2 h” should be a place, not a ritual. | M–L | 2 |
| L3 | Multi-select and bulk actions | Mark a season watched, add ten films to a playlist, remove a row from Continue Watching. | M | 1 |
| L4 | More sort options (date added, release date, runtime, last played, play count, random) | The server supports them; the UI offers Name/Added/Rating. | S | 1 |
| L5 | Richer list view (year, runtime, rating, date added, play count, size, badges) | The list view is currently a poster row; it should be a data view. | M | 2 |
| L6 | Type-ahead + section headers + “back to top” | The letter rail is good; typing a title should jump there. | S–M | 1 |
| L7 | Card badges (4K, HDR/DV, subtitles, audio language, versions, extras) | Users choose by capability, not only by poster; needs stream fields on visible cards. | M | 1 |
| L8 | Optional hover preview (muted, opt-in) | A premium touch, but only if it never surprises or makes sound. | M–L | 3 |
| L9 | Collection management (create, add/remove, cover, sort) | `/Collections` is fully supported; the client only browses. Collections are server-wide, so label them as shared. | M | 2 |
| L10 | Playlist 2.0 (drag reorder, multi-select, save queue, cover) | Playlists are a first-class server object; the UI is functional but slow. | M | 2 |
| L11 | Library summary header (count, unwatched, runtime, last scan) | Context before scrolling; verify the last-scan field before promising it. | S | 2 |
| L12 | “Needs attention” view (missing, unaired, no subtitles) | A collector’s maintenance view; the API fields exist. | S | 2 |

### 3.5 Detail, person, metadata

| ID | Opportunity | Why it matters | Effort | Phase |
|---|---|---|---|---|
| X4 | Richer metadata: release date, status, studios, tags, original title, taglines, external links | Cheap fields, high perceived depth. | S–M | 1 |
| X5 | Episode depth: air date, “days until”, guest stars, previous/next, mark season watched, play from here | Series viewers live here; the current rows are still a bit thin. | M | 1–2 |
| X6 | Crew depth and person pages (known for, birth, filmography by year) | Cast is good; crew and person context are shallow. | M | 2 |
| X7 | Spoiler guard (blur unwatched episode art/overview, hide next-episode title) | Infuse’s “Hide Spoilers” is a beloved, cheap feature. | S | 1 |
| X8 | Action bar polish (split Play/version, watchlist, likes, overflow) | Keep one primary action but expose the depth without a row of equal buttons. | S | 1 |
| X9 | “Part of collection” and “appears in” links | `GET /Items/{id}/Collections` exists; the detail page only shows one collection. | S | 1 |
| X10 | External links (IMDb/TMDb/TVDB) and “copy link” | The IMDb link exists; the other ids are already in `ProviderIds`. | S | 1 |

### 3.6 Search, command and shell

| ID | Opportunity | Why it matters | Effort | Phase |
|---|---|---|---|---|
| S1 | Command palette (`Ctrl+K`) for items, people, genres, studios and commands | The keyboard-first differentiator; search currently only searches media. Decide whether it replaces or augments the current search. | M | 1 |
| S2 | Search hints (`/Search/Hints`) for people/genres/studios | Typing “Nolan” should suggest the person, not only titles. | S–M | 1 |
| S3 | Search chips / query syntax (`genre:`, `year:`, `person:`, `is:unwatched`, `is:4k`) | Power without a form; discoverable through hints. | M | 2 |
| S5 | Result actions (go to series, trailer) | Search cards already play and open the item menu; add the remaining contextual actions. | S | 1 |
| S6 | Sidebar IA: libraries first, Browse/Tools groups, pinned libraries, counts | The content should be above the utilities. | M | 0–1 |
| S7 | Breadcrumbs, back/forward, mouse back buttons, window title | Desktop convention; the navigation stack is already there. | M | 1 |
| S8 | Persistent now-playing bar | The mini player is good; make it global and keyboard-reachable. | S–M | 1 |
| S9 | Undo toasts | Favourite/watched/remove-from-CW/playlist changes are irreversible today. | S | 0 |
| S10 | Settings IA: sections, search, reset, export | The settings page is now the densest screen. | M | 1 |
| S11 | Connect/onboarding: saved servers, diagnostics, certificate warning | First run is the highest-stakes screen; it is currently one card. | M | 2 |
| S12 | Deep links (`runefin://item/…`) + single instance | Open a title from a browser/chat; Kodi 22 and Android added deep links. | M | 3 |

### 3.7 Desktop integration

| ID | Opportunity | Why it matters | Effort | Phase |
|---|---|---|---|---|
| T1 | Tray icon + now-playing menu | Close-to-tray, play/pause, next, stop, show; Rune already has tray services. | S–M | 2 |
| T2 | Desktop notifications (next episode, download, error) | Useful if opt-in and quiet; zbus can send them through the existing session bus. | M | 2 |
| T3 | MPRIS `PropertiesChanged` push + shuffle/loop | The current MPRIS surface polls; desktop widgets expect push. Artwork is already published. | S–M | 2 |
| T5 | Gamepad / Steam Deck mapping | The design targets a Steam Machine; the input model is keyboard/mouse. | M–L | 3 |
| T6 | Autostart, start minimised, window placement on Wayland | Desktop citizenship; placement is compositor-dependent. | S | 3 |

### 3.8 Accessibility, i18n, theming

| ID | Opportunity | Why it matters | Effort | Phase |
|---|---|---|---|---|
| A1 | Reduced-motion setting | WCAG 2.3.3; springs, ambient motion and parallax should be switchable. | S | 1 |
| A2 | High-contrast theme + text scaling | WCAG contrast and low-vision use; the current dark-only palette is tuned for one setting. | M | 2 |
| A3 | Light theme + accent choices | D7 deferred it; it is a token table and a contrast audit, not a redesign. | S–M | 2 |
| A4 | Audio description / SDH detection and badges | Tracks exist but are not surfaced; WCAG 1.2.3. | S–M | 1 |
| A6 | Keyboard shortcut customisation + non-Latin layouts | Jellyfin 12 fixed non-Latin shortcuts; power users expect remapping. | M | 2 |
| A7 | i18n + RTL | The UI is English-only; a string table and layout mirroring are the real work. **Lowest priority; keep for after the product stabilises.** | L | 4 |
| A8 | Colour-blind-safe badges (icon + text, not colour alone) | Status must survive deuteranopia and a bad panel. | S | 1 |
| A9 | Focus visibility audit (contrast, 2 px, no scale-only) | The focus ring exists; prove it on every surface and at 200% zoom. | S | 1 |

### 3.9 Live TV

| ID | Opportunity | Why it matters | Effort | Phase |
|---|---|---|---|---|
| V1 | Full-day guide, “now” line, jump to now, program detail popover | The 3-hour slice is a start; a guide that cannot scroll the day is a teaser. | M | 2 |
| V2 | Channel favourites/hide, groups, logos, search | Four channels is fine; forty is not. | M | 2 |
| V3 | “On now” Home rail + last channel | Live TV should be reachable without opening the guide. | S | 1 |
| V4 | Program reminders (local notification) | No server timer needed; “watch at 21:00” is a client job. | S–M | 2 |
| V5 | Timeshift/pause live investigation | Server/tuner dependent; test before designing. | M | 3 |

### 3.10 Personal data and social

| ID | Opportunity | Why it matters | Effort | Phase |
|---|---|---|---|---|
| R1 | Likes/dislikes (`/UserItems/{id}/Rating`) | A second signal besides favourite; filters already support `Likes`/`Dislikes`. | S | 1 |
| R3 | Trakt scrobbling (optional) | Cross-client memory for people who already use Trakt; Infuse and the Jellyfin plugin prove the demand. | M | 2 |
| R4 | Letterboxd export (CSV) | Low effort, high delight for film people; no write API needed. | S | 2 |
| R5 | Local reviews/notes | A private diary; no server field, so store locally and export. | M | 3 |
| R7 | Sharing a title (copy server URL / deep link) | “Look at this” is a real desktop action; no social graph. | S | 2 |

### 3.11 Reliability, performance, storage

| ID | Opportunity | Why it matters | Effort | Phase |
|---|---|---|---|---|
| Q1 | Stale-while-revalidate on Home/Library/Detail | Revisits should paint instantly and refresh quietly; skeletons are for cold starts. | M | 1 |
| Q2 | Prefetch the next episode | Reduce the gap at EOS; define whether it is metadata/art or a video buffer. | M–L | 3 |
| Q3 | Cache limits + LRU + “clear cache” per kind | The cache is already keyed and bounded in memory; disk needs a budget. | S–M | 1 |
| Q5 | Crash-safe navigation + “resume where you left off” | Reopen the last route/scroll/focus after a restart. | S–M | 2 |
| Q7 | Retry/backoff and user-visible “server busy” | The live box has a WAF that bans on 403; do not hammer. | S | 1 |
| Q8 | Performance tests for new dense screens | The 20k grid test is a good start; filters/multi-select must stay under 16 ms. | S | 1 |
| Q9 | Downloads + offline playback + cached browse | Store files under a Settings-chosen path; download queue, pause/cancel, storage cap, play downloaded files, and browse downloaded pages offline. **Absorbs Q4.** | XL | 3 |

### 3.12 Deliberate non-goals (for now)

| ID | Non-goal | Why not now |
|---|---|---|
| N1 | Music library / player | No music libraries on the server; a music player is a different product (queue, gapless, ReplayGain, lyrics). |
| N2 | Books/comics reader | Jellyfin 12.0 makes books real, but a reader is a separate surface (pagination, EPUB/PDF, progress sync). Revisit only if a book library appears. |
| N3 | Photos | A photo viewer/slideshow is a different interaction model. |
| N4 | Multi-server aggregation | Product is one server, one window; multi-server breaks identity, search and caches. |
| N5 | Web view / embedded browser | Explicitly excluded; it would reintroduce the web client's rendering tax. |
| N6 | SyncPlay / Watch Together | WebSocket dependency, group state machine, second device to test; low retention. |
| N7 | Fake “AI recommendations” | No behavioural data; dishonest labels destroy trust. Use honest heuristics. |
| N8 | Social feed / comments / reviews network | No server data, moderation burden, and it is not the product. |
| N9 | Ads / upsell / rental store | Not a storefront. |
| N10 | HDR passthrough / Dolby Vision (today) | Blocked on wgpu/Wayland colour management and libplacebo; tone-mapped SDR is the honest deliverable. |
| N11 | Mobile/Windows/macOS ports | Out of scope; one Linux desktop target, two AMD machines. |
| N12 | Infinite feed / short-form | Wrong interaction model and attention posture. |

---

## 4. The opportunities worth designing first

Each entry gives the UX, the API, the acceptance bar and the cost of getting it wrong. The order is
roughly the order I would build them, not the order of the table. The briefs are numbered **D1–D17**
for navigation only; the catalogue IDs in §3 (`P1`, `L1`, `S1`, …) are the canonical index and are
what the roadmap and the screen audit refer to.

### D1. Segment-aware timeline and auto-skip (P1)

**UX.** Draw `Intro`, `Recap`, `Outro` and `Preview` as translucent bands on the seek bar, with a
label on hover (`Intro · 0:42–1:28`). The existing skip button stays; add a setting
`Skip intro: Ask / Always / Never` (same for credits and recap). On `Always`, seek at the segment
start and show a small toast with “Undo” for 8 s. On `Ask`, the button appears 5 s before the
segment. Clicking a band seeks to its start.

**Why.** The server already provides `GET /MediaSegments/{itemId}` and the app already consumes it;
the player currently exposes only a button. Jellyfin 12 puts chapter names in the scrub bubble and
Plex sells “Skip Intro & Credits”; TheIntroDB exists because server segments are often missing.
Bands make the feature legible and give the user control.

**API.** `GET /MediaSegments/{itemId}` (`MediaSegmentType`: `Intro`, `Recap`, `Outro`, `Preview`,
`Commercial`). Optional fallback: TheIntroDB `GET /segments` by IMDb id, opt-in, cached under
`~/.cache/runefin/segments/`; do not make a third-party call on every play.

**Acceptance.** With a fixture containing all four segment types, the bar draws four bands; `S`
seeks to the active segment end; `Always` skips without a click; `Undo` returns to the pre-skip
position; no segment data means no bands and no button. Tests: `rt.find("in-app-seek-segment-intro")`,
`rt.state.in_app_position`, fake clock. Snapshots: `in-app-segments.png`, `in-app-segments-off.png`.

**Risk.** Segment times can be wrong; never auto-skip without an undo. TheIntroDB is a network
dependency with privacy implications; keep it opt-in and cached.

### D2. “Still watching?” and sleep timer (P2)

**UX.** After N episodes (default 3) or M minutes of wall-clock playback (default 90) with no
pointer/key input, pause at the end of the current episode and show a full-screen-but-quiet card:
*“Still watching?”* with **Continue** and **Stop**. A 60 s countdown stops and reports. Separately,
a **Sleep timer** in the player menu: 10/20/30/45/60 min or “End of episode”; the OSD shows a small
moon + remaining time; volume fades over the last 20 s; cancel restores.

**Why.** Jellyfin Android TV 0.19 shipped it “by popular request” to stop entire series being marked
watched overnight; Jellyfin 12 added it to web; Plex calls its variant “Pass Out Protection”.
YouTube and Netflix both ship a sleep timer. This is a small feature with a large perceived
quality return.

**API.** None. Use the existing input clock and `poll`; persist the preference in `config.toml`.
Do not use a wall-clock thread; the app already has a frame clock.

**Acceptance.** A scripted 3-episode run with no input pauses on the “Still watching?” card; Continue
resumes the queue; Stop sends a `Stopped` report at the current position; the sleep timer pauses at
the requested time and does not fire after cancel. Tests with a fake clock. Snapshots:
`in-app-still-watching.png`, `in-app-sleep-timer.png`.

**Risk.** Do not turn it into an engagement nag; default it on for autoplay runs only, and let the
user disable it.

### D3. Resume continuity: rewind, checkpoint, transfer (P3)

**UX.** When resuming, start **10 s earlier** (setting 0/5/10/15 s) and show “Rewound 10 s” in the
OSD for a moment. The detail button still says `Resume 42:13`, so the label remains truthful.
Persist a local checkpoint every 5 s under `~/.cache/runefin/positions.json`; on a crash/restart,
offer `Resume from this device (42:13)` vs `Resume from server (41:58)` if they differ by more than
a few seconds. Add **Continue on…** in the player overflow: list the user’s other sessions with
their current position; send `POST /Sessions/{id}/Playing?playCommand=PlayNow&startPositionTicks=…`
and stop locally only after the server answers.

**Why.** Plex ships “Rewind on resume”; Apple TV turns subtitles on when you skip back; a 10 s
rewind is the difference between “where was I?” and “oh, right”. The transfer call already exists
in the 12.4 spec with `startPositionTicks`, but `play_on_session` sends only item ids.

**API.** `POST /Sessions/{sessionId}/Playing` with `playCommand=PlayNow`, `itemIds`,
`startPositionTicks`, optional `mediaSourceId`, `audioStreamIndex`, `subtitleStreamIndex`. Local
checkpoint is a small JSON map keyed by item id, capped (e.g. 500 entries).

**Acceptance.** A resumed item starts 10 s before the server position; the checkpoint survives a
simulated kill; a transfer sends the position and stops the source only on a 2xx; a newer server
position wins by default but the conflict prompt can override. Tests: `play_on_session_path` with
position, fake clock for checkpoint. Snapshots: `detail-resume-rewind.png`, `in-app-transfer.png`.

**Risk.** Starting earlier can replay a spoiler or a recap; make the setting visible and default to
5–10 s, not 30.

### D4. PlaybackInfo and a quality/transcode menu (P4)

**UX.** On the pre-playback sheet and in the player's gear menu: **Auto**, **Original**, **20 / 12 /
8 / 4 / 2 Mbps**, and resolution caps (**1080p**, **720p**, **480p**). Show the decision in plain
words: `Direct play · 4K HDR10 · 31 Mbps`, `Direct stream · audio only`, or `Transcode · 1080p ·
8 Mbps`. Add **Why?** which lists the reasons (container, video codec, audio codec, bitrate,
subtitle burn-in, device profile) and the server's chosen profile. Remember the last choice per
network (LAN vs remote) and per item.

**Why.** Runefin only requests `Videos/{id}/stream?static=true`. That is excellent on a LAN with
capable hardware and wrong the moment the user is on Wi-Fi, a hotel, or a device that cannot decode
the file. Plex, Emby, Infuse and every Jellyfin client negotiate with a device profile; Jellyfin's
own docs say the client sends the profile and the server picks the best settings. This is the
single largest capability gap.

**API.** `POST /Items/{itemId}/PlaybackInfo` with `UserId`, `MaxStreamingBitrate`, `StartTimeTicks`,
`AudioStreamIndex`, `SubtitleStreamIndex`, `MaxAudioChannels`, `MediaSourceId`, `DeviceProfile`
(`DirectPlayProfiles`, `TranscodingProfiles`, `CodecProfiles`, `ContainerProfiles`,
`SubtitleProfiles`), `EnableDirectPlay`, `EnableDirectStream`, `EnableTranscoding`,
`AllowVideoStreamCopy`, `AllowAudioStreamCopy`. Response: `MediaSources[]` with `TranscodingUrl`,
`DirectStreamUrl`, `PlaySessionId`, `SupportsDirectPlay`, `SupportsDirectStream`,
`SupportsTranscoding`. Play the HLS/progressive URL with GStreamer (`hlsdemux`/`tsdemux` from
`gst-plugins-bad`; a system dependency, not a Cargo dependency). Report `PlayMethod` and the same
`PlaySessionId` on start/progress/stop.

**Acceptance.** Direct play is the default and reports `DirectPlay`; forcing 2 Mbps returns a
`TranscodingUrl`, plays it, and reports `Transcode` with the session id; changing quality mid-play
re-negotiates and resumes at the current position; the “Why?” panel names the actual reason. The
app never transcodes silently: the menu says `Direct play` unless the reader picked a lower bitrate
or the source cannot be reproduced (codec/container/audio/subtitle). Tests: mocked `PlaybackInfo`
response at the `Client` boundary; live test on the real server (the official client transcodes
here). Snapshots: `in-app-quality.png`, `detail-preplay.png`.

**Risk.** This is a framework-sized feature: HLS demuxing, buffering, seek in a transcode,
subtitle burn-in, audio-only transcode, and a device profile that matches the actual GStreamer
capabilities. The server supports transcoding, but verify the profile and startup latency live;
keep direct play the default so a capable client never pays the transcode tax.

### D5. Versions, version selector and pre-playback summary (P5)

**UX.** If `MediaSources.len() > 1` (or `MediaSourceCount > 1`), the Play button grows a chevron
(`Play · 4K`) and the detail page shows a **Version** row: `4K HDR10 · 31 Mbps`, `1080p · 8 Mbps`,
`Director's cut · 1080p`. Long-press/right-click on Play opens the same picker. Remember the last
version per item. For episodes, the same picker applies when 12.0+ groups episode versions. The
pre-playback sheet summarises: version, container, video codec/resolution/range, audio
codec/channels/language, subtitle tracks, size, and `Direct play` vs `Transcode`.

**Why.** Jellyfin has grouped multiple versions for years and 12.0 extends it to episodes; Plex has
“Play Version”, Infuse has a long-press picker, and Findroid still has an open issue asking for it.
A collector with a 4K remux and a 1080p copy currently gets whichever the server returns first.

**API.** `Item.media_sources[]` already exists; `MediaSourceCount` is an `ItemFields` value;
`PlaybackInfo` accepts `MediaSourceId`; `Videos/{id}/stream?static=true&mediaSourceId=…` selects the
source. Add `MediaSourceCount`, `Name`, `Container`, `Bitrate`, `Size`, `RunTimeTicks` and stream
details to the wire type.

**Acceptance.** A fixture with two sources shows the version row; choosing 1080p sends the right
`mediaSourceId` and the stats overlay says 1080p; the choice is remembered; a single-source item
shows no chevron. Tests at the `Client` boundary and `rt.find("detail-version-…")`. Snapshots:
`detail-versions.png`, `in-app-preplay.png`.

**Risk.** Version labels come from filenames; do not invent “Director's cut” if the server did not
say it. Fall back to resolution/bitrate.

### D6. Extras and theme media (P6)

**UX.** Add an **Extras** rail on detail, grouped by `ExtraType`: Behind the scenes, Deleted scenes,
Interviews, Featurettes, Clips, Shorts, Other. Each card shows a thumbnail, name and runtime; play
in the in-app player with reporting off (like trailers). Add **Theme** support: a theme song plays
softly on the detail page (with a small speaker toggle), or a theme video loops muted behind the
hero; both stop when playback starts or the user navigates. Settings: `Theme songs: Off / On`,
`Theme videos: Off / On`, `Mute theme videos: always`.

**Why.** Jellyfin documents extras folders and theme music/videos; Emby and Plex surface both. The
current detail page has trailers and similar titles but ignores the extras and theme files the
server already knows about. This is high perceived value for a local library and uses existing
media.

**API.** `GET /Items/{itemId}/SpecialFeatures` returns `BaseItemDto[]` with `ExtraType`.
`GET /Items/{itemId}/ThemeMedia?inheritFromParent=true` returns `AllThemeMediaResult` with
`ThemeVideosResult`, `ThemeSongsResult`, `SoundtrackSongsResult`. Play the theme song through a
small GStreamer `playbin` (audio only); theme video through the existing player sink, muted, with a
short loop and no reporting.

**Acceptance.** A movie with three extras shows three labelled cards; playing one does not enter
Continue Watching; a series with a theme song plays it on detail and stops on navigation; a theme
video does not play when reduced motion is on. Tests: mocked `SpecialFeatures`/`ThemeMedia` at the
`Client` boundary; `rt.find("detail-extra-…")`. Snapshots: `detail-extras.png`,
`detail-theme.png`.

**Risk.** Theme audio can be annoying; default songs off, or on only after the user enables it.
Theme video adds a second video pipeline; keep it optional and reuse the sink rather than adding a
render stack.

### D7. Watchlist, likes/dislikes, and undo (R1, H5, S9)

**UX.** Add a **Watchlist** (My List) toggle on cards and detail, a sidebar entry and a home rail.
Use an existing Jellyfin capability: a playlist named `Watchlist` (create on first use, reuse
thereafter) or Favorites, whichever the server and the reader's sharing model support. Do not call
it private: it is a server object, and other clients/users may see it according to Jellyfin's
sharing rules. Add thumbs up/down on detail and in the item menu (`Likes`/`Dislikes`); use them in
filters and “because you liked” seeds. Every watch-state/favourite/playlist change gets an
**Undo** toast for 8 s; errors persist until dismissed.

**Why.** Plex and Netflix make “My List” a core primitive; Jellyfin 12.4 has no native watchlist
(the spec has no watchlist path), but favorites and playlists are available. `POST
/UserItems/{itemId}/Rating?likes=true|false` and `DELETE` are in the spec, and `ItemFilter` already
supports `Likes`/`Dislikes`. Undo is a cheap trust win; NN/g warns that toasts are bad for errors
but good for passive confirmation with an action.

**API.** `POST /Playlists` (`name`, `ids`), `POST /Playlists/{id}/Items`, `DELETE
/Playlists/{id}/Items`, `GET /Playlists/{id}/Items`; `POST/DELETE /UserItems/{itemId}/Rating`;
`POST/DELETE /UserFavoriteItems/{itemId}`. Cache the watchlist playlist id in config; if the user
deletes it server-side, recreate on next toggle.

**Acceptance.** Adding a title shows it in the rail and sidebar immediately; removing it from
another client is reflected on the next load; likes filter works; undo restores the previous state
and the server is told in the background. Tests: `rt.find("detail-watchlist")`, mocked playlist
responses. Snapshots: `home-watchlist.png`, `toast-undo.png`.

**Risk.** A playlist named “Watchlist” is a convention, not a schema; document it and verify its
visibility/permissions on the live server. Do not create a new playlist every time the user
toggles.

### D8. Filter sheet, saved views, multi-select (L1–L3)

**UX.** Replace the current All/Unwatched/Favourites + Genre menu with a **Filters** button that
opens a sheet of sections:

- **Watch state**: Unwatched, In progress, Watched, Favourites, Likes, Dislikes.
- **Video**: 4K, HD, HDR/Dolby Vision, resolution, video type.
- **Audio/subtitles**: audio language, subtitle language, has subtitles, forced only.
- **Metadata**: genre, year/decade, official rating, studio, tag, person.
- **Series**: Continuing/Ended, unaired, missing.

Show a count on the button and removable chips above the grid; “Clear all”; apply instantly with a
short debounce. Add **Save view** (name + filter + sort) to the sidebar/library header. Add
**Select** mode: checkbox on cards, Shift-click range, actions bar (watched, favourite, playlist,
collection, queue, remove from CW).

**Why.** The server exposes `Items/Filters2` (genres, tags, audio/subtitle languages) and
`Items/Filters` (genres, tags, official ratings, years), plus `/Items` parameters `is4K`, `isHd`,
`hasSubtitles`, `videoTypes`, `studios`, `personIds`, `minCommunityRating`, `seriesStatus`,
`isUnaired`, `hasThemeSong`, `hasSpecialFeature`, `filters=Likes/Dislikes/IsResumable/...`.
Jellyfin 12 added audio/subtitle-language filters and studio search; Infuse has Smart Filters; Plex
and Emby have deep filters. Runefin currently offers three facets and a genre menu.

**API.** `GET /Items/Filters2?parentId=&includeItemTypes=`; `GET /Items/Filters?parentId=`;
`GET /Items?…` with the parameters above. Saved views live in `config.toml` (name, library id,
filter set, sort); they are just queries, not server objects. Multi-select actions loop the
existing per-item endpoints, with a small concurrency cap and a progress toast.

**Acceptance.** A 20k-item fixture with filters applied stays under 16 ms per frame; the active
filter count and chips match the query; a saved view reopens with the same results; select-all
marks a season watched and the cards update optimistically. Tests: query string contract in
`jellyfin-api`, `rt.state.library.filters`, `rt.find("library-filter-…")`. Snapshots:
`library-filters.png`, `library-selection.png`, `library-saved-view.png`.

**Risk.** Filter combinations can produce empty results; always show “No items match” with “Clear
filters”. Do not fetch the whole library to filter client-side; keep the server query.

### D9. Command palette and search hints (S1–S5)

**UX.** `Ctrl+K` opens a palette with three zones: **Recent** (items and queries), **Results**
(grouped Movies, Shows, Episodes, People, Genres, Studios, Collections), and **Commands** (`Go to
Settings`, `Toggle fullscreen`, `Play random`, `Mark watched`, `Add to watchlist`, `Open
downloads`, `Switch theme`). Type `>` to search commands only. Arrow keys move; Enter opens; `Ctrl+Enter`
plays; `Tab` opens the action panel for the selected row. Each result shows a poster thumb, title,
year, type and a one-line context (`S2E4 · 42 min · 4K`). Zero results show “Search all libraries”
and suggested people/genres from `/Search/Hints`.

**Why.** The app is already keyboard-first (one focus ring, `?` sheet, `Esc` back). A command
palette is the natural extension and a desktop differentiator; Raycast, Linear and VS Code made it
a standard. Jellyfin 12 makes search plugin-extensible and adds studio search; `/Search/Hints`
returns people, genres and studios, which the current `/Items?searchTerm=` does not.

**API.** `GET /Search/Hints?searchTerm=&includePeople=true&includeGenres=true&includeStudios=true`
(returns `SearchHintResult`); `GET /Items?searchTerm=` for full results; local command registry in
the app. Persist recent queries/items in config, capped.

**Acceptance.** `Ctrl+K` from any screen; typing “Nolan” suggests the person and their films; `>full`
finds “Toggle fullscreen”; Enter opens the highlighted result; `Ctrl+Enter` plays it; Esc closes
and restores focus. Tests: `rt.find("command-palette")`, `rt.state.route`. Snapshots:
`command-palette.png`, `command-palette-commands.png`, `search-hints.png`.

**Risk.** Do not make the palette the only way to reach a command; every command also lives in the
UI. Do not let it become a second search with different ranking rules; reuse the same query.

### D10. Home contract and honest discovery (H1, H2)

**UX.** Fix the contract first: **Continue Watching** (renamed), then **Next Up**, then **Recently
Added** (up to four at the time), then **Recently Played**, then **Collections**; cap at six (both
changed on 2026-11-05: every library gets a rail and there is no cap); suppress empty
rails. Add “See all” to Continue Watching and Next Up (a filtered grid). On a Continue Watching
card, show the series name, `S2E4`, the remaining time and a progress bar; on hover/focus, a
**Remove** action and **Details**. Add **Because you watched X** only when there is a real seed:
take the last played item, pick its top genre/person/studio, return 12 unwatched items sorted by
rating or random, and label the rail with the seed. Add **Surprise me** (random item or a 10-item
Instant Mix) and **Coming up** (`/Shows/Upcoming`).

**Why.** Continue Watching is the home screen in every surveyed client. Netflix’s “Play Something”,
Plex’s Watchlist/Discover and Emby’s home sections all attack indecision; Runefin’s home is strong
but the first rail is mislabelled and the documented Recently Played rail is missing. Honest
heuristics avoid the fake-personalisation trap.

**API.** `GET /UserItems/Resume` (existing), `GET /Shows/NextUp` (existing), `GET /Items/Latest`
(existing), `GET /Items?filters=IsPlayed&sortBy=DatePlayed&descending=true` for Recently Played,
`GET /Items?genreIds=&personIds=&studios=&sortBy=Random&limit=12` for seeds, `GET
/Items/{id}/InstantMix?limit=10`, `GET /Shows/Upcoming?limit=12`.

**Acceptance.** The rail order and count match the contract in a snapshot; an empty rail is absent;
a resume card shows `S2E4 · 18 min left`; “Because you watched” only appears with a seed; “Surprise
me” starts playback. Tests: `home::merge_resume`, `rt.state.home_data`, `rt.find("rail-…")`.
Snapshots: `home.png`, `home-discovery.png`, `home-upcoming.png`.

**Risk.** “Recently Played” can duplicate Continue Watching; dedupe by series and hide items already
in CW. “Because you watched” can look like a recommendation engine; keep the label literal and add
“Why this?”.

### D11. Detail depth and spoiler guard (X4–X10)

**UX.** Add an **Extras** rail (P6), **Version** row (P5), **Watchlist/Like** actions, **Part of
collection / Appears in**, and a metadata block with release date, series status, studios, tags,
original title, taglines and external links. For series, add air date and “days until next”,
guest stars, previous/next episode, **Mark season watched**, and **Play from here** (queue the rest
of the season). Add a **Hide spoilers** setting: blur the overview and thumbnails of unwatched
episodes and the next-up synopsis until hover/focus/click; optionally hide the next episode title.

**Why.** Infuse’s Hide Spoilers is a small feature people mention by name; Jellyfin 12 added
collections on item pages, crew merging and TV creators; Plex/Kodi show version and extras depth.
The current detail page is strong on the hero and cast but shallow on the “collector” facts.

**API.** `ItemFields`: `DateCreated`, `PremiereDate`, `Studios`, `Tags`, `Taglines`,
`OriginalTitle`, `ExternalUrls`, `MediaSourceCount`, `SpecialFeatureCount`, `SeriesStudio`,
`ProductionLocations`, `OfficialRating`, `CanDownload`; `GET /Items/{id}/Collections`;
`GET /Items/{id}/SpecialFeatures`; `GET /Shows/{seriesId}/Episodes`; `POST /UserPlayedItems/{itemId}`
for season/series (verify on the live server).

**Acceptance.** A series page shows the next air date and a spoiler-blurred next-up synopsis; a
movie shows studios/tags/external links; “Mark season watched” updates every episode card; “Play
from here” queues the rest. Tests: `rt.find("detail-spoiler-…")`, `rt.state.detail`. Snapshots:
`detail-deep.png`, `detail-spoilers.png`.

**Risk.** More metadata can crowd the hero; keep it behind a disclosure or a two-column lower
section. Blur must not hide the primary action.

### D12. Audio intelligence (P7)

**UX.** In the player's audio menu, add **Dialogue boost** (off/low/high), **Night mode** (dynamic
range compression), **Downmix to stereo**, **Audio delay** (±100 ms), and **Pitch-corrected speed**
(a toggle, default on). Show the active mode in the stats overlay.

**Why.** Apple TV ships **Enhance Dialogue**; Infuse/mpv expose audio delay and downmix; Jellyfin 12
reads ReplayGain metadata. Runefin already has speed and subtitle delay; audio is the missing half
of the accessibility/late-night story. At 1.25×/1.5× a player without time-stretching raises pitch,
which sounds cheap.

**API.** GStreamer: `scaletempo`/`scaletempo2` in the audio path for pitch-corrected speed;
`audiodynamic` (compressor) for night mode/dialogue boost; `audioconvert` + `audioresample` +
capsfilter for stereo downmix; an audio sink `ts-offset` or a small `identity`/`queue` for delay
(verify the exact element). No new Cargo dependency; these are system plugins already shipped with
GStreamer.

**Acceptance.** 1.5× keeps voices natural (by ear and a spectral check); night mode reduces the
dynamic range without clipping; downmix plays 5.1 on stereo; delay shifts audio without dropping
frames; all are persisted. Tests: pipeline construction and property assertions; a local clip test.
Snapshots: `in-app-audio-modes.png`.

**Risk.** “Dialogue boost” is not a single DSP; compression helps but is not Apple’s implementation.
Label it honestly (“Night mode / dialogue boost”) and keep the default off.

### D13. Desktop integration (T1–T3)

**UX.** A tray icon with **Show/Hide**, **Play/Pause**, **Next**, **Stop**, **Quit** and the current
title; close-to-tray as a setting. Desktop notifications for “Next episode airs in 10 min”,
“Download complete”, “Playback failed” — opt-in, quiet hours, one at a time. MPRIS `PropertiesChanged`
push and `Shuffle`/`LoopStatus`; the cached poster is already published as art. `runefin://item/{id}` deep links with a
single-instance D-Bus name; “Copy link” on detail. A **Compact player** mode: a small always-on-top
window with video + minimal controls, not a second full window.

**Why.** These are the things a web client cannot do; the design already lists tray/zbus in the
dependency budget and MPRIS is shipped. Kodi 22 added deep links; Apple TV has PiP. The product is
a desktop app, so desktop citizenship is part of the UX.

**API.** `zbus` for `org.freedesktop.Notifications` and the single-instance name; Rune's tray
service; `Tx::set_fullscreen`-style window API for always-on-top/compact; `Sessions` for the tray
title. No new Cargo dependency if zbus/tray are already in Rune; otherwise this is a design.md
dependency line.

**Acceptance.** Closing the window with close-to-tray leaves playback alive; tray Play/Pause
controls the player; a notification opens the right item; `runefin://item/<id>` focuses the existing
instance; compact mode stays above other windows. Tests: command queue drained on the UI thread;
snapshot `compact-player.png`.

**Risk.** Tray support varies by desktop; always-on-top is compositor-dependent on Wayland. Degrade
to a normal window rather than failing.

### D14. Accessibility, theming and i18n (A1–A4, A6, A7)

**UX.** Settings → **Appearance**: theme (Dark / Light / High contrast / OLED), accent, text scale
(100/125/150%), reduced motion, “reduce transparency”. Settings → **Accessibility**: captions
style (existing), audio description auto-select, SDH preference, focus ring thickness, “show
keyboard hints”. Every status badge gets an icon or label, not colour alone. The shortcut sheet is
generated from the key map and works on non-Latin layouts. Strings move to a table; RTL mirrors
the shell.

**Why.** WCAG 2.2 requires captions (1.2.2), audio description (1.2.3), reduced motion (2.3.3),
focus visible (2.4.7/2.4.11) and contrast (1.4.3/1.4.11); Jellyfin 12 fixed non-Latin shortcuts;
Apple TV exposes dialogue enhancement and content restrictions. Accessibility is also good design
for a laptop in bright light or a TV across the room.

**API.** Token table in `theme.rs`; a string table; `ItemFields`/`MediaStream` for AD/SDH detection
(`DisplayTitle`, `Title`, `Language`); no server change for themes/text scale.

**Acceptance.** Light and high-contrast snapshots pass a contrast audit; 150% text reflows without
clipping; reduced motion removes springs/parallax; enabling Rune's `accessibility` feature exposes
the tree and a screen-reader pass documents what is and is not reachable; captions/AD are
selectable. Snapshots: `home-light.png`, `home-contrast.png`, `settings-accessibility.png`.

**Risk.** RTL mirroring is a layout project, not a token change, and A7 is deliberately the lowest
priority. Rune's accessibility tree is off because this app omits the feature; enabling it is the
first step, then audit, not an unknown blocker.

### D15. Downloads and offline (Q9, Q4)

**UX.** A **Download** action on detail and in the item menu, a Downloads page with queue/progress/
pause/cancel, storage budget, “Download next 3 unwatched episodes”, and an **Offline** section that
plays downloaded files when the server is unreachable. Settings → **Downloads** gets a storage
path (default under `~/.cache/runefin/downloads`), a budget and a “delete after watched” toggle.
Progress reports queue locally and flush on reconnect. Cached Home/Library/Detail pages stay
readable offline; playback of non-downloaded items is disabled with a reason.

**Why.** Netflix Smart Downloads, Plex/Emby/Infuse downloads, and the project's own M7 note. This is
the feature that makes a laptop useful on a plane, and the client already has a direct stream URL
and a cache directory.

**API.** `GET /Items/{itemId}/Download` (binary; `CanDownload` on the item), `POST
/Items/{itemId}/PlaybackInfo` if a specific version is needed. Store under the Settings path (or
`~/.cache/runefin/downloads/`) with a JSON manifest (item id, source id, size, checksum, state);
use HTTP Range for resume if the server supports it. Playback from `file://` through the same
GStreamer pipeline; report on reconnect.

**Acceptance.** A queued download survives restart; pause/resume works; offline mode lists only
complete downloads and plays them; the queue flushes reports when the server returns. Tests: a fake
download worker with a local file; no network in tests. Snapshots: `downloads.png`,
`downloads-offline.png`.

**Risk.** This is a new storage/catalogue design (M7), not a button. It also needs a policy for
disk space and deletion, and a safe path with permissions. Do not start it before P4/P1/P2 because
it is the largest item on the list.

### D16. Live TV depth (V1–V4)

**UX.** A full-day guide with a “now” line, horizontal scroll, “Jump to now”, channel logos,
favourites/hide, groups and search; clicking a program opens a detail popover with description,
season/episode and **Watch** / **Remind me**. An “On now” home rail tunes directly. `PgUp`/`PgDn`
keep working; add number entry and “last channel”. If the tuner supports timeshift, enable
pause/seek in the live OSD; if not, say so.

**Why.** The current guide is a 3-hour slice with rows; Jellyfin 12 improved the Upcoming view and
live TV metadata; TV guides are judged by “can I find what is on later?”. Recording timers remain
blocked by the provider, so reminders are the honest substitute.

**API.** `GET /LiveTv/Channels`, `GET /LiveTv/Programs` (existing), `GET /LiveTv/Guide` with a
wider window; `GET /Items?includeItemTypes=TvChannel` for logos; local reminder store. Timers:
`POST /LiveTv/Timers` (blocked, keep the error).

**Acceptance.** The guide scrolls a full day; “Jump to now” centres the current time; a reminder
fires a notification and opens the channel; a channel without a logo still lays out. Tests:
guide window math, `rt.find("live-guide-…")`. Snapshots: `live-guide-day.png`,
`live-program.png`, `live-reminder.png`.

**Risk.** A wider guide is more data; keep the window query bounded and cache per channel. Do not
fake timeshift.

### D17. Watch history, stats and Trakt (C4, C5, R3)

**UX.** A **History** page (local, private) with date, title, device, watched duration, completion
and a “play again” action; filters by type/genre/year; export CSV. A **Stats** page: total time,
films/episodes, top genres/actors/directors, completion rate, current streak? (avoid gamification),
and a **Year in review** card generated locally. Optional **Trakt**: connect via device OAuth,
scrobble start/pause/stop, push ratings, pull watchlist (optional), with the server as the source of
truth for Jellyfin items.

**Why.** Letterboxd’s diary/stats and Spotify’s year-in-review show that memory is a retention
feature; Jellyfin’s Playback Reporting plugin exists because the server does not expose a clean
per-play history. Infuse and the Jellyfin Trakt plugin prove scrobbling demand. Keeping it local
respects the product’s privacy posture.

**API.** No server history endpoint; append a JSONL record on `Stopped` (item id, series id, start
time, end position, duration, completed, device). Trakt: OAuth device flow, `POST /scrobble/start`,
`/scrobble/pause`, `/scrobble/stop`, `POST /sync/ratings`, `GET /sync/watchlist` (Trakt API;
verify current terms). Store the token 0600 like the Jellyfin token.

**Acceptance.** A completed play appears in History once (not twice on pause/stop); stats match the
log; CSV exports; Trakt is off by default and a failed scrobble never blocks playback. Tests: log
append/dedupe with a fake clock; mocked Trakt at the `Client` boundary. Snapshots: `history.png`,
`stats.png`, `settings-trakt.png`.

**Risk.** A local history is a new store (JSONL, not a DB) and a privacy surface; document exactly
what is stored and add “Clear history”. Trakt adds an account/OAuth dependency; keep it optional.

## 5. Screen-by-screen UX audit

This is the “how the UX could be improved” half. It is deliberately concrete: what is on screen
today, what leaks, and the smallest fix.

### 5.1 Connect

**Today.** One card: server, username, password, Connect; Quick Connect card; LAN discovery under
the server caption; distinct errors for unreachable/bad credentials/expired token.

**Friction.** It is a single-server, single-user form with no “saved servers”, no certificate
warning for self-signed HTTPS, no “test connection” before sign-in, and no explanation of what
Quick Connect is until it is opened. The error keeps the typed values (good), but the next action is
not always obvious.

**Fix.** Show saved servers as cards with name/address/last user/status; “Add server”; a
certificate trust prompt with the fingerprint; “Test connection” that probes `/System/Info/Public`
without signing in; a one-line explanation under Quick Connect (“Sign in on another device with this
code”). Keep the current error taxonomy.

**Evidence.** Jellyfin clients all support saved servers; Emby Connect; Plex sign-in. **Effort:** M.

### 5.2 Shell and sidebar

**Today.** Collapsible sidebar; Home, Search, Remote, Settings; libraries; playlists; user chip.

**Friction.** Utilities sit above the user’s content; there is no “Browse” grouping, no pinned
libraries, no counts, no now-playing row, and the collapsed state hides the server name. The
selected state is a fill; the focus ring is separate (good).

**Fix.** Order: **Home**, **Browse** (Libraries, Collections, Genres, Watchlist), **Tools** (Search,
Remote, Downloads, Settings), then Playlists. Pin/favourite libraries; show item counts; add a
persistent now-playing row above the user chip; keep the collapsed icon rail with tooltips; add
`1–9` shortcuts for pinned libraries.

**Evidence.** Plex/Emby/Kodi put content first; research 02 says the sidebar must not contradict
the content area. **Effort:** M.

### 5.3 Home

**At the time.** Ambient backdrop; title/logo; rails: In progress, Next up, Collections, up to four
Recently added; skeleton/empty/error; cards with progress and watched badge.

**Friction.** The first rail is mislabelled; the order contradicts the contract; Recently Played is
missing; the cap can be exceeded; Continue Watching cards do not show the series/episode/remaining
time; there is no “See all” on the first two rails; no “remove” on a resume card; no “new” badge;
no discovery row beyond the server’s similar list.

**Fix.** Apply H1/H2: rename, reorder, cap at six, add Recently Played, add card context and
“See all”, add hover Remove/Details, add “New” badges, add “Because you watched”, “Surprise me” and
“Coming up” only when they have data. Keep the ambient backdrop; do not autoplay a trailer.

**Evidence.** Netflix/Plex/Emby; research 02 R1–R3; research 05 says avoid a full-height featured
bar at 1280×800. **Effort:** S for the contract, M for discovery.

### 5.4 Library

**Today.** Virtualised grid/list; sort Name/Added/Rating + direction; All/Unwatched/Favourites;
Genre menu; A–Z rail; grid/list; hover play; context menu; Play all/Shuffle; empty/loading/error.

**Friction.** The toolbar is crowded and the filter set is shallow; there are no active filter
chips; no multi-select; no saved views; no type-ahead; the list view is a poster row, not a data
view; no 4K/HDR/subtitle badges; no “random”; no “needs attention”.

**Fix.** L1–L3 + L4–L12: a Filters sheet with all server facets, chips, saved views, multi-select and
bulk actions; richer sort; a real list view; type-ahead and section headers; capability badges;
“Needs attention”. Keep the virtualised grid and the frame budget.

**Evidence.** Jellyfin 12 filters, Infuse Smart Filters, Plex/Emby library depth. **Effort:** M–L.

### 5.5 Detail

**Today.** Full-bleed backdrop, logo/title, metadata line, IMDb/TMDb/RT, technical disclosure,
Play/Resume + Start over, Trailer, Favorite, Watched, Playlist, overview, seasons/episodes, next
up, cast, More like this, collection, playlist picker, item menu.

**Friction.** No versions, extras, theme media, watchlist, likes, studios/tags/release date, crew
depth, episode air dates, season-level watched, “play from here”, or spoiler guard. The action row
is long and mixes primary and secondary actions; the technical line is a disclosure (good) but the
“what will play” summary is missing.

**Fix.** P5/P6 + X4–X10. Keep one primary action; move Watchlist/Like/Playlist/Play on into a
compact secondary row or overflow; add a Version chevron; add Extras/Theme; add the metadata block
behind a disclosure; add the spoiler guard.

**Evidence.** Infuse pre-playback specs and Hide Spoilers; Jellyfin 12 collections/crew; Plex
versions/extras. **Effort:** M.

### 5.6 Search

**Today.** `Ctrl+K` global, `Ctrl+F` scoped; 150 ms debounce; grouped Movies/Shows/Episodes/People;
See all; idle suggestions; empty/error.

**Friction.** Search only finds items; no people/genre/studio hints; no recents persistence; no
keyboard selection model in the results; no filters; zero-result recovery is thin; no command
palette.

**Fix.** S1–S5: command palette, `/Search/Hints`, query chips, recents, result actions, zero-result
recovery. Keep the per-keystroke cancellation and the grouped layout.

**Evidence.** Jellyfin 12 studio search and plugin search; Raycast/Linear. **Effort:** M.

### 5.7 Player

**Today.** Layered OSD, trickplay, chapters, segments, skip, audio tracks, subtitles (text/bitmap),
appearance, up-next, queue, stats, buffering, idle inhibit, fullscreen, mini player, MPRIS.

**Friction.** No segment bands, no still-watching, no sleep timer, no resume rewind, no frame step,
no A–B loop, no screenshot, no dialogue boost/night mode, no audio delay, no pitch-corrected speed,
no version/quality menu, no per-item subtitle delay, no zoom/deinterlace, no diagnostics on failure,
no compact/always-on-top.

**Fix.** P1–P12. The OSD is already layered; add a **More** sheet for the long tail (frame step,
loop, screenshot, audio modes, quality, versions, diagnostics) so the primary transport stays
clean. Keep the existing key map and add the new keys to the sheet.

**Evidence.** Apple TV, YouTube, VLC/mpv/IINA, Jellyfin 12, Plex. **Effort:** S–M per item.

### 5.8 Live TV

**Today.** Channel list with now/next; 3-hour guide; tuner open/close; LIVE OSD; zapping; recordings
as a library; timers blocked.

**Friction.** The guide is a teaser; no full-day scroll, program detail, favourites/hide, logos,
search, reminders or “On now” rail; timeshift unknown.

**Fix.** V1–V4. **Effort:** M.

### 5.9 Settings

**Today.** One long column: playback (autoplay, seek step, volume), subtitle mode/background/font/
colour/outline/style/position/language/opacity/plate opacity, audio language, OMDb key, cache clear,
remote, sign out, version/server/device.

**Friction.** Ten subtitle rows dominate; playback is above them but cache/remote are below the
fold; no search; no reset; no export; no explanation of consequences; the OMDb key is a lonely
input with a “get key” link.

**Fix.** S10: sections with a left nav or tabs (**Playback**, **Subtitles**, **Appearance**,
**Network & Server**, **Storage**, **Shortcuts**, **About**); search; reset section; export/import
config; inline explanations; move OMDb under “Metadata (optional)”.

**Evidence.** research 02 §7 and research 05 “settings should explain, not enumerate”. **Effort:** M.

### 5.10 Empty, loading, error, toast

**Today.** Skeletons after 150 ms with final geometry; empty/error states on home/library/detail/
search/person/live/remote; error banner; toast for confirmations.

**Friction.** Empty states are sometimes generic; errors do not always name the next action; toasts
have no undo; offline browsing is not cached; the server-unreachable banner does not distinguish
“wrong URL” from “server asleep”.

**Fix.** S9 + Q7: every empty state says what is empty and offers one action; every error names
the cause and a retry; toasts get Undo and pause on hover; cached home/library pages remain
browsable offline; the banner includes “Retry” and “Open settings”.

**Evidence.** NN/g error-message rubric; research 02 R12. **Effort:** S–M.

### 5.11 Focus, keyboard, pointer

**Today.** One focus ring, roving focus, spatial arrows, Tab between regions, Esc back, `?` sheet,
context menu via Menu/Shift+F10, pointer cursor, hover/press washes.

**Friction.** No command palette, no type-ahead, no back/forward mouse buttons, no shortcut
customisation, no gamepad, and some long-tail actions are mouse-only (e.g. hover play is fine, but
the menu is the only way to some actions). Focus visibility is good but not audited at 200% zoom.

**Fix.** S1/S7/S11 + T5/A6/A9. Add a keyboard route to every action; show shortcut hints in menus;
support mouse back/forward; audit focus contrast and clipping; add gamepad only if Rune exposes it.

**Evidence.** Apple HIG focus guidance; Jellyfin 12 non-Latin shortcuts; 10-foot UI guidance.
**Effort:** M.

### 5.12 Visual system and motion

**Today.** Dark only (`#141414`), one accent, 2 px focus ring, spring ladder, text shadows over
artwork, 2:3 posters, 16:9 episodes, 1:1 people, rails peek.

**Friction.** No light/high-contrast/OLED theme; no text scaling; reduced motion is not a setting;
badges rely on colour in places; the 44 px display type is tuned for desktop, not a TV across the
room.

**Fix.** A1–A3: theme tokens, text scale, reduced motion, colour-blind-safe badges, and a “TV/10-foot”
density option later (larger type, more focus padding, safe-area gutters). Do not change the
default desktop look; make the alternatives first-class.

**Evidence.** WCAG 2.2; Android TV 10-foot guidance; research 05 measured type/motion. **Effort:**
M.

---

## 6. Roadmap

The phases are ordered by **value per week**, not by architecture. Each phase ends with snapshots,
tests and a commit someone can review. The merged IDs in §3 mean one item can cover several old
IDs; the old numbers are kept only where the detailed design still uses them.

### Phase 0 — Truth and polish (about 1 week)

- **H1** (absorbs H10): rename the first rail to **Continue Watching**, order CW → Next Up →
  Recently Added → Recently Played → Collections, cap at six, and add the missing Recently Played
  rail. Add the card context (`S2E4`, time left) and a **New** badge while there.
- **S6**: content before utilities in the sidebar; pin libraries; counts.
- **S9**: undo toasts for favourite/watched/remove-from-CW/playlist.
- **S10**: settings section skeleton (no new settings yet).
- Keep `features.md`, `design.md` and the code in agreement; do not claim a rail that does not exist.

**Done when:** the Home snapshot matches the contract; every destructive one-click action has an
Undo; the sidebar puts libraries first; the docs and the code agree.

### Phase 1 — Player quality of life (3–4 weeks)

P1 segment bands + auto-skip, P2 still watching + sleep timer, P3 (with the merged C1–C3) resume
rewind/checkpoint/transfer/conflict, P5 (with X3/P13) versions + pre-playback summary, P7 audio
intelligence, P9 (with Q6) diagnostics + capability report, P12 queue 2.0, A1 reduced motion,
A4 AD/SDH badges, A8/A9 badge and focus audit.

Start the first half with the playback path: P1, P2, P3, P5, P12. In parallel, Phase 1b:

H5/R2 watchlist (Jellyfin Favorites or a user playlist), R1 likes, L1 filter sheet, L3 multi-select,
L4 sorts, L7 card badges, S2 search hints, S5 result actions, S8 now-playing bar, S10 settings IA,
Q1 stale-while-revalidate, Q3 cache limits, Q7 retry/backoff, Q8 frame tests.

**Done when:** a viewer can resume with a rewind, skip an intro from the bar, be asked “still
watching?”, set a sleep timer, pick a version, watch an extra, use dialogue boost/night mode, and
filter to 4K with English subtitles — with snapshots and tests for each.

### Phase 2 — Playback capability and collector depth (6–8 weeks)

**P4 first**: PlaybackInfo + quality/transcode, direct-play-first. The policy is explicit: direct
play is the default, and the app transcodes only when the reader picks a lower bitrate or the
source cannot be reproduced (codec/container/audio/subtitle). The server supports it; verify
startup latency and the device profile live.

Then P6 extras + theme media, P8 frame step/A–B loop/screenshot/go-to-time, P11 end-of-episode
polish, H2 honest discovery, H4 upcoming, H6 collections depth, H7 optional hero, H8 badges,
H9 why/not interested, L2 saved views, L5 list view, L6 type-ahead, L9 collection management,
L10 playlist 2.0, L11 summary header, L12 needs attention, X4–X10 detail depth, C4/C5 history and
stats, Q5 crash-safe navigation, T1 tray, T2 notifications, T3 MPRIS push/shuffle/loop, A2/A3/A6
themes and shortcuts, V1–V4 live guide/reminders, R3/R4/R7 optional memory and sharing.

**Done when:** a remote/high-bitrate file plays at a chosen quality with a truthful
direct/transcode label while direct play remains the default; a collector can open a collection
with progress, see versions and extras, save a filtered view, and read a private history page.

### Phase 3 — Downloads and platform (3–4 months, split into releases)

Q9 (with Q4) downloads + offline playback: a **Settings → Downloads path**, queue with
pause/cancel, storage budget, play downloaded files through the same pipeline, cached Home/Library/
Detail while the server is away, and report on reconnect. P10 compact/always-on-top, P11 if not
already done, Q2 next-episode prefetch (define whether it is metadata/art or a buffer), S12 deep
links + single instance, T5 gamepad/Steam Deck, T6 autostart/minimised, V5 timeshift investigation,
L8 hover preview (opt-in), A7 only if the product decides.

**Done when:** a downloaded episode plays offline from the chosen path and survives a restart; the
app can live in the tray and open from a link; a second screen has a compact window.

### Phase 4 — Lowest priority / never

A7 i18n + RTL is the only remaining Phase-4 item; treat it as a deliberate later decision. The
non-goals in §3.12 stay out: SyncPlay, music/books/photos, multi-server, web view, social feed,
storefront. **Do not schedule:** V6 (provider 500), T7 (black out monitors), A5 (removed; enable
Rune accessibility instead), H3 (removed), S4 (already shipped), C6 (one user), R6 (SyncPlay).

---

## 7. What not to build

These are as important as the opportunities. Each has evidence.

1. **Autoplay previews/trailers by default.** NN/g’s video-usability guidance is blunt: users do not
   want media that starts without consent; Netflix makes previews a profile toggle; research 05
   explicitly says do not copy Moonfin’s auto-playing trailers. If a hover preview ships, it is
   muted, opt-in, and delayed.
2. **Configurable home rows in v1.** Plex/Emby make it an admin project; research 02 defers it; it
   buys nothing until there are more libraries and more row types. Start with fixed, correct order;
   add hide/show later.
3. **Watch Together / SyncPlay now.** It needs a WebSocket dependency (excluded by the design), a
   group state machine, and a second device to test; Plex removed it from most apps in Feb 2025 and
   Disney removed GroupWatch in Sep 2023. The retention signal is weak.
4. **Fake personalisation.** “Recommended for you” without behavioural data is a lie; use “Because
   you watched X”, genre/decade/studio rows, Instant Mix and “Surprise me”.
5. **Music, books, photos, multi-server, mobile.** Different products; the server may have no such
   libraries, and each needs its own interaction model. Jellyfin 12.0’s books support is real, but
   it is a reader, not a video client.
6. **A web view.** It would reintroduce the rendering tax and release coupling that
   `design.md` §2 exists to avoid.
7. **Infinite feed / short-form / social graph.** Wrong interaction model, moderation burden, no
   server data.
8. **Ads, upsell, rental store.** Not a storefront.
9. **HDR passthrough or Dolby Vision today.** Blocked on wgpu/Wayland colour management and
   libplacebo; ship honest tone-mapped SDR and a badge that says “HDR → SDR”.
10. **Gamification (streaks, badges, “watch 5 more”).** It turns a library into a chore; if stats
    ship, they are reflective, not pushy.

---

## 8. Measurement and experiments

The app has no telemetry, and that should stay true. Measure locally, expose it in a diagnostics
screen, and let the user export it.

**Local counters (no network):**

- Time to first paint (Home/Library/Detail/Search) and time to first frame (player).
- Resume accuracy: server position vs actual start; count of “resume from 0” bugs.
- Back-restore failures: a pop that resets scroll/focus.
- Search success: query → result open; zero-result rate; time to result.
- Filter usage: which facets, how often, saved views.
- Skip intro/credits: offered, used, undone.
- Still-watching: shown, continued, stopped.
- Sleep timer: set, fired, cancelled.
- Playback errors by sink (`DMA-BUF P010`, `NV12`, `BGRA`, `CPU`, `mpv`) and by codec.
- Direct play vs transcode (once P4 exists), and the reason.
- Crash-free sessions, config read/write failures, cache hit rate.
- Undo usage and the action it undid.

**Targets (initial, to be tuned):**

- Home first paint data ≤ 150 ms warm; detail ≤ 100 ms warm; search ≤ 100 ms warm.
- 20k-item grid ≤ 16 ms/frame with filters and selection open.
- Resume start within ±1 s of the server position (plus the configured rewind).
- Zero back-restore failures in a 10-iteration scripted loop.
- Playback error rate < 1% of starts on the test library.
- 100% of destructive one-click actions undoable.

**Experiments (build flags or settings, not A/B infrastructure):**

- One merged Continue Watching vs CW + Next Up (research 02’s riskiest assumption).
- Hero billboard vs no hero at 1280×800.
- Hover preview opt-in vs off.
- Filter sheet vs current toolbar for a week of personal use.
- Card badge density (2 vs 4).
- Resume rewind default 5 s vs 10 s.
- “Still watching?” after 2 vs 3 episodes.

---

## 9. Appendix

### A. API endpoints and fields behind the opportunities

All paths verified against the Jellyfin **12.4.0** OpenAPI. The live server is now 12.4.0, so the
**12.0+** paths can be exercised directly; still feature-detect with `/System/Info/Public` or a
failed request when talking to another server.

| Need | Endpoint / field |
|---|---|
| Playback negotiation | `POST /Items/{itemId}/PlaybackInfo` (`PlaybackInfoDto`, `DeviceProfile`), `GET` variant |
| Media segments | `GET /MediaSegments/{itemId}` (`MediaSegmentDto`: `Type`, `StartTicks`, `EndTicks`) |
| Extras | `GET /Items/{itemId}/SpecialFeatures` (`BaseItemDto.ExtraType`) |
| Theme media | `GET /Items/{itemId}/ThemeMedia?inheritFromParent=true` (`AllThemeMediaResult`) |
| Versions | `Item.MediaSources[]`, `ItemFields.MediaSourceCount`, `PlaybackInfo.MediaSourceId` |
| Instant mix | `GET /Items/{itemId}/InstantMix?limit=` |
| Upcoming | `GET /Shows/Upcoming?userId=&limit=` |
| Next up | `GET /Shows/NextUp` (existing) |
| Filters | `GET /Items/Filters2` (genres, tags, audio/subtitle languages), `GET /Items/Filters` (genres, tags, official ratings, years) |
| Item query facets | `/Items` params: `filters`, `is4K`, `isHd`, `hasSubtitles`, `videoTypes`, `studios`, `personIds`, `tags`, `years`, `officialRatings`, `audioLanguages`, `subtitleLanguages`, `minCommunityRating`, `seriesStatus`, `isUnaired`, `hasThemeSong`, `hasSpecialFeature`, `isMissing` |
| Search hints | `GET /Search/Hints?includePeople=&includeGenres=&includeStudios=` |
| Likes/dislikes | `POST/DELETE /UserItems/{itemId}/Rating?likes=` |
| Favourites | `POST/DELETE /UserFavoriteItems/{itemId}` |
| Played state | `POST/DELETE /UserPlayedItems/{itemId}` (verify season/series bulk behaviour live) |
| Collections | `POST /Collections`, `POST/DELETE /Collections/{id}/Items`, `GET /Items/{itemId}/Collections` |
| Playlists | `POST /Playlists`, `POST/DELETE /Playlists/{id}/Items`, `POST /Playlists/{id}/Items/{itemId}/Move/{newIndex}` |
| Transfer playback | `POST /Sessions/{sessionId}/Playing?playCommand=PlayNow&startPositionTicks=` |
| Remote commands | `POST /Sessions/{sessionId}/Playing/{command}`, `POST /Sessions/{sessionId}/Message` |
| SyncPlay (deferred) | `/SyncPlay/*` + WebSocket (not in the current dependency budget) |
| Downloads | `GET /Items/{itemId}/Download`, `ItemFields.CanDownload` |
| Live guide | `GET /LiveTv/Guide`, `GET /LiveTv/Programs`, `GET /LiveTv/Channels` |
| Storage/health | `GET /System/Info`, `GET /System/Info/Storage`, `GET /System/ActivityLog/Entries` |
| Localisation | `GET /Localization/Cultures`, `GET /Localization/ParentalRatings` |

### B. Wire-type fields to add (all in `BaseItemDto`)

`Tags`, `Studios`, `SeriesStudio`, `ExtraType`, `PremiereDate`, `DateCreated`, `DateLastMediaAdded`,
`MediaSourceCount`, `SpecialFeatureCount`, `LocalTrailerCount`, `CanDownload`, `ExternalUrls`,
`Taglines`, `OriginalTitle`, `ProductionLocations`, `OfficialRating`, `CustomRating`, `SeriesStatus`,
`IsHD`, `Is4K`, `HasSubtitles`, `HasThemeSong`, `HasThemeVideo`, `PlayAccess`, `AspectRatio`,
`RunTimeTicks` (already), `CumulativeRunTimeTicks`, `VideoType`, `IsMissing`, `IsUnaired`,
`IndexNumberEnd`, `RemoteTrailers` (already), `MediaStreams` (already via `MediaSources`).

### C. Config keys to add (all in `config.toml`, 0600)

`skip_intro` (`ask`/`always`/`never`), `skip_credits`, `skip_recap`, `still_watching_episodes`,
`still_watching_minutes`, `sleep_timer_minutes`, `resume_rewind_seconds`, `theme_songs`,
`theme_videos`, `theme_mute`, `audio_dialogue_boost`, `audio_night_mode`, `audio_downmix`,
`audio_delay_ms`, `pitch_corrected_speed`, `subtitle_delay_per_item` (map), `spoiler_guard`,
`reduced_motion`, `theme` (`dark`/`light`/`contrast`/`oled`), `text_scale`, `watchlist_playlist_id`,
`recent_searches` (capped), `saved_views`, `trakt_token` (0600), `download_path`,
`download_budget_bytes`, `delete_after_watched`, `transcode_max_bitrate` (optional),
`tray_close`, `notifications`, `deep_link_single_instance`.

### D. Snapshot scenes to add

`home` (contract), `home-discovery`, `home-upcoming`, `home-watchlist`, `home-light`,
`home-contrast`, `library-filters`, `library-selection`, `library-saved-view`, `detail-versions`,
`detail-extras`, `detail-theme`, `detail-deep`, `detail-spoilers`, `detail-resume-rewind`,
`in-app-segments`, `in-app-segments-off`, `in-app-still-watching`, `in-app-sleep-timer`,
`in-app-quality`, `in-app-preplay`, `in-app-audio-modes`, `in-app-transfer`, `command-palette`,
`command-palette-commands`, `search-hints`, `settings-accessibility`, `settings-trakt`,
`downloads`, `downloads-offline`, `live-guide-day`, `live-program`, `live-reminder`,
`compact-player`, `toast-undo`.

### E. Test plan for the new surface

- `jellyfin-api`: query-string tests for every new `/Items` parameter and `PlaybackInfo` body;
  captured fixtures for `SpecialFeatures`, `ThemeMedia`, `SearchHints`, `Filters2`,
  `MediaSegments`, `Collections`, `Sessions/{id}/Playing` with `startPositionTicks`.
- `runefin`: `Runtime` tests for rail order/cap, filter sheet, multi-select, command palette,
  spoiler blur, version selection, segment bands, still-watching with a fake clock, sleep timer,
  resume rewind, undo, watchlist toggle, history log dedupe, tray command queue.
- Ignored live tests: `PlaybackInfo` on the real server (it transcodes); `SpecialFeatures`/`ThemeMedia` on the real
  library; `Collections` create/add/remove with cleanup; `Search/Hints`; `Shows/Upcoming`;
  `MediaSegments`; transfer playback with a second session; downloads to a temp dir.
- Frame budget: the 20k grid with filters and selection open must stay under 16 ms.
- Accessibility: a manual pass with keyboard only, 200% text, high contrast, reduced motion; document
  what Rune exposes (and does not) for screen readers.

### F. Open questions for the user

The 2026-11-05 revision settled several of these: downloads are planned with a Settings path,
multi-user is out, SyncPlay stays out, i18n is lowest priority, and transcoding is unblocked with
direct-play-first.

1. **Collector vs casual.** Are versions/extras/theme/filters worth more than discovery/stats? The
   catalogue assumes a collector; say if not.
2. **Theme.** Is light/high-contrast a real request, or is dark-only still the product?
3. **TV/Steam Deck.** Is a 10-foot mode a target, or is the laptop the only screen? It changes
   type, focus padding, gamepad and safe areas.
4. **Trakt/Letterboxd.** Is external memory wanted, or is local history enough?
5. **Server version.** When does the live server move to 12.x? Episode versions, books, studio
   search and the new filters depend on it.
6. **Downloads scope.** Which items should be downloadable first: movies, next episodes, or the
   whole queue? The storage path and budget are already decided.

---

## 10. Sources

Primary sources used for the survey; the project docs are cited in the text.

- Jellyfin 12.0 release notes — <https://jellyfin.org/posts/jellyfin-release-12.0>
- Jellyfin for Android TV 0.19 — <https://jellyfin.org/posts/androidtv-v0.19.0>
- Jellyfin movies (versions, extras, theme media) — <https://jellyfin.org/docs/general/server/media/movies>
- Jellyfin shows (versions, theme media) — <https://jellyfin.org/docs/general/server/media/shows>
- Jellyfin transcoding (client sends the device profile) — <https://jellyfin.org/docs/general/post-install/transcoding>
- Jellyfin OpenAPI 12.4.0 — <https://api.jellyfin.org/openapi/jellyfin-openapi-stable.json>
- Plex plans (Skip Intro & Credits, downloads, rewind on resume, Pass Out Protection, watchlists,
  multiple cuts, parental controls, themes) — <https://www.plex.tv/plans/>
- Plex Watch Together removal (2025-02-25) — <https://www.plex.tv/blog/coming-in-hot-watch-together-chill>
- Disney GroupWatch removal — <https://whatsondisneyplus.com/disney-removes-groupwatch-feature>
- Prime Video X-Ray — <https://www.amazon.com/salp/xray>
- Apple TV app playback controls (skip-back subtitles, Enhance Dialogue, PiP, speed) —
  <https://support.apple.com/guide/tvapp/control-playback-atvb5e386e66/web>
- Apple TV app guide (profiles, restrictions, autoplay, accessibility) —
  <https://support.apple.com/guide/tvapp/welcome/web>
- Max 4K/HDR/Dolby Vision — <https://help.max.com/US/Answer/Detail/000002523>
- Netflix autoplay next — <https://help.netflix.com/en/node/121518>
- Netflix autoplay previews — <https://help.netflix.com/en/node/2102>
- Netflix Smart Downloads — <https://help.netflix.com/en/node/122916>
- Netflix “Play Something” — <https://techcrunch.com/2021/10/04/netflix-brings-its-shuffle-mode-feature-play-something-to-android-users-worldwide/>
- Netflix artwork personalisation (why Runefin should not fake it) —
  <https://netflixtechblog.com/artwork-personalization-at-netflix-c589f074ad76>
- YouTube sleep timer — <https://support.google.com/youtube/answer/15397997>
- YouTube 2024 feature update (fine speed, sleep timer, miniplayer) —
  <https://www.thurrott.com/music-videos/311518/youtube-is-getting-a-new-miniplayer-sleep-timer-and-more>
- Infuse release notes (Smart Filters, Hide Spoilers, auto-select next, continuous playback) —
  <https://firecore.com/releases>
- Infuse settings (hide spoilers, AI upscaling, black out other monitors) —
  <https://support.firecore.com/hc/en-us/articles/360015608854-Settings-Overview>
- Infuse Trakt sync — <https://support.firecore.com/hc/en-us/articles/4405098078743-Trakt-Sync>
- Kodi 22 “Piers” RC 1 (versions, HDR overlays, deep links) —
  <https://kodi.tv/article/kodi-22-piers-rc-1>
- Emby Cinema Intros — <https://emby.media/support/articles/Cinema-Intros.html>
- Emby feature matrix (theme songs/videos, intro skipping, downloads) —
  <https://emby.media/support/articles/Premiere-Feature-Matrix.html>
- Stremio addon guide (catalogues) — <https://stremio.github.io/stremio-addon-guide/step3>
- Letterboxd (diary, lists, ratings, watchlist, stats) —
  <https://letterboxd.com/about/what-is-letterboxd/>
- TheIntroDB (community segments, no-auth reads) — <https://introdb.app>
- TheIntroDB Jellyfin plugin — <https://github.com/TheIntroDB/jellyfin-plugin>
- Jellyfin SyncPlay explained — <https://www.androidauthority.com/jellyfin-syncplay-explained-3530437/>
- WCAG 2.2 — <https://www.w3.org/TR/WCAG22/>
- W3C media accessibility (captions, audio description) — <https://www.w3.org/WAI/media/av>
- NN/g video usability (autoplay) — <https://www.nngroup.com/articles/video-usability>
- NN/g indicators, validations and notifications (toasts) —
  <https://www.nngroup.com/articles/indicators-validations-notifications>
- NN/g error-message scoring rubric — <https://www.nngroup.com/articles/error-messages-scoring-rubric>
- Command palette pattern — <https://uxpatterns.dev/patterns/advanced/command-palette>
- Android TV 10-foot UI guidance —
  <https://developer.android.com/design/ui/tv/guides/foundations/design-for-tv>
- Existing project research: `01-jellyfin-clients.md`, `02-media-ux-patterns.md`,
  `03-streaming-polish.md`, `04-ui-ux-finishing-pass.md`, `05-moonfin-linux-desktop.md`,
  `06-parity-audit-2026-11.md`.

---

*Written 2026-11-04. The live server was not exercised for this document; the API claims come from
the 12.4.0 OpenAPI and the project's own live-test notes. Re-check anything marked **12.0+** before
promising it.*
