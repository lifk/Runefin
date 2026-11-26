# opencode-rune review — 2026-11-26

`anomalyco/opencode-rune` is OpenCode Desktop rewritten in Rust on Rune. It is MIT, ~1,400 Rust
files across `apps/`, `features/` and `crates/`, and it is a real shipping app (macOS, Windows,
Linux, and a WebGPU browser build). Its Rune pin (`5342f14d`) is newer than ours
(`c2e2eb6c7`), so any API named below is a pattern to check against our pin before using it.

This is a read-only review of the fetched tree (`--depth 1`, so file contents are verified but
history is not). It is not a proposal to copy code: the patterns are the value, and the licence
allows reimplementation with attribution if a snippet is ever taken.

---

## 1. Architecture: progressive disclosure and lanes

The workspace is deliberately layered so a feature never edits the centre of the app:

- `plugins/*` — Luau plugins for dialogs, panels, settings pages, composer pills and commands;
  hot-reloaded with no Rust compile.
- `features/*` — one crate per feature, zero cross-feature dependencies, entrypoints on
  non-generic `&dyn Host` boundaries.
- `crates/*` — reusable libraries: `opencode-kernel` (slots, overlays, scheduler, commands),
  `opencode-ui` (palette, dialog, scroller, popup, toast, keymap), `opencode-runtime`
  (headless state machines), `opencode-storage`, `opencode-search`, `opencode-kit` (server
  client), `session-view` (embeddable timeline).
- `apps/*` — desktop, web, installer, and a Windows thin-exe/library split.

The rule is “pick the first lane that fits; do not add fields to `struct App` or edit central
dispatch chains”. Runefin does not need Luau or a second app, but the shape is useful: pure
logic and headless state in modules, screens as thin views, and no screen reaching into another
screen's state.

**For Runefin:** the app is already three crates (`jellyfin-api`, `seerr-api`, `runefin`). The
next split is inside `runefin`: `state/` modules per domain (home, library, detail, player,
downloads, settings) with screens reading through narrow host traits, not one 5,852-line
`state.rs` and one 3,304-line `screens.rs`.

---

## 2. Explicit state machines

The strongest convention in the repo: “no implicit state machines”. Any behaviour with modes or
phases is one `enum State`, one `step(&mut State, Event) -> Effects`, and effects returned for
the caller to run. Illegal states cannot be represented.

Examples read:

- `crates/opencode-kernel/src/overlay/stack.rs` — `OverlayState` (`Empty`/`Active`),
  `OverlayEvent` (`Push`/`Close`/`Escape`/`ClearKind`), `OverlayEffect` (dismiss + restore focus).
- `crates/opencode-kit/src/client/worker.rs` — `StreamState` (`Initial`, `Reconnecting`,
  `Open`, `Waiting`, `Closed`) with backoff carried in the variant and handshake effects.
- `features/tips/src/queue.rs`, `features/narrow/src/machine.rs`,
  `crates/opencode-runtime/src/machines/*` — the same shape for tips, layout and send/queue.

Runefin already has two: `downloads::State` and the sign-in `Step` (`Idle`, `SigningIn`,
`Ready`, `Failed`). The rest of the app still encodes modes in flags: `loading`, `open`,
`requested`, `downloading`, `osd_visible`, `in_app.is_some()`, `player_panel_open()`, and the
long Escape ladder in `main.rs` (~lines 557–748) that tests each flag in turn.

**For Runefin:** convert one flow at a time, starting with server connection/reconnect and the
player/OSD. Keep the existing `Latest` request plumbing; the machine only decides *which*
request is allowed and what the UI shows. The payoff is fewer “two booleans disagree” bugs and
tests that drive `step` directly without a window.

---

## 3. Four-phase tick and a declared schedule

`apps/desktop/src/tick.rs` splits `poll` into four phases:

- **Ingest** — storage hydration, server health, local OS events, notifications/deep links,
  page loads, stream drain.
- **Derive** — timeline projections, search, composer suggestions.
- **React** — panels, status, shell layout.
- **Persist** — flush dirty state.

