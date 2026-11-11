# Runefin — the player, in-app, on Wayland, at 4K with HDR

**Status:** decision proposed, awaiting your GPU/session answers. Supersedes D1 and D3 in
`docs/design.md` (which said "no decoder in process, hand off to mpv").

**Decision in one line:** the player lives inside Runefin, one window, on Wayland; frames arrive
as **DMA-BUF** and are imported into Rune's own Vulkan device; decoding is **GStreamer + VA-API**;
HDR is **tone-mapped to SDR** in our shader until wgpu can do HDR passthrough, which is an open
upstream issue, not a feature we can schedule.

---

## 1. Why the old plan does not apply

- **Wayland cannot embed another app's window.** mpv's `--wid` works on X11, Win32 and Cocoa and
  explicitly not on Wayland ("the Wayland devs don't like embedding to arbitrary windows" —
  [mpv #9654](https://github.com/mpv-player/mpv/issues/9654)). A separate mpv window was the only
  option on Wayland; you have ruled it out.
- **The obvious in-process path is CPU.** libmpv's render API has two backends that ship today:
  `MPV_RENDER_API_TYPE_SW` (CPU BGRA) and `MPV_RENDER_API_TYPE_OPENGL`. The `gpu-next` render
  backend that would let libmpv render through our own GPU API is
  [an open, unmerged PR](https://github.com/mpv-player/mpv/pull/16818) — OpenGL only, "designed
  to accommodate Vulkan later". Upstream Rune's PR #50 uses the SW backend: 4K60 BGRA through the
  CPU is ~2 GB/s of copying before you even decode, and it remarks that hw decoders "often won't
  feed" that path.
- **Rune does not expose its `wgpu::Device`.** `NativeSurface` accepts CPU bytes
  (`push_bytes(ByteFrame)`) or, on macOS only, an `IOSurface` (`push(macos::PixelBuffer)`). There is
  no way for an app to hand Rune a GPU texture it did not import itself — which is exactly what
  [Rune issue #23](https://github.com/anomalyco/rune/issues/23) filed: *"DMA-BUF import for
  NativeSurface: zero-copy frames from VA-API video decode … the way macOS imports
  CVPixelBuffer/IOSurface. wgpu needs Vulkan external-memory interop for this."* Open, low
  priority, untouched.

So the honest shape of the work: **we extend Rune once**, then the player is an ordinary consumer
of that surface.

---

## 2. The three architectures that actually exist

| | Decode | Frame path | 4K | HDR | Effort |
|---|---|---|---|---|---|
| **A. GStreamer + VA-API → DMA-BUF → Rune** | VAAPI (Intel/AMD), NVDEC via nvcodec | zero copy, `VK_EXT_external_memory_dma_buf` into Rune's device | yes, 4K60 10-bit | tone-map now, passthrough later | 3-4 weeks |
| B. libmpv render API (OpenGL) → GL texture → DMA-BUF export → Rune | VAAPI/NVDEC inside mpv | EGL context in-process, `EGL_MESA_image_dma_buf_export`, then import | yes | mpv's libplacebo tone mapping is the best in class | 4-6 weeks + two GPU APIs in one process |
| C. libmpv SW (what PR #50 does) | VAAPI won't feed it → software | CPU BGRA → `push_bytes` | no | no | days, but fails the requirement |
| D. mpv subprocess window | everything | compositor | yes | yes | today — rejected: two windows on Wayland |

**Recommendation: A.** It is the only one where GStreamer owns the parts that are not our
business (demux, audio clock, seek, subtitle timing) while frames never touch the CPU, and it sits
directly on the primitive Rune already wants for #23.

B is not wrong — mpv's tone mapping (libplacebo, Dolby Vision profile 5/8) is better than anything
we will write — but it means maintaining an EGL context next to a Vulkan one, and exporting our
own render target as a DMA-BUF just to import it again. Revisit if A's colour work disappoints.

---

## 3. What has to be built

### 3.1 Rune patch — `NativeSurface::push_dmabuf` (the go/no-go)

Public API in the shape the macOS one already has:

```rust
// crates/rune/src/native_surface/mod.rs
pub fn push_dmabuf(&self, frame: DmabufFrame, frame_id: u64) -> Result<(), String>;

pub struct DmabufFrame {
    pub width: u32, pub height: u32,
    pub fourcc: u32,          // DRM_FORMAT_NV12 / P010 / XRGB8888 …
    pub modifier: u64,        // DRM format modifier, required by Vulkan import
    pub planes: SmallVec<[Plane; 3]>,
    pub color: ColorInfo,     // BT.709/2020, PQ/HLG/SDR, range, primaries
    pub lease: DmaBufLease,   // drop → the producer may reuse the buffer
}
```

Implementation notes, all of which the existing code already has hooks for:

- Import with `VkExternalMemoryImageCreateInfo` (`DMA_BUF` handle type) and
  `VkMemoryDedicatedAllocateInfo`; `vkGetImageMemoryRequirements` gives the layout to ask for.
  `wgpu-hal`'s Vulkan backend already exposes `import_dmabuf_memory`
  ([docs](https://wgpu.rs/doc/wgpu_hal/vulkan/struct.Device.html)); the crate
  [`wgpu-external-frame`](https://github.com/water-rs/wgpu-external-frame) does this whole dance,
  including the producer lease and fence, for DMA-BUF and IOSurface — evaluate it before writing
  our own.
- Multi-planar: NV12/P010 arrive as one buffer with two planes; sample as `R8` + `Rg8` (or
  `R16` + `Rg16` for P010), convert
  YCbCr→RGB in the shader with the matrix from `ColorInfo` (never assume BT.709).
- Leases matter: the decoder pool needs the buffer back. Drop the imported texture's lease only
  after the submission that sampled it completes (`on_submitted_work_done`), or the frame you are
  about to draw is recycled underneath you — the classic tearing/looping artefact.
- Headless (`--snap`) must keep working: no dmabuf, no GPU import; the stills path can stay on CPU.

**Amended after slice 2 (2026-11-02).** The crate `wgpu-external-frame` 0.4 imports only packed
32-bit planes, and its importer needs the `wgpu::Adapter` to choose Vulkan vs EGL. Slice 2 therefore
ships **BGRA (`XR24`/`AR24`) first**: the crate copies the frame into a wgpu-owned texture on the
GPU, and the producer's lease is released from `on_submitted_work_done`. That removes the CPU copy
(the M5a acceptance bar) but is one blit more than the zero-copy path this section describes.
The two-plane path now imports NV12 as R8/R8G8 and P010 as R16/R16G16, each plane through the
vendored `wgpu-external-frame`; BGRA remains a fallback. Tone mapping is in the video shader.

### 3.2 Rune patch — the video shader

One extra fragment path in the existing uber-quad pipeline (kinds 1-8 today, add a video kind):
YCbCr→RGB with the right matrix and range, then **tone mapping** when `ColorInfo` says PQ/HLG.
Tone map to BT.1886 SDR with a filmic curve; expose two knobs (`peak nits`, `curve`) rather than a
tone-mapping panel. HDR10 metadata (`MaxCLL`/`MaxFALL`) can drive the curve when present.

This is where "looks good" is won or lost: a clipped sky and crushed shadows on a 4K HDR remux is
exactly the difference between "a client" and "a toy". Budget real time here.

### 3.3 Runefin — the pipeline

- `playbin3`/`uridecodebin3` → `vah264dec`/`vaav1dec`/explicit VA-API elements (fall back to
  software decode only when VAAPI is absent, and say so in the UI).
- `vaapipostproc` with `video/x-raw(memory:DMABuf),format=DMA_DRM` and an NV12 modifier → a small
  `BaseSink` named `runefin-dmabuf-sink`. It advertises `GstVideoMeta` in its allocation query,
  extracts each plane's fd/offset/stride without mapping, and calls
  `NativeSurface::push_dmabuf`. `appsink` cannot negotiate this: `vapostproc` refuses DMA-BUF caps
  unless the sink supports `GstVideoMeta`. Rune imports Y as R8 and UV as R8G8 (P010: R16/R16G16) using the vendored
  `wgpu-external-frame` extension, so the two-plane shader consumes both without a CPU copy.
  AR24/XR24 remains the fallback. P010 uses BT.2020/PQ/HLG decoding and a Reinhard tone map in
  the shader before writing SDR.
- Audio: GStreamer's own sink (PipeWire via `pipewiresink`, else PulseAudio/ALSA). **One clock**:
  GStreamer's pipeline clock is the master; we never time video ourselves.
- Shipped in the app: the DMA-BUF `BaseSink` feeds `NativeSurface::push_dmabuf` (zero copy,
  P010 first, NV12 and AR24/XR24 fallbacks), with pause/seek/stop, audio tracks, mute/volume,
  fit/fill, keyboard controls, Jellyfin reporting and automatic CPU NV12 + mpv fallbacks. A real
  in-app play stored **149,406 ms** in Continue Watching; the local HDR clip exercises the
  PQ/BT.2020 path.
  `gst-plugins-good` (Matroska/MP4 demuxers) is required at run time.
- Seek: `gst_element_seek_simple` with `FLUSH|ACCURATE`; scrub previews keep the current
  single-frame path. A flush seek makes the pipeline's clock report the pre-seek second for a
  frame or two, so the app holds the requested position until the clock reaches it (or four
  seconds pass); without that the bar snapped back and jumped forward again. Space toggles
  pause, ←/→ seek the configured step and Esc stops. The chrome is a top bar (Back, title,
  Fullscreen), three centred controls (back, play/pause, forward) and one timeline row: elapsed,
  remaining, queue position, volume, mute and the settings sheet. Everything else — audio,
  subtitles, quality, speed, fit, chapters, sleep, A–B, go-to, episode stepping, transfer,
  statistics and minimize — is a row in that sheet, each showing its current value. More than one
  audio stream opens a track menu listing MediaSources titles (`Japanese - OPUS - Stereo`); the
  row sets playbin's `current-audio` by ordinal, the same order the server lists them. Mute,
  ±10 % volume and Fit/Fill live in the sheet. Chapter starts from
  `/Items?fields=Chapters` draw as ticks above the bar. Hovering the bar asks the image worker
  for the one `/Videos/{id}/Trickplay/{width}/{tile}.jpg` sprite that covers the pointer and
  crops the matching cell; only that tile is retained, because a 10×10 640 px sprite decodes to
  roughly 92 MB. The preview is a 240×135 crop in a 300 pt panel. When the server has no
  trickplay for an item (some libraries never generate it), the same panel shows the time and
  says so rather than faking a still from elsewhere. The manifest rides on the playback item
  request, so a film started from a card previews too, not just one opened through Detail.
  Resting, the timeline is a quiet 4 pt rule; under the pointer it doubles to 8 pt and grows a
  knob, which is what makes scrubbing findable over a dark frame.
- Up next: an episode's next sibling in the same season is requested alongside playback
  (`parentId=seasonId`, episode index greater than the current one). The last 30 s draw the
  end card: the ending episode shrinks to a 340×191 corner at top-right while the next
  episode's still fills the page, dimmed, behind a bottom-left card — still, `S2E03` label,
  runtime, title, overview, a ceiling countdown and Play now / Watch credits / Dismiss. The
  card wakes the OSD and takes focus from the timeline. EOS rolls to that episode unless
  dismissed, reporting the finished one first; the last episode stops as before.
- Settings sheet: one sheet in the panel slot, grouped Tracks / Playback / Timers / Navigate /
  This device, each row showing its current value. A row that needs a list (audio, subtitles,
  quality, chapters, sleep, go-to) closes the sheet; `Esc` returns to the sheet, then to the
  picture, then stops. The sheet never hides a state: the sink path, track ordinals, speed, fit,
  quality and timer state are readable without opening a row.
- Statistics: the sheet's Statistics row (its value is the sink path, `CPU` / `DMA-BUF P010`)
  opens a diagnostics overlay
  with the sink path, container, video codec/resolution/range, active audio and subtitle track,
  speed, position/duration and frame count. `Esc` closes it. It is the same frame counter the
  performance probe uses.
- Player flags: playbin's `TEXT` flag is cleared by setting `flags` from the string
  `soft-colorbalance+deinterlace+soft-volume+audio+video`. This is not cosmetic: playsink's
  default subtitleoverlay rejects `VAMemory P010`, which silently forced every HEVC file onto
  the CPU sink and could crash inside `libgstassrender` during a flush seek. With TEXT off, HEVC
  and 4K HDR stay on the DMA-BUF path. The app's own SRT overlay is the only subtitle path.
- Rate and seeks: a rate is applied as a flush seek, tracked so repeating the same rate is a
  no-op; every seek uses the full `gst_element_seek` with the tracked rate, because
  `seek_simple` would silently drop back to 1×. Stopping waits for `NULL` before drop.
- Stalls: playbin's `BUFFERING` messages feed a percentage in the player. Below 100 % the OSD
  draws a `Buffering… 42 %` chip over the picture, independent of the chrome's idle timer: a
  frozen frame with no explanation is the worst possible state. A healthy direct stream reports
  nothing, so the chip never flashes on startup or a seek.
- Skip: a small pill sits bottom-right above the timeline only while a segment is offered —
  five seconds before it starts under `Ask`, never under `Never`, and `Always` seeks past with
  Undo instead of showing it. Its label is the segment's kind (`Skip intro`, `Skip credits`,
  `Skip recap`, otherwise `Skip preview`). The pill and `S` read the same offer, so the key works
  during the lead too.
- Scrub previews carry the chapter under the pointer above the thumbnail (`24:09 · The board`),
  so a long timeline says where a jump lands before it happens.
- Fullscreen: `F11` anywhere, or the top bar's `Fullscreen` / `Windowed` chip, asks the window
  manager for borderless fullscreen. This is a small Rune fork addition: `Tx::set_fullscreen`
  and `AppHandle::set_fullscreen` reach winit's `set_fullscreen`, remember the mode across a
  hide/show, and leave a panel, launcher or overlay alone.
- The pointer hides with the chrome and comes back on any move, like the OSD itself.
- Chapters: the seek bar carries a tick per chapter start. The sheet's Chapters row opens the
  list (`in-app-chapter-panel`): a still from the server's chapter image (when it has one), the
  name, the start time, the current chapter marked with ▶ and the row highlighted; clicking a
  row seeks and keeps the list open so stepping through a film is one click per chapter. `Esc`
  closes it. Chapter data rides on the item; stills are small (160 px) low-priority image
  requests, capped at sixteen per film so they never crowd out the poster.
- Queue: "Play all" on a library header and on a collection page starts a run; "Shuffle"
  shuffles the same set. The queue is what EOS follows (a run continues before the next-up
  rule), what `N`/`P` step through, and what the timeline row's `3 of 12` reads. That chip is a button:
  it opens the run's item list with the current entry marked, and a row jumps straight to it. Clicking a single card
  or a detail page ends the run on purpose; a manual stop does too. Only kinds the player can
  open directly are queued — a Series is a container, so its page still opens its first
  unplayed episode.
- Episode neighbours: at playback start one request reads the whole series in broadcast order
  (`parentId=<series>`, recursive) and picks the episode before and after the current one. That
  crosses a season boundary — S02E01's previous is S01's finale — and replaces the two calls the
  player used to make. The sheet's Next episode row stays the episode the up-next strip offers
  while the series has one; the row only exists when a neighbour does.
- Startup: the DMA-BUF attempt gets two seconds to produce a first frame. A file the VA-API
  converter cannot feed reaches PLAYING silently, and waiting the CPU path's six-second timeout
  would delay every such file.
- Text subtitles: the custom sink has no `textoverlay`, so the app draws text itself.
  `fields=MediaSources` lists the tracks; SRT/VTT are converted by the server to SRT, ASS is
  requested as ASS, and both parsers turn markup into timed cues. SRT/VTT inline `<b>`, `<i>`,
  `<font color>`, VTT `<c.colour>` classes and `{\anN}` alignment are preserved at cue level.
  The cue under the (delayed) clock is drawn in Rune above the controls. The sheet's Subtitles
  row opens
  the track list with delay (±100 ms, `[`/`]`) and size (−/+10%, `-`/`=`); `c` cycles off →
  tracks → off.
- Subtitle appearance (Settings → Playback, saved on exit): Sans / Serif / Mono, six text
  colours, Off / Thin / Thick outline (eight offset text shadows, since Rune has no stroke),
  Normal / Bold / Italic / Bold-italic, Bottom / Middle / Top for plain cues, text opacity and
  plate opacity. ASS inline colour/weight still wins inside an ASS cue so signs and karaoke keep
  their meaning; the reader's face, outline, plate and position apply to both.
- ASS tracks are requested as ASS (`format=ass`) and read by a second parser: each event keeps
  its alignment (numpad 1–9), an explicit `\pos` anchor, its line breaks, its font size as a
  fraction of the script height, its primary colour and opacity, and its bold/italic faces
  (style and inline `\c`, `\b`, `\i` overrides). All events covering the playhead are drawn,
  each in its own band, so a sign and a dialogue line can appear at once. The style's
  `Outline`/`Shadow`, their colours, and the inline `\bord`, `\shad`, `\3c`, `\4c`, `\frz`
  and `\fscx/\fscy` overrides are reproduced: the outline is a ring of offset glyph runs (a
  box shadow would draw the line's rectangle), and rotation/scaling turn the whole cue. Rune
  scales uniformly, so a non-uniform `\fscx`/`\fscy` pair keeps its area rather than its exact
  aspect; 3D `\frx`/`\fry` are ignored. SRT/VTT cues stay centred at the bottom.
- Image subtitles: PGS and VobSub are pictures. The server refuses to convert them to a file,
  so selecting one runs `ffmpeg -map 0:<stream>` against the same direct-play URL, caches the
  extracted `.sup`/`.mks` under `~/.cache/runefin/subtitles`, and decodes it in process with
  `libbitsub-core`. Only the active display set is rendered; its RGBA rectangles are placed over
  the video by Rune, lifted above the OSD when they sit in the bottom half. The video path stays
  zero-copy. DVB (and any future unsupported bitmap codec) remains a listed row with the online
  text-search action; the extraction takes seconds on a large file, so the panel says
  "Preparing image subtitles…".
- Online subtitles: "Search online…" in that panel opens in the language from Settings (or
  English when none is set), then asks `/Items/{id}/RemoteSearch/Subtitles/{lang}`
  for every configured provider (Open Subtitles on the live box), lists the results with the facts
  that decide between them (provider, downloads, rating, exact-file match, forced/SDH, format,
  uploader) and downloads the chosen one with `POST` on the same path. The server attaches it as an
  external track, the item is re-read, and the new stream index — the one that was not in the list
  before the download — becomes the active track. Six languages, two rows of chips; search and
  download run on `rune::Latest`, so neither blocks the frame loop. `Esc` steps back from the
  lookup to the track list, then closes the panel.
- The display stays awake while a video plays: the app takes a `systemd-inhibit`
  `idle:sleep` block lock by spawning the tool and kills it on every stop path (EOS, error, Esc,
  channel zap). No session bus or no systemd means the spawn fails and playback is unchanged; the
  inhibitor is a courtesy, never a requirement.
- Volume is a preference, not per-film state: the level you left is the level the next item
  starts at, saved with the rest on exit.
- Trailers use the same player: a local trailer is a normal server item with reporting off; a
  YouTube URL is downloaded by the optional `yt-dlp` tool into `~/.cache/runefin/trailers` and
  opened from `file://` with reporting off. Without `yt-dlp` the detail action opens the browser.
- Preferences (Settings → Playback, saved to `config.toml` on exit): autoplay next episode,
  seek step 5/10/30/60 s (arrow keys and the centred ± buttons use it), starting volume in 10 %
steps, subtitle mode
  off / forced only / always, subtitle background dim / solid / off, the full subtitle
  appearance set above, and a preferred subtitle and audio language. "Forced only" is the
  default because it never puts a full track in front of a viewer who did not ask; "Always" takes
  the preferred language and falls back to the first track; an audio preference is applied once,
  when the tracks arrive, and never overrides a manual choice. A missing key in an older config
  keeps the old behaviour rather than turning something off.
- The Jellyfin side is unchanged: `Videos/{id}/stream?static=true`, `/Sessions/Playing*` reporting,
  resume from `PlaybackPositionTicks`.

### 3.4 HDR, precisely

| Capability | Today | Blocked on |
|---|---|---|
| 10-bit decode (HEVC/AV1, BT.2020 PQ) | yes, VAAPI | nothing |
| Correct colour on an SDR display (tone map) | yes — our shader | nothing |
| HDR output to an HDR display (passthrough) | **no** | wgpu has no Wayland colour-management surface; open issue [wgpu #10545](https://github.com/gfx-rs/wgpu/issues/10545) (Oct 2026) + `wp_color_management_v1` + an HDR compositor (KDE Plasma 6) |
| Dolby Vision | no | needs libplacebo (path B) |

So: **4K yes, HDR content yes, HDR output not yet.** Phase M5c flips it on when wgpu exposes
`set_image_description`/the colour-management protocol; until then a setting that says exactly
that, rather than a switch that silently does nothing.

---

## 4. Milestones

| M | Scope | Done when |
|---|---|---|
| M5a | Rune patch: `push_dmabuf` + video kind, SDR only | a 1080p H.264 file plays in-window on Wayland with no CPU copy (`perf` shows no memcpy growth; GPU busy) |
| M5b | GStreamer pipeline in Runefin: seek, pause, tracks, audio clock, Jellyfin reporting | a 2-hour 4K HEVC 10-bit file plays at 60 fps on the iGPU, <0.1% dropped, seek <250 ms, resume position survives a restart |
| M5c | Tone mapping quality + HDR10 metadata | side-by-side with mpv gpu-next on the same clip; no clipped highlights, no banding |
| M5d | HDR passthrough | when wgpu lands #10545 and your compositor is KDE Plasma with HDR on |

**Fallback stays shipped:** the mpv handoff (D3 in `design.md`) remains behind a setting, so a
driver regression cannot brick playback. On Wayland that fallback is a separate window; it is the
emergency exit, not the design.

---

## 5. Effort and risks

Rough: 3.1 ≈ 5-8 days (the import + lease/fence correctness is most of it), 3.2 ≈ 3-5 (shader and
tone mapping), 3.3 ≈ 5-7, M5c ≈ 3-5. So **3-4 focused weeks** to M5b, assuming the Rune patch is
accepted upstream or carried as a two-file fork.

Risks, in order:

1. **Driver matrix.** Intel iGPU is the well-trodden path; AMD VCN needs recent Mesa; NVIDIA means
   `nvidia-vaapi-driver` (works, but dmabuf export is the flakiest corner) or NVDEC through
   GStreamer's `nvcodec`. This decides whether "4K60" is a claim or a hope — I need your GPU.
2. **Upstream divergence.** Rune moves ~100 commits/day. A two-file patch is cheap to carry; a
   large one is not. Keep the patch to `native_surface` + one shader file, and send it upstream as
   a PR early.
3. **The lease/fence bug class.** Reusing a frame the compositor still reads gives intermittent
   tearing that looks like a driver bug. Test with `GST_DEBUG=*:5` on a 3-buffer pool.
4. **Tone mapping is taste.** Expect two or three iterations against real HDR remuxes.
5. **This is the project's centre of gravity.** It turns a 3-week browse client into a 6-week
   media client. That is fine if it is what you want; it should be a conscious trade.

---

## 6. What I need from you

1. **GPU** (model): Intel iGPU, AMD, or NVIDIA? It decides the decode element and whether 4K60 is
   realistic.
2. **Compositor**: KDE Plasma 6 (HDR possible later) or GNOME/Sway (SDR only, tone map forever)?
3. Do we send the `NativeSurface::push_dmabuf` patch **upstream to Rune as a PR**, or carry it on a
   fork until it lands? I recommend upstream-first, fork-second, because #23 is exactly this and
   the maintainers filed it themselves.

---

## 7. Fork strategy (decided: we carry a fork)

Rune moves ~100 commits a day and we need one API it does not have, so the patch rides on a fork
rather than a vendored copy. The fork exists **only** to hold that patch; everything else must stay
identical to upstream, or the rebase becomes a job nobody does.

**Remotes and branches**

| Where | Name | Rule |
|---|---|---|
| [github.com/lifk/Runefin](https://github.com/lifk/Runefin) | — | the app (this repo) |
| `github.com/lifk/rune` | `origin` | our fork, live since 2026-11-02: `dev` mirrors upstream, `runefin/player` holds the patch; never commit directly to `dev` there either |
| `github.com/anomalyco/rune` | `upstream` | source of truth; `dev` is fast-forward-only upstream and we treat it the same |
| `dev` | mirror | fast-forwarded from `upstream/dev` only; no local commits, ever |
| `runefin/player` | our patch | rebased onto `dev`; the only branch with our commits |

**Scope.** The fork carries the DMA-BUF import, its shader hooks and the window adapter plumbing.
Keep the patch reviewable and upstream it when the seam stabilizes; drift is the real cost, not a
line count.

**Sync discipline** (`scripts/upstream-sync.sh` in the app repo, run weekly and before any Rune
work):

1. `git fetch upstream`, fast-forward `dev`.
2. `git rebase dev` on `runefin/player`.
3. `cargo check -p rune --all-targets` in the fork.
4. Print `git diff --stat dev...runefin/player` for review.

**Pinning.** Runefin pins a **commit of `runefin/player`**, never the branch: a branch in
`Cargo.toml` makes every `cargo build` depend on the network and on our mood. The bump is its own
commit, with the snapshot diffs, exactly like a normal Rune bump (`AGENTS.md` rule 7).

**Exit.** When upstream lands the DMA-BUF import (issue #23) or PR #50 becomes a GPU path, delete
`runefin/player`, repoint `Cargo.toml` at `anomalyco/rune`, and archive the fork. The fork is a
loan, not an asset.

---

## 8. Targets: laptop, Steam Machine, Samsung TV

The app has to be good on two machines and, eventually, real HDR on the TV.

**Laptop — AMD Strix Halo** (RDNA 3.5 iGPU, 40 CU, LPDDR5X). This is the same stack as the Steam
Machine: Mesa `radeonsi` + VCN for decode, DMA-BUF export through the Mesa VA-API driver. Two AMD
machines means **one decode path, one interop path, no NVIDIA branch in v1** — the abstraction can
stay thin (a `DmabufFrame` producer) instead of growing a backend per vendor. Strix Halo also has
the memory bandwidth that makes 4K60 HDR compositing a non-issue: the frames move as dmabufs, and
the iGPU never has to copy them.

The vendor-specific work is deferred, not designed away: if this ever runs on NVIDIA, it is
`nvidia-vaapi-driver` or GStreamer's `nvcodec`, behind the same `DmabufFrame` producer.

**Steam Machine** (AMD Zen 4 + RDNA3 28 CU, SteamOS/Arch). Good news: it is the *easy* platform —
VA-API on VCN decodes 4K HEVC, VP9 and AV1 in hardware, and 28 CUs have nothing to do but composite
one quad. Two sessions matter:
- **Desktop Mode**: KDE Plasma on Wayland. This is where we develop and where the player must be a
  normal window.
- **Game Mode**: gamescope. If Runefin ever runs there, it is a nested Wayland client under
  gamescope, which is also the only Linux compositor today with a working HDR path
  (`gamescope --hdr-enabled`).

**Samsung TV, real HDR.** This is the part that is not ours to schedule. For the TV to show its
"HDR" badge, the app's swapchain must be tagged HDR10 and carry mastering-display + MaxCLL/MaxFALL
metadata. On Linux that means the client speaks `wp_color_management_v1` (Wayland) or sets
`VK_COLOR_SPACE_HDR10_ST2084_EXT` (X11). **wgpu does neither today**; the work is tracked in
[wgpu #10545](https://github.com/gfx-rs/wgpu/issues/10545) (application-managed Wayland surface,
opened Oct 2026) and the older [#2920](https://github.com/gfx-rs/wgpu/issues/2920). KDE Plasma 6.2+
does its side, and gamescope already does — the client is the gap.

So the plan for the TV, in order:

1. **Now → M5c:** decode 10-bit, tone map to SDR well. On a good panel this already looks correct,
   just not "HDR". No dependency on anyone.
2. **M5d:** when wgpu exposes an HDR surface, flip a flag: skip tone mapping, write PQ/BT.2020, set
   metadata. The shader is written for this from day one (two output paths, one switch), so M5d is
   a day's work *if* the primitive exists.
3. **If wgpu stalls:** the escape hatch is a small Vulkan swapchain of our own via `wgpu-hal`'s
   escape hatches (Rune already does this for Metal) — painful, contained, and only worth it if the
   TV case becomes the main use.

HDR10+ (Samsung's dynamic-metadata variant) is out of scope: nothing on Linux signals it, and
HDR10 static metadata is what a player should output anyway.
