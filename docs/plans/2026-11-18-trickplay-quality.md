# Trickplay: quality, latency and sizing — 2026-11-18

Investigation into why the scrub preview sometimes waits, why the thumbnails are only “okay” on a
4K TV, and what can be done without guessing. Measurements are from the home server and this
machine; the server is 12.4.0.

**Verdict:** the client is asking the server for the right 640 set, but it fetches a whole 10×10
sprite only when the pointer first needs it, evicts the previous one immediately, and decodes
~88 MB for one 640×360 frame. The server has **no set larger than 640**, so a genuinely bigger,
sharper preview needs a server-side trickplay profile. The best profile keeps the sprite the same
size as today: **1280×720 thumbnails in 5×5 tiles** (6400×3600, like the current 640 in 10×10),
then the client loads that set for the TV. Client-side prefetch/debounce/fallback fixes the wait.

**2026-11-18: the client half shipped** — prefetch at playback/OSD, 140 ms tile debounce, loading
state instead of a blank card, stale-response drop, no placeholder decode for sprites, per-screen
set choice (TV 960+/desktop 480), adaptive 320/240 preview, and a 256 MiB decoded-image budget.
The server profile is still the only way to get sharper than 640; the client already takes 1280
when it appears.

---

## 1. How it works now

1. `detail.rs` requests `Fields=Chapters,Trickplay`; the item carries a `Trickplay` map:
   `mediaSourceId -> width -> TrickplayInfo { width, height, tileWidth, tileHeight,
   thumbnailCount, interval, bandwidth }`.
2. `Item::trickplay_manifest` picks **640** when present, else the widest set (`models.rs`).
3. On the first pointer move over the timeline, `ensure_trickplay_tile(item, ms)`:
   - computes the thumbnail index (`ms / interval`), the sprite (`thumbnail / 100`), and the cell;
   - requests `Videos/{id}/Trickplay/{width}/{tile}.jpg` at `ImagePriority::High`;
   - removes the previous sprite immediately, keeps only the current one.
4. The image worker fetches bytes (disk cache first), also decodes a 27×17 placeholder, then
   Rune's `ImageCache` decodes the JPEG, premultiplies, builds mips and uploads it.
5. `seek_preview` draws the **whole sprite** scaled by `240 / thumbnailWidth`, offset so the
   current 640×360 cell shows in a 240×135 window. The poster hover preview does the same at
   `CARD_W`; it waits 1.1 s before starting, cycles through the middle 76 % of the film, and keeps
   the previous frame until the next one is decoded.

The tile is 10×10 thumbnails at 10 s = **1000 s of film per sprite**. A 92-minute film has 6
tiles, a 2.5-hour film 9; every tile boundary is another first load.

## 2. Measured cost (home server, this host)

Server `TrickplayOptions` today: `WidthResolutions: [640]`, `TileWidth/Height: 10`, `Interval:
10000`, `JpegQuality: 90`, `Qscale: 4`, `ProcessThreads: 4`.

- A 16:9 film offers **only 640×360**; a 4:3 item offers 352×288. No 320, no 960, no 1280.
- One 640 sprite: **6400×3600**, **2.34 MB**, **223 ms** to download over the LAN.
- Rune release decode of that sprite (including mips): **~177 ms** on this machine. A debug build
  takes seconds; use release numbers.
- Decoded RGBA: **87.9 MB** before mips/GPU copy; mips add roughly a third.
- 556–893 thumbnails per film → 6–9 sprites; `Bandwidth` ~28.8 kbps averaged over the film,
  i.e. ~2.3 MB per 1000 s tile.
- So the first preview is roughly **0.4 s of network + decode** on a fast LAN before upload,
  frame wake and paint; Wi-Fi/Steam Machine can make it 1 s+. Moving to a new 16-minute tile pays
  it again.

## 3. Why it feels slow

- **No prefetch.** The first request starts only when the pointer moves onto the timeline (or the
  poster hover delay ends). Nothing loads while the player starts, so the first hover is always
  the worst.
- **The old tile is dropped before the new one is ready.** `ensure_trickplay_tile` removes the
  previous image immediately; the preview then has no image for the new key and shows the “No
  preview” card until the sprite arrives. On a tile boundary that is a visible empty panel.
