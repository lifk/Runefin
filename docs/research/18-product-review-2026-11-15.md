# Product review — 2026-11-15

A full pass over the client as a product: every screen and feature, the visual contract, the
frame and launch budget, the Steam Machine path, and the open backlog. Five parallel audits ran
against the code and the committed scenes; every claim below was checked in the tree, not taken
from an older document. This file records the verdict, what changed in the same pass, and the
order the rest should take.

## Verdict

The architecture holds up. The browse app, the in-app player, Live TV, Seerr and the big-screen
focus router are real and covered; idle really draws no frames; the library is virtualized; the
API client is broad. The weaknesses are at the edges: **the process could not be quit cleanly on
a Steam Machine**, **the colour and focus tokens failed the accessibility contract in ways the
screenshots already showed**, **the Flatpak's tray and idle inhibitor were silently dead**, and
**Home still builds every rail for a viewport that shows three**.

Nothing found required a rewrite. Most of the felt improvement was token values, lifecycle
handling, and one toolbar.

## Shipped in this pass

### Steam Machine and desktop citizenship

- **Quit is a real path.** A SIGTERM/SIGINT handler (`signal-hook`, `main.rs`), a Settings →
  Account **Quit Runefin** row, and a tray **Quit** all run one idempotent `State::shutdown`:
  preferences saved, playback stopped and reported, idle inhibitor released. `on_quit` covers an
  app quit, where `on_close` never ran.
- **The pad cannot run away.** A disconnect clears the held direction and stick before the
  missing release can repeat forever; a failed `Gilrs::new` retries instead of killing the
  listener for the session.
- **The tray works.** Clicking it raises the window; **Show Runefin** and **Quit** joined the
  transport rows. The Flatpak now grants `org.kde.StatusNotifierWatcher`.
- **The idle inhibitor works in the sandbox.** The Freedesktop runtime has no `systemd-inhibit`
  binary (verified), so `IdleGuard` falls back to `org.freedesktop.login1.Inhibit` over the D-Bus
  stack already used by MPRIS, and the manifest grants logind on the system bus.
- **A stable app id** (`io.github.lifk.Runefin`) groups the window and its icon.
- **The window title follows the screen** and the playing episode.
- **HTTP requests time out** (30 s global, 5 s connect) instead of pinning a worker across a
  suspend.

### Visual contract

- **Secondary text passes AA**: `text_secondary` is L 0.80 in the dark theme (was 0.72, 3.27:1)
  and L 0.44 in the light one.
- **Primary ink is white on the state blue** (6.5:1, was 3.18:1): every primary button, selected
  nav pill and filled badge.
- **Focus is a near-white ring on both themes** (near-black on light), with the pointer wash under
  it on the big screen. The 5 % lift alone was invisible in the focus snapshot.
- **The watched badge no longer collides with EXTRA**, which moved into the top-right fact tray;
  selection remains bottom-left and accent-filled.
- **Resume cards protect their text**: a 38 % scrim at mid-height, readable-text shadows, 86 %
  subtitle ink.
- **The motion ladder is implemented**: 160 ms hover / 180 ms control / no bounce, replacing
  Rune's 500 ms, bounce-0.15 `SNAPPY` at 134 call sites.
- **Live TV chips name what they switch to**; the highlighted chip is the current mode, not the
  other one. The guide ruler shares the timeline's gap, so markers line up with blocks.
- **The library toolbar is two fixed rows**; the summary length and window width no longer decide
  where Grid/List wraps.
- **Home's heading loses its shadow when there is no art behind it**, and the skeleton uses the
  page gutter.
- **The spoiler guard now hides the next episode's title**, not just its still and overview.
- **The server tagline renders on detail.**
- **Music/photo/book libraries are no longer listed** in the sidebar: they opened a grid that
  could not play anything.

### Features

- **Coming up** on Home: the server's `/Shows/Upcoming` lookahead, deduplicated by series, with
  `Airs 4 Dec · S2E06` subtitles.
- **Last channel** on Live TV: the channel tuned last on this device is one click away after a
  restart.
- MPRIS stops calling `Path::exists` on every video frame; the poster URL is cached per item.

### Visual quality (second pass)

- **Detail loading is a real skeleton**: poster, title, metadata, score pills, action row, overview
  lines and one rail, matching the loaded page instead of a two-bar placeholder.
- **The filter sheet is a bounded panel** with its own surface, a Done control, and two semantic
  chip rows (source and availability). The last chip is no longer cut by the grid.
- **List rows draw a resume bar only when there is one**; Select and Grid/List disappear when the
  library is empty, and an active filter gets the right empty sentence.
- **The player gear** has 32 pt rows on desktop and 40 pt on TV, a 520 pt panel, chevrons on
  submenus, and a visible close button; MPRIS-style "value only" rows now say what they do.
- **The end card** puts the countdown in a pill beside Play now and draws a shrinking accent rail
  along the card's bottom edge.
- **The card menu** marks pickers with a chevron and removal actions in the danger tone.
- **Person pages** show Known for only when there are more than five library titles, and the rest
  as More in your library — the same cards no longer appear twice.
- **TV speech**: settings switches are 56×32, selects 40 high, steppers 40×44; the gear panel and
  close control scale with them.
- **Light-theme art**: white resume-card text keeps a black halo (the theme's white halo was
  erasing it), and the bottom scrim starts at 58 %.
