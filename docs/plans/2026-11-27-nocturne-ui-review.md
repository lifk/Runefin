# Nocturne UI review — laptop, 2026-11-27

## Method

The session on the laptop was locked, so the interactive window could not be driven. The pass used
the app's own live capture path (`--live --snap`) on the laptop, against the real Jellyfin 12.4.0
server, at 1280×720 big-screen with the release 0.4.1 UI. That renders the same view, theme,
poster caches and GPU path the window uses; only the window chrome and the session lock are absent.

Mockup: `/tmp/opencode/raw2-a.png` (the chosen Nocturne direction) and the screen/motion specs in
`docs/plans/2026-11-26-nocturne-screen-spec.md` and `…-motion-spec.md`. Captures:
`/tmp/opencode/pass/` (18 screens), contact sheet `/tmp/opencode/pass/sheet.png`.

## Verdict by screen

| Screen | Verdict | Notes |
|---|---|---|
| Home | matches plan | 50 % billboard (mockup 62 %, the plan's intentional change); 76 pt rail; 6-up Continue watching; libraries strip; combined Recently added with See all; pins sheet. The kicker is now uppercase and tracked, as the mockup. |
| Libraries hub | matches | 3 large Recent tiles, 6-column All libraries, filter, Tools, playlists, Watchlist/Seerr. Library names inside the art come from the server's library images, not a second label. |
| Library | matches | Six columns, 48 pt gutters, A-Z in the right gutter (Z no longer clipped), two-line toolbar, Play all/Shuffle, count. |
| Film detail | near match | Full-bleed backdrop (the plan also says 66 % hero; see Open). Poster column, clearlogo, ratings, actions, collections, cast. Release date is now written (`30 Oct 2002`), not ISO. The action row wraps one button at 1280. |
| Series detail | near match | Clearlogo, season strip with total, Next up with spoiler guard, episodes. Overview HTML (`<br>`) is stripped now; the release date is written. |
| Search | matches | Focused field, suggestion chips, 6-up result posters, counts and See all. |
| Live TV | matches | Channel rows, now/next, progress, Watch, favourites/refresh. The focused row draws one box around the row (see Open). |
| Guide | matches, one deviation | Time ruler + 4 channel rows + pinned preview with still, programme and Watch. The focus is the whole row; the spec says the first programme should be focused. |
| Settings | matches | Section list beside the scrolling pane, Home rails entry, version, Appearance/Playback/Subtitles/…; one pane, as the plan allows. |
| Person | matches | Known for, credits, biography, Back. (The underscore in the capture's name is the capture's `RUNEFIN_LIVE_PERSON_NAME` input, not the UI.) |
| Downloads | matches | Empty state and folder; rail now keeps Libraries lit. |
| Remote | matches | Idle sessions with Play/Stop, Refresh; rail keeps Libraries lit. |
| Playlist | matches | Play all, Shuffle, Edit, Grid/List, 3 poster cards, no sort/filter. |
| Home rails sheet | matches | All/Chosen modes, 4-pin cap, list, Reset/Done, dimmed Home behind. |
| Seerr Discover/Requests/Detail | matches | Tabs, mode toggle, search, trending/films, requests, request detail. Rail keeps Libraries lit. |
| Player OSD / keyboard / connect | not in the live pass | The headless player has no sink, and the session was signed in; the committed `in-app-osd`, `keyboard-big` and connect scenes cover these unchanged views. |

## Fixed in this pass

- **Rail state:** Downloads, Remote, Seerr/SeerrDetail and a person page now keep their parent
  anchor lit (Libraries or Home) instead of showing no destination.
- **Library A-Z:** the index was 26 × 19 pt + gaps, taller than the 720-point body, so Z was cut.
  It is 18 pt + 1 pt now and all 26 letters fit.
- **Overviews:** Jellyfin/TMDb HTML is flattened to text and common entities decoded, for film and
  series detail, the next-up strip, the player's next episode and Seerr detail; person biographies
  use the same helper.
- **Release date:** `2002-10-30` reads `30 Oct 2002` in the detail facts.
- **Kicker:** `CONTINUE WATCHING · SERIES` in the mockup's uppercase, tracked caption.
- **Focus glow:** the lift shadow is dark bars outside the card; the previous filled overlay dimmed
  the focused card itself. The 2 pt poster edge and the 1.05 scale remain.
- **Card focus:** the 2 pt edge is on the poster/wide card only (0.4.1), not the card cell.

## Open, needs a design decision

1. **Guide focus.** The spec says the first *programme* is focused and Left/Right move in the
   timeline. The implementation focuses the *row* (channel) and draws a full-width white box.
   Programme-level focus needs block keys, a time-aware Left/Right and a preview per programme.
2. **Detail hero height.** The plan says a 66 % backdrop hero; the implementation is full-bleed
   (the backdrop continues behind Cast/Collections). Both are defensible; the current one is
   more cinematic, the plan's one leaves more page.
3. **Detail actions at 1280.** Eight actions wrap the last button ("Collection") to its own line.
   A "More" menu or tighter padding would keep one row; the plan does not say which actions to
   hide.
4. **Home second rail.** At 720 the Recently added rail is only its top edge. The plan says the
   first rail is visible at 720; the mockup shows more of the second rail because its hero is 62 %.
   This follows from (2) and the 50 % billboard.

## Mockup geometry

| Token | Mockup | App |
|---|---|---|
| Rail | 76 pt, five anchors, 32-34 pt mark | 76 pt, five anchors, 32 pt mark |
| Hero | 62 % (1280×720: 446) | 50 % (360), intentional |
| Billboard copy | 44 pt from rail, bottom 34 | 49 pt from rail, copy block near the body |
| Continue watching | 6 × 16:9, 11 pt gap | 6 × 16:9, 13 pt gap |
| Cards | 2 px ring + 1.035 scale | 2 pt inset edge + 1.05 scale |
| Library | not in the Home mockup | 6 columns, 48 pt gutters, A-Z gutter |
| Focus | cyan only on progress/focus | progress cyan, focus near-white |

## Second pass — 0.4.2

- **Focus:** the 2 pt ring is now painted by the screen-level overlay from
  `focused_card_rect`, the same mechanism as the old halo. As a child of the card it sat in the
  rail's clipped row and was cut; as a screen sibling it is a 2 pt border just outside the card
  and the 1.05 focus scale follows it. No line is drawn over the art and the title/thumbnail
  framing is gone.
- **Guide:** the first *drawn* programme is focused; Left/Right walk the timeline, Down changes
  channel, and the preview follows the focused programme. A channel without guide data is
  skipped rather than focused as a full-width row box.
- **Detail:** 66 % backdrop with collections and cast below it, the info column wide enough for
  the full action row, and written release dates.
- **Smaller fixes:** the Libraries/Home rail anchor on utility and sub-pages; A–Z reaches Z;
  HTML overviews and person/Seerr biographies are flattened; the hero kicker is uppercase.
- **Intentional:** the 50 % Home billboard (mockup 62 %), the Home/Search/Libraries rail order,
  and the libraries strip + combined Recently added rail.

The final images are the live server at 1280×720 big-screen; the focus pair is
`focus-card-big.png` and `rail-focus-big.png`.

## Third pass — 0.4.3

- **Home billboard:** the old config saved `home_hero = false` (the old compact default), so the
  Nocturne Home opened without its billboard. A one-time layout migration turns it on and stores
  `layout_rev = 1`; an Off chosen after that is kept.
- **Home backdrop:** the billboard is the first non-hidden Continue Watching item; if there is no
  resume, the first item of the newest rail. It is deterministic, not random.
- **Library background:** the page had no backdrop of its own, so the grid sat on flat black. It
  now has the same top band as Home across the full height, plus a cool radial wash in the upper
  right; the card art remains the subject.
- **Library top row:** the 1.05 focus scale and its outer ring had no room at the scroll viewport
  top. The grid and list now start 10 pt inside the viewport, so the first row's card and ring
  are complete; the ring remains the screen-level overlay.