- **No request debounce.** A fast drag across a 2-hour film can cross several tile boundaries and
  queue several 2.3 MB fetches/88 MB decodes; only the last key is retained, but the others still
  download/decode. Image responses are not generation-checked, so a late response can insert an
  image that was already forgotten.
- **Four image workers compete with posters.** The fetch queue has High/Low, but the trickplay
  request is High only when it happens; a first load can sit behind poster work.
- **The placeholder decode is wasted for trickplay.** The worker decodes every image to 27×17;
  for an 88 MB sprite the 64 MB limit rejects it, so it is only overhead (small, but pointless).
- **The disk cache is entry-counted (400), not byte-counted.** One trickplay sprite is 2.3 MB;
  a library page of posters is much smaller, so trickplay can evict ordinary art.

## 4. The quality ceiling

The preview is 240×135 pt. At the TV scale it is roughly 720×405 device pixels at 3× (4K design
height 720), 360×202 at 1.5× (1080p). The 640×360 source is:

- 1×/1.5×: downscaled or 1:1 — crisp.
- 3×: 720×405 from 640×360 — a 1.125× upscale; acceptable, not “high quality”.
- A 320×180 preview at 3× needs 960×540; the 640 source would be a 1.5× upscale — soft.

The server only has 640, so **no client change can create detail that is not in the JPEG**. A
larger preview without a larger set would just be a softer upscale. The client can choose the
preview size based on what the server actually has, but it cannot exceed it.

## 5. What to do

### A. Server profile — the quality unlock (recommended first)

Set `TrickplayOptions` to:

```xml
<TrickplayOptions>
  <Interval>10000</Interval>
  <WidthResolutions>
    <int>640</int>
    <int>1280</int>
  </WidthResolutions>
  <TileWidth>5</TileWidth>
  <TileHeight>5</TileHeight>
  <Qscale>4</Qscale>
  <JpegQuality>90</JpegQuality>
  <ProcessThreads>4</ProcessThreads>
</TrickplayOptions>
```

- 1280×720 thumbnails in a 5×5 tile = **6400×3600**, exactly the current 640 10×10 sprite's
  dimensions and memory. No client memory regression, four times the linear thumbnail detail.
- The 640 set in 5×5 becomes 3200×1800 (23 MB) — perfect for poster hover and 1080p.
- Each tile now covers only 250 s, so a 2-hour film has ~29 tiles; prefetch/debounce keep that
  invisible, and only one sprite is resident. Download per tile stays ~2–3 MB.
- Regenerate trickplay images after the change (Dashboard → Scheduled Tasks → “Generate Trickplay
  Images”, or the equivalent task). A `640`-only existing set is not upgraded automatically.
- If `JpegQuality`/`Qscale` is raised for even better stills, bytes grow but decode stays the
  same; 90/4 is already good.
- The client refuses a sprite larger than 32 megapixels: 1280 in 10×10 (368 MB) falls back to
  640, while 1280 in 5×5 is accepted. A misconfigured width cannot turn a scrub into a memory
  spike.

This is the only change that makes a 320×180 preview genuinely sharp on a 4K TV.

### B. Client quick wins — hide the wait (S–M)

1. **Prefetch the current tile** when playback starts (after the manifest is known) and when the
   OSD first opens, at a priority that beats posters for that one request. Do not evict it until a
   different tile is actually needed.
2. **Debounce tile changes** for ~120–200 ms while the pointer moves; issue one request when the
   pointer settles in a new sprite. Keep the previous cell/frame visible until the new one is
   decoded (hover already does this; the player preview does not).
3. **Show the time/chapter immediately** and a subtle loading state while a tile is pending; only
   say “No preview for this video” when the item has **no** trickplay manifest.
4. **Drop stale responses:** in `Msg::Image`, ignore a key no longer in `requested_images`; this
   prevents late tiles from re-entering the cache after eviction.
5. **Skip the placeholder decode for trickplay** (a flag on the image request); it cannot succeed
   above the 64 MB limit and the preview does not fade a poster.
6. **Choose the set per consumer:** 1280 for the player preview on a big screen when available;
   640 for the desktop player and poster hover. Never ask for 1280 in a 10×10 tile (368 MB RGBA) —
   the 5×5 profile keeps it at 88 MB.
