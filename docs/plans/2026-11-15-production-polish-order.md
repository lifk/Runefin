# Production polish order — what I would work on, in order

**2026-11-15.** This is ordered by *perceived production quality per hour*, with dependencies
respected. It deliberately takes craft from production apps — hierarchy, type discipline, state
design, motion latency, asset-failure handling, copy voice — and does **not** copy their
merchandising patterns (Top 10, autoplay-on-focus, rotating heroes). A personal Jellyfin client
should feel authored, not like a streaming service with a smaller catalogue.

The test for every item: *would a stranger believe a team shipped this on purpose?*

---

## Step 1 — Kill the unfinished tells (2–3 days)

The cheapest quality win. These are not design opinions; they are things that should never reach
a default layer.

- **Copy pass.** No raw ISO dates (`2024-03-21`), scraper tags (`china`, `based on novel or
  book`), wire labels (`English - SUBRIP - External`, `Undefined - Default - ASS`), durations as
  `4 d 23 h`, or implementation words (`Decision`, `Why`, `In-app player`, `Statistics CPU`) in
  the default UI. Move technical facts under Details/Diagnostics.
- **No debug furniture.** Device id, version, server URL, device paths, request timings and
  “Continue on…” leave the first layer. Settings opens on identity, not an install report.
- **No dead controls.** Remove `Refresh` where a live screen can refresh itself; replace with
  “Updated 2 min ago”. No “See all” when there is nothing more.
- **Light theme.** Fix the notch/band at the top and re-tune the selected-pill ink; either commit
  one light frame or remove light from the picker.
- **One copy voice.** Empty/error/permission states get a headline, one sentence and one action,
  in the same voice as the wordmark. “Your library is empty” + “Browse Movies”, not “Add a movie
  or show to your Jellyfin server, then refresh.”
- **Consistency audit.** One spelling (`Favourite`), one casing (`Next up`), one label per action.
- **Cross-rail duplication.** A series should not appear in Continue Watching, Next up and Coming
  up at once. One dedupe pass, priority CW → Next up → Coming up.

**Done when:** a grep of the default UI finds no ISO date, no `Undefined`, no hex device id, no
`Refresh` button, no duplicate `See all`, every empty/error state has one action, and no series is
sold twice above the fold.

---

## Step 2 — Make type do the work (2–3 days)

This is the largest visual upgrade per line of code.

- **A real data face.** `mono_caption()` is currently proportional. Use a true mono (or
  tabular figures) for episode codes, clocks, counts, sizes, ratings and the letter rail.
- **One scale with a display range.** Add `DISPLAY 56/700/−1.2`, `PAGE 36/650/−0.6`,
  `SECTION 22/600/−0.2`, `CARD_TITLE 15/600`, `CARD_META 13/450`; tracking tokens for uppercase
  section labels. Keep the existing 85–130 % scaling.
- **Card hierarchy.** Title and meta must not be the same size. Reserve two title lines; never
  clip a title to one line; keep the meta line aligned across a row.
- **Detail metadata discipline.** One human line (`2024 · TV-MA · 3 seasons · Sci-Fi & Fantasy`),
  tagline second, scores third. Studio, tags, release date and codecs go under Details.
- **A type pair, not a font hunt.** An OFL grotesque + mono (IBM Plex Sans/Mono, Geist, etc.)
  embedded in the binary. This is the one real asset decision; avoid licensing theatre.

**Done when:** every time, count and duration is tabular; no card title clips; the page title,
section title and card title are unmistakably different; the detail page has one readable
metadata line.

---

## Step 3 — One shell and one control language (3–4 days)

Production apps feel like a set because the frame is the same everywhere.

- **`PageHeader`.** Back slot, title, subtitle, actions right, hairline on scroll. Replace the
  three current back patterns (floating slab, inline chip, “← Home”) across Home, library,
  detail, person, search, Live TV, Settings, Discover.
- **Control scale.** Heights 28 / 36 / 44 only; radii 4 / 8 / 12 / full; one `Control` helper owns
  ink, hover, focus, disabled and selected. Delete the 26/30/32/34/38/40 drift.
- **One selection language.** Selected chip = neutral wash + 1 px border + white; selected nav =
  one treatment; primary = one treatment. No three ways to say “selected”.
- **Settings identity.** Replace “This install” with avatar, name, server and Sign out; Device id
  and Version under About. Use the width: two columns above 1400 pt.
- **Sidebar rhythm.** `LIBRARIES / PLAYBACK / TOOLS`; Search in the top chrome; Settings at the
  same rhythm with a hairline; a real hit surface for collapse.

**Done when:** moving between seven screens never changes where Back lives or how a primary button
looks, and Settings no longer opens on a device table.

---

## Step 4 — An art system that survives bad assets (3–5 days)

