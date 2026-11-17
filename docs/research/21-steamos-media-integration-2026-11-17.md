# SteamOS media integration — 2026-11-17

Investigation into how deeply Runefin can integrate with SteamOS / the Steam Machine, and what
would make SteamOS friendlier for media outside games. Six parallel reviews: launch mechanics,
media-mode feasibility, Steam library/artwork, HTPC references, a code-level gap audit, and
current platform research. Dates and sources are inline; hardware-only claims are marked.

---

## 1. Verdict

**Deep integration is mostly packaging, launch, input and power management — not OS hacking.**
The supported seam is: one Flatpak, one stable Steam shortcut, one wrapper, a controller-first
UI, a correct suspend path, and an optional Desktop Mode media profile. Everything up to “a
polished tile in Gaming Mode that the TV remote can drive” survives SteamOS A/B updates and
breaks nothing.

**Achievable without Valve**

- One-click “Add to Steam” with stable identity, launch options, custom portrait/header/hero/
  logo art, and a wrapper that survives the Steam launch-environment bugs.
- Automatic big-screen + fullscreen when launched from Gaming Mode or gamescope, with a desktop
  opt-out.
- Native gamepad + CEC remote, a focus gate under the Steam overlay/OSK, a couch-only first run
  through Quick Connect, and a working in-app hint for the system keyboard.
- Suspend/resume that pauses, reports, tears down and rebuilds the pipeline, re-enumerates pads
  and audio, and re-seeks.
- A reversible “media box” profile: default to Desktop Mode, autostart Runefin Big Screen,
  Plasma power settings, optional CEC standby helper. Games still launch from Steam.
- Diagnostics, update notices, logs, and a repair action for the Steam entry.
- Steam Remote Play of the same shortcut, and an “open in…” hand-off (browser/trailer), not a
  media hub.

**Not achievable without Valve, or not worth the risk**

- A first-class Media shelf in Gaming Mode. Valve has not shipped one and said it had no plans
  for bespoke media apps in the SteamOS UI (Digital Foundry hands-on, Nov 2025; 2026 reviews
  still describe browser-only streaming).
- Per-content refresh-rate switching (23.976/24p) in Gaming Mode: gamescope exposes one refresh
  rate to clients (`gamescope #771`, still open).
- HDR output for non-Steam Wayland apps in Gaming Mode. Non-Steam media apps cannot see
  gamescope’s HDR; Desktop Mode’s KWin path is the one that works (`SteamOS #2166`).
- HDMI bitstream passthrough by default: SteamOS exposes LPCM 5.1/7.1; TrueHD/Atmos/DTS-HD need
  a community profile or a future Valve change (`SteamOS #989`).
- Widevine L1/4K DRM streaming services — a platform/licensing problem, not an app bug.
- Boot straight into Runefin, a custom kiosk session, or Steam UI injection. These are
  unsupported, fight updates, and have recovery stories users should not need.

The honest ceiling is **an appliance-grade app inside SteamOS**, plus a **documented Desktop Mode
media profile** for people who want a media-first machine. Not a replacement console OS.

---

## 2. How SteamOS actually works (2026)

### Versions and hardware

- SteamOS **3.9.5** stable (23 Oct 2026): Linux 7.2, KDE Plasma 6.7.3 in Desktop Mode, A/B atomic
  updates, read-only root, shared `/home`; Flatpaks survive under
  `/home/.steamos/offload/var/lib/flatpak`.
- Steam Machine shipped 29 Jun 2026 from $1,049: Zen 4 6C/12T, RDNA 3 28 CU, 16 GB DDR5 +
  8 GB GDDR6, HDMI 2.0 (4K120, HDR, FreeSync, CEC), DisplayPort 1.4, Wi-Fi 6E, a dedicated
  2.4 GHz Steam Controller radio. 4K120 is 4:2:0 8-bit SDR; 4K HDR is 60 Hz.
- 3.10.1 preview (16 Nov 2026) adds HDMI 2.1 VRR/FRL/ALLM on AMD, HDR fixes and
  `VK_EXT_present_timing`; not stable.

### Sessions

