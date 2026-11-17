# Rune and ecosystem update — 2026-11-17

The monthly Rune pass, plus a review of the direct dependency and runtime stack. This is the
evidence for the pin bump and the line between "take now" and "wait".

## Rune

**Pinned:** `2d549f176b2c612a7da0c37c19f6fac505e6f003`, fork branch `clean/2026-11-17`.
**Previous:** `0dbad3f0695c4e9ce11d1876491c2fb35e32e1b2`.
**Upstream:** `anomalyco/rune` `842c28c2`, **157 commits** ahead of the previous pin.

The previous clean branch was a merge of `upstream/dev` onto the 15 player/HDR commits. The new
branch merges `upstream/dev` on top of that set. Two conflicts:

- `app.rs`: upstream moved `Requests`/`Shared` into `app/requests.rs`. Took the module and
  re-applied the app's `fullscreen` and injected-key (`gamepad`) fields, their defaults, and the
  `asks_window`/`only_updates` rules.
- `runtime/frame.rs`: the frame-finish path was refactored. Kept the app's focus-by-key
  behaviour by pushing focus requests into `tx.scroll_to` and keeping quiet focus quiet.

One fork-only adaptation: upstream's Windows target asked for `parley_engine`'s
`line-dictionaries` feature, which exists only in Rune's vendored copy. A dependency's
`[patch.crates-io]` does not apply to the workspace that consumes it, so an external Linux app
resolved crates.io `parley_engine` and failed. The Windows target now uses the published crate
without that feature (line breaks fall back to `icu.dll`); Runefin does not target Windows.

### What arrived without app work

- **Text**: layout a view's text ahead into the layout cache without mounting it; whole-pixel
  text under gliding growth rows; Chromium-exact inline-code line boxes; Linux `FontSmoothing::Off`
  now follows Skia's Linux gamma (7–14 % lighter light-on-dark text).
- **Lists**: virtual rows that grow and trade places without moving the rest of the window;
  `scroll_key_to` landing an unmounted virtual row; wheel routing that does not trap on a nested
  viewport; focused scroll views that scroll on arrows/Space.
- **Focus and accessibility**: `focus_when_shown` waiting for an element; focus fallback when the
  focused element leaves; checkboxes/tabs/switches/radios/menu items say their words once; text
  fields of any length walkable by a screen reader; tooltip dismissal on trigger press.
- **Images**: inline image wakes distinguished from queued decodes; glyph and image atlas pages
  that start small and grow.
- **Layers/paint**: windows at rest return spare backdrop copies and targets; press tint on any
  button; hairlines and rectangle icons draw as Chromium's.
- **Motion**: `Cycle::DISPLAY_RATE` samples a visible opacity cycle on display frames rather than a
  fixed timer. Note: this is a *motion clock*, not display-mode switching; it does not give HDR or
  24p output.

### Worth adopting next, not in this bump

| Upstream | Use in Runefin |
|---|---|
| `Tx::focus_when_shown` / `focus_when` | Focus a control that a load or motion has not mounted yet (search results, library first card) |
| `Tx::scroll_key_to("row/mark")` | Replace some of the app's manual `scroll_into_view` calls, especially in virtual lists |
| `El::enter_delay` + `Stagger` | Stagger rail and sheet entrances; respects Reduce Motion |
| `El::hover_alone` | A play badge that does not light the whole card |
| `Tracked` fields / `region_with` | Incremental Home/library rebuilds; measure with `RUNE_REGION_MISS` first |
| `Tx::animated` | First data paint instead of a cut |
| `RUNE_OPEN_INACTIVE` | Launch a second instance without stealing focus for diagnostics |
| Linux text gamma | Already visible; check the committed snapshots (done) |

## Direct dependencies

Checked against crates.io on 2026-11-17. Applied: **zbus 6.3.0**, **ureq 3.5.2** (+ `ureq-proto`
0.7.1), **zeroize 1.9.2**, **flate2 1.1.11**. These are semver-compatible and the full suite is
green.

| Crate | In tree | Latest | Decision |
|---|---|---|---|
| `gstreamer` and plugins | 0.25.4 / 0.25.2 | 0.25.4 | Current; no action |
| `image` | 0.25.x | 0.25.10 | Current; no action |
| `serde` / `serde_json` | 1.x | 1.0.229 / 1.0.154 | Current through `cargo update` |
| `libbitsub-core` | 1.13.2 | 1.13.2 | Current |
| `gilrs` | 0.11.2 | 0.11.2 | Current |
| `signal-hook` | 0.3.18 | 0.4.6 | **Wait**: 0.4 is a major; the 0.3 iterator API is enough |
| `toml` | 0.8 | 1.2.1 | **Wait**: config format/round-trip migration; not a release-week change |
| `zbus` | 6.3.0 | 6.3.0 | Applied |
| `ureq` | 3.5.2 | 3.5.2 | Applied |
| `zeroize` / `flate2` | 1.9.2 / 1.1.11 | same | Applied |
| Flatpak runtime | Freedesktop 26.08 | 26.08 | Current |
| Rust | edition 2024, 1.85+ | toolchain 1.99 | Current |

`cargo update` reported 41 unchanged dependencies behind latest, all patch/minor transitive
changes; no direct dependency is materially behind except the two deliberate holds above.

## HDR and audio, as exposed by this stack

- **HDR output is a Rune milestone, not an app flag.** wgpu 30.0.1 exposes
  `SurfaceColorSpace::Bt2100Pq`/`Bt2100Hlg` and 10-bit formats, but Rune's window config still
  selects the surface's sRGB format and its renderer target is `Bgra8UnormSrgb`. The DMA-BUF
  shader tone-maps PQ/HLG into that SDR target (reference white 0.8). Passing HDR through needs a
  10-bit/16-bit target, an HDR color-space request on the surface, PQ/HLG output encoding for the
  whole composited frame, and a rebuild path when playback starts/stops. That is a fork feature
  with hardware verification; it cannot be turned on from `runefin` alone.
- **Audio passthrough shipped in this pass** (`player.rs`, Settings → Playback): the filter chain
  is skipped, `soft-volume` is dropped, `pulsesink` accepts AC3/EAC3/DTS/TrueHD, volume/mute live
  on the sink, a start that cannot open the output retries once as decoded, and the transcode
  profile offers 5.1. It is off by default and the fallback path is unchanged.
- **HDR output is still a Rune milestone.** The concrete surface/renderer/present-pass plan and
  the hardware matrix are in `docs/plans/2026-11-17-hdr-passthrough.md`. Do not add an app toggle
  before the Rune API exists; a 10-bit surface with sRGB tone-mapped values is worse than the
  current correct SDR picture.

The Rune bump is a clean, independently revertible commit; audio is a separate app change.
