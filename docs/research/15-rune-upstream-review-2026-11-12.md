# Rune upstream and community review — 2026-11-12

Method: `git fetch` on the fork's remotes (`origin` = lifk/rune, `upstream` = anomalyco/rune), plus
the home mirror that then stood beside them, `git log`/`git diff` of `2bc3179d..upstream/dev` (418
commits), the 548 unreleased changelog entries, the 40+ contributor branches it carried, and the
listing in `neiii/awesome-rune`. Follows `13-rune-ecosystem-review-2026-11.md`.

That mirror is retired (2026-11-12): both the app and the fork live on GitHub only, so a later
review reads contributor branches from `upstream` itself.

## Applied: the pin bump

Fork branch `clean/2026-11` at **`3fd8f44f`**, published as `origin/runefin/player` (force) and
`origin/clean/2026-11`. Merge base `2bc3179d`; the branch is 15 of our commits under one merge of
`upstream/dev`.

The previous attempt to merge upstream sat on a mass-format commit (`95e21b20`, 1266 files) and
stopped at ~70 conflicting files, most of them reformatting. This branch was rebuilt instead:
the 15 player commits were cherry-picked onto the merge base (`95e21b20`'s only real payload,
`focus_scale`, is unused by the app and was dropped), then `upstream/dev` was merged. Twelve
conflicts, all semantic:

- `app.rs`: upstream's `Requests.activation` (`Focus`/`Minimize`) replaced `focus`/`minimize`;
  `fullscreen` and `keys` stay beside it, and `asks_window()` counts the fullscreen request.
- `render.rs` / `render/pipelines.rs` / `render/upload.rs`: upstream compiles the native-surface
  pipeline on a thread (`Building`), so the DMA-BUF adapter is carried through `Building` into
  `finish()`, and the async build/prepare path creates the Linux `SurfaceRenderer` with the
  window's adapter.
- `native_surface/pipeline.rs`: the Linux fields, `new_with_adapter`, P010/Bt2020/Pq/Hlg colour
  mapping and the `Source::Dmabuf` arm move into upstream's `with_pipeline`/`build`/`finish`.
- `window/create.rs`: upstream's `OpenAs` covers the initial fullscreen; the runtime request is
  applied on reveal (`OrderIn`) and in the shell loop.
- `window/events.rs`: the design-height `effective_scale()` merged with upstream's CPU-renderer
  path, which no longer requires a GPU to be present.
- `window/lifecycle.rs`, `runtime.rs`, `tooling/virtual_desktop/window.rs`: the runtime's
  `Minimize` state machine replaces `minimize_request`; `fullscreen_request`/`fullscreen` stay.
- `window/shell_requests.rs`: `fullscreen()` read the removed `Win::visible` field; it now asks
  `life.hidden()`'s replacement, `life.shown()`.
- `lib.rs`: `windows` module re-export plus our `set_return_activates`, both kept.

The fork's own patch is now **25 files, +1019/−24** outside the vendored `wgpu-external-frame` and
`Cargo.lock`. No `cargo fmt --all` was run in the fork repo: that is what made the last sync
unmergeable.

Verified:

- Rune's lib suite: **1910 pass, 14 fail, 11 ignored**. The same 14 fail on pristine
  `upstream/dev` in this container (edit/pills/playground/GPU and macOS smoothing), so the merge
  adds none.
- App: `scripts/check.sh` green — 47 `jellyfin-api`, 25 `seerr-api`, **404** `runefin` (22
  ignored), release build.
- Snapshots: all ten committed views re-rendered. The diffs are glyph-shaped and no block moved:
  worst mean luminance delta 0.63/255 (`library-big`), 13 % of pixels differing by any channel
  value there because the grid is text-dense. Upstream's text stack (fontconfig/CoreText
  fallbacks, COLR v1 emoji, new shaping, unrounded line heights) lands glyphs a fraction of a
  pixel apart; nothing reflowed.
- One app test needed a change: `the_end_card_focuses_play_now` injected a session position the
  demo player disagreed with, and the 100 ms clock gate let `poll` pull it back to 42:00 on a
  first frame slower than the gate. It now sets `Player::preview_at` with the position.

## Free with the bump

These arrive without app work, and each one is visible in the app:

