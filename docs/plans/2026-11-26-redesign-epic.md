# Runefin redesign epic — modern TV UI

Status: proposed · 2026-11-26 · owner: Design + TV surfaces
Research: [`docs/research/28-modern-tv-ui-2026-11-26.md`](../research/28-modern-tv-ui-2026-11-26.md)
Mockups: three directions evaluated (Nocturne, Marquee, Big Picture); **Nocturne is the
chosen base**. The mockups are evaluation artifacts from this session; the token/layout spec
below is the durable contract.

---

## 1. Goal

Make Runefin read as a modern 10-foot application, not a desktop app in a TV frame. Keep the
functional base (Jellyfin client, playback, focus model, offline downloads) and replace the
visual language, navigation shell and screen compositions.

**Success looks like:** a neutral graphite surface where artwork supplies the colour; a
five-destination rail instead of a 15-row sidebar; a 60 % billboard that leads into six-up
16:9 and six-up poster rails which fill the width; a library that shows six complete columns
with no dead band; a detail page built around a backdrop and a logo; a calm player OSD; every
view reachable with the D-pad and every focused control visible.

**Non-goals:** changing the Jellyfin API layer, adding a second render stack, a Rune fork
redesign, HDR10/PQ output, a new dependency. Rune work is limited to what the research lists as
gaps only if a phase cannot ship without it.

## 2. Principles

1. **Neutral chrome, artwork colour.** No accent-tinted surfaces. Blue (or cyan) only for focus,
   progress, and at most one primary action per screen.
2. **Five destinations, then a hub.** Home, Libraries, Live TV, Search, Profile. Every server
   library and every tool lives one level down in a hub, not in permanent chrome.
3. **Full width, deliberate density.** At the 1280×720 design: content width 1092 pt after a
   76 pt rail and 56 pt gutters. Six complete poster columns and six 16:9 cards. No one-card
   dead band on the right; a rail with fewer items ends at its last card, not at a fixed column.
4. **Hero at 60 %, never 90 %.** The next rail is visible above the fold.
5. **One focus language.** Scale + lift + a light edge; the rail and cards share it. No rings
   that vanish on art, no four-bar halo workaround in the new shell.
6. **Artwork first.** Backdrop + clearlogo on Home/Detail; poster crops consistent; one static
   tint/blur while loading; no per-tile spinners.
7. **Reachable by contract.** Every control has a key and a name; Back is one level then nav;
   focus is restored on return. The existing `tv_reach_tests` grow with each screen.
8. **Calm motion.** 120–200 ms springs, focus-only; Reduce Motion respected.

## 3. Design tokens (proposed)

| Token | Value | Use |
|---|---|---|
| `bg` | `#0A0C0F` | app background |
| `surface` | `#12161B` | rails/panels, one step above bg |
| `raised` | `#191D23` | dialogs, menus, focused cards |
| `hairline` | `rgba(255,255,255,.08)` | separators, 1 px |
| `text` | `#F4F6F8` | primary |
| `muted` | `#98A2AC` | secondary/meta |
| `dim` | `#6E7882` | disabled/tertiary |
| `accent` | `#38BDF8` | focus edge, progress, one action |
| `accent-soft` | `rgba(56,189,248,.16)` | focus wash, selected underlay |
| `danger` | `#F87171` | destructive only |
| radius | 8 / 12 / 16 | cards / panels / hero |
| elevation | `0 1 3`, `0 8 24 -2`, `0 20 56 -8` | contact, hover, focused |

No blue is mixed into `bg`/`surface`/`separator` (today it is 2.8–4 %). The Jellyfin cyan in the
logo stays a brand mark; the UI accent is the lighter `#38BDF8` used as light, not fill.

**Type (design points, big screen).** `DISPLAY 48/700/-0.03`, `PAGE 32/700/-0.02`,
`SECTION 22/650/-0.01`, `CARD_TITLE 15/600`, `BODY 15/450`, `META 12/450` with `tnum`.
The current big-screen override flattens card title/label/caption/meta to 16; stop doing that.

**Spacing.** 4/8/12/16/24/32/48. Gutter 56 (big screen), rail gap 32, card gap 16 (12 for 16:9
rows). Safe area from `Cx::safe_area`.

## 4. Navigation and reachability

