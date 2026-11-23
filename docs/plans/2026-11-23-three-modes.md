# Three modes and an in-app keyboard — research and plan

**Status:** proposed, 2026-11-23. Replaces the media-profile version of this plan and the attached
`runefin-theater-mode-design-2026-11-23.md`.

**The constraint that decides the architecture:** Desktop Mode stays a real Plasma desktop, with no
Runefin process and no Runefin autostart. Media is a third mode, not an app layered over Desktop.
That leaves three modes — Desktop, Gaming, Media — and requires a session dispatcher. This plan keeps
that dispatcher as small as it can be: **one root-owned SDDM drop-in** (plus the session file only if
S2 shows SDDM cannot read one from home), everything else user-owned.

**Definition of done.** On a Steam Machine with no keyboard attached: Desktop Mode is stock Plasma
with no Runefin process in `pgrep`; Gaming Mode is stock Steam; Media Mode is Runefin fullscreen with
no Plasma session behind it, and a fresh user can connect and play using only the D-pad and the
in-app keyboard. All three modes switch in both directions without a password, greeter or TTY; ten
round trips leave no orphan process and no lost resume position. A `kill -9` in Media lands in
Desktop, not a black screen. `runefin-mode restore` removes the one root file and returns the machine
to stock SDDM. Gaming Mode still exposes no HDR for a native Wayland Flatpak; Media Mode does, via
KWin, once the TV pass passes.

---

## 1. Verdict

The previous version of this plan made Media "Desktop Mode with Runefin running fullscreen". That was
the cheap version, but it fails the requirement: it takes over Desktop. The user wants Desktop Mode
to remain Desktop Mode.

Three real modes require a custom session. There is no supported SteamOS API for a third mode; the
options are a custom SDDM session or three separate session files. The original attached draft chose
one session with a marker; that is still the right mechanism, because a single session is what makes
the switches user-level. The draft's implementation was the problem, not the mechanism: it added a
pin unit that re-wrote `/etc` on every boot, measured health by process uptime, and hard-coded
uninstall. This plan keeps the one-session idea and removes those parts.

The smallest SteamOS touch that gives three real modes is:

- **one root-owned file**, `/etc/sddm.conf.d/zzz-runefin-modes.conf`;
- **one user-owned session file and dispatcher** under `~/.local/share/runefin/modes/`;
- **one user command**, `runefin-mode`, for switching and recovery;
- **one user systemd service** that records the mode when Steam's own switch starts a stock session.

Nothing under `/usr` is edited in the normal install; the S2 fallback may add one session file under
`/usr/local`. No `/etc` file is rewritten on every boot. No Valve script is touched. No `pkexec` or
polkit is needed at runtime; only `install`, `repair` and `restore` use one `pkexec` to add or remove
the SDDM drop-in.

The keyboard is still the first milestone and is independent of the session work. It is what makes a
keyboard-free Media mode possible at all: the Steam overlay keyboard is unreliable for a sandboxed
Flatpak and absent when Steam is not running.

---

## 2. Steps

- [ ] 1. **Keyboard first.** Target registry, `TextEdit` insertion, change hooks, bottom sheet, tests,
  snapshot. Prove the connect form, search, password and go-to fields with a D-pad only.
- [ ] 2. **Mode skeleton.** `runefin-mode` (marker, switch, recovery), the dispatcher, the one SDDM
  drop-in, and a Desktop/Gaming/Media switch matrix with a placeholder Media client. No app changes.
- [ ] 3. **Media app.** `--media`, the supervisor, first-frame ready file, exit 42/43 with a graceful
  flush, Modes page, and the fallback to Desktop after three failures.
- [ ] 4. **Mode sync and repair.** `runefin-mode sync` service for Steam's own switches, `status`,
  `repair`, `restore`, `RECOVERY.txt`, and a TTY recovery path.
- [ ] 5. **Hardware pass.** Steam Machine, no keyboard: three-mode round trips, HDR, CEC, suspend,
  audio sink loss, 24p if KWin allows it.
- [ ] 6. **Docs and defaults.** Update `docs/features.md`, the Flatpak README, the 2026-11-17 SteamOS
  plan, and record the HDR/24p choice and the fallback policy.
- [ ] 7. **Optional upstream.** A generic Rune on-screen keyboard, only after a second app needs it;
  standalone pin bump, not blocking.

---

## 3. What changes from both earlier drafts