You cannot control Jellyfin's key art, so the system has to make a weak poster look intentional.

- **Responsive card size.** 6 across at ≥1680, 5 at 1440, 4 at 1280; the same card width in rails
  and grids. 176 pt thumbnails at 1920 are the single biggest “catalogue” tell.
- **Designed fallbacks.** Replace the single-letter gradient with a typographic poster: title in
  three lines, year in mono, an 8 % duotone seeded by item id. People get a monogram disc with a
  rim and inner light.
- **Flat art at rest.** Remove the 10/24 shadow and inner wash from every poster; elevate only on
  hover/focus. Five to six larger posters will carry more weight than eight floating cards.
- **Logo lock-ups.** Draw the server `Logo` on the featured surface and Continue Watching cards,
  with text fallback; reserve height so a missing logo never shifts the layout.
- **Scrims by luminance.** The image worker already bakes a 27×17 placeholder; use its average
  luminance to choose scrim strength and black/white caption ink. No guessing from the poster.
- **Backdrops.** Request 2560; use a focal-point crop (0.3 y) so 2.35:1 → 16:9 does not decapitate
  faces. Either remove the Home ambient mural or make it a real blur at 10–12 % with a mask.
- **Episode stills.** Episodes use their own 16:9 still, not a cropped series poster; add the
  parent-backdrop fields so episode cards can use the series backdrop where a still is missing.

**Done when:** deleting every image from a grid still leaves a page that looks designed, and the
worst poster in the library does not look like a broken image.

---

## Step 5 — Make the frame budget boring (2–3 days)

Polish collapses if scrolling Home costs more than it should. Do this before adding art weight.

- **Build only on-screen Home rails**, growing on scroll/focus (the library already virtualizes).
- **Budget the decoded image cache in bytes**, and request smaller widths below the fold.
- **Trickplay:** choose the smallest tile set for the drawn size (640-wide sprites are 92 MB
  decoded for a 240×135 preview).
- **Cold start:** preload symbol fallback fonts outside the first frame (~20 ms measured).
- **Layout stability:** reserve the Coming up slot; no sidebar reflow when libraries arrive.

**Done when:** Home’s first paint stays within the existing budget, scrolling does not spike, and
no late rail insertion moves what the reader is looking at.

---

## Step 6 — Home that answers “what now” without inventing data (4–5 days)

- **One opening surface**, not a rotating Netflix billboard: a compact featured area with backdrop,
  the server logo or a typeset title, one context line, Resume and Details. Default on; honest
  about why the item is there.
- **Aggregate “New this week”** instead of one Recently Added rail per library (six near-identical
  rows on the live server). Then at most two category rails with real volume.
- **Watchlist rail** from `UserData.is_favorite`; one **“Because you watched X”** from the existing
  `/Similar` data with the same “Why this?” label; one genre and one studio row.
- **Resume from the Continue Watching card** in one press; Remove moves to the menu.
- **Card facts:** `2024 · 1h 52m`, `S2E04 · 12 min left`, `Airs 20 Nov · S2E06`. One line, two
  facts maximum.
- **See all** only when there is more, with the count (`See all 18`).

**Done when:** the first screen has five distinct reasons to press something, and no series is
sold twice.

---

## Step 7 — Detail that sells an episode (3–4 days)

- **Episode composition:** two-column grid of 16:9 stills (three at ≥1600), episode code, title,
  two-line synopsis, runtime, progress on the still, “Next” on the first unplayed. Stop using a
  1,600 px row with 1,200 px empty.
- **One season header:** remove the duplicate chip/heading; Play season, Shuffle and an overflow
  (Mark season watched) on its baseline.
- **Actions:** verbs on labels (“Mark watched”, not “Unwatched”); one primary + Trailer; Favourite
  and Watched can be icon buttons; remove the stray caret unless it is visibly a split button.
- **Scores:** one neutral plate + brand rim + white value per provider; TMDb once; drop empty
  chips.
- **More like this:** name the reason (“More in Sci-Fi”, “More from A24”), not a silent row.

**Done when:** an episode list is something a viewer would browse for pleasure, not a table.

---

## Step 8 — Search and states that recover (2–3 days)

- **Search:** recent searches only (no fixture titles — already fixed), four short deduped hints,
  a clear first result, episode stills + episode titles, portrait people, nearest-match/“Request
  it” recovery.
- **One state component:** 96 pt line-art glyph, title at 28 (not 44), one sentence, one primary.
  Use it for Home, library, search, person, Live TV and Discover.
- **Error states keep context:** “Can’t reach jellyfin.example” + Retry + “Retrying in
  10 s”; keep cached content on screen.
- **Skeletons:** 150 ms delay, shapes that match the real rails, a calm 1.2 s shimmer, and no
  sidebar reflow.

