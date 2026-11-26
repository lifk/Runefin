# Rune upstream review — 2026-11-25

The Rune pin moves from `83cda583` (2026-11-17) to the 2026-11-25 branch head `c2e2eb6c7`: the
merge `91117a1` of `upstream/dev` `2b4711e4f` (744 commits), plus four follow-up commits
(`Tx::set_hdr` restored, the vendored-winit Wayland/X11 extensions gated behind
`wayland-csd-corners`, and HDR tracked as a renderer request instead of forcing blending in
`set_hdr`). This pass had two jobs: keep the fork mergeable, and read the community's changes
for UI, interaction and design-pattern lessons rather than only for APIs we can call.

Everything below was read in the fetched upstream tree (`git show upstream/dev:...`) or in the
changelog fragments under `changelog/unreleased/` (665 changelog commits since the pin, 841
fragments dated since 2026-11-14). Nothing here is a proposal to port wholesale; the last
section maps the useful parts onto Runefin.

---

## 1. The merge

- 744 commits, 27 conflicted files, 43 hunks. The conflict set is the same shape as the last two
  passes: the HDR window/video path, the 10-foot scale, injected keys, and the fork's small
  patches against files upstream rewrote.
- The fork's delta after the merge is **880 insertions across 41 files**, mostly
  `rune-media/native_surface` (HDR video), `window/hdr.rs`, `window/surface.rs`,
  `window/host/create.rs`, `render/{upload,pipelines,variant,metallib,uber_wgsl}.rs` and
  `runtime/paint/blend.rs`. Heavily refactored non-HDR files were taken from upstream:
  `window/chrome.rs`, `window/tests.rs`, `window/host/create.rs` (then HDR was re-homed into its
  new `setup_surface` closures), `text/system_fonts.rs`, and the input/focus/latch/commit/
  virtual-list internals.
- The HDR scale rides in the window pass's 16-byte `projection` stand-in (`.w`), which the
  canonical WGSL already declares and the window pass never reads: upstream's scroll-fade stops
  keep `flags.zw`, `Globals` stays 32 bytes, and the precompiled Metal/DXBC shaders stay fresh.
  The runtime `uber_wgsl::hdr` variant reads `projection.w` for the window pass only; layers keep
  unscaled linear light and are scaled when composited.
- HDR is now wired through the upstream `setup_surface` closures for initial create, GPU
  fallback and reattach. The renderer records the HDR preference its target was built for and
  `needs_reblend` compares it with `rt.hdr()`: `Tx::set_hdr` renegotiates once, an unsupported
  surface settles on SDR without looping, and a headless runtime (no surface) keeps the blend
  mode the app set. Forcing linear light inside `set_hdr` was tried first and made every
  headless `--snap` differ.

**API supersessions adopted.**

| Fork patch | Upstream API | Decision |
|---|---|---|
| `El::virtual_overdraw(viewports)` | `El::virtual_overscan(overscan, edge)` | Drop the fork builder; Runefin asks for `(1.0, 0.5)` |
| `Tx::set_fullscreen` | `Tx::set_full_screen` (+ `toggle_full_screen`) | Drop the fork alias; Runefin calls the upstream name |
| Global `rune::set_return_activates` | per-element `El::press_on_return()` | Keep the global as a fallback for the TV build; prefer per-control opt-in for new controls |

The global fallback is one line in `Runtime::takes_return` and keeps the 0.3.x TV behaviour; the
per-element builder is the better long-term shape because it does not change Return for every
dialog and menu in the app.

**Integration note.** Upstream's flat-tree accessibility fix tells downstream apps to add
`accesskit_atspi_common` at the pinned Rune rev under `[patch.crates-io]`; without it Linux
AT-SPI reports a container's set size instead of the item's. Runefin now patches it at the pin
in the workspace `Cargo.toml` (it resolves from the same git repo as `rune`).

---

## 2. What the community changed, by design pattern

### 2.1 State styling without rebuilding the view

