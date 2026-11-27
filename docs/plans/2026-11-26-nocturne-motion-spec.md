# Nocturne motion spec — every scene

Status: proposed · 2026-11-26 · the scene-by-scene companion to the implementation plan.
Inputs: the 15 Nocturne captures, the 11 stress captures, the Home rails chooser, the screen
spec's motion ladder and the Rune motion API at the pinned rev.

Motion is not decoration. On a TV it is the answer to "where did that come from, and how do I get
back?" Every transition below either shows a spatial change, confirms an input, or covers a wait.
Nothing else animates.

---

## 1. Principles

1. **Movement needs a cause.** A page, a sheet, a card becoming a hero, an item arriving. No
   ambient movement, no floating, no pulsing chrome.
2. **One hero move per transition.** The poster flight, the page rise, the rail expansion. Every
   other element fades, settles or follows on the same spring.
3. **Short and decisive.** Focus 160 ms, page 180, detail 260, OSD 120. Nothing interactive waits
   longer than 260 ms to become readable.
4. **No bounce except the detail flight.** Focus never overshoots. The shared element may carry
   bounce `0.05`; everything else is critically damped.
5. **Data does not move layout.** Server results, appended pages, image arrivals and progress
   values fade or tween in place. The page must not jump under the reader's thumb.
6. **Interrupt, do not queue.** A second D-pad press retargets the focus/scroll spring; Back during
   a page open reverses it; a second OSD key retargets show/hide. No animation queue.
7. **Direction means something.** Forward rises from below; Back settles from above; a tab
   crossfades; a sheet scales from the screen; a menu scales from its control; a player fades to
   black. No full-screen slide on every route.
8. **Reversible and reduced.** Every entrance has a matching exit. Reduce Motion keeps the state
   change and drops the travel: 80–100 ms fades, instant scroll, no matched flight, no scale.
9. **60 fps, no exceptions.** No per-card blur, no 100-card stagger, no layout animation of a
   full library, no parallax on a 4K surface. Blur is for at most four backdrop layers.
10. **Readability wins.** Subtitles, clocks, timecodes and text reflow do not animate. A progress
    bar may move; a label may not slide under the eye.

---

## 2. Motion tokens

Rune's `Spring(duration, bounce)` reads as a perceptual duration in seconds. Presence exits run
10–25 % shorter than entrances. The framework's default presence is 0.30 s enter / 0.24 s exit;
**every scene sets its own transition**, or the app feels 120 ms slower than the design.

| Token | Value | Use | Reduce Motion |
|---|---|---|---|
| `HOVER` | `0.16, 0.0` | card focus scale, ring, lift | 80 ms ring only |
| `CONTROL` | `0.18, 0.0` | button/chip/switch colour and fill | instant |
| `RAIL` | `0.20, 0.0` | rail width, scrim, labels | instant width, labels on |
| `PAGE_IN` | `0.18, 0.0` | page enter | 90 ms fade |
| `PAGE_OUT` | `0.14, 0.0` | page exit | 90 ms fade |
| `GLIDE` | `0.22, 0.05` | scroll-to-focus | instant |
| `DETAIL` | `0.26, 0.05` | matched poster/portrait flight | 100 ms fade, no flight |
| `OSD` | `0.12, 0.0` | OSD show/hide, dim | 80 ms |
| `SHEET` | `0.16, 0.0` enter / `0.13` exit | sheets, menus, keyboard | 80 ms fade |
| `TOAST` | `0.16, 0.0` enter / `0.12` exit | toast, undo | instant |
| `LAYOUT` | `0.20, 0.0` | input-caused reflow (rails, filters, reorder) | instant |
| `PROGRESS` | `0.28, 0.0` | progress width, download percent | instant |
| `IMAGE` | tween `0.20` ease-out | image arrival, hero backdrop | 80 ms |

**Pairs** for presence: `PAGE = SpringPair::new(PAGE_IN, PAGE_OUT)`, `SHEET = (0.16, 0.13)`,
`OSD = (0.12, 0.10)`, `TOAST = (0.16, 0.12)`. Scroll requests use `scroll_to_with(..., GLIDE)`;
Rune's default `GLIDE` is 0.32, so the app passes its own.

