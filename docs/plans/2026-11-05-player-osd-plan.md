# Player OSD — minimal centre transport

**Status:** **implemented 2026-11-06**. Shape **C** was chosen from three mockups (A: one row +
More; B: grouped; C: minimal centre). S0–S3 landed in three commits plus two test commits. The
sheet is a two-column wrapped grid rather than two explicit columns: Rune's hit test missed rows
nested one row deeper, and the wrap keeps every row addressable. The ledger test pins the
controls; the frame-budget and live perf suites are green.

**Baseline:** `d5b6bea` plus the uncommitted Seerr work in the working tree. The app runs against
Jellyfin **12.4.0**; the player is the in-app GStreamer path with the mpv fallback.

**Contract:** [`../design-player.md`](../design-player.md) §"Shipped in the app" (the OSD
paragraphs, ~lines 127–256) and [`../design.md`](../design.md) §4 (the transport takes a second
line at 800 points). The OSD paragraphs are the part this plan amends; everything else about
playback (decode, seek, reporting, subtitles, HDR) is untouched.

**Evidence:** the current OSD rendered at 1280, 1024 and 800 wide
(`docs/snapshots/in-app-osd.png`, `in-app-narrow.png`, `in-app-narrow-small.png`). Worst realistic
episode (two audio tracks, subtitles, chapters, a queue, an intro segment) offers **24 interactive
targets** across three groups:

| Where | Controls | Count |
|---|---|---|
| Top bar | Back, Minimize, Fullscreen, CPU/stats | 4 |
| Timeline row | Chapters, Sleep, queue position | 3 |
| Transport | ±10 s, play, skip, audio, subs, fit, speed, quality, A–B, go-to, volume, mute, prev, next, transfer, stop | 17 |

At 800×600 the transport wraps to three lines and separates the volume from mute. There is no
hierarchy between *play*, *choose* and *configure*: six menus sit in the action row, and the
longest string in the bar is a keyboard hint that repeats the buttons.

---

## 0. Outcome and non-goals

### 0.1 The user-visible outcome

1. The OSD without its sheet shows **nine targets** (ten while a skip segment is offered):
   Back, Fullscreen, back 10, play/pause, forward 10, queue position, volume, mute, gear.
2. The frame is the interface. At rest the chrome fades to a 4 pt timeline and nothing else; on
   pointer move it returns to the same nine targets at every window size.
3. One **gear** opens a sheet with every remaining action, grouped, each row showing its current
   value. Nothing is removed, only re-homed; every keyboard shortcut still works.
4. A series can still be driven end to end without the gear: `Space`, `←/→`, `N/P`, `S`, `C`,
   `Esc`.

### 0.2 Non-goals

- Changing decode, seek, reporting, subtitles, HDR, trickplay or the up-next strip.
- A new player theme, a second renderer, or a hover-only control.
- Removing any feature or shortcut. The gear is a presentation change.
- Reworking the pre-play sheet (separate, smaller follow-up; see §11).
- Touch gestures, a mobile layout, or a controller mapping.

---

## 1. The shape

### 1.1 Layout

```
┌──────────────────────────────────────────────────────────────┐
│ ← Back   Shōgun / S1E01 · Anjin                  Fullscreen  │  top bar
│                                                              │
│                     ⟨10⟩   ▶   ⟨10⟩                          │  centre cluster
│                                                              │
│                                            [ Skip credits ]  │  skip pill, only in a segment
│ ──────────────────────────────────────────────────────────── │  timeline
│ 42:00 · -6:00  [2 of 12]            ▬▬▬▬  🔊  ⚙              │  timeline row
└──────────────────────────────────────────────────────────────┘
```

- **Top bar:** `← Back` (the only stop), title and episode label, `Fullscreen`.
- **Centre:** back 10, play/pause, forward 10. Vertically centred, 52 pt play button, 40 pt sides.
- **Timeline row:** elapsed, remaining, queue chip on the left; volume slider, mute and gear on the
  right.
- **Skip pill:** its own line, right-aligned above the timeline row, only while a segment is
  offered (§1.5). It takes no space when absent.
