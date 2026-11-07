# How media apps look on a TV — evidence for Runefin's big-screen mode

**Status:** analysis, 2026-11-07. Companion to `09-tv-mode.md` (feasibility) and the plan in
`docs/plans/2026-11-07-big-screen-mode.md`.

**Scope.** What the major TV apps actually put on a 10-foot screen, and which of those choices
Runefin should copy. This is not a feature survey; it covers shell, navigation, focus, type,
density and playback chrome. Sources are at the end.

---

## 1. The six shells worth looking at

**Netflix (2025 redesign).** The reference for a modern TV home. It moved navigation from a left
rail to a **top bar** (Home, Shows, Movies, Games, My Netflix, Search), made the tiles **larger**,
and put the "why this title" information next to the row instead of in a separate panel. The back
button returns to the top bar; the row you left is restored. Rendering is built around 60 fps and
**key-input latency** as a first-class metric. Netflix also ships video previews and rotating
billboard art — the parts Runefin does not need.

**Plex (TV apps).** The conservative shell: a **left rail** of libraries and services, then
horizontal hubs under a hero. Focus is a bright border with a slight scale; cards are large; the
player OSD is a bottom bar with title, scrubber and a small transport. It proves that a left rail
is still a valid TV pattern, and that server-defined rows survive at 10 feet if the rows are
ordered well.

**Jellyfin Android TV / Fire TV.** Top tabs or a left drawer depending on the client, then rows of
posters; the artwork is the UI. Focus is a scale plus a colored ring. Settings are deliberately
shallow. Its weakness is the same as the server's: rows are configured, not curated, so empty or
duplicated rails show up unless the client suppresses them.

**Kodi (Estuary).** A **vertical category spine** (Movies, TV shows, Music, …) where each category
owns a stack of widget rows. It is the most keyboard/remote-native of the group: a clear two-axis
focus path, a big focused widget, a details pane that follows focus. Estuary is the model for
"focus is the layout".

**Apple TV app.** A top nav, edge-to-edge art, and the most explicit focus treatment: items
**expand and lift** rather than gain a border, with a parallax highlight. Apple's HIG calls for
subtle, fluid motion and artwork that goes to the screen edge. It is the taste ceiling, not a
layout to copy literally.

**Disney+ / Max / Prime Video.** All converge on the same shape: 4–5 top destinations, a hero or
billboard, then horizontal rails; focus is a scale + ring; the player OSD auto-hides and the
scrubber is the only persistent control. They differ in brand, not in interaction.

**Steam Big Picture / Steam Machine.** The controller-first shell: large tiles, a single focus
ring, A to select, B to go back, shoulders to change section, and an on-screen keyboard summoned
by Steam. It is the input model Runefin has to satisfy, even though it is not a media app.

---

## 2. What they agree on

| Layer | TV practice | Number / rule |
|---|---|---|
| Aspect | 16:9 only, landscape | design at 960×540 and scale to 1080p/4K |
| Safe area | overscan margin for critical UI | 5 % (48 dp sides, 27 dp top/bottom on Android TV) |
| Navigation | 4–5 destinations; left rail or top bar | more than five is an overloaded IA |
| Focus | always visible; scale + ring, not border alone | ~1.05–1.15 scale, 150–250 ms |
| Type | large, high contrast, short lines | body ≥ 14 pt at 3 m; TV type ≈ 1.4× desktop |
| Rails | horizontal, 3–5 cards visible, peek at the edge | vertical feed only for search/results |
| Content | Continue Watching first | the highest-value rail on the screen |
| Back | one level; at the top, return to nav | restore the focused item and scroll |
| Player | OSD auto-hides; scrubber + transport | no hover-only affordances |
| Input | D-pad/left stick, A select, B back, shoulders page | no pointer, no swipe-only |
| Motion | snappy, input-led, no auto-advance fights reading | 60 fps and low key latency |

Sources: Android TV layout/overscan guidance, Apple HIG for tvOS, Netflix's 2025 redesign coverage
and its TV performance write-up, Kodi Estuary's skin layout, Plex/Jellyfin TV clients, Smashing's
TV design guide, and the 2026 streaming-app UX survey (see §5).

---

## 3. Where Runefin already matches

- **One focus system** with rings, groups, tab order, focus memory and scroll-into-view — the
  hardest engine piece is already in Rune.
- **Continue Watching first**, merged and deduped, and the rail peeks at the edge.
- **Edge-to-edge detail backdrop** with a two-axis scrim, and a wordmark/title over art.
- **A player OSD** with a scrubber, chapter ticks, skip pill, panels and a stats overlay.
- **Text scaling** (85–135 %) and a 130 % snapshot, so the type path is proven.
- **Fullscreen** and cursor hiding already exist.

## 4. Where it falls short

1. **No controller input** and no spatial focus; arrows rove inside a group, Tab moves groups.
2. **No initial focus**; the first D-pad press can do nothing.
3. **Hover-only paths**: remove-from-resume, trickplay preview, scrub preview.
4. **No on-screen keyboard** of our own (Steam provides one).
5. **Density is desktop**: 176 pt cards, 40 pt gutters, 14 pt labels.
6. **The shell is a 224 pt sidebar** with a pointer-sized hit area; at 10 feet it is small.
7. **Focus coverage is uneven**: Live TV has no group; the player's controls are individual
   focusables; the rail is a long Tab run.

## 5. Decisions this evidence supports

1. **Keep a left rail, scaled** (Plex/Jellyfin/Android TV), rather than rebuild the shell as a top
   bar. Android TV explicitly recommends left/right navigation; Runefin already has the rail and
   the focus groups. A top bar is a later option if the rail fails on the couch.
2. **Design the TV layout at 960×540** and scale to 1080p/4K, with a 5 % safe margin.
3. **Scale, don't redesign**: 1.5× cards, 1.3× type, wider gutters, 3 px ring. The mode is a
   presentation flag plus an input profile, exactly as `09-tv-mode.md` concluded.
4. **Replace hover paths with focus paths** and a context button (Start / Menu).
5. **Native controller support, not Steam Input only.** Steam Input is a fallback; the app should
   read the pad itself so it also works outside Steam and so A/B can mean select/back in every
   screen.
6. **Initial focus is part of the feature**, not a polish item: the first D-pad press must move.

## 6. Sources

- Android Developers — *Build TV layouts* (landscape, left/right navigation, 5 % overscan) and
  *Key considerations for TV* (D-pad, 10-foot UI, no swipe-only). 2026.
- Apple — *Designing for tvOS* and *Focus and selection* (HIG).
- Netflix — *Unveiling our innovative new TV experience* (May 2025): top nav, larger tiles,
  responsive rows; The Verge/Engadget coverage of the same; *Crafting a high-performance TV UI*
  (key latency, 60 fps) and the 2013 *Building the new Netflix experience* (LRUD input).
- Plex — TV app UI and recommendation management support pages.
- Jellyfin — Android TV client; `02-media-ux-patterns.md` has the server-side row model.
- Kodi — Estuary `Home.xml` (vertical spine, per-category widget rows).
- Smashing Magazine — *Designing for TV: principles, patterns, practical guidance*.
- Forasoft — *Streaming app UX best practices* (2026): 4–5 destinations, 14 pt at 3 m, focus cues.