- Gaming Mode = **gamescope as the DRM session compositor** + Steam Big Picture. There is no
  desktop under it, no window manager, no tray.
- Desktop Mode = **KDE Plasma 6.7 on Wayland**.
- `steamosctl switch-to-desktop-mode` / `switch-to-game-mode` and
  `set-default-login-mode game|desktop` are the supported switches. `steamos-session-select` is
  legacy; 3.9 renamed many `steamos-*` tools to `holo-*`.
- Gaming Mode does **not** run XDG autostart. A `systemd --user` unit in
  `~/.config/systemd/user` is the only hook that runs in both sessions and survives updates.

### Launch

- Steam knows Steam apps and shortcuts in
  `userdata/<id>/config/shortcuts.vdf` (native Steam or
  `~/.var/app/com.valvesoftware.Steam/...` for Flatpak Steam).
- A shortcut records `AppName`, `Exe`, `StartDir`, `LaunchOptions`, `icon`, `FlatpakAppID`,
  `tags`, `LastPlayTime`. Steam executes `Exe` directly, no shell; `Exe` is quoted,
  `StartDir` quoted.
- Add paths: **Add a Non-Steam Game** (correct icon, random appid), **`steam://rungameid/<64-bit
  id>`** (needs the computed id), or **write the VDF** with Steam closed (only way to a stable
  appid, name, args and art in one step). `steamos-add-to-steam` / `steam://addnonsteamgame`
  adds a running client without a restart.
- Stable identity matters: changing `Exe`/args/name changes the shortcut id; artwork and any
  collection mapping are keyed by it. Read the id back; do not compute and assume.
- Steam injects `SteamClientLaunch`, `SteamEnv`, `SteamGameId`, `SteamGamepadUI`, `SteamDeck`,
  `SteamTenfoot` and gamescope markers. For shortcuts `SteamAppId` is `0`.
- The September 2026 Steam client regression leaked Steam’s `LD_LIBRARY_PATH`/`LD_PRELOAD` into
  `flatpak run` on SteamOS and broke every Flatpak/AppImage shortcut; `env -u` did not always
  work, but a wrapper script that unsets both and `exec`s `/usr/bin/flatpak` did
  (`steam-for-linux #13651`). A wrapper is the durable launch path, not a nicety.

### Gaming Mode inheritance

- Gamescope gives fullscreen/scaling, the QAM frame limiter, refresh rate, VRR, HDR toggle,
  screenshots and suspend/resume to the app.
- **No** window manager, taskbar, tray or sane multi-window handling. A portal file chooser or a
  second window can land behind Steam or black. Runefin’s single-window design is exactly right;
  keep dialogs in-app.
- The Steam overlay/OSK on a **Flatpak** non-Steam shortcut is the highest-uncertainty area.
  Gamescope integrates the overlay at the compositor level, but Flatpak strips the
  `LD_PRELOAD` gameoverlayrenderer injection. Reports conflict on whether the overlay, F12
  screenshots and Steam+X work for a sandboxed shortcut. **Verify on a Steam Machine before
  promising overlay/screenshot/OSK behavior.**

### Input

- With Steam Input on, the Deck/Machine pad is a **virtual Xbox 360 pad** (`28de:11ff`); the
  kernel `hid-steam` gamepad is hidden. gilrs sees the virtual pad; back buttons, trackpads,
  gyro and the Steam/QAM buttons are not exposed.
- Steam Input off: `hid-steam` exposes a native pad; 3.8.28 added Steam Controller 2026 support
  for native controller apps when Steam is not running.
- Steam+X is the system OSK, but its reliability in non-Steam Flatpaks is unverified; the
  `steam://open/keyboard?Mode=0` deeplink works only when the overlay is injected. The OSK types
  X key events, so plain text fields are the right target; IME/preedit is not.
- CEC: SteamOS 3.8.1+ runs `cecd`; TV remote keys arrive as keyboard events. `cecd` mapping is
  global to the session. Sleep/wake CEC is still buggy on some TVs (standby not sent, wake
  loops, CEC hijacking other devices — `SteamOS #2817`, `#2803`, `#2951`).

### Display and audio

