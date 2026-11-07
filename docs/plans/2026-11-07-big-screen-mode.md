# Big-screen mode and controller support — plan

**Status:** implemented on `main`, 2026-11-07. Mapping and focus are covered by unit and state
tests plus 1080p/4K captures; the hands-on Steam Machine pass remains. Implements
`docs/research/09-tv-mode.md` with the evidence in `10-tv-media-apps.md`.

**Definition of done.** Runefin has two complete interface modes — **Desktop** (today's app) and
**Big Screen** (a 10-foot layout for the Steam Machine) — chosen in Settings or at launch, with
every screen laid out for the mode. A game controller moves focus, activates, goes back, changes
sections and controls playback without touching a keyboard or a mouse. Snapshots exist for both
modes at 1280×720, 1920×1080 and 3840×2160.

---

## 1. Decisions

| Question | Decision | Why |
|---|---|---|
| Shell on TV | Keep the **left rail**, scaled | Android TV recommends left/right nav; Plex/Jellyfin use it; no shell rewrite |
| Mode switch | Explicit `ui_mode` setting + `--big-screen` + `RUNEFIN_UI=big` | Predictable, testable; Steam launch option sets it |
| Auto-detect | Not in v1 | A controller or a TV does not imply intent; an explicit mode is safer |
| Layout scale | **UI scale in Rune**: the logical viewport is 1280×720 on every TV, so the scale is the display height / 720 (1.5× at 1080p, 3× at 4K) | one number keeps cards, type and spacing proportional and crisp at every resolution; a token table cannot hold at both 1080p and 4K |
| Fullscreen | On in Big Screen at launch; cursor hidden | TV convention; F11 still toggles |
| Controller | **Native** `gilrs` in the app; Rune gains key injection and focus-by-name | Steam Input is a fallback; works outside Steam; A/B are real; no second input model |
| Focus | Rune's groups + a small `focus_key`/`focus_first` API | No second focus system; every screen inherits it |
| Hover actions | Replaced by a context button (Start) and chapter/seek paths | No pointer on a couch |
| Text entry | Steam's on-screen keyboard (non-Steam game) | An in-app keyboard is a later slice |
| Packaging | Out of scope for the UI work; document the SteamOS prerequisite | The UI is independent of Flatpak/AppImage |

---

## 2. Modes

`ui_mode` lives in `Config` (`desktop` | `big-screen`); `--big-screen` and `RUNEFIN_UI=big`
override the file for one launch. Settings → Appearance gets an **Interface** row (Desktop / Big
screen) that saves immediately. Changing it shows a one-line "Restart to apply" note rather than
re-laying out a live window.

Big Screen activates, at startup:

- fullscreen, borderless, cursor hidden until the pointer is moved;
- a 5 % safe margin around the content (gutter 56 pt at 960×540 design scale);
- `theme::set_big_screen(true)` — tokens below;
- initial focus on the first content item, not the rail (see §4).

Desktop is untouched: all scales are functions that read the flag, and the default is the current
values.

## 3. UI scale

Big Screen runs the same point layout on a smaller logical viewport. The scale is the display
height divided by 720: **1.5× at 1080p, 3× at 4K**, 1× at 720p. At 1280×720 points the app already
lays out 5 cards across a rail and the 16 pt body becomes 24 px at 1080p — the TV baseline — so
cards, type and spacing need no separate scale table.

Implementation, in Rune:

- `AppSpec::ui_scale: Option<f32>` (and `--ui-scale` for tooling) overrides the monitor's scale
  factor for the runtime; the physical window stays the display's full size, so the logical
  viewport becomes 1280×720.
- Every `window.scale_factor()` that feeds the runtime (creation, resize, input, presented surface)
  reads the override through one helper, so input and drawing never disagree.
- A headless scene uses the same override, so a `--scale 3 --size 1280x720` snapshot is exactly a
  4K TV frame.

App-side, one token changes: the page gutter goes from 40 to **64 pt** in Big Screen, the 5 % safe
margin. Focus rings, radii and card sizes are physical-scale copies of the desktop values.

## 4. Focus and navigation

One focus system, two inputs.

- **Initial focus**: a new public `Tx::focus_key(key)` selects the node with that key and scrolls
  it into view. Each screen declares its first key; Home focuses the first Continue Watching card
  once the rail exists, Library the first card, Detail the primary action, Search the field,
  Settings the first row, Live TV the first channel.
- **D-pad / left stick** moves inside the focused group (Rune's roving focus).
- **LB / RB** move between groups, in reading order: rail → toolbar → content → panels.
- **Back (B)** is the existing back stack; at a top-level screen it returns focus to the rail.
  Scroll and focused key are restored, which the app already stores.
- **Start** opens the context action for the focused item (card menu) or the player's gear.
- Hover-only actions get focus paths: remove-from-resume via the card menu; trickplay preview via
  the chapter list; scrub preview via left/right seek while the OSD is open.
- The focused item is always visible: Rune already scrolls keyboard focus into view. The TV layout
  pads rails vertically so the 1.05 focus scale cannot be clipped.

## 5. Controller

Controller input lives in the app (`crates/runefin/src/pad.rs`), not in Rune. Rune carries only the
two primitives the app needs: `AppHandle::key(KeyEvent)` queues a key from a background source and
wakes the loop, and `Tx::focus_key(key)` focuses and reveals a named node. `gilrs` 0.11 polls on
one worker thread; events become the same `KeyEvent` the keyboard produces and go through the
existing focus/shortcut path — no second input model and no UI-thread work.

Mapping (gilrs standard gamepad):

| Control | Key | Effect |
|---|---|---|
| D-pad / left stick | arrows | move focus; seek/volume in the player |
| A (South) | Space | activate; play/pause in the player |
| B (East) | Escape | back / close; the player's OSD first |
| Start | ContextMenu | card menu; player gear |
| LB / RB | Shift+Tab / Tab | previous/next focus group |
| Back/Select | Escape | back |

- Repeat: 400 ms initial delay, 80 ms interval for directions; a stick re-arms only after it
  returns inside the deadzone or turns to another axis.
- A sends Space rather than Enter: Space is the app's universal activate and the player's
  play/pause, while Enter only reaches default buttons and rows.
- X/Y are deliberately unbound. Letter shortcuts (`c`, `f`) are remappable, and a controller
  press would type into Search when no player is open; the player's Start → gear reaches
  subtitles and fit without that collision.
- Steam Machine: add Runefin as a non-Steam game with `--big-screen`; leave Steam Input off so the
  physical pad reaches `gilrs`. If Steam Input is on, it must present a standard virtual pad
  (Xbox layout) — test both on the machine. Steam+X opens the on-screen keyboard for Search and
  Settings. `RUNEFIN_GAMEPAD=0` disables the listener.

## 6. Screen pass

| Screen | Big-screen changes |
|---|---|
| Home | rail scaled; hero band and See all at 1.3 type; first card focused; 3 cards per rail at 1080p |
| Library | 1.5× cards → fewer columns; toolbar keeps two rows; letter rail larger; focus ring clear of clips |
| Detail | poster/hero at 1.3; action row wraps less; season chips and cast scale; primary focused |
| Search | field and result rails scale; keyboard via Steam; hints list |
| Settings | Appearance row for Interface; rows scale; switches/chevrons stay; 280 rail |
| Live TV | channel rows scale; add the missing focus group; guide cells focusable |
| Seerr | Discover/Requests rails and request sheet scale |
| Player | OSD chips 1.3×; skip pill; panels; chapter list as the trickplay replacement; Enter = pause |
| Connect | wordmark and fields scale; focus starts on Server |

## 7. Milestones

1. **M1 — mode plumbing.** `Config.ui_mode`, `--big-screen`, `RUNEFIN_UI`, Settings row, Rune
   `ui_scale` + safe gutter, fullscreen + cursor, 1920×1080 and 3840×2160 snapshots of
   Home/Library/Detail.
2. **M2 — screen pass.** Every screen in §6; snapshots at 1080p and 4K; 130 % text; no hover-only
   loss.
3. **M3 — focus.** `focus_key`/`focus_first` in Rune (fork rev bump), initial focus per screen,
   tab order and group hints, back restoration.
4. **M4 — controller.** Rune `AppHandle::key` + `Tx::focus_key`, `gilrs` in the app, mapping and
   repeat, mapping unit tests, Steam checklist, docs.
5. **M5 — verification.** Snapshot matrix, input tests, `check.sh`, live capture at 1080p, Steam
   Machine manual pass.

## 8. Tests

- Unit: gamepad mapping (pure function), repeat timing, `focus_key` resolution, big-screen
  route focus.
- Runtime: a big-screen launch focuses the first card; D-pad moves within a rail; RB changes
  group; B restores the prior screen and focus.
- Snapshots: `*-big` at 1920×1080 and `*-big-4k` at 3840×2160 for Home, Library, Detail, Search,
  Settings, Live TV, Player, Connect; a 130 % text variant.
- Manual on the Steam Machine: launch, pad only, play an episode, back out, search with Steam's
  keyboard.

## 9. Risks

| Risk | Mitigation |
|---|---|
| Rune's group focus still needs shoulders between rails | Design the rails as one group, show LB/RB hints in the TV chrome |
| `gilrs` device mappings vary | Test Xbox and DualSense; gilrs' SDL mapping covers both |
| Steam Input hides the physical pad | Document "native mode: Steam Input off"; test the virtual-pad path |
| UI scale desyncs input and drawing | one helper for every scale read; pointer tests at 1.5 and 3 |
| Fork API churn | Keep `focus_key` + gamepad small, separate commits, rev bump with snapshots |
| Long focus runs in the rail | Collapsed rail option in Big Screen after the first pass if it proves slow |

## 10. Open questions the user may answer later

1. Which controller is on the Steam Machine (Steam Controller / Xbox / DualSense)? The mapping is
   standard; the labels in the hints change.
2. Is the app already installed there, or is packaging part of the next slice?
3. Is a top navigation bar wanted eventually (Netflix 2025) or is the left rail the identity?