- **Text**: fallback fonts through fontconfig on Linux (no folder walk), Han glyph forms chosen by
  the text's language, mixed-direction (Arabic/Hebrew) text in every static text and field, COLR v1
  colour emoji, Chromium-exact line heights. Jellyfin metadata is exactly this kind of mixed,
  multilingual text.
- **Images**: `ImageCache` decodes on up to four threads, so a rail of posters loads in parallel.
- **Startup**: glyph masks are kept across launches; a frame that places new glyphs no longer
  submits twice more.
- **Player**: the first video's pipeline compiles on its own thread (our DMA-BUF port rides it), so
  the opening frame no longer freezes input while the shader builds.
- **Lists**: virtual-row fixes for a fold gliding open over mounted rows, a grown viewport, a
  scroll cutting through the list, and rows out of sight entering without animation.
- **Paint**: shadows keep their middle clear under a translucent fill, hairlines and rectangle
  icons draw as Chromium's, a closed layer's target is reused for the next layer of its size.
- **Focus/input**: Tab from outside the Tab order goes on from its place; focus falls back to a
  neighbour when the focused element leaves; F6 cycles landmarks.

## Worth acting on

Ordered by value per hour for this app. Nothing here needs a fork patch.

| # | Change | What it buys | Effort |
|---|---|---|---|
| 1 | ~~`rune::prewarm_gpu()` in `main`~~ | **Already done by Rune**: `run_configured` calls `prewarm_gpu()` before it builds the event loop (`window/event_loop.rs:65`), so GPU init already overlaps winit setup and window creation. The app's own call would only cover the few milliseconds of config reading and `App::new` before `run`. No action | — |
| 2 | Rune's `accessibility` feature | **Applied**: `accesskit_unix` publishes the tree to AT-SPI, the four unnamed controls are named, the timeline and volume read as sliders with ranges, and the end card announces once | Done |
| 3 | Audit with upstream's debug tools: `RUNE_PERF_WARN=1`, `RUNE_REGION_MISS=1`, `RUNE_FRAME_LOG=1` | Names a `poll` that reports change with nothing changing, a window drawing 120 frames in 2 s idle, a memo/region that rebuilds, and the stages of a slow launch. A TV that burns frames while resting is the worst case for a media box | Hours, no API; needs a live window |
| 4 | Give every control an accessible name, then pin it with a test | **Applied**: `every_screen_names_every_control` walks eleven screens and fails on any unnamed interactive node | Done |
| 5 | `El::enter_delay` + `rune::Stagger` for rails and sheets that mount many rows at once | Rows enter in sequence instead of together; it holds the whole entrance (offset, fade, blur) on one progress, and respects Reduce Motion | Small; snapshots move |
| 6 | `El::hover_alone` on the play/press affordance inside a poster; `El::animated_changes` around the OSD | A pointer on the play badge no longer lights the whole card; the OSD keeps its own reveal while the page under it animates | Small |
| 7 | `Tracked` fields and `region_with` for the home and library screens | Upstream's regions rebuild only when a value they read changed. The app has one large `State`; this is the incremental way to stop rebuilding whole screens for one unrelated field | Large; per-screen, measurable with `RUNE_REGION_MISS` |
| 8 | `Family::Named` CSS-list for the subtitle Serif slot; `set_font_families` if a UI font setting ever ships; `font_epoch` in any measurement cache | **Applied** for Serif: `"DejaVu Serif, Liberation Serif, Noto Serif, FreeSerif, serif"` falls through on a minimal install. `set_font_families` stays unused until the app offers a UI font | Done / small |
| 9 | `El::stuck_reveal` for the library toolbar | The sort/filter row stays visible while the grid scrolls (deferred in review 13 as an optional design change; still optional) | Medium; a design decision |

## Applied in the second pass (2026-11-12)