- Gaming Mode: refresh rate and HDR are set by Steam/gamescope, not the app; one refresh rate
  to clients; no per-film 24p switch. HDR for non-Steam Wayland apps is not exposed; gamescope
  only lights the HDR badge when the app presents an HDR swapchain.
- Desktop Mode: KWin handles HDR/VRR and `wp_color_management_v1`; this is the only practical
  HDR path for a non-game app today.
- Audio: SteamOS exposes LPCM 5.1/7.1; TrueHD/Atmos/DTS-HD bitstream is not default. The app’s
  new passthrough attempts `pulsesink`, but the SteamOS default stack may not expose compressed
  caps; treat it as receiver/OS dependent.

---

## 3. What “deep integration” can mean — a ladder

| Tier | What the user gets | Mechanism | Risk / support |
|---|---|---|---|
| **0. App-level** | Big-screen UI, native pad, Quick Connect, CEC map | Already in Runefin | Supported; no OS changes |
| **1. First-class Steam entry** | One tile named Runefin with art, `--big-screen`, wrapper, stable id | `steamos-add-to-steam` or `shortcuts.vdf` + `grid/` art + wrapper | Low; survives updates; installer on host |
| **2. Session-aware app** | Auto big-screen/fullscreen on gamescope; focus gate; OSK hint; CEC transport; suspend/resume; diagnostics | Env detection + Rune focus + logind + key events | Low–medium; all app-side |
| **3. Desktop media profile** | Boots to Plasma and autostarts Runefin fullscreen; games still in Steam; CEC/power helper | `steamosctl set-default-login-mode desktop`, `~/.config/autostart`, user service | Low, reversible; still a desktop |
| **4. Custom media session** | Boots straight into Runefin with no Steam shell | SDDM `[Wayland] SessionDir` + `gamescope -f -- runefin` | High; unsupported; TTY recovery required |
| **5. Platform** | Media shelf, HDR for non-Steam apps, 24p, TrueHD, Widevine L1, boot-to-app | Valve | Not available |

**Tier 1–3 is the product.** Tier 4 is a documented hobby. Tier 5 is not in our hands.

---

## 4. Current Runefin gaps (code-level)

Severity for a Steam Machine user; effort S/M/L.

### Launch and identity

- **Blocker: nothing detects Steam/gamescope.** Mode is chosen only by `--big-screen`,
  `RUNEFIN_UI=big`, or the saved mode (`main.rs:84-97`). The ordinary desktop entry launched
  from Gaming Mode runs the desktop UI: no fullscreen request, no first-focus, no TV gutters,
  no focus zones, and a 1280×800 window on a 4K TV. `RUNEFIN_UI` has no `desktop` opt-out.
- **Major: fullscreen is applied after the first frame** (`main.rs:298-300`), so one windowed
  frame appears; Rune can create the window fullscreen.
- **Major: geometry is shared between modes**; a 1080p Gaming Mode close makes the next Desktop
  Mode window huge, and a Deck desktop window can force 720 next time. Fullscreen is not
  persisted.
- **Major: second launch is display-blind and drops flags.** The single-instance socket is keyed
  only by app id; `--big-screen` is not a URL, so launching Big Screen while a desktop instance
  runs just raises the desktop window. Desktop and Gaming Mode share `XDG_RUNTIME_DIR`; a
  leftover instance can make the Steam “game” exit in milliseconds while the real one plays on
  a compositor that is gone.
- **Major: quit may lose the stop report.** `on_quit` calls `stop_in_app` → `worker.report_stopped`,
  which spawns a fire-and-forget thread; the process can exit before it sends the final
  `PositionTicks`. Steam “Exit game” sends SIGTERM, so this is the normal path.
- **Minor: signal handler is installed after GPU prewarm/config**; a very early SIGTERM is
  unclean.
- **Minor: MPRIS `Raise` lacks an activation token** under Wayland/gamescope.
- **Minor: tray icon is a flat blue square**, not the app icon; harmless in Gaming Mode.
- **Packaging: `--require-version=1.16.0`** is stricter than `--device=input` needs (1.15.6);
  older SteamOS installs refuse.
