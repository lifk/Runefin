# Nocturne at scale: what each screen does as the server grows

Status: evaluation, 2026-11-26 · follows [`2026-11-26-nocturne-screen-spec.md`](2026-11-26-nocturne-screen-spec.md)
and [`2026-11-26-redesign-epic.md`](2026-11-26-redesign-epic.md). Execution:
[`2026-11-26-nocturne-implementation-plan.md`](2026-11-26-nocturne-implementation-plan.md).
Mockups: eleven stress views at 1280×720 (local captures, not committed; the film artwork is
evaluation-only). The Steam Machine was not used.

---

## 1. Method

Four tiers per entity: **typical**, **large**, **extreme** (what a real Jellyfin server can hold
without being broken), and **the current cap** in the code. Every number below is read from the
tree at `51e0e8f`, not estimated from the sample library:

- `crates/runefin/src/home.rs` — one `latest(library, 12)` per video library, spawned one thread
  per library inside a `thread::scope`; `resume(24)`, `next_up_all(12)`, `upcoming(24)` cut to 12,
  `live_channels(8)`, `collections(12)`, `recently_played` cut to 12.
- `crates/runefin/src/screens.rs` — `home_rails()` is 4 fixed rails + one per library + Recently
  Played + Collections; `HOME_RAIL_ESTIMATE_H = 386`; rails are virtualized with
  `virtual_overscan(1.0, 0.5)`.
- `crates/runefin/src/library.rs` — `PAGE_SIZE = 100`, `GRID_COLUMNS = 5` (the 224 px sidebar;
  the Nocturne rail gives the sixth column), `GRID_ROW_H = 344`.
- `crates/runefin/src/state/live_state.rs` — `live_channels(120)`.
- `crates/runefin/src/detail.rs` — `children()` uses `limit = 300` for seasons and episodes.
- `crates/runefin/src/screens/detail.rs` — season chips are one unwrapped `row()`; the episode
  list is a plain `col()` of every loaded episode.
- `crates/runefin/src/screens/person.rs` — server credits are all built; Seerr extras `take(24)`.
- `crates/runefin/src/screens/in_app.rs` — the chapters sheet renders `take(8)` and
  `"and N more"`, with no control that reaches the rest.
- `crates/runefin/src/state/image_state.rs` — the current-image set is rebuilt per screen pass;
  only mounted tiles request art, at 2× width, inside a 768 MB decoded budget.

The stress captures change only data volume, never the layout or the tokens.

## 2. Cardinality matrix

| Entity | Typical | Large | Extreme | What grows with it | Today | Rule |
|---|---|---|---|---|---|---|
| Video libraries | 1–5 | 6–12 | 13–40 | Home rails, hub tiles, queries | 1 rail + 1 query + 1 thread **each** | One combined latest, or ≤ 4 explicit pins; hub filters; rail never lists them |
| Home rails | 4–6 | 7–12 | 13–18+ | scroll length, image set | `L + 6` (18 at 12 libs) | ≤ 6, curated; per-library only if pinned |
| Rail items | 12 | 12 | 24 | images, card build | 12 per library, 24 resume | 12; only mounted rails request art |
| Library items | < 500 | 500–5,000 | 5,000–50,000 | pages, rows | 100/page, windowed | 100/page, prefetch 1, A–Z + search |
| Live channels | 5–20 | 20–60 | 60–500 | guide rows, cells | fetch 120, **all rows built** | window 10 rows; cap 200; favourites first |
| Seasons | 1–5 | 5–12 | 12–30 | chips | one unwrapped row | scroll row + "N more" |
| Episodes / season | 10–25 | 25–60 | 60–300 | episode rows | all built, cap 300 | window 10; keep 300 as the hard cap |
| Person credits | 10–30 | 30–100 | 100–300 | poster grid | all built | window 24–36 or virtualize |
| Audio tracks | 1–4 | 4–12 | 12–32 | track list | one list | one scrollable sheet |
| Subtitle tracks | 0–10 | 10–40 | 40–120 | track list | one list | filter by language; scroll |
| Chapters | 0–20 | 20–80 | 80–400 | chapter list | **8 shown, rest unreachable** | scrollable list, all reachable |
| Playlists | 0–10 | 10–50 | 50+ | hub rows, playlist page | flat rows | recent + all; search in the hub |
| Downloads | 1–20 | 20–100 | 100+ | rows, storage | flat rows | rows + storage summary; cap by disk |
| Search results | 0–20 | 20–80 | 80+ | result rails | 80 per query, grouped | 6 per group + "See all" |