- **Pin**: `3fd8f44f`, unchanged by this pass: everything here is app-side.
- **Accessibility**: feature on (`accesskit` crates), four controls named (`sidebar-toggle`, the
  player's video area, its scrim, the gear sheet), the timeline and volume slider report as
  sliders, the end card announces once, and `every_screen_names_every_control` pins all of it.
  `docs/design.md` gained the rule-5 line; `docs/accessibility.md` a "Screen readers" section.
- **Duplicate matched ids**: gone. A card's shared id is its own key
  (`poster-{rail}-{item-key}`), which is unique even when two rails show one series or a search
  rail holds two episodes of it, and `open_detail_from_card` carries the pressed card's id into
  the detail poster, so the flight starts where the press was.
  `a_card_hands_the_detail_its_own_shared_id` pins it, and the warnings on every Home render and
  in the search screen are gone.
- **Subtitle Serif**: a family list through fontconfig instead of one face.
- **Not done, needs the live app**: the `RUNE_PERF_WARN` / `RUNE_REGION_MISS` audit, which wants a
  real window and a session, and the `caret-blink` pick (it would add a permanent fork patch for a
  desktop-only caret; better asked upstream).

## Findings about our own code, from running the new pin

- **Duplicate matched ids on Home** (fixed above). Every Home render printed, in a debug build:
  `duplicate matched id "poster-quiet-floor" … give each element its own id` (also
  `poster-service`, `poster-long-orbit`, `poster-harbour`). Two rails show the same series, and
  `screens.rs` matches both on `poster-{id}`. Rune matches the last in tree order, so the flight
  from a card to a detail page can start at the other rail's copy. Fix: a rail suffix on the
  card's id and the origin carried into the detail's poster. Today it is a warning and a
  wrong-origin flight, not a broken screen.
- **Accessibility is compiled out** (fixed above). `crates/runefin/Cargo.toml` asked for
  `default-features = false, features = ["embedded-fonts", "svg", "dbus"]`; upstream's `accessibility`
  is a default feature, so it is off here. The tree exists either way (`--tree`,
  `Runtime::accessibility_tree`), the platform adapter does not.
- **Startup budget.** Debug first frame on `home` is 406 ms with 140 ms of GPU init; the snapshot
  views report 105–283 ms of GPU init each. The snapshot path discards the prewarm, so those
  numbers are the cold path, not what a window pays.
- **The end-card test depended on wall clock.** Fixed here; the pattern to avoid is injecting a
  session position without the player clock that `poll` reads.

## Leave out

- Windows (51 commits: vendored winit, video layers, jump lists, drag-out, UIA), macOS (25: Dock,
  notifications, Metal), web/wasm (15), Media Foundation: no target.
- `metal-backend`, `rune-fx`, `rune-light`, `rune-knobs`, `rune-mac`, `rune-luau`: the app has its
  own tokens and components; adopting the kit would be a redesign.
- Mermaid, Markdown find, spell checking: the app shows no documents.
- `rune-kit`'s toast stack and popover geometry: our toasts and sheets predate them and read our
  own tokens; only the *behaviour* (a re-shown toast pulsing in place, a stack that peeks) is
  worth copying by hand if the toast queue ever grows.

## Community

- The mirror's contributor branches (read then as `forgejo/*`): 40+ branches, all at least 750
  commits behind `upstream/dev`; most are already in `dev` (`p111-image-pool` = the four-thread
  decoder, `emoji-color`, `boundary-lints`, `mermaid-lenient`). Still open and interesting:
  - **`caret-blink`** (one commit, 207 lines with tests): a text-field caret blinking 0.5 s on and
    off, restarted by an edit or a caret move, one wake per change, still when the window isn't
    key, off under Reduce Motion. The app has real text fields on the desktop side (search, server
    address, Seerr key); a solid caret is the remaining rough edge. A candidate to cherry-pick
    into the fork, or to ask upstream to land.
  - `t1208-timing` / `t1208-sampler` (1.2k / 814 lines): time every interaction from input to
    present, named by page and control; sample the UI stack when a frame runs slow. The right
    tool if remote-to-picture latency ever becomes the complaint, but big and stale.
  - `select-peek`, `scroll-indicator`, `segmented-badge`, `fix/kit-expanded-trigger-overlays`:
    small `rune-kit` affordances (a listbox that shows half its next row, edge fades on a list, a
    badge on a segment). Ideas to copy into our own lists; the code is kit-shaped.
- `neiii/awesome-rune`: the listing now includes **Runefin** under Apps → Media with the prepared
  description. No further action there (the entry is JSON; Flathub or a signed repo would be the
  next reason to touch it).

## Recommendation

Ship the bump on its own, then take items 1–4 in that order: prewarm and the accessibility feature
are one-line changes with a design-doc line, the debug-tool audit is free information, and naming
the controls closes the loop on the tree. Items 5, 6 and 8 are polish that moves snapshots and can
ride any UI change. Item 7 is its own milestone; if the app ever feels slow on a weaker box, read
`RUNE_REGION_MISS` first and convert the screen that reports rebuilds, not the whole `State`.
