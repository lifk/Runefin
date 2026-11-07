# Assessment of the visual review and Rune ecosystem scan

**Reviewed report:** `runefin-report-2026-11-07.md`, written against `86d5768`.
**This assessment:** 2026-11-07, against the same tree plus a fresh read of
`theme.rs`, the screen sources and the committed captures.
**Purpose:** separate findings that are true and actionable from ones that are
stale, overstated, or a product decision disguised as a defect. No pixels were changed.

---

## Verdict

The report is useful and mostly accurate on the parts that touch the code. The
measured light-theme contrasts are exact (2.78:1 and 3.54:1), the duplicate year
and `play(s)` strings are real, the guide ruler really mixes `+1h` and `+75m`,
and `focus_highlight` really exists at the pinned Rune rev.

It is less reliable when it generalises: “TV mode is the desktop layout scaled”
is the wrong framing, the proposed 1.35× type branch is not a drop-in, and the
rune-kit/rev-bump parts are ecosystem opportunities rather than work this app
should do now.

There are three buckets:

1. **Apply now** — small, evidence-backed fixes with snapshots.
2. **Design pass** — real problems, but they need a visual decision and a
   snapshot diff, not a literal patch from the report.
3. **Defer / reject** — ecosystem options and product features that are outside
   the current visual contract.

---

## 1. Findings confirmed by the code

| Report claim | Status | Evidence | Applicable action |
|---|---|---|---|
| Light nav label on the accent pill is 2.78:1 | **Confirmed** | `nav_item` always fills with `tok::accent(0, true)` and writes the label in `t.bg`; the computed ratio is 2.78:1. | Fix for light mode. |
| Light accent as text is 3.54:1 | **Confirmed** | `tok::accent(0, false)` on the light `bg` computes to 3.54:1. | Fine for large headings; do not use for 14 pt labels. |
| `readable_text` always uses a black shadow | **Confirmed** | `theme::shadowed` hardcodes `Color::BLACK.alpha(0.80)`. | Make it theme-aware, but test over real art first. |
| Gutters differ: 40 vs 28 pt | **Confirmed** | `theme::gutter()` is 40; library/search/live (and person) use `28.0`. | Use one page gutter, then re-render affected screens. |
| List view prints the year twice for films | **Confirmed** | `home::subtitle` returns the year for a film; `library_list_row` also pushes `production_year`. | Drop the duplicate only when the subtitle is the year; episodes still need it. |
| `1 play(s)` | **Confirmed** | `format!("{plays} play(s)")`. | “1 play” / “n plays”. |
| Guide ruler mixes `+1h` and `+75m` | **Confirmed** | `live.rs` formats `% 60 == 0` as hours, everything else as minutes. | Absolute clock times or one unit. |
| Day/Guide/Favourites have no active state | **Confirmed** | `live::button` always fills with `t.card`; no selected flag. | Add selected styling. |
| Home H1 changes with content | **Confirmed** | `content()` chooses `Home` when `resume` is empty, `Continue Watching` otherwise. | Stable H1; make the rail an H2. |
| Seerr copy is internal language | **Confirmed** | `seerr.rs`: “Films and series from Seerr’s catalogue”. | Copy change. |
| Settings shadows hardcode black | **Confirmed** | `settings.rs` uses `Color::BLACK` in several `box_shadow`/`shadow` calls. | Prefer `t.shadow`/`t.shadow_soft` if the tokens fit. |
| Caption is 12 pt and no big-screen type token differs | **Confirmed** | `type_::CAPTION` is 12; `is_big_screen()` changes only the gutter. | Real concern; see §2. |
| Focus ring is 2 pt | **Confirmed** | Every control uses `focus_ring(2.0, 2.0, FOCUS)`; at 1.5× that is 3 px. | Big-screen ring/highlight pass. |
| `focus_highlight` is available at the pin | **Confirmed** | `El::focus_highlight` is in the pinned Rune source; Runefin uses it nowhere. | Use it for TV. It also makes Return activate a highlighted row. |
| Detail has seven equal actions and a second Resume | **Confirmed** | `detail.rs` builds Resume/Start over/Trailer/Favorite/Watched plus a separate Like/Dislike row; Next up uses `Resume <time>` from the same item when resumable. | Design pass. |
| OSD keeps a 4 pt resting seek bar | **Confirmed** | `seek_bar` uses 4 pt resting, 8 pt under the pointer; the knob is hover-only. | Big-screen OSD pass. |
| Person page lacks known-for/grouping | **Confirmed** | `PersonPage` has biography and a flat credits grid; no known-for rail. | Design pass. |
| Person page has no biography | **Incorrect** | `person_screen` renders `person-biography` when the server sends a biography; only the demo fixture has `biography: None`. | Do not “fix” a code gap that is not there. |

---

## 2. Confirmed but not a literal patch

### 2.1 TV type scale

The factual half is right: no type token changes for big screen. But the framing
“desktop scaled 1.5×” misses the actual design decision: desktop is 1280×800,
big screen is a **1280×720-point design grid** with a 64 pt gutter. That is 720
logical points, not a 720-line image: Rune renders 1920×1080 at 1.5× and
3840×2160 at 3×. The panel is 4K when it is 4K; the design grid only fixes the
size of a point relative to the screen height.

The 12 pt caption is therefore 1.67 % of the screen height on every panel
(18 px at 1080p, 36 px at 4K). A 4K panel makes the same caption sharper, not
physically larger, unless the panel itself is larger or the viewer sits closer.
That is why the 3 m/55-inch angle still applies at 4K, and why 720 points is a
TV design scale rather than a resolution.