| Concern | Attached Theater draft | Previous media-profile plan | This plan |
|---|---|---|---|
| Desktop Mode | Wrapped by the custom session | Runefin autostarts over Plasma | **Stock Plasma, no Runefin** |
| Media Mode | Custom KWin kiosk | Desktop + fullscreen app | **Custom KWin kiosk, no Plasma** |
| Session files | One dispatcher + pin unit | None | One dispatcher, no pin unit |
| Root files | Drop-in + systemd unit + pin | None | **One drop-in** |
| Mode state | Marker + Valve state | SteamOS login mode + wrapper | Marker, sync service records stock sessions |
| Media failure | Crash counter, otherwise black | Plasma behind the app | Supervisor falls back to Desktop |
| Health | 30 s process uptime | n/a | First-frame ready file |
| Uninstall | Hard-coded game session | Remove user files | Remove the one drop-in; stock returns |
| Keyboard | Steam overlay | In-app | In-app, first milestone |

The attached draft's six mechanical blockers are addressed directly:

1. **Drop-in precedence.** The file is `zzz-runefin-modes.conf`, which sorts after
   `zz-steamos-autologin.conf`; it sets `User`, `Relogin`, `Session` and `SessionDir` itself. There is
   no pin unit racing SteamOS every boot.
2. **Healthy restarts.** The app writes a first-frame ready file; the supervisor resets failures on
   it. No 30-second process-uptime proxy.
3. **One state.** The marker is the mode. A user service records the mode when Steam's own switch
   starts a stock session; `status` reports if SDDM's effective session is not ours.
4. **A real watchdog.** A missing ready file within 15 seconds kills the app and counts a failure;
   three failures in ten minutes write `desktop` and exit.
5. **Exit 42/43 flush.** The app runs the same stop-report and checkpoint path as Quit, with a bounded
   wait, before exiting.
6. **Uninstall.** Removing our one drop-in reveals SteamOS's own config; no Valve file is edited or
   restored by hand.

---

## 4. The three modes

| Mode | Session body | What runs | Enter from | Leave by |
|---|---|---|---|---|
| **Desktop** | `startplasma-wayland` | Stock Plasma; no Runefin, no autostart | Default at login; `runefin-mode desktop`; Steam's Switch to Desktop | `Runefin Media` / `Runefin Game` desktop entries; Steam |
| **Gaming** | `gamescope-session` | Stock Steam Game Mode; no Runefin | `runefin-mode game`; Steam's Switch to Game Mode | Steam's Switch to Desktop; `Runefin Media` Steam tile |
| **Media** | `kwin_wayland --exit-with-session supervisor` | Runefin fullscreen; no Plasma | `runefin-mode media`; desktop entry; Steam tile | Runefin Modes page (Desktop/Game) |

"Media" is a kiosk by design: a TV shell needs the whole screen, and Plasma's panel, notifications and
portals are not part of the film path. "Desktop" is the opposite: stock Plasma, no app taking it over.
The dispatcher is the only session SDDM starts; each body is a normal component, not a reimplementation.

The dispatcher defaults to **Desktop** if the marker is missing or invalid. A corrupted marker must
never strand the machine in a kiosk.

---

## 5. What we touch

**Root, once:**

- `/etc/sddm.conf.d/zzz-runefin-modes.conf` — `[Autologin] User=<user>`, `Relogin=true`,
  `Session=runefin-modes.desktop`; `[Wayland] SessionDir=` with the stock directories plus the user
  directory.
- That is the entire root surface in the normal install. The installer backs up nothing in Valve's
  files and removes only its own file on `restore`; the S2 fallback adds one `.desktop` under
  `/usr/local/share/wayland-sessions`, still with no Valve edits.

**User:**

- `~/.local/share/runefin/modes/runefin-modes.desktop` — the SDDM session entry.
- `~/.local/share/runefin/modes/session.sh` — the dispatcher.
- `~/.local/share/runefin/modes/supervisor.sh` — the Media app supervisor.
- `~/.local/bin/runefin-mode` — install/status/repair/restore/switch/sync.
- `~/.config/systemd/user/runefin-mode-sync.service` — records stock sessions started by Steam.
- `~/.local/share/applications/runefin-media.desktop` and `runefin-game.desktop` — desktop entries.
- Optional Steam library tiles through `scripts/steam-install.py`, using the existing wrapper.

**Not touched:**