The **Today** column is where the mockups bite: the app's data caps are mostly sensible, and the
layout is what fails first.

## 3. Home: the one screen that fails at scale

The rail formula is `4 fixed + L + 2` (Continue Watching, Next up, Coming up, On now; one
"Recently added in `<library>`" per library; Recently Played, Collections). Empty rails drop out,
so a fresh server is fine and a full one is not.

At **12 libraries** the captured Home has **18 rails**:

- ≈ 386 px per rail × 18 ≈ 6,950 px of rails + 446 px hero = **≈ 7,400 px ≈ 10.3 screens**.
- Collections and Recently Played sit **~9 screens down**, below twelve shelves of the same 12
  newest items from each library.
- A full top-to-bottom pass references **≈ 220 images** (24 resume + 12 next up + 12 coming up +
  8 on-now + 12 × 12 library + 12 recently played + 12 collections). At 720 px posters
  (≈ 3.9 MB decoded) that is ≈ 870 MB, past the 768 MB budget: later rails evict the first ones,
  so scrolling back re-fetches. Mounting only the rail window keeps the *peak* sane (≈ 4 rails,
  ≤ 48 images) but not the *scroll cost*.
- Loading spawns **one thread and one HTTP request per library** (12 each). The server answers,
  but the burst is unbounded: 40 libraries is 40 threads and 40 `latest` calls on every Home load.

The scrolled capture shows the other half of the problem: the per-library rails are
indistinguishable. "Recently added in Movies", "…in Shows", "…in Anime" are the same card, the
same metadata, the same "See all"; the only difference is the library name in the header. This is
not discovery; it is a list of libraries in disguise.

**Home budget (recommended):**

| Rail | Content | Source |
|---|---|---|
| 1 | Continue watching | `/UserItems/Resume`, 24 → 12 cards |
| 2 | Your libraries | already-loaded library list, 8 chips + "All N" |
| 3 | Recently added | **one** `/Items/Latest` with no `parentId`, 12 mixed cards — or one per pinned library (≤ 4) |
| 4 | Next up | `/Shows/NextUp`, 12 |
| 5 | On now | `live_channels(8)`, only with a tuner |
| 6 | Collections | box sets, 12 |

- **Six rails by default.** Four explicit pins take it to nine; nothing else may add a rail.
- **One latest query**, not one per library. The API already supports the global form
  (`latest(None, 12)`); the mixed rail needs 16:9 cards because the items are not all posters.
- **Your libraries is a chip strip**, not a rail of art: 8 chips at 34 px plus "All 12". It costs
  61 px, not 386, and answers "where are my libraries" without a trip to the hub.
- Per-library "Recently added" rails are **off by default**; a reader can pin libraries explicitly
  (next section). A pin replaces the combined rail, never appends beside it.
- The hero stays the resume item. At 50 % it buys the third group above the fold; the earlier 62 %
  shows one and a half rails. 50–52 % is the recommendation for a Home with a libraries strip.
- Image requests follow the mounted window only (already true); the combined rail plus the chips
  takes the full-page set to **≈ 60 images**, inside one budget with room for the detail flight.

The curated capture is 6 rails, **≈ 2.2 screens**, one latest query, one thread.

### 3.1 Pinned library rails (opt-in)

A reader can choose libraries for their own rail, but the choice is a **pin**, not an automatic
rail per library. Capture: the `home-rails` stress view at 1280×720 (local, not committed).