The 12 pt caption is genuinely small at 3 m (18 px at 1080p, 0.22°). The
report’s `big(base) * 1.35` would make it 16.2 pt, but it would also grow body
text to 21.6 pt and card titles to 18.9 pt while poster geometry stays
176×264 — more wrapping, fewer useful words, and a different density at 720p,
1080p and 4K. It is a design change, not a one-line seam like `text_scale()`.

**Applicable version:** define a big-screen **type ramp** (probably only
caption/meta/label/card-title, not page/section), decide whether card and row
heights grow with it, then capture 1080p and 4K. The report’s own 0.25–0.33°
threshold is a good acceptance test; its blanket 1.35 multiplier is not a
drop-in.

### 2.2 Light-theme nav fix

`tok::accent(0, false)` with the existing near-white label is **3.99:1**, not
AA. Dropping the light accent to L 0.52 gets white to about 5.6:1, but the
cheaper fix is to keep the L 0.66 blue and use dark `t.text` on it (about
5.45:1). Whichever is chosen, the report’s one-line `accent(0, !light)` alone
does not finish the job.

### 2.3 TV focus

`focus_highlight` is the right primitive and needs no Rune bump. It does two
things the report did not connect:

- it gives the couch a fill as well as a thin ring;
- Rune treats a `focus_highlight` row as taking Return, so the same change makes
  a CEC remote’s OK/Enter activate cards without remapping `cecd`.

That makes it the highest-value single change in the report.

### 2.4 Real artwork

The report is right that the committed 141 captures are hermetic and all use
letter placeholders. It is too strong to say artwork is untested: a live
1080p capture against the real server was checked earlier. The gap is in the
**committed contract**. Add a synthetic high-contrast art fixture (not personal
art) so text-over-art is tested reproducibly.

### 2.5 Detail, OSD, Up Next, settings, context menu

These are real observations, but they are design directions, not defects with a
single correct fix:

- the seven-action detail row and the second Resume wording;
- the OSD’s two control languages and hover-only scrubber;
- the full-width Up Next card (could be a deliberate climax treatment);
- the 3,600 pt Settings scene (the real page scrolls; a section index is a
  feature, not a bug);
- the flat 15-row context menu.

Apply them only with a design pass and before/after captures.

### 2.6 List-view year

“Drop the year from `facts`” is right for films, wrong for episodes: an
episode’s subtitle is `S2E04 · name`, and its year is still useful. Make the
dedupe conditional on the subtitle being the year.

---

## 3. Ecosystem section — what is applicable

The GitHub numbers are close to current (3,540 stars, 101 forks, 91 open issues
at this reading; the report said 3,535/100/91), the repo really is two weeks old,
there are no releases, and PR #96 (“Rewind”) is still open. The `awesome-rune`
projects named in the report exist; Flicks and stremio-rune are the useful
peers.

Applicability:

- **Useful now:** `focus_highlight`; the pin discipline; the report’s warning
  that the snapshot suite is the defence against framework churn.
- **Defer:** the 7 Nov API batch. `scroll_fade`, `help_keys`,
  `ScrollBarStyle::native`, `border_sides`, `cutout_circle`, `anchored_round`
  and `matched_crop` are not needed for the findings above. A Rune pin bump is
  a standalone commit, a fork rebase and a 141-snapshot review per `AGENTS.md`;
  spend it when a feature needs an API, not for cosmetics.
- **Correction:** the app already uses `Tx::set_fullscreen`; the report’s
  `Tx::toggle_full_screen` line is not a blocker for this tree.
- **Reject now:** `rune-kit` as a dependency. Its state model and contrast
  floors are worth borrowing, but its 12 px desktop density contradicts the
  TV type finding, and replacing the hand-built controls is a framework-sized
  rewrite. `rune-fx`, `rune-light` and `system_material` are not justified by
  this app’s needs.

The report’s product ideas — ambient screensaver, PIN, server maintenance,
per-series audio memory, a real remote, SyncPlay — are a separate roadmap. They
are not “applicable” to a visual review, even where they are good ideas.

---

## 4. Recommended order

**Commit 1 — small fixes, no dependency:**

- light-theme nav contrast and `readable_text` shadow, with a real-art fixture;
- one page gutter;
- list-view year/pluralisation;
- live-guide ruler and active buttons;
- stable Home heading; Seerr copy; settings shadow tokens.

**Commit 2 — TV focus:**

- `focus_highlight` on cards/rows; 4 pt ring in big screen; verify Enter/CEC
  activation. No Rune bump.

**Commit 3 — optional fallback:**

- generated poster/hero palette; decide whether it belongs in the design
  contract before touching all captures.

**Design pass, not a patch:** detail action hierarchy, OSD consolidation,
Settings index, context menu groups, person page, TV type ramp.

**Defer:** rune-kit, rune-fx/rune-light/system_material, the 7 Nov API rev bump,
and the product features.

---

## Bottom line

The report is a strong diagnosis of concrete defects and a weaker prescription.
Its best applicable material is Part I §3.3 (light contrast/shadow), §3.4
(gutters), §3.9 (list detail), §3.8 (live guide), and Part I §3.2/Part II’s
`focus_highlight` note. Its TV type-scale proposal, rune-kit adoption, rev-bump
list and product roadmap should be treated as options, not scheduled work.

The resulting work is sequenced in
[`plans/2026-11-07-visual-review-follow-up.md`](../plans/2026-11-07-visual-review-follow-up.md).
