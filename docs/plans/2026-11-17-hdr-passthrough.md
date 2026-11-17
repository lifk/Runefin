# HDR passthrough — implementation plan

**2026-11-17.** Audio passthrough shipped this pass; HDR output remains a Rune renderer
milestone. This document is the scoped plan and the reason it is not an app toggle.

## Today

- The video path decodes 10-bit HEVC/AV1, keeps BT.2020, and tone-maps PQ/HLG in
  `native_surface/video.wgsl` (Reinhard; reference white 0.8) into the app's 8-bit sRGB surface.
- Rune's window config (`window.rs::surface_config`) picks the surface's **sRGB** format
  (`Bgra8UnormSrgb` by default) and its renderer is built for that target. The UI, images,
  video and OSD all composite there.
- wgpu 30.0.1 now exposes `SurfaceColorSpace::Bt2100Pq` / `Bt2100Hlg` and 10-bit formats;
  `wgpu-hal` maps Vulkan `HDR10_ST2084_EXT` and reports color-space capabilities per format.
  Whether a surface advertises HDR is the compositor's answer (`wp_color_management_v1` on
  Wayland; KDE Plasma 6 is the realistic test). This is a capability to probe, not a guarantee.
- Rune does not request HDR, has no API to switch color space at runtime, and its renderer target
  is 8-bit sRGB. Setting `RUNE_HDR=1` in the app would produce wrong colors, not HDR.

## What “HDR passthrough” has to mean

1. The decoder's BT.2020/PQ (or HLG) samples reach the display without the Reinhard SDR map.
2. The UI stays SDR at a defined white level (about 100 nits in a 1 000-nit container), and the
   video's highlights go above it.
3. The compositor receives an HDR surface with the right color space and, where the protocol
   allows, mastering metadata.
4. The OSD, subtitles and menus are composited over the HDR frame, not in a separate window.
5. A display/compositor that cannot accept HDR falls back to the existing SDR tone map, visibly
   and honestly.

## Architecture

**Preferred: one HDR render target and a final PQ encode.**

- Render the whole UI + video into a linear `Rgba16Float` (or `Rgb10a2Unorm`) target.
- UI colors are sRGB-decoded to linear and scaled so SDR reference white is 100 nits
  (0.1 of a 1 000-nit container); video PQ is decoded to linear scene values and kept above 1.0.
- Composite in linear, then a fullscreen present shader encodes PQ (or HLG) and writes the
  HDR surface.
- Blend/alpha behavior must be checked: sRGB-encoded UI on a linear HDR target is the classic
  “washed out / too bright” bug.

**Rejected alternatives.**

- Toggle a 10-bit surface while still writing tone-mapped sRGB: wrong colors, no benefit.
- Separate HDR video window plus SDR OSD window: focus, fullscreen, compositor mixing and
  multi-window lifecycle are worse than one renderer path, and Rune is single-window by design.
- Only request HDR when the file is HDR, without a renderer path: a black/incorrect frame is
  worse than a correct SDR tone map.

## Rune changes (fork, then upstream)

1. **Surface options.** `SurfaceConfigOptions { color: Sdr | HdrAuto | HdrPq }` on `WindowSpec`
   (default `Sdr`). `surface_config` asks for `Rgb10a2Unorm` or `Rgba16Float` + `Bt2100Pq`
   when the caps include it; store the chosen format/color space and a `hdr_available` flag.
2. **Capability probe.** Expose `WindowHandle::hdr_surface()` / `hdr_color_space()` (or a
   `HdrSupport` enum) so the app can show “available / unsupported / SDR fallback” in
   Diagnostics and only offer the setting when it can work.
3. **Renderer target.** Permit a linear HDR target: all pipelines write linear values or a
   dedicated present pass converts the existing SDR UI texture. The smallest correct version:
   - keep the existing SDR renderer for UI into an offscreen `Rgba8UnormSrgb`;
   - render video into an `Rgba16Float` HDR texture (no tone map);
   - fullscreen composite shader: HDR video + sRGB-decoded UI at 100 nits → PQ;
   - present that to the HDR surface.
   That isolates the change from 40+ UI pipelines and is the recommended first slice.
4. **Video shader.** Add an HDR output mode: PQ EOTF without Reinhard, BT.2020 matrix, values
   above 1.0 preserved; the SDR mode keeps the calibrated tone map. Cap metadata (MaxCLL,
   MaxFALL, mastering display) when the surface/protocol accepts it.
5. **Runtime switch.** Reconfigure the surface between SDR and HDR when playback starts/stops
   (or when the setting changes); rebuild the renderer and the present pass, and keep the
   window alive. A failed reconfiguration returns to SDR with a logged reason.
6. **Tests.** Unit tests for the format/color-space choice and the PQ encode; a headless render
   of a known HDR ramp through the present shader; a window integration test that simulates
   “HDR not advertised” and falls back.
7. **Docs and diagnostics.** `docs/design-player.md` §3.4 and the Settings row say what is
   actually happening. No dead switch.

## App changes

- `Settings → Playback → HDR output: Auto / Off` (Auto only when the surface reports HDR).
- `PlaybackInfo` profile: mark HDR-capable and choose DirectPlay for HDR10 sources; keep the
  Dolby Vision refusal (DV needs libplacebo).
- Player OSD: `HDR10` / `HLG` badge only when the output is actually HDR; `SDR tone map` in
  diagnostics otherwise.
- `RUNEFIN_HDR=0/auto` for a launch, and a one-time toast when the compositor refuses HDR.
- The SDR fallback remains the default and the tested path on every machine without HDR.

## Verification

- KDE Plasma 6 with HDR enabled, AMD RADV/VCN, 4K HDR10 TV over HDMI 2.1.
- Test patterns: a 1 000-nit ramp, black-level steps, BT.2020 primaries, HLG, and a real HDR10
  film; compare against `mpv --target-colorspace-hint=yes` on the same display.
- Check UI white level, subtitle white, OSD contrast, alpha blending over video, and that
  leaving playback restores SDR without a flash or a lost surface.
- Measure the present pass cost at 4K60; HDR must not add a frame.
- Fallback test: compositor with HDR off, NVIDIA without a working HDR surface, and a monitor
  that advertises 10-bit but not PQ.

## Estimate and risks

- Rune surface + capability + present path: **2–3 weeks**, most of it in the composited present
  pass and the reconfiguration path, plus hardware iteration.
- App settings/profile/OSD: **3–5 days** after the Rune API exists.
- Risks: Wayland color-management protocol versions, compositor HDR behavior, SDR white-level
  calibration, UI-in-HDR banding, and deciding whether HDR is only allowed fullscreen. None is
  a reason to fake it in the app.

## Interim

Keep the calibrated SDR tone map and the honest “HDR content, SDR output” label. A later
quality pass can replace Reinhard with BT.2390 and add 10-bit dithering if the HDR milestone
slips; that improves the fallback, not the passthrough.
