# More formats in Rune's window — renderer/pipeline options

**Status:** analysis, 2026-11-10. Does not change `docs/design-player.md`; it prices the paths that
doc deferred. Rune under review is the pinned fork rev `93766b16` (wgpu 30.0.1).

**Method.** Read the design doc, `crates/runefin/src/{player,dmabuf_sink,handoff}.rs`,
`state/in_app_state.rs`, the Rune fork (`native_surface/`, `window.rs`, `render/layers.rs`) and the
GStreamer/libplacebo sources. Box: RADV STRIX_HALO, KDE Wayland, GStreamer 1.28.8, mpv 0.41 +
libplacebo 7.360, FFmpeg 9.0.

---

## 1. What today's path does, exactly

`playbin` → `vapostproc` → `capsfilter` → `RunefinDmabufSink` → `NativeSurface::push_dmabuf` →
`dmabuf::Importer::import` → video quad (`crates/runefin/src/player.rs:492`, `:732`;
`dmabuf_sink.rs:114`, `:176`). The importer copies each plane into a wgpu-owned texture and releases
the producer after `on_submitted_work_done` (`crates/rune/src/native_surface/dmabuf.rs:283`): **zero
CPU copy, one GPU blit**, not a direct external-image sample. Colour conversion and PQ/HLG→SDR tone
mapping live in `native_surface/video.wgsl:41` with a Reinhard shoulder (`mod.rs:400`). Fallbacks:
CPU NV12 `appsink` (`player.rs:759`), server transcode retry (`state/in_app_state.rs:781`), then a
separate mpv window (`handoff.rs:5`).

Consequences that decide this document:

- **No Dolby Vision metadata ever reaches us.** Our Jellyfin device profile forbids the DOVI
  ranges so the server transcodes instead (`crates/jellyfin-api/src/client.rs:434`).
- **HDR10 static metadata is already in the caps and we ignore it.** `h265parse` publishes
  `mastering-display-info` and `content-light-level` (`gsth265parse.c:2688-2721`), and
  `dmabuf_sink.rs:114` reads only colorimetry matrix/transfer.
- **HDR output does not happen**: `surface_config` always picks an sRGB format and never sets
  `color_space` (`crates/rune/src/window.rs:670`).

---

## 2. The options

