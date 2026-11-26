# Steam Machine field notes — 2026-11-26

Written from the live Kali image on the machine itself, with the installed SteamOS filesystems
mounted read-write. It records what the machine actually is, what the Runefin install left
behind, the session bug that stranded it, and the maintenance and test items that follow. No
credentials are included.

---

## 1. Hardware and OS

- AMD Custom CPU 1772: 6 cores / 12 threads, 12 CPUs visible.
- Radeon Navi 33 (`1002:7481`, subsystem `1e44:1717`), `amdgpu`.
- 4K Samsung TV (`SAM 29725`) on `HDMI-A-1`, 3840×2160 at 60 Hz, KWin scale 1.7, HDR **off**,
  VRR **Never**, `colorProfileSource=sRGB`, `wideColorGamut=false`, SDR brightness 200.
- Audio: card 0 `HDA ATI HDMI`, card 1 `HD-Audio Generic`.
- SteamOS **3.9.5** (build 20261023.100) active in slot A; SteamOS **3.8.28** (build 20260922.1)
  inactive in slot B. Kernel `7.2.7-valve2-2-neptune-72-g31504582a073`.

### Partition layout

| Partition | Size | FS | Label | Role |
|---|---|---|---|---|
| nvme0n1p1 | 256M | vfat | `esp` | shared ESP |
| nvme0n1p2 | 64M | vfat | `efi-A` | GRUB for slot A |
| nvme0n1p3 | 64M | vfat | `efi-B` | GRUB for slot B |
| nvme0n1p4 | 5G | btrfs | `rootfs` | active root (3.9.5) |
| nvme0n1p5 | 5G | btrfs | `rootfs` | inactive root (3.8.28) |
| nvme0n1p6 | 256M | ext4 | `var-A` | runtime `/var` for slot A |
| nvme0n1p7 | 256M | ext4 | `var-B` | runtime `/var` for slot B |
| nvme0n1p8 | 466G | ext4 | `home` | shared home and Flatpaks |

`/efi-A` and `/efi-B` hold `EFI/steamos/grubx64.efi` and `grub.cfg`; the `SteamOS/partsets`
files map `A`, `B`, `self`, `shared`, `all` to PARTUUIDs. The active root is A
(`<A-uuid>`), the inactive root is B (`<B-uuid>`). The **home partition is shared**; the
**`var` partition is per-slot**, and so is everything written under `/etc`.

### Kernel command line

```text
amd_iommu=off amdgpu.lockup_timeout=5000,10000,10000,5000 ttm.pages_min=2097152
amdgpu.sched_hw_submission=4 amdgpu.dcdebugmask=0x20000 audit=0 fbcon=vc:4-6
fsck.mode=auto fsck.repair=preen loglevel=3 quiet splash plymouth.ignore-serial-consoles
```

Two practical consequences:

- **TTYs are on F4–F6**, not F2. `fbcon=vc:4-6` is why `Ctrl+Alt+F4` reached a login prompt and
  `Ctrl+Alt+F2` did not. Recovery instructions should say F4/F5/F6.
- IOMMU is off, GPU timeouts are 5–10 s, and TTM is given a 2 GiB page floor. HDR/GPU issues
  should be compared with these values before blaming the app.

---

## 2. The `/etc` overlay and the A/B update model

SteamOS 3.9 keeps the rootfs immutable and mounts `/etc` as an overlay:

```text
lowerdir=/etc
upperdir=/var/lib/overlays/etc/upper
workdir=/var/lib/overlays/etc/work
```

The active slot's `/var` is `var-A` (`nvme0n1p6`, 256 MiB). That is why the rootfs `/etc` seemed
to have no Runefin SDDM drop-in — it was in the overlay upper:

```text
/var/lib/overlays/etc/upper/sddm.conf.d/zzz-runefin-modes.conf
```

**Implications**

- Editing the rootfs `/etc` from a live image does nothing at runtime. Use the running system's
  `pkexec`/root writes, or edit `/var/lib/overlays/etc/upper` while the slot is off.