The clearest direction in this batch is that visual state should be *painted*, not *rebuilt*.

- `El::hover_fill`, `El::press_fill`, `El::hover_border`, `El::press_border` are declarative
  replacement colours; press overrides hover, `Color::TRANSPARENT` is a real override, and
  gradient fills fade by `1 - replacement_weight`. `El::press_motion(down, up)` drives the
  transition without a view rebuild.
- `El::transition_motion(enter, exit)`, `El::matched_motion(anim)`, `El::frame_motion(anim)` let
  any `Anim` (including a timed `Curve::tween` and `Anim::INSTANT`) drive presence, matched
  flights, layout frames and press tracks, while Reduce Motion and instant commits stay
  authoritative.
- `El::live_text` now survives `.wrap()`, `.truncate*`, `.max_lines`, spans, reveals and editors,
  re-wrapping and re-eliding in place. `El::live_range_value` binds a range's number to a
  `Live<f32>` for assistive technology without a commit. `El::live_rotation` adds a live channel.
- `El::el-scale-settle-tail` removed a 0.1 s frame-request tail after a scale track lands.

**Lesson.** Runefin already uses `.hover(..)`/`.press(..)` overlays, which the runtime paints
without a rebuild. The replacement-colour builders cover the other half: a card whose *fill*
changes with state (selected, focused, watched) can paint the replacement instead of rebuilding
the view, which is the cost a 10-foot UI cannot pay per card.

### 2.2 Focus is a state machine, and the framework owns the edges

- A focused control that becomes **disabled or unfocusable while it stays in the tree** hands
  focus to its neighbour, as a removed one does; the same holds inside a kept `region_with`
  region. A control in a **roving group** hands focus to the group's next item, not to a control
  inside it.
- Closing a modal no longer returns focus to an opener that became disabled, inert or
  unfocusable. A modal replacing closing modals in the same commit carries the outermost
  removed modal's return target forward.
- `El::focus_visible_within_ring` draws a ring while focus is on or inside an element **and the
  keys put it there**; `El::focus_ring_for(key, …)` draws it on one element while another has
  focus; `El::focus_border(color)` is a keyboard-only border override;
  `El::focus_ring_radius(f32)` decouples the ring's radius from the element's.
- `El::preserve_focus_on_press(true)` stops a focusable element, or an `on_key`/scroll ancestor,
  from taking focus on click; `El::keyboard_focus_only()` keeps a control Tab-reachable but lets
  a click press it without focusing it.
- `El::on_focus_change`, `_any`, `on_focus_within_change`, `_any` report real transitions across
  keyboard, pointer, AT, field blur, `Tx::focus`/`Tx::blur`, modal autofocus/return and removal.

**Lesson.** A TV UI has no pointer to repair focus after the fact, so the rescue rules matter more
than the rings. Runefin's rail walk already relies on focus surviving commits; these rules cover
the cases it currently handles by hand (a disabled Play button, a modal closing onto a removed
opener, a card that should not take focus when clicked).

### 2.3 Overlays: placement, dismissal, and where the input goes

- `Placement` now covers Floating UI's full 12 placements (`LeftStart`, `LeftEnd`, `RightEnd`
  added). `El::anchored_fit` clamps on both axes and lowers definite `min_width`/`min_height`
  floors when the room is smaller, so submenus stay inside a 420×400 window.
- `El::modal_trap` gives a dialog with no `on_close` a real modal (no close button, no Escape,
  no outside click) instead of dropping it silently. `El::modal_wheel_outside(true)` closes a
  dismissible modal on an outside non-momentum wheel and **reposts** the wheel to the viewport
  beneath once the dismissal commits.
- A **secondary press outside a menu** follows the platform split: Windows closes and reposts to
  what lies under it; macOS, Linux and web close and consume (suppressing the web context menu).
  Dialogs, `Select` listboxes and non-modal `scrim` overlays dismiss and consume everywhere.
