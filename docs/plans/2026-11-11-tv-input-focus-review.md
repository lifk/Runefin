# TV input and focus — review and proposed model

**Status:** review and first implementation, 2026-11-11. See "Implemented" at the end.
**Scope:** big-screen mode with an HDMI-CEC remote, a gamepad, or a keyboard. Desktop pointer
behaviour stays as it is.

## 1. What is happening, and why

| Symptom | Root cause | Where |
|---|---|---|
| Inside a Home rail, Up/Down move left/right | Rune treats Up/Down as backward/forward in any group that is not a vertical role (`menu`, `listbox`, `list`, `tree`). A card rail is a plain `focus_group()`, so the four arrows collapse to two | `runtime/input/focus.rs:446-448`; rails at `screens.rs:2025`, `:1907` |
| Left at the start of a rail wraps to the last card | Plain rows are not in `CLAMPED_ROLES` (`list`, `tree`, `grid`), so groups wrap | `focus.rs:15`, `:456-460` |
| Up/Down never leave the rail | Rune has no spatial navigation: moving outside a group is Tab/Shift+Tab, and only the app can request another key | `runtime/input/focus.rs`, `Tx::focus_key` |
| Detail screen: nothing moves | The screen opens with focus on `detail-play`, but the action row is not a focus group, so arrows have no next item. Rune only moves *inside* a group | `screens/detail.rs:826` and the row below it |
| Player: OK on the CEC remote does not pause | `play_pause` defaults to `space`; Return is not bound. `set_return_activates` only makes Return press a *focused* control, and the OSD starts with nothing focused | `shortcuts.rs:65`; `main.rs:283` |
| Player: arrows only reach the OSD via the shoulders | In the player, Left/Right and Up/Down are consumed for seek and volume before focus can move; Tab (LB/RB) is the only way through the controls | `main.rs` in-app key block; `screens/in_app.rs` transport |
| Focus looks like a blue box | `theme::FOCUS` is a solid blue and big screen doubles the ring to 4 pt; `focus_highlight` adds a 10 % accent fill underneath. Desktop hover is a neutral `t.hover` wash | `theme.rs:144`, `:152-160` |
| TV-remote seeking shows no trickplay preview | The preview is driven by pointer hover (`in_app_hover`); a remote seek jumps without setting a scrub position | `screens/in_app.rs` seek bar; `state/in_app_state.rs` `hover_in_app_seek` |

All six are consequences of one architectural choice: **screens ask Rune for a starting focus key
and then leave navigation to Rune's desktop model** (Tab order plus group arrows). A TV remote and a
gamepad are spatial, and the model has to be ours.

What Rune does give us, and should keep:

- `El::focus_ring`, `El::focus_fill`, `focus_highlight` for drawing focus;
- Space/Return activation of the focused control;
- `Tx::focus_key(key)` to move focus to a key, and `Tx::focused()` in a window `on_key` to read
  the key that had focus when the key arrived (`el/tx/keys.rs:52`).

## 2. Decision

**Own the TV focus model in the app** (`crates/runefin/src/tv_focus.rs`), on top of `Tx::focused()`
and `Tx::focus_key()`. Rune keeps rendering the ring and delivering activation; it no longer
decides where a direction goes in big-screen mode.

Rejected alternative: add spatial navigation inside Rune (a "spatial group" mode, edge-clamping
groups, Up/Down leaving horizontal groups). It is a larger fork surface, changes desktop semantics
for every app, and still would not know that Detail's actions sit above "Next up" and below the
season chips. Screens know their own geometry; Rune does not.

## 3. The model

A screen declares **zones** in reading order. A zone is a list of focusable keys with a
`Direction` step, plus edges:

```
Zone { id, keys, axis: Horizontal | Vertical, parent: Option<ZoneId> }
```

Navigation rules:

1. Left/Right move inside a horizontal zone; Up/Down inside a vertical one.
2. Up/Down at a horizontal zone's edge move to the adjacent zone (the one above/below in the
   screen's zone order), remembering the **column** you came from; coming back restores it.
3. Left at the start of a content zone goes to its parent (the sidebar for Home and Library).
4. Edges **clamp**: no wrap. A rail that ends at the right stops; it does not jump to the first
   card.
5. Right from the sidebar enters the current section's first content zone.
6. The player OSD is its own screen of zones (timeline, transport, bottom bar); a focused control
   decides what Up/Down means (the timeline scrubs; the volume slider adjusts).
7. Modals trap all of it; Back closes them first.

