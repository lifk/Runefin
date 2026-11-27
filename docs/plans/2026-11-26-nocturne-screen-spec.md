# Nocturne screen set, navigation and motion

Status: evaluation, 2026-11-26 · follows [`2026-11-26-redesign-epic.md`](2026-11-26-redesign-epic.md)
At scale: [`2026-11-26-nocturne-scale-analysis.md`](2026-11-26-nocturne-scale-analysis.md) — eleven
stress captures, the cardinality matrix and the six-rail Home budget.
Execution: [`2026-11-26-nocturne-implementation-plan.md`](2026-11-26-nocturne-implementation-plan.md).
Mockups: 15 views captured at 1280×720 from the local Nocturne set (film artwork not committed;
the captures are the evaluation artifacts). The Steam Machine was not used.

---

## 1. The screen set

| View | Purpose | First focus | Key geometry at 1280×720 |
|---|---|---|---|
| Home | Continue and discover | first Continue Watching card | 50 % backdrop hero; libraries strip; 6 × 16:9; 6 × 2:3 |
| Home · rail focus | nav state | current destination | 76 px rail expands to 284 px over the dimmed page |
| Libraries hub | every library, playlist and tool | first Recent tile | Recent 3; all libraries 3 × 3; Tools 4 |
| Library · grid | browse a library | first poster | 6 columns; A-Z in the right gutter; one toolbar row |
| Library · list | same data, row form | first row | 76 px rows; 132 × 74 still; resume line |
| Detail · film | decide and start | Resume | 66 % backdrop; title over it; cast; related below |
| Detail · series | seasons and episodes | Play S1E1 | same hero; season tabs; 4 episode rows |
| Search | find | the field | 52 px field; recent; Films/Series poster rows |
| Live TV · guide | time-based channels | the live programme | 200 px channel column + 5 time columns; preview panel |
| Player · OSD | watch | play/pause | bottom gradient only; scrubber, chapter ticks, tracks |
| Downloads | offline copies | first row | storage summary; rows with poster/size/action |
| Remote | control another session | first session | rows; Play/Stop on idle, Pause/Stop on playing |
| Settings | configure | first section | two-pane; section list + rows with segmented/toggles |
| Person | cast biography | Back / filmography | portrait, bio, 6-column filmography |
| Playlist | ordered entries | Play all | rows with drag handle; Play all / Shuffle / Edit |

The mockups apply the tokens in the epic: graphite `#0A0C0F` / `#12161B` / `#191D23`, hairlines
at 8 %, text `#F4F6F8`, muted `#98A2AC`, cyan `#38BDF8` only for progress, focus and one action,
8/10/12 px radii, the type table in the implementation plan (15/19/20/36/46) and the 2× image
scale.

## 2. Navigation model

**Recommended:** a 76 px icon rail at rest with five destinations — **Home, Search, Libraries,
Live TV, Settings** — and a **Libraries hub** one level down. The hub opens on a **Recent** row
(the last three libraries) followed by every library and a Tools row (Downloads, Remote,
Controls) and the user's playlists. At rest the rail shows icons; moving into it (Left) expands
it to 284 px as an overlay with labels and the same five anchors, so it never pushes the page.
The library flyout (Recent + all) can replace the hub for a reader who wants the old direct
list; the expanded full list is an optional mode, not the default (counts below).

- The rail's current destination is focused when the reader enters it; Left from content goes
  there, so Home→Home is one press.
- Enter on a library is one press; Back from a library opened from the hub returns to the hub
  with the tile focused; Back again goes Home. From Home, Back leaves the top-level page to the
  rail.
- Search is the second anchor, not the twelfth: it is the most used utility and the old sidebar
  put it after every library. Settings is the last anchor; Downloads and Remote live in the hub's
  Tools and Settings, not beside every library.
- A screen's first focus is the primary action: Resume on Detail, the field on Search, the live
  programme on the guide, Play all on a playlist, the first row on Downloads/Remote.

### 2.1 Interaction counts

One D-pad press or activation is one interaction. The start is Home with the Continue Watching
card focused. "Current" is the 224 px sidebar (Left to it, Down to the row, Enter); "5 + hub" is
the recommended rail; "Expanded" is the full rail list shown in the nav-focus mockup.

