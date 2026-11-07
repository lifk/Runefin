# TV mode — feasibility and cost

**Status:** exploration, 2026-11-06. No code. This brief answers one question and recommends a
first slice; it is not a plan.

**Question.** How difficult is a **TV mode** for Runefin — a layout built for a 10-foot screen and a
controller, toggled from the app — for the Steam Machine on the living-room TV?

**Answer.** The layout is the cheap half and the controller is the expensive half.

- A **10-foot presentation with a toggle** is **S–M**: about **two weeks** for the token change,
  the per-screen pass and the snapshots. It adds no dependency and cannot break the desktop path.
- A **controller that works today** is **S**: Steam Input maps the pad to keyboard events the app
  already handles. The catch is that the D-pad can only move inside a focus group; moving between
  rails needs Tab on a shoulder button, and nothing is focused until the first Tab.
- A **console-grade D-pad with spatial focus** is **L**: Rune has no gamepad input, no 2D focus
  navigation and no public "focus this" API. That means either an app-owned focus model with
  `gilrs` (10–15 days) or a Rune fork (15–25 days plus a rev bump).

**Recommendation.** Ship the presentation and a documented Steam Input profile first, live with it
on the Steam Machine for a week, then decide whether the D-pad feel justifies the focus work. Do
not start with the fork.

---

## 1. What the mode has to change

| Layer | Today | TV mode |
|---|---|---|
| Size | 176 pt cards, 16 pt gaps, 40 pt gutter, 14 pt labels | 1.3–1.5× cards and type, fewer columns, larger focus ring |
| Input | pointer + keyboard | controller, no pointer |
| Focus | roving within a group, Tab between | must move in two dimensions, visibly, from the first press |
| Hover | card menu, remove action, trickplay, scrub preview | needs a button or a menu path |
| Text | physical keyboard | on-screen keyboard |
| Startup | windowed, cursor visible | fullscreen, cursor hidden, focus already somewhere |
| Toggle | — | Settings row + `--tv` launch flag |

The mode is a **presentation flag plus an input profile**, not a second UI. One `tv: bool` in
`State`/`Config` and a token scale are enough for the first slice.

## 2. What the app already gives us

- **Focus system**: `focus_group`, `focus_ring`, `focus_highlight`, `focus_follows`, `tab_index`,
  `focus_return`, modal focus trapping, and scroll-into-view on keyboard navigation. Rune's
  `focus.rs` is the whole engine.
- **Text scaling** already exists: 0.85–1.35, applied through `theme::set_text_scale`; 130 % is a
  reviewed snapshot (`seerr-130`).
- **Fullscreen** (`F11`, `Tx::set_fullscreen`) and a cursor that hides with the player chrome.
- **A keyboard path for every action**, which is what Steam Input feeds.
- **Sidebar and rails** already use focus groups (Home 4, detail 2, settings 2, search 2,
  library 1, seerr 1, remote 1).

## 3. Where it falls short for a controller

Verified against the pinned Rune checkout (`5221a35`):

1. **No gamepad input at all.** No `gilrs`, no evdev, no joystick code; input is winit keyboard and
   pointer only. A controller must arrive as keyboard events (Steam Input) or through a Rune fork.
2. **No spatial navigation.** Arrows rove **inside one focus group**; Tab moves between groups.
   There is no geometry-based "the card below this one".
3. **No initial focus.** `set_focus` is `pub(super)`; an arrow with nothing focused returns false.
   Tab focuses the first tab-order element. On a controller, the first D-pad press can do nothing
   and the first Tab lands on the sidebar, not the first card.
4. **No focus-by-key API** the app can call, so an app-owned input layer cannot move Rune's focus.
5. **Hover-only paths**: the Continue Watching remove action, trickplay preview and scrub preview
   are pointer features. The card menu (Menu key) is the keyboard equivalent of the remove action;
   trickplay and scrub preview have no controller equivalent.
6. **Text entry** has no on-screen keyboard. Search and Settings need one.
7. **Focus coverage is uneven**: Live TV has no focus group, the player has none (its controls are
   focusable one by one), and the sidebar is a long Tab run.

## 4. The three input paths

| Path | What it needs | Effort | Feel | Risk |
|---|---|---|---|---|
| **A. Steam Input** | A profile: D-pad → arrows, A → Enter, B → Esc, LB/RB → Tab/Shift+Tab, Y → Menu, Steam+X for the keyboard | **S** (2–3 days, mostly docs and focus-order fixes) | Console-ish inside a rail; shoulder buttons between rails; first press needs Tab | Steam-only; no controller outside Steam |
| **B. App focus + `gilrs`** | `gilrs` dependency, a `tv_focus` state per screen, a ring drawn by the app, `Tx::scroll_to_x/y` to reveal, Enter dispatch | **L** (10–15 days) | True 2D D-pad, one ring, works outside Steam | Duplicates focus logic; keyboard focus must keep working; every screen registers its items |
| **C. Rune fork** | `gilrs` in Rune, a `GamepadEvent` or key synthesis, spatial focus in `focus.rs`, a public focus request, tests | **XL** (15–25 days + rev bump) | Best: one focus system for both inputs, every screen inherits it | Fork churn, a Rune rev bump, the largest change |

