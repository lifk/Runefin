# Nocturne implementation plan

Status: proposed · 2026-11-26 · execution companion to the redesign epic.
Inputs: [`2026-11-26-redesign-epic.md`](2026-11-26-redesign-epic.md) (why),
[`2026-11-26-nocturne-screen-spec.md`](2026-11-26-nocturne-screen-spec.md) (what),
[`2026-11-26-nocturne-scale-analysis.md`](2026-11-26-nocturne-scale-analysis.md) (limits),
[`2026-11-26-nocturne-motion-spec.md`](2026-11-26-nocturne-motion-spec.md) (every scene's
animation), and the 15 screen captures, 11 stress captures and the Home rails chooser.

Scope: `crates/runefin`, its tests, scenes and `docs/`; `jellyfin-api` only where a type already
exists; no new dependency, no second render stack, no Rune fork work unless a phase is blocked.

---

## 1. Contract locked by this plan

Where the epic and the captures disagree, this section wins. Phase 0 amends the screen spec and
`docs/design.md` to match it.

### 1.1 Geometry at the 1280×720 design

| Element | Now | This plan |
|---|---|---|
| Shell | 224 pt sidebar, collapsible | 76 pt rail; 284 pt overlay when focused |
| Content width | 1280 − 224 − 2×40 | 1280 − 76 − 2×48 = **1108** |
| Gutter | 40 | 48 |
| Library columns | 5 (a sidebar artifact) | **6** |
| Poster | 176×264, 16 gap | **173×260**, 14 gap |
| 16:9 card | 304×171 resume; library cards | **175×98**, 12 gap |
| Library row | 76 | 76 (unchanged) |
| Home hero | compact optional hero | **50 %** backdrop billboard |
| Detail hero | full-bleed, poster column | **66 %** backdrop; poster only if the server has one |
| Hub | none | **6 × 3 = 18 tiles/screen**, Recent 3 |
| Live guide | 168 label + 12 slices | **200 label** + 12 slices; 58 pt rows, 10 mounted |
| Season chips | one unwrapped row | scroll row + “N more” |
| Chapters | 8 + “+N more” (unreachable) | scrollable sheet, all reachable |

4K is a 3× scale of the same point layout, not a second layout. The 1280×800 desktop window keeps
working; it is not the design target.

### 1.2 Colour and type

**Dark** (the mockups): `bg #0A0C0F`, `surface #12161B`, `raised #191D23`, hairline
`rgba(255,255,255,.08)`, `text #F4F6F8`, `muted #98A2AC`, `dim #6E7882`, `accent #38BDF8`,
`accent-soft rgba(56,189,248,.16)`, `danger #F87171`. No accent mixed into a surface: the
current 2.8–4 % blue in the background goes.

**Light** is kept and derived in Phase 1: `bg #F4F6F8`, `surface #FFFFFF`, `raised #FFFFFF`,
hairline `rgba(10,12,15,.10)`, `text #12161B`, `muted #5F6A74`, `dim #8B96A1`, `accent #0284C7`,
`accent-soft rgba(2,132,199,.14)`, `danger #DC2626`. High contrast keeps its existing overrides.

| Token | 720 design | Big screen (1080p) | Use |
|---|---|---|---|
| `HERO` | 46 / 730 / −0.035 / 46 | same | billboard title |
| `PAGE` | 36 / 700 / −0.03 / 40 | same | page title |
| `SECTION` | 20 / 650 / −0.01 / 26 | same | page section |
| `ROW` | 19 / 640 / −0.01 / 24 | same | rail head |
| `CARD` | 15 / 600 / 20 | 16 / 600 / 22 | card title |
| `BODY` | 15 / 450 / 22 | same | body |
| `META` | 12 / 450 / 16 | 15 / 450 / 20 | metadata |
| `LABEL` | 13 / 600 / 18 | 15 / 600 / 20 | control label |
| `CAPTION` | 11 / 500 / 14 | 14 / 500 / 18 | badge, timecode |
| `TITLE` | 18 / 600 / 24 | same | compact panel title |
| `OVERVIEW` | 15 / 400 / 23 | same | synopsis |
| `MONO` | 11 / 500 / 14 | 14 / 500 / 18 | timecode, debug |

The page and section steps follow the captures; card/meta/label are raised from the capture's
11.5/10/12 to the couch floor. The user's text scale multiplies every size and its line height.
The old `big()` flattening of card/meta/label/caption/mono to 16 stops; each token gets its own
big-screen value.

Radii 8 (cards) / 10 (panels) / 12 (hero, sheet) / 16 (modal). Elevation: contact
`0 1 3 rgba(0,0,0,.3)`, lift `0 8 24 -2 rgba(0,0,0,.45)`, focus `0 20 56 -8 rgba(0,0,0,.55)`.

### 1.3 Navigation

Rail order is **Home, Search, Libraries, Live TV, Settings**. The rail captures draw Libraries
before Search; the interaction counts in the screen spec (Search = 3 presses) and this plan use
Search second, and Phase 2 re-renders the screen.

- 76 pt at rest, labels hidden; Left from content enters the rail and expands it to 284 pt as an
  overlay; the page dims, never reflows. A second Left/Back leaves the rail.
- The hub is `Route::Libraries`: Recent 3, all libraries (6 × 3, filter), Tools (Downloads,
  Remote, Controls), playlists, Watchlist, Seerr if configured. Every library and tool is
  reachable here.