Within a phase, systems name a `Stage` and may declare `after("other")`. `opencode-kernel`'s
`LifecycleScheduler` resolves the DAG once at startup and panics on duplicate names, missing
targets, backward edges or cycles; registration order never changes execution order.

Runefin's `State::poll` (in `state.rs`, called from `main.rs`) is a single long function that
interleaves all four concerns. It works, but the order is implicit and every new feature adds
another block to the same function.

**For Runefin:** do not port the DAG machinery. Adopt the *phase discipline*: four private
functions (`poll_ingest`, `poll_derive`, `poll_react`, `poll_persist`) called in order from
`State::poll`, each returning `changed`, with a comment naming the stages. That makes the
existing order explicit and gives new features an obvious home without a new scheduler crate.

---

## 4. State clusters and tracked cells

`apps/desktop/src/shell/app_state.rs` groups the app's fields into plain structs
(`FixtureRun`, `DiagState`, `WorkspacePanes`, `ShellGestures`, `TimelineFx`,
`AppearanceState`, ...), and every field is a `rune::Tracked<T>`. Feature-owned state lives in
`opencode_kernel::StateSlots` behind static `Slot<T>` handles; a slot is a dense `Vec` index,
not a `TypeId` hash, and reads inside `rune::region`/`tracked_reads` subscribe while writes
invalidate only the regions that read them.

Runefin's `State` has hundreds of plain fields. Any change marks the whole view dirty, so the
player's ticking clock or a download percentage rebuilds screens that did not change.

**For Runefin:** two steps, in order.

1. **Clusters, no behaviour change.** Move fields into `HomeState`, `LibraryState`,
   `DetailState`, `PlayerState`, `DownloadsState`, `SettingsState`, `ShellState`. This alone
   makes `state.rs` navigable and gives screens a narrower borrow.
2. **Tracked cells on hot paths.** Wrap the fields that change per frame or per second
   (`player.position_ms`, OSD timers, download progress, search text, toast timers) in
   `rune::Tracked`, and put the screens that read them in `rune::region`. `Tracked`, `region`
   and `tracked_reads` are already in our pin. Do not wrap everything at once; measure the
   player and Home first.

---

## 5. Overlays, key bands and Escape order

`opencode-kernel` has an `OverlayStack` state machine plus a declarative dispatcher:

- visual layers, bottom to top: popovers, switcher, media lightbox, action dialogs, floating
  chrome, dialogs, elevated dialogs, chrome popups, plugin top, toasts;
- key bands, first to last: popup, modal, mid, low, with higher visual layers handling keys
  first inside a band unless an explicit `after_key` says otherwise;
- Escape bands: chrome, session action, page popover, fallback, after the stack's topmost
  entry.

Every overlay registers an id, layer, key band, dismiss callback and focus-return target. The
dispatcher resolves order once and validates duplicates/cycles at startup.

Runefin's Escape handling is an if-ladder: rebind capture, sheet, card menu, collection picker,
playlist picker, subtitle search, player panel, cast picker, playlist create, gear, stats,
shortcuts, shelf, then route. Adding an overlay means finding the right rung, and focus return
is per-dialog code.

**For Runefin:** a small `overlay` module with `Overlay { id, kind, dismiss_on_escape,
return_focus }`, a `Vec` stack, and `step(Escape) -> Dismiss(id)`; each screen pushes/pops when
it opens/closes a dialog, menu, OSD or picker. This replaces the ladder and makes “Escape closes
the topmost thing” a single rule. It also gives the TV Back key one place to consult.

---

## 6. Dialogs: size presets instead of ad-hoc widths

`crates/opencode-ui/src/dialog.rs` defines four presets and every dialog takes one; a guardrail
fails a dialog that sets its own width or height:

- `Normal` 480×368 (confirmations, sign-in, small forms),
- `Tall` 480×600 (a short form that reveals a list),
- `Large` 680×600 (pickers, lists),
- `XLarge` 880×680 (long documents, settings-like panels).