- **Rail:** 76 pt, icon-only, labels on focus; Home, Libraries, Live TV, Search, Profile.
  Selected = 8 % white fill + 1 px 12 % border, no accent fill.
- **Libraries hub:** a full-width grid/list of every server library (video only), the user's
  playlists, Watchlist, Downloads, Remote, Settings, Seerr if configured. This replaces the
  long sidebar and makes the hidden/filtered items explicit and reachable.
- **Back:** one level up; from a top-level page Back focuses the rail; from the hub Back goes
  Home. The rail's focused row is scrolled into view (the current `tv_focus::in_sidebar` +
  scoped scroll work is the seed).
- **Focus:** restore the last item on return to a screen; never leave focus on a hidden key.
  The sidebar focus zone must be generated from the same filtered list the rail renders (the
  bug fixed in this session is the template).

## 5. Screen by screen

**Home.** 60 % billboard: backdrop + clearlogo (fallback title), one metadata line, Resume +
Details, left/bottom scrim. Below: Continue Watching as six 16:9 cards (166×93, 12 gap),
progress bar, `S2E04 · 12 min left`; Next Up / Recently Added as six 2:3 posters (168×252).
Section heads 22/650 with a muted count; “See all” as a quiet text action. Artwork-derived
wash at 8 % behind the page. No chips, no outlined panels.

**Library.** Full-width poster grid, six columns (168×252) at 1280 design; the A-Z rail becomes
an overlay in the right gutter, not a layout sibling. One compact toolbar row: search, sort,
direction, view toggle, filters. Active filter = text + underline/soft fill, not a blue pill.
List mode: 16:9 thumb + title/meta + resume. Empty/error states = one sentence + one action.

**Detail.** 60 % backdrop with a two-axis scrim, clearlogo/title, one metadata line
(`2023 · 12 · 92 min · 4K HDR · IMDb 5.4`), Resume/Play + Trailer/Favourite, then seasons (tabs)
and episodes (16:9 stills), cast (1:1), related (2:3). No left-column-with-empty-right layout.
The page scrolls under the hero; Back sits above the scroll.

**Player OSD.** Bottom gradient only; scrubber with chapter ticks and trickplay; play/pause,
±10 s, audio/subs, gear; auto-hide 3–5 s; video dims 40–60 % while visible; Up opens chapters
and tracks; no persistent chrome.

**Live TV.** Two-pane: channel list + now/next with a guide timeline filling the width; program
blocks with progress; Watch as the focused action. No narrow column with a dead middle.

**Remote.** Session cards as rows with client, user, now-playing and transport; idle sessions
show Play/Stop only, not Pause/+30 s.

**Downloads.** Storage summary + rows with poster, title, size, state and actions; the folder
path is a setting, not a banner; empty state explains the action.

**Search.** Full-width field, recent searches, results in mixed rails (films, series, episodes);
the keyboard opens from the field's action (already fixed), never on focus.

**Settings.** Two-pane on TV: sections list + detail; no wall of full-width rows; the
on-screen keyboard mode (Auto/On demand/Off), theme, text scale, HDR, downloads, account.

## 6. Phases

### Phase 0 — Artwork reliability (this session, shipped)
`home::poster_source` (SeriesPrimary → Primary → Thumb → Backdrop) and `own_image_source` for
episode stills, every key and request uses the same kind, resume fallback, image worker
validates before caching and deletes a bad disk entry, fallback on any failed first URL, the
2x width cap and 768 MB decoded budget, `current_images` is rebuilt per screen pass and pins
the visible set through trim, Live TV/detail extras re-request on scale change, tests.

Follow-ups: a **negative cache** for 404/400 per session, a bounded retry with backoff for a
transient failure, a static colour/blur placeholder while loading (the letter is still the
only in-flight state), a now-playing poster request on playback start, retiring a failed Rune
decode handle, and queue cancellation on route/scale change.

### Phase 1 — Tokens and type (1–2 commits)
Rewrite `theme.rs` `colors()` to the table above; remove the accent mix from surfaces; add
`DISPLAY`, `PAGE`, `SECTION`, `CARD_TITLE`, `BODY`, `META`; stop the big-screen flattening;
add elevation/radius tokens. Update `docs/design.md` §5 and the snapshots. Acceptance: no
screen has more than one accent-filled surface; the snapshots read neutral.

