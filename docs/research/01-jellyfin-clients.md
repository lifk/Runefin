# Jellyfin client landscape — survey for Runefin

Snapshot: November 2026, server 10.11/12.x. In-app decoding is context only — Runefin has no
decoder and launches mpv.

## Jellyfin Web

**Stack.** TypeScript/React on webpack, built on the generated `@jellyfin/sdk`, `hls.js` for HLS
playback and `libass-wasm` for ASS subtitles ([package.json](https://github.com/jellyfin/jellyfin-web/blob/master/package.json)).

**Navigation.** Persistent left drawer plus an AppBar; library views carry their own tab strip.
The MUI-based "experimental layout" ([PR #4291](https://github.com/jellyfin/jellyfin-web/pull/4291))
adds a pinned sidebar and, in [PR #6582](https://github.com/jellyfin/jellyfin-web/pull/6582), moves
library switching into the header as ordered tabs with a "More" overflow — because the number of
libraries made the old model collapse.

**Playback.** `<video>` + HLS, server transcodes on demand; also the reference cast receiver.

**Does well.** Zero install, always current with the API, trickplay, SyncPlay, and universal
remote-controllability.

**Complaints.** Maintainers call the debt structural: "The web client has a ton of technical debt and
bad design decisions… a brand new client based on modern technology … is the easier route"
([#1901](https://github.com/jellyfin/jellyfin-web/issues/1901)). Users report slowdowns after 10.11,
home rows loading one-by-one
([r/jellyfin](https://reddit.com/r/jellyfin/comments/1olmc1u/interface_slower_since_1011_update/)),
and friction from the 12.x pass — pagination dropped from the bottom of grids, a "Play all" button
nobody asked for ([r/jellyfin](https://reddit.com/r/jellyfin/comments/1wkd23h/im_all_for_updating_the_ui_but/)).

## Jellyfin Media Player → Jellyfin Desktop (`jellyfin/jellyfin-desktop`)

**Stack.** C++/Qt 6 WebEngine + libmpv via MpvQt. Repo created 2021, renamed and moved to Qt 6 in
[v2.0.0](https://github.com/jellyfin/jellyfin-desktop/releases/tag/v2.0.0) (Dec 2025), with the
explicit warning that old settings would not migrate.

**Navigation.** Identical to Jellyfin Web — because since
[v1.11.0](https://github.com/jellyfin/jellyfin-desktop/releases/tag/v1.11.0) the app "always use[s]
the external web client", i.e. it loads the *server's* copy of jellyfin-web inside a WebEngine view.

**Playback.** The web client's `<video>` is intercepted and handed to an embedded libmpv, giving
MKV/ASS/audio-passthrough/HDR without transcoding
([README](https://github.com/jellyfin/jellyfin-desktop)).

**Does well.** Direct play of formats the browser cannot handle, MPRIS/media-key integration,
multi-profile, per-profile `mpv.conf`.

**Complaints.** UI jank against the same web UI in a browser is the oldest wound — "[UI/UX] Choppy UI
and Scrolling … Compared to Web Client" ([#1001](https://github.com/jellyfin/jellyfin-desktop/issues/1001)),
open since 2022 as "Poor UI Performance" ([#299](https://github.com/jellyfin/jellyfin-desktop/issues/299)),
still reported on Windows in 2026 ([r/jellyfin](https://reddit.com/r/jellyfin/comments/1sy2bh0/jellyfin_media_player_interface_lagging_on_windows/)).
Teardown is fragile: black screen and a dead scrollbar after an episode ends
([#634](https://github.com/jellyfin/jellyfin-desktop/issues/634)). Mouse-only control recurs in HTPC
threads ([r/jellyfin](https://reddit.com/r/jellyfin/comments/1vzxx9b/keyboard_navigation_on_desktop_client/)).
And wrapping the server's web client couples app releases to server releases — v1.12.0 had to "fix
caching issues to prevent broken web client when the server web version updates".

## Swiftfin

**Stack.** Swift/SwiftUI on iOS 18+/tvOS 26+, `jellyfin-sdk-swift` (OpenAPI-generated), MPV (via
MPVUI) as default player with AVPlayer as the alternative
([players.md](https://github.com/jellyfin/Swiftfin/blob/main/Documentation/players.md)).

**Navigation.** Five `TabItem`s — Home, Media, Live TV, Search, Settings — plus an admin dashboard
([TabItem.swift](https://github.com/jellyfin/Swiftfin/blob/main/Shared/Coordinators/Tabs/TabItem.swift)),
with a user-selectable `TabBarPlacement` of `sidebar` or `tabBar`
([TabBarPlacement.swift](https://github.com/jellyfin/Swiftfin/blob/main/Shared/Objects/TabBarPlacement.swift)).

**Playback.** In-app MPV or AVPlayer; the docs are honest that AVPlayer wins AirPlay and tvOS
framerate matching, MPV wins containers and secondary subtitles.

**Does well.** The most native-feeling client; honest trade-off docs; per-item routing that makes
deep links and window restore trivial.

**Complaints.** Session loss on tvOS after storage pressure wiped Core Data — 68 comments
([#776](https://github.com/jellyfin/Swiftfin/issues/776)); repeated player-engine regressions
([#1853](https://github.com/jellyfin/Swiftfin/issues/1853), [#790](https://github.com/jellyfin/Swiftfin/issues/790));
tvOS showing an empty library where iOS is fine ([#621](https://github.com/jellyfin/Swiftfin/issues/621)).

## Streamyfin

**Stack.** React Native + Expo (TypeScript), MPV via MPVKit as the primary player, optional
server-side `jellyfin-plugin-streamyfin` for pushing settings to all users
([README](https://github.com/streamyfin/streamyfin)).

**Navigation.** Bottom tab bar (labels optional), a hero carousel on Home, then rails: Continue
Watching, Next Up. Library tab drills folder → filter sheet.

**Playback.** In-app MPV on all platforms, Chromecast for external handoff, downloads by having the
server transcode to HLS which FFmpeg muxes locally.

**Does well.** Trickplay ("the new golden standard"), skip intro/credits, Seerr requests, sessions
view, and server-pushed defaults so one admin configures every user.

**Complaints.** Performance scales with library count: a user bisected it to the 17th library
before the UI "crumbled" ([#379](https://github.com/streamyfin/streamyfin/issues/379)). First-run
emptiness with no explanation, unreadable filters, hidden tab labels, and filter/sort state lost on
restart all appear in one widely-referenced report
([#764](https://github.com/streamyfin/streamyfin/issues/764)). TV support was hard precisely because
of remote input: "The hard part is adapting the UI to work with remotes"
([#137](https://github.com/streamyfin/streamyfin/issues/137)).

## Findroid

**Stack.** Kotlin + Jetpack Compose (post-[#974](https://github.com/jarnedemeulemeester/findroid/issues/974)
rewrite), `org.jellyfin.sdk:jellyfin-core`, Media3/ExoPlayer with an optional libmpv player
([libs.versions.toml](https://github.com/jarnedemeulemeester/findroid/blob/main/gradle/libs.versions.toml)).

**Navigation.** `TabBarItem`s for Home / Libraries / Downloads in a Compose bottom bar
([NavigationRoot.kt](https://github.com/jarnedemeulemeester/findroid/blob/main/app/phone/src/main/java/dev/jdtech/jellyfin/NavigationRoot.kt)),
separate TV module with its own screens.

**Playback.** In-app ExoPlayer or mpv, direct play only — the README states "Direct play only, (no
transcoding)" ([README](https://github.com/jarnedemeulemeester/findroid)).

**Does well.** Fast native scrolling, offline downloads, trickplay, chapters, per-item player
fallback.

**Complaints.** Codec gaps in the default player (HEVC 10-bit,
[#176](https://github.com/jarnedemeulemeester/findroid/issues/176)); Android TV took years
([#927](https://github.com/jarnedemeulemeester/findroid/issues/927)).

## Finamp

**Stack.** Flutter/Dart, `just_audio` + `media_kit`, `audio_service` (Android Auto/CarPlay, MPRIS),
hand-rolled Chopper API client ([pubspec.yaml](https://github.com/finamp-app/finamp/blob/redesign/pubspec.yaml)).

**Navigation.** Bottom tabs (Home, Library) with a persistent expandable queue panel and a lyrics
screen — the player is a *state*, not a page ([README](https://github.com/finamp-app/finamp)).

**Playback.** Audio-only in-app pipeline: gapless, ReplayGain, transcoded streaming, offline
downloads, and Playback Reporting support while offline.

**Does well.** Queue-first design; downloads are a first-class library, not a cache; correct resume
across devices for music.

**Complaints.** The 1.x UI was dated enough that the redesign thread became the repo's largest by far
— 279 comments ([#220](https://github.com/finamp-app/finamp/issues/220)); background playback
reliability ([#956](https://github.com/finamp-app/finamp/issues/956)).

## Jellyfin for Kodi

**Stack.** Python add-on driving Kodi's own player.

**Navigation.** None of its own. In sync mode Jellyfin libraries appear as native Kodi
Movies/TV Shows under whatever skin you use; unsynced libraries appear as plugin listings
([docs](https://jellyfin.org/docs/general/clients/kodi)).

**Playback.** Two modes: addon mode streams through Kodi via the plugin URL, native mode hands Kodi
a direct file path (required for chapter images and for add-ons that need the real path)
([README](https://github.com/jellyfin/jellyfin-kodi)).

**Does well.** "Ultra smooth navigation" is the README's own pitch and it is earned — the UI is
Kodi's, not a webview's. Two-way watched/resume state, any skin, remote-controllable from Jellyfin
web/mobile.

**Complaints.** Library cleanup deleting Jellyfin entries ([#409](https://github.com/jellyfin/jellyfin-kodi/issues/409)),
slow sync ([#725](https://github.com/jellyfin/jellyfin-kodi/issues/725)), wrong resume position
([#912](https://github.com/jellyfin/jellyfin-kodi/issues/912)), subtitles not saved
([#657](https://github.com/jellyfin/jellyfin-kodi/issues/657)).

## Emby Theater lineage (retired)

Emby Theater Windows was a WinForms shell running an Electron app side by side, with DirectShow +
MadVR for playback ([archived README](https://github.com/jellyfin-archive/emby-theater-windows));
the Electron half survives as the archived `Emby.Theater.Electron` repo. Jellyfin's line kept the
pattern and swapped both halves: Qt WebEngine instead of WinForms+Electron, libmpv instead of
DirectShow — renamed `jellyfin-desktop` in 2025.

The lesson: every generation is a web UI in a shell with a native decoder bolted on, and every
generation inherits the same two defects — a rendering tax on the UI and release-coupling to
jellyfin-web. Runefin's premise is that both are avoidable.

## Patterns to steal

1. **Server-driven home, cheap first paint.** One `/UserViews` call, then a small fixed set of rails
   (`/UserItems/Resume`, Next Up, latest per library). Never fan out per library on the home screen —
   that is exactly the Streamyfin failure mode in [#379](https://github.com/streamyfin/streamyfin/issues/379).
2. **Addressable routes for every item type.** Copy Swiftfin's `NavigationRoute+Item/Library/Media`
   split: deep links, window restore, and scripted navigation all come free.
3. **Quick Connect as the primary auth path**, password as fallback. It exists to avoid typing on
   constrained inputs, and the docs table shows exactly which clients implement it
   ([docs](https://jellyfin.org/docs/general/server/quick-connect/)).
4. **Be a cast target as well as a cast sender.** mpv-shim's headline feature is "Fully controllable
   from Jellyfin web and mobile apps"; achieving that means polling `/Sessions` and honouring
   `PlaybackStopped`/`Play` commands, not just reporting progress.
5. **Report progress on a timer plus on every state change**, with `PositionTicks`/`IsPaused`/`PlayMethod`,
   so server-side resume and the dashboard stay truthful
   ([OpenAPI](https://api.jellyfin.org/openapi/jellyfin-openapi-stable.json), `ReportPlaybackProgress`).
6. **Spawn mpv as a child process and own its IPC socket.** Do not embed, do not re-encode; set
   `--input-ipc-server` and map pause/seek/stop both ways. This is the only path that keeps HDR and
   tone mapping intact ([mpv-shim](https://github.com/jellyfin/jellyfin-mpv-shim)).
7. **Persist navigation and view state per user + per library** — sidebar state, last library, sort,
   filter — and always show nav labels. Both jellyfin-web's TODO list and Streamyfin's #764 point at
   this gap; Streamyfin users had to hunt for tabs whose labels were hidden.

## Patterns to avoid

1. **Embedding the server's web client.** It is the root cause of the desktop client's decade of
   jank ([#1001](https://github.com/jellyfin/jellyfin-desktop/issues/1001),
   [#299](https://github.com/jellyfin/jellyfin-desktop/issues/299)) and of its version coupling
   (v1.12.0 release notes). A Rust-native UI is the entire point of Runefin; do not re-import the
   problem via a webview for "just the settings page".
2. **Mouse-only interaction.** HTPC users hit this immediately
   ([r/jellyfin](https://reddit.com/r/jellyfin/comments/1vzxx9b/keyboard_navigation_on_desktop_client/)),
   and Streamyfin's maintainer names remote adaptation as the blocker for TV support
   ([#137](https://github.com/streamyfin/streamyfin/issues/137)). Full keyboard focus model + escape
   handling from day one.
3. **Unbounded per-library requests.** Streamyfin's UI collapsed at the 17th library
   ([#379](https://github.com/streamyfin/streamyfin/issues/379)). Page, cache, and render lazily.
4. **An empty first run with no explanation.** Streamyfin showed a blank home with no reason
   ([#764](https://github.com/streamyfin/streamyfin/issues/764)); the desktop client's v1.10.1 notes
   warn that an older server yields "a blank home screen" with no message. Detect server version and
   say so.
5. **Renaming and re-platforming the product.** `jellyfin-media-player` → `jellyfin-desktop` with
   non-migrating settings ([v2.0.0](https://github.com/jellyfin/jellyfin-desktop/releases/tag/v2.0.0)),
   plus a CEF client that appeared and stopped being official, left users asking which client is even
   official ([r/jellyfin](https://reddit.com/r/jellyfin/comments/1vl6quo/desktop_client_selection/)).
   Ship one name and a settings migration path.

## REST endpoints the surveyed clients depend on

Verified against the stable OpenAPI document (`info.version` 12.4.0,
[spec](https://api.jellyfin.org/openapi/jellyfin-openapi-stable.json)). "Verified use" means I read
the calling code, not just that the endpoint exists.

| Concern | Endpoints | Verified use | Notes |
|---|---|---|---|
| Auth (password) | `POST /Users/AuthenticateByName` | Verified directly in Kodi `api.py`; other clients use generated SDKs for the same call | Returns the access token and `User.Id`; send it in the `Authorization: MediaBrowser` header |
| Auth (Quick Connect) | `POST /QuickConnect/Initiate`, `GET /QuickConnect/Connect`, `POST /QuickConnect/Authorize`, `POST /Users/AuthenticateWithQuickConnect` | Jellyfin Web, Jellyfin Desktop, Swiftfin iOS/tvOS ([docs](https://jellyfin.org/docs/general/server/quick-connect/)) | Kodi: not supported. Findroid, Streamyfin, Finamp: **unverified** — no Quick Connect code in Findroid's tree, but absence was not proven |
| Library browsing | `GET /UserViews` (as `/Views` on the Kodi path), `GET /Items`, `GET /Items/Filters` | `/Views` + `/Items` verified in Kodi `api.py`; Web/Findroid/Swiftfin/Finamp go through generated SDKs | `/Items` takes `parentId`, `includeItemTypes`, `sortBy`, `fields`; `/Items/Filters` is the filter-vocabulary call |
| Resume state | `UserData.PlaybackPositionTicks` inline on `/Items` (`fields=UserData`), `GET /UserItems/Resume`, `POST /UserItems/{itemId}/UserData` | Kodi reads/writes `UserData.PlaybackPositionTicks` in `objects/actions.py`; `/UserItems/Resume` is the current rail | `/Users/{userId}/Items/Resume` is **not** in 12.4.0 — do not copy old blog posts |
| Images | `GET /Items/{itemId}/Images/{imageType}` with `maxWidth`/`maxHeight`/`tag`/`quality`/`blur` | Endpoint + params verified in spec; per-client call sites **unverified** (SDK-mediated) | Keep `tag` in cache keys or posters go stale after metadata refresh |
| Playback reporting | `POST /Sessions/Playing`, `POST /Sessions/Playing/Progress`, `POST /Sessions/Playing/Stopped` | Kodi `api.py` hits `/Playing`, `/Playing/Progress`, `/Playing/Stopped` | Start must precede progress; always send `PlaybackStopInfo` with final `PositionTicks` or resume drifts |
| Cast / remote control | `GET /Sessions`, `POST /Sessions/{sessionId}/Playing` (`playCommand`, `itemIds`, `startPositionTicks`), plus `/Sessions/{sessionId}/Command` | Kodi README claims remote control from web/mobile; shapes verified in spec, receive loop **unverified** | `PlayCommand` matters: `PlayNow` vs `PlayNext` vs `PlayLast` |

Two API-shape warnings: playback reporting is three posts with different payload types
(`PlaybackStartInfo` / `PlaybackProgressInfo` / `PlaybackStopInfo`), and `/Items` omits `UserData`
unless requested in `fields`.
