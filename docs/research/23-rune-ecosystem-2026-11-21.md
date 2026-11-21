# Rune ecosystem and upgrade assessment — 2026-11-21

Two questions: what the 143 upstream commits since our last merge (2026-11-17) offer the app, and
what the community has built with Rune that is worth borrowing. The upgrade was attempted first
because it changes the answer to the second: our pin is on a pre-split layout.

Everything below marked "verified" was read in the fetched upstream tree; anything else says so.

---

## 1. The upgrade is a crate split, not a bump

`git rev-list --count <pin>..upstream/dev` = **143 commits**, and `git merge` stops with **25
conflicted files** (43 hunks) because upstream split the workspace: `rune-text`, `rune-media`,
`rune-gpu`, `rune-core`, `rune-image`, `rune-ref`, `rune-taffy`, `rune-notify` and others are now
their own crates. Our fork's `crates/rune/src/native_surface` no longer exists; the code is in
`crates/rune-media/src/native_surface`.

Our delta since the last merge is small enough to port by hand: **647 insertions across 26 files**
(`git diff --stat 9198ed54..e9dcf1a2`), mostly `window.rs` (204), `window/create.rs` (86),
`runtime/paint/blend.rs` (52), `render/uber_wgsl.rs` (46), `native_surface/pipeline.rs` (72) and
`video.wgsl` (26). `git apply --3way` cannot follow the moved window files, so this is a re-home,
not a mechanical re-apply. The merge was aborted and the pin left at `e9dcf1a2`.

**Two things the split buys us** (verified): `unsafe_code` is denied workspace-wide with
`allow(unsafe_code)` isolated to leaf modules, and `clippy::undocumented_unsafe_blocks` is denied.
Our workspace already forbids `unsafe`, so this is upstream meeting us halfway.

## 2. What upstream added, in the areas we touch (all verified)

**Focus — the most useful cluster for a 10-foot UI.**

- `El::focus_ring_for(key, …)`: a keyboard-only ring drawn on one element while another (by key)
  has focus — for a scroll region's ring on an overlay above what covers its edges.
- `El::focus_visible_within_ring`: a ring around an element while focus is on it or inside it and
  *the keys* put it there — not for a click, not for handed-over focus. This is the behavior our
  TV walk wants from cards that take focus by pointer.
- Focus **Entry is preserved across commits** when focus stays on the same node.
- A roving group is **one Tab stop** for the rescue path; a control disabled while focused hands
  focus on, in a region too.

**Render and text.**

- `rune::hairline` / `El::hairline`: a CSS `scaleY(0.5)` divider. We draw 1 pt borders that land
  on fractions at 1.5× and 3×; this is the primitive for a true hairline.
- `El::on_pill_secondary` for text_input context menus.
- SVG: nesting depth bounded at 512, SMIL monochrome icons animate with partial damage; text:
  word-reveal nodes are copied once caught up.

**Runtime and input.**

- A **dense mounted-node table** bounded by tree size (perf; the old lookup was a map).
- `Tx::touch_tap()`: a press handler can tell a finger tap from a mouse click **on the same
  display**. A Steam Deck has both, which is exactly our case.
- A notched wheel **glides** on Chromium's curve (cubic-bezier(0.42, 0, 0.58, 1), 200 ms at 120 px
  down to 100 ms at 480), with `Runtime::set_wheel_glide(false)` to opt out; precise trackpads and
  macOS stay 1:1.
- **Loop-only frames**: when nothing reached the window and every loop on screen can be stepped,
  the frame skips `poll`, view, layout and paint. An app's `poll` **no longer runs on those
  frames** — timer work must ask with `Tx::wake_after`.
- `task::on_idle`: runs once per idle spell (10 s quiet, 180 s minimum gap, never without a hook)
  on a shared pool thread; pool threads park without a repeating timer.

**Media — the one that may delete code.**

- `NativeSurface::push_dmabuf` (Linux): a producer renders BGRA8 (`DRM_FORMAT_ARGB8888`) into
  linear single-plane dmabufs; Rune imports once on its Vulkan device, copies the first frame into
  its own texture, calls `on_read` from its own thread when the GPU is done, keeps at most 16
  buffers (512 MB), and enables `VULKAN_EXTERNAL_MEMORY_DMA_BUF`. Other formats and modifiers are
  refused. Our fork vendors `wgpu-external-frame` (429 files in `vendor/`) and has its own
  `native_surface/dmabuf.rs` for the same job; if the BGRA path matches VA-API's
  `vaapipostproc` export, the upgrade can **delete the vendor crate and that patch**, which is the
  largest single piece of our divergence.

