# HDR output — implementation plan

**2026-11-18.** The first HDR slice shipped: extended-range **scRGB** output
(`Rgba16Float` + `ExtendedSrgbLinear`) with a linear composite, absolute PQ/HLG video and an
honest SDR fallback. This document records the design, what is deliberately not done, and the
hardware verification that remains.

## What shipped

- **Rune surface.** `App::hdr(Hdr::Auto)` makes a window ask its surface for `Rgba16Float` in
  `ExtendedSrgbLinear` when the surface advertises that colour space; otherwise it keeps the SDR
  format. `surface_config_hdr` is the single selection point, and it retries SDR if the driver
  refuses the HDR swap chain.
- **Linear composite.** An HDR window forces `srgb_blend = false`: fills, images, text coverage,
  layers, backdrops and video all hold linear light. The window pass scales that light by
  `HDR_WHITE_SCALE` (`203/80 = 2.5375`) so UI white lands on the compositor's SDR reference white
  (BT.2408 graphics white, 203 cd/m²). Isolated layers stay unscaled and are scaled when
  composited, so a layer never double-scales its contents.
- **Video shader.** `native_surface/video.wgsl` keeps two output paths. On SDR it tone-maps as
  before (Reinhard). On scRGB it decodes PQ to absolute luminance (0–10,000 cd/m²), runs HLG
  through the BT.2100 OOTF at a 1,000 cd/m² nominal peak, converts BT.2020 → BT.709 and divides
  by 203 so `1.0` is SDR white and highlights stay above it.
- **Runtime switch.** `Tx::set_hdr(Hdr)` reconfigures the surface and rebuilds the renderer and
  the video pipelines in place; no playback restart. `rune::hdr_status()` reports `Active`
  (with format and colour space), `Unsupported`, `Failed`, `Off` or `Unknown`, and the app shows
  that under its Settings row.
- **App.** `Settings → Playback → HDR output: Auto / Off`; Auto is the default, the switch
  applies on the next frame, and the note states what the surface actually got.
- **Verified locally** on RADV/Wayland: the surface configures and presents as
  `Rgba16Float + ExtendedSrgbLinear`; the video shader's PQ/HLG math was checked against
  ST.2084/BT.2100 on a float target for 203 cd/m², 1,000 cd/m² and HLG 75 % (the first two land
  at 2.52 and 12.68 in the window, i.e. SDR white and ~5× white, with no clip).

## What “HDR output” means here

1. The decoder's BT.2020/PQ (or HLG) samples reach the display without the Reinhard SDR map.
2. The UI stays SDR at a defined reference white (203 cd/m²), and the video's highlights go above
   it.
3. The compositor receives extended-range linear light and owns gamut mapping, tone mapping and
   the display signal.
4. The OSD, subtitles and menus composite over the HDR frame in the same window.
5. A display/compositor that cannot accept HDR falls back to the existing SDR tone map, visibly
   and honestly.

## Why scRGB first, not a PQ present pass

The plan originally proposed rendering into a float target, then a fullscreen present pass that
encodes PQ/BT.2020 and writes an `Rgb10a2Unorm` surface. That is the path a TV needs for an
**HDR10 badge** (PQ signal plus mastering metadata), but it has costs this slice did not need:

- every pass would either encode PQ before blending (wrong for text and AA) or need a real linear
  intermediate plus a present pass;
- HDR10 metadata is not settable through wgpu 30, so the badge could not be attached anyway;
- scRGB is advertised by RADV, Metal EDR and DX12, and KWin maps it to the panel correctly.

scRGB carries absolute linear light, so the video grade is preserved and the compositor does the
display-specific work. It is the portable path today. PQ/BT.2020 output with static metadata is
still the follow-up if a TV badge or a non-tone-mapping sink is required; it builds on the same
linear scene.

**Rejected alternatives.**

- Toggle a 10-bit surface while still writing tone-mapped sRGB: wrong colours, no benefit.
- Separate HDR video window plus SDR OSD window: focus, fullscreen, compositor mixing and
  multi-window lifecycle are worse than one renderer path.
- Only request HDR when the file is HDR, without a renderer path: a black/incorrect frame is worse
  than a correct SDR tone map.

## Remaining work

1. **Hardware verification.** A real HDR TV in Desktop Mode (KWin HDR on): UI white level,
   subtitle white, OSD contrast, alpha over video, 1,000/4,000-nit ramps, BT.2020 primaries, HLG,
   and the SDR↔HDR switch with playback running. The development panel here reports HDR
   unsupported, so this is still open.
2. **Gaming Mode.** gamescope does HDR, but it does not expose HDR/colour management to a
   non-Steam Wayland client; a Steam tile launched under gamescope stays SDR. Desktop Mode is the
   supported HDR path.
3. **PQ/BT.2020 with metadata.** A linear intermediate plus a present pass, `Rgb10a2Unorm`, and
   mastering-display/MaxCLL/MaxFALL metadata once the compositor/API surface exists. Needed only
   for a TV badge or a sink that cannot do scRGB.
4. **Per-content policy.** Auto currently enables HDR for the whole window, which is the TV model
   (UI in HDR at reference white). If a user wants HDR only while a film plays, that is a policy
   change, not a renderer change.
5. **Dolby Vision.** Still needs libplacebo; the app refuses DV profiles and DirectPlays the
   HDR10 base layer where the server offers one.

## Verification that is already automated

- `window::tests::hdr_*` cover the surface choice: scRGB only for `Rgba16Float` with
  `ExtendedSrgbLinear`, SDR for a float surface that only offers sRGB, and the SDR format choice
  with and without gamma blending.
- The video shader was rendered to an `Rgba16Float` target with a PQ/HLG frame and read back:
  203 cd/m² → 2.5176 (Q16), 1,000 cd/m² → 12.68, layer scale → 0.992, SDR tone map unchanged
  (0.889), HLG 75 % → 2.521. This is a local GPU check, not a committed CI test yet.

## Risks

Wayland colour-management protocol versions and compositor HDR behaviour; SDR white-level
calibration (203 cd/m² is BT.2408's graphics white, not every desktop's SDR setting); banding in
smooth UI gradients at 16-bit float (none expected, but a 4K TV pass should confirm); and the
per-display HDR state, which wgpu does not expose on Vulkan/Linux — the app can only ask whether
the *surface* supports the colour space, not whether the panel is currently in HDR mode. KWin
tone maps scRGB to SDR when HDR is off, so Auto remains correct there; it just does not make an
SDR panel brighter.