`widgets::sized`/`sized_modal` clamp to the window, centre, and choose fixed/content/anchored
height. Runefin currently uses 400, 420, 430, 460, 520 and 640 in different dialogs.

**For Runefin:** add a `DialogSize` table to `theme.rs` (sizes may differ for a 10-foot layout;
720-point design height is the constraint) and a `sized_modal(key, size, fit, window, card)`
helper in `screens.rs`. Then a test or guardrail can assert every dialog uses it.

---

## 7. Small UI patterns worth taking

- **Toast stack** (`opencode-ui/src/toast`): multiple toasts, a 160 ms 2.5% pulse when one is
  re-shown, per-toast timers and actions. Runefin has one `toast: Option<String>` plus an undo
  action; a stack would let “Download complete”, “Server unreachable” and an undo coexist.
- **Scroller defaults** (`opencode-ui/src/scroller.rs`): one scroll container with a named bar
  (`Native` 8 pt gutter, `ScrollView` overlay, `MenuScroll`, `Tree`, `Hidden`), device-pixel
  thumb rounding, and a `gutter(key)` query for content laid out at a set width. Runefin has
  scroll bars, but each screen chooses its own; a default plus named exceptions is the same
  discipline as dialogs.
- **Narrow layout machine** (`features/narrow`): a 768 px breakpoint, 640 px `sm`, a
  Session/Changes/More switcher, drawers and sheets, with the width measured in the view and
  the machine stepped from that. Runefin has three modes and a resizable window; this is the
  pattern for a desktop layout that folds below a breakpoint instead of squeezing.
- **Paced tips** (`features/tips`): at most one tip a day, the first an hour after first seen,
  persisted in the app's storage, shown as a toast with an action. A good fit for TV remote
  hints and the in-app keyboard.
- **Settings shell** (`features/settings`): a 328 pt rail beside a 720 pt column, a search index
  over pages and rows, and preferences stored under a versioned shape. Runefin already has a
  settings search; the two-pane shell and the search index are the missing pieces.
- **Find index** (`features/find`): matches indexed across the whole transcript, cached per
  message, only changed parts re-scanned, row hashes so `memo` rebuilds only rows whose
  highlights changed. The analogue for Runefin is a local index over Home, Downloads and the
  current library, not the server-side search.

---

## 8. Network, storage and restore

**Server health as a value.** `opencode-kit/src/servers/probe.rs` returns
`ProbeResult::{Healthy, Unauthorized, Unreachable}` from `GET /api/info`, with a 30 s total
budget, two retries at 100/200 ms, and a `loopback_origin` normaliser. Runefin's `net.rs`
already has a `Latest` request pattern, but connection state is spread across `Step`, `Failure`
and several flags. A `ServerHealth` machine with those three outcomes would make reconnect and
offline mode explicit.

**Three worker threads, one event stream.** `opencode-kit/src/client` uses a fetch worker, a
write worker and an SSE worker, all waking the window; the stream machine backs off 250 ms
doubling to 3 s, resets on `server.connected`, and treats 45 s of silence as idle. Runefin's
`net.rs` uses a pool and `Latest`; the useful part is the explicit stream state machine and the
“wake only when data lands” rule, which matches our no-blocking rule.

**Write-behind storage and a draft journal.** `opencode-storage` hydrates the first frame in one
read, then writes behind: state and window frames coalesce for 250 ms, documents for 500 ms,
and Tier 0 rows (outbox, plugin trust) commit with `synchronous = FULL` before their callback.
A separate journal records composer drafts through a kill even while the database is busy.
Runefin's `checkpoint.rs` writes a whole JSON map on a thread, and downloads use manifests.

**For Runefin:** the journal idea fits two things we already have: resume positions and queued
playback reports. A small append-only journal (or a coalescing writer) that survives a crash is
better than rewriting `positions.json` whole, and the same mechanism can hold “report this stop
to the server once it is reachable”. The `Tier 0` distinction is the key: some writes must be
durable before the UI says they happened.