**Done when:** an empty or failed screen still tells the reader what to do next and does not look
like bare text in a void.

---

## Step 9 — Motion with intent (3–4 days)

- **One motion anchor** on the screen root; `hover_motion_within(HOVER, HOVER_EXIT)`.
- **Delete the poster’s `tx.settle()`** so the 160 ms wash is not cancelled on pointer-enter.
- **Page transitions:** 180 in / 140 out, 12 pt rise, direction-aware; panels 200/160; OSD 120/200.
- **Reduce motion actually reduces motion:** `Spring::NONE` + `tx.instant()` on input/route
  changes, with a test.
- **TV focus growth** 1.06 (hero 1.10), 200 ms, no bounce; pin the halo to `Spring::NONE`.
- **Focus rest reveals metadata** (600 ms) using the existing preview timer; no sound, no autoplay.
- **Skeletons/toasts/end card** stop popping: animated first data, scheduled toast exit, 280 ms
  video shrink at EOS.

**Done when:** nothing answers the pointer in one frame, nothing bounces text, nothing moves at
rest, and reduced motion lands in one frame.

---

## Step 10 — The 10-foot pass (3–5 days)

- **TV opening surface** default on; drop the page title; 4-up landscape/higher card type at
  1080p.
- **Controller hint bar** (fading) and first-run overlay; detect the last input device and draw
  glyphs rather than mixing “A” and “←”.
- **TV sidebar** Home / Shows / Movies / Live TV / Search + More; no 14-item scroll.
- **Focus preview** and metadata on rest; rail paging that snaps to card boundaries.
- **Safe area** for the player (5 %), settings switches ≥56×32, 44 pt targets.
- **On-screen keyboard path** for search and the one text field inside the player; document
  Steam+X honestly.
- **Hardware honesty:** a Settings line for refresh-rate matching/HDR/audio-device state; document
  what the compositor controls.

**Done when:** a couch user can navigate, search, play, change audio/subtitles, and quit without a
keyboard or pointer, and every focus move is visible from 3 m.

---

## Step 11 — The player’s last details (3–5 days)

- **Skip Intro independent of the OSD** (highest-value single player fix).
- **One safe inset**; 20 pt clock with remaining time inline; CC + audio glyphs in the bar.
- **Remote scrub preview** on hold Left/Right; chapter ticks that are either useful or gone.
- **Human track labels** (“English · SDH”); Advanced hides downmix/delay; one viewer-facing gear
  sheet.
- **Launch:** keep the artwork, spinner after 0.5 s, “Preparing 4K…” after 1.5 s; 2 s DMA-BUF
  attempt, not 8.
- **Failure:** stay in-app with Retry/software decode on TV; mpv stays a desktop setting.
- **Resume:** take the later position silently, toast it; no “This device / Server” modal.
- **Back:** raises the OSD, then closes it, then exits; keep the checkpoint.
- **End card:** bottom-right, 720 pt, backdrop art, countdown ring; honest “Up next” when autoplay
  is off.
- **Queue:** 96×54 thumbs, hover/focus handles, no admin table.

**Done when:** a viewer can watch an episode, skip the intro, change subtitles, and roll to the
next episode without seeing an implementation word or leaving the app.

---

## Step 12 — Voice and a point of view (ongoing)

- **Neutral chrome, authored accent.** Desaturate the page/sidebar; put chroma on the wordmark,
  progress and one indicator. Primary becomes a light surface with dark ink; accent stays for
  progress/focus.
- **The wordmark’s violet→cyan** becomes the signature; a two-stop progress bar is the one repeated
  motif.
- **Pixel motif used three times, never more:** the `R` as a 3-frame loading mark, 2 px stepped
  focus corners, a 3×3 dither in empty states.
- **Voice:** write empty/error/permission copy like the product, not the server.
- **A small style page** in `docs/design.md`: the eight checks every screen must pass (focal
  point, worst-asset fallback, consumer copy, focus at distance, motion latency, one accent,
  one primary, snapshot proof).

**Done when:** a screenshot with the logo cropped out is still recognisably Runefin.

---

## What I would not work on

1. Top 10 ranks, awards badges, “Leaving soon” — no data, or data that measures one person.
2. Autoplay-on-focus previews and rotating heroes — motion sickness, battery, and a personal
   library does not need to be sold.
3. Per-title licensed key art, alternate art, motion posters — a service-scale pipeline.
4. A custom typeface program — one OFL pair gets the legibility and the voice.
5. Endless personalised rows without a model — two explainable rows beat ten invented ones.
6. A second shell for TV — one layout, fewer destinations, larger type.

---

## The order in one sentence

**Strip the unfinished tells, make type and the frame consistent, build an art system that
survives the worst poster, then improve Home, detail, search and states, then make motion and the
TV/player feel intentional, and only then give it a brand voice.**
