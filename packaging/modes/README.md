# Three modes (Desktop, Gaming, Media)

The one-command install is the host installer:

```sh
curl -fsSLO https://raw.githubusercontent.com/lifk/Runefin/main/scripts/steam-install.py
python3 steam-install.py --three-modes --default-mode media
```

It installs the Flatpak and Steam tile (as `steam-install.py` always does), writes the mode
dispatcher and supervisor under `~/.local/share/runefin/modes`, writes `runefin-mode` under
`~/.local/bin`, and installs the single SDDM drop-in. `pkexec` asks once for that file. Reboot
afterwards, or run `runefin-mode media` to switch immediately.

`--default-mode desktop` (the default) leaves the machine booting into stock Plasma. `game` boots
Steam; `media` boots KWin + Runefin. The choice is only the marker; it can always be changed later.

## What runs

| Mode | Body | Runefin |
|---|---|---|
| Desktop | stock `startplasma-wayland` | not running |
| Gaming | stock `gamescope-session` | not running |
| Media | `kwin_wayland --exit-with-session supervisor.sh` | full screen |

The Steam tile now switches from Gaming to Media when the drop-in is present. From Media, the
Settings → **SteamOS modes** buttons switch to Desktop or Gaming. From Desktop, the **Runefin
Media** and **Runefin Game** entries do the same. From Media, **Restart app** exits 0 and the
supervisor restarts it.

## Commands

```sh
runefin-mode media|desktop|game   # write the marker and restart the session
runefin-mode status               # mode, session, drop-in and app version
runefin-mode repair               # re-write the SDDM drop-in after a SteamOS update
runefin-mode restore              # remove the drop-in; stock SDDM returns
```

The supported switch is the app/desktop entry and `runefin-mode`; Steam's own **Switch to
Desktop** is best-effort because our SDDM session owns the login. If it is intercepted, go through
Media → SteamOS modes → Desktop.

## Failure and recovery

Media has a first-frame marker in the Flatpak cache. The supervisor restarts a crash, and after
three failures in the window writes `desktop` and exits, so the machine lands in Plasma rather
than a black screen. A KWin failure is caught by the next session start. From a TTY — on
SteamOS 3.9 the console is on **F4–F6** (`fbcon=vc:4-6`), not F2:

```sh
runefin-mode desktop
```

The SDDM drop-in is additive and sorts last; `runefin-mode restore` removes it and SteamOS's own
autologin applies. It lives in the active slot's `/var` overlay upper (`/etc` is an overlay on
SteamOS 3.9), so after an OS update or an A/B switch run `runefin-mode repair` before switching
to Media. There is no per-boot rewrite and no root process at runtime.
