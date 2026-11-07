# Moonfin on Linux desktop — design cues, Jellyfin capability map, and the next Runefin pass

Date: 2026-11-03. Scope: **Moonfin-Core 3.0.0 on Linux desktop only**. This is a teardown of the
desktop client, not the TV, mobile, web, Emby, Plex or Silo products. It separates what Jellyfin
itself provides from what Moonfin adds through its own Moonbase plugin, and turns the useful parts
into a Runefin roadmap.

**Bottom line:** Moonfin's best desktop ideas are not its hundreds of settings. They are four
things Runefin can adopt without becoming a different product:

1. **Artwork before chrome.** Near-black surfaces, translucent navigation, item logos and
   backdrops carry the visual identity; the UI itself stays quiet.
2. **A real desktop player surface.** A layered OSD with buffered progress, trickplay, chapter
   marks, track sheets and a mini player. This is where a media client feels finished.
3. **A compact, overlay rail.** Moonfin's left navigation is a 72 px rail that expands to 280 px
   over the content on hover or focus; it never squeezes the library.
4. **Desktop citizenship.** Native title bar, saved window geometry with work-area correction,
   single instance, `.desktop` integration and a shortcut sheet.

The parts to leave behind are the **8 detail styles, 6 media-bar layouts, 4 themes, theme store,
seasonal effects, games, books, multi-server aggregation, Plex/Emby/Silo and the plugin ecosystem**.
Those are Moonfin's product; they are not Runefin's edge.

> **Licence guardrail.** Moonfin-Core is GPL-2.0; Runefin is MIT. This document records behaviour,
> measurements and public design cues only. Do not copy Moonfin source, shaders, assets or strings
> into Runefin without a deliberate licence decision.

---

## 1. What Moonfin actually is on Linux

Moonfin-Core is a Flutter application in a GTK 3 shell. Linux is one of its shared
"TV and desktop" layouts, not a separate native desktop UI. The important facts, taken from source
at commit `d4430f66bc7a08f470ef09c41bc17668a0077957` (the 3.0.0 tree):

| Area | Moonfin 3.0 on Linux | Why it matters to Runefin |
|---|---|---|
| Window | GTK 3, default **1280×720**, minimum **800×500**, native title bar, no custom chrome | Runefin should trust the compositor's title bar as well |
| Persistence | Saves width, height, x, y, maximised and fullscreen; clamps restored bounds to the nearest work area | Our window currently opens at the default size every time |
| Instancing | Single instance; second launch raises the window; `moonfin://` links are forwarded | A professional desktop app does not spawn duplicate windows |
| Packaging | AppImage, deb, rpm, Flatpak, snap, tarball; AppImage writes its own `.desktop` entry, icon and `StartupWMClass` | Runefin has source builds only |
| Rendering | Flutter + Skia (Impeller is explicitly disabled on Linux), optional glass themes | The visual effects are Flutter-side, but the composition rules transfer |
| Playback | `media_kit` / libmpv on Linux, texture-backed | Runefin's GStreamer/DMA-BUF player is a different engine; visual cues still apply |
| Default navigation | **Top navigation**; a left sidebar is an option, not the default | Runefin already chose a sidebar; keep it, but improve its rail behaviour |
| Default home | Featured media bar enabled, **full viewport height** on desktop, auto-advance every 10 s | This is the biggest philosophical difference; see §5 |
| Default detail | `modern`, the responsive cinematic two-pane layout | The right reference for Runefin's one adaptive detail layout |
| Themes | Moonfin, Neon Pulse, Glass, 8-bit Hero; custom themes and server sync | Do not copy the theme marketplace |
| Feature breadth | Jellyfin, Emby, Plex, Silo, multi-server, games, books, music, Live TV, DVR, downloads, admin | Breadth is Moonfin's moat and its complexity cost |

**Method.** Source audit of the 3.0.0 tree, its Linux runner and design package; the 3.0.0 release
notes; the public screenshots and wiki. No signed-in Linux session was run for this note, so this
is not a frame-time or codec-playback benchmark. The source is the better evidence for shell
behaviour and spacing.

The repository's screenshots are shared TV/desktop marketing compositions, not proof of Linux
window chrome.

### Source map

- Linux shell: `linux/runner/my_application.cc` — GTK window, 1280×720 default, single instance,
  AppImage desktop entry, icon installation, Flatpak app-ID handling.
- Window model: `lib/main.dart` and `lib/util/window_geometry.dart` — saved geometry, minimum
  800×500, work-area fitting.
- Top navigation: `lib/ui/widgets/top_toolbar.dart` — 80 px desktop toolbar, centred translucent
  pill, 40 px avatar, clock.
- Left navigation: `lib/ui/widgets/left_sidebar.dart` — 72 px collapsed / 280 px expanded,
  200 ms transition, labels appear 100 ms after expansion, expands over content.
- Home: `lib/ui/screens/home/home_screen.dart` — full-height media bar on desktop, backdrop
  styles, row focus, peek-home-row logic.
- Media bar: `lib/ui/widgets/media_bar.dart` and `lib/ui/widgets/mediabar/` — six layouts,
  auto-advance, trailer previews, ambient art, metadata overlay.
- Cards: `lib/ui/widgets/media_card.dart` — 8 px radius, 3 px focus ring with 5 px inset,
  1.05 desktop expansion, hover/focus parity.
