# Steam Machine Game Mode pass — 2026-11-26

What the first live Game Mode test actually ran, the three symptoms it showed, and the fixes.
Written from the machine over SSH while it was in its custom Game session.

---

## 1. The installed build was 95 commits behind

`flatpak info io.github.lifk.Runefin` reported **0.1.30**, built **2026-11-11**, from commit
`9ecfedc` (not a commit in this repository). `main` is **0.3.2**, and `git rev-list --count
v0.1.30..HEAD` is **95**. The running process was the release build:

```text
SteamLaunch AppId=4224007420 -- /usr/bin/flatpak run ... --command=runefin \
    io.github.lifk.Runefin --big-screen
```

So the pass described 0.1.30, not the code in this tree. Two of the three symptoms are already
fixed in `main`; the third is a real regression that this change fixes.

## 2. Symptom → cause

### A. A × on the focused Continue Watching card

v0.1.30 draws the remove button for every resume card:

```text
crates/runefin/src/screens.rs (v0.1.30)
    let remove_id = item.id.clone();
    art = art.child(el().key(format!("resume-remove-{remove_id}")) ...)
```

Commit `660afc2` (2026-11-11, the TV focus model) put it behind `if !s.big_screen`: on a
television the remote opens the card menu instead, and a focused card must not grow an × the
arrows cannot reach. The installed 0.1.30 predates that commit, which is why the × appeared on a
`--big-screen` run. `main` does not draw it. A test now pins it:
`a_big_screen_resume_card_has_no_pointer_only_remove_button`.

### B. Pixelated artwork

The request widths were constants: `POSTER_WIDTH` 360, the Home backdrop 1280, the resume
backdrop 640, the detail poster 520. The big-screen design height is 720, so at 3840×2160 the
runtime scale is **3**: a 304 pt resume card covers 912 physical pixels, a 176 pt poster 528, and
the backdrop all 3840. A 360 px poster was therefore stretched 2.5×, and the 1280 px backdrop
3×. Text stayed crisp because it was drawn at 3×; the art was not.

The fix adds `State::image_scale`, sampled from `App::on_window_geometry`:

- big screen: `physical_height / 720` (3.0 at 4K, 1.5 at 1080p);
- desk: `physical_height / logical_height`, or the compositor scale before the first layout.

Every image key and request is now `image_width(base) = base × scale`, capped at 3840. Width is
part of the cache key, so when the factor changes the app drops the old art and re-requests the
current route. A 1× session keeps the old numbers exactly. The design contract is in
`docs/design.md` §5 (Images).

### C. Top and bottom black bars

Two separate causes stacked.

1. **Game Mode's canvas was 16:10.** `/usr/lib/steamos/gamescope-session` hardcodes the Deck
   panel:

   ```text
   -w 1280 -h 800 ... -O '*',eDP-1
   ```

   The connected output is HDMI-A-1 at 3840×2160. Gamescope composed a 1280×800 (16:10) canvas
   and presented it on the 16:9 TV, so 16:9 content (Steam's pages and the app) sat inside a
   16:10 frame with bars above and below, and the whole canvas was upscaled. The screenshot
   API returns the 3840×2160 *output* frame, which is why the bars were visible in a 4K PNG.
   `packaging/modes/gamescope-session.sh` now rewrites `-w/-h` to the connected panel's
   preferred mode and `-O` to that connector, and `steam-install.py --three-modes` overrides
   the user unit's `ExecStart` with it. A Deck panel resolves to 1280×800, so nothing changes
   there. Verified: after the change Steam Big Picture fills the 16:9 screen with no bars.

2. **The app kept its desk window size.** A `--big-screen` launch used the saved 1365×768
   window, so gamescope had a 768-line surface to scale over a 2160-line TV. Rune's
   `take_full_screen_request()` consumes the request even when no native window is shown yet,
   and the first `poll` can run that early, so the request was silently dropped. The
   window-geometry sample now asks for fullscreen again, where a window is known to exist, and
   resizes the surface to the output's size. On the machine the sample reports
   `physical=(3840, 2160) factor=1`; the layout viewport is `1280×720` and the image factor is
   `3`, which is the intended 4K big-screen render. The resize compares the window's point size
   with the output's and corrects it in both directions, so a saved 4K size on a 1080p TV is
   reduced too. The image scale follows the sample, so the same launch asks for 1080 px posters
   instead of 360.


## 3. Machine facts worth keeping

- The SteamOS game session hardcodes a Deck canvas:

  ```text
  /usr/lib/steamos/gamescope-session
      -w 1280 -h 800 ... -O '*',eDP-1
  ```

  but the connected output is **HDMI-A-1** at 3840×2160 and there is no `eDP-1`. `-O` is
  `--prefer-output`, a preference list, so the missing connector falls back to the only output
  but the 16:10 canvas remains. The unit drop-in now replaces `ExecStart` with the mode-aware
  wrapper; do not edit the immutable `/usr` script.
- `gamescopectl` (gamescope 3.16.30.1) accepts `screenshot <path>`; the PNG appears a moment
  after the command returns. `gamescopectl help` lists convars, not commands.
- Current custom session chain: `/etc/sddm.conf` → `/etc/runefin-modes/runefin-modes.desktop` →
  `~/.local/share/runefin/modes/session.sh`; `session.log` shows `session start: game` and
  `gamescope-session.target` active.
- `runefin-mode game` still ends the session with `loginctl terminate-session`, which SDDM
  treats as an auth failure and leaves the greeter. Use `sudo systemctl restart sddm` until the
  switch is patched.

## 4. Follow-ups

1. Deploy 0.3.2 to the machine (`packaging/flatpak/deploy.sh`) and rerun the three checks.
2. Patch `runefin-mode` to restart SDDM (or perform a clean Plasma logout) instead of
   `loginctl terminate-session`.
3. Point one Steam shortcut at `/home/deck/.local/share/runefin/modes/runefin-steam.sh`, apply
   art to that appid, and remove the duplicate Flatpak tile.
4. Give the Flatpak `xdg-download` access (or route downloads through a portal).

## 5. Second live pass — keyboard and library columns

Two more quirks showed up while testing the 4K build on the TV.

- **The keyboard took over on focus.** `KeyboardMode::Auto` opened the sheet as soon as a
  text field received focus. The library search box sits in the same D-pad row as the cards,
  so arrowing across it trapped the reader: Back hid the sheet, focus returned to the field,
  and the next frame opened it again. Auto now opens only from the field's own action, Enter
  on the field or Menu/Start; focus alone never opens it. On demand keeps Menu/Start only, and
  Off keeps a physical keyboard. The connect form's Enter and the library search's Enter both
  ask for the sheet in Auto and keep their own submit in the other modes.
- **The last library column was clipped.** The grid computed its column count from the body's
  measured width, but the A-Z letter rail and the page gutter are laid out beside the
  scrollable viewport. On the 354-item Movies page that made five cards claim a width five
  could not fit, and the page has no horizontal scroll to reveal the fifth. The count now
  comes from the viewport's own width, with the last gap included, so the row ends inside the
  rail.
- **The sidebar focus ring read as a white line.** Focus drew the near-white 2 pt ring, which
  is six physical pixels at 4K, around each 36 pt nav row as the D-pad moved. The sidebar now
  answers focus with the same contained wash the cards use (14 % white, 20 % on the selected
  pill) and drops the ring; the desk layout keeps its ring. `focus-sidebar-big.png` pins the
  new look.
