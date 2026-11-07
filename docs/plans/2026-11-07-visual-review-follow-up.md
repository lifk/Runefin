# Visual review follow-up — execution plan

**Status:** implemented, 2026-11-07; the M3 headless focus capture and the M6 list-column variant remain optional/conditional. Follows
[`research/12-visual-review-assessment.md`](../research/12-visual-review-assessment.md) and the
external review it assesses.

**Goal.** Apply the review’s confirmed defects and the design work that survives scrutiny, without
adopting its incorrect claims or its ecosystem/product detours. Every visual change lands with
snapshots at 1080p and 4K.

**Definition of done.** The validated items below are implemented, the excluded items are not
silently reintroduced, `scripts/check.sh` is green, and the changed captures are committed with a
one-line reason for each intended diff.

---

## 1. What is in scope

| Workstream | Why it is in |
|---|---|
| Evidence fixture | The committed set has no real-art contrast test; the light shadow cannot be judged without one. |
| Light theme contrast and shadow | Measured defects: selected nav 2.78:1, black halo on light, hardcoded black shadows. |
| Gutter consistency | 40 pt vs 28 pt page edges is a real 12 pt jump. |
| List view facts | Duplicate year for films and `1 play(s)` are concrete bugs. |
| Live TV guide | Mixed `+1h`/`+75m` units, no active toolbar state, no remaining time. |
| Home/Seerr copy | The H1 changes with content; the Seerr line is internal language. |
| TV focus | 2 pt ring is 3 px at 1080p; `focus_highlight` is already at the pin and also makes Enter/CEC OK work. |
| TV type ramp | 12 pt caption is 18 px/0.22° at 3 m; needs a measured ramp, not the report’s blanket 1.35×. |
| Detail hierarchy | Seven actions, two Resume labels, orphaned Like/Dislike. |
| Player OSD | Two control languages, 4 pt resting scrubber with a hover-only knob, permanent volume. |
| Settings/context/person | No section index, 15 flat menu rows, no known-for rail. |
| Generated fallback | Optional polish after the contract fixes; still a design decision. |

## 2. Explicitly out of scope

These are removed because they are wrong, stale, or a separate product decision:

- **“TV mode is the desktop layout scaled.”** The 1280×720 *point grid* is the TV design; 4K is a
  3× render of it. The valid part is the type/density question, handled in M2.
- **The blanket 1.35× type multiplier.** It would grow text faster than cards and change density;
  M2 measures a targeted ramp.
- **`tok::accent(0, !light)` as the complete light nav fix.** That is 3.99:1; M1 uses dark text on
  the L 0.66 fill or a darker fill.
- **A real-art fixture made from personal artwork.** M0 uses a synthetic bright/dark asset.
- **“Person has no biography.”** The page renders `person-biography` when the server sends one;
  only the demo fixture lacks it. M6 adds known-for, not a biography fix.
- **`rune-kit`, `rune-fx`, `rune-light`, `system_material`.** No dependency and no visual layer
  rewrite. Borrow the state/contrast ideas if useful; do not add the crates.
- **The 7 Nov Rune API rev bump.** `focus_highlight` already exists at the pin; the rest is
  cosmetic until a feature needs it. A bump is a standalone fork/rebuild/snapshot commit.
- **`Tx::toggle_full_screen` as a blocker.** The pin already has `Tx::set_fullscreen`, which the
  app uses.
- **The 3,600 pt Settings “defect.”** The scenario is tall; the page scrolls. M6 adds an index
  because navigation is genuinely hard, not because the page is too tall.
- **“Up Next eats the episode” as a bug.** Treat it as a design option inside M5.
- **Screensaver, PIN/kids mode, server maintenance, per-series audio memory, a richer remote,
  SyncPlay.** Separate product roadmap; not this visual execution.

---

## 3. Milestones

### M0 — Real-art evidence fixture

**Why first.** The light shadow, logo-over-image and text-over-backdrop cases cannot be judged
against letter placeholders.

- [x] Add a synthetic 2:3 poster and 16:9 backdrop under `Assets/fixtures/` with bright, dark and
      high-frequency regions. No personal artwork; keep the files small.
- [x] Add a demo scene that injects the fixture into the image cache for a few cards and the
      detail backdrop, plus `home-art` / `detail-art` captures.
- [x] Keep existing hermetic scenes unchanged; the fixture is additional proof, not a replacement.
- [x] Add the scenes to `scripts/snapshots.sh` at 1080p and 4K.

**Acceptance:** the new capture shows text over bright and dark art; `scripts/check.sh` passes; no
network.