- The `var` partition is only 256 MiB; it holds the overlay, caches and any logs. There is **no
  persistent journal** here (`/var/log` is empty), so a failed boot leaves no `journalctl`
  history. The only persistent diagnostics are the home-directory logs below.
- An A/B switch or OS update lands on the other root and the other `var`; the SDDM drop-in is
  **not carried over**. After an update, run `runefin-mode repair` (or the installer) again.
- RAUC declares `rootfs.0`/`rootfs.1` (A/B) and `verity.0`/`verity.1`, with a `dev` rootfs slot;
  `bootloader=custom`, `activate-installed=false`, `max-bundle-download-size=256 MiB`, desync
  `caibx` seeding. This is the update mechanism behind the A/B pair.

The inactive 3.8.28 slot still uses the older layout: stock `/etc/sddm.conf.d/steamdeck.conf`
and `steamos.conf`, plus `/usr/lib/sddm/sddm.conf.d/steamos.conf`. The active 3.9.5 slot uses
`/usr/lib/sddm/sddm.conf.d/{default,holo}.conf` and leaves `/etc/sddm.conf.d` for local
overrides. The installer's `install -D` creates the local directory on the running slot, which is
why it worked despite the rootfs showing no directory.

---

## 3. Sessions on 3.9.5

Stock SDDM config (`/usr/lib/sddm/sddm.conf.d/holo.conf`):

```ini
[General]
DisplayServer=wayland

[Autologin]
Relogin=true
Session=gamescope-wayland.desktop
User=deck
```

Stock session files:

- `plasma.desktop` → `Exec=/usr/lib/plasma-dbus-run-session-if-needed /usr/bin/startplasma-wayland`
- `gamescope-wayland.desktop` → `Exec=start-gamescope-session`

`start-gamescope-session` is the important one. It:

- stops any leftover user `gamescope-session.target` and graphical pre-target,
- sets `XDG_SESSION_TYPE=x11` (gamescope apps are X11),
- updates the systemd activation environment and unsets `DISPLAY`/`XAUTHORITY`,
- sets `XDG_DESKTOP_PORTAL_DIR` to the gamescope portal,
- starts and waits on `gamescope-session.target`.

That target `Upholds=steam-launcher.service` and wants `ibus-gamescope`, `steam-notif-daemon`,
`galileo-mura-setup` and `gamescope-mangoapp`. Running `/usr/lib/steamos/gamescope-session`
directly skips all of that and hangs at the splash.

The Runefin wrapper had both bugs:

- game: exec'd `gamescope-session` directly instead of `start-gamescope-session`;
- desktop: ran `startplasma-wayland` without `XDG_SESSION_TYPE=wayland` and without the stock
  `plasma-dbus-run-session-if-needed` wrapper.

Both are fixed in `packaging/modes/session.sh` (commit `5a0af4d`) and were patched in the
installed copy on the machine. The machine's mode is now `desktop` for the first boot.

The custom session `.desktop` lives in `~/.local/share/runefin/modes` and is listed in the
drop-in's `SessionDir`. SDDM did record it as the last session
(`/var/lib/sddm/state.conf`, as a full path with a trailing space), but the wrapper's own
`session.log` was never written. The installer generated `Exec="…/session.sh"` with quotes, and
the script used `$HOME` without a fallback; either can make the session fail silently. The
installer now writes an unquoted `Exec` and the script falls back to `/home/deck`. The file still
lives under the 700-permission home, so moving the `.desktop` to `/etc/runefin-modes` and adding
that to `SessionDir` is the robust fix for SDDM discovery. The top-level
`XDG_SESSION_TYPE=wayland` export is what makes the session type unambiguous.

---

## 4. Runefin install footprint

### Flatpak

- App id `io.github.lifk.Runefin`, user install under `~/.local/share/flatpak`, runtime
  `org.freedesktop.Platform/x86_64/26.08`, active commit `9ecfedc…`.