- Back: library → its opener (hub or Home) with the tile/card focused; hub → Home; detail →
  library/Home; person → detail; top-level page → rail focus. One `back_target` per pushed page,
  not route guessing.
- Fixed contract: every destination from Home ≤ 8 presses; Search ≤ 3; a recent library ≤ 5; no
  focus on a hidden key.

### 1.4 Home

Six rails by default, in this order:

1. Continue watching — 6 × 16:9, progress, resume label.
2. Your libraries — 8 chips (34 pt) + “All N”, counts, no art.
3. Recently added — one global `/Items/Latest` (no `parentId`), 12 mixed items, 16:9 cards.
4. Next up — 12 × 2:3 posters.
5. On now — 8 × 16:9, only with a tuner.
6. Collections — 12 box sets, 16:9.

- **Pins replace the combined rail.** `config.home_libraries` holds ≤ 4 library ids in selection
  order; each pin is a 12-item “Recently added in X” poster rail and there is no combined rail.
  Four pins = 9 rails, ≈3.8 screens, 4 `latest` calls. No pins = 1 `latest` call.
- Only mounted rails request images; a full curated page is ≈60 images, not ≈220.
- Hero: 50 %, backdrop/still, title (clearlogo when the server has one), meta, Resume + More info.
- Empty/loading/error are per screen: skeleton 3 rails, first-run libraries prompt, inline retry.

### 1.5 Motion

`HOVER .16`, `CONTROL .18`, `RAIL .20`, `PAGE .18`, `GLIDE .22/.05`, `DETAIL .26/.05`, `OSD .12`,
`SHEET .16`, `TOAST .16`, `LAYOUT .20`, `PROGRESS .28`, all zero bounce except the two with .05. Reduce Motion: 80–100 ms fades, instant scroll, no
scale/flight. One spring per role; no ad-hoc numbers at call sites. The per-scene entry, focus,
internal and Back choreography is in the motion spec; each screen phase implements its section.

### 1.6 Scale budgets

| Budget | Value |
|---|---|
| Home rails | 6 default; 9 with 4 pins |
| Home first paint | ≤ 3 requests (resume, libraries, latest) |
| Home full image set | ≤ 60 at 6 rails |
| Mounted images | ≤ 48 per pass; only the rail window |
| Decoded image budget | 768 MB; 720 px posters at 2× |
| Hub | 18 tiles/screen; filter reaches any in 1–2 |
| Library | 100/page, prefetch 1, 6 columns, A–Z when sorted by name |
| Guide | ≤ 10 mounted rows; fetch ≤ 200; favourites first |
| Detail | 14 chips + scroll; ≤ 10 mounted episode rows; 300 cap |
| Person | ≤ 36 mounted credits or virtualized |
| Player | 300 chapters reachable; 100 subtitles filterable |
| Rail | never lists libraries |

### 1.7 Hard rules

`AGENTS.md` applies: no blocking on the UI thread, stable keys and accessible names, a snapshot
for any pixel change, no `unwrap` outside tests, no new dependency, Rune rev pinned. New controls
get `.key` and `.label` in the same commit. Tests mock at `Client`; no test touches the network.

---

## 2. Phase map

| Phase | Outcome | Views | Depends on | Commits |
|---|---|---|---|---|
| 0 | Contract, docs, scale fixtures and baseline | — (no pixels) | — | 1–2 |
| 1 | Tokens, type, motion | every screen | 0 | 1–2 |
| 2 | 76 pt rail + Libraries hub + frame | all screens | 1 | 2–3 |
| 3 | Home billboard, curated rails, library pins | Home, chooser | 2 | 2–3 |
| 4 | Library grid/list, toolbar, A–Z in the gutter | Library | 2 | 2 |
| 5 | Detail and Person | Detail film/series, Person | 3, 4 | 2 |
| 6 | Player OSD and tracks/chapters sheet | Player | 3 | 2 |
| 7 | Live TV, Search, Downloads, Remote | four screens | 2, 3 | 2 |
| 8 | Settings, Playlist, Seerr, empty/loading/error | four screens + states | 2–5 | 2–3 |
| 9 | Motion, accessibility, performance, release | all | 3–8 | 2 |

Each phase ends with `scripts/check.sh` green, the core snapshots committed, `--all` reviewed for
the touched scenes, and the new reachability test in place.

---

## 3. Phase details

### Phase 0 — Contract, fixtures, baseline

**Outcome:** one agreed token/geometry table, scale fixtures that later phases test against, and a
clean baseline. No pixels change.

**Work**

1. Amend the three plan docs: final geometry (1.1), type ladder (1.2), rail order (1.3), Home
   50 %/six rails/pins (1.4); fix the duplicated sentence in the screen spec §2.1.
2. Link this plan from the epic, screen spec and scale analysis; the epic stays the rationale.
3. Add scale fixtures beside `demo::library(count)` in `crates/runefin/src/demo.rs`:
   `home_with_libraries(n)`, `series_detail(seasons, episodes)`, `person_credits(n)`,
   `guide_channels(n)`, `in_app_with_chapters(n)`. Reuse the lean `library_items` path for
   10,000 items; do not add runtime metadata at that size.
4. Add a test helper (`scale_state()` / `assert_rails`) so each phase can state its fixture in one
   line. Tests are added per phase, not here.