| Destination | Current | 5 + hub | Expanded | Path in the recommended model |
|---|---:|---:|---:|---|
| Search | 13 | **3** | 11 | Left, Down (Search), Enter |
| Settings | 17 | **6** | 14 | Left, Down ×4, Enter |
| Live TV | 6 | **5** | 8 | Left, Down ×3, Enter |
| Movies (last used) | 7 | **5** | 3 | Left, Down ×2 (Libraries), Enter, Enter (Recent tile) |
| Anime (not recent) | 3 | 7 | 5 | rail→hub→all grid (3×3) |
| Downloads | 15 | **8** | 13 | rail→hub→Tools→Downloads |
| Remote | 14 | **8** | 13 | rail→hub→Tools→Remote |
| Playlists | 9 | **7** | 5 | rail→hub→Playlists tile |
| Detail from Home | 2 | 2 | 2 | Down to the poster rail, Enter |
| Player from Home | **1** | **1** | **1** | Enter on Continue Watching |
| Player from Detail | 2 | 2 | 2 | Detail, Enter on Resume |
| Person | 4 | 4 | 4 | Detail, Down to Cast, Enter |
| Live guide tab | 7 | **6** | 9 | Live TV, Right to Guide |

The trade is explicit: the rail + hub is the best model for the utilities (Search 13→3, Settings
17→6, Downloads/Remote 14–15→8) and for a **recent** library (7→5), while a non-recent library
costs 1–4 more than the direct sidebar. The **expanded rail** is the opposite: the last library
is 3 and Anime 5, but Search/Settings/Downloads are 11–14. The recommendation is the 5 + hub with
the **Recent** row; if library switching dominates, enable the Libraries flyout (Recent + all)
so the recent library is 4 and the non-recent 6–8 without paying 10+ for Search.