| | 1. GStreamer + a libplacebo element | 2. mpv embedded (`mpv_render_context`) | 3. FFmpeg decode + libplacebo | 4. Close the GStreamer gaps | 5. libplacebo *inside Rune*, GStreamer keeps the rest |
|---|---|---|---|---|---|
| **Exists upstream today?** | **No** — there is no libplacebo element (see §3.1) | Yes, but only GL/D3D11/SW backends | Yes (all pieces are libraries) | Yes | Yes (libplacebo API is public and stable) |
| **DV 8.x** | base layer only | yes, all profiles | yes, all profiles | base layer only | yes, if we can feed RPU data |
| **DV 5** | no | **yes** | **yes** | no | only with an RPU parser we write |
| **DV 7 (FEL/MEL)** | base layer (HDR10) | base layer only | base layer only | base layer | base layer |
| **HDR10/HLG output** | no (nothing downstream is HDR) | no (Rune presents) | no (Rune presents) | no | no (Rune presents) |
| **Tone-map quality** | libplacebo-class | best in class | best in class | our shader + MaxCLL | best in class |
| **Broken containers** | unchanged GStreamer demux | mpv's own demuxers | FFmpeg's demuxers | fix here, cheaply | unchanged GStreamer demux |
| **Subtitle burn-in** | no | yes (mpv's libass OSD, replaces ours) | no | no | no |
| **Zero-copy** | yes (dmabuf in) | no: GL FBO → EGL dmabuf → import (one extra copy) | yes in, GPU blit out | yes (today) | **best**: libplacebo reads the dmabuf and writes into a Rune texture |
| **New deps** | libplacebo + a new element | libmpv (GL/EGL) | libavcodec/libavformat + libplacebo | none | libplacebo (+ maybe `libdovi`) |
| **Licence** | LGPL-2.1+ | **GPLv2+ default**, LGPLv2.1+ only with `-Dgpl=false` | LGPL-2.1+ (FFmpeg build dependent) | — | LGPL-2.1+ |
| **Rough effort** | 3-4 weeks, and still no DV5 | 4-6 weeks, and we lose our player | 8-12 weeks | **3-5 days** | 4-6 weeks (1-2 week spike first) |
| **Verdict** | not a real option | escape hatch, not the design | only if GStreamer decode is the actual problem | do this first | **the strategic path** |

---

## 3. Three questions, answered

### 3.1 Is there a GStreamer 1.28 element that parses DV and tone-maps it?

**No.** Four independent checks:

- The GStreamer monorepo has no libplacebo plugin: `subprojects/gst-plugins-bad/ext/` lists no
  `libplacebo` entry, and there is no such page in the plugin docs
  (<https://gstreamer.freedesktop.org/documentation/plugins_doc.html>). `gst-inspect-1.0 libplacebo`
  and `placebovideosink` are both missing on a complete 1.28.8 install here. The 1.28 release notes
  (<https://gstreamer.freedesktop.org/releases/1.28/>) mention no libplacebo and no Dolby Vision.
- **DV is not demuxed as DV.** `qtdemux` recognises the `dvh1`/`dvhe`/`dva1`/`dvav` sample entries
  (`qtdemux_types.c:198`, `qtdemux.c:9727`) but parses them as plain HEVC; the `dvcC`/`dvvC`
  configuration record is never surfaced in caps. `matroskademux` parses `BlockAdditions` and then
  **frees them unless they are VP8/VP9 alpha** (`matroska-demux.c:5761`), which is where the mkv RPU
  lives. `h265parse` exposes no DOVI caps field, and the GStreamer video library has no Dolby Vision
  meta type at all (`grep -ri dovi /usr/include/gstreamer-1.0/` finds only AC-3 riff ids).
- **No tone mapper.** `vulkancolorconvert` does matrix/range conversion only; primaries conversion
  is commented out (`ext/vulkan/vkcolorconvert.c:483`) and it exposes no properties. `glcolorconvert`
  is the same class of element. `vulkansink` is its own window (`vulkan` plugin docs:
  <https://gstreamer.freedesktop.org/documentation/vulkan/index.html>).
- `gldisplay` is not an element name; the GL plugin has `glcolorconvert`, `glimagesink`,
  `gldownload` and friends, none of which tone-map.

What 1.28 *did* add that is relevant: 10-bit `vulkanh265dec` (but it outputs
`memory:VulkanImage`, not DMA-BUF — `gst-inspect-1.0 vulkanh265dec`), and waylandsink HDR10
metadata/colorimetry handling (1.28 highlights) — both in sinks we cannot use, because they own a
surface.

### 3.2 Is `mpv_render_context` workable on Wayland with a Vulkan texture shared into Rune?

**Not as "a Vulkan texture".** The render API has exactly three backends — `..._OPENGL`,
`..._D3D11`, `..._SW` (`include/mpv/render.h:52-54`); there is no Vulkan render API. The
`gpu-next`-through-the-app's-GPU PR ([mpv #16818](https://github.com/mpv-player/mpv/pull/16818)) is
still open and unmerged (checked 2026-11-10, last touched 2026-09-16) and is OpenGL-first.

Workable shape on Wayland: create our own EGL/GLES context (`MPV_RENDER_PARAM_WL_DISPLAY`,
`render.h:249`), render mpv into an FBO, export that buffer with `EGL_MESA_image_dma_buf_export`, and
feed the fd to Rune's importer, which already handles explicit DRM modifiers on Vulkan and EGL.
Costs: a GL context beside a Vulkan device, modifier roulette between Mesa's GL export and RADV's
import, one extra full-frame copy, mpv's own audio clock and libass OSD (subtitles we draw today),
and the GPLv2 default. It works; it just replaces our player with mpv's and adds a copy.

### 3.3 What does HDR output require in Rune?

`wgpu` 30.0.1 is **not** the blocker `design-player.md §3.4` assumed. It has
`SurfaceColorSpace::{Bt2100Pq, Bt2100Hlg, ExtendedSrgbLinear, …}`
(`wgpu-types-30.0.1/src/surface.rs:242`), per-format capability sets
(`SurfaceCapabilities::format_capabilities`), `Surface::display_hdr_info`, and the Vulkan HAL maps
the colour space onto `VK_COLOR_SPACE_HDR10_ST2084_EXT`/`HDR10_HLG_EXT`
(`wgpu-hal-30.0.1/src/vulkan/conv.rs:190`). On this box `vulkaninfo` reports the Wayland surface
accepting both HDR10 colour spaces with `FORMAT_A2B10G10R10_UNORM_PACK32`.

So the work is in Rune, and it is contained but not small:

1. Pick a `(format, color_space)` pair from `format_capabilities` — `Rgb10a2Unorm` + `Bt2100Pq`,
   keeping today's sRGB path as fallback — in `surface_config` (`crates/rune/src/window.rs:670`),
   and re-run the choice when `display_hdr_info` changes (OS HDR toggled).
2. **Encode PQ + BT.2020 at the end of the frame.** wgpu encodes nothing for non-`*Srgb` formats.
   Rune already keeps linear premultiplied `Rgba16Float` intermediates "to preserve HDR colours"
   (`crates/rune/src/render/layers.rs:6`), so the clean version is: always composite to that
   intermediate, map 1.0 → SDR white (≈203 nits, so chrome looks unchanged), then PQ-encode once in
   the final blit. Per-fragment encoding in `shader.wgsl:359` would blend chrome in PQ space.
3. **Video writes absolute light.** The video shader's `tone_map` branch becomes a passthrough for the
   HDR output case (`video.wgsl:73`): PQ→linear→BT.2020, values above 1.0 for highlights; tone
   mapping is used only when the display is SDR or the source exceeds its peak.
4. **Mastering metadata.** wgpu's docs are explicit that `Bt2100Pq` "configures only the PQ colour
   space", not ST 2086/MaxCLL. A TV shows a PQ signal without its HDR badge unless the compositor
   gets an image description with mastering luminance — on Wayland `wp_color_management_v1`, which
   wgpu has no API for. Either a wgpu/Rune-hal gap to file, or "HDR picture without the badge".
5. **Verify the compositor actually switches.** RADV advertising the colour space is necessary, not
   sufficient; Plasma must be in HDR mode and Mesa must tag the surface. Not verified here.

---

## 4. Recommendation

**Do 4 now, prototype 5, keep 2 as the emergency exit, do not start 1 or 3.**

Option 4 (days) buys real quality with no new dependency: read `mastering-display-info` and
`content-light-level` in `dmabuf_sink.rs:114` into `ColorInfo` so the tone-map curve uses MaxCLL
instead of a fixed exposure; show the sink path and colour facts in Statistics; add `playbin3` +
`decodebin3` fallbacks and an explicit "DOVI → transcode" notice so a DV5 file does not look like a
bug. It does **not** give DV5 or HDR output.

Option 5 scales because the blocker for Dolby Vision is *metadata plumbing*, not tone-mapping quality:

- libplacebo imports an existing device (`pl_vulkan_import`, `libplacebo/vulkan.h:484`), wraps a
  `VkImage` as a texture (`pl_vulkan_wrap`, `:539`), and imports DMA-BUF planes directly
  (`PL_HANDLE_DMA_BUF`, `libplacebo/gpu.h:163`, with a DRM modifier). Rune already reaches backend
  handles through `wgpu-hal` (`crates/rune/src/gpu.rs:20` does this for Metal), so libplacebo can
  render **into a Rune-owned texture on Rune's own queue** — no second window, no second GPU API,
  one blit less than today.
- DV5/PQ reshaping then needs `pl_color_repr::dovi` filled from the per-frame RPU
  (`libplacebo/colorspace.h:132`, `:183`). FFmpeg extracts it (`AV_FRAME_DATA_DOVI_METADATA`,
  `libavutil/frame.h:208`; `libavutil/dovi_meta.h`), **GStreamer never does** — so on the GStreamer
  path we must either rule DV5 out (server transcode, today's behaviour) or parse RPU ourselves
  (e.g. the `dolby_vision`/libdovi Rust crate) and carry it to the sink, which GStreamer has no meta
  type for. That is the real reason option 3 exists at all.
- AGENTS.md rule 5/6: libplacebo is a new dependency and needs its line in `docs/design.md`; it is a
  *video shader* library, not a second text/render stack, which is the distinction to write down
  explicitly. Licence LGPL-2.1+, no async runtime, no window.

Rough effort: 5 is 1-2 weeks to a go/no-go spike (device sharing, dmabuf import, one tone-mapped
frame next to mpv gpu-next), then 3-4 weeks to ship with HDR10 static metadata and the HDR output
path from §3.3. Option 4 is 3-5 days and should land regardless, because it is the fallback when 5
slips.

## 5. Prototype order (each with a gate)

1. **Measure the failures** (1 day). Play DV5, DV8.1, HDR10, HLG, AV1 and TrueHD files with
   `GST_DEBUG=3` and record the failing stage. Otherwise every option is priced against a guess.
2. **libplacebo device-sharing spike** (3-5 days). `pl_vulkan_import` Rune's device, import one
   P010 dmabuf, `pl_render_image` into a `Rgb10a2Unorm` wgpu texture, compare a frame against mpv
   gpu-next. Gate: no validation errors, no queue deadlock, ≤1 frame extra latency, and the image
   matches mpv.
3. **HDR surface spike** (2 days). A Rune branch that configures `Rgb10a2Unorm` + `Bt2100Pq` with a
   PQ ramp, on Plasma with HDR on and off. Gate: the ramp is not crushed, the SDR fallback still
   works, chrome at 203 nits matches today's white.
4. **DV5 through libavcodec** (only if 2 passed). Confirm libplacebo's DV reshape with a profile-5
   sample fed by FFmpeg. Gate: colours match mpv; if it needs a second decoder stack, stop and keep
   the server transcode for DV5.

## 6. Defer

- HDR10+ dynamic metadata (nothing on Linux signals it; static HDR10 is the right output), and the
  DV profile 7 FEL enhancement layer (mpv ignores it too).
- NVIDIA (`nvidia-vaapi-driver` or `nvcodec`), Windows, macOS — same `DmabufFrame`/`SharedHandle`
  seam, separately designed.
- Burning subtitles into the picture: Rune's overlay is the product decision, and only mpv changes it.
- Replacing GStreamer with FFmpeg wholesale (option 3): rebuilding seek, rate, the audio clock,
  buffering messages and the audio filter chain for a decode path that is not the bottleneck.

## 7. Where I am unsure

- Whether Mesa's Wayland WSI sets a colour-management image description when Plasma is in HDR mode;
  `vulkaninfo` shows the colour spaces are offered, but end-to-end HDR was **not** verified here.
- Whether wgpu's Vulkan device shares cleanly with libplacebo in practice: `pl_vulkan_import` wants
  instance/physical-device/device/queue plus a queue family, and `wgpu-hal` exposes
  `raw_device()`, `raw_queue()`, `queue_family_index()` and `shared_instance()`
  (`wgpu-hal-30.0.1/src/vulkan/device.rs:818`), but the two command streams must be synchronised
  explicitly. That is what prototype 2 is for.
- Exact GStreamer failure modes for `dvh1` mp4s (our profile note says "stalls qtdemux" —
  `client.rs:434`); I had no DV sample to reproduce it.