Path A is what the Steam Machine gives for free. Path B is the smallest way to get a real D-pad
without touching Rune. Path C is the right long-term architecture and the wrong first step.

## 5. Layout cost by screen

| Screen | Work | Size |
|---|---|---|
| Tokens | `CARD_W`, `CARD_GAP`, `GUTTER`, `SIDEBAR_W`, focus ring and type become a `tv` scale | S |
| Home | fewer, larger rails; sidebar collapsed or one focus group; initial focus | M |
| Library | fewer columns, larger cards, list view stays | S |
| Detail | action bar, season chips, episode rows, cast rail | M |
| Search | result rails and the hints list | S |
| Settings | row height, section spacing, the toggle itself | S |
| Live TV | focus groups for channels and the guide; the guide needs a focus model | M–L |
| Seerr | rails and the request sheet | S |
| Player | larger chips, the skip pill, the sheet rows; no hover trickplay | S |
| On-screen keyboard | optional, only if Steam's is not enough | M |

The token change is a handful of `const` → `fn` conversions; the per-screen pass is mostly
spacing and column counts, not new components.

## 6. Steam Machine specifics

- **Add Runefin as a non-Steam game.** That is what gives Steam Input, the Big Picture launch and
  the on-screen keyboard (Steam+X) inside gamescope. A controller works in the desktop session too,
  but with the desktop layout, not per-game bindings.
- **Fullscreen on launch** belongs to TV mode (`--tv` or the setting), not to the window manager.
- **HDR is not part of this.** The player tone-maps P010 to SDR today; HDR output is a separate
  piece of work and a compositor negotiation, not a TV layout feature.
- **Deployment is the real prerequisite.** The app needs GStreamer with base/good/libav and VA-API,
  and mpv as the fallback. SteamOS has a read-only rootfs; check what the image ships before
  promising a one-line install. A Flatpak or AppImage is the safe path; dev mode is the quick one.
- **Resolution and scaling**: the mode must hold at 1920×1080 and 3840×2160 with 130 % text, and at
  the 800×600 minimum the app already supports.

## 7. Suggested phases

| Phase | Scope | Effort | Ships |
|---|---|---|---|
| **P0** | `tv` flag + config + Settings row + `--tv`; tokens (type, cards, gutter, ring); fullscreen and hidden cursor on launch | 4–6 days | A TV-sized app, still keyboard/Steam Input |
| **P1** | Per-screen pass: Home, Library, Detail, Search, Settings, Seerr, Player; snapshots at 1080p and 4K | 4–6 days | A readable 10-foot UI |
| **P2** | Focus order: rail-to-rail Tab, a first-focus workaround, Live TV groups; Steam Input profile + docs | 2–3 days | A usable controller on the Steam Machine |
| **P3** | Native gamepad + app-owned focus (path B) **or** Rune fork (path C) | 10–15 / 15–25 days | A true D-pad |
| **P4** | Text entry, if Steam's keyboard is not enough | 3–5 days | Search and Settings from the couch |

P0–P2 are about **two weeks** and are low risk. P3 is the fork-sized decision.

## 8. Risks

| Risk | Mitigation |
|---|---|
| Rune's focus model is the product's ceiling on a controller. | Prove it with Steam Input first; only then choose B or C. |
| Two focus systems (path B) drift. | Keep the keyboard path on Rune's focus; the TV ring is additive and only drawn when `tv` is on. |
| Hover-only actions disappear on a TV. | Card menu on a button, chapter list instead of trickplay, arrow seek instead of scrub. |
| Text entry blocks search. | Steam+X for v1; an in-app grid keyboard only if it fails on the couch. |
| SteamOS packaging. | Decide Flatpak vs AppImage before P0; the UI work is independent. |
| 130 % text plus 10-foot sizing clips. | The existing `-130` snapshots and a 4K capture in the matrix. |

## 9. Open questions

1. Is **Steam Input acceptable for v1**, or must the app read the controller itself?
2. Should TV mode **start fullscreen** and hide the cursor, or keep the window state?
3. Are **horizontal rails** required, or is a vertical feed acceptable for some rows? A single
   column is the one layout Rune's focus model handles perfectly today.
4. Which pad is on the Steam Machine — Steam Controller, Xbox or DualSense? The mapping differs.
5. Does the app already run on the machine (GStreamer, mpv), or is packaging part of the work?