5. Confirm the API primitives the plan relies on: `ItemQuery::name_starts_with` and
   `Client::latest(None, 12)`; both exist. Record it in the plan's appendix.
6. Run the baseline `scripts/check.sh` and `scripts/snapshots.sh`; note the green test count and
   keep the core PNGs unchanged.

**Acceptance:** docs agree; fixtures compile; zero pixel diff; baseline green.

**Commits:** docs + fixtures.

**Risks:** a 10,000-item fixture is only for tests — keep the lean path and do not clone it in a
loop. Do not add a `dev-dependency` for data.

---

### Phase 1 — Tokens, type, motion

**Outcome:** the neutral Nocturne surface, a legible 10-foot type ladder and the motion roles.
Structure and geometry are still the old shell.

**Work**

1. `crates/runefin/src/theme.rs`
   - `colors()`: the table in 1.2; remove the accent mix from `bg`/`card`/`separator`; keep light
     and high-contrast branches and check their contrast.
   - `type_`: replace the flat `PAGE/SECTION/...` and the `big()` flattening with the tokens in
     1.2; keep `scaled()` as the only multiplier. Rename call sites mechanically.
   - `motion`: add the full table from the motion spec §2 — `RAIL`, `PAGE` (a `SpringPair`),
     `GLIDE`, `DETAIL`, `OSD`, `SHEET`, `TOAST`, `LAYOUT`, `PROGRESS` — and a `reduced()` policy
     helper so call sites do not branch on the setting themselves.
   - Geometry lands with the phase that first needs it: `GUTTER`/`RAIL_W` in Phase 2, the card and
     still sizes in Phase 3, `GRID_COLUMNS`/`GRID_ROW_H` in Phase 4.
2. `docs/design.md` §5 and the screen-spec token line point at this table.
3. Tests: one unit test that the token ladder and radii are the contract values; the existing
   contrast tests continue to cover text on surfaces. No behavioural test here.
4. Snapshots: core + `--all`; the colour/type diff is intended and reviewed for contrast, not
   waved through.

**Acceptance:** surfaces are neutral; no screen has more than one accent-filled surface; 3-metre
readability of card and meta text at 1080p; Reduce Motion still switches every spring to a fade.

**Commits:** `theme: Nocturne tokens and type`, `docs: visual contract`.

**Risks:** the light theme is derived, not captured; review it on every screen. Existing tests
assert some type sizes/colours indirectly through snapshots only, so the PNG review is the guard.

---

### Phase 2 — Shell: 76 pt rail, Libraries hub, frame

**Outcome:** the sidebar becomes a five-destination rail, every library and tool moves into a
hub, and every screen is built through one `frame()` so no page owns its own chrome.

**Work**

1. **Frame first.** Set `GUTTER = 48`, `RAIL_W = 76`, `RAIL_EXPANDED_W = 284`; add
   `screens::frame(s, t, content)`: a 76 pt spacer, an absolutely positioned `rail(s, t)`, and the
   content clipped to the remaining width. Replace the 25 `row().child(sidebar(s, &t)).child(...)` sites
   in `screens.rs`, `screens/{library,live,search,settings,downloads,remote,person,seerr}.rs`.
   Delete `sidebar()` and `nav_item()` at the end; keep `shelf_screen` on the same frame.
2. **Rail** (`screens.rs` or `screens/rail.rs`): anchors in the order
   `nav-home`, `nav-search`, `nav-libraries`, `nav-live`, `nav-settings`; 48×44 hit, 19 px icon,
   label on focus/expanded; selected = 8 % white + 1 px 12 % border, no accent fill.
   `s.rail_expanded` is derived from `s.focused_key` in `poll` (no new key handler); the rail
   renders 76 or 284 and a scrim child dims the page. Keep the rail's own scroll container for
   the expanded 5 rows (there are only five; it never lists libraries).
