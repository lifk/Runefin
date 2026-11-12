# Performance benchmarks — 2026-11-12

A real account on the home server, release build, 1280×800 and 1280×720, AMD Radeon
8060S (RADV/Vulkan) on Wayland. Method: `RUNEFIN_BENCH=1` (this pass's instrumented measurements —
launch marks, per-frame cost, per-request latency; see `docs/engineering.md`) and
`scripts/bench.sh`, which replay the numbers below. Every figure is a median of three or more runs
unless it says otherwise; the server is over the LAN, not localhost.

## What a launch costs

Process start to the first presented frame, windowed:

| | ms |
|---|---|
| Before this pass | 143.6 |
| After `prewarm_gpu()` first in `main` | 98.6 / 99.7 / 108.0 |
| Now, with the home snapshot loaded as well | 86.3 / 90.7 / 97.4 |

Where the remaining ~90 ms goes (one run, marks from Rune and the app):

```
bench.main          +0.0     process start
bench.config        +0.3     config.toml
bench.setup         +5.4     checkpoints, history, tray, MPRIS, image cache
window.created      +57.4    window, GPU adapter (+40.2), device, renderer (+52.6), surface
frame.first.build   +57.4
frame.first.presented +90.0  first frame built, encoded, presented
```

Two things dominate:

- **GPU setup, ~35–40 ms**, behind the first frame. Rune starts it once the event loop is being
  built; the app now starts it as the first statement in `main`, so it overlaps everything a
  launch does before the loop (config, state, window). That is the 43 ms the first row saves.
- **The first frame's own build**, 5–33 ms depending on what it paints (below).

The second launch of the same binary pays the same; there is no disk cache for shaders or fonts
here, and Rune's glyph masks survive only within a process.

## What a screen costs to paint

Headless live render, real data, 1280×800, one frame built from the server's answer:

| Screen | update | encode | total | layout nodes | new glyphs | text raster | text outline |
|---|---|---|---|---|---|---|---|
| Home | 58.0 | 14.8 | **72.8** | 1242 | 304 | 20.4 | 20.0 |
| Library (Movies) | 54.9 | 9.1 | **64.0** | 790 | 270 | 19.4 | 18.9 |
| Detail (film) | 46.2 | 13.8 | **60.0** | 191 | 340 | 21.8 | 21.3 |
| Search ("dune") | 46.1 | 8.7 | **54.8** | 327 | 230 | 14.8 | 14.3 |

