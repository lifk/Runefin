# Rune ecosystem review — 2026-11-09

Method: `git fetch upstream` in the fork, comparison of `runefin/player` with `upstream/dev`, the
unreleased changelog, and a trial rebase. The public listing was read from `neiii/awesome-rune`
(`projects.json`, schema, PR template).

## Applied 2026-11-10

Branch `rune-bump` (this change), fork branch `runefin/bump-2026-11` at `489750e5`, based on
`upstream/dev` `2bc3179d` (176 commits since the previous base).

What the rebase needed:

- `render.rs`: upstream's shader machine added a `classic` argument to `Renderer::create`; the
  DMA-BUF adapter became one more argument, so `new_with_adapter` keeps working.
- `window/create.rs`: the window renderer moved out of `window.rs`; the adapter is passed there.
- `window/create.rs` and `lifecycle.rs`: `set_fullscreen` re-applied on the rewritten window
  creation/resume paths.
- `window.rs` / `window/events.rs`: `set_design_height` and its `ScaleFactorChanged` guard
  re-applied around the new scale handling.
- `runtime/input/focus.rs`: upstream's `web_focus` clause and our `return_activates` clause now
  sit side by side in `takes_return`.

Adopted automatically by the bump: Blink-compatible line breaking, TextEdit IME composition and
undo grouping, the tooltip state machine, the scroll mode machine and the virtual-list motion
gate.

Verified:

- Rune's own lib suite fails the same 13 tests on `upstream/dev` and on the rebased branch
  (macOS precompiled metallib, two macOS smoothing tests, GPU/lavapipe-sensitive render and
  video tests), so the rebase adds no failures.
- App: `scripts/check.sh` green, 315 tests.
- Player: the DMA-BUF probe renders 24.0 frames/s with zero stalls and a 41 ms longest gap on
  the rebased renderer.
- Snapshots: every scene was regenerated during the bump; the diff was text rendering
  (Windfoil) plus the fixture titles. The branch then cut the committed set to eight core views
  (`scripts/snapshots.sh --all` renders the rest locally); see the snapshot README.

Deferred, with the reason:

- **`El::focus_fill`**: `focus_highlight` still gives both the big-screen fill and Return
  activation; switching would split one decision across two attributes for no visible gain.
- **RTL layout**: the UI copy is English, so mirroring the sidebar and rails would be a product
  decision, not an upgrade. RTL *text* is shaped by Rune's bidi pass either way; revisit when
  the interface itself is localized.
- **Sticky headers (`El::stuck_reveal`)**: optional design change, not a defect; the library
  toolbar is already reachable by scrolling to the top.

The fixture library also gained its neutral titles here (`Aurora 000`, `Beacon 001`, …): the
earlier public-domain pass had missed the ten-word list behind the paged library scenes.

## Where the fork stands

- Merge base: `ddf14140` (2026-11-06). Since then upstream has **168 commits**; the fork adds
  **12** of its own and none of them are upstream:
  `native_surface` DMA-BUF import + HDR, the renderer's DMA-BUF adapter, `set_design_height`,
  `Tx::set_fullscreen`, key injection for controller navigation, and `set_return_activates`.
- Pin: `f3eec1a5`. The app builds and the suites are green against it.
- Trial rebase (`git rebase --onto upstream/dev ddf14140 runefin/player`) stops at our second
  DMA-BUF slice with a conflict in `crates/rune/src/render.rs`; `native_surface/mod.rs` and
  `pipeline.rs` auto-merge. The window and focus rewrites were not reached before the trial was
  aborted.

Upstream's 168 commits are mostly platform work: ~20 Windows, ~15 web/wasm, 4 Apple. The rest is
core input, text, scrolling, motion and the kit.

## What is worth incorporating

| Upstream change | Why it matters to Runefin | Cost |
|---|---|---|
| **Focus and input as explicit state machines** (#408) plus `El::focus_fill` (#398) | Our big-screen focus is a patch on the old focus path (`focus_highlight`, `tv_focus`, `set_return_activates`). A clean focus machine is the right foundation for a remote, and `focus_fill` may replace the hand-rolled fill | Rewrite `takes_return` opt-in on the new machine; re-test every big-screen flow |
| **Scroll as one explicit mode** (#401) and the virtual-list motion gate (#414) | The library is a virtualized grid; a row that keeps its place while a scroll cuts through it is exactly the case #414 fixes. Our scroll restoration and `on_far_from_bottom` paging live on this path | Low risk, but paging and restored offsets need re-testing |
| **Text: Blink-compatible line breaking** (#416) | Episode titles and overviews wrap as Chromium does; fewer ragged lines in rails and cards | Reflows text: every snapshot with wrapped copy changes |
| **TextEdit: IME composition, undo grouping** (#385) | Search, server address and the Seerr key are text fields; IME is the gap for non-Latin input | Low risk |
| **Tooltips state machine** (#383) | The desktop build already sets `help(..)`; upstream's timing and suppression are better than what we get now | Low risk |
| **Direction / RTL layout** (#381, #339) | Jellyfin metadata can be Arabic or Hebrew; today the UI stays LTR | Medium: needs an RTL pass and tests |
| **Sticky headers** (`El::stuck_reveal`, #399) | The library toolbar could stay visible while the grid scrolls | Optional design decision, not a fix |

## What to leave out

- The Windows titlebar, toasts, menus and D3D shared-texture surface: no Windows target.
- Web/wasm work: Runefin is native only.
- `rune-kit`, `rune-fx`, `rune-light`, `rune-knobs`: the app has its own token table and
  components; adopting the kit would be a redesign, not an upgrade.
- `rune-mac` full screen and notifications: we already have `Tx::set_fullscreen` and a Linux
  notification path.

## Recommendation

Treat the Rune bump as its own milestone, not as part of a feature change.

1. Rebase on a branch, resolve the `render.rs` conflict in the DMA-BUF import, then adapt the
   three window patches (`design_height`, fullscreen, DMA-BUF adapter) to the rewritten
   `window/` modules.
2. Move `set_return_activates` onto the new focus state machine, or drop it if `focus_fill` and
   the new press rules cover the remote.
3. Expect a snapshot pass: the text changes reflow copy, so a large diff is intended and must be
   reviewed frame by frame rather than accepted wholesale.
4. Do it after the current round of TV feedback, so a regression is not confused with the bump.

Cherry-picking individual fixes is not worth it: each pick becomes another patch the fork has to
carry, and the fork is already 12 patches deep.

## awesome-rune

- Still **48 projects**, same schema and PR checklist; the Media subcategory has 10 entries and
  none of them is a Jellyfin client (`stremio-rune` is a Stremio client).
- No new fields, categories or stability rules since the last review, and no Runefin entry yet.
- The prepared entry is still valid: `Apps` → `Media`, canonical GitHub URL, optional `image`,
  one-line description. `cargo run --quiet -- validate` and `cargo run --quiet -- sort --check`
  are the only gates.

Nothing in the listing suggests waiting for more maturity; the entry is a JSON object that can be
updated after Flathub or a signed repository exists.
