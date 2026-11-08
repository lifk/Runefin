# Snapshot captures

Every PNG in this directory is rendered by `scripts/snapshots.sh` from a named `AppSpec` scene,
headlessly, at a fixed virtual time and scale (2× unless noted). The renderer is the same one the
window uses, so a diff in the image is a diff in the UI. These are the captures committed with
the code; **personal live captures are never committed** (they contain the real session's ids and
artwork).

```sh
scripts/snapshots.sh       # regenerate every file in place; commit intended diffs
target/debug/runefin --list-scenes
target/debug/runefin --snap /tmp/home.png --scene home --size 1440x900 --scale 2
```

150 files from 116 scenes (a few scenes render at more than one size). Regenerated on 2026-11-07
from `main`.

## Big screen

Fourteen scenes also render at the big-screen 1280×720 design height: `*-big.png` at 1.5×
(exactly 1920×1080) with `--big-screen`, and eleven of them again as `*-big-4k.png` at 3×
(3840×2160). The pair proves the same
point layout and the 64 pt safe gutter hold at 1080p and 4K; the 4K frames are not a second
layout.

`home-art`, `home-art-light` and `detail-art` use a synthetic high-contrast image (no personal
library art) to prove text, logos and scrims over bright and dark artwork. `home-art-light` uses
the light theme; the others use the dark theme.

## Connect and shell

| File | Scene / size | What it proves |
|---|---|---|
| `connect-idle.png` | `idle`, 560×520 | Server field, discovery, Quick Connect entry, no signed-in data. |
| `connect-discovery.png` | `connect-discovery`, 1280×800 | LAN discovery results fill the form. |
| `connect-quick.png` | `connect-quick`, 560×520 | Quick Connect code and cancel state. |
| `connect-signing-in.png` | `signing-in`, 560×520 | Busy state during sign-in. |
| `connect-error.png` | `error`, 560×520 | Error copy and a usable retry. |
| `shortcuts.png` | `shortcuts`, 1280×800 | The shortcut sheet; player rows show their current bindings. |
| `settings.png` | `settings`, 1280×800 | Playback, text size, notifications and the settings search. |
| `settings-keyboard.png` | `settings`, 1280×2600 | The full Keyboard section: eight remappable actions and Reset. |
| `settings-seerr.png` | `settings-seerr`, 1280×3600 | The Requests (Seerr) card: address, masked key, user id, Test/Clear/Open Discover and the connected status. |
| `toast.png` | `toast`, 1280×800 | A short confirmation over a page. |

## Home

| File | Scene / size | What it proves |
|---|---|---|
| `home.png` | `home`, 1280×800 | Continue Watching, Next Up, rails and the ambient backdrop. |
| `home-1440.png` | `home`, 1440×900 | The same layout at the wider desktop size. |
| `home-contrast.png` | `home-contrast`, 1280×800 | High-contrast surfaces. |
| `home-light.png` | `home-light`, 1280×800 | The light theme. |
| `home-hero.png` | `home-hero`, 1280×800 | The optional compact Home hero. |
| `home-contract.png` | `home-contract`, 1280×800 | The Phase 0 Home contract in one frame. |
| `toast-undo.png` | `toast-undo`, 1280×800 | An Undo toast with its action. |
| `home-collapsed.png` | `home-collapsed`, 1280×800 | Collapsed sidebar and the icon rail. |
| `home-shelf.png` | `home-shelf`, 1280×800 | A Home rail's full “See all” grid. |
| `home-collections.png` | `home-collections`, 1280×800 | Collections rail. |
| `home-loading.png` | `home-loading`, 1280×800 | Skeleton geometry before data. |
| `home-empty.png` | `home-empty`, 1280×800 | Empty-library state. |
| `home-error.png` | `home-error`, 1280×800 | Unreachable-server banner and retry. |
| `card-menu.png` | `card-menu`, 1280×800 | Right-click item menu: play, playlist, add to collection, watched, favourite, resume removal. |
| `cast-picker.png` | `cast-picker`, 1280×800 | “Play on…” target list. |

## Libraries, playlists and collections