- **Distribution: the Flatpak build fetches crates/Rune from the network**, so it is not Flathub
  eligible; Discover can only install after a third-party remote is added.

### Input

- **Blocker: first run needs a keyboard.** Quick Connect refuses an empty server field, discovery
  only runs on button press, and big-screen focus lands on the server text field. A controller-
  only user cannot get to the one path designed for them.
- **Major: pad events are not focus-gated.** `pad::run` sends every event to the app regardless
  of window focus; the pad moves/activates controls behind the Steam overlay or OSK.
- **Major: the pad map is thin.** D-pad/stick, A/B/Start, LB/RB only. Analog triggers (natural
  seek/scrub), right stick (volume/scroll), Steam Controller paddles, trackpads and gyro are
  absent. A single `Pad` state is shared across devices; a virtual Steam pad plus a physical one
  can double-drive focus and clear each other’s held directions.
- **Major: CEC transport is dead.** `cecd.toml` maps only the two Menu buttons. TV Play/Stop/FF/
  Rewind arrive as `NamedKey::MediaPlayPause` etc., which Rune drops; `tv_input` has no media
  actions. OK/Back/D-pad work by default; transport does not.
- **Docs: the README/features claim a Select→Space and transport mapping that the current
  `cecd.toml` does not contain.** Fix or implement.
- **OSK: unowned.** No in-app OSK, no “Steam+X” hint, no `on_ime`/needs; the app assumes a
  system keyboard. The README tells users to leave Steam Input off, which is the configuration
  where Steam+X is least verified.

### Power, display, audio

- **Major: no suspend/resume path.** Idle inhibitor exists, but no `PrepareForSleep`, no pipeline
  pause/teardown, no audio/controller re-enumeration, no position resync. `Instant` is
  `CLOCK_MONOTONIC` and stops during suspend, so the session clock can run behind. This is the
  most common couch action.
- **Major: no audio-device selection or sink-loss recovery.** Passthrough builds a bare
  `pulsesink` with no `device`; decoded audio follows the default sink. A TV input switch or
  receiver power cycle can end playback with an error and no reselect.
- **Major: no refresh-rate/HDR path.** HDR is a Rune renderer milestone; per-content 24p is
  blocked by gamescope. Document it; do not imply parity.
- **Major: big screen is hard-coded 16:9.** On a Deck 1280×800, gamescope letterboxes; the app
  claims the opposite motive. Add a 16:10 Deck preset or accept the letterbox explicitly.
- **Minor: notifications** spawn `notify-send`; the runtime may not have it and Gaming Mode has no
  desktop notification surface. The in-app toast is the real path.

### Good foundations

SIGTERM/SIGINT → on_quit, single-window design, hotplug reset, logind inhibitor, `--device=input`
(not `all`), MPRIS own-name, app id, AT-SPI, Quick Connect, native pad path, and a working CEC
script. These are ahead of most media clients.

---

## 5. What would make SteamOS friendlier for media

### 5.1 The supported core (ship now)

1. **One stable Steam tile.** Installer writes a `.desktop` launcher and adds it with
   `steamos-add-to-steam` (or `steam://addnonsteamgame`), then reads back the appid. It copies
   generated portrait/header/hero/logo art into `config/grid/`, records
   `~/.local/share/runefin/steam.json`, and supports `--uninstall`. It upserts by name, never
   creates a second entry, and never touches other shortcuts. SteamGridDB is optional and
   user-keyed; ship our own art.
2. **A wrapper launcher.** `runefin-steam.sh`: unset `LD_LIBRARY_PATH`/`LD_PRELOAD`, `exec
   /usr/bin/flatpak run io.github.lifk.Runefin --big-screen`, log to
   `~/.var/app/io.github.lifk.Runefin/launch.log`. Set it as `Exe`; leave `LaunchOptions` empty.
   This is the mitigation for the Sept 2026 environment leak and the only place to add
   diagnostics.