- Anchored popup modals no longer report `modal` (WAI-ARIA 1.2), and their trigger stays
  readable while input outside stays blocked. `El::hover_passes_below` lets a non-modal dismiss
  layer take presses while elements under the pointer keep `:hover`.
- Overlays mounted before their anchor in tree order now resolve the later anchor and lift in
  paint and hit order.

**Lesson.** The "close and repost" idea is the important one: dismissal should not swallow the
input that caused it, and it should not leave the layer underneath with stale hover. Runefin's
menus and dialogs are mostly full-screen TV surfaces today, so this matters when the desktop
layout grows context menus and anchored popovers.

### 2.4 Motion: reduced motion never adds, instant stays instant

- Reduced motion never adds or lengthens fades: `El::reveal(0.0)` stays instant and positive
  reveals cap at 0.15 s instead of stretching. `PropClass::reduce`,
  `AnimationStore::play_keyframes` and `Scene::match_geometry` preserve instant and
  zero-duration curves and never lengthen a shorter timed fade.
- `Transition::hold_layout()` collapses an exiting in-flow child's main-axis slot analytically to
  the seam between its surviving siblings, with zero per-frame layout passes. `TabStrip` tabs and
  the keycap overlay now collapse instead of fading over sliding neighbours.
- `El::matched_in_place()` keeps a flying matched element in its destination's paint order and
  ancestor layers, so a segmented-control pill stays behind labels painted after it.
- `El::enter_in_instant()` lets a new element play its entrance during a `Tx::instant()` commit
  while layout and removals still snap.
- `Beat::translate` / `Beat::rotate` bring CSS `@keyframes`-style movement and rotation to
  `El::sequence`; `El::sequence` under Reduce Motion lands a one-shot at its last beat.
- `play_keyframes` returns `Option<TrackId>` and an empty `Keyframes` creates nothing.
- `ScrollBarStyle::scroll_view` and `menu_scroll` fade the thumb on CSS `ease` over 200 ms
  instead of a spring, landing exactly on 0.

**Lesson.** These are invariants, not features. The reduced-motion rule ("never add a fade") is
worth copying into Runefin's own transition helpers, and the analytic `hold_layout` collapse is a
much cheaper answer to the tab-strip slide than a per-frame layout.

### 2.5 Virtualization is a set of explicit trade-offs

- `El::virtual_overscan(overscan, edge)` (the supersession of our `virtual_overdraw`) separates
  **how much stays mounted** from **how close the viewport may come before the window moves**.
  Defaults: `2.0` / `1.75` for `virtual_rows`, `2.0` / `0.5` for `virtualize`; non-finite inputs
  are ignored and `edge` is clamped below `overscan`.
- `El::virtual_rows_append_only()` promises that a growing count appends, so a commit reads only
  the new keys (O(new rows) instead of O(rows)); broken promises fall back to a full rekey and
  warn once in debug.
- Scroll anchoring: `El::no_scroll_anchor()` excludes a row or subtree (a top loading
  placeholder) and can disable anchoring on a whole list; `El::scroll_anchor()` descends into
  tall mounted rows; changing the estimated row height finds the anchor against the old heights;
  a running glide shifts its target by the same anchor delta as the viewport.
- `Tx::scroll_key_to`, `Tx::scroll_key_center_at` and reveal requests resolve scoped paths
  (`"scope/row/mark"`) against **unmounted** `virtual_rows`; `Tx::scroll_into_view_in` scopes the
  reveal to viewports at or inside a scope.
- `VirtualItems` is an app-windowed keyed container for 2D canvases and node graphs: it remounts
  entering/leaving items without re-running `view`, preserves staying subtrees, pins the focused
  item until blur, and supports a mount budget and nearest-first priority.
- Caret browsing crosses unmounted `virtual_rows` with a `PendingCaretMove` machine and pinned
  selection anchors.