**Reachability contract:** every destination from Home is ≤8 interactions (≤3 for Search, Home,
the last library and the player); every top-level page is one Back from its parent; focus is
restored to the tile/card that opened a page; no focus ever lands on a hidden row (the current
sidebar's filtered-library bug is the example). The five anchors and the hub always expose every
server library, the user's playlists, Downloads, Remote, Controls and Settings.

## 3. Motion

The motion ladder is calm and short: a TV reader sees the focus move, not a show. Rune springs
with zero bounce, except the shared detail transition at 0.05. The scene-by-scene entry, focus,
internal and Back choreography — including what does **not** animate — is in
[`2026-11-26-nocturne-motion-spec.md`](2026-11-26-nocturne-motion-spec.md); this table is the
summary.

| Transition | Time | Spring | What moves | Reduce Motion |
|---|---|---|---|---|
| Focus within a row | 160 ms | HOVER (0.16, 0) | card scale 1.05, 2 px light edge, lift shadow; neighbours still | 80 ms ring only |
| Enter/leave the rail | 200 / 180 ms | RAIL (0.20, 0) | width 76↔284, labels 120 ms, page dim 160 ms; overlay, no layout shift | instant, labels on |
| Scroll to focused card | 220 ms | GLIDE (0.22, 0.05) | the row/page scroll; 48 px lead before the card | instant |
| Page open (hub/library/search) | 180 ms | PAGE (0.18, 0) | old page 120 ms out; new page fade + 8 px rise | 80 ms fade |
| Detail open | 260 ms | DETAIL (0.26, 0.05) | card art to hero/poster (matched); content 180 ms; backdrop 200 ms | 100 ms fade, no flight |
| Detail Back | 220 ms | DETAIL reverse | hero back to the card; restored focus | 80 ms |
| Hub tile → library | 200 ms | PAGE | tile art to the library header, else fade | 80 ms |
| Season tab | 120 ms | CONTROL (0.18, 0) | episode list crossfade + 4 px shift | instant |
| OSD show/hide | 120 ms | OSD (0.12, 0) | opacity + 8 px; video dims 200 ms; auto-hide after 3 s | 80 ms |
| Menu / keyboard / shortcuts | 160 ms | CONTROL | scale 0.98→1, fade, scrim 160 ms | instant |
| Image arrival | 200 ms | tween ease-out | blurred placeholder → image | 80 ms |
| Toast | 160 ms | CONTROL | 10 px slide + fade | instant |

- Every transition starts on the same frame as the input; a second press retargets the spring
  (focus, scroll) or reverses the page (Back mid-open) rather than queueing a second animation.
- Direction carries meaning: the page rises from the row that opened it; the detail uses the
  card art as the shared element; Back reverses the same path. No full-screen slide on every
  navigation.
- The OSD fades, never slides the video; controls auto-hide; Up opens chapters/tracks; the first
  input while hidden only shows the OSD.
- Reduced Motion is a real setting: no scale, no flights, no spring; 80–100 ms fades and instant
  scroll. The mockup set's layout is unchanged with it.
- Add `RAIL`, `PAGE`, `DETAIL` and `OSD` to `theme::motion` next to the existing `HOVER` and
  `CONTROL`; one spring per role, no ad-hoc numbers at call sites.

## 4. Per-screen focus and notes

- **Home:** Continue Watching card → its row → the Next Up row → the rail (Left). Up from the
  first card reaches the hero actions (Resume, More info). The hero is 50 % with the libraries
  strip, so the strip and the first rail are visible without scrolling. See all opens the shelf.
- **Hub:** first Recent tile → the rest of Recent → all libraries (3 × 3) → Tools → playlists.
  Enter opens; Back returns the tile focused. The hub is the only place every library is listed.
- **Library grid:** first poster → row → column; Up reaches the toolbar (search, sort, filters);
  Right at the last column reaches the A-Z overlay; Left at the first column reaches the rail.
  The A-Z is an overlay in the right gutter, never a layout sibling that pushes the grid.
- **Library list:** the same zones; Down moves a row; the row's trailing action is the resume.
- **Detail film:** Resume → Start over → Trailer → favourite/overflow; Down to Cast → related.
  The backdrop is the hero; a poster appears only when the server has one. For an item with only
  a 16:9 still the hero is the still, not a cropped 2:3 poster.
- **Detail series:** Play S1E1 → Resume → season tabs → episodes → cast → related. The season
  tabs are a focus row; the episode list crossfades.
- **Search:** the field takes focus; Enter/Menu opens the keyboard; Down to recent → results;
  the results grid uses the same 6-up poster rhythm. No auto-open on entry.
- **Live guide:** the first live programme is focused; Left/Right move in the timeline; Down
  moves channels; Enter opens the preview panel's Watch; the preview's More info is one Right.
  The guide fills the width; the bottom preview removes the old dead middle.
- **Player:** play/pause focused; Left/Right ±10 s; Up the chapter/tracks sheet; Down the
  scrubber; Back hides the OSD, then leaves. Auto-hide 3 s.
- **Downloads:** first row; Play/Pause/Remove; the storage summary is a read-only card; the
  folder path lives in Settings.
- **Remote:** first session; the transport is the row's actions; idle sessions show Play/Stop,
  playing ones Pause/Stop; Refresh is a toolbar action.
- **Settings:** the section list is the left zone; rows are right; the segmented controls and
  switches are their own focusables; Back to Home.
- **Person:** Back / filmography; the filmography is a 6-column grid; the portrait is the
  server's, initials are the fallback.
- **Playlist:** Play all / Shuffle / Edit → rows; Edit exposes the drag handles and Remove; Back
  returns to the hub/playlist list.

## 5. Issues the mockups surfaced

- **The 16:9 fallback crop** is worse on a 2:3 card than on the detail hero: a film with only a
  Thumb (28 Days Later) crops its own title. The detail mockup uses the still as the hero, not a
  poster. Phase 4/5 should letterbox/blur the fallback instead of a hard crop.
- **The expanded full rail loses the utilities** (Search 11, Settings 14). Keep it as an optional
  library-flyout mode, not the default.
- **The hub's all-libraries grid** needs a stable order (usage first, then server order) and a
  Recent row; otherwise a non-recent library is slower than the old sidebar.
- **Live TV** was a narrow column with a dead middle; the guide + preview fixes it, but the
  channel count must not leave the timeline half empty (the 4-channel server does).
- **The player OSD** is legible over the red frame; chapter ticks and a dimmed video are enough;
  no permanent chrome.
- **Empty/loading/error** are not separate mockups yet; the epic's states (skeleton, one-line
  empty with one action, retry) apply to every screen.
- **Seerr** is absent from this set because the test account has no Seerr URL; when present it
  is a hub entry and a Discover rail, not a sixth permanent icon.

## 6. Implementation mapping

Phase 1 (tokens/type) and Phase 2 (rail/hub) enable every screen here. Phase 3 Home, Phase 4
Library/A-Z, Phase 5 Detail, Phase 6 OSD, Phase 7 Live/Search/Downloads/Remote/Settings/Person/
Playlist. The motion tokens and the interaction contract belong in Phase 2/8. The screen
captures are the visual target; the epic remains the phase plan.