3. **Session detection with an opt-out.** Precedence:
   `--big-screen`/`RUNEFIN_UI=big` → big; `--desktop`/`RUNEFIN_UI=desktop` → desktop;
   `--snap`/`--tree` → desktop; `SteamTenfoot=1` or `GAMESCOPE_WAYLAND_DISPLAY` → big+fullscreen;
   `SteamDeck=1` alone → big but windowed; else the saved config. Add a Settings row
   “TV mode on SteamOS: Auto / Always / Never”.
4. **Fullscreen at window creation**, with per-mode geometry and a persisted fullscreen flag.
5. **Couch first run:** discover servers on setup; if one answers, prefill it; if several, focus
   the first; put Quick Connect above the credential fields in big screen; add a “Steam+X for
   keyboard” hint.
6. **Focus gate:** `on_focus` → atomic; drop pad events and repeat while the window is not
   focused, so the overlay/OSK is not fought.
7. **Durable quit:** install signal handlers before config load; flush the stop report with a
   bounded wait (≤1.5 s) before exit.
8. **CEC transport:** either extend `setup-cecd.sh` to map Play/Pause/Stop/FF/Rewind to
   Space/Left/Right (global side effect, documented) or add media-key support in Rune and handle
   it in `tv_input`. Fix the stale docs either way.

### 5.2 The appliance profile (reversible, next)

A “Media box” setup script, in the same style as `setup-cecd.sh`:

```sh
steamosctl set-default-login-mode desktop
# ~/.config/autostart/runefin-media.desktop → flatpak run … --big-screen
# Plasma power: no blank while fullscreen media; screen off after N minutes paused
# optional systemd --user CEC standby helper / logind sleep hook
# --restore: steamosctl set-default-login-mode game; remove autostart; remove helper
```

Why Desktop Mode: KWin owns HDR/VRR and color management (the only path that can eventually
do HDR for Runefin), autostart works, notifications and a real window manager exist, and games
still launch from the Steam client. The cost is that you see Plasma first and the console
experience is one click away. This is a **media-first machine**, not a console replacement.

The profile must: run only once, be idempotent, log every change, back up files, and restore
with one command. It should not edit `/usr`, `/etc`, `devkit-steam`, `shortcuts.vdf`, or the
session definition.

### 5.3 The experimental edge (document, do not support)

- Custom SDDM session via `/etc/sddm.conf.d` `[Wayland] SessionDir` pointing at
  `~/.local/share/wayland-sessions`, `Exec=gamescope -f -- runefin --big-screen`, plus TTY
  recovery instructions. This fights A/B updates and Valve script renames.
- `devkit-steam` autoboot — explicitly dangerous, bootloop-prone; do not recommend.
- Decky/Steam UI injection — breaks on Steam client updates (Decky 3.2.10 failed on 3.9.3 in
  Oct 2026); never put core functionality there.
- A Steam store listing — Steamworks’ non-game categories (Audio/Video *Production*, Player
  Tools) do not fit a Jellyfin client; $100 + paperwork + a second build/update channel. Not
  worth it. Flathub is the better distribution step (needs vendored sources and no network
  build).

### 5.4 The gap Valve must close

- A media shelf / boot-to-app mode in Big Picture.
- HDR for non-Steam Wayland apps in gamescope (not just games), with overlay/Steam Input intact.
- Per-content refresh switching and 24p in Game Mode.
- HDMI bitstream passthrough (TrueHD/Atmos/DTS-HD) and auto lip-sync.
- Reliable CEC (standby, wake, volume, no hijacking).
- OSK with modifiers/F-keys and reliable non-Steam launch.
- Media-aware power management (inhibit sleep during playback; clean resume).
- Native apps / Widevine L1 for subscription 4K HDR.

Until then: a streaming stick/Apple TV/Shield remains the right answer for 4K HDR DRM
services; the Steam Machine is excellent for games, the local library, and 1080p browser media.

---

## 6. Deep integration blueprint

### Installer (`scripts/steam-install.sh`)

1. Detect native vs Flatpak Steam and the most-recent account.
2. Write `~/.local/share/applications/runefin-steam.desktop` with `Name=Runefin`,
   `Exec=<wrapper> --big-screen`, `Icon=io.github.lifk.Runefin`.
3. If Steam is running and `steamos-add-to-steam` exists, use it; otherwise close Steam, back up
   `shortcuts.vdf`, upsert by `AppName`, and restart. Read back the appid.