| File | Scene / size | What it proves |
|---|---|---|
| `library.png` | `library`, 1280×800 | Grid, toolbar, A–Z rail, progress and watched badges. |
| `library-1024.png` | `library`, 1024×768 | Grid reflow at a smaller desktop. |
| `library-1440.png` | `library`, 1440×900 | Grid at the wider desktop size. |
| `library-narrow.png` | `library`, 800×600 | The narrow grid and toolbar. |
| `library-list.png` | `library-list`, 1280×800 | List mode. |
| `library-genres.png` | `library-genres`, 1280×800 | Genre menu and filter. |
| `library-shows.png` | `library-shows`, 1280×800 | A TV library with season/episode cards. |
| `library-loading.png` | `library-loading`, 1280×800 | Loading rows. |
| `library-empty.png` | `library-empty`, 1280×800 | Empty filtered/first-run state. |
| `library-error.png` | `library-error`, 1280×800 | Library request failure. |
| `library-filters.png` | `library-filters`, 1280×800 | The full filter/sort sheet. |
| `library-filters-views.png` | `library-filters-views`, 1280×800 | Saved views inside the filter sheet. |
| `library-select.png` | `library-select`, 1280×800 | Multi-select bulk bar, including Add to collection. |
| `library-badges.png` | `library-badges`, 1280×800 | 4K/HDR/CC/5.1/VERS/NEW text badges. |
| `watchlist.png` | `watchlist`, 1280×800 | The cross-library Watchlist page. |
| `collection.png` | `collection`, 1280×800 | A box set's detail page with its one-line sort and member titles. |
| `playlist.png` | `playlist`, 1280×800 | Ordered playlist grid, Play all / Shuffle. |
| `playlist-edit.png` | `playlist-edit`, 1280×800 | Drag hint, move/remove controls. |
| `playlist-picker.png` | `playlist-picker`, 1280×800 | Add-to-playlist panel. |
| `playlist-create.png` | `playlist-create`, 1280×800 | New-playlist prompt. |

## Detail, person and search

| File | Scene / size | What it proves |
|---|---|---|
| `detail.png` | `detail`, 1280×800 | Backdrop, scores, actions, overview, series chips, next up. |
| `detail-narrow.png` | `detail-similar`, 800×600 | A recommendation rail at the narrow width (the scene starts from a similar page). |
| `detail-versions.png` | `detail-versions`, 1280×800 | Multiple media sources and the Version row. |
| `detail-collections.png` | `detail-collections`, 1280×800 | “Part of …” collection links. |
| `detail-extras.png` | `detail-extras`, 1280×800 | Special features grouped by type. |
| `detail-theme.png` | `detail-theme`, 1280×800 | Theme song and muted theme video toggles. |
| `detail-tech.png` | `detail-tech`, 1280×800 | Technical disclosure and sparse metadata. |
| `detail-similar.png` | `detail-similar`, 1280×800 | “More like this” rail. |
| `detail-loading.png` | `detail-loading`, 1280×800 | Skeleton hero. |
| `detail-error.png` | `detail-error`, 1280×800 | Missing/failed item. |
| `person.png` | `person`, 1280×800 | A cast member's filmography grid. |
| `person-seerr.png` | `person-seerr`, 1280×1000 | The two sections: library titles, then the rest of the filmography on Seerr. |
| `seerr-person.png` | `seerr-person`, 1280×800 | A person Seerr knows but the server does not, with the requestable rail. |
| `search.png` | `search`, 1280×800 | Grouped results with scopes. |
| `search-seerr.png` | `search-seerr`, 1280×900 | The same query on the server and in Seerr's catalogue. |
| `search-expanded.png` | `search-expanded`, 1280×800 | “See all” grid in place. |
| `search-idle.png` | `search-idle`, 1280×800 | Idle suggestions/recents. |
| `search-empty.png` | `search-empty`, 1280×800 | No-results state. |
| `search-hints.png` | `search-hints`, 1280×800 | `/Search/Hints` people, genres and studios. |
| `remote.png` | `remote`, 1280×800 | Remote-control session list and transport. |

## Seerr companion

| File | Scene / size | What it proves |
|---|---|---|
| `seerr.png` | `seerr`, 1280×800 | Discover/Explore: tabs, search, trending/films rows, availability chips. |
| `seerr-browse.png` | `seerr-browse`, 1280×900 | Discover/Browse: type, sort, decade, genre and studio chips, grid, Load more. |
| `seerr-search.png` | `seerr-search`, 1280×800 | Search results grouped Films and Series. |
| `seerr-detail-tv.png` | `seerr-detail-tv`, 1280×800 | Series detail with the season picker and Request seasons. |
| `seerr-detail-movie.png` | `seerr-detail-movie`, 1280×800 | Film detail with genres, tagline, cast rail and its Processing state. |
| `requests.png` | `requests`, 1280×800 | All requests: a live bar, a completed pack counted once, available and pending rows. |
| `requests-live.png` | `requests-live`, 1280×800 | Active requests with no queue rows: Waiting for a download. |
| `requests-empty.png` | `requests-empty`, 1280×800 | Empty requests. |
| `requests-error.png` | `requests-error`, 1280×800 | Unreachable Seerr. |
| `requests-unconfigured.png` | `requests-unconfigured`, 1280×800 | Setup card; the nav entry is absent. |
| `requests-narrow.png` | `requests`, 800×600 | Request rows at the narrow window. |
| `seerr-130.png` | `seerr-130`, 1280×800 | Discover at 130 % text. |
| `detail-seerr.png` | `detail-seerr`, 1280×800 | Jellyfin series offering Request season 3. |

## Player and playback

