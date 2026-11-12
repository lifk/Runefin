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
| Home content ready | 100 + 355–580 + 73 = **530–750 ms** | **113–128 ms** (snapshot painted in the first frame), fresh rails at +355 |

The snapshot is `~/.cache/runefin/home-<hash>.json` (84–97 KB here): read in **3.6 ms** before
the window exists, painted with the first frame (343 glyphs, 40–56 ms update), then replaced when
the server answers. Cold (no file) the app behaves as before.

## Idle

While resting with nothing animating, the window **draws no frames** (0 over 13 s), and a covered
window draws none at all. Idle cost is a poll per wake, not a frame per display refresh.

## Not measured here

- **The player.** It needs a window and audio; its own meters cover it. Run
  `RUNEFIN_TRACE_PLAYBACK=1` (sink pacing every two seconds) with
  `RUNEFIN_LIVE_ROUTE=play RUNEFIN_LIVE_ITEM_ID=<id>` and `volume = 0.0` in the config; the DMA-BUF
  path was last measured at 24 fps with a 41 ms worst gap
  (`docs/research/14-player-renderer-options.md`).
- **A cold image cache.** All screen figures above run with the disk image cache warm, which is
  what a second launch sees.
- **Windowed interaction latency.** A covered window draws nothing on this compositor, so key →
  frame is measured in the app's own frame trace (`input_ms` in the `bench 1s` line) rather than
  here.

## What to do next, in order

1. **Build only the rails on screen.** 300 of Home's 490 glyphs are below the fold. Limiting the
   rail list to the viewport plus one, growing it on scroll and on focus, should cut the home
   paint from 73 to ~30 ms and its glyph rasterisation by two thirds. It touches `tv_focus` zones,
   the range tests and eight snapshots.
2. **Paint when Continue Watching answers, not when the last rail does.** 155 ms instead of
   355 ms to first content on a cold snapshot. Needs `home_loaded` to mean "enough to draw" and
   the rails to append as they land; the skeleton already matches final geometry.
3. **Skip Recently Played until it can be paid for.** It is the slowest single query (337 ms) and
   its rail is five screens down. Request it when the page nears it.
4. **Ask Rune for a way to preload the symbol fallback fonts.** The first frame that draws a `·`,
   `✓` or `—` loads 20 ms of DejaVu/Noto files inside the frame; the window spends 40 ms waiting
   for the GPU adapter just before, which would hide it.

Measured on 2026-11-12; re-run `scripts/bench.sh` after any change to these screens.