`update` is poll, the app's own `view`, layout, text and paint; `encode` is uploads and draw
commands. Split by source: the app's `view` is **0.25–5.0 ms** — element construction is not the
problem. Text is: 29–43 ms of every first paint, rasterising 230–340 new glyphs at ~130 µs each
(one mask plus one outline record per glyph, Rune's Windfoil path), with layout and paint taking
the rest.

The glyph count is what varies between screens, and it is a function of how much text is built,
not of how much is visible: at 1280×800 Home shows about three of its nine rails.

## What the server costs

`bench net` lines, windowed, real account, requests fanned out by `home::load` and four sibling
`Latest` loads:

| Request | app (median) | curl, alone |
|---|---|---|
| `/Library/MediaFolders` (libraries) | 133–270 | 45 |
| Continue Watching (`/UserItems/Resume`) | 133–253 | 122 |
| One library's Recently Added (`/Items/Latest`) | 45–98 each | 59–93 |
| `/Shows/NextUp` | 193–214 | 127 |
| Recently Played (`/Items?Filters=IsPlayed`) | 337 | 154 |
| **`home.total`** (libraries, then resume and latest) | **355** (median of 5) | — |

Every concurrent request pays two to three times the latency it pays alone: the server queue is
the bottleneck, not the network. Continue Watching does not need the library list, so it now runs
beside it: 373 → 355 ms median, a small win that is within the jitter of this box.

## Home, end to end

| | Before | Now |
|---|---|---|
| First frame on screen | 100 ms (skeleton or restored route) | 90 ms, and with the snapshot: Home's rails |
| Home **content** on a warm install | 530–750 ms | **113–128 ms** (snapshot painted in the first frame) |
| Home **content** on a cold install | 530–750 ms | **~150 ms of state + one frame**: Continue Watching lands at 120–175 ms, the rails at ~300 ms |
| Recently Played | requested at launch, 337 ms, competing with the rails | requested after the rails land (147–160 ms, +460–550 ms), its rail five screens down |

The snapshot is `~/.cache/runefin/home-<hash>.json` (84–97 KB here): read in **3.6 ms** before
the window exists, painted with the first frame (343 glyphs, 40–56 ms update), then replaced when
the server answers. Cold, Continue Watching no longer waits for the library list or the rails, so
the first paint holds real content instead of a skeleton within ~200 ms of process start.

## Playback start

The same bench marks on the player path (`RUNEFIN_BENCH=1`, muted through `volume = 0.0`, a
1080p H.264 episode and an unplayed film, both direct-play over the LAN):

| | before | now |
|---|---|---|
| Play requested → pipeline PLAYING | 240 ms | 120–240 ms |
| Pipeline → first decoded picture | **1.65 s** (resumed) | **0.29–0.53 s** |
| Process start → picture | 2.29 s | **0.79–1.05 s** |
| Steady state | — | 22.8–25.0 fps, DMA-BUF P010, stalls 0–2 per 4 s, longest gap 45–73 ms |

The 1.65 s was the resume: the app waited for a picture at the file's beginning, then seeked and
waited again. `Player::start_at` queues the seek the moment the pipeline is PLAYING, so the frame
the wait sees is the resume frame; a seek the pipeline dropped is caught by a clock check and
retried once, which is exactly the old path. A transcode URL that already carries
`StartTimeTicks` is not seeked at all, as before.

Where the remaining ~300–430 ms goes, over three items (play requested → pipeline 80–134 ms;
pipeline setup 21 ms; preroll → first picture 214–267 ms):

- **80–134 ms** before the pipeline: the worker thread and the `/PlaybackInfo` negotiation, a
  server round trip. Asking for it when the detail page opens, instead of when Play is pressed,
  would take it off this path.
- **21 ms** building playbin, the audio filter, `vapostproc`, the capsfilter and the sink — too
  little to be worth warming.
- **214–267 ms** of preroll: the source's connection, demux, decoder start, the converter's first
  buffer. Not ours to shorten.

## Idle

While resting with nothing animating, the window **draws no frames** (0 over 13 s), and a covered
window draws none at all. Idle cost is a poll per wake, not a frame per display refresh.

## Not measured here

- **Playback *quality* on a TV.** The numbers above come from a headless render against the real
  server; the sink is the same, but a 4K HDR file on the TV's display is the other half, and it
  was last measured at 24 fps with a 41 ms worst gap
  (`docs/research/14-player-renderer-options.md`).
- **A cold image cache.** All screen figures above run with the disk image cache warm, which is
  what a second launch sees.
- **Windowed interaction latency.** A covered window draws nothing on this compositor, so key →
  frame is measured in the app's own frame trace (`input_ms` in the `bench 1s` line) rather than
  here.

## What to do next, in order

1. **Build only the rails on screen.** Rune already skips glyphs outside the viewport (a 400 pt
   tall window rasterises 257 glyphs where an 800 pt one rasterises 490), so this is about *layout
   and paint*: all nine rails are laid out (1259 nodes) and the offscreen ones cost ~10–12 ms of
   the 58 ms update, plus their share of the 219 MB resident. Limiting the rail list to the
   viewport plus one, growing on scroll and on focus, touches `tv_focus` zones, the reach tests
   and eight snapshots — a product decision as much as a performance one.
2. **A budget for poster art.** Home prefetches every card in every rail (by design: no empty
   boxes while scrolling), which is ~100 decoded 360 px posters and most of the 219 MB RSS. A cap
   or a smaller decode for rails far below the fold would trade a few grey boxes for ~100 MB on a
   TV box.
3. **Ask for `/PlaybackInfo` when a detail page opens.** It is a server round trip (80–134 ms)
   paid between pressing Play and the pipeline starting; the detail page already knows what it
   will play (`play_target`). A warm-up of GStreamer's elements, by contrast, saves only 21 ms.
4. **Ask Rune for a way to preload the symbol fallback fonts.** The first frame that draws a `·`,
   `✓` or `—` loads 20 ms of DejaVu/Noto files inside the frame; the window spends 40 ms waiting
   for the GPU adapter just before, which would hide it.

Measured on 2026-11-12; re-run `scripts/bench.sh` after any change to these screens.