The router is attached to the root element's `on_key`, which Rune runs before its focus-group
navigation. The window's `on_key` alone is too late once a control has focus: Rune consumes arrow
keys first. The root hook returns `true` when it has moved focus, so Rune never applies its
desktop group rules in big-screen mode. A focused text field is the exception — Rune keeps its
keys — so Search passes Down out of the field in its own `on_input_key`.

## 4. Buttons per context

`OK` is Return on CEC, A on the pad, Enter on a keyboard. `Back` is Escape, B, or the remote's Back.

| Context | D-pad / arrows | OK | Back | Shoulders | Menu/Start |
|---|---|---|---|---|---|
| Sidebar | Up/Down between destinations; Right into content | open destination | — | page the sidebar | — |
| Home rail | Left/Right cards; Up/Down to the rail above/below; Left at first card → sidebar | open detail | to sidebar if in content | jump 5 cards | card menu |
| Library grid/list | arrows move in the grid; edges stay; Left at the first column → sidebar | open detail | Home | page | card menu |
| Detail | Up/Down between actions / seasons / episodes; Left/Right inside the action row | primary action | back to origin | jump section | — |
| Search | type in the field; Down into results; Left at the first column → sidebar | open result | close search | switch result group | — |
| Settings | Up/Down rows; Left/Right on a row's control | activate/toggle | back | page | — |
| Player, OSD hidden | Left/Right seek 10 s; Up/Down volume | play/pause | stop | — | open gear |
| Player, OSD shown | move between OSD zones; focused control decides Left/Right | activate the focused control; on the timeline, play/pause | close OSD | switch zone | gear |
| Player, timeline focused | Left/Right scrub with the trickplay preview and a time bubble | commit the scrub (or pause if not scrubbing) | close OSD | — | — |
| Player, volume focused | Up/Down adjust the level; Left/Right move to the next control | — | close OSD | — | — |

The CEC mapping needs one change: OK arrives as Return, which the player did not bind. The router
handles Return as OK; Space stays as a second activation key for the pad.

## 5. Focus visuals

One system for both modes:

- **Big screen: the hover treatment or the hover wash, no ring.** A focused card shows the same
  rounded poster outline, play/remove affordance and soft halo as the pointer, without scaling
  or filling the title; other controls get the pointer's `t.hover` fill. Both suppress Rune's
  default ring (`no_focus_ring`). A ring or a filled box around a card, sidebar row or detail
  button reads as a selection, not as focus. Desktop keeps its accent `focus_ring`;
  `theme::FocusVisual` picks the treatment.
- **Fill**: `t.hover`, exactly the desktop hover token, at the element's own radius.
- **Selected ≠ focused**: the sidebar's current destination keeps its accent pill; focus adds the
  neutral wash inside it.
- **No motion on focus**: no scale, no spring; focus moves must be instant on a TV.
- **Pressed**: the existing press scale; unchanged.
- Text colour does not change on focus; contrast must survive a bright poster.

Because focus is not part of the committed snapshots, the focus pair (`focus-card-big`,
`focus-sidebar-big`) renders at 1080p past the first poll, so the visual contract is reviewable.

## 6. Work, in slices

1. **S1 — router + Home/Library** (the largest slice). `tv_focus` module, zone declarations for
   the sidebar, Home rails and the library grid/list, lane memory, clamp edges, sidebar
   round-trip. Headless tests: a key script per screen asserting the focused key after every
   press.
2. **S2 — Detail, Person, Search, Settings.** Detail gets an explicit action zone, seasons zone,
   next-up zone, episode list; search results become zones; settings rows a vertical zone.
3. **S3 — Player OSD.** Zones, Return as OK, scrub-with-preview on the timeline, volume slider's
   Left/Right, OSD opens focused on the timeline; trickplay preview follows the scrub position.
4. **S4 — Visuals.** Ring/fill tokens above, the two-scene focus snapshot, and a pass over
   `tv_focus()` users to delete the accent wash.
5. **S5 — Mapping and docs.** Update the CEC sample (`cecd.toml`) if any key still differs,
   document the table above in `packaging/flatpak/README.md`, and add the pad map to the
   in-app shortcuts sheet.

Immediate one-liners worth taking before S1: bind Return to play/pause in the player when nothing
is focused, and stop wrapping at rail edges (a `focus_group` role change). Both are removed or
absorbed by S1 anyway.

## 7. Open decisions

- **Up from the top rail**: page header (Home title) or straight to the sidebar? The sidebar is one
  fewer press to leave the page; the header is spatial. Recommendation: sidebar.
- **Wrap or clamp**: clamp everywhere, including the sidebar (recommended: a TV remote's edge
  should feel like an edge), except menus and popups where wrapping is a desktop convention.
