# Professional polish gap — Runefin vs Netflix / HBO Max / Disney+ / Apple TV

**2026-11-15.** Six parallel reviews: visual system, home merchandising, motion, player/TV, a
brutal screenshot critique, and current streaming-design research. Every claim was checked
against the live account screenshots (`/tmp/runefin-live`, `lifk` on
the home server), the committed scenes, and the code. This is not a feature list; it is
what separates a competent client from a product that feels authored.

---

## The verdict in one paragraph

Runefin already has the hard parts: a real focus router, a GPU UI that idles at zero frames, a
working player with subtitles/segments/next-up, a coherent dark palette, and an actual visual
contract. What it does not have is **art direction, merchandising, and the last 20 % of craft**.
Beside Netflix it reads as a well-built database with posters; beside Plex it reads as the more
ambitious app. The realistic target is not Netflix's asset pipeline or editorial desk — it is
**Apple TV/Plex-grade composition plus one authored point of view**. That is fully achievable.

The five-second test on the live app says: “tidy, findable, obviously built by someone who cares”
— then “why is there a ghost of an anime character behind the shelves, and why is the top third
empty?”

---

## 1. What “professional” actually means in 2026

The research is unusually consistent across services that otherwise disagree on everything:

1. **Chrome is neutral; the art is the colour.** Netflix `#141414`; Max moved to a monochrome
   identity in March 2025 and back to the HBO name in July; Apple TV is white-on-black; Disney+
   is blue-black. Runefin's fields are roughly twice as blue as Disney+'s; posters sit on a blue
   cast and the app reads as a settings panel, not a cinema. Cool-tinted near-black is right;
   strongly blue chrome is not.
2. **The accent has a pixel budget.** Saturated brand colour belongs in the wordmark, progress,
   focus, and at most one primary surface. Netflix's Play is white with black text; Apple's
   primary is white; Max's new UI is monochrome. Nothing puts 16 px white text on a saturated
   fill. (Measured on the current build: white on the state blue is **3.14:1**; near-black ink is
   **5.2:1**.)
3. **The ratio names the entity.** 2:3 for titles, 16:9 for continue-watching/episodes, 1:1 for
   people. Art bleeds; text sits on a scrim or inside a logo lock-up, never on a plate. Netflix
   has shown artwork is a decision surface, not decoration (take-rate and viewing-hours tests).
4. **One focused item carries information.** The 2025 Netflix TV redesign went from six tiles to
   four and moved to a top bar with five tabs; Disney+ moved rows to “more cinematic poster-style
   artwork” and added editorial badges. They disagree on shell; they agree on focus.
5. **Motion is judged by latency, not curves.** Netflix measures key-input responsiveness, time to
   interactivity, FPS and memory on sub-GHz TV hardware. The app must never animate at rest
   (Runefin passes) and must never make a remote feel like a mouse.
6. **The player defers to the picture.** Apple's HIG is explicit: resume without asking, show
   content immediately, keep overlays translucent (some panels retain static logos), thumbnail
   scrubbing, and land on a relevant screen on exit.
7. **TV numbers are published.** Google: 960×540 canvas, 5 % safe margin, 12 columns. Fire TV:
   body ≥14 sp (~28 px at 1080p), nothing critical in the outer 5 %. Runefin's 64 pt big-screen
   gutter matches the 5 % rule; its 16 pt card title is still below the Fire TV floor.

### What is craft, and what is scale

| A solo client can close | A service buys with scale |
|---|---|
| Neutral chrome; accent discipline | Licensed key art, alternate art per title, localised art |
| Ratio-correct art, scrims, focal crops | Artwork personalisation (bandits/embeddings) |
| Focus that never disappears; Back restores focus and scroll | Editorial rails: Top 10, “New & Popular”, awards, “leaving soon” |
| Resume-first, no prompt on the normal path | Autoplay video previews from clip libraries |
| One primary action whose label carries state | Recommendation data from millions of viewers |
| A good neutral grotesque + mono for data | A custom typeface (Netflix Sans saves them millions) |
| One motion clock, 60 fps, low latency | A/B infrastructure, sonic branding, household profiles |