**Restore.** `features/restore` keeps the open page, every draft, closed tabs, panes, sidebar
layout, boot previews and offline transcript copies, one row per thing, written behind.
Runefin restores the last library/detail and has a Home cache; a `restore` module could add the
last route, scroll position, search draft and player resume in one place.

---

## 9. Engineering guardrails and build speed

The repo enforces its architecture with scripts, not review comments:

- `scripts/arch-check.sh`: resolved downward crate layers and zero cycles; zero cross-feature
  deps; core-crate isolation; `#![forbid(unsafe_code)]` on core crates; at most 10 root `.rs`
  files in `apps/desktop/src/`; a `struct App` field-count ratchet; a single Rune rev pin;
  no wildcard imports.
- `scripts/file-size.sh`: warn above 800 lines, error above 1,500 unless the file is in a
  ratchet list; ceilings only go down.
- `scripts/folder-shape.sh`: prefix-family subfolders and folder breadth (>40 warn, >60 error).
- `scripts/dialog-size.sh`: every dialog takes a `DialogSize`.
- `scripts/tables-check.sh`: story/command/i18n tables match their generated lists.
- `scripts/cursor-allow.mjs`: every cursor, link and selectable text is listed with the official
  source line that justifies it.

**For Runefin:** a `scripts/guardrails.sh` with three cheap rules would already protect the
design contract: (1) file-size ratchet (start with the current large files as exceptions),
(2) every dialog uses `DialogSize`, (3) the `rune` rev in `crates/runefin/Cargo.toml` and the
`accesskit_atspi_common` patch rev match. The cursor allow-list is overkill for us, but the
“every exception names its reason” shape is good.

**Build profiles.** `docs/fast-builds.md` defines:

| Profile | What it changes | Use |
|---|---|---|
| `dev-fast` | inherits `dev`, `debug = 0`, `codegen-units = 64` | default local loop, fast relink |
| `release-quick` | inherits `release`, `lto = false`, `codegen-units = 16` | fast optimized iteration and Linux packaging |
| `release-thin` | `lto = "thin"`, 16 units | near-release speed without fat LTO |
| `perf` | `release-quick` + `debug = 1` | profiler symbols |
| `release` | fat LTO, 1 unit | shipped binary |

They also build hot dependencies (`rune`, `parley_engine`, `image`, `flate2`, `crc32fast`,
`memchr`, their `opencode-search`) at `opt-level = 3` even in `dev`, and keep everything else at
`opt-level = 1` with `debug = false`. Runefin currently only sets `debug = 0` and dependency
debug off. Adding `dev-fast`, `release-quick`, `release-thin` and the hot-crate `opt-level`
lines is a small change with a large effect on the local loop, especially under our 6-core cap.

**Non-generic boundaries.** Feature entrypoints are `&mut dyn Host`, with generics only at
`rune::El`/`Tx` callbacks, so a feature's codegen stays in its own `.rlib` instead of being
re-monomorphised in the app crate. Runefin's screens are already concrete functions; if they
move behind host traits, keep the trait object at the screen boundary.

---

## 10. What not to take

- **No automated tests.** The repo has a deliberate “application code is truth” policy and a
  guardrail that forbids `#[test]`. Runefin's `Runtime` tests and committed snapshots are the
  regression net that lets a Rune pin bump land safely; do not weaken them.
- **Luau plugins.** There is no plugin architecture in Runefin, and the Decky plugin is optional
  by design. The hot-reload lane solves a problem we do not have.
- **CEF browser, Office preview, terminal, updater, i18n catalogs.** All are real features for a
  coding agent, none is in the three-mode product.
- **The full scheduler/overlay DAG machinery.** The phase names, the overlay stack and the
  dialog presets are the transferable parts; a single-app crate does not need a validated graph
  engine.
- **Their Rune pin.** It is newer than ours and may use APIs we do not have. Check the API in
  our fork before porting a call.

---

## 11. Mapped to Runefin