- **Gear sheet:** a right-anchored panel in the same slot as the existing panels, grouped into
  *Tracks*, *Playback*, *Navigate*, *Timers*, *This device*.

### 1.2 Control map — nothing is lost

| Today | In C | Key |
|---|---|---|
| `← Back` (stop) | top bar | `in-app-stop` |
| bottom `■` stop | **removed** (duplicate) | — |
| Minimize | gear → This device | `in-app-minimize` |
| Fullscreen / Windowed | top bar | `in-app-fullscreen` |
| `CPU` sink chip | gear → This device → Statistics, with the sink path as its value | `in-app-stats-toggle` |
| Chapters | gear → Navigate | `in-app-chapters` |
| Sleep | gear → Timers | `in-app-sleep` |
| queue `2 of 12` | timeline row | `in-app-queue` |
| back 10 / forward 10 | centre | `in-app-back-10`, `in-app-forward-10` |
| play / pause | centre | `in-app-pause` |
| Skip intro/credits/recap/preview | floating pill, bottom right, above the timeline row | `in-app-skip-segment` |
| Audio | gear → Tracks | `in-app-audio` |
| Subtitles | gear → Tracks | `in-app-subtitles` |
| subtitle delay `+0.5 s` | gear row value; a label by the clock while non-zero | `in-app-subtitle-delay` |
| Fit / Fill | gear → Playback | `in-app-fit` |
| speed `1×` | gear → Playback | `in-app-speed` |
| quality `Auto` | gear → Playback | `in-app-quality` |
| A–B loop | gear → Timers | `in-app-loop` |
| Go to time | gear → Navigate | `in-app-goto` |
| volume slider, mute | timeline row | `in-app-volume-slider`, `in-app-mute` |
| Prev / Next episode | gear → Navigate (N/P unchanged) | `in-app-prev-episode`, `in-app-next-episode` |
| Continue on… | gear → This device | `in-app-transfer` |
| — | gear button | `in-app-gear` (new) |
| — | gear sheet | `in-app-gear-menu` (new) |

Rows that open an existing panel keep that panel's key on the row, so `find(key)` still means
"the way to this action". The sheet shows the current value on the right of each row
(`Subtitles · 1/2`, `Quality · Auto`, `Speed · 1×`, `Sleep · off`), so state stays visible without
opening anything.

### 1.3 Live TV

The centre cluster keeps only play/pause. `live_status` stays in the timeline row with `LIVE`,
channel, now/next and the two channel buttons; volume, mute and gear join the right side. The
channel glyphs move from `⏮ CH`/`CH ⏭` to the same chevron icons the transport uses (§2).

### 1.4 Functionality ledger

Everything the player can do today, and where it lives in C. "Unchanged" means the code is not
touched; every row exists so the plan can be checked against the tree before and after.

| Surface | Today | In C |
|---|---|---|
| Top bar | Back, Minimize, Fullscreen, sink chip | Back, Fullscreen; Minimize and Statistics move to the gear |
| Timeline | drag/scrub, chapter ticks, segment marks, loop marks, knob | **unchanged** |
| Trickplay | hover preview: still, time, chapter under the pointer | **unchanged** |
| Clock | elapsed, remaining | **unchanged** |
| Centre | — | back 10, play/pause, forward 10 |
| Skip | chip in the transport row | floating bottom-right pill (§1.5) |
| Tracks | Audio, Subs chips, delay label | gear rows; the delay label stays by the clock while non-zero |
| Playback | Fit/Fill, speed, quality, A–B, Go to | gear rows |
| Episode stepping | Prev/Next chips | gear rows; `N`/`P` unchanged |
| Queue | `2 of 12` chip | timeline row, **unchanged** |
| Timers | Sleep chip | gear row |
| Window | Minimize, Fullscreen | Fullscreen in the top bar; Minimize in the gear |
| Diagnostics | sink chip → stats overlay | gear row (its value keeps `CPU` / `DMA-BUF P010` visible) → **the same** overlay |
| Transfer | Continue on… | gear row → **the same** picker |
| Panels | audio, subtitles (tracks/delay/size/search online), quality, chapters, queue, sleep, go-to, stats | **unchanged**, opened from the gear |
| Up next | full-width strip: still, label, title, countdown, Play now, Watch credits, Dismiss | **unchanged**, sits above the skip pill |
| Still watching | countdown, Keep watching, Stop | **unchanged** |
| Resume conflict | resume / start-over card | **unchanged** |
| Buffering | `Buffering… 42 %` chip | **unchanged** |
| Subtitles | text and PGS/VobSub overlays, lifted above the OSD | **unchanged** |
| Launching | spinner and Cancel | **unchanged** |
| Pointer | move shows the OSD, it hides with the chrome; click on video toggles pause | **unchanged** |

