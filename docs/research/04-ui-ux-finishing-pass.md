# UI/UX finishing pass — investigation and first implementation

Date: 2026-11-02. Scope: desktop Jellyfin client in Rune, default 1280×800, mouse first with
keyboard/D-pad parity. This note records the audit, the decisions taken, what shipped in the first
pass, and what remains.

## 1. Method

Two independent audits:

1. A code/snapshot audit of every Runefin screen, state path and relevant Rune API.
2. A reference audit of Plex, Jellyfin 12 Modern, Infuse, Apple TV, Kodi/Estuary, Netflix, Apple
   HIG, Material 3, Fluent and WCAG 2.2.

The committed snapshots are image-free fixtures; missing poster art is not counted as a visual
defect. Typography, spacing, surfaces, interaction states and state truthfulness are in scope.

## 2. What the references agree on

- **Keep the desktop sidebar.** Plex's 2026 redesign reversed its top-nav experiment back toward a
  persistent sidebar; Material/Fluent also support a left navigation pane at this width
  ([Plex forum](https://forums.plex.tv/t/navigation-updates-coming-to-preview-users/941751),
  [Fluent NavigationView](https://learn.microsoft.com/en-us/windows/apps/design/controls/navigationview)).
  Use 224–240 px expanded and a compact icon rail only on narrow windows.
- **Compose Home, do not dump the server.** Continue Watching first, then a small number of curated
  rails; suppress empty rows; cap the page. Plex/Emby/Jellyfin all converge here
  ([Plex recommendations](https://support.plex.tv/articles/manage-recommendations/),
  [Jellyfin 12](https://jellyfin.org/posts/jellyfin-release-12.0/)).
- **Continue Watching is the hero on desktop.** A full-bleed streaming-service hero costs 35–45% of
  a 1280×800 viewport and pushes the useful row below the fold. Netflix's larger hero is a TV
  pattern, not a desktop one ([Vulture](https://www.vulture.com/article/why-netflix-had-to-redesign-its-homepage.html)).
- **Cards need a state contract.** Hover/focus reveals actions; metadata is one title plus one line;
  progress and watched state are not colour-only. Apple requires focused items to stand out without
  crowding neighbours ([Apple focus](https://developer.apple.com/design/human-interface-guidelines/focus-and-selection)).
- **The player gets out of the way.** Auto-hide the OSD while playing, never while paused, keep
  keyboard transport, expose audio/subtitle tracks quickly, and return to a relevant detail screen
  ([Apple playing video](https://developer.apple.com/design/human-interface-guidelines/playing-video),
  [Jellyfin 12](https://jellyfin.org/posts/jellyfin-release-12.0/)).
- **Focus is a first-class API, not a scale effect.** Rune already provides `focusable`,
  `focus_group`, `focus_ring`, `focus_highlight`, `default_button` and `tab_visits_controls`; the app
  was not using them. WCAG 2.4.13 requires a 2 px perimeter and 3:1 change
  ([W3C](https://www.w3.org/WAI/WCAG22/Understanding/focus-appearance.html)).
- **States are product surfaces.** Skeleton geometry must match the final layout, appear after
  150 ms, and empty/error states need a corrective action
  ([NN/g skeletons](https://www.nngroup.com/articles/skeleton-screens/),
  [NN/g empty states](https://www.nngroup.com/articles/empty-state-interface-design/)).

## 3. Current audit — the prototype tells

The full audit found these highest-impact gaps:

1. **No app-owned type/space/radius scale.** Rune's 11.5–13 px defaults were used everywhere;
   page titles ranged from 13 to 40 px; spacing and radii were ad hoc. The design document promised
   16 px body / 32 px page / 44 px hero and 4/8 px spacing.
2. **No focus contract.** Cards, nav items, toolbar chips, season chips, episode rows, remote
   controls and settings actions were not focusable. Tab did not visit controls because the app
   never opted in.
3. **Cards had no interaction state.** The most-clicked surface had no hover, press or focus cue;
   library list rows, detail episode rows and search people chips looked interactive but did nothing.
4. **Home led with the server name** and repeated “Continue watching” in the header, subtitle and
   rail title. Episode cards repeated the series name in both title and subtitle.
5. **Detail was content-incomplete and hard to read.** 13 px overview at 140+ characters; a single
   vertical scrim; tiny episode progress slivers; no cast/next-up; “More” shown even on short text.
6. **Settings and Remote read as debug pages.** Full-width identical bars; destructive sign-out
   indistinguishable; remote controls the same fill as their row; no max width or scroll.
7. **Player chrome was a toolbar.** Permanent control panel, text-only transport, non-interactive
   progress, invisible Cancel on the launching screen, a 100% progress bar at 87.5%.
8. **Truthful states were missing.** Library could say “No items” while showing an error; episode
   failures replaced the whole detail page; search errors had no retry; skeletons flashed and did
   not match final geometry.
9. **Responsive layout was fixed at 1280.** Five fixed 172 px columns and no max widths; Settings and
   Remote could overflow without scrolling.

Preserve: the shell structure and dark mood, card geometry/image pipeline, Home CW merge semantics,
library virtualization/paging, shared-element poster, resume label, search cancellation, playback
reporting, stable keys and the headless test infrastructure.

## 4. First pass shipped

This pass establishes the visual and interaction foundation without rewriting data flow.

- **`theme.rs`**: DISPLAY 44/700, PAGE 32/700, SECTION 20/600, CARD_TITLE 14/600, BODY 16/400,
  META 14/400, LABEL 14/600, CAPTION 12/500, OVERVIEW 15/400; 176×264 cards, 16 px gaps, 40 px
  rail/page gutters, 224 px sidebar; semantic radius and focus tokens. All screens now use it.
- **Focus contract**: `App::tab_visits_controls(true)`; nav items, cards, primary/back buttons,
  toolbar chips, library rows, letter rail, season chips, episode rows, settings/remote/player
  controls have a 2 px white ring outside the artwork; rails/toolbars/control groups are
  `focus_group`s. Press scales give click feedback.
- **Collapsible sidebar**: an expanded 224 px rail and a 72 px icon rail with tooltips, a persistent
  user chip, and a real toggle; the content relayouts rather than squeezing.
- **Iconography**: a small embedded SVG catalog for Home, Search, Remote, Settings, Library,
  play/pause, seek, volume/mute and stop; the sidebar is no longer a text list.
- **Mockup pass**: a cool navy shell, accent-gradient wordmark and title, glowing selected nav,
  conditional nav shadows, card/backdrop elevation, and the first Continue Watching card carrying
  an accent edge/shadow as the current item.
- **Home hierarchy**: “Continue watching” is the page title; the server is a quiet status chip;
  the rail is “In progress”; 16:9 Continue Watching cards use the item backdrop (poster fallback),
  with title, episode label and progress over a dark gradient; the latest rails are capped at four
  and carry a “See all” link;
  episode subtitles read `S2E04 · The After Hours`; 40 px gutters/rail gaps.
- **Detail**: two-axis scrim (vertical + left-to-right), 32 px title, readable 15/23 overview with a
  720 px measure, conditional More, larger primary action, neutral season selection, interactive
  episode rows that start playback.
- **Library**: back control above the title so the heading and grid align; measured responsive
  grid columns (3–7), 1024 and 1440 snapshots; clickable list rows; larger letter targets;
  focus/selection states.
- **Settings/Remote/Search**: max-width content, grouped settings cards with visible secondary
  buttons and destructive sign-out treatment; remote controls separate from the row fill; search field capped
  at 640 pt, with an idle suggestions state and no-results recovery.
- **Now-playing/player**: a centered 980 px playback card, visible secondary controls, progress
  fill fixed as two grow segments (no more 100% at 87.5%), launching Cancel visible, icon transport,
  and controls that auto-hide after three seconds while playing (pointer movement or any key
  reveals them; paused keeps them).

## 5. Next passes

> The Moonfin desktop teardown in [`05-moonfin-linux-desktop.md`](05-moonfin-linux-desktop.md)
> turns these into a ranked desktop roadmap. Its main additions are the ambient home backdrop,
> item logos, cast/next-up, the player OSD, the mini player, and window-level desktop integration.

1. **Home composition**: merge Recently Added across libraries, cap at five rails, 16:9 Continue
   Watching cards, “See all”, empty-rail suppression and a CW-first hierarchy.
2. **Sidebar**: collapsed 64 px mode, icons, overflow “More” for >5 libraries, user/account menu,
   search row with `Ctrl K`, remove the orphaned bottom sign-out.
3. **Player OSD**: auto-hide 3 s while playing, never while paused; full-bleed control gradient;
   interactive scrub with chapter ticks; audio/subtitle popovers; `?` shortcut sheet; buffering and
   “still watching”; `F` fullscreen.
4. **Library/search completeness**: adaptive 4–6 columns via layout reads, sticky toolbar, filter
   chips with Clear all, letter rail only when valid; search recents, scope chip, group counts,
   See all, no-results actions.
5. **Detail completion**: cast rail, next-up callout, episode thumbnails, watched check, sticky
   compact header, movie vs series variants.
6. **State system**: delayed geometry-matching skeletons, global offline banner, error taxonomy,
   retries, toasts with undo.
7. **Accessibility/responsive verification**: 1024×768 and 1440×900 snapshots, keyboard traversal,
   tooltips for truncation, contrast audit, reduced-motion.

## 6. Decisions

- No full-bleed hero at 1280×800. Continue Watching is the anchor.
- No Netflix-style sibling push in a virtualized grid; interaction feedback is overlay/transform,
  not layout.
- No resume/start-over modal; the primary label carries state.
- One accent action per screen; selection is neutral/tinted, not solid accent everywhere.
- Monospace only for timecodes, ids and paths, not ordinary metadata.
- No new dependency for this pass; Rune and the existing image/player stack are enough.

## 7. Acceptance

- 1280×800 snapshots for every screen and state, plus 1024/1440 when responsive work lands.
- `cargo test -p runefin`: stable keys and interaction tests remain green.
- Keyboard: Tab reaches every control; focus ring is visible on dark and light artwork; Esc always
  returns to the previous context.
- No state can contradict itself (“No items” while erroring; 100% at 87.5%; loading label twice).
- Screenshots are reviewed as product surfaces, not just compilation artifacts.