3. **Routing.** Add `Route::Libraries` and `State::open_libraries()`. Add a single
   `back_target: Option<Route>` to `State` (or reuse `detail_return`'s pattern) set by every
   opener: hub → Home, library → hub/Home, detail → library/Home, person → detail. `Back` uses
   it, restores focus to the opener key, then clears it. `open_library` records the origin; the
   hub's `open_library` sets `Libraries`.
4. **Hub** (`screens/hub.rs`): `Recent` (last 3, from `config.recent_libraries`), `All libraries`
   6 × 3 with a filter field, `Tools` (Downloads, Remote, Controls; Settings stays the rail anchor), `Playlists`, `Watchlist`, `Seerr` when configured. Keys
   `hub-recent-{id}`, `hub-library-{id}`, `hub-tool-*`, `hub-playlist-{id}`, `hub-watchlist`,
   `hub-seerr`, `hub-filter`. Tile art = library Primary via `poster_layers`; glyph fallback;
   counts from the library list. Empty server = first-run copy + one action.
   `config.recent_libraries: Option<Vec<String>>` updated by `open_library`/`open_playlist`/
   `open_watchlist`; ignore ids the server no longer has.
5. **Focus.** `tv_focus::sidebar()` becomes `rail()` with the five keys and `parent: None`;
   `in_sidebar` → `in_rail`; update the scroll scope from `sidebar-nav-scroll` to `rail-nav-scroll`
   (only the expanded rail). Add `hub_zones()`: filter → recent → libraries (6 columns) → tools →
   playlists. Left at the first column goes to the rail; Back in the rail leaves to the page.
6. **Input.** `on_key`: the rail's Enter follows the anchor; `Escape`/Back expands then leaves,
   not both at once. The page dim is 160 ms; Reduce Motion is instant with labels on.
7. **Tests.** Update `tv_reach_tests` to start at the rail; add `hub`, `hub-40` (every library key
   present, filter narrows to one), `back_from_library_returns_to_hub`, `back_from_hub_home`,
   `rail_never_lists_libraries`, and `no_hidden_focus_in_hub`. Update every test that used
   `nav-downloads`/`nav-remote`/`nav-watchlist`/`library-*` in the sidebar.
8. **Snapshots.** Add core `hub`; add `hub-40`, `rail-focus`; refresh every screen with the rail.
   The old `focus-sidebar` scene is replaced by `rail-focus`.

**Acceptance:** every existing route reachable via the rail/hub in the documented count; Back
restores the opener; no library row in the rail; expansion does not move the page; 40 libraries
fit 3 screens and the filter reaches any in 1–2.

**Commits:** `shell: frame and rail`, `shell: Libraries hub and back targets`, `tests/snapshots`.

**Risks:** this is the highest-churn phase (25 call sites). Migrate compile-first with `frame()`,
then delete `sidebar()`. Watch `tv_focus` zone indices and the `scroll_into_view` paths; the
existing sidebar scroll bug is the template for the hub.

---

### Phase 3 — Home

**Outcome:** the Home in the captures: a 50 % billboard, six curated rails, a libraries strip, an
opt-in four-pin chooser, and no automatic per-library shelf wall.

**Work**

1. **Data** (`crates/runefin/src/home.rs`):
   - `LatestRail.library_id: Option<String>`; `None` means the combined rail.
   - `load_rails(client, pins: &[String])`: pins empty → one `client.latest(None, 12)`; else one
     `client.latest(Some(id), 12)` per pin, in order, at most four. `libraries` is unchanged.
     The `thread::scope` spawns only for the pins (≤4); the combined path is one call.
   - `HomeData`/`HomeShelf`: add `HomeShelf::RecentlyAdded` for the combined “See all” (it requests
     `latest(None, 60)` once when opened); keep the other shelves.
   - `home_cache`: bump `VERSION`; changing pins calls `home_cache::forget` (or replaces the
     cached Home) so a stale per-library rail is never painted after the choice.
2. **Rails** (`screens.rs`): `home_rails()` returns the fixed order in 1.4; drop the automatic
   `Recently added in X` loop; insert the pins (or the combined) in slot 3; cap at six (nine with
   four pins). Keys: keep `rail-continue-watching-*`, `rail-next-up-*`, `rail-collections-*`;
   use `rail-recent-{index}` for the combined/pins; add `rail-libraries-*`, `rail-on-now-*`.
3. **Cards and hero:** 6-up 16:9 for Continue/Recently/On now/Collections; 6-up 2:3 for Next up.
   The `home_hero` preference stays: on = the 50 % billboard (backdrop/still, title, clearlogo when
   available, meta, Resume/More info); off = the plain header + rails. Remove the `Home` page
   header when the hero is on; the hero is the header. The first rail is visible at 720.
   Set `CARD_W = 173`, `CARD_H = 260`, `CARD_GAP = 14`, `STILL_W = 175`, `STILL_H = 98` here: six
   poster cards at 1108 need 173 pt, and Home and the library then share the geometry.
4. **Libraries strip:** eight `home-library-{id}` chips + `home-library-all`; counts; no art;
   `All N` opens the hub; chips reuse the hub's library list, so no extra request.
5. **Pins chooser:** a sheet over Home (`screens/home_rails.rs` or `screens/settings.rs`):
   `home-rails-all` / `home-rails-chosen` segmented, rows `home-rails-{library_id}` with count and
   position, `home-rails-reset`, `home-rails-done`; 4 max; Enter toggles, Back saves. Entry:
   a `Home rails` row in the existing Settings page now (Phase 8 moves it into Interface) and a
   `hub-tool-home-rails` shortcut. `config.home_libraries` persists per device. The chosen state
   is the mockup; the all state greys the list.
6. **Artwork fallback:** add a `SurfaceFit::Letterbox` (or blurred Fill + Fit) for a 16:9 source
   in a 2:3 card and for the detail hero when no poster exists; use it in `poster_layers` so Home
   and library share it. Do not add a decoder; reuse the image cache and cap blur at four layers.
7. **States:** skeleton 3 rails on `!home_loaded`; `home_empty` first-run with “Open Libraries”
   and “Refresh”; `home_error` inline retry; image placeholder while loading.
8. **Tests:** `home_12_libraries_has_6_rails` (no per-library rails), `home_four_pins_has_9_rails_and_4_latest`,
   `home_pin_empty_keeps_pin`, `home_pins_reset_returns_combined`, `home_first_focus_is_resume`,
   `home_libraries_strip_reaches_hub`, `home_no_blocking` (the existing bench/Runtime pattern).
9. **Snapshots:** `home`, `home-pins`, `home-scrolled`, `home-empty`, `home-loading`, `home-error`,
   `home-big-4k`; update the core `home` and `focus-card`.

**Acceptance:** 12 libraries → 6 rails and ≤3 first-paint requests; 4 pins → 9 rails, 4 `latest`
calls, no combined; a pin with no items shows no rail and keeps its pin; the first rail is
visible without scrolling; the libraries strip opens the hub; no automatic per-library query.

**Commits:** `home: combined recent and rail budget`, `home: billboard and libraries strip`,
`home: library pins chooser`, `tests/snapshots`.

**Risks:** the billboard at 50 % against the 62 % capture is an intentional change; verify the
first rail is visible at 720 and 4K. `home_cache` version/invalidations. The pin list must be
filtered against the current libraries so a deleted id never blanks the page.

---

### Phase 4 — Library grid, list and A–Z

**Outcome:** six complete columns, one toolbar row, the letter rail in the right gutter, and the
10,000-item case behaving like a page, not a wall.

**Work**

1. **Geometry** (`library.rs`, `screens/library.rs`): set `GRID_COLUMNS = 6`; `CARD_W`/`CARD_GAP`
   from Phase 3; recompute `GRID_ROW_H` (≈320) and the skeleton row. The measured `columns`
   calculation already clamps 2–12; at 1108 it must land on 6, not 5. Verify 1280, 1440, 4K.
2. **Toolbar:** one row at 720: search field, Name/Added/Rating sort, direction, filters, Grid/List.
   Active filter = text + underline/soft fill, no blue pill. Up from the first row reaches it;
   Enter/Menu on the field opens the keyboard (never focus alone).
3. **A–Z:** `letter_rail()` already exists and `LibraryState.letter` + `toggle_letter` already
   query `name_starts_with`; move the 28 pt rail out of the body flow into an absolute gutter
   overlay (`letter-rail`, `letter-A..Z`), keep the keys, and add the active letter's
   scroll-into-view. Right at the last column reaches it; Left at the first column reaches the
   rail. It is hidden for Random/relevance sorts.
4. **List:** 76 pt rows with a 132×74 still, title/meta, resume line; the same toolbar and focus
   zones. The row's trailing action is resume, not a separate focus trap.
5. **Paging:** keep `PAGE_SIZE = 100`, one prefetched page and mounted-row virtualization; show
   `1–100 of 10,000` from `TotalRecordCount`; a failed page is an inline row with Retry.
6. **States:** skeleton grid, empty library (“Nothing here yet” + Refresh), error, and the image
   placeholder.
7. **Tests:** `library_1280_has_6_columns`, `library_10000_uses_100_pages_and_az`,
   `letter_jump_requests_name_starts_with`, `letter_rail_is_reachable_from_last_column`,
   `library_paging_failure_keeps_items`, `library_empty_and_error_are_reachable`.
8. **Snapshots:** grid, list, 10k, empty/loading/error, 4K. The playlist page is Phase 8.

**Acceptance:** six complete columns with no card under the rail; A–Z in the gutter and reachable;
toolbar one row; 10,000 items has a stable first page and a jump; no layout shift at 4K.

**Commits:** `library: six columns and gutter A–Z`, `library: toolbar/list/states`, `tests/snapshots`.

**Risks:** the columns calculation derives from the measured width; test it at 1280 before
changing constants. The letter overlay must not steal focus from the last column (the existing
epic risk).

---

### Phase 5 — Detail and Person

**Outcome:** a backdrop-led detail page for a film and a series, a season/episode list that scales
to 20/500, and a person page that reaches 100 credits.

**Work**

1. **Film detail** (`screens/detail.rs`): 66 % backdrop hero; title (clearlogo when present);
   one meta line; Resume/Start over/Trailer/favourite/overflow; overview, cast, related below.
   No 2:3 poster column when the server has no poster; a 16:9 still is the hero. Back above the
   scroll. Keep the existing `detail-*` keys where they are; add `detail-overflow` only if needed.
2. **Series:** season chips become a scroll row with `detail-seasons-more` (“N more”); the active
   season scrolls into view; episode rows window to 10 mounted with `N more` and a 300 fetch cap;
   `Play season`/`Shuffle`/`Mark season` stay. The episode still uses `own_image_source`.
3. **Person** (`screens/person.rs`): portrait/initials, bio, `In your library` six-column grid
   windowed to the mounted rows or 36 + “Show all 100”; Seerr extras stay 24; Back to detail.
4. **Fallback:** reuse the Phase 3 letterbox helper for any remaining 2:3 card; the detail hero
   never receives a cropped poster.
5. **Tests:** `detail_series_20_seasons_reaches_last`, `episode_list_mounts_10_of_300`,
   `person_100_credits_reaches_last`, `detail_still_hero_without_poster`,
   `detail_person_back_restores_focus`.
6. **Snapshots:** film, series, series-20, person-100, the 16:9 fallback case, 4K.

**Acceptance:** the page fills the width; every season/episode/credit is reachable; the hero is
legible over bright art; Back restores the cast/detail focus.

**Commits:** `detail: hero and series window`, `person: credit window`, `tests/snapshots`.

**Risks:** the series hero art in the sample is bright; the scrim/legibility pass is part of the
phase. Episode windowing must not break autoplay-next or watched state.

---

### Phase 6 — Player OSD and tracks/chapters

**Outcome:** a calm OSD and one sheet that reaches every audio track, subtitle and chapter.

**Work**

1. **OSD** (`screens/in_app.rs`, `state/in_app_state.rs`): bottom gradient only; scrubber with
   chapter ticks; play/pause, ±10 s, audio/subs, gear, fullscreen/back; auto-hide 3 s; video dims
   40–60 % while visible; first input while hidden only shows the OSD; Up opens the sheet.
2. **Tracks & chapters sheet:** tabs Audio (20), Subtitles (100), Chapters (300); a scrollable
   list with a position bar; chapters virtualized and every index reachable; subtitles filtered by
   language after 20; audio selection applies immediately. Replace `take(8)` + “and N more” with
   the real list.
3. **Motion:** `OSD` 120 ms fade/8 px; sheet 160 ms; Reduce Motion 80 ms.
4. **Tests:** `player_300_chapters_all_reachable`, `subtitle_filter_narrows_100`,
   `audio_20_selection_changes_track`, `osd_auto_hides_and_first_key_reveals`,
   `chapter_sheet_focus_order`.
5. **Snapshots:** OSD over bright and dark frames, sheet on each tab, 4K.

**Acceptance:** no control overlaps the video when hidden; the sheet reaches all 300 chapters;
the OSD is legible over the red frame; every control named.

**Commits:** `player: OSD`, `player: tracks and chapters sheet`, `tests/snapshots`.

**Risks:** the OSD and the subtitle renderer share the bottom area; verify with subtitles on.
Chapter virtualization must keep the current chapter in view.

---

### Phase 7 — Live TV, Search, Downloads, Remote

**Outcome:** the four utility screens of the captures, with the scale limits applied.

**Work**

1. **Live TV** (`screens/live.rs`, `state/live_state.rs`): guide with a 200 pt channel label and
   12 slices; 58 pt rows with 10 mounted; a pinned preview panel (art, title, time, Watch/More
   info); now line; favourites (8) one press away, All as the browse mode; the fetch cap rises from
   120 to 200 with a “showing 200 of N” note; tuner-less empty state. The current all-rows build becomes a windowed
   list.
2. **Search** (`screens/search.rs`): 52 pt field, recent searches, grouped Films/Series/Episodes
   rails six-up with “See all”; 80-result query; keyboard from Enter/Menu; no results with the
   term quoted; scope preserved.
3. **Downloads** (`screens/downloads.rs`): storage summary card, rows with poster/title/size/
   state/Play/Pause/Remove, empty state; the folder path moves to Settings.
4. **Remote** (`screens/remote.rs`): session rows, idle Play/Stop vs playing Pause/Stop, Refresh
   toolbar action, empty when no sessions.
5. **Tests:** `guide_60_mounts_10_rows`, `guide_favourites_filter`, `search_results_group_and_empty`,
   `downloads_empty_and_actions`, `remote_idle_actions`.
6. **Snapshots:** guide, search, downloads, remote, 4K; the guide’s 60-channel stress view.

**Acceptance:** 60 channels mounts 10 rows at a time (6 beside the preview), not 60; search
results use the same six-up rhythm; no dead middle; every empty state has one action.

**Commits:** `live: guide window and preview`, `search/downloads/remote`, `tests/snapshots`.

**Risks:** the guide's program lookup is O(rows × slices × programs); window before optimizing.
Favourites must be stable when the server reorders channels.

---

### Phase 8 — Settings, Playlist, Seerr, states

**Outcome:** the remaining views and the state sheet.

**Work**

1. **Settings two-pane** (`screens/settings.rs`): a left section list (Interface, Modes, Playback,
   Subtitles, Keyboard, Metadata, Downloads/Storage, History, Seerr, Account, Install/About) and a right
   detail pane; the existing section builders move under their headings; segmented controls and
   switches stay their own focusables; `Home rails` lands here; `Back` returns to the opener.
   Keys keep `settings-section-*` and the control keys; the section list gets
   `settings-nav-{section}`.
2. **Playlist** (the `collection_type=playlists` branch of the library screen): Play all/Shuffle/
   Edit, rows, reorder/remove, empty playlist; reachable from the hub; the virtual Playlists
   library does not 404.
3. **Seerr:** only when configured; hub entry + Discover; no sixth rail icon; request states and
   the existing detail.
4. **States:** apply the state sheet to every screen: first run, empty library, search no results,
   no tuner, no downloads, no credits, paging failure, image placeholder. One line + one primary
   + one secondary; Back always works.
5. **Tests:** `settings_sections_reachable`, `home_rails_row_opens_chooser`,
   `playlist_edit_reorders_and_removes`, `seerr_only_with_url`, `every_empty_state_has_action`.
6. **Snapshots:** settings, playlist, seerr, the state sheet; 4K.

**Acceptance:** every setting from the old page is reachable in the two-pane; the playlist edit
round-trips; Seerr absent changes nothing; no empty state is a dead end.

**Commits:** `settings: two-pane`, `playlist/seerr`, `states`, `tests/snapshots`.

**Risks:** the settings page has many controls; move sections without renaming keys, or update
the tests and snapshots in the same commit. The two-pane must keep the keyboard's focus handoff.

---

### Phase 9 — Motion, accessibility, performance, release

**Outcome:** the plan's motion and accessibility promises are enforced, the scale budgets hold,
and the release gates are green.

**Work**

1. **Motion:** replace ad-hoc springs with `theme::motion`; the detail open is a matched
   card→hero flight, Back reverses it; a second press retargets; Reduce Motion is 80–100 ms fades
   and instant scroll. The rail/page/OSD times come from 1.5.
2. **Accessibility:** `every_screen_names_every_control` over every new scene; focus ring visible
   on artwork; contrast of muted/dim on surfaces and on art; text scale 85–130 %; keyboard mode;
   reduced motion; no focus on hidden keys.
3. **Performance:** mounted-only images, ≤48 per pass, 768 MB; no UI-thread network/disk;
   rails/grid/guide/episodes/credits/chapters windowed; Home ≤3 first-paint requests; 4K render
   time within the frame budget.
4. **Motion audit:** walk the motion spec's §5 table at 1280, 1080p and 4K; check every scene's
   entry/focus/Back, every interruption, and the Reduce Motion path; use `--trace`.
5. **Scale tests:** all thirteen in the scale analysis, plus the per-phase tests added in 2–8;
   run with `--test-threads=1`.
6. **Docs and release:** update `docs/design.md`, `docs/conventions.md` if it lists tokens,
   `docs/snapshots/` (core) and the README image; `scripts/check.sh`; `scripts/snapshots.sh --all`;
   note the intended diffs in the commit. No release tag or install in this phase.

**Acceptance:** the epic acceptance list and the scale budgets hold; `check.sh` green; core
snapshots reviewed; no known visual regression at 720/1080p/4K.

**Commits:** `motion: one ladder`, `a11y/perf`, `docs/snapshots`.

**Risks:** motion retargeting and the detail flight are the most likely regressions; keep a
reduced-motion snapshot and test. Performance work can invalidate image caches; verify a cold
launch and a warm cache.

---

## 4. Mockup → phase

| Capture | Phase | Scene / note |
|---|---|---|
| Home (15-view set) | 3 | `home` |
| Home · rail focus | 2 | `rail-focus` |
| Libraries hub | 2 | `hub`; 6×3 tiles, not the 3×3 capture |
| Library · grid | 4 | `library` |
| Library · list | 4 | `library-list` |
| Detail · film | 5 | `detail` |
| Detail · series | 5 | `detail-series` |
| Search | 7 | `search` |
| Live TV · guide | 7 | `live-tv` |
| Player · OSD | 6 | `in-app-osd` |
| Downloads | 7 | `downloads` |
| Remote | 7 | `remote` |
| Settings | 8 | `settings` |
| Person | 5 | `person-100` |
| Playlist | 8 | `playlist` |
| Home rails chooser | 3 | `home-pins` |
| Home with 12 libraries (scale) | 3 | `home-12` |
| Home rails 5–7 (scale) | 3 | `home-scrolled` |
| Hub 40 / Library 10k / Guide 60 | 2/4/7 | `hub-40`, `library-10k`, `guide-60` |
| Series 20 / Person 100 / Player 300 | 5/5/6 | `detail-series-20`, `person-100`, `player-tracks` |
| Rail 30 / Empty states | 2/8 | `rail-30` is the rejected library-list mode (scale analysis §9); the rail itself shows five anchors. The per-screen empty scenes. |

The captures are evaluation artifacts; the committed scenes are the contract.

## 5. Cross-cutting checklist

- **Every commit:** compiles, `scripts/check.sh`, stable keys, `.label` on every control,
  no `unwrap`/`todo!`/`dbg!`, no blocking in a handler or `poll`.
- **Every screen phase:** first focus is the primary action; Back restores the opener; Left at the
  first column reaches the rail; focus never lands on a hidden key; the new scene is in
  `every_screen_names_every_control` and `tv_reach_tests`.
- **Every pixel phase:** core snapshots rendered; `--all` for the touched views; the commit names
  the intended diff.
- **Images:** 2× width, 768 MB decoded, mounted-only requests; the fallback chain from Phase 0 of
  the research remains; blur ≤ four layers.
- **Motion:** the motion spec is the per-scene contract. A screen phase is not done until its
  §4 scenes exist, are interruptible, and have a Reduce Motion path. Phase 9 audits the master
  table and 4K frames.
- **Docs:** `docs/design.md` is the contract; this plan is the order; the spec is the visual
  reference; the scale analysis is the budget. A change to one updates the other in the same
  commit.

## 6. Release gates

1. All phase acceptance criteria met.
2. `scripts/check.sh` green (tests, clippy, release build, guardrails).
3. Core snapshots committed with a note for every intended diff; `--all` reviewed.
4. 1280×720, 1920×1080 and 4K frames show no clipped columns, rings or hairlines.
5. Scale fixtures: 12 libraries, 40 libraries, 10k items, 60 channels, 20 seasons, 100 credits,
   300 chapters.
6. No new dependency, no Rune rev bump, no change to `jellyfin-api` semantics.

## 7. Out of scope

- Installing or testing on the Steam Machine during this plan; that is a separate validation pass
  after the code lands.
- HDR hardware validation, Flatpak `xdg-download`, `runefin-mode` loginctl restart, Mac Metal
  builtin-table regeneration, the Steam tile art/duplicate.
- Seerr changes beyond its hub entry and existing tabs.
- A Rune fork feature; if a phase is blocked, stop and record the gap in the research note.
- The optional 284 pt library flyout (the 30-library rail capture). The five-anchor rail plus the
  hub is the supported model; if the flyout is revisited it follows scale analysis §9.

## Appendix — API and code facts checked for this plan

- `Client::latest(parent: Option<&str>, limit)` already supports the global combined query.
- `ItemQuery::name_starts_with` exists and is already used by `LibraryState::letter`; the A–Z UI
  only needs to move into the gutter.
- `LibraryState` already has `letter`, `columns`, `view`, paging and `PAGE_SIZE = 100`;
  `screens/library.rs` already virtualizes rows and builds the letter rail.
- `HomeData.latest` is `Vec<LatestRail>`; `load_rails` spawns one thread/query per video library.
- `theme::SIDEBAR_W = 224`; `sidebar()` has 25 call sites; `tv_focus::sidebar()` mirrors the list.
- `scripts/snapshots.sh core()` is eight views plus the focus pair; the hub becomes a core view.

---

## 10. Status after the first implementation pass (2026-11-27)

Landed and checked with `scripts/check.sh` (493 tests, clippy, release) and live captures against
the real server at 1280×720:

- **Phase 1:** the graphite palette, the 36/20/19/15/13/12/11 type ladder, 48 pt gutters and the
  motion roles. No accent is mixed into a surface.
- **Phase 2:** the 76 pt five-anchor rail (Home, Search, Libraries, Live TV, Settings), the 284 pt
  overlay, the Libraries hub with Recent, library art, a local filter, Tools, playlists,
  Watchlist and Seerr; Back returns a library to the hub.
- **Phase 3:** the 50 % billboard, one combined Recently added rail, the libraries chip strip,
  6-up Continue/On now/Recently added/Collections, Next up posters, the four-pin chooser and
  `home_libraries`; pins replace the combined rail and only pinned libraries are queried. The
  home cache version is bumped.
- **Phase 4:** six 173/172 pt columns, the A–Z rail moved into the right gutter, the library's
  back label and the hub route.
- **Phase 5:** the existing full-bleed detail kept; the season strip scrolls; the person grid is
  six columns.
- **Phase 6:** the chapter sheet lists every chapter (300 tested) instead of eight and a dead
  "and N more"; the OSD has its transition.
- **Phase 7:** the live guide scrolls; Downloads is reachable from the live route; Search and
  Remote use the new shell.
- **Phase 8:** Settings is a left section list plus the scrolling pane, with the Home rails entry.
- **Phase 9:** in-use motion tokens (rail, sheets, toast, OSD, matched detail, layout, progress),
  reduce-motion paths, accessibility/reachability tests, core snapshots and the snapshot README.

Live captures: `/tmp/opencode/live-phase3-hero.png`, `live-hub-final.png`,
`live-library-az.png`, `live-phase5-detail-before.png`, `live-series.png`, `live-search.png`,
`live-live-guide.png`, `live-settings.png`, `live-person.png`, `live-downloads2.png`,
`live-remote.png`, `live-home-pinned.png`; the post-merge Rail/Home is `live-phase3-pins.png`.

**Status: the planned phases are implemented.** The two-pane Settings is a section list plus its
detail pane; the OSD is the existing composition with the new chapter sheet and its transition; the
page transition is directional (push rises, Back settles from above) and Home, hub and rail cards
scale on focus. The extreme-count cases are unit-tested because the review server has 8 libraries,
354 movies and 4 channels; they are not live captures.

The only verification left is outside the code: playback and the OSD need a real display (the
headless live route stops at "Starting the in-app player…"), and the change needs the user's
review and commit. The test count is 502 and `scripts/check.sh` passes.

### Polish pass after the laptop review

The first hands-on review found three classes of problem; each is fixed and now pinned by the
`arrows_never_move_against_the_press` test (every arrow from every visible control must land in
that direction) plus the existing reachability walk.

- **Focus line, cut and pale.** The big-screen focus ring was drawn outside a clipped poster and
  the card's white "focus glow" sat behind the art, so a focused card showed a pale line cut at
  the rail's edge. The ring is now 2 pt *inside* the border box; the card and wide card carry an
  explicit inset edge overlay above the image (`theme::focus_edge`); the glow is a soft dark lift
  shadow, not a white halo. The rail's ring in `rail-focus-big.png` is the clean reference.
- **D-pad moved somewhere else.**
  - Settings had no zones and turned Up/Down into Tab; the section list and form now have real
    zones, Left lands on the matching section (`parent_at`), Right enters that section, and the
    list's ends stop. The rail is `role("list")`, so it no longer wraps Home → Settings.
  - A rail's "See all" was the last key in its row: Left from it jumped to the far-right card and
    Right from the last card slid onto it. The header is now a real row (Up from a card reaches
    it; Down from it enters the cards), and the column, not the flat index, carries between lanes.
  - The library toolbar's two wrapped lines were one zone (Right at the end jumped back left);
    they are now two lanes. The grid and list mount one row of overscan, so Down from the last
    visible row no longer aims at an unmounted card.
  - Hub content zones escape Left to the rail instead of the row above; detail facts sit above
    the actions and the season chips above Next up, matching the page; the person grids escape to
    the rail. The guide preview and the shelf keep their own zones.
- **The line under the Home billboard.** With the billboard up, the ambient backdrop still ran
  under the rest of the page; the billboard faded to solid `bg`, so the boundary between solid
  background and the dim backdrop read as a horizontal line. The ambient is now skipped when the
  hero is drawn.

`scripts/check.sh` passes with 502 tests; core snapshots were regenerated at 1280×720 and the
1280×800 focus/rail scenes.