Nothing in this table is removed. The only deletions are the duplicate bottom stop and the
persistent hint line; the `CPU` chip is relabelled *Statistics* with the sink path kept as its
value.

### 1.5 Skip segments

The skip control is the one thing that must stay dynamic and appear at the right moment.

- **Placement:** a small pill, right-aligned with the 24 pt gutter, immediately above the timeline
  row. It is not in the transport row and occupies no space when absent.
- **When:** only while a segment is offered. `Ask` (the default): from **5 s before** the segment
  starts until it ends. `Always`: no pill; the player seeks past and offers Undo. `Never` and Live
  TV: no pill. The lead is today's `upcoming_in_app_segment(5_000)`.
- **Label:** `Intro → Skip intro`, `Outro → Skip credits`, `Recap → Skip recap`, any other kind →
  `Skip preview`. The label comes from the server's segment kind, never from the clock.
- **Action:** click or `S` seeks to `segment.end_ms()` and keeps the OSD awake.
- **Stacking:** the up-next strip is full width and sits above the pill; the pill is never covered.
  When both are absent, the timeline row is the bottom of the chrome.
- **No collision:** the pill has its own line, so it never overlaps the volume, mute or gear.
- **One source of truth:** the pill and the `S` key must read the same helper
  (`offered_in_app_segment()`), so the key works during the 5 s lead too (§2.6).

### 1.6 Keyboard, unchanged

`Space` pause · `←/→` seek step · `,`/`.` frame step · `↑/↓` volume · `M` mute · `C` subtitles ·
`S` skip · `[`/`]` subtitle delay · `-`/`=` subtitle size · `N`/`P` episode · `>`/`<` speed ·
`F11` fullscreen · `?` shortcuts · `Esc` closes panel → gear → stop.

The only addition is the gear in the `Esc` chain. Every binding stays remappable as it is today.

---

## 2. Defects fixed on the way

These are independent of the layout and land first, in their own commit.

1. **The quality menu cannot open.** `State::toggle_quality_menu` (`state.rs:1421`) sets
   `in_app_quality_open = false` inside the branch that just set it to `true` (line 1429); the
   line was meant to be `in_app_goto_open = false`. Clicking `Auto` on a real player does nothing.
   The snapshot passes because the fixture sets the flag directly.
2. **The player menus are not exclusive.** `toggle_in_app_chapters` closes audio, subtitles and
   stats but leaves goto, quality, sleep and queue set; the render chain is an `if/else if`, so a
   stale flag reappears when the visible panel closes. Replace the per-menu `if` blocks with one
   `open_player_panel(Panel)` helper and a test that opening any panel closes the other six.
3. **Tofu glyphs.** `⏮ Prev` / `Next ⏭` (`in_app.rs:2726`, `:2735`) and `⏮ CH` / `CH ⏭`
   (`:2309`, `:2312`) use U+23EE/U+23ED, which the UI font does not carry: the committed snapshots
   show empty boxes. Use `icon_control` with the transport's own chevron/bar icons.
4. **The hint lies.** `←/→ 10 s` and the `Back 10 seconds` tooltip are hardcoded while
   `seek_in_app_step` uses the configured 5/10/30/60 s step. Derive both from `s.seek_step_ms`.
5. **Duplicate stop.** The top `← Back` and the bottom `■` both call `stop_in_app`. C keeps the
   top one only; this falls out of the layout, but the test should assert one stop target.