- **Player start**: OSD focused on the timeline (recommended, so Left/Right scrub immediately) or
  nothing focused (so Left/Right seek blind)?
- **Ring colour**: neutral `t.text` at 70 % or the accent at 60 %? The accent keeps the brand; the
  neutral is quieter over artwork.

## 8. What this does not change

- Pointer behaviour, hover, drag and the desktop mode.
- Rune itself; no fork change is required for this model.
- The playback stack, subtitles and the auto-sync plan.

## Implemented 2026-11-11

- `tv_input`: one semantic action per key (directions, activate, back, menu, paging), shared by
  the router and the player. CEC, keyboard and the gamepad thread all arrive here.
- `tv_focus`: app-owned zones with explicit edges for Home (sidebar and every rail, including
  on-now, each library rail, recently played and collections), Library (grid columns and list),
  Detail (back, actions, seasons, episodes) and the player OSD (timeline and controls). Up/Down
  switch lanes, Left at a zone's start returns to the sidebar, edges clamp, nothing wraps. The
  window's `on_key` runs the router before Rune and consumes the direction when it owns the
  screen.
- OSD: Down from the timeline reaches the controls, the volume slider spends Up/Down on the
  level (Left/Right move on to the next control), Return on the timeline is play/pause, and a
  remote seek shows the trickplay preview at the new position while the chrome is up.
- Focus visuals: neutral `t.hover` wash; the 4 pt solid blue and the accent highlight wash are
  gone (the 2 pt ring this first pass kept was removed in the sixth).
- `--windowed`: same layout, zones and keys in a normal window, for laptop iteration.

Second pass (later the same day): zones for Search (the field plus each result group), Person
(back, Known for, the library grid) and Seerr (the tabs plus the current tab's cards or request
rows). Settings is a vertical form with dozens of control keys, so a direction there becomes
Next/Previous in tab order, which already follows the page, instead of a hand-kept key list.

Third pass: Live TV (channels, or the guide rows while the guide is open) and Remote (session
rows) join the sidebar in the router. A focused panel, menu or scrollbar is left to Rune: the
router only moves when the focused key belongs to one of its zones, so the card menu, the gear
sheet and the subtitle panel keep their own directions.

Fourth pass: Seerr's detail page (Back, the action row, the season picker, the cast rail). The
zones mirror what the page actually draws: only rendered actions take focus (no request button
while a season is still unchosen, or once Seerr owns the title), seasons skip specials and empty
ones, and only the first twelve named cast members are reachable. Moving down from a vertical
list into a rail now starts at the rail's first item instead of carrying the row number across as
a column. The router also claims Search, Person, Seerr, Live TV, Remote and the Seerr detail page
at their edges, not only when a move succeeded, so Rune's group rules cannot take over there.

Fifth pass: the router had only ever run on the window's `on_key`, which Rune calls *after* its
own focus navigation once a control is focused; the arrow keys were still wrapping the rail. It
now hangs off the root element's `on_key`, so it sees directions first. The first runtime tests
that press real keys pin this: Down changes Home lanes instead of moving one card, and Down from
the Search field lands on the first movie. Settings maps Up/Down to its tab order and leaves
Left/Right to the focused slider or switch. The player's timeline scrubs with Left/Right and
commits with OK; the OSD asks for the timeline's focus when it opens, and a live stream omits the
timeline zone because it has no bar. Two big-screen focus shots (`focus-card-big`,
`focus-sidebar-big`) join the committed set; focus is runtime state, so they render past the first
poll and the sidebar one walks there with a script.

Sixth pass: big-screen focus is the pointer's hover treatment (rounded poster outline, play or
remove affordance, a soft halo) and, for buttons and rows, the hover wash, with no ring at all
(Rune's default ring is suppressed too), because an outline or fill still read as a selection box.
A 6 % card lift (`El::focus_scale`, added to the fork and pinned) was tried and removed in the
seventh pass: it zoomed the card in place without making the row breathe, and it only applied to
the Continue Watching cards until every card builder shared the treatment. The Detail zones now
mirror the whole page: every action that is drawn (play options, Start over from the item the primary opens, theme
audio/video, trailer, Seerr request, collection Play all/Shuffle, favourite, watched, playlist),
the Next up card, the season chips, and the episode header's Play season / Shuffle / Season
watched controls. That is what made Start over, Play next, Play season and Shuffle unreachable.

Still open: a hardware pass over the zones now that the router actually receives the keys, and a
decision on whether Settings text fields should let Up/Down leave them (today they keep every
arrow, as Rune does for inputs).