4. Generate/copy art: `grid/<appid>p.png` 600×900, `<appid>.jpg` 460×215 (or 920×430),
   `<appid>_hero.jpg` 3840×1240, `<appid>_logo.png` transparent, `<appid>_icon.png`.
5. Store `{appid, files, shortcut}` in `~/.local/share/runefin/steam.json`; support `--dry-run`,
   `--uninstall`, `--art-only`.
6. Never edit another shortcut, never write while Steam is mid-close, never trust a computed
   appid.

### Wrapper (`packaging/steam/runefin-steam.sh`)

```sh
#!/bin/sh
unset LD_LIBRARY_PATH LD_PRELOAD
exec /usr/bin/flatpak run --branch=stable --arch=x86_64 \
  io.github.lifk.Runefin --big-screen "$@"
```

Log stderr to `~/.var/app/io.github.lifk.Runefin/launch.log`; include the environment and
whether Steam/gamescope markers were seen. This keeps launch failures diagnosable when Steam
swallows the exit code.

### App changes

- `big_screen_requested()` env precedence and `--desktop`; auto fullscreen; per-mode geometry.
- `on_focus` gate for pad/key injection; last-active-pad selection; ignore virtual pad when a
  physical pad is present; optional LT/RT seek and right-stick volume.
- `steam://open/keyboard?Mode=0` on big-screen text focus (best-effort) plus a visible hint.
- `PrepareForSleep` listener: hold a delay inhibitor while playing, pause and report progress,
  release, tear down; on resume re-init GStreamer/audio/pad, reselect sink, re-seek.
- Audio output device row via GStreamer device monitor; reselect on sink loss; document
  passthrough requirements (HDMI receiver, DSP bypass).
- Deep-link/flag forwarding for a second launch; namespace the instance per Wayland display;
  activation token for MPRIS Raise.
- Diagnostics screen: version, Flatpak runtime, session type, gamescope, HDR state, refresh,
  audio sink, controller names, server URL/version, last pipeline error, “copy diagnostics”.
- Update notice: compare installed version with the Flatpak remote; never self-update.
- CEC/media keys and docs.

### What not to build

- A second window for file pickers/OSK overlays; keep everything in-window.
- A launcher inside a launcher; per-film shortcuts; mutable appids.
- A Steam UI plugin or Decky dependency.
- A self-updater inside the Flatpak.
- A custom kernel/session/kiosk as a supported product.
- HDR or 24p claims in Gaming Mode until Valve exposes them.

---

## 7. Roadmap

### Stage 0 — one week, no platform risk

Installer + wrapper + generated art; session detection/`--desktop`; fullscreen at create;
per-mode geometry; focus gate; couch first-run/Quick Connect; durable quit; CEC transport and
doc fixes; lower `--require-version`.

**Done when:** a fresh Steam Machine user can install, add one tile, launch it from Gaming Mode,
sign in with a pad/remote, watch, exit, and return with Continue Watching correct — without a
keyboard or a terminal.

### Stage 1 — two weeks

Pad map (triggers, right stick, Steam virtual-pad de-dupe); OSK request/hint; audio device
selection and sink-loss recovery; diagnostics screen; update notice; Steam Remote Play docs;
“repair Steam entry”.

**Done when:** the app survives a TV input switch, a receiver power cycle, and a controller
hotplug, and every failure has a copyable reason.

### Stage 2 — two to four weeks

Suspend/resume contract (`PrepareForSleep`, pipeline teardown/rebuild, post-resume re-seek);
Desktop media profile script with `--restore`; CEC/power helper; 16:10 Deck preset; Flathub
vendoring.

**Done when:** a 2-hour film survives suspend/resume and leaves a truthful Continue Watching
position; the media profile is one reversible command.

### Stage 3 — experimental, documented only

Custom gamescope session; Steam store evaluation; “open in…” hand-off; HDR path in Desktop Mode
once Rune has an HDR renderer. No Decky, no `devkit-steam`, no boot-to-app as a promise.

---

## 8. Hardware test matrix