**Tooltips and diagnostics.** `El::help_open` holds a tooltip up whatever the pointer does;
`HelpLook::on_focus` opens it on keyboard focus; the OpenCode look waits 600 ms.

## 3. What the community is building (awesome-rune, 2026-11-21)

- **Flicks** — the closest peer: a Plex alternative, servers in Rust, every UI in Rune, its own
  MKV/MP4/TS readers and decoders, HDR test clips, and a documented perf method. Worth reading for
  player/seek/resume patterns and for how they write measured numbers. Their own roadmap still
  lists fullscreen as "needs Rune", which we already have through `Tx::set_fullscreen`.
- **Wholeseek** — "play before you download": the player streams into a cache while the file
  arrives, and only a deliberate **Keep** moves it into the downloads folder; album playback
  prefetches the next track. That is a cleaner model for **Q9** than download-then-watch.
- **vidshow** — two windows: a control window and a borderless presenter window on a chosen
  display. A pattern for **P10** (compact/always-on-top/PiP), which otherwise needs Rune work.
- **Nightjar** (rune-fx shader visualizers), **Hokum** (anomalyco's Wayland control centre),
  **Moonframe** (app factory with a Rune renderer), **runex** (TypeScript bindings),
  **rune-material-shapes**, **rune-kit/knobs** (live tuning) and **rune-light** (SDF shadows, GI).
- Our own listing is stale: awesome-rune still shows `v0.1.38`. A `projects.json` PR after
  people start using 0.2.x is a cheap, honest touch.

## 4. Mapped to our backlog

| Item | Upstream pattern | What it changes |
|---|---|---|
| **P1** Home rails | dense node table; `RUNE_REGION_MISS` | Fewer per-update allocations, and a real measurement of which screens rebuild |
| **P1/Q10** image traffic | decoders run on shared task threads | Our decoder budget interacts with the shared pool; re-measure the image cache |
| TV focus work | `focus_visible_within_ring`, `focus_ring_for`, Entry preserved | Rings that follow the keys, not the pointer; fewer full-tree focus rebuilds |
| Dividers | `El::hairline` | True 0.5 px rules at 1.5×/3×, replacing 1 pt borders |
| Steam Deck input | `Tx::touch_tap`, wheel glide, `set_wheel_glide(false)` | Touch and wheel can behave differently on the same screen; wheel feel matches Chromium |
| OSD, trickplay timers | loop-only frames, `Tx::wake_after` | Audit: any `poll`-driven timer now needs a wake, or it stops during a spinner |
| background tidying | `task::on_idle` | Image disk-cache trim and stale `Latest` drops without a repeating timer |
| player video | `push_dmabuf` | Likely replaces our vendored dmabuf stack (BGRA linear only — confirm NV12 is unused) |
| **Q9** downloads | Wholeseek's stream-to-cache + Keep | Play while downloading, promote on Keep; fewer half-downloads |
| **P10** compact/PiP | vidshow's second window | A real always-on-top presenter window may not need Rune work |
| Tooltips | `help_open`, focus-open, 600 ms | "Copied", pad hints, and the timeline's chapter note |

## 5. Recommended order

1. **Port the HDR patch onto the split** (one change, one review): merge upstream with the moved
   files taken from theirs, then re-home `window.rs`/`host/*` surface selection, `render/variant`,
   `render/upload`, `runtime/frame`, `paint/blend` and the `uber_wgsl` variant. Render snapshots;
   they must be identical. Verify HDR on the RADV/Wayland box again.
2. **Replace the vendored dmabuf** with `push_dmabuf` only if VA-API exports BGRA linear on the
   Steam Machine; otherwise keep ours for NV12 and say why in `docs/design-player.md`.
3. **Take the focus and hairline primitives** in the next UI change (P1 is a natural host).
4. **Re-audit `poll` timers** after the loop-only-frame change: trickplay already uses
   `wake_after`; OSD auto-hide and the still-watching countdown need checking.
5. Then **Q9**, modelled on Wholeseek's stream-and-keep rather than a plain download queue.

**Landed 2026-11-21.** Steps 1–2 are in: the fork's `clean/2026-11-17` is now upstream/dev
(143 commits, 25 crates) plus the HDR window and video paths, the 10-foot input and scale, the
injected keys and `focused_key`; the app is ported onto it (`7ceb1c5c`, merged to main) and
`scripts/check.sh` passes with every committed view rendering byte-identical. The vendored
`wgpu-external-frame` stays out: the app's sink now pushes upstream `Dmabuf` frames, with the
HDR colour carried by our `rune-media` extension. Steps 3–5 (focus and hairline primitives,
the `poll`-timer audit, Q9) are the next work.

Two upstream bugs surfaced only for external builds and are fixed on the branch: `rune-text`
asked crates.io `parley_engine` for a feature only Rune's vendored copy has, and the Wayland
corner rounding called into the vendored winit (now behind `wayland-csd-corners`).

**P1 landed 2026-11-21.** Home's rails are now `El::virtual_rows` over a `HomeRail` descriptor
list (`944ffe0` carried the focus and hairline primitives; the rails came next). The first
attempt changed nothing: Rune mounts two viewports of rows either side of the viewport, and a
Home rail is over half a viewport tall, so with seven rails the window still covered the page.
The fork gets `El::virtual_overdraw(viewports)` (`83cda583`, default two, clamped 0–8) and Home
asks for one: the next rail coming is all a reader can see. Measured on the demo server at
1280×800/2×: **450 → 354 nodes** (21%), glyphs unchanged at 411, and the committed snapshots are
byte-identical. What the reader can reach is tested rather than assumed — Down from the first
card now walks to the last rail, which mounts as the page scrolls, and the shell test scrolls to
`rail-collections` and finds its cards.

The other half of Q10 (image traffic) is untouched: posters are requested by the rails in the
window, so a server with thirty libraries no longer builds thirty rails but still fetches art for
the next two ahead. The image-cache byte budget is still the open item.

**Q9 landed 2026-11-21, first slice.** Downloads are in: the action on a film's or episode's
detail page, a Downloads page under Tools, a Settings folder and budget, resumable `.part` files
with an atomic manifest, Keep against eviction, and playback through the in-app player from
`file://` with no server at all. The contract is `design.md` §3 (Downloads).

This is deliberately **not** Wholeseek's stream-to-cache yet. Their player fetches the stream
itself, so it can write what it plays; ours hands the URL to GStreamer, and teeing a demuxed
pipeline gives back elementary streams, not the container the server stored. Recording while
playing would need a local proxy in front of the file — a new moving part on the playback path,
and not one to add for a feature's first slice. What landed is the storage, the queue and the
Keep flag that model needs; automatic download-on-play (unkept, promoted by Keep) is the next
step, and it needs no proxy: two connections to a LAN server are cheap, and the copy is ready
before the credits.

Still open from the Phase 3 plan: automatic download-on-play with Keep promotion, "download the
next 3 unwatched episodes", offline browsing of Home/Library/Detail (Q4), flushing playback
reports queued while the server was away, and posters for rows whose image cache is cold.

**Downloads at a transcoded size, probed live (2026-11-21).** Jellyfin gives a client two ways to
ask for a smaller copy, and the DeviceProfile picks which:

- `TranscodingProfile` with `Protocol: hls` → the answer's `TranscodingUrl` is
  `/videos/{id}/master.m3u8`, whose child is `live.m3u8`: a sliding playlist of segments. A
  downloader would have to walk the playlist and concatenate segments, and a pause throws the
  session away.
- `TranscodingProfile` with `Protocol: http`, container `mp4` → the answer's `TranscodingUrl` is
  `/videos/{id}/stream.mp4?static=false&…`: **one progressive MP4**. That is the one the
  download worker can write unchanged, and it is what Runefin asks for. A direct-play or
  direct-stream capability in the same profile would let the server answer with the original
  file, so the download request turns both off: a cap must mean a smaller copy.

Verified on the live server (12.4.0): a 1.5 Mbps cap with that profile returns
`VideoBitrate=1308000&AudioBitrate=192000&TranscodeReasons=ContainerBitrateExceedsLimit` and the
progressive URL. What could **not** be verified is the bytes: this server has
`HardwareAccelerationType: qsv` while the box has no Intel GPU, so every transcode request — HLS
variant or progressive — answers `500 Error processing request` about 150 ms in. Switching the
encoding settings to `vaapi` (the GPU that runs the app) or `none` should make both paths work;
until then the download fails with the server's status, and the app says so.