- `/usr`; `/usr/local` only in the S2 fallback that puts the session `.desktop` there.
- `gamescope-session`, `startplasma-wayland`, `steamosctl`, `devkit-steam`, Steam UI, Decky,
  `shortcuts.vdf` (except the existing installer's own tiles), the Flatpak manifest.
- No `flatpak-spawn --host`; the Flatpak has no new filesystem access.

If `steamosctl get-valid-desktop-sessions` accepts a custom session on the target, the plan should use
it and drop the root drop-in entirely. That is G0.1; otherwise the one drop-in is the minimum.

---

## 6. Architecture

```
SDDM
  └── runefin-modes.desktop  (one session, autologin, Relogin=true)
        └── session.sh  reads ~/.local/state/runefin/mode
              ├── desktop -> exec startplasma-wayland
              ├── game    -> exec gamescope-session
              └── media   -> exec kwin_wayland --exit-with-session supervisor.sh
                               └── flatpak run io.github.lifk.Runefin --media
                                     exit 0  -> restart the app
                                     exit 42 -> marker=game;    exit; KWin exits; SDDM re-runs
                                     exit 43 -> marker=desktop; exit; KWin exits; SDDM re-runs
                                     crash   -> failures/timestamps; 3 in 10 min -> marker=desktop
```

`runefin-mode media|desktop|game` writes the marker atomically and terminates the user's current
graphical session (`loginctl terminate-session`). `Relogin=true` makes SDDM start the same session
again, and `session.sh` reads the new marker. No root, no greeter.

A user systemd service, `runefin-mode-sync`, runs in stock Desktop/Gaming sessions and writes the
marker to match the session it finds. That covers Steam's own "Switch to Desktop" and "Switch to Game
Mode"; the dispatcher then follows the same mode at the next login. It skips the custom session so it
cannot overwrite Media with Desktop.

---

## 7. Switching, failure and recovery

### 7.1 The switch command

```
runefin-mode media|desktop|game
```

- Validates the word; anything else is refused, never guessed.
- Writes `~/.local/state/runefin/mode` with `tmp` + `rename`.
- Finds the active graphical session: `XDG_SESSION_ID` if set, otherwise the user's active
  `loginctl` session; calls `loginctl terminate-session`.
- If the call fails, prints the marker path and the TTY command; does not pretend to have switched.
- From a TTY, the same command terminates the graphical session; it needs no root for the user's own
  sessions.

### 7.2 From Media

The app's Modes page:

- **Switch to Desktop** — exit 43; supervisor writes `desktop`; KWin exits; SDDM starts Desktop.
- **Switch to Game** — exit 42; supervisor writes `game`; KWin exits; SDDM starts Gaming.
- **Restart app** — exit 0; supervisor restarts. It never exits to a black screen.

Each exit first runs the graceful stop path: final position reporting and checkpoint flush, with a
bounded wait (≤1.5 s). A direct `std::process::exit` that skips `on_quit` would lose the resume
position; the earlier draft made that mistake.

### 7.3 From Desktop

- `Runefin Media` desktop entry → `runefin-mode media`; the session ends; SDDM starts Media.
- `Runefin Game` desktop entry → `runefin-mode game`; the session ends; SDDM starts Gaming.
- Normal Plasma use is untouched; Runefin is not running and not autostarted.

### 7.4 From Gaming

- A `Runefin Media` Steam tile runs `runefin-mode media`; the game session ends; SDDM starts Media.
- A `Runefin Desktop` tile runs `runefin-mode desktop`; the same for Desktop.
- Steam's own **Switch to Desktop** is best-effort. The sync service records a stock session that
  actually starts, but if our SDDM session wins, the switch never reaches a stock session and the
  marker stays `game`; the dispatcher would start Gaming again. S7 decides this. If it is
  intercepted, the tiles/entries are the only supported switch and the docs say so; do not claim the
  native switch. `runefin-mode status` reports when Steam has changed SDDM's effective session, and
  `repair` puts the drop-in back. If S7 fails, the README says "use the Runefin Desktop tile", not
  Steam's button.

The Steam tiles and desktop entries are **required for the deterministic matrix**, not an optional
extra; the existing `scripts/steam-install.py` owns the shortcut machinery. They are host scripts,
not Flatpak permissions. Keep the existing **Runefin** tile distinct from the new **Runefin Media**
tile: the former launches the app inside Gaming Mode (SDR, no HDR, an app, not a mode); the latter
switches the machine to Media. The labels must say which is which.

### 7.5 Health and fallback

The Media supervisor is a watchdog, not an uptime calculator:

- Before launch, remove the ready file (the app writes it after its first presented frame inside its
  Flatpak state directory).
- Launch `flatpak run … --media`.
- If ready appears within 15 s, reset the failure count and wait for the exit.
- If it does not, kill the app and count a failure.
- A crash within 60 s of ready counts; a crash after a longer healthy run restarts once and clears the
  count.
- Three failures in ten minutes write `desktop` and exit. Desktop is the safe fallback: visible,
  normal, and one command from Media again.
- A session-level counter catches KWin failing before the supervisor ever runs: before KWin starts,
  the dispatcher records a `media-pending` timestamp; the supervisor's ready file is the proof that
  the boot reached a frame. A pending boot with no newer ready file is a failure; after three, the
  dispatcher writes `desktop` and starts Plasma.

The ready file is a host-visible file in the Flatpak's state directory
(`~/.var/app/io.github.lifk.Runefin/...`), not a sandbox escape. Its exact path is a G0 item.

### 7.6 Recovery

- If Media is black, `Ctrl+Alt+F3`, log in, `runefin-mode desktop`, then reboot or let
  `loginctl` restart the session.
- If the dispatcher itself is broken, `runefin-mode restore` removes the one drop-in and returns SDDM
  to stock; the user can boot Desktop/Gaming normally.
- `RECOVERY.txt` in the state directory repeats those commands.
- `runefin-mode status` reads the effective SDDM config and says whether our file wins, which session
  will autologin, and whether the custom session is present. It needs no root.

### 7.7 Install, repair, restore

```
runefin-mode install [--default media|desktop|game] [--dry-run]
runefin-mode status [--json]
runefin-mode repair
runefin-mode restore
```

- `install` writes the user files and, with one `pkexec`, the SDDM drop-in. `--default` sets the
  marker; the ordinary default is `desktop`, so a machine that has not opted in still boots to a
  normal desktop. A media box passes `--default media`.
- `repair` re-writes only the drop-in from the user template; it does not touch Valve's files. It is
  the after-update command.
- `restore` removes the drop-in with one `pkexec`, removes its user files, and leaves Runefin, the
  Flatpak and the Steam tile alone. Because the drop-in is additive and sorts last, removing it
  reveals SteamOS's own configuration.
- `status` is the support artifact: effective session, marker, presence of files, Flatpak version,
  `steamosctl` availability, and the next boot target.

---

## 8. In-app on-screen keyboard

### 8.1 Why it is the first milestone

The first-run path in `docs/research/21` is blocked: a controller-only user cannot fill the server
field, and the Steam keyboard is unverified for a sandboxed Flatpak. In Media there is no Steam at
all. Without an in-app keyboard, "no keyboard attached" is only true after a setup that needed one.

The keyboard also helps the existing Steam tile and a manually launched big-screen app. It is app
work, not SteamOS work.

### 8.2 UX

- A bottom sheet over a dimmed page, 40–45% of the screen high. The field being edited stays visible
  above it, repeated as a one-line target strip: label, value, caret, secure marker.
- A 10-foot grid: large keys, one focus ring, D-pad navigation, OK to type, B/Esc to hide, a visible
  Enter key that submits. Keys are at least 48×48 at the 1280×720 design height.
- Pages: `abc` (lowercase QWERTY with `, . - : / @` on a second row), `ABC` (shift), `123` (digits and
  symbols), and a URL-friendly page with `: / . _ - ~ ? & = % @`. Space, Backspace, Enter, Tab, Hide
  and a `Show` toggle for secure fields.
- The target strip says what is being edited ("Server URL", "Password", "Search").
- Auto-open in Media and big-screen when a text field gains focus; a Hide key dismisses it for that
  field. A Settings → Interface row ("On-screen keyboard: Auto in big screen / On demand / Off")
  keeps a physical-keyboard user from being surprised. Desktop Mode defaults to On demand; Media
  defaults to Auto.
- No clipboard for passwords; no last-character echo in a subtitle or toast.

### 8.3 State and targets

Add `crates/runefin/src/state/keyboard.rs`:

```rust
pub enum TextTarget {
    ConnectServer, ConnectUser, ConnectPassword,
    Search, LibrarySearch, SettingsSearch, SeerrSearch,
    OmdbKey, SeerrUrl, SeerrKey, SeerrUser,
    PlaylistName, GotoTime, DownloadsPath,
}

pub struct Keyboard {
    pub open: bool,
    pub target: Option<TextTarget>,
    pub page: KeyboardPage,
    pub shift: bool,
    pub reveal: bool,
}
```

- There are 14 fields today: three connect fields through the `field()` helper, and eleven direct
  `text_input` sites. A single wrapper records the target when a field is focused, so a new field
  cannot silently miss the keyboard.
- The target is taken from `tx.focused()` in `poll` through `target_for_key()`. Use the runtime's key,
  not only `State::focused_key`: several fields never call `note_focus`.
- `State::keyboard_insert(&str)` matches the target, calls the field's `TextEdit::insert`, then calls
  that field's change hook. Search fields call `search_changed()` / `library_search_changed()` /
  `seerr_search_changed()`; connect fields only repaint. Programmatic `TextEdit` edits do not fire
  `on_input_change`, so the hook is explicit.
- `keyboard_backspace`, `keyboard_enter`, `keyboard_clear`, `keyboard_close` and `keyboard_next` are
  the other operations. `keyboard_enter` runs the field's submit action (connect, search, go-to) and
  closes the sheet.
- `TextEdit` is already public (`rune::TextEdit`: `insert`, `replace_text`, `set_caret`, `clear`,
  `secure`, `set_revealed`). No new Rune API is required for this design.

### 8.4 Focus, caret and insertion

The keyboard is a modal focus layer. On open:

1. record the field key and set the target `TextEdit.focused = true` so its caret stays visible;
2. move Rune focus to the first key of the sheet;
3. keep the field model in app state; the target strip redraws from it.

Each key mutates the target `TextEdit` directly; it does not send a synthetic key to a field that no
longer has Rune focus. This avoids the focus round-trip and makes every key deterministic.

On close, `tx.focus(field_key, true)` returns focus to the field. `Esc`/B hides; `Tab` or Next moves
to the next text field and reopens the sheet with the new target.

The alternative — keep the field focused and send `Key::Text` through `Tx::send_key` — is simpler in
theory but splits the D-pad between the field (Left/Right for the caret) and the keyboard grid. The
modal/direct-insert model gives one navigation model. G0 S7 tests it; if direct insertion cannot draw
reliably without moving focus, fall back to the send-key model and field interception.

### 8.5 Secure fields

- Password fields keep `TextEdit::secure()`; the sheet shows a masked value and a length, never the
  plaintext.
- A `Show` key toggles `set_revealed` while the sheet is open, and resets when it closes.
- No clipboard, no last-key bubble, no screen-reader announcement of individual characters.
- The accessibility name is "Password"; the value is reported as masked.

### 8.6 Input and accessibility

- The keyboard is a normal `El` focus group: every key has `.key("osk-a")`, `.role("button")`,
  `.label("A")`, `.focusable(true)` and a focus visual. Rune's D-pad spatial navigation moves between
  keys; no bespoke key router is needed inside the grid.
- The app's `tv_key` leaves Up/Down/Left/Right/OK to Rune while focus is inside the sheet, and
  intercepts Escape and the page switch. The sheet is a modal focus group, so page zones and the
  player do not receive those keys; the OSD and card menu are inert while it is open.
- The pad maps Start to ContextMenu and has no X/Y; page switching is a visible `123`/`ABC` key, not a
  shoulder chord. The visible Enter key submits.
- CEC remote keys arrive as the same key events as D-pad/OK/Back in `tv_input`; the keyboard needs no
  CEC-specific code.
- `every_screen_names_every_control` covers the sheet; `tv_reach_tests` gets a keyboard layer case and
  every key must be reachable from the first focus.
- A screen reader hears the sheet as a dialog with the target field's name; it does not hear each
  typed character. The field value is read when focus returns.

### 8.7 Opening and first run

- In Media and big-screen, focusing a text field opens the sheet automatically. Hide closes it for
  that field; refocusing or the "Keyboard" chip opens it again.
- On the connect form, the first focus is the server field and the sheet is already open: type the
  URL, move to username and password with Tab/Next.
- A visible "Keyboard" action sits beside the field for pointer users and On-demand mode.
- The Steam tile in Gaming Mode gets the same sheet; the app never calls `steam://open/keyboard`, so a
  broken overlay injection is no longer a setup blocker.

### 8.8 Where the code lives

- **Runefin first.** `screens/keyboard.rs` draws the sheet; `state/keyboard.rs` holds the target, page
  and operations; each field site gets the target wrapper. No new dependency, no Rune pin bump.
- **Rune upstream later, only if it earns it.** The reusable part is the layout/navigation, not the
  app's target list. A generic `rune-kit` keyboard would need a `TextEdit` accessor API; that is a
  separate upstream contribution and a standalone pin bump. Do not block this feature on it.
- If direct insertion proves awkward, the fallback is a small `Tx::insert_text`-style Rune helper;
  that is a G0 result, not an assumption.

---

## 9. SteamOS details

### 9.1 The one drop-in

```
# /etc/sddm.conf.d/zzz-runefin-modes.conf
[Autologin]
User=<user>
Relogin=true
Session=runefin-modes.desktop

[Wayland]
SessionDir=/usr/local/share/wayland-sessions,/usr/share/wayland-sessions,<home>/.local/share/runefin/modes
```

- The `zzz` name sorts after `zz-steamos-autologin.conf`; SDDM reads later files last.
- `User=` is required for autologin and was missing from the attached draft.
- `Relogin=true` is what makes "session ends → same session starts again" work without root.
- `SessionDir` replaces the default list, so all stock directories are repeated.
- Removing the file reveals SteamOS's own configuration; no Valve file is edited.
- If SDDM refuses a session file under home, install the `.desktop` in
  `/usr/local/share/wayland-sessions` (one more root file) and keep the session script in home. G0.2.

Target SteamOS **3.9.5 SDDM**. SteamOS 3.10's `plasmalogin` is a follow-up; the plan must not guess
its keys. `steamosctl get-valid-desktop-sessions` is tried first; if it accepts a custom session, the
drop-in is unnecessary.

### 9.2 The session file and dispatcher

`~/.local/share/runefin/modes/runefin-modes.desktop`:

```ini
[Desktop Entry]
Name=Runefin Modes
Comment=Desktop, Gaming or Media
Exec=/home/<user>/.local/share/runefin/modes/session.sh
Type=Application
DesktopNames=KDE
```

`session.sh`:

```sh
#!/bin/sh
set -eu
state="$HOME/.local/state/runefin"
target="$(cat "$state/mode" 2>/dev/null || echo desktop)"
case "$target" in media|desktop|game) ;; *) target=desktop ;; esac
mkdir -p "$state"
printf '%s\n' "$target" > "$state/mode"

case "$target" in
  desktop)
    export XDG_SESSION_DESKTOP=plasma XDG_CURRENT_DESKTOP=KDE
    exec startplasma-wayland
    ;;
  game)
    export XDG_SESSION_DESKTOP=gamescope XDG_CURRENT_DESKTOP=gamescope
    exec gamescope-session
    ;;
  media)
    export XDG_SESSION_DESKTOP=runefin-modes XDG_CURRENT_DESKTOP=KDE RUNEFIN_MEDIA=1
    exec kwin_wayland --exit-with-session "$HOME/.local/share/runefin/modes/supervisor.sh"
    ;;
esac
```

Paths are probed by the installer (`command -v kwin_wayland`, `gamescope-session`,
`startplasma-wayland`); the script is generated or the installer refuses with the exact package.
The script also checks at each start, after an update can remove a package: if `kwin_wayland` or
`gamescope-session` is missing, it writes `desktop` and starts Plasma instead of looping on a missing
binary.

### 9.3 The sync service

`~/.config/systemd/user/runefin-mode-sync.service`:

```ini
[Unit]
Description=Record the active Runefin mode
After=graphical-session.target
PartOf=graphical-session.target

[Service]
Type=oneshot
ExecStart=%h/.local/bin/runefin-mode sync

[Install]
WantedBy=graphical-session.target
```

`sync` reads the session's `Desktop` property (`loginctl show-session "$XDG_SESSION_ID" -p Desktop
--value`) or falls back to the environment/process names. It skips `runefin-modes` (our dispatcher
already wrote the marker). It writes `game` for `gamescope`/Steam and `desktop` for Plasma. It runs in
both stock sessions because `systemd --user` survives where XDG autostart does not; the exact
detection is a G0 item because the user manager's environment is not the session's. If Gaming does
not start `graphical-session.target`, a user timer or a Plasma autostart entry is the fallback; S7
decides. The unit is a recorder, not a launcher; it never starts Runefin.

This is what keeps Steam's own **Switch to Desktop** from leaving a stale marker.

### 9.4 Display, audio, power and input

- **HDR:** Media uses KWin, the only path that can give a native Wayland client an HDR surface. The
  app's scRGB path exists; the TV pass is still open. Gaming Mode remains SDR for a non-Steam Wayland
  client.
- **24p:** KWin may allow output-mode switching (`kscreen-doctor` or KWin's control) where gamescope
  does not. Test it; if it works, a host helper can set 23.976 on play and restore on stop. If not,
  document 60 Hz and the 3:2 judder. Do not promise HDR and 24p together without a measurement.
- **Resolution:** on the Steam Machine's HDMI 2.0, 4K120 is SDR and 4K60 is the HDR mode. State the
  target explicitly.
- **Audio:** sink selection and sink-loss recovery are still gaps; a media box needs them.
- **Suspend/resume:** the app has no `PrepareForSleep` contract today; add it before calling Media an
  appliance.
- **CEC:** `cecd` runs, but Research 21 records that media transport keys are dropped. A TV remote
  that cannot play/pause/seek is not enough; fix the mapping or document the limit.
- **Steam Controller:** without Steam Input the pad is basic; trackpads, gyro and the Steam/QAM
  buttons are not exposed. D-pad + A/B is enough for the keyboard grid.

### 9.5 Updates, repair and restore

- User files under `/home` survive A/B updates.
- The drop-in may be reset by an update; `runefin-mode status` reads the effective config, and
  `repair` re-adds it with one `pkexec`. No pin unit, no daemon, no per-boot rewrite.
- If `plasmalogin` replaces SDDM, `status` reports unsupported and `repair` stops; that is a new G0,
  not a guess.
- `restore` removes the one drop-in; SteamOS's own session returns. The user files are removed
  separately; Runefin and the Flatpak stay.

---

## 10. G0 — experiments before code

| # | Experiment | Pass | If it fails |
|---|---|---|---|
| S0 | `flatpak run … --media`; app exits 42 and 43; second copy | Codes arrive unchanged; no stolen instance | Fix the ABI before the wrapper |
| S1 | `steamosctl get-valid-desktop-sessions` / `set-default-desktop-session` with the custom session | Custom session accepted and selectable without root | Use the one drop-in |
| S2 | SDDM with the drop-in: custom session in home, `User`, `SessionDir`, `Relogin`; `sddm --test-mode` | Boots the dispatcher; Desktop/Gaming bodies start | Put the `.desktop` in `/usr/local`; one more root file, still no Valve edits |
| S3 | `loginctl terminate-session` + `Relogin=true` from Desktop, Gaming and Media | Same session restarts; no greeter; 10× | Stop; a root helper is not acceptable at runtime |
| S4 | Desktop body: `startplasma-wayland` through the dispatcher | Stock Plasma; no Runefin process or autostart | Fix the dispatcher; Desktop must stay stock |
| S5 | Gaming body: `gamescope-session` through the dispatcher | Steam Game Mode; no Runefin | Fix the dispatcher/paths |
| S6 | Media body: KWin + supervisor; first-frame ready; `kill -9`; hung first frame | Fallback to Desktop after 3 failures; no black screen | Fix the supervisor before Media ships |
| S7 | Steam's own Switch to Desktop/Game Mode with the sync service | Marker follows; next login matches; no loop | Use the tiles/entries as the only supported switch |
| S8 | OSK insertion/focus: open, type, close, physical keyboard after | Text lands in the target; caret correct | Fall back to `Tx::send_key` |
| S9 | OSK with only a CEC remote and only a pad | Full URL + password + search; no Steam keyboard | Fix input before Media |
| S10 | HDR in Media on the TV; the app covers the display | scRGB; panel in HDR; no panel above | SDR only; say so |
| S11 | Suspend/resume and audio sink loss | Playback returns; position truthful | Document as beta gaps |
| S12 | KWin refresh switching, if attempted | 23.976 set and restored; HDR preserved | 60 Hz only |
| S13 | Second user logs in while Media runs; a second Runefin launch | Per-user marker; no duplicate instance or focus theft | Document single-user or namespace the instance |

---

## 11. Milestones

| M | Deliverable | Gate |
|---|---|---|
| **M1** | OSK state, target registry, sheet, connect/search/goto, tests, snapshot | S8/S9; first run with no keyboard reaches Home |
| **M2** | Dispatcher, one drop-in, `runefin-mode`, Desktop/Gaming/Media skeleton; placeholder Media client | S1–S5; Desktop is stock; switches work |
| **M3** | `--media`, supervisor, ready file, exit 42/43 flush, Modes page, fallback | S0/S6; no black screen; 10 round trips |
| **M4** | Sync service, native Steam switches, `status`/`repair`/`restore`, `RECOVERY.txt` | S7; update simulation; TTY recovery |
| **M5** | Hardware pass: HDR, CEC, suspend, audio, 24p if possible | S10–S13; docs state limits |
| **M6** | Optional Rune upstream keyboard | Standalone pin bump; not blocking |

M1 ships on its own. M2 proves the session plumbing with no app changes. M3 is the first real Media.
M4 is the point where the machine survives a SteamOS update and Steam's own switches.

---

## 12. Non-goals and stop conditions

**Not in this plan:**

- `/usr` edits, a systemd system unit, a pin unit, or a per-boot `/etc` rewrite. The only exception is
  the S2 fallback that puts the session `.desktop` in `/usr/local/share/wayland-sessions`.
- A Desktop media profile: that is the previous plan and it takes over Desktop.
- HDR in Gaming Mode for a native Wayland Flatpak.
- 24p + HDR as a guarantee.
- Widevine L1, TrueHD/Atmos, a Steam UI plugin, Decky, a self-updater.
- A full IME; the keyboard covers ASCII/URL/password and the app's search text. Non-Latin is later.

**Stop if:**

- `loginctl` + `Relogin` cannot restart the same session without a greeter; a root helper at runtime
  is not an acceptable substitute.
- The one drop-in is overwritten so often that a user-level `repair` cannot keep up.
- Media cannot fall back to Desktop on a crash or a hung first frame.
- Desktop cannot be kept stock (no Runefin process/autostart) through the dispatcher.
- The OSK cannot be made reliable with a remote; do not ship a keyboard-free Media mode whose first
  run needs a keyboard.
- HDR or 24p at the target size is unacceptable; the Steam Machine is a console first, and a
  streaming box is the honest media answer.

---

## 13. Open decisions

| # | Question | Recommendation |
|---|---|---|
| D1 | Default boot mode after install? | `desktop` unless `--default media`; a media box opts in. |
| D2 | Media failure fallback? | Desktop. Visible, normal, one command from Media again. |
| D3 | Steam's own switches? | Best-effort only if S7 passes; otherwise the tiles/entries are the supported matrix. |
| D4 | OSK open on focus or on a button? | Auto in Media/big-screen; On demand in Desktop. |
| D5 | HDR or 24p? | Measure KWin's output control; otherwise HDR at 60 Hz and document the judder. |
| D6 | Root drop-in or `steamosctl` custom session? | Prefer `steamosctl` if S1 passes; otherwise one drop-in. |
| D7 | Rune upstream keyboard? | After a second app needs it; not a blocker. |

---

## 14. File map

Existing:

- `docs/research/21-steamos-media-integration-2026-11-17.md` — supported seams, tier ladder, gaps.
- `docs/plans/2026-11-17-steamos-media-integration.md` — step 10 points here.
- `scripts/steam-install.py`, `packaging/steam/README.md` — rootless Steam tile/wrapper.
- `crates/runefin/src/main.rs` — `big_screen_requested`, `tv_key`, `on_key`, `single_instance`.
- `crates/runefin/src/screens.rs` — `field()` and the connect form.
- `crates/runefin/src/screens/{search,library,settings,detail,in_app,seerr}.rs` — text fields.
- `crates/rune/src/el/input.rs` — `text_input`, `on_input_change`, `on_input_key`.
- `crates/rune-edit/src/lib.rs` — `TextEdit::insert`, `replace_text`, `set_caret`, `secure`.
- `crates/rune/src/el/tx.rs` — `send_key` and focus.
- `packaging/flatpak/io.github.lifk.Runefin.yml` — no new permission required.

New, when built:

- `crates/runefin/src/screens/keyboard.rs`
- `crates/runefin/src/state/keyboard.rs`
- `crates/runefin/src/state/keyboard_tests.rs` (or tests in `main.rs`)
- `packaging/modes/session.sh`, `supervisor.sh`, `runefin-modes.desktop`
- `packaging/modes/runefin-mode` and `README.md` / `RECOVERY.txt`
- `~/.config/systemd/user/runefin-mode-sync.service` (installed)
- a `keyboard` scene and `docs/snapshots/keyboard-big.png`
- a `tv_reach_tests` case for the sheet.

---

## Appendix A — if the dispatcher is impossible

The only fallback that needs no root is the previous media-profile plan: Desktop Mode with Runefin
autostart, no third session. It is **not acceptable under the current requirement** because it takes
over Desktop. It is recorded here only as a fallback if S2/S3 fail and the user changes that
requirement. The other fallback is the existing Steam tile in Gaming Mode: SDR, no HDR, but no
session changes.

The attached draft's custom session is this plan's ancestor; its six blockers are the six fixes in
§3. Nothing else from it is kept.

---

## Appendix B — text fields the keyboard must cover

| Target | Key | Site |
|---|---|---|
| ConnectServer | `server` | `screens.rs` connect form |
| ConnectUser | `username` | `screens.rs` connect form |
| ConnectPassword | `password` | `screens.rs` connect form (secure) |
| Search | `search-input` | `screens/search.rs` |
| LibrarySearch | `library-search` | `screens/library.rs` |
| SeerrSearch | `seerr-search` | `screens/seerr.rs` |
| SettingsSearch | `settings-search` | `screens/settings.rs` |
| OmdbKey | `setting-omdb-key` | `screens/settings.rs` (secure) |
| SeerrUrl | `setting-seerr-url` | `screens/settings.rs` |
| SeerrKey | `setting-seerr-key-input` | `screens/settings.rs` (secure; `setting-seerr-key` reveals it) |
| SeerrUser | `setting-seerr-user` | `screens/settings.rs` |
| PlaylistName | `playlist-name-input` | `screens/detail.rs` |
| GotoTime | `in-app-goto-input` | `screens/in_app.rs` |
| DownloadsPath | `setting-downloads-path` | `screens/settings.rs` |

The keys are read from the source in the same commit that adds the target map; the table is a
checklist, not a second source of truth. There are 14 fields today.