**The trap:** designing every composition as if it will receive a beautiful 16:9 with no text.
Jellyfin posters come from scrapers, logos exist for perhaps half a library, and backdrops are
often a frame grab. **Design for the worst asset, not the best.**

---

## 2. Where Runefin stands

Scores are for perceived streaming-app polish, from the screenshot critique (1–10).

| Screen | Score | One line |
|---|---|---|
| Home | 6 | Solid shelves; no billboard; ghost mural; search buried in Tools |
| Library | 5 | Two toolbars and a blank first poster put the database before the films |
| Detail (film/series) | 7 | Hero is tier-1; full-width episode rows waste half the screen |
| Search | 4 | 90-character chips and six identical posters read as a query debugger |
| Live TV | 4 | A channel table with letter avatars and a manual Refresh |
| Discover (Seerr) | 6 | Best-looking browse surface, buried under four nav layers |
| Settings | 5 | Opens on a device-id table; half the page is empty |
| Player OSD | 7 | Calm and legible; the 2 px rail and corner timestamps feel unfinished |
| Player panels | 5 | Right structure, wrong words (`Undefined - ASS`, `CPU`) |
| End card / queue | 7 | End card is genuinely good; the queue is an admin table |
| States | 5 | All three exist; all three are bare text and a button |
| Theme / a11y | 6 | 130 % works; light theme has visible artifacts |

**What is already professional and must be protected:** the detail hero composition, the
Resume-first policy, the spoon-fed “Spoiler hidden” card, the end card's information set, player
target sizes, accessible names and reachability, the calm palette, and the fact that loading,
empty and error states exist at all.

---

## 3. The gap catalogue

### A. Foundations (type, colour, space, imagery)

**A1 · Type has no display range and the data face is fake.** Tokens stop at 32/20/18/16/14/14/12;
there is no 44–64 display step, no tracking token, and `mono_caption()` is not monospace. Card
titles and meta are the same size, six points apart. Fix: `DISPLAY 56/700/−1.2`, `PAGE 36/650`,
`SECTION 22/600`, `CARD_TITLE 15/600`, `CARD_META 13/450`; `.mono()` on `mono_caption`; `'tnum'`
on every duration, count and clock. Add an OFL grotesque + mono (IBM Plex Sans/Mono or Geist) and
embed it — the first real asset decision.

**A2 · The card title was clipped to one line by a bug.** `.max_lines(2).truncate()` invokes Rune's
`truncate`, which forces `one_line`; “The Adventures of Maid…”, “Ace Ventura: When Nat…” and
“The Amazing Spider-M…” are the result. Fixed in this pass: two lines, which fit inside `GRID_ROW_H`
without changing the grid rhythm.

**A3 · The accent has seven jobs.** It fills selected nav, primary CTAs, progress, focused play,
filter chips, switches and the search field. Netflix/Apple put chroma on the wordmark, progress
and at most one primary. Fix: primary action becomes a **light surface with dark ink** (the
tier-1 pattern, and it fixes contrast); accent stays for progress, focus and one indicator.
Selection becomes a neutral wash + 1 px border + white text.