**Lesson.** Home's rails already use `virtual_overscan(1.0, 0.5)` after this pass. The
append-only promise fits library pages and the downloads queue; `no_scroll_anchor` fits the
top loading placeholder; the scoped reveal fits "scroll to the next episode" inside a nested
list.

### 2.6 Text: geometry, inline content, and honest truncation

- Geometry queries: `Runtime::text_line(key, byte)` / `Tx::text_line`, `text_offset_at`,
  `Cx::field_selection_rects`, `Tx::link_rect` give an app the field's visual lines, byte offsets,
  selection boxes and link boxes from the last layout — without re-laying out.
- `El::inline(at, slot, child)` places interactive children inside in-flow `InlineBox` slots,
  wrapping with the line, growing line height, supporting nested text, `memo`, scroll-tracked hit
  testing, reading-order focus, AccessKit and RTL. `InlineIcon::label` / `help` name and explain
  an icon set inline in text.
- `SpanBox` adds layout-affecting horizontal span padding and per-corner radii without shifting
  byte offsets; `El::overline` joins underline/strikethrough; `LineFill` adds full-width line
  backgrounds and left bars; `PillStyle::ring`/`radius`, `El::pill_image`, `El::pill_with_image`.
- Elision: `El::elide_middle` and `El::elide_head` combine order-independently with
  `max_lines(n)` and keep trailing text (file extensions) across multi-line clamps.
  `El::nowrap` lays a multi-line field out at natural width inside `scroll_x`/`scroll_xy` with
  `follow_caret`.
- Truncation clips glyphs and decorations to the content box, paints nothing at zero width, and
  keeps the first grapheme clipped rather than replacing it with an ellipsis when only a sliver
  remains (CSS Overflow 3).
- `El::text_indent(points)` handles first-line and negative hanging indents with RTL mirroring;
  `El::selection_enabled(bool)` is a subtree switch; `El::reveal_caret_on_focus(bool)` scrolls
  the enclosing viewport to a field's caret; `El::caret_browsing(bool)` is opt-in keyboard caret
  navigation over selectable text.
- `El::highlight_texts(f)` highlights matches in every text in a subtree without wrapping each
  builder (a settings search can mark matches on whatever page it opens).

**Lesson.** `elide_middle` is the right default for episode file names and long titles;
`highlight_texts` is a cheap search-result treatment; `text_line`/`text_offset_at` are the
primitives for subtitles, chapter marks and a timeline that must align with text. `nowrap` plus a
horizontal viewport is the pattern for a long synopsis the reader can pan instead of clamp.

### 2.7 Accessibility is now a first-class surface, not a label

- `El::aria_disabled(bool)` marks an element unavailable without removing focusability
  (advertises `Focus`, suppresses activation). `El::auto_complete`, `El::has_popup`,
  `El::orientation`, `El::multiselectable`, `El::required`, `El::controls`, `El::owns` wire into
  AccessKit, `Runtime::accessibility_json()` and the web mirror.