- Player: `lib/ui/screens/playback/video_player_screen.dart` — trickplay, chapters, skip segments,
  next-up, track sheets, OSD layering.
- Mini player: `lib/ui/widgets/video_mini_player.dart` — 96 px wide bar, 64 px compact bar,
  progress strip, transport, tracks, volume, restore.
- Shortcuts: `lib/playback/player_key_bindings.dart` and
  `lib/ui/widgets/keyboard_shortcuts/keyboard_shortcut_reference.dart` — one key table shared by
  the handler and the reference sheet.
- Design tokens: `packages/design/lib/src/theme/` — `app_colors.dart`, `theme_spec.dart`,
  `themes/moonfin_theme_spec.dart`, `app_shapes.dart`, `app_typography.dart`.
- Feature claims: Moonfin 3.0.0 release notes, `README.md`, and the wiki pages
  [Features](https://github.com/Moonfin-Client/Moonfin-Core/wiki/Features) and
  [Playback and Codecs](https://github.com/Moonfin-Client/Moonfin-Core/wiki/Playback-and-Codecs).

---

## 2. The design language, measured

### 2.1 The palette is mostly black, not navy

Moonfin's default theme is neutral and deliberately unglamorous:

| Token | Value | Use |
|---|---|---|
| Background | `#101010` | page base |
| Surface | `#1A1A1A` | cards and panels |
| Card fallback | `#202020` | when an image is absent |
| Accent | `#00A4DC` | focus, progress, play, selected state |
| Secondary | `#AA5CC3` | occasional emphasis |
| Body text | `#FFFFFF` | headings and primary copy |
| Secondary text | `#999999` | metadata |
| Card radius | 8 px | poster and episode cards |
| Chip radius | 999 px | pills |
| Focus ring | 3 px, cyan, 5 px outside the art | keyboard and selected states |

The marketing site uses a deep navy field. The **app** does not: the colour in the product comes
from the artwork. That is the important cue. A blue-grey wash over the whole UI makes posters
muddy; a near-black shell lets the backdrop and logo do the work.

Runefin's recent mockup pass moved the shell toward navy. That is a valid brand direction, but it
should be a measured tint, not a filter over the content. A good rule for the next pass:

- Keep a cool tint at **2–4% maximum** in the background and card surfaces.
- Never tint text, poster images or backdrops.
- Let the focused item's backdrop supply the colour temperature, the way Moonfin's home does.
- Test against warm skin tones and white titles before increasing the tint.

### 2.2 The detail is cinematic; the chrome is quiet

Moonfin's strongest visual pattern is a three-layer page:

1. **Backdrop** — a full-bleed still, often with a slow ambient drift or a soft blur.
2. **Vignette** — a vertical dark gradient plus a horizontal gradient toward the text side.
3. **Content** — logo or title, one metadata line, short overview, one primary action, then a
   low-contrast surface for the next section.

There are no thick borders, no bright panel backgrounds and no permanent toolbar sitting on top
of the art. The logo is the title whenever the server has one. The image's own contrast creates the
edge; the UI does not draw a box around it.

This maps directly onto Runefin's existing two-axis detail scrim. The missing piece is **logo-first
titling**: Jellyfin exposes a `Logo` image kind, Runefin's API already models it, and the app never
requests it. A logo is a much stronger identity than the current split-colour "Continue watching"
headline and costs one image request with a text fallback.

### 2.3 The top pill is a floating layer, not a bar

Moonfin's default top navigation is an 80 px transparent region with a centred, translucent pill:

- 36 px pill radius.
- Default translucent grey at 50% opacity; the image beneath shows through.
- 40 px avatar on the left; home/search/libraries/etc. in the centre; clock on the right.
- The content scrolls under it. The pill is a layer, not a layout band.
- On TV, the toolbar can translate away with scroll; on desktop it stays put.

Runefin does not need to switch to a top bar. The cue worth taking is **floating chrome over
content**, not another fixed horizontal strip. The sidebar already has the right idea with its
gradient; the missing part is allowing artwork to reach the window edge behind the content
instead of starting every screen with an opaque header block.

### 2.4 The rail expands over the library, not into it

Moonfin's left navigation is a 72 px icon rail. On desktop it expands to 280 px on pointer enter or
keyboard focus, animates in 200 ms, and reveals labels 100 ms later. The content does **not**
reflow when it opens; the rail is an overlay.

This is better than a permanent 224 px sidebar for large libraries on a 1280 px screen because the
poster grid never jumps columns when the user opens navigation. It has a cost: the open rail covers
the left edge of the grid. Moonfin accepts that trade because the rail is transient and the user
can move the pointer away.

For Runefin:

- Keep the **persistent 224 px sidebar as the default**; it is part of the app's identity and the
  design doc's desktop-first model.
- Add a **72 px compact rail at narrower widths**, with an overlay expansion on hover/focus and a
  pin control. Do not resize the content while it expands.
- Keep the current icon catalog; Moonfin's Material icons are less distinctive than Runefin's SVG
  set.
- Preserve the user chip and server status; those are useful desktop facts Moonfin hides behind an
  avatar menu.

### 2.5 Cards are art plus a state, not boxes

Moonfin's `MediaCard` is deliberately simple:

- 8 px radius, no permanent border.
- Poster fills the card; title and subtitle sit below it.
- Hover and focus both produce a 3 px ring with a 5 px gap and a 1.05 scale on desktop.
- Focused cards can grow into a wider "Modern" card with ratings and a three-line overview.
- The focused card is pulled to the leading edge of a rail; the row scrolls, the layout does not
  push siblings around.
- A missing image gets a gradient, the title set in uppercase and wide letter-spacing.

Runefin already has the right card geometry and a better image pipeline. The gaps are state and
typographic restraint:

- Current cards carry a 1 px border and a permanent drop shadow; Moonfin's default is borderless.
  A thin border is acceptable in a navy shell, but it should be quieter than the artwork.
- Our hover state is a 5.5% white overlay. Moonfin's is a ring and a scale. Add an **elevation
  shift** (a crisper shadow and a 1–2% lift) and let the focus ring be the real signal.
- Do not add a 1.05 scale to a virtualized grid: it clips at the viewport edge and makes row
  heights unstable. Reserve scale for the CW rail and pointer hover, or use a 1.01 lift.
- Keep Runefin's 2 px white focus ring; it has better contrast on bright posters than Moonfin's
  cyan ring. Add a 4 px outside offset and a 2 px dark inner line where the art is bright.

### 2.6 Type is dense, not enormous

Moonfin's UI type scale is small for a "cinematic" app:

`32 / 28 / 24 / 20 / 18 / 16 / 14 / 12`, body 16, metadata 14, labels 12–14. The marketing images
use 64 px headlines, but the product does not. Large type is reserved for the media bar and the
detail title; ordinary rows stay compact.

Runefin's scale is already close and slightly more generous: display 44, page 32, section 20,
body 16. Keep it. The improvement is not a larger title; it is **more consistent line lengths,
more space between sections and fewer competing labels**.

Recommended limits for the upgrade:

- Detail overview: 720 px measure, 15/23, four lines collapsed.
- Home section titles: 20/26, secondary colour, one line.
- Card metadata: 14/20, one line, ellipsis.
- Player timecodes: 12/16, monospace, never below 12.
- Hero logo: max 320×110, `contain`; fall back to the 32/38 page title.

### 2.7 Motion is short and purposeful

Moonfin's desktop rail is 200 ms; page transitions are 150/300/450 ms depending on preference;
the default card transition is a short ease. The media bar's auto-advance is 10 s. Seasonal effects
respect the system reduce-motion setting; the rest of the app does not have a strong global
reduced-motion story.

Runefin's design already defines a good ladder: 160/120 ms hover, 220 ms rail settle, 320/240 ms
detail, 180 ms page, 120/200 ms OSD. Keep that. Add:

- A **reduced-motion setting** that turns scale, shared-element travel and backdrop drift into
  opacity-only transitions.
- No auto-advance in the default home. If a spotlight is added, pause on hover/focus, stop after
  one cycle, and never animate under reduced motion.
- No motion on more than one axis at once; a card should not scale and lift and glow in the same
  frame.

### 2.8 The player is the real product

The playback screenshot is the most useful artefact in the repository. The anatomy:

- Top bar: back, title/logo, path or quality badge.
- Picture: full-bleed, black letterbox.
- Bottom stack: a **filmstrip** of trickplay frames above the seek bar, then the bar, then a
  transport row.
- Seek bar: buffered range, played range, chapter ticks, scrub thumb.
- Time labels: elapsed and remaining; optional wall-clock end time.
- Panel: a subtitle or audio sheet that overlays the right side, with track names, codec, language,
  default/forced flags, delay controls and a download option.
- Secondary actions: speed, favourite, next episode, audio, subtitles, volume, PiP, info.
- Keyboard: Space/K play/pause, arrows/J/L seek, Up/Down volume, M mute, C subtitles, S skip,
  Shift+N/P next/previous, F fullscreen, I info, `?`/F1 shortcut sheet, Esc/Backspace layer-aware.
- Backspace/Delete hides the Next Up card, then an open panel, then the controls — one layer at a
  time.

Runefin's current player is a **functional toolbar**: title, elapsed/remaining, a 4 px progress
line, seek/play/audio/fit/volume/stop, and a path hint. It proves the hard part — zero-copy frames,
audio clock, seek and reporting — but it does not yet feel like a player. The biggest visual and
functional gap in the whole app is here.

### 2.9 The mini player is the desktop differentiator

Moonfin's 3.0 mini player keeps video playing while the user browses:

- A bar along the bottom, outside the navigator, so it survives route changes.
- 96 px wide layout, 64 px compact layout below 720 px.
- 80 px 16:9 thumbnail, title/subtitle, time, transport, seek, next, stop, audio, subtitles,
  volume and "open player".
- A 2 px progress strip along the top.
- Clicking the thumbnail returns to the full player at the same position.
- It hides on audio playback, stops on sign-out, and handles a native HDR window under the
  thumbnail on Windows.

This is a **desktop behaviour**, not a mobile PiP. Runefin already owns the player and the surface;
the missing work is decoupling the session from the player route and rehosting the native surface
in a small bar. That should be a first-class milestone, not a footer afterthought.

### 2.10 Settings should explain, not enumerate

Moonfin groups settings into Account, General, Appearance, Playback, Storage & Downloads,
Integrations and About, with search and a live setup wizard. The breadth is excessive for
Runefin. The **information architecture** is good:

- Account: server, user, sign out.
- Appearance: theme tint, reduced motion, sidebar pin.
- Playback: default quality, auto-play next, remember position, subtitle style.
- Storage: image cache, downloads (later).
- About: version, build, keyboard shortcuts, licences.

Do not build a settings search for eight rows. Do build groups, descriptions and a single
"Restore defaults" action. Runefin's current Settings page is an install/debug readout; it is the
least product-like screen in the app.

### 2.11 Desktop integration is part of the design

Moonfin's Linux shell is not flashy, but it is careful:

- Native GTK title bar; no fake traffic lights or custom drag region.
- AppImage installs an icon and `.desktop` entry with `StartupWMClass` and a `moonfin://` MIME
  handler, unless a manager already owns the entry.
- Single instance with command-line forwarding.
- Window geometry is restored only if it still fits a current monitor; otherwise it is clamped.
- AppImage/Flatpak differences are handled rather than assumed away.

Runefin's current binary has none of this. It is still a developer launch. For a Linux desktop
client, packaging and window behaviour are as visible as the card radius.

---

## 3. Runefin today

Baseline: app at `9015945`, Rune fork at `412f7e76`, 64 app tests and 27 snapshots; the live
server was Jellyfin 10.11.2 at that time (it is now 12.4.0, matching the design contract).

**Keep and protect**

- The Rune-drawn shell; no web view.
- The DMA-BUF GStreamer player with CPU and mpv fallbacks.
- The image pipeline: 27×17 placeholders, 200 ms crossfade, priority queue, disk cache and
  immutable image tags.
- The pure-state screens and headless `Runtime` tests.
- The 224 px sidebar, 40 px gutters, 176×264 posters and 16:9 CW rail.
- Playback reporting and the resume label.

**Missing or thin**

| Area | Current Runefin | Consequence |
|---|---|---|
| Item logo | `ImageKind::Logo` exists, never requested | Text-only heroes; weaker identity |
| Ambient home | No backdrop; flat navy surface | Home reads as a catalogue, not a place |
| Cast | `People` not parsed | Detail lacks faces and roles |
| Next Up | No series next-up query | Users return to the season list instead of the next episode |
| Similar | No `/Items/{id}/Similar` | Detail dead-ends |
| Chapters | Not parsed or drawn | No chapter ticks or chapter menu |
| Trickplay | Not parsed or fetched | Scrub is blind |
| Media segments | No `/MediaSegments/{id}` integration | No skip intro/credits |
| Subtitles | No track enumeration or styling | Only audio switching is exposed |
| Buffering | No buffered range, no buffering state | Seek bar lies about network health |
| Interactive seek | Progress is a 4 px readout, not a control | No scrub, no preview |
| Mini player | Player screen owns the session | Browsing stops playback |
| Window geometry | Opens at the default size each run | Desktop app feels unfinished |
| Focus restoration | Scroll is restored; focus key is not | Back lands at the right offset but the wrong card |
| Reduced motion | No setting or audit | Motion cannot be disabled |
| Quick Connect | API methods exist, no UI | Password-only sign-in |
| Packaging | Source binary only | No menu entry, icon or single instance |

Quick Connect is a specific example of a doc/code gap: `quick_connect_initiate`,
`quick_connect_poll` and `authenticate_with_quick_connect` are in `jellyfin-api`, but the connect
screen only exposes username/password. Either surface it or remove the claim from the design.

---

## 4. Jellyfin capability map

“Native” means the capability comes from Jellyfin itself. “Moonbase” means Moonfin's own server
plugin. “Third party” means an external service. Runefin should only promise native Jellyfin
features it can feature-detect and degrade from.

### Identity, shell and desktop

| Capability | Moonfin 3.0 Linux | Runefin | Call |
|---|---|---|---|
| Manual server URL | Yes | Yes | Keep |
| LAN server discovery | Yes | No | Later |
| Password sign-in | Yes | Yes | Keep |
| Quick Connect / QR | Yes, native Jellyfin | API only, no UI | **Now (small)** |
| Multiple servers | Jellyfin/Emby/Plex/Silo, merged | No | No |
| Window geometry restore | Yes | No | **Now** |
| Single instance / deep links | Yes | No | Later |
| Native title bar | Yes | Yes | Keep; no fake chrome |
| AppImage/deb/rpm/Flatpak | Yes | No | Later |
| Keyboard shortcut sheet | `?` / F1, context-aware | No | **Now** |
| UI scale | Small–Great-Grandparents | Fixed | Later |
| Reduce motion | Partial (seasonal only) | No | **Now** |
| Global fullscreen / quit | F11, Alt+Enter, Ctrl+Q | No | Later |

### Home

| Capability | Moonfin 3.0 Linux | Runefin | Call |
|---|---|---|---|
| Continue Watching | Yes, mergeable with Next Up | Yes, merged by series | Keep |
| Next Up row | Yes | No | **Adopt** |
| Recently added rails | Yes | Yes, capped at 4 | Keep |
| Featured media bar | 6 layouts, full height, 10 s auto-advance | No | **Optional compact** |
| Backdrop style | Off / full / corner / top | None | **Adopt full/corner** |
| Peek first row | Optional on desktop | No | Later |
| Row reorder/toggle | Yes | No | Later |
| Hide CW items / max days | Yes | Stale 60 days only | Later |
| Continue Watching default action | Play or detail, user choice | Detail only | Later |
| Custom plugin rows | Moonbase | No | No |
| Seasonal row/effects | Moonbase | No | No |

### Library and search

| Capability | Moonfin 3.0 Linux | Runefin | Call |
|---|---|---|---|
| Poster grid | Yes | Yes, virtualized | Keep |
| List view | Yes | Yes | Keep |
| Sort | Many fields | Name/Added/Rating | Add a few |
| Filter | Genre, decade, rating, watched, etc. | Unwatched only | **Adopt core facets** |
| Group by | Decade, genre, rating | No | Later |
| Folder view | Yes | No | Later |
| Letter rail | Yes | At ≥100 items, name sort | Keep |
| Quick return | Yes | No | Later |
| Titles under posters | Toggle | Always | Later |
| Library styles | Classic / Modern / Docked | One | No |
| Search scope | Global and in-library | Yes | Keep |
| Search recents | Yes | No | **Adopt** |
| Search groups | Movies/shows/episodes/people | Yes | Keep |
| People results | Open person pages | People chips | **Adopt person page** |
| Collections/playlists/favourites | Yes | No | Later |

### Detail

| Capability | Moonfin 3.0 Linux | Runefin | Call |
|---|---|---|---|
| Detail styles | 8 | 1 | No; stay adaptive |
| Backdrop + scrim | Yes | Yes, two-axis | Keep |
| Item logo | Yes | No | **Adopt** |
| One primary action | Yes, configurable extras | Yes | Keep |
| Tabs/sections | Overview, cast, similar, etc. | Overview + episodes | **Adopt cast/next-up/similar** |
| Cast and roles | Yes | No | **Adopt** |
| Next Up card | Yes | No | **Adopt** |
| Similar / More Like This | Yes | No | **Adopt** |
| Episode thumbnails | Yes | Yes | Keep |
| Chapters | Yes | No | Later |
| Extras / trailers | Yes | No | Later |
| Reviews / ratings | TMDB/MDBList | No | No |
| Technical info | Yes | No | Later |
| Sticky compact header | Partial | No | **Adopt** |
| Admin actions | Yes | No | No |

### Playback

| Capability | Moonfin 3.0 Linux | Runefin | Call |
|---|---|---|---|
| Engine | libmpv / media_kit | GStreamer + VA-API DMA-BUF | Keep |
| Direct play | Yes | Yes | Keep |
| Transcode fallback | Server transcoder | mpv fallback, no transcode | Later |
| HDR10/HLG tone map | mpv | Shader | Keep |
| HDR passthrough | Windows native; Linux SDR | No | No for now |
| Dolby Vision | mpv/Apple/Android | No | No |
| Audio tracks | Yes, labelled | Cycle 1/n | **Adopt labels** |
| Subtitles | Yes, styles, delay, dual | No | **Adopt core** |
| Trickplay | Single/filmstrip/full | No | **Adopt filmstrip** |
| Chapter marks | Yes, off by default | No | **Adopt** |
| Skip intro/credits | Yes, server segments | No | **Adopt** |
| Next Up in player | Yes | No | **Adopt** |
| Still watching | Yes | No | Later |
| Interactive seek | Yes | No | **Adopt** |
| Buffered track | Yes | No | **Adopt** |
| Speed control | Yes | No | Later |
| Crop black bars | Yes | No | Later |
| Player time labels | 6 configurable slots | Elapsed/remaining only | Later |
| OSD button customisation | Yes | No | No |
| Mini player | Yes | No | **Adopt** |
| Shortcut sheet | Yes | No | **Adopt** |
| Playback reporting | Yes | Yes, robust | Keep |
| Playback info | Yes | No | Later |

### Jellyfin beyond playback

| Capability | Moonfin 3.0 Linux | Runefin | Call |
|---|---|---|---|
| Remote control other sessions | D-pad, search, tracks | Pause/seek | Later |
| Cast to Google Cast/DLNA/AirPlay | Yes | No | No |
| SyncPlay | Yes | No | No |
| Live TV / DVR | Yes | No | Later |
| Downloads / offline | Yes | No | Later |
| Music | Yes | No | No |
| Books / audiobooks | Yes | No | No |
| Retro games | Moonbase | No | No |
| Personal ratings | Yes | Read-only favourite flag | Later |
| Favourites | Yes | Read-only | Later |
| Playlists | Yes | No | Later |
| Collections | Yes | Read-only via library | Later |
| Parental controls / Kids | Yes | No | No |
| Admin panel | Yes | No | No |
| Seerr | Third party | No | No |
| Achievements / anime markers | Moonbase | No | No |
| Settings sync / messages | Moonbase | No | No |
| Diagnostics / update check | Yes | No | Later |

**What this table says:** Runefin does not need more *categories* to feel complete. It needs depth
in the categories it already has: home, library, detail, player, settings. The “Later” list is
mostly additive product surface; the “Now” list is mostly finishing work.

---

## 5. The home decision

Moonfin's default desktop home is a **full-viewport featured media bar**: the first screen is one
title, its backdrop, its logo, metadata and overview; the rows begin below the fold. It is a
cinematic choice, and it works on a TV or a large monitor. It is also a poor default for a
1280×800 desk client whose primary job is “resume what I was watching”.

The current Runefin design doc is right to reject a full-bleed hero at this size. Do not reverse
that. Instead, use Moonfin's *treatment* without its *dominance*:

### Recommended: “Home with a view”

- Keep the **Continue Watching rail above the fold**, with the 44 px title, a quiet subtitle and
  the current 16:9 cards.
- Add a **full-window ambient backdrop** behind the home content, taken from the first resume
  item. Decode once at 1280 max, apply a 35–45% black scrim and a bottom fade into the page; no
  blur by default, no auto-advance.
- Add an **optional Spotlight band** (off by default) above the rail:
  - 240–280 px tall, inside the 40 px gutters, not full-bleed.
  - Item logo (fallback title), one metadata line, one primary Resume/Play, slide dots.
  - No automatic trailer; no auto-advance unless the user turns it on.
  - At 1280×800 this leaves the CW rail visible; at 1440×900 it feels generous.
- Keep the current title text for the page. Do not copy Moonfin's giant marketing headline.

This gives the client the “cinema” cue — a living backdrop, a logo, a single hero action — while
preserving the resume-first hierarchy that differentiates Runefin from Moonfin.

### If the user later asks for a full media bar

Treat it as an **appearance mode**, not a rewrite:

- Add a `HomeLayout` preference: `Continue first` (default) or `Featured first`.
- Reuse the same `HomeData`; the bar is a presentation of the first resume/latest item.
- Cap it at 46–55% of viewport height, not 100%, even in featured mode.
- Disable auto-advance by default.
- Ship a snapshot for both modes at 1280 and 1440.

---

## 6. Upgrade roadmap

The order is chosen for **visible confidence per unit of risk**. Do not start with a new data
source when the shell still opens at the wrong size.

### Phase 1 — Desktop shell and identity

**Goal:** the app feels installed, not launched from a terminal.

1. **Window geometry**
   - Save width, height, x, y, maximised and fullscreen in `Config`.
   - Restore only if the saved rectangle still intersects a monitor; otherwise clamp to the
     nearest work area.
   - Minimum 800×500; default 1280×800.
   - Use Rune's existing window options, not a new dependency.
2. **Shortcut reference**
   - Extract player keys from `main.rs` into one `keymap` table.
   - `?` and F1 open a focused overlay with app and player sections, using the same table as the
     handler so they cannot drift.
   - Add F11/Alt+Enter fullscreen and Ctrl+Q quit only after the window layer supports them.
3. **Item logo pipeline**
   - Request `ImageKind::Logo` for detail and the optional spotlight, max width 520.
   - Decode once, cache by tag, `contain`, max 320×110; fall back to text.
   - Never put a logo on a small card; it is a hero element.
4. **Reduced motion**
   - Add a setting. When on, replace scale/translate/shared-element travel with 120 ms opacity.
5. **Focus restoration**
   - Add `focus_key` to the route stack and restore it on back, not just scroll.
   - This closes a promise already made in `docs/design.md`.

**Acceptance:** restart restores the window; 1024/1280/1440 snapshots unchanged except the new
shortcut overlay; Tab reaches the overlay and Esc closes it; no network in tests.

### Phase 2 — Home with a view, and a complete detail page

**Home**

- Ambient backdrop with 300 ms crossfade, no layout shift when art arrives.
- Optional compact Spotlight band; default off.
- Next Up row, merged or adjacent to CW; cap and empty-state rules.
- “See all” and rail link counts remain truthful.

**Detail**

- Logo/title fallback.
- Cast rail (1:1 portraits, name and role, 92 px).
- Next Up card (16:9 still, `S2E04 · title`, resume position, one Play action).
- Similar rail, using `/Items/{id}/Similar` with a cap of 12.
- Sticky compact header after 220 px of scroll: backdrop blur/darken, title, play, back.
- Keep the current overview measure and episode rows.

**Acceptance:** detail loads the item immediately and fills cast/next-up/similar without blocking
the page; skeleton geometry matches the final sections; no dead “More” affordance; snapshots at
1280 and 1440.

### Phase 3 — The player OSD

**Goal:** the in-app player stops looking like a debug toolbar.

1. **Chrome**
   - Top gradient, back, title, path badge; bottom gradient; centre transport.
   - Bottom stack: filmstrip (when available), timeline, transport row.
   - Controls auto-hide after 3 s while playing; never hide while paused.
2. **Timeline**
   - Interactive drag with keyboard step support.
   - Buffered range (3 px), played range (4 px), 10 px thumb, 2×6 px chapter ticks.
   - Elapsed / remaining; optional end time later.
3. **Trickplay**
   - Parse the item's `Trickplay` info; fetch tiles through the existing image worker with a
     dedicated key and high priority.
   - Filmstrip of 5 tiles around the pointer; single tile on keyboard focus.
   - Prefetch ±15 s; cancel stale requests; no blocking on the UI thread.
4. **Tracks**
   - Audio: labelled list with language, codec and default flag; current selection.
   - Subtitles: off + embedded/external; label, language, codec, default/forced; delay controls.
   - Keep the current direct-play fallback. If subtitle rendering cannot stay on the GPU path,
     render it in Rune over the surface rather than dragging frames back to the CPU.
5. **Jellyfin context**
   - Skip intro/credits when `/MediaSegments/{id}` offers a segment; 404 means no button.
   - Next Up prompt near the end, with a 10 s countdown; dismiss keeps the current item playing.
   - Buffering, reconnecting and terminal error states with a retry that does not lose position.
6. **Keyboard**
   - Space/K, ←/→ or J/L, Up/Down volume, M mute, C subtitles, S skip, Shift+N/P next/previous,
     F fullscreen, I info, Esc/Backspace layer-aware.
   - The shortcut sheet from Phase 1 reflects this table.

**Acceptance:** a live 1080p item scrubs with a preview; chapter marks match the item; a missing
segment endpoint leaves no dead control; pause never auto-hides; all tests and snapshots pass;
4K HDR still falls back cleanly rather than showing a black screen.

### Phase 4 — Mini player

**Goal:** browse without stopping playback.

- Split the playback session from the player route. The session owns the `Player`; the route
  owns presentation.
- Add a bottom bar outside the route tree:
  - 96 px wide; 64 px below 720 px.
  - 80 px 16:9 thumbnail, title/subtitle, time, previous/seek/play/seek/next/stop,
    audio/subtitle, volume, expand.
  - 2 px progress strip on top.
- The thumbnail opens the full player at the current position; the bar survives home, library,
  detail, search and settings.
- Stop on sign-out, item end or explicit stop. Hide for audio items.
- If Rune cannot rehost the native surface in a second tree, gate the milestone on a prototype
  and fall back to a paused poster bar rather than faking video.

**Acceptance:** play → minimize → browse → reopen restores the same position; no dropped frames
in the bar; no second player instance; test with keyboard and pointer; snapshot both wide and
compact.

### Phase 5 — Library, search, settings

- Library: add genre/decade/rating filters behind a compact filter row; show active filter count;
  preserve current virtualization.
- Search: recent searches per user (local config), grouped result counts, “See all” only when
  there is more than the cap.
- Settings: Account / Appearance / Playback / Storage / About groups with descriptions; keep the
  destructive sign-out isolated; add “Restore defaults”.
- Add a first-run connect hint for Quick Connect (QR/link) if the Phase 1 UI is ready.

### Phase 6 — Jellyfin depth, only after the surface is polished

Order by user value and API confidence:

1. Quick Connect UI.
2. Subtitle track labels and styling.
3. Playback info and an explicit “server transcode” fallback for unsupported codecs.
4. Favourites, watched/unwatched and playlists.
5. Next Up across the home and detail.
6. Remote control D-pad and search.
7. Live TV/DVR, downloads, SyncPlay, multi-user, admin — only on explicit request.

### Phase 7 — Linux packaging

- AppImage first; deb second.
- `.desktop` entry with icon, `StartupWMClass`, `AudioVideo;Video;`, and a `runefin://` scheme if
  deep links are added.
- Single instance through the same D-Bus/command-line forwarding Moonfin uses.
- A `--version` flag and a visible build identifier in About.
- No WebKit, no second renderer, no new runtime dependency.

---

## 7. Engineering notes for the next implementer

### 7.1 Extend the wire types, do not fork the client

The current `Item` model deliberately omits `People`, `Chapters`, `MediaSources`, `MediaStreams`,
`Trickplay`, `Studios` and `ProviderIds`. Add optional fields with `#[serde(default)]` and use
`fields=` only where needed:

```text
People, Chapters, MediaSources, MediaStreams, Trickplay, Studios, ProviderIds
```

Suggested additions:

- `Person { name, id, role, kind, primary_image_tag }`
- `ChapterInfo { start_position_ticks, name, image_tag }`
- `MediaSource { id, name, container, runtime_ticks, media_streams }`
- `MediaStream { kind, index, codec, language, display_title, is_default, is_forced }`
- `TrickplayInfo { width, height, tile_width, tile_height, thumbnail_count, interval, bandwidth }`
- `MediaSegment { id, kind, start_ticks, end_ticks }`

Add client methods only after the endpoint is pinned against the target server:

- `next_up(series_id, limit)` → `/Shows/NextUp`
- `similar(item_id, limit)` → `/Items/{id}/Similar`
- `media_segments(item_id)` → `/MediaSegments/{id}`
- `playback_info(item_id)` → `/Items/{id}/PlaybackInfo`
- `favorite(item_id, on)` → `/UserFavoriteItems/{id}`

A 404 from `MediaSegments` is a normal answer for older servers; it must not surface as a screen
error.

### 7.2 Keep the threading contract

- All added HTTP work goes through `Latest` or the existing worker; no request in a handler or
  `poll`.
- Trickplay prefetch belongs on the existing image worker with a new priority lane, not a new
  thread per frame.
- A scrub that moves faster than the network must cancel the old preview; the last answer must be
  matched to the current item/source and ignored otherwise.
- The player's bus polling already runs on the frame; do not add blocking `timed_pop` calls that
  wait.

### 7.3 Image keys and memory

- Logo: `(server, item, "Logo", tag, 520)`.
- Ambient: `(server, item, "Backdrop", tag, 1280)`; one at a time.
- Trickplay: `(server, item, source, "Trickplay", width, index)`; small decode (320 px wide).
- Chapter thumbnails: reuse the item `Thumb` or chapter image; do not fetch a second full backdrop.
- Keep the 256-image cap; an ambient backdrop must be evictable and must not pin the whole home.

### 7.4 Focus and navigation

The design doc says the navigation stack holds `(route, scroll, focus_key)`. The code currently
restores scroll only. Add a focus key to `Route`, set it on card press, and call the runtime's focus
API on back before the first paint. Test with:

- card 37 in a library → detail → Esc → the same card focused;
- search result → detail → Esc → the search field focused;
- player → stop → the detail Play button focused.

### 7.5 Window geometry

- Persist only after the window has settled; a live drag should debounce.
- On Wayland the compositor may ignore position; save size and fullscreen, and treat position as
  best effort.
- Clamp to the monitor with the greatest overlap; never restore a title bar off-screen.
- Do not add `window_manager` or a desktop-environment crate; Rune already has the window handle.

### 7.6 Tests and snapshots

Add scenes rather than mutating old ones:

- `home-backdrop`
- `home-spotlight`
- `detail-cast`
- `detail-next-up`
- `player-osd`
- `player-tracks`
- `player-buffering`
- `player-error`
- `mini-player-wide`
- `mini-player-compact`
- `shortcut-sheet`
- `settings-appearance`
- `library-filtered`

Render 1280×800 and 1440×900 for the home/detail/player set. Keep the existing 27 scenes; every
new screen needs a snapshot because it can move pixels.

---

## 8. What not to copy

### Do not copy the style marketplace

Eight detail styles, six media bars, four themes, a theme store, seasonal effects and a setup
wizard multiply the visual QA matrix by an order of magnitude. Runefin has one coherent visual
language; the upgrade should deepen it, not let the user dismantle it.

### Do not make a full-height featured bar the default at 1280×800

Moonfin can afford it because it is a general-purpose client for phones, tablets and TVs. Runefin
is a desk client whose first job is resume. A 240–280 px optional spotlight is enough cinema.

### Do not copy auto-playing trailers

They fight the app's own player, cost network and GPU, surprise the user, and complicate
reporting. If a trailer is added later, it is an explicit action with sound off by default.

### Do not copy 1.05 card scaling into the virtualized grid

It clips at the viewport edge, changes row heights and makes a 20 000-item grid harder to keep
stable. Use ring, shadow and a small translate; reserve expansion for the CW rail or pointer
hover.

### Do not copy the top navigation

Runefin's sidebar is the right shell for a library-heavy desktop app, and it is already good. The
lesson from Moonfin is the **floating, translucent treatment**, not the top placement.

### Do not copy the multi-platform scope

Plex, Emby, Silo, games, books, music, audiobooks, admin, multi-server, achievements, Seerr and
theme sync are a different product. Every one of them is a maintenance surface Runefin does not
need to be “professional”.

### Do not copy GPL code

Moonfin-Core is GPL-2.0; Runefin is MIT. Use measurements, behaviour and public design language.
If a specific algorithm is worth reusing, reimplement it from the API contract or make a conscious
licence decision.

---

## 9. Open decisions

1. **Home default.** Keep Continue Watching first (recommended), or adopt a compact Spotlight
   above it? The document recommends backdrop + optional spotlight, not a full media bar.
2. **Mini player before subtitles?** The document puts the OSD/tracks first and the mini player
   next, because the OSD is lower risk and unlocks the player's visual quality.
3. **How many preferences?** Eight to twelve, grouped. Anything that changes a layout needs a
   snapshot; anything that changes a data source needs a test.
4. **Packaging target.** AppImage is the lowest-friction first target; deb is the second. Flatpak
   only when the app has a stable ID and a sandbox story.
5. **Live TV, downloads and SyncPlay.** Defer until the browse + playback loop feels finished;
   each is a product, not a toggle.

---

## 10. Acceptance for the whole upgrade

At 1280×800:

- The app opens at the saved size, within the work area.
- Home shows a real backdrop, the Continue Watching rail above the fold, and no empty section.
- A card opens a detail page with logo, cast, next-up, similar and a readable overview.
- The detail back returns to the same card and offset.
- Play starts in-app, the OSD is legible over bright and dark frames, and pause keeps it visible.
- Scrub shows a preview; chapters are visible when present; tracks are labelled; no missing
  segment endpoint leaves a dead button.
- Minimize keeps playback alive while browsing; reopen restores the exact position.
- `?` opens a shortcut sheet that matches the keys that actually work.
- 1024, 1280 and 1440 snapshots are committed; `scripts/check.sh` is green; no test touches the
  network.

The target is not “Moonfin with a Rune renderer”. It is **Moonfin's sense of place, with Runefin's
focus**: one server, one user, one excellent desktop surface, and a player that stays out of the
way until it is needed.

---

## 11. Primary sources

- Moonfin-Core repository:
  [github.com/Moonfin-Client/Moonfin-Core](https://github.com/Moonfin-Client/Moonfin-Core).
  Source read at commit `d4430f66bc7a08f470ef09c41bc17668a0077957` (3.0.0).
- Moonfin 3.0.0 release:
  [Releases](https://github.com/Moonfin-Client/Moonfin-Core/releases/tag/3.0.0).
- Moonfin feature wiki:
  [Features](https://github.com/Moonfin-Client/Moonfin-Core/wiki/Features).
- Moonfin playback and codecs:
  [Playback and Codecs](https://github.com/Moonfin-Client/Moonfin-Core/wiki/Playback-and-Codecs).
- Runefin design and current decisions: `docs/design.md`, `docs/design-player.md`,
  `docs/research/04-ui-ux-finishing-pass.md`.
- Runefin current source: `crates/runefin/src/` and `crates/jellyfin-api/src/`.