- Steam Machine, Gaming Mode: launch → first frame fullscreen; Steam overlay/OSK/F12; Exit game →
  correct stop report, no orphan, Steam UI returns; controller wake and suspend.
- Steam Input on/off: double navigation, virtual pad, Steam button, OSK into password/search,
  hot-unplug/re-plug mid-rail.
- Deck 1280×800: letterbox, paddles/trackpads/gyro, suspend mid-episode → resume position/audio.
- HDMI: TV input switch, receiver power cycle, eARC passthrough, Bluetooth headset mid-play.
- CEC remote: Play/Stop/FF/Rewind/Back/Menu; TV standby/wake; other-device hijack.
- 4K TV: overscan, 24p judder, HDR absent as documented, text legibility at 3×.
- Single instance across Desktop/Gaming Mode; MPRIS Raise; Discover/`flatpak update`.

---

## 9. References

- [Valve: Add a Non-Steam Game](https://developer.valvesoftware.com/wiki/Add_Non-Steam_Game) ·
  [SteamOS news 3.8–3.10](https://www.steamdeck.com/en/news) ·
  [Steam Machine](https://store.steampowered.com/hardware/steammachine) ·
  [Steamworks hardware](https://partner.steamgames.com/doc/steamhardware/steammachine)
- [steamosctl](https://github.com/tldr-pages/tldr/blob/main/pages/linux/steamosctl.md) ·
  [SteamOS teardown/partitions](https://github.com/randombk/steamos-teardown/blob/master/docs/partitions.md) ·
  [Iliana on A/B updates](https://iliana.fyi/blog/build-your-own-steamos-updates)
- [steam-for-linux #13651 (Flatpak launch env)](https://github.com/ValveSoftware/steam-for-linux/issues/13651) ·
  [#8952 overlay/Flatpak](https://github.com/ValveSoftware/steam-for-linux/issues/8952) ·
  [#8513 focus/SteamGameId](https://github.com/ValveSoftware/steam-for-linux/issues/8513)
- [gamescope #771 refresh](https://github.com/ValveSoftware/gamescope/issues/771) ·
  [SteamOS #2166 HDR non-Steam](https://github.com/ValveSoftware/SteamOS/issues/2166) ·
  [SteamOS #989 passthrough](https://github.com/ValveSoftware/SteamOS/issues/989) ·
  [CEC #2817](https://github.com/ValveSoftware/SteamOS/issues/2817)
- [shortcuts.vdf format](https://github.com/Corecii/steam-binary-vdf-ts) ·
  [appid generation](https://github.com/SteamGridDB/steam-rom-manager/blob/master/src/lib/helpers/steam/generate-app-id.ts) ·
  [SteamGridDB FAQ](https://www.steamgriddb.com/faq) ·
  [SteamTinkerLaunch](https://github.com/sonic2kk/steamtinkerlaunch/wiki/Add-Non-Steam-Game)
- [Kodi on SteamOS #27490](https://github.com/xbmc/xbmc/issues/27490) ·
  [Jellyfin Desktop #237](https://github.com/jellyfin/jellyfin-desktop/issues/237) ·
  [Chiaki-ng HDR](https://chiaki-ng.com/steam-deck-hdr-remote-play/) ·
  [Moonlight #1881](https://github.com/moonlight-stream/moonlight-qt/issues/1881)
- [Decky compatibility](https://decky-loader.com/safety-compatibility/) ·
  [Steam ROM Manager](https://github.com/SteamGridDB/steam-rom-manager) ·
  [Bazzite HTPC](https://docs.bazzite.gg/Handheld_and_HTPC_edition/quirks/) ·
  [ChimeraOS](https://chimeraos.org/) · [LibreELEC](https://libreelec.tv/about/)
- [Valve on no media apps](https://www.tweaktown.com/news/108861/steam-machine-wont-provide-a-console-like-experience-for-media-playback-yet/index.html) ·
  [Steam Machine as a media box](https://gardinerbryant.com/the-steam-machine-media-center/) ·
  [Digital Foundry hands-on](https://www.digitalfoundry.net/features/hands-on-with-steam-machine-valves-new-pcconsole-hybrid)
