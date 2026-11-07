# Accessibility and focus audit — 2026-11-05

The checklist behind A1, A4, A8 and A9. It records what the code does now, not an aspiration.
Anything not listed here is not claimed.

## Motion (A1, WCAG 2.3.3)

- **Setting:** Settings → Playback → Reduced motion, persisted as `reduce_motion`.
- Turning it on stops a running theme video and refuses a new one; the toast says why.
- The setting does not remove functional transitions (focus, hover, OSD fade). Those carry no
  parallax or large-area movement and are needed for orientation.
- Theme media is **off by default**; the theme video is opt-in even with motion enabled.

## Text size (A2)

- Settings → Playback → **Text size** cycles 85 %, 100 %, 115 % and 130 %. It scales every
  `type_` token's size and line height together; poster and card geometry do not change.
- The preference is written to the 0600 config on close. The default is 100 %, so existing
  captures are unchanged.
- The library toolbar and detail actions flex-wrap at large text; a live 115 % render was checked
  for clipping. Settings rows keep a fixed label column and give the remaining width to the value.

## Keyboard customisation (A6)

- Settings → Keyboard remaps play/pause, mute, cycle subtitles, skip intro/credits, next and
  previous episode, fit/fill and fullscreen.
- Binding capture takes the next logical key, so a non-Latin layout can bind its own letters.
  Escape cancels. A duplicate binding is refused with a toast; Reset restores the defaults.
- Navigation keys (Escape, Tab, arrows, Ctrl K/Ctrl F) stay fixed so a bad binding cannot trap
  the window. The shortcut sheet reads the current bindings.

## Track badges (A4, WCAG 1.2.3)

- Audio description: the audio panel appends `· AD` when the server's display title contains
  "descriptive" or "audio description".
- SDH: the subtitle panel appends `· SDH` for "SDH", "hearing impaired" or "hard of hearing".
- Badges come from the server's own track titles; the app never guesses from language alone.
- The track remains selectable with the badge; the badge is not the only control label.

## Capability badges (A8)

- Library cards can show `4K`, `HDR`, `CC`, `5.1`, `Atmos` and `VERS`.
- Every badge is **text**, not a colour swatch, so it survives monochrome and colour-blind
  palettes. The colours behind them are neutral black at 68 % and never encode state alone.
- Watched and selection states use a check mark (`✓`) plus a filled shape; selection also says
  "selected" in the bulk bar.

## Focus (A9)

- Every interactive element added in Phases 0–2 has a stable `.key(..)`, `focusable(true)` and a
  2 px `focus_ring` (`theme::FOCUS`); the ring is drawn outside the fill so a dark card still
  shows it.
- Focus is supplied in addition to hover/press washes and a press scale, never instead of them.
- The audit covered: sidebar and watchlist, library toolbar/filter sheet/select mode, card
  menu and its new Like/Dislike rows, detail actions/versions/extras/theme toggles, pre-playback
  sheet, resume-conflict and still-watching cards, sleep panel, queue controls, audio panel,
  transfer picker, settings search/reset/export, the Playback/Subtitles & audio/Keyboard split,
  collection sort and pickers, the live reminder bell, the Requests (Seerr) connection card, and
  the Discover/Requests/request-panel pages.
- **Not yet audited:** measured contrast ratios for every token pair, and a keyboard-only pass
  across all 121 snapshots. The visible focus ring is present; the numeric contrast record is
  still a follow-up.

## Tests

- `reduced_motion` is exercised through the setting and the theme-video guard.
- `audio_modes_cycle_and_show_in_the_audio_panel` checks the panel keys.
- `a_library_card_shows_capability_badges` and `a_card_shows_audio_and_version_badges` check the
  text badges are rendered and addressable.
- `the_text_size_preference_scales_size_and_leading_together` checks the scale maths, and
  `shortcuts_can_be_remapped_but_not_duplicated` checks remapping and duplicate refusal.