- **Settings → Interface → Home rails** (also "Customize Home" in the hub's Tools) opens a sheet
  over the page: one row per video library with name, item count, checkbox and its rail position.
  Enter toggles, Back saves; no drag, no keyboard.
- **Four pins maximum**, shown as `1..4` in selection order; that order is the rail order, and
  deselecting renumbers. `Reset` returns to the combined rail.
- **A pin replaces the combined rail.** No pins → one "Recently added" across all libraries. One to
  four pins → that many "Recently added in X" rails, and no combined rail, so no card repeats.
- **Only pinned libraries are queried.** `load_rails` takes the pinned ids and issues one `latest`
  per pin; the default remains one global `latest`. The 12-query burst never returns, chooser or
  not.
- A pinned library with no items is skipped for that load and keeps its pin (the chooser says "no
  recent items"); a library deleted from the server is ignored and `Reset` clears it.
- Stored per device as `home_libraries` beside `hidden_home` in `config.rs`; the chooser makes no
  server call.
- Budget: default 6 rails / ≈ 2.2 screens / 1 query; 4 pins is 9 rails / ≈ 3.8 screens / 4 queries.
  The cap, not the chooser, is what stops an explicit choice from rebuilding the 18-rail wall.

## 4. Libraries hub and library page

**Hub.** 40 libraries at 6 columns × 3 rows = **18 tiles per screen**, so the last library is
**≈ 2.3 screens** away. A Recent row keeps the daily three on screen 1. Two things are required:

- a **filter field in the page head** that narrows by name; at 40 libraries this is faster than any
  grid position, and it is the only path that stays constant as the count grows;
- **one stable order** (recent first, then server order) so a tile does not move between visits.

Tile art is the library's primary image when present; otherwise a glyph plus the name. No
generated poster and no borrowed film art: a library is not a film.

**Library page.** 10,000 items is 100 pages of 100; at six columns one page is 17 rows ≈ 6.5
screens, so the whole library is ≈ 650 screens. Browse is not the path; search and A–Z are.

- Keep `PAGE_SIZE = 100` and one prefetched page; the mounted-row virtualization stays.
- The header shows `1–100 of 10,000`; the count is the server's `TotalRecordCount`, not a guess.
- **A–Z jumps to the first item whose sort name starts with the letter**, not to page N/26. It is
  only offered when the sort is by name; Random hides it. The right gutter is an overlay and
  never a layout sibling.
- Six columns at the 76 px rail: 173 px posters, 259 px tall; 12 complete cards on screen. The
  current five columns are a sidebar artifact.
- The list view is the same data at 76 px rows (≈ 8 per screen); its job is scanning, not bulk
  browsing.

## 5. Live TV

The fetch cap is 120 channels. At the current 58 px rows about **ten fit per screen → 6 screens**
for 60 channels; the mockup's larger 84 px rows with the preview show six. The current build loops
every fetched channel and builds every row, every slice: 120 channels × 12 slices = **1,440 cells**,
each with a program lookup. That is the one screen where "just build it" is a visible cost.

- Window the guide to the **10 mounted rows**, like the library grid; keep the header and the
  scroll bar as the position cue.
- The preview panel is pinned (it does not scroll); Up/Down moves channels, Left/Right moves time,
  Enter plays the focused programme.
- Favourites (8 on the sample server) are the usable guide; "All 60" is the browse mode. Cap the
  fetched channels at 200 and say so when the server has more.
- A tuner-less server is an empty state, not an empty grid.

## 6. Detail: seasons and episodes

A 20-season, 500-episode series is ordinary for a long-running show. At 1280, **14 season chips**
fit; the current unwrapped row clips the other **6**, so Season 15–20 have no visible control.
The active season must stay visible when it is Season 18.

- Season chips: a horizontally scrollable row with an explicit **"6 more ›"** count; the active
  chip scrolls into view on open.
- Episodes: 25 per season is normal; the list is built from one 300-limit query. **Window the
  episode list to 10 rows** and keep the "23 more episodes" line; 300 is the fetch ceiling, not a
  layout target.
- A 500-episode show is 20 season fetches over its life, never one load: only the active season's
  children are queried.
- Next up stays one row; it must not become a second episode list.

## 7. Person

100 credits is 17 rows of six posters, ≈ 5 screens, and the current screen builds all 100 cards in
one pass. That is survivable at 100 and not at 300.

- Keep the server credits first ("In your library"), then the Seerr extras (24).
- Window the grid to the mounted rows, or cap the initial render at 36 with "Show all 100".
- The portrait falls back to initials; the page does not borrow a film still for a face.

## 8. Player: tracks and chapters

A remux can carry **20 audio tracks, 100 subtitles and 300 chapters**. The current chapters sheet
shows **8 and "+292 more"** with no way to reach them: a real dead end, not a scale nicety.

- One **Tracks & chapters** sheet, one tab per stream kind, a scrollable list and a position
  indicator; chapter times are the list's second column.
- Subtitles need a language filter or a search field once past ~20; audio rarely does.
- Chapter virtualization is cheap; 300 rows of 34 px is 10,200 px otherwise.
- The OSD itself stays at seven controls plus two actions; none of this belongs on the video.

## 9. The full rail

The expanded rail with 30 libraries is **34 rows** (Home, two labels, Recent 2, 28 libraries,
Settings) at 45 px = 1,530 px against a 720 px rail. About 25 fit; the rest are clipped, and flex
shrink compresses the visible rows to ≈ 28 px before it clips. A list that silently shrinks is
worse than one that scrolls.

The recommendation stands: **the 5-icon rail never lists libraries**; the hub does. If the library
flyout is enabled, it shows Recent (8), a filter field and "All libraries"; it scrolls, never
compresses, and it is a mode the reader turns on.

## 10. Empty, loading and error at every size

The same six states exist at 1 library and at 40; only the copy changes:

- **First run** — server connected, no video libraries: one line, Refresh, Server settings.
- **Empty library / empty Home rail** — the state belongs to the screen, not to a broken grid.
- **Search with no results** — quote the term, offer "Search all libraries", never a blank page.
- **Live TV with no tuner** — explain, offer Refresh/Settings; do not show an empty guide frame.
- **Downloads empty** — Browse library / Settings.
- **Person with no credits** — the name is known, the work is not on this server.
- **Loading** — a skeleton of the real layout (3 rails on Home), not a centered spinner.
- **Paging failure** — an inline row with Retry that keeps the loaded items.
- **Image failure** — the existing artwork chain (SeriesPrimary → Primary → Thumb → Backdrop) and
  the initials/placeholder fallback; never a blank card.

Every state has one primary action, one secondary, and Back always works. The state sheet capture
is the copy reference.

## 11. Budgets and testable thresholds

| Budget | Value |
|---|---|
| Home rails | ≤ 6 default; ≤ 9 with 4 explicit pins; pins replace the combined rail |
| Home first paint | ≤ 3 requests (resume, libraries, latest); rails land beside it |
| Home queries | 1 latest (or 1 per pin, ≤ 4), 1 resume, 1 next up, 1 collections, 1 live, 1 recently played |
| Mounted images | ≤ 48 per screen pass; only the rail window |
| Full Home image set | ≤ 60 at 6 rails (was ≈ 220) |
| Decoded image budget | 768 MB, ≈ 190 posters at 720 px; trim current keys |
| Hub | 18 tiles/screen at 40 libraries → ≤ 3 screens; filter reaches any in 1–2 |
| Library | 100/page, prefetch 1, 6 columns, A–Z when sorted by name |
| Guide | ≤ 10 mounted rows; fetch ≤ 200 channels; favourites pinned |
| Detail | 14 chips + scroll; ≤ 10 mounted episode rows; 300-episode fetch ceiling |
| Person | ≤ 36 mounted credits or virtualized; Seerr extras 24 |
| Player | 300 chapters reachable; 100 subtitles filterable |
| Rail | never lists libraries; if enabled, scroll + filter |

Tests that can hold this without a network (mock at `Client`):

1. Home with 12 libraries → `home_rail_keys` has ≤ 6 keys and exactly one `rail-recent-*`.
2. Home with 0 libraries → the libraries strip and the empty state, no blank rails.
3. Hub with 40 libraries → every library is present, the filter narrows to one, Recent has 3.
4. Library with 10,000 items → page size 100, header count, A–Z disabled for Random.
5. Series with 20 seasons → the chips row reports 6 more and the last season is reachable.
6. Series with a 300-episode season → mounted rows ≤ 10, all reachable.
7. Person with 100 credits → mounted ≤ 36, "Show all" reaches the rest.
8. Player with 300 chapters → every chapter index is reachable from the sheet.
9. Guide with 60 channels → mounted rows ≤ 10, the rest by scroll.
10. Rail with 30 libraries → the default rail exposes no library row; the flyout scrolls.
11. Home with 4 pinned libraries → exactly four `rail-recent-*` keys, no combined rail, four
    `latest` calls.
12. A pinned library that is empty or gone → its rail is skipped, the pin is kept, the rest render.
13. Reset Home rails → the combined rail returns and the query count is one.

## 12. Mapping to the epic

- **Phase 2 (rail/hub):** the 5-icon rail, the hub filter, Recent row, stable order; never the
  full library list in the rail.
- **Phase 3 (Home):** the six-rail budget, one combined latest, the libraries strip, 50–52 % hero,
  and the opt-in Home rails chooser (four pins, stored per device).
- **Phase 4 (library):** six columns, A–Z jumps, paging count.
- **Phase 5 (detail):** season chip scroll, episode window.
- **Phase 6 (player):** the tracks/chapters sheet.
- **Phase 7 (live/search/person/downloads/remote/settings):** guide windowing, person window,
  empty states.
- **Phase 8 (scale fixtures):** the ten tests above, run on mocked counts.

The stress captures and this note are the input to those phases; the screen spec stays the visual
contract, and this note is the cardinality contract beside it.