| File | Scene / size | What it proves |
|---|---|---|
| `now-playing.png` | `now-playing`, 1280×800 | The browsing shell with the media player's now-playing panel. |
| `now-playing-paused.png` | `now-playing-paused`, 1280×800 | Paused state. |
| `in-app-launching.png` | `in-app-launching`, 1280×800 | “Launching” overlay with cancel. |
| `in-app-osd.png` | `in-app-osd`, 1280×800 | The minimal OSD: top bar, centred transport, timeline row, settings gear. |
| `in-app-mini.png` | `in-app-mini`, 1280×800 | Mini player while browsing. |
| `in-app-intro.png` | `in-app-intro`, 1280×800 | The bottom-right skip pill and the coloured segment band on the timeline. |
| `in-app-gear.png` | `in-app-gear`, 1280×800 | The settings sheet: tracks, playback, timers, navigate, this device, with values. |
| `in-app-skip-credits.png` | `in-app-skip-credits`, 1280×800 | The credits label in the same pill, late in an episode. |
| `in-app-segments.png` | `in-app-segments`, 1280×800 | Segment bands with their click-to-seek ranges. |
| `in-app-still-watching.png` | `in-app-still-watching`, 1280×800 | The quiet “Still watching?” card. |
| `in-app-sleep-timer.png` | `in-app-sleep-timer`, 1280×800 | Sleep-timer options in the OSD. |
| `in-app-preplay.png` | `in-app-preplay`, 1280×800 | Pre-playback summary with versions and the direct-play decision. |
| `in-app-resume-conflict.png` | `in-app-resume-conflict`, 1280×800 | Local checkpoint versus server position after a crash. |
| `in-app-transfer.png` | `in-app-transfer`, 1280×800 | Continue on another device from the current position. |
| `in-app-audio-modes.png` | `in-app-audio-modes`, 1280×800 | Downmix, sound mode and audio delay in the audio panel. |
| `in-app-loop.png` | `in-app-loop`, 1280×800 | A–B loop points on the OSD. |
| `in-app-goto.png` | `in-app-goto`, 1280×800 | Go-to-time panel. |
| `in-app-quality.png` | `in-app-quality`, 1280×800 | Quality options and the active playback method. |
| `in-app-why.png` | `in-app-why`, 1280×800 | The reason behind the current method. |
| `detail-preplay.png` | `detail-preplay`, 1280×800 | Pre-playback summary with the quality decision. |
| `in-app-trickplay.png` | `in-app-trickplay`, 1280×800 | 240×135 trickplay preview, chapter pill, 8 pt hovered bar and knob. |
| `in-app-subtitles.png` | `in-app-subtitles`, 1280×800 | SRT cues over the picture. |
| `in-app-subtitles-clear.png` | `in-app-subtitles-clear`, 1280×800 | Subtitles without the plate. |
| `in-app-subtitle-style.png` | `in-app-subtitle-style`, 1280×800 | Reader appearance: face, colour, outline, weight. |
| `in-app-subtitles-ass.png` | `in-app-subtitles-ass`, 1280×800 | ASS sign: glyph outline, drop shadow, rotation and scale. |
| `in-app-subtitle-panel.png` | `in-app-subtitle-panel`, 1280×800 | Track list including PGS and DVB as normal choices. |
| `in-app-subtitle-search.png` | `in-app-subtitle-search`, 1280×800 | Online subtitle search results. |
| `in-app-image-subtitle.png` | `in-app-image-subtitle`, 1280×800 | A PGS/VobSub bitmap placed over the picture. |
| `in-app-next.png` | `in-app-next`, 1280×800 | Last-30-seconds up-next strip. |
| `in-app-audio.png` | `in-app-audio`, 1280×800 | Audio track menu. |
| `in-app-chapters.png` | `in-app-chapters`, 1280×800 | Chapter list with stills and the current chapter. |
| `in-app-queue.png` | `in-app-queue`, 1280×800 | Queue chip / next episode. |
| `in-app-queue-panel.png` | `in-app-queue-panel`, 1280×800 | The run's item list. |
| `in-app-stats.png` | `in-app-stats`, 1280×800 | Sink/codec/range/position diagnostics. |
| `in-app-buffering.png` | `in-app-buffering`, 1280×800 | Buffering percentage. |
| `in-app-narrow.png` | `in-app-narrow`, 1024×700 | Player at a smaller window. |
| `in-app-narrow-small.png` | `in-app-narrow`, 800×600 | Player at the smallest supported window. |
| `in-app-live.png` | `in-app-live`, 1280×800 | LIVE marker, program name, channel controls. |
| `live-tv-favorites.png` | `live-tv-favorites`, 1280×800 | The channel list filtered to favourites. |
| `live-tv.png` | `live-tv`, 1280×800 | Channel list with now/next and the reminder bell. |
| `live-guide-day.png` | `live-guide-day`, 1280×800 | The 24-hour guide window. |
| `live-guide.png` | `live-guide`, 1280×800 | Three-hour guide grid. |

There is intentionally no snapshot for MPRIS (a bus service, not a surface), for the HDR tone
curve (it is GPU math pinned by CPU tests), or for a fullscreen window (the chip and shortcut are
captured; the window manager owns the mode).

The fixtures behind these frames are public-domain films and invented series, episodes, channels
and people; the scene code never fetches artwork, so no third-party still is committed.