### M1 — Confirmed small fixes

**Light theme**

- [x] Fix `nav_item`: keep the L 0.66 accent and use dark `t.text` for the selected label/icon
      (≈5.45:1) or use a darker fill with white; verify with a contrast calculation/test.
- [x] Make `theme::shadowed` theme-aware. Evaluate “white shadow” against “no shadow” on the M0
      fixture; choose the one that reads over both bright and dark art.
- [x] Replace hardcoded `Color::BLACK` depths in `settings.rs` with the theme shadow tokens where
      the token matches.
- [x] Keep the selected-state accent use limited; do not invert the whole theme.

**Gutters**

- [x] Use `theme::gutter()` for page edges in library, search, live and person; keep 28 pt for
      internal panel padding where density is intentional.
- [x] Re-render narrow and desktop captures; confirm no new toolbar wrap.

**List view**

- [x] Drop the duplicate year only when `home::subtitle` already returns the year (films); keep it
      for episodes.
- [x] Pluralise plays: `1 play`, `2 plays`.
- [x] Leave real columns/`tnum` for M6; this commit is the data fix.

**Live TV**

- [x] Ruler in absolute clock times (or one unit), with a visible “now” line and hourly gridlines.
- [x] Give Day/Guide/Favourites a selected state; the label can keep its current text.
- [x] Add remaining time and a thin progress bar under the current programme; show the next start
      time.
- [x] Add tests for the ruler formatter and the selected-state flags.

**Copy and headings**

- [x] Stable Home H1 (`Home` or a stable product title); “Continue Watching” becomes the rail H2.
- [x] Change “Films and series from Seerr’s catalogue” to reader-facing copy.
- [x] Update every affected snapshot; include `home-light`, `settings`, `library`, `search`,
      `library-list`, `live-tv`, `live-guide`, and their 4K variants.

**Acceptance:** no duplicate year, “1 play” reads correctly, guide labels never mix units, selected
tools show a state, light-theme selected nav ≥4.5:1, `scripts/check.sh` green.

### M2 — Big-screen type ramp

**Decision:** keep the 720-point grid; grow only the small tokens. Do not apply a blanket 1.35×.

- [x] Measure the target at 3 m/55-inch: caption/meta/label ≥24 px at 1080p (16 design pt);
      body already reaches 24 px and stays unless the measurement says otherwise.
- [x] Add a `type_::big` (or a `big_screen` branch inside `scaled`) for `CAPTION`, `META`,
      `LABEL` and `CARD_TITLE`; leave `PAGE` and `SECTION` alone.
- [x] Recheck card-title wrapping and row heights. Bump sidebar rows and list rows only if the
      larger type clips; do not change poster geometry without a separate decision.
- [x] Render 1080p and 4K; check five-card density, truncation, and the 64 pt gutter.
- [x] Document the ramp and the angular-size calculation in `docs/design.md` §5.

**Acceptance:** caption is at least 24 px at 1080p; no clipped labels in the core screens; 4K
captures use 48 px for the ramped 16 pt caption; no change to desktop typography.

### M3 — TV focus and CEC activation

- [x] Add a small helper that applies `focus_highlight(accent.alpha(0.10))` on big-screen cards,
      rows, toolbar controls and OSD buttons. Apply it only when `is_big_screen()`.
- [x] Change big-screen focus rings from 2 pt to 4 pt (offset 2); desktop stays 2 pt.
- [x] Confirm the side effect: Rune makes a `focus_highlight` row take Return, so a CEC remote’s
      Select/OK (`KEY_ENTER`) activates cards without a `cecd` remap.
- [x] Add a runtime test: a focus-highlighted row activates on Enter; space still activates.
- [x] Keep `cecd.toml` for media keys; update its comment to say Select now works stock, while
      Play/Pause/Skip/Channel still need the mapping.
- [ ] Add a small headless focus hook (a `--focus` scene argument or a debug scene) so the
      focused state can be captured. A runtime test now proves Enter activation; no static focus
      frame is claimed.

**Acceptance:** focus is visible at couch distance; Enter and Space both activate; no double
highlight with hover/selected; desktop unchanged.

### M4 — Detail hierarchy

- [x] One primary: `Resume S2E02 · 12:00` (or `Play S2E02`) with the next-up card saying
      `Play next`, so the two CTAs do not repeat.
- [x] Move trailer, favourite, watched, playlist and collection into a secondary icon/chip row;
      keep stable keys and labels for tests.