**A4 · Ink on accent failed contrast.** White on the state blue is 3.14:1; near-black is 5.2:1.
The build used white for a while (a previous audit's 6.5:1 claim was wrong); reverted to dark ink.
The real fix is A3.

**A5 · Posters float; tier-1 art is flat.** Every poster carries a 10/24 shadow, a 5.5 % inner
wash and a hairline, producing a dashboard grid of floating rectangles. Remove elevation at rest;
elevate on hover/focus only.

**A6 · The Home ambient is a smudge.** A 30 % backdrop under a 40–78 % gradient plus a 340 pt band
shows a recognisable face/eyes behind the title on the live Home. Either delete it or make it
intentional: real blur, 10–12 % opacity, a left-to-right mask, no face in the title zone, and a
1920/2560 request.

**A7 · Art is thumbnail-sized at desktop widths.** A fixed 176 pt card gives 8 across at 1920;
tier-1 shows 5–6 at 260–300 pt. Fix responsive width: 6 across at ≥1680, 5 at 1440, 4 at 1280;
same width in rails and grids. Person pages then stop leaving 400 px empty.

**A8 · Placeholders look like broken images.** A single 44 pt letter on a seeded gradient reads
“asset failed”. Design a typographic poster: title in 3 lines at 15/600 over an 8 % duotone,
year in mono; people get a monogram disc with a rim and inner light. Pure code.

**A9 · Images are upscaled.** Backdrops are requested at 1600 and shown at 1920 (and more on
HiDPI); faces are visibly soft on the live detail page. Request 2560 and add a focal-point crop
(0.3 y bias) so 2.35:1 → 16:9 does not decapitate.

**A10 · Control scales are ad hoc.** Heights on one screen: 26/28/30/32/34/36/40; radii
4/5/6/7/8/9/10/12/13/14/18/26. Fix to 28/36/44 heights and 4/8/12/full radii with one `Control`
helper owning ink, hover and focus.

### B. Shell and composition

**B1 · No page-header system.** Home 40/650; Library/Search/Settings/Discover 32; Back is a 34 pt
“← Home” chip, a 32 pt inline chip, or a 92 pt slab floating on the detail backdrop. Build one
sticky `PageHeader`: back slot, title 36/650, subtitle 16 at 72 %, actions right, hairline on
scroll. Seven screens become siblings.

**B2 · Settings uses a third of the window.** A 940 pt left column with ~900 px of void at 1920.
Start with a 72 pt identity card (avatar, name, server, Sign out), move Device id/Version into an
About disclosure, use 15 px labels and 56 px rows, two columns above 1400, and a left section
rail with a visible selected state.

**B3 · Discover stacks four nav layers** before content: back+title, tabs, 640 pt search,
Explore/Browse. Collapse to one header (title/tabs/refresh), one search+mode line; show the last
column as a 40 % peek with an edge fade.

**B4 · The sidebar is a desktop IA on a TV.** Fourteen destinations; `Search` sits under “Tools”;
Settings floats below a gap; Playlists/Recordings sit inside “Libraries”. Use
`LIBRARIES / PLAYBACK / TOOLS`, put Search in the top chrome, and on TV show Home, Shows, Movies,
Live TV, Search + More. Give the collapse control a real hit surface.

**B5 · The light theme has artifacts.** A pale rectangular notch and a washed band at the top; the
active pill's ink needs re-tuned. If light ships, commit one light frame and fix it; if not,
remove it from Appearance.

### C. Home and merchandising

**C1 · The same three series are sold three times above the fold.** Live Home shows Ranma ½ in
Continue Watching *and* Coming up; VisionQuest in CW *and* Next up; Overgeared in Next up *and*
Coming up. CW dedupes by series, Next up does not dedupe against CW, and Coming up only dedupes
against itself (even though `features.md` claims otherwise). One dedupe pass with priority
CW → Next up → Coming up; Coming up keeps only series with no playable next episode. **S**, local.

**C2 · No billboard.** The only cinematic entry is a 180 pt compact hero, off by default, and it
can only feature a CW item. Promote it to the default 320–360 pt hero with the server Logo,
context line, one Resume + Details; select unfinished episode → newest backdrop. Relabel
“Featured” to “Continue watching” so it stays honest.

**C3 · “Home” spends the first screen saying “Home.”** On TV it renders at 72 px and eats a rail's
height. Drop it on big screen; on desktop use one greeting/context line.

**C4 · Every library gets a Recently Added rail.** On the live server that is six near-identical
rows (Anime, Collections, Comparativa, Movies, Películas, Shows) plus Collections again. Replace
with one aggregated **New this week** (16:9, spotlight first card), then at most two category
rails with ≥6 new items; move library-specific rails to the library page.

**C5 · No My List, no “Because you watched”, no genre/studio rows.** Favourites are already on
every item; `/Items/{id}/Similar` is already wired. Add a Watchlist rail, one honest
“Because you watched X” with the same Why-this? label, one genre and one studio row.

**C6 · Resume from Continue Watching costs two clicks.** Press opens detail; the only hover
affordance is destructive Remove. Press should resume, with a small Resume puck; Remove moves to
the card menu.

**C7 · “See all” is unconditional and often a dead end.** Show only when there is more, label
with the count (“See all 18”), and render the shelf with the same 16:9 cards, not a plain poster
grid.

**C8 · Newness stops at a 30-day NEW badge.** Derive “New season / New episode / Added 3 days
ago” from the newest episode's `DateCreated`; show one, not a tray.

**C9 · Card metadata says almost nothing.** Films get a year only; episodes get `S2E04 · name`.
Use one fact line: `2024 · 1h 52m`, `S2E04 · 12 min left`, `Airs 20 Nov · S2E06`.

### D. Cards, detail and artwork

**D1 · Episode cards repeat the series poster.** Search shows six identical Ranma posters with
truncated release names. Use the episode's own 16:9 still, episode title, `S2E03` badge, runtime.
**M**.

**D2 · Episode rows are a table that lost its columns.** A 160×90 still, code, name and
`60 min · 2024-03-21`, then 1 200 px of empty. Add a 2-line synopsis, right-aligned mono runtime,
a play chevron, progress on the still, “Next” marker; cap the row at ~1 040 pt or use a 2-column
grid of 16:9 still cards. This is the most unfinished screen in the app.

**D3 · Detail metadata is a debug dump.** `Released 2024-03-21 · Continuing · Netflix · china,
based on novel or book, alien civilization` sits above the scores. One human line
(`2024 · TV-MA · 3 seasons · Sci-Fi & Fantasy`), tagline second, scores third; studio, tags,
release date and codecs under Details. Localise dates to `Mar 2024`.

**D4 · Scores don't read as a set.** TMDb appears both as a score and a provider link; two chips
render almost black. Use one neutral plate + brand rim + white value per provider; TMDb once.

**D5 · The action row labels states, not actions.** Visible “Unwatched” with the accessible name
“Mark unwatched”; “✓ Season” means mark watched. Use verbs; move “mark season watched” to the
overflow; on a finished series offer “Play S1E01” and “New season” when one exists.

**D6 · The season chip is duplicated** as a pill and a heading. One season row; actions on its
baseline.

**D7 · Badges are the loudest textile in the grid.** `4K HDR CC +2` on most posters, 12 pt in a
black plate. Keep one default fact (NEW / New season / 4K), move the rest to the list view and
detail, put the remaining mark at the poster's bottom-left over its own scrim.

**D8 · Title logo art exists but is used only on detail.** Draw `Logo` on the hero and CW card
above the scrim, text fallback; reserve the layout height so a missing logo never shifts the row.

### E. Search and discovery

**E1 · Fixture suggestions leaked into the real app.** With no recents, Search offered “The Quiet
Floor”, “Nosferatu”, “Service”, “Harbour” — demo data. Fixed: recents only, plus “Recent searches
will appear here.”

**E2 · Hint chips are a wall of duplicates.** Eight chips, five all reading “Ranma 1-2 · Episode”,
wrapping two lines. Keep Person/Genre/Studio/BoxSet only, dedupe, max four, prefix the kind
(“Genre · Shonen”), truncate at ~32 chars.

**E3 · People are text chips.** Use 96 pt round portraits, name, “Actor · 12 titles”.

**E4 · No-results recovery is generic.** Offer nearest matches (shortened prefix/genre) and
“Request it” when Seerr is configured.

**E5 · No top result.** Tier-1 opens search with one large card (backdrop, title, Play/Details)
before the grouped rows; the eye needs an anchor.

### F. Player and 10-foot

**F1 · Skip intro only exists while the OSD is up.** The pill is a child of the bottom chrome;
intros land after the 3 s auto-hide, so in practice the viewer rarely sees it. Draw it as an
independent overlay and hold the chrome open while a segment is offered. Highest-value player fix.

**F2 · Chrome sits outside a TV's safe area.** Player padding is 20/24 pt while browse uses the
64 pt rule. One `player_inset()` at 5 % of width for top bar, OSD, panels and end card.

**F3 · The two most-read numbers are the smallest on screen.** `42:00 · -6:00` at 16 pt/64 %
white. Use 20 pt, 90 % white, inline at the rail ends, and `-48 min` for films over an hour.

**F4 · Audio/subtitles are two layers deep, and scrub is pointer-only.** Put CC and audio glyphs in
the bottom row; on a remote, hold Left/Right to drive the existing scrub preview.

**F5 · Track labels print wire titles.** “English - SUBRIP - External”, “Undefined - Default -
ASS”. Compose “English · SDH” and demote format to a second line. Move downmix/sound/delay behind
Advanced.

**F6 · The gear mixes viewer and developer furniture.** Continue on…, Statistics CPU, Minimize,
Fit, A–B loop sit beside Audio/Subtitles. Keep Audio, Subtitles, Episodes, Quality, Sleep; move the
rest behind Diagnostics or a long-press.

**F7 · The launch interstitial names the implementation.** “Starting the in-app player…” on navy;
tier-1 keeps the artwork and shows a spinner. Keep the backdrop, add a ring after 0.5 s, a
“Preparing 4K stream…” line after 1.5 s.

**F8 · Failure falls into an mpv window on a TV.** Stay in-app with a Retry/software-decode card
in big-screen mode; keep mpv for desktop behind a setting.

**F9 · Resume conflict asks “This device / Server”.** Take the later position silently and toast
“Resumed from 1:12:30”; keep the choice in diagnostics.

**F10 · The end card is bottom-left on a 2 560 px frame.** Move bottom-right, 720 pt, backdrop art
under a left scrim, a 52 pt countdown ring; drop the inner blue bar (or make it the ring's twin).
If autoplay is off, say “Up next” with no countdown.

**F11 · B ends the film.** Back should raise the OSD first, close it next, exit on the third press
(or hold), keeping the checkpoint and series page.

**F12 · TV browse has no hint layer and no focus reveal.** Add a fading bottom hint bar and a
600 ms focus-rest metadata strip (title, year, one line, Resume), reusing the existing trickplay
timer. No sound, no autoplay.

**F13 · Output is 60 Hz.** Nothing matches 23.976/24; 3:2 judder is the biggest “PC player” tell.
At minimum document it; ideally use KWin/gamescope mode APIs. Also add audio-device selection.

**F14 · TV controls are below the Fire TV floor.** 16 pt card titles ≈24 px at 1080p, below
28 px. Raise big-screen card title/meta, and use 4-up so labels stop truncating.

### G. Motion and states

**G1 · The motion ladder is true at the tokens and false at the call sites.** Rune's hover track
is instant in / 100 ms out, press release is `POP` 0.25 with bounce, mounts are 200/150, and the
root layout is `SETTLE` 300. Add page/panel/morph/OSD tokens, put one motion anchor on the screen
root, and use `hover_motion_within`.

**G2 · A poster's hover is cancelled by its own handler.** `on_hover_change` ends in `tx.settle()`,
which lands every running track; pointer-enter cuts the 160 ms wash to one frame. Delete the
settle. This one line fixes ~120 controls.

**G3 · “Reduce motion” does not reduce motion.** The setting is consulted only for theme video;
Rune's own reduction is private. Add `theme::motion::motion(spring)` returning `Spring::NONE` and
`tx.instant()` on key/route changes, with a test.

**G4 · Navigation is an unmanaged crossfade.** Pages are swapped in place at 200/150 with no rise
and no direction. Key the page root, 180 in / 140 out, 12 pt rise, reversed on pop.

**G5 · TV focus is a halo, not a state.** No growth, no metadata. Tier-1 grows the focused tile
6–12 %. Grow the card 1.06 (hero 1.10) at 200 ms, keep the halo, pin the halo position
(`Spring::NONE`), and reveal metadata on focus rest.

**G6 · Back from Home loses scroll; Back on detail glides to top.** Save/restore Home's offset and
use an instant scroll for Back.

**G7 · OSD timings are inverted** (200 in / 150 out; should be 120/200) and panel swaps crossfade
through each other. One keyed chrome wrapper; a stable panel host with a 120 ms content crossfade.

**G8 · Skeleton → content and toasts pop.** Delay skeletons 150 ms, animate the first data paint
(180 ms, 8 pt), schedule the toast's expiry wake and give it a 120 ms exit.

---

## 4. Cheap tells that make it read as a developer app

1. Raw ISO dates, provider strings and wire titles: `2024-03-21`, `china`, `based on novel or
   book`, `Undefined - Default - ASS`, `English - PGS - External - PGSSUB`, `4 d 23 h`.
2. Debug surfaces in the default view: Device id hex, Version, `Statistics / CPU`,
   `Continue on…`, `Refresh` buttons, keyboard cribs inside the gear, quality “Decision/Why”.
3. 35-chip filter soup, nine 12 px settings tabs, four stacked nav rows in Discover.
4. Every marker is the same black pill with white 12 px text — NEW, NEW EXTRA, 4K HDR CC +2 —
   plus check circles; capability facts used as merchandising.
5. Placeholder art as a saturated panel with a giant letter.
6. The backdrop mural behind shelves (a smudge in dark; a band/notch in light).
7. A 12–14 px type floor at 1080p.
8. Uneven rhythm: CW tiles 230 vs 288 px, Discover rows 258 vs 272, “See all” a chip on one
   screen and text on another.
9. Unlabelled symbolic controls: bell, star, sliders, check.
10. Truncation with no reveal, and a last column clipped mid-poster with no fade.
11. Duplicated labels: Search heading + “Search settings” + Discover search; Season 1 twice;
    Discover three times.
12. Empty/loading/error states centred in a void with no illustration, and a sidebar that reflows
    when data lands.

---

## 5. What not to copy

1. **Autoplay previews on focus.** Netflix's 2025 TV UI has documented motion-sickness complaints;
   Max built an AI tool to mine “drop-in moments”. Runefin's silent trickplay preview, off by
   default, is the right ceiling.
2. **A focus card that inflates and swallows the row.** It serves people who don't know what to
   watch; your users do. Every extra scroll is friction.
3. **An auto-rotating hero.** It fights reading and needs idle detection to be tolerable.
4. **Top 10 ranks, awards, “leaving soon”.** These need multi-user telemetry or expiry data
   Jellyfin does not have. On one user they measure your own habits and look invented.
5. **A custom typeface / sonic logo / brand-coloured fields.** One good neutral grotesque + mono
   does the legibility work; chroma belongs in the wordmark and progress.
6. **Endless personalised rows without a model.** Two honest, explainable rows beat ten invented
   ones.

---

## 6. The plan

### Stage 0 — Credibility, 2–3 days

Search fixture suggestions (done), filter booleans `is4K`/`isMissing`/`isUnaired`/`hasSubtitles`
(done), two-line card titles (done), measured ink on accent (done), a copy pass over raw dates,
provider strings, wire titles and debug labels, one `State` component for loading/empty/error,
and the light-theme artifacts.

### Stage 1 — The streaming threshold, 1–2 weeks

1. `PageHeader` and one Back pattern across seven screens.
2. Responsive card width (6/5/4 across) in grids and rails; placeholders as typographic posters.
3. Default cinematic billboard with Logo, context, Resume/Details; cut the page title on TV.
4. Cross-rail dedupe (CW → Next up → Coming up) and one aggregated “New this week”.
5. Episode stills in search and next-up; rebuild detail episode rows with synopsis/runtime/progress.
6. Detail metadata discipline, one score row, verbs on actions, one season header.
7. Settings identity card; Discover one header; Live TV programme rows with art and no Refresh.
8. Art direction: de-shadow posters at rest, remove/replace the Home mural, request 2560 backdrops,
   reduce the badge tray to one fact.

That stage changes the first screen more than anything else in this document.

### Stage 2 — Feel, 1 week

Root motion anchor + `hover_motion_within`; delete the poster `settle()`; page rise + direction;
reduce motion wired; TV focus growth and pinned halo; focus-rest metadata/preview; OSD 120/200
and a stable panel host; skeleton delay + animated first data; scheduled toast exit.

### Stage 3 — Player, 1 week

Skip intro independent of the OSD; one player safe inset; 20 pt clock/end time; CC + audio in the
bar; remote scrub preview; reader-facing track labels; a viewer's gear sheet; keep the artwork on
launch and stay in-app on failure; silent resume; Back raises the OSD; end card bottom-right with a
countdown ring; queue rows with thumbs.

### Stage 4 — Point of view, ongoing

IBM Plex Sans/Mono or Geist (+ mono for data), a light primary with dark ink, the wordmark's
violet/cyan as the accent pair and progress duotone, the pixel `R` as a three-frame loading mark,
2 px stepped focus corners, typographic placeholder posters, and a written voice for empty/error
copy. This is the stage a viewer describes as “it has a look”.

### Stage 5 — Performance guardrail

The Home rail build (P1) and image-cache byte budget must land alongside Stage 1; art-led layouts
are heavier. Measure with `scripts/bench.sh` after every stage.

---

## 7. Top 15 by perceived-quality gain per hour

| # | Change | Est. | Why it pays |
|---|---|---|---|
| 1 | Remove the ghost mural; add proper scrims | 0.5 h | The strongest unfinished signal on Home |
| 2 | Library: one toolbar row, counts to a subline, drop the A–Z rail | 2 h | Database → catalogue |
| 3 | Designed poster fallback (title/duotone), not a letter | 1.5 h | A broken first card poisons the grid |
| 4 | Detail episodes → 2-column 16:9 grid with synopsis | 3 h | Biggest composition win; half the screen returns |
| 5 | Copy pass: dates, tags, track labels, duplicate “See all” | 1 h | Highest consumer-feel per keystroke |
| 6 | Home billboard by default | 4 h | Gives the app an opening shot |
| 7 | Search: top result, short chips, landscape episode cards | 2 h | Query debugger → search experience |
| 8 | Settings identity card + About disclosure | 1.5 h | First five seconds stop being a device id |
| 9 | OSD: inset bar, 5 px rail, inline times, Next chip | 3 h | The screen watched longest |
| 10 | One state component with a glyph and real copy | 2 h | Trust at the moment something fails |
| 11 | Light theme: notch, stripes, active-pill ink | 1 h | A visible bug and a contrast fail |
| 12 | Discover: one bar, status as edge, peek/fade | 3 h | Unlocks the best content surface |
| 13 | Live TV programme rows with art; delete Refresh | 3 h | The emptiest screen becomes ordinary-good |
| 14 | Player panels: one column, 56 pt rows, formatted labels | 2 h | Removes the developer tells |
| 15 | End card right + countdown ring; queue thumbs | 2 h | A part that is already 80 % there |

Doing 1–6 moves the app from “Jellyfin skin” to “streaming app” without touching the player.

---

## 8. Definition of done — the review checklist

Every screen or change should pass all eight:

1. **Five-second test:** one focal point; the eye knows where to go; the title and one action are
   readable at a glance.
2. **Ten-foot test:** nothing below the measured type floor; focus is visible on bright and dark
   art; every control is D-pad reachable; no pointer-only action.
3. **Worst-asset test:** remove the poster, logo and backdrop. Does the fallback still look
   designed and keep its geometry?
4. **Consumer-copy test:** no implementation words, raw ISO dates, wire track titles, device ids
   or “Decision/Why” in the default layer.
5. **Continuity test:** Back restores focus and scroll; resume starts where it left off; the next
   episode is one action away; quitting saves.
6. **Motion test:** nothing moves at rest; exits are shorter; text never bounces; reduced motion
   lands in one frame.
7. **One-accent test:** count the saturated blue elements. More than wordmark + progress + one
   primary means the accent has lost meaning.
8. **Snapshot test:** the change is rendered at 1280×800, 1080p and 4K, dark and light, 130 %
   text; the diff is intended and explained.

---

## 9. Sources

- Apple HIG — [Designing for tvOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-tvos),
  [Focus and selection](https://developer.apple.com/design/human-interface-guidelines/focus-and-selection),
  [Playing video](https://developer.apple.com/design/human-interface-guidelines/playing-video).
- Google TV — [Design for TV](https://developer.android.com/design/ui/tv/guides/foundations/design-for-tv),
  [Focus system](https://developer.android.com/design/ui/tv/guides/styles/focus-system),
  [Layouts](https://developer.android.com/design/ui/tv/guides/styles/layouts),
  [Color on TV](https://developer.android.com/design/ui/tv/guides/foundations/color-on-tv).
- Amazon — [Fire TV Design & UX Guidelines](https://developer.amazon.com/docs/fire-tv/design-and-user-experience-guidelines.html).
- Smashing — [Designing For TV: Principles, Patterns And Practical Guidance](https://www.smashingmagazine.com/2025/09/designing-tv-principles-patterns-practical-guidance/)
  (2025-09-04).
- Netflix — [New TV layout](https://www.netflix.com/tudum/articles/netflix-new-tv-layout) (2025-05-07)
  and [the reception](https://www.techradar.com/streaming/the-new-ui-is-borderline-unusable-netflix-subscribers-are-still-complaining-about-the-app-re-design-and-i-am-100-percent-with-them) (2025-09-18);
  TechBlog [artwork A/B](https://netflixtechblog.com/selecting-the-best-artwork-for-videos-through-a-b-testing-f6155c4595f6),
  [artwork personalisation](https://netflixtechblog.com/artwork-personalization-at-netflix-c589f074ad76),
  [high-performance TV UI](https://netflixtechblog.com/crafting-a-high-performance-tv-user-interface-using-react-3350e5a6ad3b).
- Brand churn — [Variety on Max going monochrome](https://variety.com/2025/digital/news/max-changes-logo-black-and-white-hbo-1236352161/),
  [Disney+ redesign](https://www.disneyplus.com/explore/articles/disney-plus-app-redesign-new-features).
- Baseline — [Plex's rewrite](https://www.plex.tv/blog/the-new-plex-experience/),
  [Jellyfin's austerity](https://www.howtogeek.com/why-i-folded-on-jellyfin-after-weeks-of-trying-to-ditch-plex/).

---

## 10. Bottom line

The app needs **fewer new features and more authorship**. The three things that would most change
how professional it looks are: **(1) a real billboard and cross-rail dedupe on Home**, **(2) an
episode/detail composition that uses the width it has**, and **(3) a neutral, light-primary art
system with a proper type pair and a designed fallback**. The player is already close; the browse
surfaces, states and copy are where the “Jellyfin skin” impression lives. None of this requires a
web view, a service, or a design team — it requires a small system and the discipline to apply it
to every screen.