7. **Adaptive preview size:** 320×180 on the big screen only when a 960/1280 set exists; otherwise
   keep 240×135. Desktop keeps 240.
8. **Diagnostics row:** available widths, interval, tile size, last fetch/decode time, cache hit;
   makes a slow server or a missing 1280 set visible instead of mysterious.

**Shipped 2026-11-19, against this 640-only server:** B1, B2, B3, B4, B5, B6, B7 and B8 (a
"Preview" row in the statistics sheet: every published width, the set in use, the tile geometry,
and whether the hovered tile is cached or still loading), plus the 32 MP sprite guard and the
byte-budget cache from C. B2 became **a loading card with the right time** rather than keeping the
previous sprite: holding it would keep two 88 MB decodes resident (176 MB at 640, and the same at
1280 5×5), which is the case C already calls out as too much for a Steam Machine. A pointer that
rests on a new tile sees the card for the ~0.4 s of fetch and decode; the rest of the drag is
covered by the debounce, and the playhead's own tile is prefetched.

### C. Cache and memory (S–M, links Q10)

- Move the image cache from 256 entries to a **byte budget** (LRU by decoded size). One 640
  sprite is 88 MB; 256 entries is not a meaningful bound for trickplay.
- Keep one current tile plus, if the budget allows, the previous tile for a back-scrub. With the
  1280 5×5 profile both are 88 MB; two is 176 MB, so one may be the honest default on a Steam
  Machine. The previous **cell** (not the whole sprite) is much cheaper if crop-on-decode lands.
- The disk cache should keep the full sprite bytes (2–3 MB) and trim by bytes, not 400 entries.
  **Done:** the cache now holds 400 entries **and** 512 MiB, oldest first; a sprite-heavy session
  can no longer squeeze out a library's posters by count alone.

### D. Crop-on-decode — only if larger tiles are required (M–L)

Rune already exposes `ImageCache::rgba(width, height, bytes)`. Runefin's image worker could
decode the sprite, crop the requested cell to RGBA, and hand only that to Rune; the full sprite
bytes stay on disk. This drops GPU memory from ~88 MB (plus mips) to ~1–4 MB per preview and makes
a 1280 **10×10** set (368 MB) unnecessary. It still costs the full JPEG decode per tile; keep the
decoded sprite for the current tile in the worker or cache a few cells to avoid re-decoding when
the pointer moves inside the same tile. This is the right next step only if the server profile
cannot use 5×5 tiles, or if a future 4K/8K preview wants more than 1280.

## 6. Recommended order

1. **Server profile** 640+1280, 5×5 (user action; no code) and regenerate.
2. **Client quick wins B1–B8** — prefetch, debounce, keep-previous, stale drop, skip placeholder,
   set choice, adaptive size, diagnostics.
3. **Byte-budget cache (C)** alongside Q10; this is the same memory work.
4. **Crop-on-decode (D)** only if 5×5/1280 proves insufficient.

## 7. Verification

- Measure first-preview latency with the OSD: time from pointer enter to the first frame, cold
  and warm; add bench marks around fetch/decode/paint.
- Confirm a 6-tile film crosses a tile boundary without an empty panel (keep-previous + skeleton).
- Confirm a fast drag across 30 minutes issues one request, not five.
- Confirm the 1280 5×5 sprite decodes at the same size/memory as today's 640 10×10 and the
  preview is 320×180 at 3× without visible softness.
- Check the disk cache does not evict posters after a long trickplay session.
- Tests are in place for the width choice per screen, 5×5 tile/cell math, the debounce (and the
  wake that makes it fire), stale-response drop, byte accounting and the 240/320 panel size
  (320 needs a 960+ set, 4:3 keeps its own height). The `in-app-trickplay` and
  `in-app-trickplay-loading` scenes freeze the cropped frame and the card at the same panel size.
  Still to measure on hardware: cold and warm first-preview latency, and a tile boundary on a
  6-tile film.

## 8. Deliberately not doing

- Requesting a width the server has not generated (Jellyfin serves only configured sets).
- Upscaling 640 to 320 pt and calling it “high quality”; it is a soft upscale.
- A second per-thumbnail endpoint or a custom server plugin; the sprite protocol is what Jellyfin
  provides.
- Fetching every tile for a film in advance: 6–29 × 2.3 MB and 88 MB decoded each is not a cache.