| Runefin area | Pattern | Where it lands | Effort |
|---|---|---|---|
| Dialogs and pickers | `DialogSize` + `sized_modal` | `theme.rs`, `screens.rs` | S |
| Escape / Back | overlay stack with ids and focus return | new `overlay.rs`, `main.rs` key ladder | M |
| Toasts | toast stack with timers/actions | `state.rs` toast fields, `screens.rs` | S |
| `State::poll` | four named phases | `state.rs` | M |
| `State` size | domain clusters, then `Tracked` on hot fields | `state.rs` split | M–L |
| Player OSD / downloads | `Tracked` + `rune::region` | `screens/in_app.rs`, `downloads.rs` | M |
| Connection | `ServerHealth` + stream machine | `net.rs`, `state.rs` | M |
| Resume / reports | write-behind + crash-safe journal | `checkpoint.rs`, new `journal.rs` | M |
| Settings | two-pane shell + search index | `screens/settings.rs` | M |
| Local search | incremental index + row hashes | `search.rs`, `home.rs` | M |
| Narrow window | breakpoint machine | shell layout | M |
| Onboarding | paced tips queue | new `tips.rs` | S |
| Build loop | `dev-fast`/`release-quick`/`release-thin` + hot opt-level | `Cargo.toml` | S |
| Architecture | file-size/dialog/pin guardrails | `scripts/guardrails.sh` | S |
| Player/settings tests | host traits + headless scenes | `screens/*` | L |

**Recommended order.** (1) build profiles and guardrails, because they make everything else
cheaper; (2) dialog presets and the overlay stack, because they remove the most day-to-day
friction; (3) split `State::poll` into phases and `State` into clusters; (4) `Tracked` on the
player and downloads; (5) the storage journal and server health machine; (6) the larger screen
host-trait work and the narrow layout.

---

## 12. Verification

Read-only review of `anomalyco/opencode-rune` at `987d475` (shallow clone, MIT). No Runefin code
was changed. API availability was checked against our pin for `rune::Tracked`,
`rune::region` and `rune::tracked_reads`; the rest of their Rune surface is newer than our pin
and must be checked before use.

---

## 13. Landed 2026-11-26

The first pass took the cheap, behaviour-preserving items:

- **Build profiles.** `dev-fast`, `release-quick`, `release-thin` and `perf` in the workspace
  `Cargo.toml`; `release` still ships from CI.
- **Guardrails.** `scripts/guardrails.sh` runs first in `scripts/check.sh`: one Rune rev for the
  app and the AccessKit patch, no `use crate::*`, and a file-size ratchet in
  `scripts/file-size-exceptions.txt` (warn at 800, cap at 1,500, listed ceilings only go down).
- **One Back/Escape ladder.** `crates/runefin/src/overlay.rs` owns the order; `State::top_overlay`
  and `close_overlay` in `state/overlay_state.rs` dispatch it, and `on_key` has one check instead
  of the per-dialog `if … && key.key == Key::Escape` chain. Preplay and the detail page's
  new-playlist sheet now close on Back like every other modal.
- **Dialog width tokens.** `theme::dialog` names the widths the menus, pickers and sheets use, so
  the next dialog starts from a token instead of a bare number.

Second slice, same day:

- **Four-phase `poll`.** The tick now lives in `crates/runefin/src/state/poll.rs` as Ingest,
  Derive, React and Persist, in the exact order the old single function ran. `state.rs` dropped
  from 5,891 to 5,282 lines and its ratchet ceiling came down with it.
- **First `Tracked` clusters.** `Toast` (`message`, `undo`, `until`) replaces three loose fields,
  and `DownloadsState` wraps `entries`, `free`, `notice` and `error` in `rune::Tracked`. Reads
  still look like the value; writes go through the cell. Regions come next, once every read in a
  screen is tracked or an explicit `region_with` input.

Still deferred, in the review's order: the toast stack, the write-behind storage journal, the
server-health machine, the narrow-layout machine, paced tips, and screen host traits. They are
larger changes and should land one at a time with their own snapshots and tests.