6. **The skip key and the skip pill disagree.** In `Ask` mode the pill appears five seconds before
   a segment (`upcoming_in_app_segment(5_000)`), but `S` only fires inside it
   (`active_in_app_segment`, `main.rs:241`). Pressing `S` while the pill is already visible does
   nothing. Both must read one `offered_in_app_segment()` helper, and the test must cover the
   five-second lead.

---

## 3. State and screen changes

### 3.1 State

- `State.in_app_gear_open: bool` beside the other panel flags; `toggle_in_app_gear()`,
  `close_in_app_gear()`.
- `open_player_panel(Panel)` closes every panel flag, then sets the chosen one. `Panel` covers
  Audio, Subtitles, Quality, Chapters, Queue, Sleep, Goto, Gear, None. The gear closes when a row
  opens a panel, so `Esc` steps *panel → gear → OSD*.
- `in_app_gear_open` joins the "keep the OSD awake" condition (the same place the other panels
  clear `osd_hide_at`).

### 3.2 Screen

- `bottom_osd` (`in_app.rs:2451`) loses the meta-row chips and the transport row; it keeps the
  timeline, the clock row, the new right cluster, and a `skip_pill` line that renders nothing when
  no segment is offered. `seek_bar` is unchanged.
- New `skip_pill` (one row, `justify_end`) above the timeline row. It reads the same
  `offered_in_app_segment()` helper as `S`, so the label and the key can never disagree.
- `top_bar` (`:2006`) loses Minimize and the sink chip.
- New `gear_panel` beside `quality_panel`, same panel geometry, section headings, rows built from
  the existing `choice_row`/`stepper_row` helpers where they fit.
- `next_up_banner`'s bottom padding drops to clear the pill rather than the old transport row; the
  strip itself is unchanged.
- `transport_primary`/`icon_control`/`control` stay as they are; the centre cluster reuses them and
  keeps the `in-app-transport` key. The bottom layer keeps `in-app-controls`, so `--tree`, scripted
  taps and tests address the same anchors as today.
- The persistent hint (`:2552`) is removed. The shortcuts move to a single quiet footer line in
  the gear sheet (`Space pause · ←/→ seek · N/P episode · C subtitles · Esc stop`), derived from
  the configured step.

### 3.3 Narrow windows

At 800 points the timeline row wraps once: clock and queue on the first line, volume/mute/gear on
the second. The centre cluster and the skip pill do not move. This replaces the current three-line
wrap and is the reason `in-app-narrow` and `in-app-narrow-small` must be re-rendered and inspected,
not just regenerated.

### 3.4 Accessibility

- Every row keeps a visible label; no icon-only target is added except the gear, which carries
  `label("Player settings")` and `help("Tracks, quality, chapters, timers")`.
- Tab order: top bar → centre → timeline row → gear sheet. The sheet is not focus-trapped; `Esc`
  closes it, matching the existing panels.
- The volume slider keeps its current role and keys; the gear is reachable without a pointer.
- 130 % text: the sheet scrolls, the timeline row wraps, the centre cluster stays centred.

---

## 4. Work breakdown

### S0 — the five defects (0.5 day)

- [x] Fix `toggle_quality_menu` and replace the per-menu close blocks with `open_player_panel`.
- [x] Add the regression tests in §5.
- [x] Replace the four tofu labels with `icon_control`; derive the seek labels and hint from
  `seek_step_ms`.
- [x] Add `offered_in_app_segment()` and use it for both the pill and `S`.
- [x] Remove the duplicate bottom stop.

**Done when:** clicking `Auto` opens the quality panel on a real player; opening a second panel
closes the first; `S` skips during the five-second lead; `in-app-osd` shows no empty boxes.

### S1 — the minimal OSD (1–1.5 days)

- [x] Split `bottom_osd` into `timeline_row` and `centre_cluster`; move the clock, queue, volume,
  mute and gear into the row.
- [x] Add `skip_pill` above the row with the timing, labels and stacking of §1.5.
- [x] Trim `top_bar` to Back, title, Fullscreen.
- [x] Add `in_app_gear_open`, `toggle_in_app_gear`, and the `Esc` chain.
- [x] Delete the persistent hint; add the sheet footer.