### Phase 2 — Shell and rail (2–3 commits)
Replace the 224 pt sidebar with the 76 pt rail; add the Libraries hub route/list; move
Settings/Downloads/Remote/Playlists/Seerr to the hub; update `tv_focus` zones to match the
rendered list; scoped scroll; Back contract; reachability tests. Acceptance: every existing
view is reachable in ≤3 D-pad steps from Home and Back restores the rail; no hidden focus keys.

### Phase 3 — Home (2 commits)
60 % billboard, clearlogo/title, metadata line, actions; six-up 16:9 Continue Watching with
progress; six-up 2:3 Next Up; Recently Added; section heads; artwork wash. Acceptance: the
rails fill the width at 1280 and 4K; the first rail is visible under the hero; snapshot pair.

### Phase 4 — Library (2 commits)
Six-column grid at 1280 (full width), A-Z overlay, one toolbar row, text/underline filters,
list mode, empty/error states. Acceptance: six complete columns at 1280, eight at a 1920
design if the design height changes; no card under the rail; the toolbar is one row at 720.

### Phase 5 — Detail (1–2 commits)
Hero, logo, meta line, actions, seasons/episodes, cast, related; full-bleed; no empty right
column. Acceptance: the page fills the width; every control is reachable; the backdrop is
cached and does not flash.

### Phase 6 — Player OSD (1–2 commits)
Bottom gradient, chapter ticks, trickplay, tracks, gear; auto-hide; dim; Up opens chapters.
Acceptance: the OSD is legible over bright and dark frames; every control reachable; the
video is untouched when the OSD is hidden.

### Phase 7 — Secondary screens (2 commits)
Live TV two-pane, Remote transport, Downloads storage, Search results, Settings two-pane,
Seerr. Acceptance: no narrow column with a dead middle; consistent section heads/cards.

### Phase 8 — Motion, accessibility, performance (1 commit)
Focus springs, press scale, Reduce Motion, contrast checks on the new palette, image cache
budgets, snapshot refresh, `scripts/check.sh`, `--all` snapshots reviewed.

## 7. Acceptance for the epic

- A 10-foot reviewer can name the focused item from three metres in every screen.
- No screen uses accent fill on more than one control; surfaces are neutral.
- At 1280×720 design, no horizontal dead band larger than one card gap; rails end at the last
  card or are intentionally full.
- Every view reachable by D-pad; Back is one level then nav; focus restored; no invisible focus.
- Artwork: no letter placeholder for an item that has any of SeriesPrimary/Primary/Thumb/
  Backdrop; a failed image retries instead of sticking; a broken cache entry self-heals.
- Player OSD hides completely; Snapshots updated; `check.sh` green.

## 8. Decisions taken

- **Nocturne**, not Marquee/Big Picture. It is the most space-efficient with mixed Jellyfin art
  and the easiest to keep reachable. Borrow Marquee's typography/hero discipline and Big
  Picture's focus physics.
- **Icon rail + Libraries hub**, not top tabs. Five destinations are not enough for a server
  with many libraries; a hub keeps the rail small and every library reachable.
- **Six columns at the 1280 design**, not four. The current four come from the 224 sidebar and
  64 gutters; the rail/hub and 56 gutters free two columns. An eight-column layout needs a
  1920 design width, which is not the current contract.
- **Keep the 720 design height**; 4K is a 3× scale, not a separate layout.
- **No Rune fork work in this epic** unless a phase is blocked; the research lists the gaps
  (image filters, text shadow, conic gradients, scroll-linked motion).
- **A 16:9 fallback is cropped, not final.** An item with no Primary now uses its Thumb or
  Backdrop, but a 2:3 card crops a 16:9 still and cuts the artwork's own title (28 Days Later
  is the live example). Phase 4/5 should draw the fallback fit over a blurred copy of itself,
  or use the backdrop hero on Detail, instead of a hard 2:3 crop. The letter is gone; the crop
  is the remaining polish.

## 9. Risks

- Mixed poster/still aspect ratios on cards without Primary: the new fallback crops a 16:9
  still to 2:3; acceptable but watch faces. A future server-side `fillWidth/fillHeight` or a
  per-kind crop anchor can improve it.
- The A-Z overlay over a grid must not steal D-pad focus from the last column; test it.
- Backdrop blur is capped at four per frame; do not use it on every card.
- The Steam tile art/duplicate and `runefin-mode` switch are outside this epic; they remain
  machine follow-ups in the research note.