- Permissions: `wayland`, `fallback-x11`, `pulseaudio`, `dri`, `input`, `ipc`, `network`,
  `xdg-run/pipewire-0`, notifications talk, MPRIS own name, `StatusNotifierWatcher`,
  `logind` system talk, `org.a11y.Bus`.
- **No home or `xdg-download` filesystem permission.** The app's default download root is
  `XDG_CACHE_HOME/runefin/downloads` — inside the sandbox that is
  `~/.var/app/io.github.lifk.Runefin/cache/runefin/downloads`, which is writable but hidden from
  the user and may be purged as cache. A user-configured path outside the sandbox (for example
  `~/Videos`) will fail. This needs a portal or a `--filesystem=xdg-videos`/`xdg-download`
  grant before downloads can be recommended on the Steam Machine.
- App data: `~/.var/app/io.github.lifk.Runefin/{config,cache,data}`. `config/runefin/config.toml`
  is the settings file; `data` was empty; `cache` holds posters, images and RADV shader caches.

### Settings found in `config.toml` (non-secret)

`autoplay_next=true`, `skip_segments="ask"`, `still_watching=true` (3 episodes / 90 min),
`resume_rewind_seconds=10`, `playback_quality="auto"`, `subtitle_mode="forced"`,
`subtitle_background="dim"`, `audio_mode="off"`, `downmix_stereo=false`, `reduce_motion=false`,
`notifications=false`, `home_hero=false`, `hover_preview=false`, `light_theme=false`,
`text_scale=1.0`, `high_contrast=false`, last place `detail/500c969b…`, window 1363×720.
The file has no `hdr` key, so HDR is at its default `Auto`; KWin currently reports HDR off, so
the app runs SDR.

### Steam tile

The Steam user `<steam-account>` has two Runefin shortcuts, both **Flatpak exports**, not the wrapper:

| Name | appid | Exe | LaunchOptions | ShortcutPath |
|---|---|---|---|---|
| Runefin Big Screen | -70959876 | `/usr/bin/flatpak` | `run … io.github.lifk.Runefin --big-screen` | `…/io.github.lifk.Runefin.BigScreen.desktop` |
| Runefin | -138262842 | `/usr/bin/flatpak` | `run … io.github.lifk.Runefin` | `…/io.github.lifk.Runefin.desktop` |

The wrapper `~/.local/share/runefin/runefin-steam.sh` exists but is **not referenced by either
shortcut**. Its `launch.log` is empty. That means:

- the wrapper's `unset LD_LIBRARY_PATH LD_PRELOAD` guard is bypassed;
- the wrapper's `runefin-mode media` switch is bypassed, so the tile does **not** switch Gaming
  to the Media session as the release notes describe;
- the "Runefin" tile has no `--big-screen`, while the "Runefin Big Screen" tile has it but no
  artwork (the installer recorded appid `-138262842`, the non-big-screen one).

`find_shortcut` in `steam-install.py` has a fallback that accepts any shortcut named
`Runefin` with `runefin`/`io.github.lifk.Runefin` in its Exe/options. That is what matched the
Flatpak shortcut and let the installer believe it had added the wrapper. The fallback should be
removed or narrowed to the wrapper, and the wrapper shortcut should be art'd.

### Mode files and drop-in

- `~/.local/share/runefin/modes/{session.sh,supervisor.sh,runefin-modes.desktop,sddm.conf}`
- `~/.local/bin/runefin-mode`
- `/var/lib/overlays/etc/upper/sddm.conf.d/zzz-runefin-modes.conf` (active `var-A`)
- `~/.local/state/runefin/{mode,session.log,media.log,…}` (currently `mode=desktop`; the logs
  were empty because the directory had been cleared)
- `~/.local/share/runefin/steam.json`: appid `-138262842`, wrapper/launcher paths, artwork
  paths for that appid.

The installer's `runefin-mode` command writes the mode to home and terminates the graphical
session; `runefin-mode repair` rewrites the drop-in after a SteamOS update. Both should be in
the post-update checklist.

---

## 5. Display, HDR and audio state