- **Errors use the danger tone**, not the magenta accent, everywhere outside the player's segment
  colours.
- **Live-account check found an API bug**: `IsMissing`, `IsUnaired`, `HasSubtitles` and `Is4K` were
  sent inside `filters=`, which Jellyfin 12.4 accepts and ignores. The Movies header read
  “351 missing” on a healthy library. They are now `isMissing=true`, `isUnaired=true`,
  `hasSubtitles=true` and `is4K=true`; verified against the server (353 → 0 missing, 351 → 343
  with subtitles, 353 → 235 4K). This affected the Attention panel, the header counts and four
  filter chips.

### Documentation

`features.md` no longer claims a recordings browser, "virtualized" episode rows, or silent local
checkpoints; the test counts are current; `design.md` records the signal and logind dependencies.
This review is the new evidence file.

## Scorecard

| Area | State | Notes |
|---|---|---|
| Connect / session | Strong | Quick Connect, discovery, distinct errors; single server is a documented limit |
| Home | Strong, costly | First paint ~150 ms cold with Continue Watching; every rail still laid out |
| Libraries | Strong | Virtualized, filters, sort, bulk, saved views; toolbar now stable |
| Detail | Strong | Shared element, seasons, extras, collections, tagline; action row still dense |
| Player | Very strong | DMA-BUF, HDR tone map, subtitles, segments, queue, end card; no audio passthrough |
| Live TV | Good | Channels, guide, reminders, last channel; no recordings, now-line or search |
| Search | Good | Grouped, debounced, hints; no query syntax, no command palette |
| Desktop integration | Good now | Quit, tray, app id, MPRIS, idle inhibitor all honest |
| Steam Machine | Good now | Pad reconnect, quit path, Flatpak names; no OSK path, no refresh-rate switch |
| Accessibility | Strong | AT-SPI tree, slider roles, names enforced by test; focus contrast fixed |
| Performance | Good | Idle is free; Home rail build and image memory are the two big remaining costs |

## The next eight, in order

1. **Build only the rails on Home that are on screen** (P1). 1 356 layout nodes for ~3 visible
   rails, ~10–12 ms of the update, plus most of the image traffic. Library already has the
   virtualization pattern; Home needs a visible-range offset and growth on scroll/focus.
2. **Budget the image cache in bytes, not handles.** 256 handles range from 1.8 KB placeholders to
   7.6 MB decoded backdrops; a library page can double the 263 MB resident set. Add a decoded-byte
   budget and request smaller widths below the fold.
3. **Ask for the smallest trickplay tile set.** `trickplay_manifest` always takes the 640-wide
   set: a 6 400×3 600 sprite, ~92 MB decoded, for a 180 pt card or a 240×135 preview.
4. **Preload the symbol fallback fonts.** Measured 20 ms inside the first frame, ~24 % of a cold
   launch. Warm `·✓—` during setup, or get a Rune hook.
5. **Audio passthrough and output-device selection.** Every stream is decoded to PCM and every
   transcode is stereo, so the Atmos badge is a file fact on a TV that may never hear it. Add a
   passthrough setting, a device row and a 5.1 transcode profile.
6. **A controller-first text path.** Steam+X is documented but unverified for a non-Steam
   Flatpak; there is no in-app OSK, and first-run sign-in still assumes a keyboard unless Quick
   Connect is used. At minimum, detect and explain it.
7. **Live TV depth**: `/LiveTv/Recordings`, a now-line and jump-to-now in the guide, and program
   search. Recordings is the largest honest gap in a shipped Live TV section.
8. **Warm `/PlaybackInfo` when a detail page opens.** 80–134 ms sits between Play and the
   pipeline; the detail page already knows the target. Cache by item + source + quality.

After those: MPRIS `PropertiesChanged`, guide program popover, brief/compact mode and a real
always-on-top window, and the remaining spoiler-guard surfaces (episode thumbnails in the list).

## Backlog triage

Closed by this pass: **H4** (Coming up), **V3** (last channel), **X4** (tagline), **X7** (spoiler
title), **S7** (window title; breadcrumbs/mouse-back still open).

Still open and worth doing, in value order: **P1** Home rails, **U3** idle audit on hardware,
**Q2** next-episode stream prefetch, **V1** guide now-line and program popover, **T3** MPRIS
push, **V2** channel groups/search, **X5** episode depth, **T6** autostart/start-minimised,
**H2** "Because you watched", **X6** crew/by-year, **S1** command palette, **S11** saved servers
and certificate warning, **S12** deep links, **C5** year in review, **H8** "in collection" badge,
**S3** search syntax, **P10** compact mode, **T2** more notifications, **Q9** downloads.

Stale or blocked, do not schedule: **P8** screenshot (needs a CPU readback branch in the DMA-BUF
sink), **V5** timeshift (tuner unproven), **R5** notes, **A7** i18n (large, no audience yet),
**R3** Trakt (external account), **U5** caret blink (fork patch).

## Hardware-only checks

Steam+X under gamescope, pad hotplug and udev inside `--device=input`, suspend/resume of the
GStreamer pipeline and Vulkan swapchain, HDMI audio device loss, HDR/VRR negotiation, and whether
the KDE media widget reacts to the MPRIS property reads without `PropertiesChanged`. The code now
fails these safely; they still need a box.