**Done when:** the OSD without the gear has exactly the nine targets in §1.1 at 1440, 1024 and
800; the pill appears only in a segment and clears the up-next strip; the frame is quiet at rest.

### S2 — the gear sheet (1 day)

- [x] Build `gear_panel` with the five sections and one row per control map entry.
- [x] Show each row's current value; disable Prev/Next when there is no neighbour.
- [x] Route each row to the existing action or panel; the sheet closes when a panel opens.
- [x] Keep `in-app-gear-menu` out of the `Esc`-to-stop path.

**Done when:** every action in §1.2 is reachable in at most two clicks and the sheet fits 800×600
without clipping.

### S3 — docs, snapshots, counts (0.5 day)

- [x] Amend `design-player.md` §"Shipped in the app" to the new inventory, and `design.md` §4's
  transport sentence.
- [x] Update `features.md`, `accessibility.md`, `HANDOFF.md`, `snapshots/README.md` counts.
- [x] Render the full snapshot set and inspect the player scenes by eye, not only by exit code.

**Done when:** `scripts/check.sh` and `scripts/snapshots.sh` are green, the counts match, and the
new `in-app-gear` scene exists.

---

## 5. Test matrix

| Layer | Where | What |
|---|---|---|
| Menu exclusivity | `crates/runefin/src/main.rs` | Opening each of the seven panels closes the other six; the quality button opens the quality panel (regression for §2.1). |
| OSD inventory | same | Without the gear: `in-app-quality`, `in-app-chapters`, `in-app-audio`, `in-app-speed` are absent; `in-app-gear`, `in-app-back-10`, `in-app-volume-slider`, `in-app-mute` are present. With the gear: the first four are present. |
| One stop | same | Exactly one target calls `stop_in_app`; `in-app-stop-bottom` no longer exists. |
| Skip pill | same | Intro/Outro/Recap/Preview produce the right label; `Ask` shows it 5 s before and hides it at the end; `S` works during that lead; `Always` shows no pill and seeks; `Never` and live show none; the pill sits above the timeline row and clears the up-next strip. |
| Ledger | same | A const list of every player key is reachable in the mode that shows it — bar, gear, panel, banner, overlay — so a lost control fails the suite instead of a snapshot. |
| Step labels | same | With `seek_step_ms = 30_000`, the back/forward labels and the sheet footer say 30 s. |
| Gear routing | same | Each gear row invokes its action (spot-check quality, chapters, sleep, stats, transfer) and closes the sheet when it opens a panel. |
| Escape chain | same | `Esc` closes panel → gear → stops, in that order. |
| Narrow | same | At 800×600 the gear opens, the row wraps once, and the centre cluster keeps three targets. |
| Snapshots | `scripts/snapshots.sh` | `in-app-osd`, `in-app-narrow`, `in-app-narrow-small`, `in-app-live`, `in-app-gear` (new). |

**No test hits the network.** The player tests drive `State`/`Msg` directly, as today.

---

## 6. Snapshot matrix

| Scene | Size | Proves |
|---|---|---|
| `in-app-osd` | 1280×800 | Nine targets, quiet timeline, no hint, no tofu. |
| `in-app-gear` | 1280×800 | The sheet: five sections, current values, footer shortcuts. **New.** |
| `in-app-narrow` | 1024×700 | One wrap, centre intact. |
| `in-app-narrow-small` | 800×600 | The same at the smallest window. |
| `in-app-live` | 1280×800 | Centre play only; LIVE row with channel icons. |
| `in-app-quality`, `-audio`, `-chapters`, `-queue-panel`, `-sleep-timer`, `-goto`, `-transfer`, `-stats` | 1280×800 | Panels unchanged, opened from the gear; the minimal bar behind them. |
| `in-app-intro`, `-segments`, `-next` | 1280×800 | Skip pill for an intro, and the up-next strip above it. |
| `in-app-skip-credits` | 1280×800 | The credits label, bottom-right placement, and the strip above it. **New.** |

All other `in-app-*` scenes re-render because the bar behind them changed; inspect them, do not
just accept the diff. The new total is **108 scenes** (two added), captures to be confirmed by
`scripts/snapshots.sh`.

