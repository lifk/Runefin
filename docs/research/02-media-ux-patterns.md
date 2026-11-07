# Media library UX patterns — brief for Runefin v1

Scope: information architecture and interaction mechanics in Plex (desktop/web), Emby, Kodi (Estuary), Infuse, Apple TV app, Jellyfin Web. Styling is out of scope. Ends with a proposed IA, screen requirements, and the riskiest assumptions.

## 1. Home composition

All six converge: a small fixed number of horizontal rails, "in progress" pinned near the top, the rest recency-driven. They differ in who owns ordering and how rows degrade.

**Plex** is the most server-controlled. Admins choose which rows appear per library (`Settings → Server → Manage → Libraries → Manage Recommendations`), with three columns — Library Recommended / Home / Shared Users' Home — reorderable by drag. Two rules matter: rows from a pinned library are **grouped in sidebar-pin order**, and **empty rows are suppressed**. Continue Watching is the exception: always shown, admin cannot disable it ([Plex support](https://support.plex.tv/articles/manage-recommendations/)).

**Emby** goes further: unlimited drag-reordered home sections, scoped to libraries, buildable from playlist/collection/genre/studio/tag — and it deleted the standalone "Next Up" section, folding it into Continue Watching via "Include next up episodes" ([Emby docs](https://emby.media/support/articles/apps/common/User-Prefs-HomeScreen.html)).

**Jellyfin Web** gives up to 10 user-assigned slots from a fixed palette: library tiles, library buttons, Continue Watching, Continue Listening, Continue Reading, Next Up, Latest Media, Live TV, Active Recordings, none — defaults put resume first, then next-up, then latest. A TV-layout special case force-prepends library tiles so libraries stay reachable ([jellyfin-web internals](https://deepwiki.com/jellyfin/jellyfin-web/3.5-home-sections-and-library-navigation)).

**Kodi/Estuary** inverts it: a skin-fixed vertical category spine (Movies, TV shows, Music…) where each category owns a stack of widget rows; `Home.xml` declares a separate group-list per category id ([skin.estuary Home.xml](https://raw.githubusercontent.com/xbmc/xbmc/master/addons/skin.estuary/xml/Home.xml)). **Infuse** collapses to one dominant list — "the main Up Next List on the home screen" plus recently added ([Infuse settings](https://support.firecore.com/hc/en-us/articles/360015608854-Settings-Overview)). The **Apple TV app** does it at OS scale: one cross-service Up Next queue, then per-service rows ([Apple TV guide](https://support.apple.com/guide/tvapp/)).

**Continue Watching semantics.** The client does not compute this. Jellyfin's `/Items/Resume` returns items carrying a playback position, and the server's only knobs are thresholds — `MinResumePct = 5`, `MaxResumePct = 90`, `MinResumeDurationSeconds = 300` — with **no expiry field at all** ([ServerConfiguration.cs](https://raw.githubusercontent.com/jellyfin/jellyfin/master/MediaBrowser.Model/Configuration/ServerConfiguration.cs)). That absence is why Jellyfin CW lists go stale and users complain. Plex orders by recency and lets users fight the hub, a symptom of opaque ordering ([Plex forum](https://forums.plex.tv/t/continue-watching-mod-enable-disable-hub/913734)).

**Decision:** order by last activity desc, dedupe to one card per *series* (the in-progress episode, not the show), hide entries untouched for 60 days *client-side* without touching server state. Two rails duplicate the same shows; take Emby's merge for v1 and use Next Up only to backfill when CW is short.

## 2. Library browse

Grid is the default everywhere; list is the density escape hatch. Jellyfin Web splits cards (home rails, poster grids) from list view (episodes, tracks, search results), with **index headers derived from the sort key** — first letter or `#` for `sortname`, rating bands for `officialrating`, disc numbers for `disc` ([cards & list views](https://deepwiki.com/jellyfin/jellyfin-web/3.4-cards-and-list-views)). Those headers are the data structure letter-jumping needs; nobody invents a separate alphabet index.

Pagination is where clients fail. jellyfin-vue's library view "loads every item, resulting in a very large DOM and really bad performance", fixed by a virtual scroller ([issue #59](https://github.com/jellyfin/jellyfin-vue/issues/59)). At 1k–50k items the only defensible model is windowed fetch (`StartIndex`/`Limit`, 100–200 per page), fixed-row-height virtualization, one page prefetched, image decode off the UI thread. A 50k poster grid is ~2.5k rows; laying them all out will hitch even in wgpu.

**Decision:** virtualized grid, fixed row height (poster aspect is fixed, trivial in Taffy), 100-item pages, prefetch 1. Letter rail only when sorting by name, jumping via a cached `(id, sortName)` projection binary-searched per letter — cheaper and more accurate than 26 count queries. Collections are a **filter facet**, not a tree: keep series→season→episode as real hierarchy and stop collections polluting the grid the way Plex and Kodi allow. Sort/filter live in a persistent toolbar (not a modal), remembered per library, executed server-side above ~2k items.

## 3. Detail pages

Jellyfin's field order is defensible: title/breadcrumb, primary metadata (year, runtime, rating, production location), secondary technical line (resolution, codec, bitrate), genres as links, clamped overview with expand, tags, external links, and a play button that flips between *Resume* and *Play from start* on `PlaybackPositionTicks` ([item details](https://deepwiki.com/jellyfin/jellyfin-web/3.2-item-details-and-navigation)). Its Series ▸ Season ▸ Episode breadcrumb is the right hierarchy signal and costs nothing on movies.

Season chips (Specials, S1…Sn) beat a flat 300-episode list; a flat list is only acceptable when the user arrived from a specific season. Episodes are list rows — thumb, number, title, runtime, watched tick, progress bar — virtualized.

**Next up on a series page** must be computed locally: first episode with `UserData.Played == false` after the highest-indexed played episode, skipping unaired by `PremiereDate`, else S1E1. `/Shows/NextUp` agrees for in-progress series but returns nothing useful for a series you've never opened, so the local rule is the fallback.

**Decision:** hero backdrop, one primary button whose label carries state ("Resume 42:13" / "Play"), seasons as chips, virtualized episodes, cast next, technical info collapsed, trailer and "more like this" last. No tabs.

## 4. Search

Two surfaces, not one: global from the shell (`/` or Ctrl+K) and scoped inside a library (Ctrl+F), which pre-applies the filter and shows scope as a removable chip. Global search debounces at 150 ms, minimum 2 characters, and **cancels in-flight requests per keystroke** — on a LAN this is latency-dominated, so cancellation matters more than the debounce value.

Results group by type in fixed order: Movies, Shows, Episodes, People, Collections. Cap each group at 6 with "see all" opening a filtered grid; never paginate inside the dropdown. Empty state shows recent searches and top genres — a blank rectangle after typing is the most common search failure. No-results must state what was searched and offer scope widening ("search all libraries").

## 5. Focus and navigation

Mouse+keyboard first, but must not become D-pad-hostile since remote mode is roadmapped. One model serves both: a **single focus ring**, roving focus inside containers, spatial arrows in grids, Tab between regions (sidebar → toolbar → grid → panes). Apple's HIG is explicit that focus must always be visible and that navigation is driven by a focus engine with predictable directional movement ([HIG: focus and selection](https://developer.apple.com/design/human-interface-guidelines/focus-and-selection), [designing for tvOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-tvos)).

Back is a real navigation stack: Esc pops one level and restores the *exact* prior state — scroll offset and focused item id. Browsers get that free from bfcache; native apps implement it ([web.dev: bfcache](https://web.dev/articles/bfcache)). Store `(route, scrollOffset, focusedItemId)` per stack entry and restore before first paint, or the grid visibly jumps. Focus traps only in modals, and Esc always closes them.

Breadcrumbs in library context only (Movies ▸ Genre: Sci-Fi ▸ Dune); home is the root and has none. The sidebar must not highlight a library while the user sits on a detail page reached from home, or the two nav states contradict each other.

## 6. Loading, empty, error

- Skeletons after a 150 ms delay, never instantly (LAN responses would flash), matching final geometry — placeholders set layout expectations and cut perceived wait ([NN/g](https://www.nngroup.com/articles/skeleton-screens/)).
- Spinner only for indeterminate *actions* (cast, handoff, refresh), never for content.
- Empty library explains itself: no items matched the filter vs the library is empty, each with the corrective action.
- Server unreachable: persistent banner, cached catalog still browsable, playback disabled with a reason. Never a blank screen.
- Long-running states: with no decoder, transcoding isn't ours to show. Surface handoff instead — *Launching mpv…*, *Playing on Living Room TV*, *Session ended*. Scan/probe status is the server's, in a small status strip.

## 7. Settings split

Client: grid density, poster size, theme, default sort per library, mpv path/args, cast target, cache size, shortcuts. Server, never duplicated: libraries, users, metadata providers, transcoding, tasks, API keys. Plex, Emby and Jellyfin all keep library/user/metadata config server-side; Infuse shows the failure mode of the alternative — it syncs watched history through iCloud when pointed at raw files, deferring to Plex/Emby/Jellyfin only when those are the source ([Infuse](https://support.firecore.com/hc/en-us/articles/360006462093-Streaming-from-Plex-Emby-and-Jellyfin)). The server owns watch state, full stop.

## 8. Proposed IA for v1

- Home — Continue Watching (merged with Next Up) · Recently Added (per pinned library, pin order) · Recently Played · Collections
- Library `/library/:id` — toolbar (sort, facets, grid/list, letter index) → grid → detail; collection → detail
- Series detail `/item/:id` → season chips → episodes → episode detail
- Movie detail `/item/:id` · Person `/person/:id`
- Search `/search?q=` → grouped results → "see all" filtered grid
- Cast `/cast` — target picker, live session status
- Settings — Appearance · Playback & Player · Network & Server · Cache · Shortcuts · About
- Shell — sidebar (libraries, search, settings), status strip (server, session), toasts

## 9. Screen-level requirements

| # | Screen | Requirement | Derived from |
|---|---|---|---|
| R1 | Home | ≤5 rails, CW first, order fixed in v1; reordering deferred | Plex/Emby row config is a v2 feature |
| R2 | Home | CW deduped per series, last-activity order, stale >60 d hidden client-side | Jellyfin has no expiry |
| R3 | Home | Rail skeleton matches final card geometry, appears at 150 ms | NN/g skeleton guidance |
| R4 | Library | Virtualized grid, fixed row height, 100-item pages, 1-page prefetch | jellyfin-vue #59 |
| R5 | Library | Sort/filter persist per library; letter rail only for name sort | jellyfin-web index headers |
| R6 | Library | Back from detail restores scroll offset and focused card | bfcache/state stack |
| R7 | Detail | One primary button, labelled with resume position or "Play" | jellyfin-web play/resume split |
| R8 | Detail | Season chips; virtualized episodes; Series ▸ Season ▸ Episode breadcrumb | jellyfin-web hierarchy |
| R9 | Detail | Next-up computed locally, skips unaired, falls back to S1E1 | /Shows/NextUp is empty for unstarted series |
| R10 | Search | 150 ms debounce, 2-char min, per-keystroke cancellation, groups capped at 6 | latency-dominated LAN search |
| R11 | Shell | Esc pops the stack; modals are the only focus traps | HIG focus model |
| R12 | Shell | Unreachable server = banner + cached browse + disabled playback | degrade-to-catalog |

## 10. Riskiest assumptions

1. **Users want one merged Continue Watching + Next Up rail.** Emby merged; Plex and Jellyfin kept both. If wrong, users lose the mental split between "half-watched" and "next episode". *Test:* two-rail vs one-rail prototype, 5 users, 30-item history, measure time-to-resume; ship behind a build flag.
2. **A virtualized fixed-row grid feels instant at 20k items.** Virtualization fixes layout, not image or metadata cost. *Test:* synthesize a 20k fixture or point at a real library, log frame times while keyboard-flinging and scrollbar-dragging; fail on any frame >16 ms or placeholder gaps >200 ms.
3. **Focus/scroll restoration is invisible when right and fatal when wrong.** Nobody files it as a bug; they stop using back. *Test:* instrument every pop with restored-vs-reset counts and run a 10-iteration scripted loop (open grid → open item → Esc). Any reset blocks release; no study needed.

### Where sources disagree, and what I'd pick

- **Configurable rows vs fixed order:** Plex/Emby make it admin config, Jellyfin fixed slots. Fixed order for v1 — the settings UI costs a week that buys nothing until 3+ libraries exist.
- **Empty rows:** Plex suppresses, Jellyfin sometimes renders them. Suppress; an empty rail reads as a bug and burns a scroll stop.
- **One aggregate queue (Apple, Infuse) vs per-library rails (Plex, Emby):** per-library — on a small local server "Recently Added in Movies" beats one merged list.
- **Kodi's fixed spine** fits remote mode, not mouse-first desktop: keep the sidebar but make it collapsible.