- KWin output `HDMI-A-1`: 3840×2160@60, scale 1.7, **HDR off**, VRR Never, sRGB profile,
  SDR brightness 200. The TV is a Samsung `SAM 29725`.
- To test HDR, enable HDR in Desktop/Media KWin output settings first; the app requests `Auto`,
  but KWin must offer `ExtendedSrgbLinear`/`Rgba16Float`. Gaming Mode is expected to stay SDR
  because gamescope does not expose HDR to a native Flatpak.
- Audio: HDMI (card 0) and analog (card 1) are present. The Flatpak has the PulseAudio socket and
  `xdg-run/pipewire-0`; no user WirePlumber/PipeWire configs exist, so the system defaults apply.
  Passthrough remains a receiver/PipeWire profile question, not an app setting.

---

## 6. Fixes and follow-ups

**Already fixed**

- `packaging/modes/session.sh` (`5a0af4d`): `XDG_SESSION_TYPE=wayland` for all modes, stock
  Plasma dbus wrapper, `start-gamescope-session` for game with fallbacks, and the desktop
  fallback inside the game branch.
- The installed copy on the machine was patched the same way; mode set to `desktop`.

**Recovery state (2026-11-26)**

Because the custom session never reached a log, the machine was forced to the stock Plasma
session to separate session selection from Plasma/KWin:

- `/etc/sddm.conf` in the active var overlay: `DisplayServer=wayland`, autologin `deck`,
  `Session=plasma.desktop`, `SessionDir=/usr/share/wayland-sessions`.
- `/var/lib/sddm/state.conf`: `Session=/usr/share/wayland-sessions/plasma.desktop`.
- The Runefin drop-in was renamed `.disabled` in the overlay.
- The custom `session.sh` was fixed with the stock wrappers, `XDG_SESSION_TYPE=wayland`, a
  `$HOME` fallback and the stock Plasma/Game commands; its `.desktop` was rewritten with an
  unquoted `Exec=/home/deck/...`.
- The rootfs vendor config was **not** modified; the rw remount did not make `/usr` writable
  (verity/immutable root), and the attempted write left no file.

**Follow-ups**

| Item | Why | Change |
|---|---|---|
| Steam tile wrapper | Tile bypasses the wrapper; no mode switch, no LD guard, wrong art entry | Remove the `find_shortcut` name fallback, add/point one shortcut at `runefin-steam.sh`, art that appid, and decide whether the separate Flatpak Big Screen tile stays |
| Flatpak downloads | Default root is a sandbox cache; user paths are denied | Add `--filesystem=xdg-videos` or a document portal, and default downloads to persistent `XDG_DATA_HOME` or a visible folder |
| Recovery docs | `fbcon=vc:4-6` means F4–F6, not F2 | Update `docs/design.md`/release notes/README recovery steps |
| A/B maintenance | The drop-in lives in the active slot's `var` overlay | After an OS update or slot switch, run `runefin-mode repair` or rerun the installer; document it |
| HDR test | KWin reports HDR off; app is Auto | Enable HDR in KWin Desktop/Media settings, then check the app's status and an HDR title; keep Gaming expectations SDR |
| Journal | `/var/log` is empty; failed boots leave no journal | If debugging field failures, enable persistent journald on the machine or use the home `session.log`/`launch.log` |
| Custom session location | `.desktop` is in a plain modes folder | Optionally install it under `~/.local/share/wayland-sessions/` and keep it in `SessionDir`; the env export already fixes the type |
| Decky | Not installed on this machine | Only install it when specifically testing the optional QAM plugin |

**Do not**

- Do not edit the rootfs `/etc` expecting runtime effect while the slot is active; use the
  overlay upper or the running system.
- Do not assume both rootfs slots have the same SDDM layout; 3.8.28 and 3.9.5 differ.
- Do not run `gamescope-session` directly or `startplasma-wayland` without the dbus wrapper.
- Do not put the mode drop-in on the rootfs lower; it belongs in `/var/lib/overlays/etc/upper`.