---

## 7. Documentation and counts

| File | Change |
|---|---|
| `docs/design-player.md` | The OSD paragraphs: the nine targets, the gear, the sheet sections, the Esc chain, the removed hint. The quality/chapters/sleep/A–B/go-to/transfer/stats/prev-next sentences move from "chip" to "gear row". |
| `docs/design.md` | §4: the transport no longer takes a second line at 800; the timeline row does. |
| `docs/features.md` | The player section: describe the gear and the nine targets. |
| `docs/accessibility.md` | Tab order and the keyboard list; the new snapshot count. |
| `docs/HANDOFF.md` | Counts and the player row. |
| `docs/snapshots/README.md` | `in-app-gear`, the new counts. |
| `README.md` | No count today; keep it that way. |

**Count rule:** every test or capture added changes the numbers in `features.md`, `HANDOFF.md`,
`accessibility.md` and `snapshots/README.md` in the same commit.

---

## 8. Commit sequence

1. **`fix(player): open the quality menu and keep player panels exclusive`** — §2.1–2.2 and §2.6,
   tests. No layout change; snapshots unchanged.
2. **`fix(player): use real icons and the configured seek step in the OSD`** — §2.3–2.5.
   `in-app-osd` and `in-app-narrow*` re-render.
3. **`feat(player): minimal centre transport and a settings sheet`** — S1–S2, including the skip
   pill. All player scenes re-render; `in-app-gear` and `in-app-skip-credits` added.
4. **`docs(player): describe the minimal OSD`** — S3, counts.

Each commit: `scripts/check.sh`, `scripts/snapshots.sh`, and the PNG diffs. Do not bump the Rune
rev.

---

## 9. Risks

| Risk | Mitigation |
|---|---|
| Tracks become hard to find; the gear is the main cost of C. | The gear row shows `Subtitles · 1/2` / `Audio · 1/2`; `C` and the subtitle keys still work; the gear is the first chip on the right. |
| Quality is two clicks instead of one. | Quality is a per-session choice; the row shows the current value so the state is never hidden. |
| The sheet becomes a second bloated bar. | Five sections, at most fourteen rows, no nested menus beyond the existing panels; if it grows, split Tracks into its own panel rather than adding rows. |
| Test and snapshot churn (35 player scenes). | The inventory test is key-based, not pixel-based; inspect the six layout scenes by eye. |
| The 800 px wrap separates related controls. | The wrap is one line and groups volume/mute/gear together; the pill stays on its own line above. `in-app-narrow-small` is a reviewed capture. |

---

## 10. Definition of done

- The OSD without the gear shows exactly nine targets (ten with a skip segment) at 1440, 1024 and
  800.
- Every action in §1.2 is reachable in at most two clicks, and every keyboard shortcut still
  works.
- The skip pill appears only while a segment is offered, carries the segment's own label, and `S`
  works during the same window.
- Every row of the §1.4 ledger resolves: nothing in it is missing from the built player.
- No removed key is referenced by a test; `in-app-stop-bottom` is gone on purpose.
- The quality menu opens from the button on a real player.
- `scripts/check.sh` and `scripts/snapshots.sh` are green; counts are updated.
- `design-player.md` and the code agree at the end of the change.

---

## 11. Open questions

1. **Prev/Next in the gear.** C puts episode stepping behind the gear; `N`/`P` and the up-next
   strip cover it. If series bingeing feels worse, promote the two chips to the centre cluster
   (five targets) rather than the timeline row.
2. **Volume placement.** The mockup omitted volume; this plan puts the slider, mute and gear on
   the right of the timeline row. The alternative is a mute button that opens a vertical slider —
   one more click, one less control.
3. **Hint.** Removed in favour of the sheet footer. A one-time first-run hint is the alternative,
   at the cost of a new persisted flag.
4. **Pre-play sheet.** Out of scope here; the same review found `Decision` and `Why` saying the
   same thing, a `Subtitles 0 track(s)` row, and `Container`/`Size` in the first rank. A separate
   small plan if wanted.