- [x] Move Like/Dislike into the card menu or a compact rating control; remove the orphaned row.
- [x] Selected season: filled surface plus white text, not only a hairline.
- [x] Render `detail`, `detail-preplay`, `detail-versions`, 1080p and 4K; update tests for labels.

**Acceptance:** one primary action, no duplicate Resume wording, no second-row orphan, selected
season reads as selected without looking like a focus ring.

### M5 — Player OSD and Up Next

- [x] Consolidate controls into one bottom bar: play, ±10 s, time, scrubber, chapters, volume,
      subtitles, gear, fullscreen. Keep title and Back on top.
- [x] Big screen: 8 pt resting bar, 12 pt focused/hovered, always-visible knob; desktop may keep
      the hover knob.
- [x] Decide whether volume stays visible on TV; if it does, use the same icon language as the
      rest of the bar.
- [x] Reduce Up Next to a compact glass card with still, title and countdown; “Play now” primary,
      “Watch credits”/“Dismiss” secondary. This is a design option, not a bug fix.
- [x] Keep the controller/remote paths and keys; re-render `in-app-osd`, `in-app-next`,
      `in-app-gear`, plus 4K.

**Acceptance:** no hover-only control on big screen; scrubber is visible and operable with a
remote/gamepad; fullscreen/volume/gear live in one control language; playback keys still work.

### M6 — Settings, context menu, person, list columns

- [x] Settings: sticky section index (Appearance, Playback, Subtitles, Seerr, Data…), active
      section, content width 720–800 pt, “This install” as a status block.
- [x] Context menu: group Play · Organise · Share · State with separators, icons on toggles, and
      the destructive action in the danger tone with Undo.
- [x] Person: add a known-for rail (3–5 posters) and an “In your library / All credits” filter;
      biography already exists when the server sends it.
- [ ] List view: real columns with tabular figures (title · year · runtime · rating · added ·
      plays). Not adopted: the three-line row remains readable after the year/play fix, and true
      columns need a narrow-window fallback.
- [x] Snapshot `settings`, `settings-seerr`, `card-menu`, `person`, `library-list` and 4K.

**Acceptance:** Settings is navigable without search; menu groups are scannable; known-for is
visible; list numbers align.

### M7 — Optional generated fallback

Only after M1/M2, and only with design approval:

- [x] `theme::palette(item)` returns two low-chroma OKLCH stops from a stable item seed.
- [x] Use it for the card fill, detail backdrop under the scrim, and audio/library fallback.
- [x] Keep the letter/none fallback for failure; verify with the M0 art fixture.
- [x] Re-render every capture; this is a large visual diff, so commit it by itself.

If the palette weakens text contrast or makes placeholders look like real posters, stop and keep
the current neutral fallback.

---

## 4. Tests and captures

Each milestone:

- `cargo test -p runefin` (focus/Enter, guide formatter, list facts, labels).
- `scripts/check.sh`.
- `scripts/snapshots.sh`; commit every changed PNG.
- 1080p and 4K for M2–M5; desktop and light variants for M1.
- New M0 fixture in the committed set.
- A short commit note for each intended visual diff; no unexplained pixel changes.

Suggested runtime tests:

- a `focus_highlight` row activates on Enter and Space;
- the live ruler never emits both `h` and `m` in one scale;
- list facts omit the year only when the subtitle is the year;
- selected nav contrast is at least 4.5:1 (a pure color/contrast test if practical).

## 5. Risks and rollback

| Risk | Mitigation |
|---|---|
| Type ramp breaks density | Apply only small tokens; keep 720 grid; review 5-card and 4K captures; revert the ramp independently. |
| Light shadow looks wrong over art | Decide with the M0 fixture, not a flat background; keep the old shadow behind a flag if needed. |
| Focus highlight double-highlights | Apply only in big screen; test hover + keyboard + selected; keep desktop rings unchanged. |
| Enter behavior changes | `focus_highlight` is the intended Rune mechanism; test both Enter and Space; keep the cecd media-key mapping. |
| Detail/OSD key churn breaks tests | Keep public keys stable; update labels/tests in the same commit. |
| Gutter causes toolbar wrap | Capture narrow widths; keep 28 pt for internal panel padding. |
| Scope creep from the report | The out-of-scope table is authoritative; new work needs a new plan. |

## 6. Order

`M0 → M1 → M2 → M3 → M4 → M5 → M6 → optional M7`

M0 first because it supplies the evidence for M1. M1 is the low-risk confidence pass. M2 changes
the TV contract and must be reviewed before the detail/OSD redesigns. M3 fixes focus and CEC
without a Rune bump. M4–M6 are design passes. M7 is last and may be dropped.
