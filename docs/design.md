# Runefin — design

**Status:** approved; M1–M6 are built, the in-app GStreamer player ships with DMA-BUF/HDR and
mpv fallback, and Phase 2/2b adds direct-play-first transcoding, text scaling, remappable player
shortcuts, live reminders and collection/playlist management depth. D1 and D3 are superseded by
[`design-player.md`](design-player.md); the rest of this document is the browse-app contract.

**One sentence:** a Jellyfin desktop client for Linux, drawn entirely by Rune, that browses your
library and plays video in the same window (mpv is the fallback), reporting progress back to the
server — built as a laboratory for agent-driven UI work, not for feature parity.

Evidence behind the claims here lives in `research/`: `01` (Jellyfin clients), `02` (media-library
IA), `03` (streaming polish). Numbers marked **[est]** are estimates from those briefs, not
published figures.

---

## 0. Decisions I need from you

Recommendation is the default; say "all recommended" and I build to that.

| # | Question | Recommendation | What changes if you disagree |
|---|---|---|---|
| D1 | Does v1 decode video in-app? | ~~No — hand off to mpv.~~ **Superseded by [`design-player.md`](design-player.md): in-app, on Wayland, frames as DMA-BUF into Rune, GStreamer + VA-API.** | That is a framework-sized job: Rune has no DMA-BUF import (issue #23) and exposes no `wgpu::Device`. The addendum is the costed plan. |
| D2 | One merged Continue Watching rail, or CW + Next Up? | **Merged**, one card per series, deduped, stale entries (>60 d untouched) hidden client-side. | Two rails is 3 h more work and shows the same show twice; make it a build flag if unsure. |
| D3 | How is mpv driven? | ~~Spawned `mpv` + JSON IPC.~~ **Superseded:** Wayland cannot embed a foreign window ([mpv #9654](https://github.com/mpv-player/mpv/issues/9654)), so a second window is unavoidable on that path. Kept as the shipped fallback, not the design. See [`design-player.md`](design-player.md). | — |
| D4 | Local catalogue database in v1? | **No.** Server is the source of truth; cache only rendered pages in memory + images on disk. | A local DB buys offline browsing and instant cold start, and costs a schema, migrations and sync bugs. Defer to M7. |
| D5 | Where does the access token live? | **`~/.config/runefin/config.toml`, mode 0600, `zeroize` on drop.** No keyring in v1. | A keyring crate is one dependency and a portal dialog on every reinstall; add once the app is worth it. |
| D6 | Shell shape | **Collapsible left sidebar** (Home, libraries, Live TV, Playlists, tools) + content area. | Top bar suits a 3-library server but does not scale; TV-style spine is wrong for mouse-first. |
| D7 | Theme | **Dark by default, `#141414`, one accent; a light theme and high contrast shipped after v1.** | The light theme is the same token table with a contrast audit, not a redesign. |
| D8 | Remote control / cast | **M6, after handoff works.** Sessions list + play/pause/seek on another session. | Cutting it removes the only feature that makes this more than a browser. |

---

## 1. What this app is

A single-user desktop client for a Jellyfin server on a LAN, used at a desk, with a mouse and a
keyboard, on Linux. It is fast to open, quiet at rest, and it never pretends to be a video player.

**v1 is done when:** cold start shows my Continue Watching in under 300 ms, a 20 000-item library
scrolls without a dropped frame, choosing an item starts mpv at the right position, and stopping
mpv makes the item reappear in Continue Watching at that position — with a snapshot for every
screen in the repo.

**Explicitly not v1:** music, sync-play, mobile, Windows and macOS. In-app decoding, Live TV and
the subtitle surface were added after v1; their current contract is §5 and
[`design-player.md`](design-player.md). Transcoding (P4, direct-play-first) and downloads (Q9, a
Settings storage path, queue and offline playback) are post-v1 projects tracked in
[`research/07-feature-opportunities-2026-11.md`](research/07-feature-opportunities-2026-11.md);
they amend this contract when implemented.

---

## 2. Prior art worth stealing from

Full citations in `research/01-jellyfin-clients.md`; the conclusions it supports:

**Every existing desktop client is the server's web UI in a shell with a native decoder bolted
on.** Jellyfin Media Player (now *Jellyfin Desktop*, renamed in Dec 2025 with non-migrating
settings) literally loads the server's own copy of `jellyfin-web` in Qt WebEngine and intercepts
`<video>` into libmpv. That inheritance costs two things that never go away: a rendering tax — the
oldest open issues in the repo are "Choppy UI and Scrolling" ([#1001](https://github.com/jellyfin/jellyfin-desktop/issues/1001),
[#299](https://github.com/jellyfin/jellyfin-desktop/issues/299), open since 2022) — and release
coupling, to the point where a client release had to "fix caching issues to prevent broken web
client when the server web version updates".

**The native clients that survive** (Swiftfin, Findroid, Finamp) are the ones that made the same
trade this design makes: own the UI, delegate the decode. Their recurring defects are instructive
and avoidable: session loss when platform storage is wiped ([Swiftfin #776](https://github.com/jellyfin/Swiftfin/issues/776)),
player-engine regressions, and UI cost that scales with library count — Streamyfin's UI reportedly
"crumbled" at the 17th library ([#379](https://github.com/streamyfin/streamyfin/issues/379)).

Five patterns are consistent enough to treat as requirements rather than taste:

1. **Continue Watching is the home screen.** Everything else is secondary.
2. **One primary button per detail page** whose label carries state ("Resume 42:13"), never a row
   of equally-weighted actions. On a series the state is the Next Up episode's: the label and the
   action share one `play_target`, so "Resume 18:41" opens the episode that row is showing.
3. **Series ▸ Season ▸ Episode is a hierarchy, not a list.** Season chips beat a 300-row list.
4. **Search must cancel per keystroke** — on a LAN the round trip dominates the debounce.
5. **Back restores scroll and focus**, or people stop using back.

And four anti-patterns, each with a citation in the brief: configurable home rails as a v1 feature,
mouse-only interaction (the HTPC complaint that chased Jellyfin Desktop for years), unbounded
per-library fan-out on the home screen, and a first run that shows an empty screen with no
explanation.

One finding changed this design: **`/Users/{userId}/Items/Resume` does not exist in the 12.4.0
API** — the current endpoints are `/UserViews?userId=`, `/Items?userId=`, `/UserItems/Resume`, and
`/Items/Latest`. The client is written against the current spec, and §12 records what was verified.

---

## 3. UX model

Adapted from `research/02-media-ux-patterns.md`; that file holds the requirement table (R1-R12).

**Shell.** Collapsible sidebar: Home, one entry per library (server order, pinned first), Live TV,
Watchlist, Playlists, **Discover** (only when the optional Seerr companion is configured), then
Search, Remote and Controls. **Settings is pinned below the scroll.** Every library, playlist and
tool row shares one scroll viewport with no count cap: a server with many libraries scrolls rather
than dropping entries, and the pinned Settings row keeps the way out in reach. The header carries
the wordmark from `Assets/Logo_with_name.png` (the mark alone when collapsed), decoded once through
the existing `image` crate. The server name is not repeated under it; it lives in Settings. A
library's icon comes from its collection type — film, TV, layers, antenna, list, record — not one
folder for every entry; Watchlist keeps the heart and Discover the compass. Content area owns its
own scroll.

**Home.** The page heading *is* the Continue Watching rail's heading: the rail carries no second
title, and its **See all** sits at the header's right. The server name lives in Settings, not
in a chip on the page or under the wordmark. A 360 pt card-to-transparent band lights the top of the page, so the
header has depth and the rails sit on the page's own black — no artwork needed for it. Fixed order: Continue Watching (merged) → Next Up (the server's
one-episode-per-series answer) → **one Recently Added rail for every video library** → Recently
Played → Collections. There is no rail cap: a server with many libraries gets many rails and Home
scrolls. A rail hidden with “Not interested” is removed. Empty rails are suppressed, never rendered blank. Rails are horizontal,
momentum-scrolled, snapped to card boundaries. Every card in a rail asks for its poster when the
rail is built, not when it first scrolls into view, so swiping a rail never uncovers empty boxes.

**Compact hero (optional, off by default).** Settings → Interface adds one 180 pt card above
Continue Watching with the first visible Continue Watching entry: its own backdrop, *Featured*,
the series name, `S2E04 · Overtime · 12 min left`, and **Resume** / **Details**. It is the first
focus on Home while it is on, sits above the rails in the focus order (Up from any rail reaches
Resume), and is skipped when Continue Watching is empty or its entry is hidden. It never borrows
another title's backdrop, and it never grows to a full-height billboard.

**Hover preview (optional, off by default).** Settings → Interface turns on silent trickplay
previews: after the pointer rests on a poster for 1.1 s, the card shows that item's own
trickplay frames through the middle 76 % of its runtime, one every ~750 ms, with a `PREVIEW`
badge and timestamp. It fetches one manifest and one sprite tile, keeps only the current tile,
and stops the moment the pointer leaves. There is no audio track and no second player, so a
preview cannot make sound or play the wrong stream; an item without trickplay simply does not
preview.

**Library.** Toolbar (sort, facets — Unwatched, Favourites, Genre — Filters, Select, grid/list,
letter rail when sorted by name) over a virtualized
grid, 100 items per fetch, one page prefetched. The heading carries a summary from the server's
own filtered totals: `60 items · 11 unwatched · 2 missing · 4 d 23 h`, with runtime only once
the whole page set is loaded. When the same query reports missing or unaired items, an
**Attention** chip opens a two-row panel (Missing, Unaired) with counts; a clean library shows
neither. Past 720 points of scroll a **↑ Top** pill returns to the first row through the same
scroll request as reopening the library. In name order the first visible row's letter sticks
to the top-left while its group scrolls under it; other sorts have no letters. **Filters** opens the full sheet: both facets,
every sort (Name, Added, Rating, Played, Release, Runtime, Plays, Random), the direction, genre
chips, year, minimum rating, audio/subtitle language, “has subtitles”, “4K only”, **studio**,
**tag** and, for a series library, **Continuing/Ended**. Every active filter is a removable chip
above the grid, plus **Clear all**; the Filters button counts them. **Select**
turns the cards into checkboxes with a bulk bar (Mark watched, Clear, Done); no card opens while
it is on. Capability badges (4K, HDR, CC) come from `MediaSources/MediaStreams` on the page, and an
`EXTRA` badge from `SpecialFeatureCount`, so a disc's special features are visible before the
detail page
request, so they are facts about the file, not guesses from the title.

**Watchlist.** The sidebar's Watchlist is the server's favourite flag as one cross-library page
(`filters=IsFavorite`, no `parentId`). It is deliberately not a second store: the card menu and
the detail page write the same flag, so the page is always what the server says. The label
"Watchlist" is the product word; no privacy is claimed beyond the server's own visibility. Fixed row height — poster aspect is constant, so
Taffy gets a trivial layout and the scroll can be exact. The wheel and trackpad scroll the grid
vertically; the virtual window only builds the rows the viewport can show. Collections are a filter facet, not a
folder tree.

**Home control.** A card menu on Home can hide an item for this profile (“Not interested”) and
say why the rail contains it (“Why this?”). Hidden ids are local config, never a server write;
Undo puts the card back.

**TV focus.** A controller-only test (`tv_reach_tests`) walks each screen, overlay and player
layer with direction keys and fails if any visible control is unreachable, so a missing zone is
a red test rather than a remote that dead-ends. Focus moves are quiet to Rune and carry their own
animated scroll, so a rail-to-rail move glides; Back on a detail page also returns its scroll to
the top, and a focused face is ringed rather than washed. The router's zones cover the whole page, not just the first
row: a detail page
adds collections, cast, extras and *More like this* below the episode list, and the player's gear
sheet and each panel are focus groups so a remote can enter them. Back from a panel returns to
the gear row that opened it, never to the page behind the picture. A vertical move glides the
page to the new row instead of jumping, and a settings index chip moves the selector into the
section it jumps to. On the timeline and the volume slider, focus is shown by the knob (larger,
ringed, glowing) rather than a translucent band across the picture.

**Player extras.** `,` / `.` step one frame and pause. The OSD marks an A–B loop as a coloured
range and seeks back to A at B. **Go to** accepts `mm:ss` or `h:mm:ss`, clamps to the duration and
keeps the same seek path as the timeline.

**Playback negotiation.** Every in-app start asks `POST /Items/{id}/PlaybackInfo` with a
`DeviceProfile` built from the GStreamer capabilities the player already requires. Direct play is
the default; the server's `DirectStreamUrl` is used when video/audio copying is the answer, and
`TranscodingUrl` when the reader capped the bitrate below the file or the profile cannot play it.
The OSD quality menu restarts at the current position and the stats/pre-playback labels report
`Direct play`, `Direct stream` or `Transcode` from the response, never from a guess. Start and
progress reports carry the same `PlaySessionId` and `PlayMethod`. A failed negotiation falls back
to the static stream and says so.

**Live TV.** The `livetv` view is not a library screen: it opens a channel list
(`/LiveTv/Channels`) with each channel's current program and, from `/LiveTv/Programs`, the next
one when the guide has it. Selecting a channel goes through
`/LiveStreams/Open` and plays the tuner URL in the same in-app player; the OSD replaces the
timeline and remaining time with a LIVE marker and the program name, and hides seek/skip. The
screen has a Guide mode: one 15-minute-sliced timeline row per channel over three hours, with
"Now / +30m / …" markers, the current program tinted, and pressing a row tuning that channel.
Every stop, error and end path calls `/LiveStreams/Close`, and a channel never enters Continue
Watching. In the player, `PgUp`/`PgDn` and the OSD's `⏮ CH` / `CH ⏭` change channel: the app
stops and closes the current tuner first, then opens the next one when that close answers —
the server counts tuner connections, so an open racing a close is refused. The list wraps. The
live row shows `CH 3 · Cinemax · <program> · Next: <program>` so zapping has a fixed reference
and the next programme is visible without opening the guide.

**Reminders and notifications.** The bell beside a channel's next programme stores its id,
channel and start time in the local config. `poll` checks once a second and shows an in-app
toast when the start arrives; when desktop notifications are opted in it also spawns
`notify-send`. No timer is created on the server. A missed reminder older than two minutes is
dropped instead of surprising the viewer later.

**Detail.** Full-bleed backdrop with the two-axis scrim → wordmark (server `Logo` image) or title →
one metadata line → the three scores (IMDb, TMDb, Rotten Tomatoes; see below) → a technical line
from `MediaSources` with a Details disclosure → one primary button and a Trailer action → overview
(clamped, expandable) → for series, season chips
and virtualized episode rows (with a `Play season` / `Shuffle` pair in the season header; the
active season chip is accent-filled, not a dark chip with dark text) → cast → **More like this** → collapsed technical info.

**Desktop media controls.** MPRIS2 on the session bus exposes the in-app player to the
desktop's media widget, `playerctl` and the lock screen: identity, playback status, title,
series/album, length, position, volume, rate, seek and next/previous, plus the root
`Fullscreen` property. The service runs on its own thread with a current-thread Tokio runtime
and reads an immutable snapshot; the UI thread only publishes and drains a command queue, so a
missing bus never blocks a frame. `zbus` and Tokio are already in the tree for Rune's tray and
file-dialog services, so this adds no new runtime. No session bus (headless, a bare TTY) simply
means no service.

**Scores.** TMDb is Jellyfin's `CommunityRating` and Rotten Tomatoes is `CriticRating`; both come
with the item. IMDb is not in Jellyfin's schema: the first page with an IMDb id downloads IMDb's
public `title.ratings.tsv.gz` once (8.7 MB), turns it into a sorted 10 MB index under
`~/.cache/runefin`, and answers every lookup with a binary search. No API key is required; the
optional OMDb field in Settings is only a fallback when that download fails. Missing values leave
a compact IMDb link rather than a zero.

**Trailers.** A server-hosted trailer (`/Items/{id}/LocalTrailers`) plays in the in-app player.
A provider URL (usually YouTube) is downloaded by `yt-dlp` into
`~/.cache/runefin/trailers` and then plays in the same in-app player, with reporting disabled so
it never enters Continue Watching. `yt-dlp` is an optional runtime tool: without it (or without
ffmpeg for the merge), the Trailer action falls back to opening the URL in the browser. No web
view is involved either way.
`More like this` is the server's `/Items/{id}/Similar` rail, loaded beside the page rather than
inside it: the hero paints from the item request, and a slow or empty recommendation list never
delays or empties the page. It is a horizontal rail of the same cards used everywhere else, so a
recommendation opens like any other item. Opening a card **morphs** into this
screen (§5, shared element). Next-up is computed locally: first unplayed episode after the highest
played one, skipping unaired, falling back to S1E1.

**Person.** A cast card or a search person opens `/Items?personIds=…`: portrait, name, credit
count and the same poster grid, so an actor becomes a browse path. Back restores the detail page
or search that opened it, including the detail page's own nested state.

**Search.** `Ctrl+K` global, `Ctrl+F` scoped to the open library (scope shown as a removable chip).
150 ms debounce, minimum 2 characters, and every keystroke cancels the in-flight request — this is
`rune::Latest`'s native behaviour, not extra work. Results grouped Movies · Shows · Episodes ·
People, six each, with "See all" expanding that rail into a grid in place (and "Show less"
collapsing it); the query and scope stay put.

**Back / focus.** One navigation stack of `(route, scroll_offset, focused_key)`. `Esc` pops and
restores both before the first paint. A single focus model serves mouse, keyboard and (later)
D-pad: roving focus inside containers, spatial arrows in grids, Tab between regions, one visible
focus ring. Modals are the only focus traps and always close on `Esc`.

**Window.** One resizable window; its content size and positive screen origin are remembered on
close and requested on the next launch (size clamped to a sane range). The compositor may ignore
the position, especially on Wayland; the default placement then stands. `F11` and the player's
`Fullscreen` chip ask the window manager for borderless fullscreen and back; Wayland's compositor
may grant it asynchronously.

**States.** Skeletons after a 150 ms delay, matching final geometry — never a spinner for content.
Server unreachable: persistent banner, cached screens still browsable, playback disabled with a
reason. Handoff states are explicit: *Launching mpv…* → *Playing in mpv* → *Session ended*.

---

## 4. Playback handoff (the part that makes this a client)

> **Superseded for the target platform.** This section describes the mpv-subprocess handoff that
> v1 shipped with. Wayland cannot embed a foreign window, so in-app playback means DMA-BUF import
> into Rune's own device — costed in [`design-player.md`](design-player.md). Everything below
> about reporting, resume and failure modes still holds; only the sink changes.

No decoder in-process. One code path, four phases:

1. **Resolve.** `GET /Items/{id}?userId=` → we do not fetch `MediaSources` or negotiate a device
   profile; we use `Videos/{id}/stream?static=true`, which serves the original file. mpv gets the
   token through `--http-header-fields` (the spec-clean path); `?api_key=` is the fallback for
   players that cannot set headers. M5 ships the `api_key` fallback; moving to headers is the
   next hardening step.
2. **Launch.** Spawn `mpv --no-terminal --input-ipc-server=$RUNTIME/mpv.sock --start=<pos>
   --force-window=yes <url>`, with `--fs` from a setting and the app's own window staying open,
   dimmed, showing the now-playing panel with a poster, title, elapsed/remaining, and *Stop*.
3. **Report.** `POST /Sessions/Playing` on start; `POST /Sessions/Playing/Progress` every 10 s,
   and on pause/seek/resume; `POST /Sessions/Playing/Stopped` with the final position on exit.
   `PlayMethod: DirectPlay`, `CanSeek: true`. Every report carries the same `PlaySessionId` so the
   server groups them into one session.
4. **Recover.** mpv exits by itself → read the final position from IPC before the socket dies, post
   *Stopped*, return focus to the detail page, and refresh that item's `UserData` so the resume
   label updates without a full reload.

Failure modes, all of them visible: mpv missing (`Error::user_message` → "mpv isn't installed" with
the package name), socket timeout (kill and report *Stopped* at the last known position), server
401 mid-playback (keep playing — it is a local file — and mark the session as un-reported rather
than losing the position silently).

Why mpv and not the Jellyfin web player: mpv does hardware decode, HDR, subtitles and audio
passthrough correctly today, and exposing it costs one subprocess instead of a decoder stack.
The cost is honest: the OSD is mpv's, not ours (§7, stretch).

---

## 5. Visual system

Tokens first — a single `theme.rs` in the app, not scattered literals, because Rune's own token set
(`rune::tok`) stays untouched so a theme can be swapped later.

**Palette.** `bg #141414` (near-black: pure black kills elevation), `surface #1F1F1F`,
`text #FFFFFF` (18.4:1), `muted #B3B3B3` (8.8:1), `faint #8C8C8C` (5.5:1), `accent` from the
`tok::accent` family (blue by default). Accent is never body copy. Focus is a 2 px inner ring at
0.9 white plus scale — not an outline.

**Status.** The accent means selected or actionable, never a state. Availability and request
statuses are tonal pills from `theme::status` — success (available, completed), warning
(processing, partial), danger (declined, failed, removed), info (pending, approved) — the tone at
full strength on a 14 % tint. Over artwork they become an opaque black chip with a 6 px tone dot,
so a bright poster cannot wash the label out.

**Surfaces.** A panel that floats over artwork — the player's gear, audio, chapter, queue and
subtitle panels, and the card menu — is frosted: `theme::elevation::frosted` blurs what is behind
it (`El::backdrop_blur(16)`), tints the blur with the card colour at 52 %, lays a 6 % white sheen
over the top and a 12 % hairline around the edge. The player's bottom OSD is not: it stays the
full-width gradient, because chrome that sits on the film should not read as a second window.
Rune blurs at most four elements in a frame (`MAX_BACKDROP_BLURS`), so only the open panel or the
menu is frosted.

**Quiet surfaces.** Settings groups, Live TV channels, Remote sessions, Seerr requests and the
Detail Next up card use `theme::SurfaceStyle`: a 3.5 % top sheen and a 1 px inset white edge over
the card fill, so a panel that is not floating over artwork still catches the light. Menus and
player panels rise 8–12 pt as they enter; the toast does too.

**Elevation.** A floating surface uses `theme::elevation::floating_layers`: contact 0/1/2.5 at
35 %, mid 0/6/12/−2 at 38 %, ambient 0/16/48/−6 at 45 %, and a 1 px inset white at 6 %. One wide
shadow reads as a halo; the layers give a contact edge, and Rune springs the first four, so a
pressed card tightens instead of sliding.

**Backdrops.** The detail page masks its still on two axes — `bg` at 92 % through the text column,
falling to nothing by 62 % of the width, under a vertical transparent-to-`bg` fade. Secondary
metadata must stay legible on a bright poster, not just on a dark one.

**Overlays.** The pre-play sheet, the still-watching and resume cards, the shortcuts sheet and the
toast are glass too: `theme::elevation::frosted`, or a 14 pt blur under a 68 % tint for the toast.
The scrim stays; the blur shows the page behind as colour, never as a readable second layer.

**Light.** Progress bars carry a 1 px white top line, an accent glow and a 4 pt lit cap. A poster's
hover adds a 45° sheen under its focus outline; watched and NEW badges have a 1 px rim; the LIVE dot
a red halo. The active nav item is a filled pill with a contained top highlight and a 1 px bottom shade: the
sidebar's scroll viewport clips anything that spills past its width, so an outer halo would end
on a hard edge.
Media facts live in one tray at the poster's top-right — three labels plus a count —
never three floating pills with the poster showing through their gaps, and never one blurred
element per badge (the frame budget is four). The Connect screen sits on a 10 % accent bloom, empty states on a card-coloured pool,
and placeholders on a faint top light. All of it is gradient and shadow — no artwork.

**Type.** Body 16 px, meta 13 px, detail title 32 px, hero title 40 px at 650 with −0.9 tracking.
Line height 1.4 body, 1.15 titles. All of it on Rune's own text stack, so no per-platform substitution.

**Motion ladder.** Rune animates with analytic springs (`Spring::new(duration, bounce)`), so these
map directly — bounce 0 is critically damped:

| Interaction | Duration | Bounce |
|---|---|---|
| Card hover in / out | 160 / 120 ms | 0.00 |
| Detail open (shared element) | 320 ms | 0.00 |
| Detail close | 240 ms | 0.00 |
| Page crossfade + 12 px rise | 180 ms | 0.00 |
| Rail settle | 220 ms | 0.10 |
| OSD fade in / out | 120 / 200 ms | — |

Exits are always shorter than enters. Nothing containing text gets bounce above 0.05 — a title
that overshoots is a title that shakes.

**Text over artwork.** A label that sits on the backdrop — the Home heading, rail titles, the
detail title, metadata, technical line, overview and section titles — carries a soft dark shadow
so a bright poster behind it never swallows the words. Rune has no glyph-shadow primitive, so the
shadow is a second run of the same glyphs in black at 80 %, blurred 2 pt and offset 1.5 pt down.
Text on a solid card does not get one.

**Pointer feedback.** A clickable element sets the pointer cursor; a dismiss scrim, a full-screen
backdrop and the player surface keep the arrow (the player hides the cursor while the OSD is
idle). Hover is a 10 % white wash and press 16 %, enough that a detail-page button visibly answers
the pointer on its card fill. Drag surfaces, such as the seek and volume sliders, show `Grab`.

**Layout ratios.** Poster 2:3, episode thumb 16:9, person 1:1. Card width 180 px at 1440, gap
0.10 × card width, one page gutter (`theme::gutter()`: 40 pt on desktop, 64 pt on big screen),
40 px between rails. The trailing card is always
visibly clipped so a rail *peeks* — a flush rail reads as a dead end.

**Controls.** Page actions are 36 pt high with 14–18 pt of side padding and a 9 pt radius;
compact toolbar controls are 30; the player's transport is the only 44 pt tier. A primary hugs
its label with a 150 pt minimum, never a fixed-width slab — a 210 pt button for a short label
reads heavier than the page it sits on. Every action carries a top sheen (7.5 % white, 18 % on
the primary) under its hairline and a two-layer contact shadow; the primary adds an accent glow.
Toolbar chips stay flat — a raised chip would read as a second button. In Settings a boolean is a
switch and a picker carries a chevron; one pill shape for both hides which is which.

**Interface modes.** Desktop is the default. **Big screen** runs the same point layout on a
1280×720 logical viewport — Rune's design height, which is 1.5× at 1080p and 3× at 4K — with a
64 pt safe gutter, fullscreen on launch, focus already on the first item and the pointer hidden
outside controls. It raises only the small type tokens (caption, meta, label and card title to
16 pt) so the smallest text is 24 px at 1080p; page and section titles and poster geometry stay
at the desktop values. The saved choice is
`ui_mode`; `--big-screen` and `RUNEFIN_UI=big` override it for one launch, and a change applies at
restart. Both modes share every screen and every focus group, so a bug fixed in one is fixed in
both.

**Controller.** A gamepad is a first-class input in both modes; Big Screen is the mode where it
is the only one. A focused card takes its pointer-hover treatment — the rounded white poster
outline, the play/remove affordance and a soft halo — with no scale, no fill and no ring; the
halo is painted at the window root, outside any rail scroll clip, so all four sides survive. Other
controls take the pointer's `t.hover` wash. Focus reads like hover rather than a selection box. Return activation comes from `set_return_activates`, so a CEC
remote's Select/OK reaches the same control. Desktop keeps its accent ring. The light theme uses a light text
halo over artwork, and the selected nav pill uses dark text on the blue fill. D-pad and the left
stick move focus (400 ms then 80 ms repeats), A is Space —
activate a card, play/pause in the player — B and Back are Escape, Start is the context key (a
card's menu; the player's gear), and LB/RB are Shift+Tab/Tab between focus groups. On big-screen
screens the app owns the four directions (`tv_focus` zones with clamped edges and a sidebar
round-trip), so Up/Down change lanes and leave a rail; a focused panel or menu keeps Rune's own
directions. The mapping is
the keyboard's, not a second interaction model, so every shortcut, focus ring and layer behaves
identically with either device. A big-screen launch focuses the first item of each route, and an
opened layer (card menu, gear, subtitle/audio list, chapters, queue) focuses its first control, so
a fresh screen is one press from playing. On the Steam Machine Runefin is added as a non-Steam game
with `--big-screen`; the physical pad reaches the native listener without a Steam Input layout, and
Steam+X still opens the on-screen keyboard for Search and Settings.

**Images.** Geometry first: reserve the box from the aspect ratio, paint a blurred 27×17
placeholder, crossfade over 200 ms. Rune's `ImageCache` decodes on its own worker and dedupes by
content, but `ImageCache::url` is web-only — on Linux we fetch bytes ourselves (below). The image
worker bakes the placeholder with the `image` crate Rune already uses (PNG/JPEG), so the app adds
no second decoder or async runtime.

---

## 6. Architecture

```
crates/runefin
  main.rs        AppSpec: setup(waker), poll(state, clock, Tx) -> bool
  state.rs       App state + Msg enum + the navigation stack
  state/seerr_state.rs  Seerr Settings buffers + connection probe
  net.rs         every call that can block: Jellyfin/Seerr requests, image fetch, mpv spawn/IPC
  image.rs       fetch → disk cache → ImageCache::load(bytes)
  image_subtitles.rs  ffmpeg extraction + libbitsub-core PGS/VobSub decode → RGBA bitmaps
  trailer.rs     optional yt-dlp download → cached MP4 for in-app trailers
  imdb.rs        IMDb's public ratings dataset → sorted index → binary search
  ratings.rs     optional OMDb fallback for IMDb
  screens/       connect, home, library, detail, search, settings, now_playing
  theme.rs       the token table from §5
crates/jellyfin-api
  the blocking client that net.rs calls (already written and tested)
crates/seerr-api
  blocking Seerr/Jellyseerr v1 client: catalogue, requests and download rows
```

**DMA-BUF extension.** The Rune fork vendors `wgpu-external-frame` 0.4 with two extra
single-plane formats, R8/R8G8 and R16/R16G16, so NV12/P010 luma and chroma can be imported into
separate textures.
It stays in the fork because it is only reachable through Rune's `NativeSurface` seam.

**Threading.** No async runtime. Rune gives two primitives and both are enough:
`rune::Latest<T>` for request/response work (built-in debounce, cancellation, one wake per answer)
and the `setup` waker for long-lived workers (mpv IPC reader, image fetcher). A handler never
blocks; `poll` drains finished work and returns `true` only when something visible changed.

**State.** One `State` struct with a `Msg` queue; screens are pure functions `(&State, &mut Cx) ->
El`. Navigation is a `Vec<Route>` of `(route, scroll, focus_key)`. No global singletons, no
observable graph — Rune's model is "re-run the view between frames, retarget property tracks", and
that is the whole state story.

**Persistence.**
- `~/.config/runefin/config.toml` — server URL, user id, token (0600), device id, player args,
  grid density.
- `~/.cache/runefin/images/<item>-<kind>-<tag>-<width>.webp` — keyed by the server's immutable
  image tag, which is why nothing ever goes stale and scroll is instant on a second visit.
- Nothing else. No catalogue DB in v1 (D4).

**Networking.** `jellyfin-api` is blocking and dependency-light (ureq + serde, 42 tests).
`seerr-api` is the same shape for the optional Seerr/Jellyseerr service: it reuses the workspace's
`ureq` and `serde`, adds no runtime, web view or render stack, and is only instantiated when the
reader configures a URL and key. One
`ureq::Agent` rides in the client and every clone shares its connection pool, so a screenful of
posters pays one TLS handshake instead of one per image; image fetches use the auth header or
`api_key`. Measured effect: home first paint 176 → 104 ms, detail 71 → 54 ms, search 83 → 66 ms. The app uses `serde_json`, already in
the workspace through `jellyfin-api`, for mpv's JSON IPC in `handoff.rs`. The in-app player uses
the `gstreamer`, `gstreamer-app`, `gstreamer-video` and `gstreamer-allocators` 0.25 crates over
the system GStreamer; see [`design-player.md`](design-player.md) §3.3. Bitmap subtitles use
`libbitsub-core` (pure Rust PGS/VobSub parsers; DVB is transcoded by ffmpeg into DVD packets) and `flate2` (the IMDb dataset is gzipped);
the image track itself is extracted with the system `ffmpeg`. Both additions are listed here
because rule 5 asks for a line, and neither pulls an async runtime, a web view or a second
render stack.

**Controller input.** `gilrs` 0.11 (`crates/runefin/src/pad.rs`) is the one input dependency: a
worker thread polls it and injects the same `KeyEvent`s the keyboard sends through Rune's
`AppHandle::key`, so focus, shortcuts and the player keep one input path and the UI thread never
blocks on a device. On Linux it builds against the system `libudev`. `RUNEFIN_GAMEPAD=0` disables
the listener for a launch; a missing pad is not an error, the thread idles and picks one up later.
It adds no async runtime, web view or second render stack.

---

## 7. Designed-for extensions

The paths below were designed as extensions; the first four shipped, and §7.2 records the optional
Seerr companion, which is in progress. The paragraphs stay as the rationale for their seams.

- **In-app player (D1 alternative).** Shipped: `now_playing` is a screen, not a modal, and the
  in-app player imports DMA-BUF frames into Rune; `Handoff` still keeps mpv as the fallback.
- **Cast / remote control.** Shipped: a `/Sessions` target picker sends
  `POST /Sessions/{id}/Playing` with the same reporting path and a different sink.
- **Subtitles, HDR, audio tracks.** The in-app player owns all three (see
  `docs/design-player.md`), including PGS/VobSub bitmap rendering and the full subtitle
  appearance set. Settings holds the defaults; they are written to the config on exit with
  subtitle size, delay, typography, position and opacity. DVB shares the bitmap path: ffmpeg
  decodes it and writes DVD subtitle packets, which the VobSub parser reads.
- **Text size and shortcuts.** Settings scales every type token from 85 % to 130 % without
  changing card geometry. Eight player actions (play/pause, mute, subtitles, skip,
  next/previous episode, fit, fullscreen) are remappable; navigation keys stay fixed and a
  duplicate binding is refused. Settings also holds the desktop-notification opt-in.

### 7.1 Connect and LAN discovery

The Server field's caption carries `Find my server`: a UDP broadcast to port 7359 with the
payload `who is JellyfinServer?`. Each reply is a small JSON document (`Address`, `Id`, `Name`,
`EndpointAddress`); the screen lists them by name with their address, and a click fills the field.
`std::net::UdpSocket` plus `serde_json` is the whole dependency budget. An empty answer means a
quiet subnet or a server with discovery disabled, and is shown as "0 found" rather than an error.
Broadcasts are blocked on some networks (this box cannot reach the server's subnet directly), so
the protocol is tested against a loopback responder and the live probe is a diagnostic.

### 7.2 Optional Seerr companion (shipped 2026-11-05)

Seerr (the merged Overseerr/Jellyseerr) is an optional second HTTP service, not a second media
server. `crates/seerr-api` is a blocking client; `config.toml` holds `seerr_url`,
`seerr_api_key` (0600) and an optional `seerr_user_id`. With no URL/key the app is unchanged: no
nav entry, no polling, no request. Settings proves the connection; **Discover** searches the
catalogue, a film or a set of seasons can be requested, and **Requests** shows status and
byte-accurate progress. Titles/posters come from a cached `/movie` or `/tv` summary because
`/request` has neither; a season pack is one download with many episode rows and is counted once by
`downloadId`. Polling is 5 s only while the Requests tab is visible. A Seerr title also carries
genres, a tagline and its billed cast; a face opens the person page. The server and Seerr are
joined on TMDB ids: a Jellyfin person item exposes `ProviderIds.Tmdb`, Seerr exposes
`/person/{id}` and `/person/{id}/combined_credits`, and the person page shows the library titles
first and the rest of the filmography as requestable Seerr cards. The contract and the live
measurements are in [`research/08-seerr-client.md`](research/08-seerr-client.md) and the plan is
[`plans/2026-11-05-seerr-requests-plan.md`](plans/2026-11-05-seerr-requests-plan.md).

---

## 8. Implementation order

Each milestone ends with a snapshot scene, a scripted-input test, and a commit someone can review.

| M | Scope | Done when |
|---|---|---|
| M0 | Scaffold | `cargo test` green, app opens, `scripts/snapshots.sh` writes PNGs |
| M1 | Connect: server probe, password + Quick Connect, saved servers, config file | Bad URL, bad password, unreachable server each render a distinct state; token survives restart |
| M2 | Home: CW + Recently Added rails, rails scroll with momentum, skeleton states | 300 ms to first paint from cache; empty and error states snapshot-tested |
| M3 | Library: virtualized grid, sort/facets, letter rail, back restores scroll | 20 000-item fixture scrolls with no frame over 16 ms |
| M4 | Detail: shared-element open, season chips, episode rows, resume label | Card→detail→Esc round-trips to the same card and offset |
| M5 | Handoff: mpv spawn + IPC, progress reporting, now-playing screen | Play → stop at 10:00 → item shows 10:00 in Continue Watching after refresh |
| M6 | Search, cast/remote, settings | Ctrl+K from anywhere; a second session can be paused from here |
| M7 | downloads/offline (Q9); music remains out of scope | after P4, if real use asks for it |
| M8 | Optional Seerr companion (S0–S3), shipped 2026-11-05 | No Seerr URL: app unchanged. Configured: probe succeeds; a film/season can be requested; Requests shows titles, statuses and progress (or "Waiting for a download" with sync off) |

A playable item's detail page has a `Playlist` action, and the item menu has the same entry, so
adding does not require opening the page first. Both open a picker of the playlists the sidebar
already loaded and append the item with `POST /Playlists/{id}/Items`. A short toast over the page
confirms the add or names the failure, since the result can happen far from the detail page.
`Play on…` lists the same user's other sessions (never this window) and sends
`POST /Sessions/{id}/Playing?playCommand=PlayNow&itemIds=…`; the command travels as a query, not a
JSON body — the 10.11 controller binds it from the URL and answers 400 otherwise. The picker
never waits on a request to open; only the add does. The picker's last row opens a
small prompt: the item on the page becomes the first entry, the name field takes focus, Enter
or `Create` sends `POST /Playlists`, and the new playlist appears in the sidebar at once. The
picker still says so when the server has none.

Playlists are a sidebar section and a page. A playlist opens in the same virtualized grid as a
library, but with no sorts, filters or genres: its order is the playlist, so only the view
toggle, `Edit` and `Play all`/`Shuffle` remain. `Edit` shows a `✕` and `↑`/`↓` on every entry; `✕` removes
that entry by its `PlaylistItemId` (not the item id — a film can appear twice) and the page
re-reads from the server, so the count and the grid always agree with it. `↑`/`↓` reorder
optimistically and call `POST /Playlists/{id}/Items/{entry}/Move/{index}`; the first entry has no
`↑` and the last no `↓`, and a refused move reloads the page so the order is never wrong for long.
Dragging a poster moves the entry by whole rows and uses the same rotate-then-Move path, so the
entries between the old and new positions keep their order. Entries come from `/Playlists/{id}/Items`, not
`/Items?parentId=`, because playlist entries are not children of the playlist. The sidebar
loads with the home rails (`/Items?includeItemTypes=Playlist`), and a failure there is an empty
section rather than a broken home.

Collections are server-owned box sets. A box-set detail page lists its members, reports watched
progress, can queue them, and sorts the visible grid by server order, name, year or runtime
without rewriting the server order. A card, the detail action row or the library bulk bar opens
an "Add to collection" picker; the write is `POST /Collections/{id}/Items?ids=…`. Creating a
collection uses `POST /Collections?name=…&ids=…` (Jellyfin 12.4 takes query parameters; the old
JSON body answers 500). Image covers are not exposed because the server returns 500 to image
uploads.

Right-clicking (or the Menu key / Shift+F10) on any poster opens an in-app item menu: Open,
Play or Resume, Start over, Play next while something plays, Add to playlist…, Play on…,
Favourite, Like, Dislike, Mark watched, Remove from Continue Watching (clears the resume position
without marking it watched), and Go to series for an episode. Removing a playlist entry or moving
it shows an Undo toast; the Iist updates before the server answers and Undo reverses both the page
and the server call. A watch-state change applies to every in-memory copy of the item at once,
and marking something watched removes its card from Continue Watching immediately; the server is
told in the background, unlike the favicon-stale approach of waiting for a reload.
It is drawn by the app rather than by the platform (Rune's native menus are macOS-only), closes
on a click anywhere else or Esc, and is clipped to the window so a card at the edge still shows
the whole menu.

Rows of controls wrap instead of clipping when the window is narrow: the detail actions and the
library toolbar each take a second line at 800 points, and the player's timeline row wraps once
while its three centred controls stay put. The narrow
snapshots (`detail-narrow`, `library-narrow`, `in-app-narrow-small`) keep that honest.

Hovering a playable poster shows a play button in its lower-right corner; it starts the item
where it left off without opening the detail page. Containers (a Series) keep the plain card,
since they have no stream of their own. Playlist edit controls live on the opposite corner, so
the two never collide.

Finished items are visible at a glance: a played fill (or a series with nothing unplayed) puts a
small check badge on the poster's corner, and the list row adds `Watched` to its facts. The
20 000-item fixture used by the frame-budget test stays without per-item watch state on purpose.

The library toolbar's sort chips carry a direction arrow (`↑ Asc` / `↓ Desc`) that re-reads
from page one: the server's order is the only order, so reversing the loaded page would be
wrong past page one. Every sort starts from its natural direction (A–Z, newest, best rated).
A resumed item's detail page shows `Start over` beside `Resume`, so going back to zero is one
click instead of a seek. A series shows it when its Next Up episode is in progress, because that
is the episode `Resume` opens.

A TV library lists series, which cannot be streamed as one item, so its header offers
`Shuffle episodes` instead of `Play all`: the server randomises `includeItemTypes=Episode` under
the library (`sortBy=Random`, 100 entries), because shuffling only the loaded page would not be a
shuffle. A watched episode row also carries the same check as a finished card.

The library header also carries `Play all` and `Shuffle`. Both cover the whole library, not the
first page: when the grid has not reached the end, the run is built from one wide request with
the same filters and order the grid shows (capped at 5 000 items). A collection page queues its
members. A queue
continues into its next item at EOS, and `N`/`P` step through it from the OSD.

M1-M4 are the experiment: they are pure Rune, pure design, no OS integration. M5 is the one that
makes it a Jellyfin client rather than a pretty browser.

---

## 9. How it gets tested (the agent loop)

- `cargo test -p jellyfin-api`: parsing and URL contract, no network (42 today).
- `cargo test -p seerr-api`: the optional companion's wire types and URL/error contract, no network
  (22 today); its `tests/live.rs` is read-only and opt-in with `SEERR_URL`/`SEERR_API_KEY`.
- `cargo test -p runefin`: `Runtime` drives real input through real layout and springs at a fixed
  step; assert on state, `rt.find(key)`, `rt.presented(key)` — never on colours.
- `scripts/snapshots.sh`: the committed views to `docs/snapshots/*.png` at a fixed virtual
  time; `--all` renders every `AppSpec::scene` into a local directory instead.
  A UI change is a PNG diff in the commit. `--live` opts a headless `--snap` out of the hermetic
  fixture and renders the session on disk instead, for personal screenshots of a real library;
  committed snapshots never use it.
- `--script` for motion: a JSON script of moves/taps at timestamps, rendered as a frame strip, so
  "the rail settles on a card" is a picture, not a claim.
- Later, against a real server: a `jellyfin-api` integration test behind an ignored-by-default
  feature flag, pointed at a dev container.

This is the part of Rune worth the risk: the framework was built so an agent can *show* the
result rather than describe it. Every milestone above is expressible as a PNG and a test.

---

## 10. Risks

1. **Rune's Linux window layer is unproven.** M2's own roadmap lists Windows/Linux as "partly";
   442 macOS `cfg` sites against 21 Linux ones upstream. *Kill criterion:* if window creation,
   HiDPI or fractional scaling misbehaves on the real desktop in M1, stop and reassess before
   writing screens.
2. **Upstream churn.** ~1 400 commits in ten days, one primary author, no releases, literal-heavy
   API. Mitigation: pinned rev, standalone bump commits, snapshots as the regression net.
3. **mpv IPC is a second process to babysit.** Crash recovery, orphaned sockets, a user who
   closes mpv holding the item. Mitigation: one `Handoff` state machine with explicit states and
   a `Stopped` report on every exit path, tested with a fake player binary first.
4. **The design outruns the framework.** Shared-element morphs, momentum rails and blur-up
   placeholders are all custom work in Rune — no CSS transitions to lean on. If the morph alone
   eats a week, cut it and crossfade (the app still reads as premium).
5. **Scope creep into "real client".** Every extra surface (music, Live TV, downloads) re-opens
   the decoder question this design exists to avoid.

---

## 11. Open questions I'd rather you answer than assume

1. Which server do we point the integration tests at — a throwaway container on the Proxmox box,
   or a real library with a read-only user?
2. Keyboard-first or mouse-first defaults? It changes focus ring prominence and whether arrows
   move focus or scroll.
3. Is "one app that does browse + handoff" acceptable, or do you want the player in-app badly
   enough to make that the project (it is a different project, §10.1)?
4. Do you want the Jellyfin brand skin (red, web-like) or a distinct look of its own? Both are a
   token table; the second is more fun and less "another Jellyfin client".

---

## 12. Appendix — what was verified against the server

Checked against the stable OpenAPI document, `info.version` **12.4.0**
([spec](https://api.jellyfin.org/openapi/jellyfin-openapi-stable.json)), not against blog posts.
This is what `crates/jellyfin-api` implements, and 42 tests pin the shapes.

**Live check, 2026-11-02 (historical, Jellyfin 10.11.2):** a real server was exercised through
the ignored `crates/jellyfin-api/tests/live.rs`: 7 libraries, 11 Resume rows with
`SeriesPrimaryImageTag` and `UserData`, Recently Added, a paged library, seasons/episodes and an
image fetch all matched the shapes below. The M5 playback test also ran against mpv and the
server: a 60 s stop at 59,497 ms came back as 59,497 ms in Continue Watching.

**Live check, 2026-11-05 (Jellyfin 12.4.0):** the upgraded server has 8 libraries, 14 Resume
rows, 12 Next Up rows, 41 box sets and 4 live channels. All three ignored tests pass: direct
play stays direct, a 2 Mbps cap returns a fetchable HLS playlist, and a collection can be
created, added to, had a member removed and deleted. The only contract change found was
`POST /Collections`, which now takes `name` and `ids` as query parameters.

| Concern | Endpoint as implemented | Notes |
|---|---|---|
| Server probe | `GET /System/Info/Public` | no auth; also tells us the server version to warn about too-old servers |
| Password auth | `POST /Users/AuthenticateByName` `{Username,Pw}` | returns `User`, `AccessToken`, `ServerId` |
| Quick Connect | `POST /QuickConnect/Initiate` → poll `GET /QuickConnect/Connect?secret=` → `POST /Users/AuthenticateWithQuickConnect {Secret}` | **`QuickConnectResult` carries no token.** The token only arrives from the third call — a detail most examples get wrong |
| Libraries | `GET /UserViews?userId=` | returns each library's `Id` and `CollectionType` |
| Browse | `GET /Items?userId=&parentId=&includeItemTypes=&recursive=true&sortBy=&startIndex=&limit=&fields=UserData` | `fields=UserData` is mandatory: without it watch state is silently omitted and no poster shows progress |
| Item | `GET /Items/{itemId}?userId=` | full item incl. `UserData`; there is no `fields` param here |
| Continue Watching | `GET /UserItems/Resume?userId=&limit=&mediaTypes=Video&fields=UserData` | the old `/Users/{userId}/Items/Resume` is **not** in 12.4.0 |
| Recently added | `GET /Items/Latest?userId=&parentId=&limit=&fields=UserData` | |
| Images | `GET /Items/{itemId}/Images/{type}?maxWidth=&quality=&tag=` | keep `tag` in the URL: it is the cache key that never goes stale |
| Direct play | `GET /Videos/{itemId}/stream?static=true` | mpv gets the token via `--http-header-fields` (spec-clean); `?api_key=` also works and is the fallback |
| Playback start | `POST /Sessions/Playing` — `PlaybackStartInfo` | `PlayMethod` is `DirectPlay`, `DirectStream` or `Transcode` from the negotiated source |
| Progress | `POST /Sessions/Playing/Progress` — `PlaybackProgressInfo` | every 10 s plus every state change |
| Stop | `POST /Sessions/Playing/Stopped` — `PlaybackStopInfo` | **different payload**: no `IsPaused`/`CanSeek`; final `PositionTicks` is what keeps resume truthful |
| Cast / remote | `GET /Sessions`, `POST /Sessions/{sessionId}/Playing` | M6 |
| Collections | `POST /Collections?name=&ids=`, `POST`/`DELETE /Collections/{id}/Items?ids=` | Jellyfin 12.4 takes creation as query parameters, not a JSON body |
| Playlists | `GET /Playlists/{id}/Items`, `POST /Playlists`, `POST /Playlists/{id}/Items?ids=`, `POST …/Move/{index}`, `DELETE …?entryIds=` | entries are not children, so `parentId=` is wrong |

Two shapes that bit this scaffold and are now pinned by tests: the three playback calls take
three different payload types, and `/Items` returns nothing useful without `fields=UserData`.