**Geometry:** page rise 8 pt forward / 6 pt back; sheet rise 8 pt + scale 0.98; menu scale 0.96
from its anchor; toast 10 pt; OSD 8 pt; scroll lead 48 pt for rails, 24 pt for grids; focus scale
1.05 rails/cards, 1.03 grids/hub, 1.06 rail icons, 1.08 cast faces and OSD transport.

---

## 3. Global systems

### 3.1 Page transitions

Add `NavKind { Push, Pop, Tab, Overlay, Player }` to the state, set by every opener and by Back.
The screen root reads it; the transition is authored once, not per screen.

| Kind | New page | Old page | Time | Reduce |
|---|---|---|---|---|
| `Push` | fade 0→1 + 8 pt up | fade 1→0 + 4 pt up | 180 / 140 | 90 ms crossfade |
| `Pop` | fade 0→1 + 6 pt down | fade 1→0 + 6 pt down | 180 / 140 | 90 ms crossfade |
| `Tab` (rail anchor) | fade 0→1 | fade 1→0 | 180 / 140 | 90 ms |
| `Overlay` | page dims under a sheet/menu | — | 160 | 80 ms |
| `Player` | fade to/from black | — | 200 | 90 ms |

- The page root gets `.transition(Transition::opacity().combined(Transition::offset(0, ±8)))`,
  `.springs(PAGE)` and `.instant_changes()` so children do not each fade on their own (which would
  multiply alpha and read as a slow start). The matched poster/portrait marks itself
  `.animated_changes()` (which stops the ancestor's instant group) and flies on
  `matched_motion(DETAIL)`.
- `Push` is used by the hub/library/search/tool opens. `Pop` is Back. Rail anchors are `Tab`
  (no direction; the rail is not a stack). A sheet uses no page transition, only `Overlay`.
- Back restores scroll and focus **before** the enter: the page appears already in place and
  already focused. Never glide 5,000 pt to a restored position.
- A page opened from a virtualized source (a Home rail card that is no longer mounted) uses the
  same `Pop`/`Push` fade; the matched element's own fallback transition runs instead of a flight.
- Back during an open retargets the same presence tracks; it does not mount a third copy.

### 3.2 Focus

| Element | On focus | Spring | Notes |
|---|---|---|---|
| Rail icon | scale 1.06, fill 8 % white, 1 px 12 % border | HOVER | label appears only in the expanded rail |
| Poster/still card | scale 1.05, 2 pt ring, lift shadow | HOVER | neighbours do not move; art does not zoom |
| Grid/hub card | scale 1.03, ring, lift | HOVER | 1.05 would collide with the next column |
| Cast face | scale 1.08, ring | HOVER | small target |
| OSD transport | scale 1.08, ring | OSD | video does not move |
| Chip/segment | fill/underline only | CONTROL | no scale; text must not wobble |
| Toolbar field | ring + fill | CONTROL | caret owns the interior |

Rune's `focus_visual` draws the ring and wash but has no focus scale. The scale is state-driven:
the card computes `s.focused_key == Some(key)` and sets `.scale(1.05)` with `.motion(HOVER)`; Rune
springs the property change. Focus is never delayed for an exit animation.

- Focus moving within a row changes only the two cards. The row scrolls if the next card is
  outside the 48 pt lead; the scroll glides on `GLIDE`.
- A move longer than ~900 pt (A–Z, hub jump, a restored page) lands with an instant scroll and a
  120 ms fade of the destination, not a long glide.
- Entering the rail expands it; leaving collapses it. The page dims; it never reflows.
- Press: scale 0.98 cards / 0.96 buttons for 90 ms down, spring back on `HOVER`; no ripple.
- A focus ring on a poster is 2 pt near-white; on light art it remains near-black in the light
  theme; never accent-coloured.

### 3.3 Scroll

- A TV has no free scroll. D-pad moves focus; the page/row scroll is a consequence, on `GLIDE`.
- Rails centre vertically; grids centre the row with a 24 pt lead; the rail never scrolls (five
  anchors).
- Edge fades appear on 80 ms `CONTROL`; they do not animate per frame.
- No parallax on the hero or any rail. The hero scrolls at page speed; a 4K surface cannot afford
  a second transform and a couch does not read it.
- Virtual rails/grids/guide/episodes land newly mounted rows at their height (Rune's rule); they
  do not fade in. The reader is scrolling; a 12-card cascade would feel like lag.
- Scroll restoration on Back is instant. Scroll-to-focus is the only animated scroll.

### 3.4 Images and data

- Every image has a fixed box and a graphite placeholder (letter/icon). No layout shift, ever.
- A card image arrives with a 180 ms opacity crossfade. The commit is data, so the image layer
  asks for `tx.animated()` (or `animated_changes`); without it Rune lands the change instantly by
  design.
- A hero backdrop is the only image allowed a develop: opacity 240 ms + 1.5 pt blur → 0. One or
  two layers, never per tile.
- The matched poster's art does not crossfade during the flight; it is the same image.
- Failure keeps the placeholder; a retry does not flash. A broken cache entry self-heals (Phase 0
  of the epic).
- Progress bars: watch progress and download percent tween on `PROGRESS` (280 ms) when the value
  changes; the player scrubber follows the playhead linearly (see 4.12); no looping shine.
- Skeleton → content is a 200 ms fade of the skeleton out and content in. No shimmer. A 1.2 s
  breathing loop on a TV reads as a pulse, not as loading.

### 3.5 Overlays

| Overlay | Enter | Exit | Focus |
|---|---|---|---|
| Sheet (tracks, pins, cast, playlist) | fade + 8 pt up + scale 0.98, `SHEET` | fade + 4 pt down, 130 ms | first control or the current value |
| Menu (card, gear, overflow) | fade + scale 0.96 from the control anchor, 120 ms | fade 100 ms | first item |
| Keyboard | fade + 8 pt up, 160 ms; scrim 160 | fade 120; caret stays | field above, keys below |
| Toast/undo | fade + 10 pt up, 160 | fade 120; auto 4 s | not focusable unless Undo |
| Filter/sort panel (inline) | expand 180 + content fade 120 | reverse 140 | the first control |

The page behind dims 160 ms; it does not slide. A modal scrim never flashes. A menu's exit and the
page enter may overlap by at most 80 ms; the scrim clears with the menu, not after it.

### 3.6 Tabs, segments and layout

- A tab/segment selection uses a matched indicator (`matched_motion(CONTROL)`) so the fill slides;
  the pane crossfades + 4 pt, 120 ms. Season tabs, the pins All/Chosen control, Settings sections,
  Live Now/Guide, and the player Audio/Subs/Chapters use this.
- `animate_layout()` is only for input-caused reflow that the reader is watching: the Home rail
  stack after a pin change, the hub grid after a filter, the settings section content, a playlist
  reorder. It is not used for a 100-card sort, a page append or a data arrival.
- Grid ↔ list is a 140 ms crossfade, not a morph: the two layouts are different trees and a
  hundred cards flying is noise. The toolbar stays put; the scroll resets to the top.
- Sort/filter is a 140 ms crossfade and instant scroll top. No card-by-card reorder.
- A–Z is a 140 ms grid crossfade + instant scroll top; the active letter fills 160.
- Appended pages land instantly; the footer "loading" row fades out 120. No stagger.

### 3.7 Reduce Motion

A real setting, not a hint:

- All page/tab/overlay transitions become 80–100 ms opacity fades; no offset, no scale.
- Focus ring still moves on `HOVER` (80 ms) but the card does **not** scale.
- The rail expands instantly and shows labels; the page dim is instant or 80 ms.
- No matched flight: the detail/person crossfades in place; Back crossfades.
- Scroll is instant; no glide.
- Progress still updates (information), but at 120 ms.
- Stagger is disabled entirely.
- The layout is identical; snapshots with Reduce Motion differ only by focus scale/positions.

### 3.8 What does not animate

- Text reflow and text swaps (a count, a timecode, a title variant) — crossfade 80 ms at most.
- Subtitle cues — a 60 ms fade in, no slide; a cue never moves.
- Clocks and the live now line — step at the minute; no creeping.
- Page content on a data poll — no layout spring.
- Focus on a hidden/off-screen card — it is never focused.
- Error text — no shake, no red pulse.
- The video itself — the OSD fades over it; the picture does not zoom.

---

## 4. Scene by scene

### 4.1 First launch, connect, sign-in

- **First frame:** 160 ms fade from black to the connect card, driven by a one-shot `first_paint`
  flag (`tx.animated()`); a launch frame is otherwise unprompted and lands instantly. Not a splash
  loop.
- **Discovery:** the server rows fade 140 ms once; no stagger. A second discovery pass updates in
  place.
- **Signing in:** the card stays; a 2 pt progress line draws across the card in 1.2 s, looping only
  while the request is in flight. Reduce Motion: a static "Signing in…" + fade.
- **Error:** the message fades 160; focus moves to Retry. No shake.
- **Keyboard:** 160 ms sheet; keys press 80 ms; typing is instant.
- **To Home:** the connect card fades out 180; Home's hero backdrop is already fading in 240, so
  there is no black gap. This is a `Tab`-style crossfade, not a push.

### 4.2 Home

- **Entry:** from the rail/Home a 180 ms tab crossfade; from a library a 180 ms pop with the rail
  and scroll restored first; from Detail the poster flies back to its card (260); from Player a
  200 ms fade from black. The rail never animates the page's position.
- **Hero, first mount only:** backdrop 240 (blur 1.5→0); title/meta/actions fade + 8 pt up 180 with
  a 40 ms delay; the clearlogo, if any, fades 200. Once per mount, not on every scroll.
- **Rails:** skeleton out/content in 200; no stagger. A pin change reflows the rail stack on
  `LAYOUT` 220 and crossfades the changed rails 160; the images do not animate.
- **Focus:** first Continue card at 1.05; Up reaches the hero's Resume/More info; Left enters the
  rail; Down/Right move cards and glide the row on 220 with a 48 pt lead.
- **Cards:** progress bars fill 300 ms on first paint; a card's title/meta never move. The
  libraries strip chips fill on `CONTROL`; pressing one is a `Push`.
- **See all:** a 180 ms push to the shelf; back is a 180 ms pop to the same scroll.
- **On now:** when the current programme changes, the card's title/time crossfade 120; the art
  crossfades 180. No card reorder.
- **Back:** focuses the rail (200 expansion); no page exit.

### 4.3 Rail

- **Enter (Left from content):** width 76→284 on `RAIL` 200; labels fade 120 after 40; scrim 160;
  the page dims to ~62 % and does not reflow.
- **Focus within:** icon 1.06 on `HOVER`; selected fill 180; the current anchor is focused first.
- **Press:** the destination starts its page transition; the rail collapses 180 as the page
  enters, labels out 80, scrim 120. The two overlap by design.
- **Leave (Right/Back):** collapse 180; scrim 120; focus returns to the content key it left.
- **Back from a top-level page:** the rail expands 200 and focuses the current anchor; no
  navigation and no page motion.
- **Reduce:** instant width, labels immediately on, no scrim transition.

### 4.4 Libraries hub

- **Entry:** 180 ms push; the Recent/All/Tools content fades as one page; no tile stagger.
- **Focus:** 1.03 + ring; a tile's art and label do not move. The filter field is one Up.
- **Filter:** typing debounces 100 ms; the grid crossfades 140; surviving tiles reflow on `LAYOUT`
  180; the count text crossfades 80. No per-tile stagger, no scroll jump.
- **Recent:** the last three; pressing is a 180 push. If a library header has art, it may match the
  tile; the current design has no header art, so it is a plain push.
- **Tools/playlists:** 180 push; Controls opens a sheet (Overlay).
- **Back:** 180 pop to Home; focus to the Libraries anchor. From a library opened here, Back
  returns to the hub with the tile focused.
- **Empty server:** the first-run card fades 180; no illustration animation.

### 4.5 Library grid

- **Entry:** 180 push; header/toolbar/content fade together if loaded, or the header/toolbar appear
  and the skeleton fades out 200 when the first page lands.
- **Focus:** 1.03; the grid scrolls on `GLIDE` 220 with a 24 pt lead. Left at the first column
  enters the rail; Right at the last column enters the A–Z overlay.
- **A–Z:** the letter buttons focus with a 160 fill. Pressing a letter loads that page; the grid
  crossfades 140 and jumps to the top instantly; the active letter fills 160. No 5,000 pt glide.
- **Sort/filter:** the chip/underline changes 160; the grid crossfades 140; the scroll resets;
  counts crossfade 80. No card reorder animation.
- **Grid ↔ list:** 140 crossfade; the toolbar stays; scroll resets. Not a morph.
- **Paging:** the appended 100 land instantly; the footer loading row fades out 120; the scroll
  position is unchanged.
- **Open a card:** 260 matched poster to Detail if the item has a poster; otherwise a 180 push with
  a fade. Back reverses; if the card is unmounted, 180 fade + restore scroll/focus.
- **Empty/error:** the state replaces the grid with a 160 fade; Retry keeps the loaded items and
  adds an inline row 140. No shake.

### 4.6 Library list

- **Entry:** 180 push; rows fade as one.
- **Focus:** row fill 160; no scale (a 76 pt row scaling would collide). The still is static; the
  trailing resume is a separate focus stop.
- **Open:** 180 push to Detail (no poster flight; the list still is 16:9 and the detail poster is
  2:3). Back restores the row focus and scroll instantly.
- **Playlist rows:** see 4.20.

### 4.7 Detail — film

- **Entry (with a poster):** the card's poster matches the detail poster, `DETAIL` 260, 0.05
  bounce; the backdrop fades 200 under it; the copy/actions fade + 8 pt 180 after 40 ms; the rest
  of the page (overview/cast/related) fades 180.
- **Entry (no poster, only a 16:9 still):** no flight; the hero fades 200 + 4 pt rise; the copy
  180. The card fallback is the letterboxed still, not a crop.
- **Focus:** Resume first; the button press 0.98; Down moves to Cast and glides 220. A cast face
  focuses at 1.08; the portrait is static.
- **Cast → Person:** the face matches the person portrait, `DETAIL` 260; the bio/grid fade 180.
  Back reverses to the face if it is still mounted.
- **Related card → Detail:** 180 push; a matched poster if the new detail has one, otherwise fade.
- **Trailer / Play:** 200 fade to black into the player; the detail stays beneath, unchanged.
- **Overflow:** scale 0.96 + fade 120 from the button; scrim 160; close 100.
- **Back:** reverse poster flight 260; restore the page's scroll and the card focus instantly. If
  the source card is unmounted, 180 fade + restore.

### 4.8 Detail — series

- **Entry:** same as film; the series poster flies.
- **Season chips:** focus fill 160. Pressing a chip changes the selected fill 160 and crossfades
  the episode list + 4 pt in 120; the chip row horizontally scrolls 180 to keep the active chip
  visible; the episode count crossfades 80. The page scroll does not move.
- **Episodes:** row fill 160; the still is static. The first focus after a season change is the
  first episode or the resume row; no scrolling animation for the list replacement.
- **"N more":** fades 120 when the window changes; appended rows (data) land instantly.
- **Play/Resume:** 200 fade to black.
- **Back:** reverse flight to the source card.

### 4.9 Person

- **Entry:** 260 matched portrait from the cast face; the bio and filmography fade 180. From Seerr
  or a direct open, 180 fade.
- **Focus:** Back first; the filmography cards 1.03; the portrait static. Scroll `GLIDE` 220.
- **Show all 100:** the new rows enter 160 + 4 pt (input-caused); no long stagger (a 20 ms cap on
  the first six, then none).
- **Play/film detail:** 260 push with the poster flight.
- **Back:** reverse portrait; if the face is unmounted, 180 fade.
- **No credits:** the empty line fades 160.

### 4.10 Search

- **Entry:** 180 tab/push; the field takes focus; the keyboard does not open.
- **Typing:** 150 ms debounce; old results fade out 80, new in 140; the hint row 120; no layout
  jump under the field. The page may grow below; the field stays at the top.
- **Recent chip:** fill 160; fills the field; the results update as typing.
- **Keyboard:** 160 sheet; the page dims; typing is instant; Enter submit; close 120 and the field
  stays focused.
- **No results:** the line fades 140; "Search all" is the primary action; no shake.
- **Result card → Detail:** matched poster 260 or 180 push; Back restores the query, results and
  focus (no re-fetch flash; the cached results stay).
- **Back:** if the keyboard is open, close it 120; otherwise 180 pop.

### 4.11 Live TV

- **Entry:** 180 push; the tabs and guide fade as one; the now line appears.
- **Tabs:** the underline/segment matches 160; the pane crossfades + 4 pt 120.
- **Program focus:** 1.02 + ring 160; the preview panel content crossfades 100 (art, title, time,
  description) while the panel itself stays pinned. The guide row does not scroll unless the next
  channel is off-screen, then `GLIDE` 220.
- **Now line:** steps at the minute; the live programme bar advances linearly over its duration
  (1 s ticks), never springs.
- **Favourites:** the channel set crossfades 140; rows reflow on `LAYOUT` 180; no stagger.
- **Refresh:** a 160 ms tint on the guide; when data lands the guide fades 160 back. No spinner.
- **Watch:** 200 fade to black.
- **No tuner:** the empty state fades 160.
- **60 channels:** only mounted rows animate; scrolling lands new rows instantly.

### 4.12 Player OSD

- **Open:** 200 fade from black; the OSD is visible for 3 s, then hides.
- **Show:** opacity + 8 pt up on `OSD` 120; the video dims 40–60 % over 200. **Hide:** 120 fade +
  8 pt down; dim back 200. The auto-hide is a timer commit, so it calls `tx.animated()` to play;
  a user input is already animated.
- **First key while hidden:** only shows the OSD; it does not act.
- **Play/pause:** the icon crossfades 120; no bounce. **Seek ±10:** the track tweens 120; the time
  text crossfades 80; a 2 pt tick can pulse once (skip).
- **Scrubber:** the played width follows the playhead on a 1 s linear tick; on seek, 120 tween to
  the new position (no spring lag). Chapter ticks brighten 160 at the current chapter.
- **Tracks/chapters sheet:** 160 fade + 8 up; the page (video) dims; the tabs match 160; the list
  scrolls; close 140; focus returns to the gear.
- **Subtitles:** a new cue fades 60; no slide; a cue never moves the picture.
- **Back:** first hides the OSD (120); a second Back fades to the page (200).
- **Reduce:** 80 ms fades, no 8 pt move, instant sheet.

### 4.13 Tracks & chapters sheet

- **Entry:** 160 sheet; focus the current audio/chapter.
- **Tabs:** matched pill 160; list crossfade 120.
- **List:** scroll; the selected row fills 160; the current chapter is visible on open. With 300
  chapters, only mounted rows exist; scrolling lands new rows instantly.
- **Subtitle filter:** typing filters on 100 ms; the list fades 120; the field keeps focus.
- **Select:** check 120; the sheet stays open; a toast is not needed.
- **Close:** 140; focus returns to the OSD gear; the OSD auto-hide timer restarts.

### 4.14 Downloads

- **Entry:** 180 push; the summary and rows fade as one.
- **Focus:** row fill 160; Play/Pause button 0.96 press; the icon crossfades 120.
- **Progress:** the bar tweens 200/280 as bytes arrive; the percent text crossfades 80 (data, so
  `tx.animated` on opacity/width only); the row does not reflow.
- **State change (downloading → ready):** the action label crossfades 120; the storage summary
  updates 200.
- **Remove:** the row fades 160 and leaves layout; the summary updates 200. No collapse bounce.
- **Empty:** fade 160.

### 4.15 Remote

- **Entry:** 180 push; the session rows fade.
- **Focus:** row fill; Play/Pause/Stop press 0.96.
- **Action:** the request dims the row 80; the response crossfades the icon/status 120. No spinner.
- **Refresh:** 160 tint; rows update 120; no shuffle animation.
- **Empty:** fade 160.

### 4.16 Settings

- **Entry:** 180 push; the section list and detail fade together.
- **Sections:** the selected pill matches 180; the detail pane crossfades + 4 pt 140; the pane's
  scroll resets to the section top instantly (no glide).
- **Rows:** switch knobs slide 180; segmented indicators match 160; values crossfade 100.
- **Inline panels (filters/sort):** expand 180 + fade 120; close 140; the row focus stays.
- **Home rails:** opens the chooser sheet (4.17); on save, the Home behind updates on return.
- **Back:** closes an open sub-panel first (140), else 180 pop.

### 4.17 Home rails chooser

- **Entry:** 160 sheet over Home; the page dims 160; focus the first row or the current selection.
- **Toggle:** the checkbox fills 120; the position badge changes; the footer "n of 4" crossfades
  80. At four, the other rows dim to 0.55 over 160 and a 160 toast says "Remove one to add
  another"; no shake.
- **All libraries ↔ Chosen:** the segment matches 160; the list dims 0.5 over 160; the copy.
- **Reset:** rows clear 120; the segment returns to All.
- **Done/Back:** close 140; Home's rail stack reflows on `LAYOUT` 220 and the changed rails
  crossfade 160; a 160 toast "Home rails saved". Keep the old rails visible until the new data
  lands; no skeleton flash.
- **Reduce:** 80 fades; the Home stack updates instantly.

### 4.18 States: loading, empty, error, image failure

- **Loading:** no shimmer. The skeleton is static; when the content lands, skeleton out/content in
  200. The header/toolbar may appear immediately.
- **Empty:** icon/title/action fade 180; one primary action; no bounce.
- **Error:** message fades 160; focus Retry; no shake, no red pulse.
- **Paging failure:** the inline row fades 140; the loaded items stay; Retry.
- **Image failure:** the placeholder stays; a retry does not flash; no layout shift.
- **First run:** the card fades 180.

### 4.19 Overlays not tied to a page

- **Card menu / gear menu:** scale 0.96 from the control + fade 120; scrim 160; close 100; focus
  returns to the control.
- **Keyboard:** 160 sheet; close 120; the field stays focused.
- **Shortcuts / controls:** 160 sheet; the key rows never animate on capture.
- **Cast picker:** 160 sheet; the session rows 120.
- **Playlist create / add-to-playlist:** 160 sheet; the new name field; the keyboard on Enter.
- **Toast/undo:** 160 in, 120 out, 4 s; Undo is a button, not a timer bar.

### 4.20 Playlist

- **Entry:** 180 push from the hub; Play all/Shuffle/Edit and the rows fade as one page.
- **Focus:** Play all first; rows fill 160 (no scale, like the library list); the drag handle is
  visible only in Edit and is not a focus stop.
- **Edit:** the action bar crossfades 120; the handles and Remove controls fade 120; the row
  heights do not change. Reorder uses Menu → Move up/down: the moved row slides with `offset`/
  `LAYOUT` 180, the list reflows once, and the order persists. Remove fades the row 160 and
  reflows the list 180; the count crossfades 80.
- **Play all / Shuffle:** 200 fade to black into the player; Back returns to the same scroll and
  focus.
- **Empty:** the icon/line fades 160 with "Add something" as the primary action.
- **Back:** 180 pop to the hub; in Edit, Back leaves Edit first (140), then pops.

### 4.21 Shelves (See all)

- **Entry:** 180 push from Home; the shelf title/grid fades as one. No paging; a shelf is a short
  grid.
- **Focus:** cards 1.03 with the 24 pt grid lead; a card opens Detail with the poster flight.
- **Back:** 180 pop to the same Home scroll with the originating See all control focused; no
  re-scroll animation.

---

## 5. Master event table

| Event | Motion | Time | Reduce |
|---|---|---|---|
| Focus card | scale 1.05 + ring | 160 | ring 80, no scale |
| Focus grid/hub | scale 1.03 + ring | 160 | ring 80 |
| Focus rail | scale 1.06 + fill | 160 | fill 80 |
| Press | scale 0.98/0.96 | 90 down | instant |
| Scroll to focus | glide, 48/24 lead | 220 | instant |
| Page push | fade + 8 up | 180 | 90 fade |
| Page pop | fade + 6 down | 180 | 90 fade |
| Rail anchor | fade | 180 | 90 fade |
| Rail expand | width + labels + scrim | 200/120/160 | instant |
| Detail flight | matched poster | 260 | 100 fade |
| Detail fallback | fade + 4 up | 180 | 90 |
| Season tab | list fade + 4 | 120 | instant |
| Grid/list | crossfade | 140 | instant |
| Sort/filter | crossfade + scroll top | 140 | instant |
| A–Z | grid fade + scroll top | 140 | instant |
| Page append | none | — | — |
| Image | crossfade | 180 | 80 |
| Hero backdrop | fade + blur 1.5→0 | 240 | 90 |
| Skeleton → content | fade | 200 | 90 |
| OSD show/hide | fade + 8 | 120 | 80 |
| Sheet | fade + 8 + .98 | 160 | 80 |
| Menu | fade + .96 | 120 | instant |
| Toast | fade + 10 | 160 | instant |
| Switch | knob slide | 180 | instant |
| Segment | matched indicator | 160 | instant |
| Progress | tween | 280 | 120 |
| Playlist reorder | row offset + reflow | 180 | instant |
| Now line | step | minute | step |
| Subtitle cue | fade | 60 | 60 |

---

## 6. Rune mapping

Everything below exists at the pinned rev; no fork work is required.

| Need | API | Notes |
|---|---|---|
| Page enter/exit | `El::transition(Transition::…)`, `.springs(Pair)` | set on the screen root; `.instant_changes()` so children do not multiply the fade |
| Directional page motion | `Transition::offset(0, 8).combined(Transition::opacity())` | mirror the offset on Pop |
| Shared poster/portrait | `.matched(id)`, `.matched_motion(DETAIL)`, `.animated_changes()` | the card and detail share the id; `animated_changes` escapes the page's group fade; a fallback transition runs when the source is absent |
| Input-caused reflow | `.animate_layout()` + `tx.motion(LAYOUT)` | Home rail stack, hub filter, settings pane, playlist reorder |
| Focus scale | state-driven `.scale(if focused {1.05} else {1.0})` + `.motion(HOVER)` | `focus_visual` has no scale; compare `s.focused_key` |
| Ring/wash | `.focus_visual(2.0)` / `theme::FocusVisual` | unchanged |
| Press | `.press_scale(0.98)` | unchanged |
| Scroll | `tx.scroll_center`, `tx.scroll_into_view_in`, `tx.scroll_to_with(..., GLIDE)` | scope the rail/hub scroll; instant on restore |
| Presence | `.transition(Transition::opacity().combined(offset))` | sheets, toast, OSD |
| Data fade | `tx.animated()` / `El::animated_changes()` | image arrival, progress, timer auto-hide |
| Reduce | Rune applies it to presence; branch for scale/matched/stagger | `State::reduce_motion` |
| Motion lint | `rune::motion_gate` / `--trace` | Phase 9 audit |

Add four small helpers in `theme::motion` so scenes do not hand-roll pairs:
`page(kind)`, `overlay()`, `data_fade()`, `focus_scale(focused, base)`.

**Do not:** use `Transition::springs` with a generic 0.3; leave the framework default on a page;
apply blur per card; animate the video, a subtitle, a clock or a text reflow; use a stagger on
data; animate a 100-card sort.

---

## 7. Performance and accessibility

- **Frame budget:** one matched flight or layout animation at a time; no layout animation of a
  full library; at most four blurred backdrop layers; no per-card blur.
- **Focus latency:** focus visual starts on the same frame as the key; the spring never delays the
  target. A second press retargets.
- **Reduce Motion:** every scene has a reduced path (2.8 and 3.7), not a global "off" that leaves
  a dead cut. The layout is unchanged.
- **Readability:** no animation of body text, subtitles, timecodes or counts; a progress bar may
  move.
- **Contrast:** the ring is near-white/near-black, never accent; the focused card scale never
  covers a neighbour's title.
- **Motion sickness:** no zoom on focus beyond 1.08, no parallax, no looping ambient motion, no
  full-screen slides, no overshoot.
- **Testing:** a runtime test asserts the state/focus/scroll target after a key (the existing
  `Runtime` pattern); motion curves are reviewed with `--trace` and 4K snapshots. Snapshots do not
  prove motion; the scene list in §4 is the manual review card at 1280, 1080p and 4K.

---

## 8. Phase integration

| Phase | Motion work |
|---|---|
| 1 | Tokens and helpers; no scene motion yet |
| 2 | Page kinds, rail expand/collapse, hub filter/layout, focus scale |
| 3 | Home hero/skeleton/rails, pin reflow, image crossfade |
| 4 | Grid/list, sort/filter, A–Z, paging, card→detail fallback |
| 5 | Detail flight/fallback, season crossfade, person portrait, cast |
| 6 | OSD show/hide/auto-hide, scrubber, tracks sheet, subtitle cue |
| 7 | Guide preview/now, search results/keyboard, downloads progress, remote state |
| 8 | Settings panes, pins chooser, playlist reorder, states |
| 9 | Audit the table, `--trace`, Reduce Motion, 4K, final snapshots |

Each phase's acceptance includes its §4 scenes: the transition exists, is interruptible, has a
Reduced path, and does not move data.

## 9. Current gaps this fills

- `theme::motion` has only `HOVER` and `CONTROL`; there are 306 call sites but no page, rail,
  overlay or detail pair.
- Route changes rely on the framework's default 0.30 s presence; pages have no authored direction.
- Cards have a ring/wash but no focus scale; the captures show one.
- The OSD and sheets mostly appear/disappear without a presence transition; the auto-hide timer is
  a data-style commit and needs `tx.animated()`.
- The detail already uses `matched` on a poster, but there is no authored flight curve, no fallback
  when the source card is unmounted, and the new hero layout may not have a poster to match.
- Images land through data commits; without an explicit `tx.animated()` they cut in.
- There is no motion spec for the hub, the pins chooser, the guide preview, the tracks sheet or the
  300-chapter case.