- `El::on_set_value` receives AccessKit numeric `SetValue` directly, clamped to `range_value`
  bounds (matching Chrome's `<input type=range>`); controls with only `on_step` replace queued
  steps instead of adding them up.
- `El::on_a11y_action` lets a composite container (`listbox`, `tree`, `grid`, `table`, `menu`,
  `tablist`) receive unhandled actions sent to enabled descendants, bubbling innermost-first;
  actionable items advertise `Click`.
- Virtual lists and grids: non-item rows no longer get `position_in_set`; `row_count`,
  `column_count`, `row_index`, `column_index` map to AccessKit and the web mirror, and
  `virtual_rows` on a `grid`/`table` populates them.
- Live regions: `LiveRegion::Off` opts a child out instead of inheriting; regions pruned behind a
  modal return with `Live::Off` until their content changes, so closing a modal does not
  re-announce unchanged status lines; `El::live_range_value` updates a meter without a commit.
- Web mirror: `aria-pressed`/`checked`/`selected` per role, `tablist` as a `div` with
  `role="tablist"`, closable tabs emitted as sibling buttons, `aria-valuetext` on ranges,
  `role="progressbar"` with min/max/now, `aria-description` for link help, `aria-autocomplete`,
  and status/alert/log politeness carried through `a11y_json` and `bridge.js`.

**Lesson.** A TV app's screen-reader story is thin today: cards, badges and progress bars are
mostly silent or over-described. The `aria_*` setters plus `live_range_value` would let the
player's progress, volume and "Up next" lines be correct without rebuilding.

### 2.8 Input and pointer semantics worth copying

- Hover now follows the **topmost hit element and its ancestors** (CSS `:hover`), so an
  overhanging interactive child keeps its group lit and blocks elements underneath;
  `El::hover_passes_below` is the explicit exception. `on_pointer_move`, `on_hover_change` and
  hover delays reach only that topmost chain.
- An element with both `on_drag` and `on_press` no longer fires `on_press` after a drag past the
  click slop, even when released back inside. A held drag receives a cancelled `Phase::Up` when a
  modal opens or the dragged element leaves/exit/inert.
- `El::pointer_buttons` supports `Secondary` drags; `El::on_button` bubbles Middle/Back/Forward
  without becoming a press target. `El::on_any_press` is a passive window-wide observer firing
  topmost-first before target actions; `PointerEvent::pressing` says whether any button is held.
- `Cx::press_cursor(Cursor)` is a view-wide default for pressables; `Tx::press_point()` gives
  local and window coordinates for pointer-driven activations (`None` on keyboard/AT).
- `Cursor::Help`/`Progress`, the four resize cursors, `El::link_cursor`, and
  `Preview::pan_cursors(bool)` close the remaining cursor gaps.
- `App::media_keys()` opts into hardware transport keys (`PlayPause`, `Next`, `Previous`, …),
  ignored by focused text fields; `Tx::haptic` has a 30 ms same-kind cooldown and an inline ring
  buffer (macOS plays it through `NSHapticFeedbackManager`).
- `El::on_key_preview` gives allowed ancestors key-down events outermost-first, before bubbling
  `on_key`, default buttons, `on_input_key`, focus navigation and editing — a clean place for a
  remote's global Back/Menu chord.

**Lesson.** Media keys are an obvious win for a media app: a Bluetooth remote or keyboard's
transport keys currently do nothing in Runefin. `on_key_preview` is the shape for the TV Back
chord without claiming the key from every screen.

### 2.9 Kit and shell patterns

- `Kit::metrics` makes component metrics (button boxes per `ControlSize`, switch tracks, field
  height/padding/radius/focus outline, text-area style, checkbox colours) data instead of code,
  with `None` defaults that keep the current look.
- One keyboard focus ring across controls, with the OpenCode geometry: buttons 2 pt out;
  checkboxes/switches/radios/collapsible triggers 2 pt out in place of a border; segments, field
  copy/clear buttons and tab pills 2 pt inside. `Kit::forced` (a contrast theme) draws field
  borders in the text colour, 2 pt dashed when invalid, and a 2 pt focus ring 2 out.
- `ColorPicker`, `Slider`, `Canvas`/`ZoomPan`, `Table::virtual_columns`/`frozen_columns`,
  `Table::on_reorder`, `Menu` submenus with `on_submenu`, `Combobox::search_label`,
  `Dialog::fit` bounds/wrapping, `Button::show_shortcut`, `Keybind::keep_case`.
- Shell: `ColorScheme` (`System`/`Light`/`Dark`) per window and app-wide, `SystemMaterial::Sidebar`
  and a per-window native-material cap with fallback reporting, `SplitView::flat_main`,
  `SidebarToggle::labels`, `El::hairline` (a true half-pixel rule), `El::cutout_circles`,
  elevation tokens in Figma layer order, freedesktop themed icons, `ui-monospace` as the default
  mono face.

**Lesson.** Runefin has its own token table, so `Kit::metrics` is a pattern rather than an API to
adopt: the values it now keeps in code (button box, field height, focus ring offset) belong in
`theme.rs` as data. `hairline` and the flat-main shell are cheap visual upgrades; `ColorScheme`
is the missing piece for a desktop light mode.

---

## 3. Mapped to Runefin

| Area | Upstream pattern | Where it lands |
|---|---|---|
| Home rails | `virtual_overscan(1.0, 0.5)` (adopted) | `screens.rs` `home-rails` |
| Fullscreen | `Tx::set_full_screen` (adopted) | `main.rs` pending-fullscreen handler |
| Return on TV | `El::press_on_return()` per control | cards, Play, dialogs; keep the global as fallback |
| TV focus | rescue on disable/removal, `focus_visible_within_ring`, `preserve_focus_on_press`, `keyboard_focus_only` | `tv_focus.rs`, card builders |
| Remote Back/Menu | `El::on_key_preview` | root of each screen, before per-screen keys |
| Media transport | `App::media_keys()` + `Key::Media` | player and shell |
| Search | `El::highlight_texts`, `El::reveal_caret_on_focus`, `TextInput::auto_complete` | `search.rs`, search field |
| Long titles | `El::elide_middle` / `elide_head` + `max_lines` | episode labels, detail headings |
| Descriptions | `El::nowrap` inside `scroll_x`; `Markdown::wrap_anywhere` | detail synopsis, person bios |
| Player OSD | `El::live_text` (tabular digits), `El::live_range_value`, `ScrollSource` | player timeline, volume, progress |
| Dialogs/menus | `El::modal_trap`, `modal_wheel_outside`, `anchored_fit`, close-and-repost | desktop layout, context menus |
| Library/downloads | `virtual_rows_append_only`, `no_scroll_anchor`, scoped reveals | `library.rs`, `downloads.rs` |
| Screen reader | `aria_*` setters, `live_range_value`, `on_a11y_action` | cards, progress, dialogs |
| Theme | `hairline`, `ColorScheme`, metrics as data | `theme.rs`, shell chrome |
| Linux a11y | `[patch.crates-io] accesskit_atspi_common` at the pin | workspace `Cargo.toml` |

**Not adopted now.** The web mirror, `rune-fx`, KaTeX math, `VirtualItems`, `ZoomPan`,
`ColorPicker` and `SystemMaterial` are either platform-specific or outside the three modes; they
stay upstream features, not app work. `Tx::haptic` is worth a try only on a controller that
actually reports haptics (the API is there; the Steam Deck path is unverified).

---

## 4. Verification

- `cargo check --workspace` passes on the merge.
- `cargo test -p rune --lib` on the merge: **3393 passed, 3 failed, 14 ignored**. Two of the
  three also fail on pristine `upstream/dev` in this container, so they are baseline, not
  regressions: `a11y::auto_complete_tests::auto_complete_edges_secure_disabled_read_only_and_rtl`
  and `runtime::selection::field_lines::tests::hard_lines_give_their_starts_and_neighbours_stacked_down_the_field`.
- The third is fork-only and artifact-shaped:
  `render::metallib::tests::precompiled_builtin_and_windfoil_metallibs_match_rune_wgsl` reports
  the fork's HDR `rune-media/native_surface/video.wgsl` missing from
  `crates/rune-gpu/src/metal/builtin_table.rs`. Only `scripts/precompile-metal-shaders.sh` on a
  Mac can regenerate that table; until then macOS compiles that one shader at run time. The
  canonical uber WGSL is unchanged, so the uber Metal/DXBC freshness tests pass.
- Runefin's `scripts/check.sh` and `scripts/snapshots.sh` run after the pin bump; the committed
  views are expected to be byte-identical because none of this pass's adopted APIs changes layout
  or colour.
- HDR hardware validation (HDR TV, scRGB surface, tone mapping) remains a separate pass; the
  merge only preserves the existing path.
