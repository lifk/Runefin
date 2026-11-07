# 03 · What makes a streaming UI feel expensive

Scope: visual and motion craft only — home, rails, focus, detail, OSD, loading. IA lives in `02-media-ux-patterns.md`. Netflix, Disney+, Apple TV+, Prime Video and Max publish almost no UI numbers, so hard values below come from systems that *do* publish (Material, Apple HIG, Video.js) and are applied to what the streaming apps visibly do. Anything I could not source is marked **[est]** (estimated from observation) or **[inferred]** (my recommendation, not practice).

## 1. The mechanism

Expensiveness is not more effects. It is three properties holding at once:

1. **One clock.** A single interpolation family and duration ladder; no element snaps while another eases.
2. **Continuous focus.** The eye never loses the cursor, and nothing is ever focused but invisible.
3. **Chrome never touches artwork.** Titles and controls float on gradients, not on boxes.

Netflix is the reference because it draws through one widget primitive straight to the GPU: one animation model, 60 fps on sub-GHz hardware, "key input responsiveness" as a first-class metric ([Netflix Tech Blog](https://netflixtechblog.com/crafting-a-high-performance-tv-user-interface-using-react-3350e5a6ad3b), [TvAppLab on Gibbon](https://tvapplab.com/apps/netflix)). Runefin has that natively; the risk is spending it on effects instead of removing seams.

## 2. Home / billboard

- **Artwork.** Full-bleed 16:9 backdrop, edge to edge, no frame or corner radius on the hero. Apple asks for "beautiful, edge-to-edge artwork" and "subtle and fluid animations" ([HIG: designing for tvOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-tvos)).
- **Title lockup.** Use the transparent PNG wordmark (`/Items/{id}/Images/Logo`), not typeset text — transparency is what lets a rigid grid feel sophisticated ([Smashing](https://www.smashingmagazine.com/2025/09/designing-tv-principles-patterns-practical-guidance/)). Typeset only when no logo exists, and never over a bright region.
- **Scrim.** Two gradients, not one: a vertical ramp to near-black across the bottom ~40% for the text block, plus a weak horizontal ramp on the text side so a bright still cannot eat the title **[est]**. The tell of a cheap client is a single gradient starting too high, greying the whole image.
- **Height.** 52–58 vh, text block bottom-left inside the gutter, ~34 characters per line **[est]**. Taller heroes push rails below the fold and read as an empty app.
- **Rotation.** Netflix's billboard rotates on a timer (~30 s, idle-gated; input resets it) **[est]**; no published value exists. **Decision: no auto-advance in v1.** It fights reading, needs idle detection and pause-on-focus, and buys nothing a click can't.

## 3. Rails and cards

- **Ratios.** 2:3 for film/TV posters, 16:9 for continue-watching and episode thumbs, 1:1 sparingly for people and collections — the three ratios streaming UIs converge on ([Smashing](https://www.smashingmagazine.com/2025/09/designing-tv-principles-patterns-practical-guidance/)).
- **Gaps as ratios.** Card gap = 0.10 × card width; gutter = 3.5 vw. Posters 168–196 px at 1280–1440 wide **[est]**.
- **Peek.** The trailing card must be visibly clipped at the right edge — the shelf "peeks" to promise more ([Smashing](https://www.smashingmagazine.com/2025/09/designing-tv-principles-patterns-practical-guidance/)); Smashing's odd-column trick (13 instead of 12) keeps the last card inside the margin while still peeking. A rail flush with the gutter is a dead end.
- **Arrows.** 40 px circular scrim chip, 0.55 alpha black, 20 px chevron, fading in over 120 ms on rail hover only; hidden at the ends rather than greyed **[est]**. Persistent arrows are the loudest web-app tell.
- **Momentum.** One model for wheel, drag and keyboard: velocity-carrying, friction decay, settling on a card boundary with a ~220 ms spring **[est]**. Mixing native wheel scroll with animated paging is what makes carousels feel drunk.
- **Hover.** Scale 1.04, brightness ×1.10, 160 ms; no shadow (invisible on near-black anyway) **[est]**. Expansion waits ~450 ms of dwell — Netflix ships rail video previews validated by A/B tests and eye-tracking ([Netflix Tech Blog](https://netflixtechblog.com/crafting-a-high-performance-tv-user-interface-using-react-3350e5a6ad3b)), and the dwell is what stops a moving mouse firing five previews.

## 4. Motion

Published anchors: Material sizes duration by area — 100 ms switch, 250/200 ms sheet expand/collapse, 300/250 ms FAB→dialog — and states the rule that **exits are shorter than enters** ([Material: speed](https://m2.material.io/design/motion/speed.html)). Anything that begins and ends at rest uses `cubic-bezier(0.4, 0, 0.2, 1)` ([Material: speed](https://m2.material.io/design/motion/speed.html)). Expressed as Rune's duration + bounce (0 = critically damped):

| Interaction | Duration | Bounce | Note |
|---|---|---|---|
| Card hover in / out | 160 / 120 ms | 0.00 | 4% scale; overshoot reads as wobble |
| Card → preview expand | 240 ms | 0.06 | after 450 ms dwell **[est]** |
| Detail open (shared element) | 320 ms | 0.00 | Material's large-surface 300 ms **[est]** |
| Detail close | 240 ms | 0.00 | exits shorter |
| Page transition | 180 ms | 0.00 | crossfade + 12 px rise **[est]** |
| OSD fade in / out | 120 / 200 ms | — | |
| Rail settle | 220 ms | 0.10 | the only place bounce > 0.05 earns its keep **[est]** |

Bounce above ~0.15 on anything containing text is worse than none: a title that overshoots is a title that shakes.

## 5. Focus and selection

Apple's framing is that focus should "gently highlight and expand onscreen items" so people always know where they are ([HIG](https://developer.apple.com/design/human-interface-guidelines/designing-for-tvos)) — expand, not outline. Smashing catalogues the three techniques in use: scale, colour inversion, border ([Smashing](https://www.smashingmagazine.com/2025/09/designing-tv-principles-patterns-practical-guidance/)). A border alone is the browser look; inversion is the TV look and too loud for a pointer UI.

**Decision:** scale 1.04 + 2 px inner ring at 0.9 white + brightness lift, on the hover spring. Draw the ring *inside* the card so no parent clip can cut it, and pad the rail's scroll container vertically by the max scale delta so the halo never crops **[inferred]**. Keyboard scrolling keeps the focused card about one-third from the left, so cards ahead are already visible **[est]**. Focus memory is per rail: return restores the last card, not index 0.

## 6. Detail page

Backdrop with the same two-axis scrim → wordmark/title → one metadata line (year · runtime · rating · genres) → action row → overview → seasons/episodes → cast → collapsed technical info.

- **One filled button.** Primary is Play, becoming "Resume 42:13" when a position exists — the label carries state, not a second button. Trailer is a ghost button; add-to-list and more are icon-only. Two filled buttons is the commonest hierarchy failure in Jellyfin clients **[inferred]**.
- **Trailer** never takes the primary slot, opens an overlay player, and never autoplays on load **[est]**.
- **Episodes.** Virtualized rows: 16:9 thumb, number, title, runtime, watched tick, 3 px progress bar under the thumb. The current episode scrolls into view on open.
- **Next episode.** A countdown panel over the final ~20 s: "Next episode in 5" with Cancel and "Watch credits". Five seconds is the observed cadence **[est]**; the visible countdown plus an escape hatch is what separates this from hostile autoplay.
- **Open transition.** The card *expands into* the detail view (shared element on backdrop and title) rather than pushing a route. This is the strongest premium cue in the app — it preserves spatial continuity, which nothing else does.

## 7. Playback OSD (stretch, ~15% of effort)

- **Scrim.** Bottom-only, transparent → 0.7 black over the lower 25%, plus a soft 0.35 top gradient for the title. Never a full-screen dim; it destroys the image being watched **[est]**.
- **Auto-hide.** 2 s of inactivity, and **controls stay visible while paused** — the behaviour Video.js codifies, with the idle delay fixed at two seconds ([Video.js](https://videojs.org/docs/framework/html/guides/controls)). Any input re-arms the timer; keyboard focus inside the bar suppresses hiding ([Video.js](https://videojs.org/docs/framework/html/guides/controls)).
- **Buffering.** No spinner over black. Hold the last decoded frame, show an indeterminate hairline on the progress rail, escalate to a "Buffering…" label only after ~1.5 s **[est]**.
- **Subtitles.** Netflix exposes font, size, shadow and background colour ([Netflix Help](https://help.netflix.com/en/node/100267)); good defaults are pure white, 4 px shadow, a 0.75-alpha black box, never smaller than body text **[est]**.

## 8. Type, density, colour

- **Near-black, not black.** `#141414` surface (Netflix's near-black background, observed **[est]**); pure `#000` kills elevation cues and reads as a void. Netflix red is `#E50914` ([brand colours](https://www.brandcolorcode.com/netflix)).
- **Contrast, computed** against `#141414`: white 18.4:1, `#B3B3B3` 8.8:1, `#8C8C8C` 5.5:1, `#E50914` 3.8:1. Body ≥ 7:1, secondary ≥ 4.5:1, and never set body copy in the accent red.
- **Muted palette.** Large pure-white areas glare in a dark room; tint brights toward brand and reserve high contrast for focus ([Smashing](https://www.smashingmagazine.com/2025/09/designing-tv-principles-patterns-practical-guidance/)).
- **Type scale.** Smashing's 24 px baseline is for a 10-foot UI ([Smashing](https://www.smashingmagazine.com/2025/09/designing-tv-principles-patterns-practical-guidance/)); at ~1 m on desktop, run 16 px body / 13 px meta / 28–32 px detail title. Copying TV sizes onto desktop looks like a mockup.
- **Text over art.** Never trust the image: scrim ≥ 0.45 alpha under the text run, a 2 px shadow, or move the text off the art.

## 9. Loading

Skeletons work because they mimic final layout and set expectations; a frame-only skeleton (header and background, no content shapes) is explicitly not recommended — users read it as broken ([NN/g](https://www.nngroup.com/articles/skeleton-screens/)). Shimmer helps but distracts: keep it slow, low-contrast ([NN/g](https://www.nngroup.com/articles/skeleton-screens/)), and delayed ~150 ms so LAN responses don't flash it.

For images the key is geometry-first: reserve the box from the aspect ratio so nothing reflows, paint a ~27×17 blurred placeholder, crossfade to the full image ([Medium's progressive loading](https://jmperezperez.com/medium-image-progressive-loading-placeholder/)). In a GPU renderer that's one blurred texture and a 200 ms alpha tween **[inferred]**.

## Design tokens

| Token | Value | Used for |
|---|---|---|
| `motion.hover.in` / `out` | 160 / 120 ms, bounce 0.00 | card scale + brightness |
| `motion.expand` | 240 ms, bounce 0.06 | card → preview |
| `motion.detail.open` / `close` | 320 / 240 ms, bounce 0.00 | shared-element open/dismiss |
| `motion.page` | 180 ms, bounce 0.00 | route crossfade + 12 px rise |
| `motion.rail.settle` | 220 ms, bounce 0.10 | scroll snap |
| `motion.osd.in` / `out` | 120 / 200 ms | OSD fade |
| `motion.curve.standard` | cubic-bezier(0.4, 0, 0.2, 1) | non-spring tweens |
| `time.osd.hide` | 2000 ms idle | OSD auto-hide |
| `time.hover.dwell` | 450 ms | preview trigger |
| `time.skeleton.delay` | 150 ms | before painting skeletons |
| `radius.card` | 6 px @ 180 px wide | posters/thumbs, scales with width |
| `radius.chip` / `panel` | 999 px / 12 px | arrow chips; modals, sheets |
| `space.unit` | 4 px | base grid |
| `space.gutter` | 3.5 vw | page edges |
| `space.rail.gap` | 0.10 × card width | between cards |
| `space.rail.v` | 40 px | between rails |
| `hero.height` | 55 vh | billboard |
| `hero.scrim.bottom` | 0 → 0.85 alpha @ 40% height | text block |
| `type.body` / `meta` / `title` | 16 / 13 / 32 px | desktop scale |
| `color.bg` | `#141414` | app surface |
| `color.text` / `muted` / `faint` | `#FFFFFF` / `#B3B3B3` / `#8C8C8C` | 18.4 / 8.8 / 5.5 : 1 |
| `color.accent` | `#E50914` | brand, never body text (3.8:1) |
| `aspect.poster` / `thumb` / `square` | 2:3 / 16:9 / 1:1 | artwork |

## The 5 patterns with the best polish-per-effort

1. **Shared-element card → detail.** One spring, two transforms; reads as a native app rather than a page swap.
2. **Scale focus with an inset ring.** Reuses the hover spring and deletes every browser-ish outline.
3. **Peek + hidden arrows.** Free — a gutter and a clip rect — and instantly reads as a streaming service.
4. **Two-axis hero scrim + PNG wordmark.** Turns a Jellyfin backdrop into key art with no new assets.
5. **Blur-up + skeleton geometry.** Removes both layout jumps and the blank-screen moment for one texture and one alpha tween.

## 3 patterns not worth it for v1

1. **Rails previews on hover.** Dwell timer, decode, audio policy, cancel-on-move — for a feature most desktop users disable. The 450 ms dwell bug will outlive the feature.
2. **Auto-rotating billboard.** A timer that fights reading, needs idle detection and pause-on-focus, and buys nothing a click can't.
3. **Parallax / gyro-style focus tilt** (the tvOS icon treatment). Apple composites it at OS level; per-card perspective transforms on a flat layout look like a template, not a product.
