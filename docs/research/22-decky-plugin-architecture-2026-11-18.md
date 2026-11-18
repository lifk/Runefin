# Decky Loader and plugins — architecture review — 2026-11-18

Question: is Decky's architecture something Runefin should borrow to make a Steam Machine a
better mixed-use media and gaming machine? This reviews the loader, its plugin API, the plugin
store, Steam Machine compatibility, and the plugins that already do media-adjacent things. All
15 comments on [SteamOS #2166](https://github.com/ValveSoftware/SteamOS/issues/2166) were read
separately; the HDR finding is recorded in
[`21-steamos-media-integration-2026-11-17.md`](21-steamos-media-integration-2026-11-17.md#steamos-2166-comments-checked-2026-11-18).

**Verdict:** Decky is a working **homebrew UI-injection layer**, not a supported SteamOS
integration API. One narrow pattern is worth borrowing — a **thin QAM frontend over a stable
host-side API** — plus its **one-shot launcher lifecycle** and its use of Steam's own
`collectionStore`. Its root CEF injector, React route patching, unsandboxed plugin runtime and
dependence on Steam UI internals are not worth adopting as a product dependency. For Runefin:
keep the supported Steam tile; add deep links and keep MPRIS as the control seam; optionally ship
a small Decky QAM plugin as an unsupported extra. Do not build a media hub inside Steam, and do
not make HDR or core media depend on Decky.

---

## 1. What Decky actually is

Decky is a **plugin loader injected into Steam's CEF UI**. The mechanism, verified in the
loader source and installer:

1. The installer touches `~/.steam/steam/.cef-enable-remote-debugging`, which makes Steam's CEF
   open its DevTools endpoint on `127.0.0.1:8080`.
2. It writes `/etc/systemd/system/plugin_loader.service`, a **root** systemd unit
   (`User=root`, `Restart=always`) that runs a PyInstaller binary from
   `~/homebrew/services/PluginLoader`.
3. The loader serves an aiohttp API on `127.0.0.1:1337`, connects to the CEF
   `SharedJSContext` tab (the Big Picture React UI), and **injects JavaScript** through the
   DevTools WebSocket. It reinjects on page navigation and restarts `steamwebhelper` when the
   page already has it.
4. The injected frontend renders a **Quick Access Menu (QAM)** page and optional full-screen
   routes under `/decky/...`. It calls a plugin's Python backend over a local socket/WebSocket.
5. Plugins live in `~/homebrew/plugins/<name>/`: `plugin.json`, `main.py` (Python backend),
   `src/` (TypeScript/React), built with `@decky/ui` and `@decky/api`. Each backend runs in its
   own process, `setuid` to the host user by default, or to **root** when the plugin asks for
   `"flags": ["root"]`. The process is not sandboxed; "SandboxedPlugin" is only a subprocess
   boundary.
6. The plugin store (≈130 entries in the database) is a GitHub repository of submodules plus a
   review process for code, remote binaries and test reports. There is **no runtime permission
   model**: a plugin can read the user's files, use D-Bus, run binaries and make network calls.

Installation needs an admin password (or the installer temporarily sets one). The loader is
GPL-2.0; `@decky/ui` and `@decky/api` are LGPL-2.1; individual plugins vary (MIT/GPL-3).

**What this buys:** Steam-native QAM/full-screen UI, Python host access, a plugin store, and
automatic reloading. **What it costs:** a root service, a global CEF debug port, a hard dependency
on Steam's minified React internals, and a plugin ecosystem with user-level (sometimes root)
access.

---

## 2. Steam Machine reality

Decky is not officially supported on Steam Machine, but users report it works:

- [decky-loader #945](https://github.com/SteamDeckHomebrew/decky-loader/issues/945) is the
  official-support request (open, July 2026). Users confirm the loader itself works on SteamOS
  3.8.16 and 3.8.26 Steam Machines; several plugins work, others fail.
- [PR #962](https://github.com/SteamDeckHomebrew/decky-loader/pull/962) was needed when Steam's
  new client changed the router/error-boundary matchers; without it, Decky's settings and store
  pages are blank. It was verified on a Steam Machine running 3.8.26.
- [#964](https://github.com/SteamDeckHomebrew/decky-loader/issues/964) (Sept 2026) shows blank
  store/settings on the Steam Beta client; a Steam Machine user reproduced it and a factory
  reset did not help.
- [#988](https://github.com/SteamDeckHomebrew/decky-loader/issues/988) (Oct 2026) shows the next
  client update splitting the QAM CSS; `TitleView` crashed on `Title`, again on Steam Machines.
- [#1011](https://github.com/SteamDeckHomebrew/decky-loader/issues/1011) shows the updater
  replacing the loader binary with an HTML error page when GitHub rate-limits.
- Individual plugins can hang the machine: `Deck Shelves` froze several Steam Machines at boot;
  users had to disable it over SSH.

So Decky on a Steam Machine **works today, unofficially**, and **breaks on some Steam client
updates**. It is not a stable platform to build a product on. The SteamOS 3.10 preview (Nov 2026)
is not represented in the loader issues we could search; that is exactly the kind of update that
breaks route matchers and class names.

---

## 3. Plugins that are relevant to media

| Plugin | What it proves | What it does not solve |
|---|---|---|
| **Desktop Apps** (`decky-desktop-apps`) | A QAM list can launch a Flatpak/`.desktop` app without adding it to Steam: `SteamClient.Apps.AddShortcut`, `RunGame`, `RegisterForAppLifetimeNotifications`, temporary-shortcut cleanup. ~6 KB of Python plus a small React panel. | Nothing media-specific; it is a launcher. |
| **Museck** | A QAM panel can be a real **Jellyfin/Plex/Emby** client: server URL, credentials, playlists, search, queue, playback controls. | It is music; no video, no HDR, no player surface. |
| **Junk-Store** | A full **launcher framework** in the QAM, 375k+ installs: non-Steam games, per-game settings, Proton prefixes, extensible scripts. | Games only; not a media library. |
| **HDR Auto Pilot** | A plugin can toggle **gamescope HDR** with `gamescopectl hdr_enabled` before launch and restore it after; it also patches library badges. | It toggles the display, but a native Wayland app still gets no HDR surface (see §4). |
| **NonSteamLaunchers** | Media services in Gaming Mode are normally added as **browser shortcuts** (Netflix, Plex, Disney+, YouTube…). | Browser DRM/4K/HDR limits; no native app integration. |
| **Deck Shelves** | A full-screen home shelf is possible via route patching. | It also froze Steam Machines; the most brittle surface. |

There is **no Jellyfin video plugin** in the store. A Runefin plugin would be new.

---

## 4. HDR is not a Decky problem

The #2166 blocker is a **protocol** gap: gamescope exposes its own
`frog_color_management_v1` to clients, while wgpu/Mesa and native apps speak
`wp_color_management_v1`. Even with `gamescope --expose-wayland`, a native Wayland client does
not see an HDR surface. HDR Auto Pilot works for **games** because games are Proton/DXVK or
Steam's own clients on gamescope's HDR path.

A Decky plugin can toggle HDR on the display (`gamescopectl hdr_enabled`) and can relaunch
Runefin, but it cannot give Runefin's swapchain an extended-range format. XWayland is the only
other candidate; it needs a standing X11 socket (the Flatpak has `fallback-x11`) and this host's
RADV X11 WSI advertises only `SRGB_NONLINEAR`. Rune currently accepts only
`Rgba16Float + ExtendedSrgbLinear`; a PQ-only X11 surface would also need the PQ present pass we
deliberately skipped. Decky does not change that.

**Conclusion:** do not build HDR expectations on a plugin. Desktop Mode + KWin remains the
supported HDR path.

---

## 5. What to borrow, and what not to

**Borrow**

1. **Frontend/backend split.** A thin React QAM panel over a host-side API is a good shape: the UI
   is replaceable, and the host owns state and side effects. Runefin already has the host half in
   MPRIS; a companion can be a thin client.
2. **One-shot launcher lifecycle.** `Desktop Apps` creates a temporary shortcut, launches it,
   watches `RegisterForAppLifetimeNotifications`, and removes the shortcut when the app exits.
   That is the right pattern for "launch Runefin at this item" without polluting the library.
3. **Steam's own `collectionStore`.** A plugin can create a user collection ("Media") and add
   Runefin/streaming shortcuts through Steam's data API. That is less brittle than patching the
   home React tree and gives a visible Media grouping in the library.
4. **Manifest/API versioning.** If a companion is shipped, `plugin.json` + `api_version` + a
   pinned `@decky/*` version is the minimum for reproducibility.

**Do not borrow**

- **Root CEF injection and the debug port.** Owning a Steam UI injector is more fragile than
  using Decky, and enabling a DevTools port is a security trade for a media app.
- **React route patching / a home shelf.** Steam's UI internals change (the Sept/Oct 2026
  breakages); a shelf duplicates Runefin's own Home and is the most likely thing to freeze a
  console.
- **An unsandboxed plugin ecosystem.** Runefin is a single curated app; arbitrary plugins with
  user/root access are a liability, not a feature.
- **Decky as a dependency.** The core app must work on a stock Steam Machine with no root service
  and no CEF debug port.
- **HDR via gamescope.** See §4.

---

## 6. Options for Runefin

| Option | Effort | Risk | Value | Verdict |
|---|---|---|---|---|
| **A. Supported seam only** | none | none | the current tile, big-screen UI, Desktop Mode HDR | keep |
| **B. Runefin bridge** — deep links (`runefin --play <id>`, `runefin://play/<id>`) and a small local control/read API; MPRIS already covers transport | low–medium | low | every companion (Decky, phone, Home Assistant) can launch/control without Steam internals | **do first** |
| **C. Optional Decky QAM plugin** — Open Runefin, Now Playing/transport via MPRIS, Quit, create a Media collection; no route patches, no root flag | low | medium (unsupported, Steam UI updates) | a media entry and transport inside Steam for users who already run Decky | optional, after B |
| **D. Full media hub plugin** — home shelf, Jellyfin "Continue watching", QAM library | high | high (route patching, Jellyfin auth duplication, Steam UI churn) | duplicates Runefin's Home; a little faster access | not recommended |
| **E. Decky-like injector/plugin system for Runefin** | very high | high | none for a single media app | reject |

**Recommendation: A + B now; C as a separate optional experiment after B. D and E no.**

The core mixed-use wins are not plugin-system wins:

- **HDR**: Desktop Mode, KWin, already implemented.
- **Return to Steam / quit**: Runefin already quits through the same path as the tray and Steam's
  "Exit game"; a QAM "Quit" button is only a shortcut to that.
- **Suspend/resume, CEC, audio, controller**: app/session work already planned; a plugin cannot
  make them better than handling logind, CEC keys and the sink in the app.
- **24p**: gamescope exposes one refresh rate in Gaming Mode; Desktop Mode is the path. A plugin
  could experiment with Steam's refresh settings, but it is not a reliable API.
- **Media library**: keep it in Runefin. A plugin that re-implements "Continue watching" in React
  is more code and more failure modes for less screen.

---

## 7. If C is pursued

Scope it as a **separate, MIT, unsupported companion**, not part of the Runefin binary:

1. `plugin.json` with `api_version: 1`, no `root` flag; `@decky/api`/`@decky/ui` pinned.
2. QAM panel:
   - **Open Runefin**: find the host wrapper shortcut (`runefin-steam.sh`) or create a temporary
     one with `SteamClient.Apps.AddShortcut`; `RunGame(gameId, "", -1, 100)`; clean up on exit.
   - **Now playing**: metadata and transport through MPRIS
     (`org.mpris.MediaPlayer2.runefin`), which already supports play/pause/stop/next/previous/
     seek/volume/rate/fullscreen/shuffle/loop/raise/quit.
   - **Quit to Steam**: MPRIS `Quit`, which uses the app's normal shutdown path.
   - **Create a Media collection**: `collectionStore.NewUnsavedCollection` + `SaveCollection`,
     adding Runefin and selected non-Steam/streaming shortcuts. No route patch.
3. If **B** exists, add "Continue watching" entries that launch `runefin --play <itemId>` through
   a temporary shortcut. Without B, omit the list; do not read the Jellyfin token from Runefin's
   config in a plugin.
4. Build a zip and a `--decky` option in `scripts/steam-install.py` only when Decky is already
   present; never install Decky itself.
5. Test on a Steam Machine with Decky prerelease: QAM opens, overlay over a non-Steam Flatpak,
   MPRIS from the plugin, temporary shortcut cleanup, collection survives a restart, plugin
   removal is clean. Mark it experimental; a Steam client update can break it.

**Do not** submit it to the Decky store until it has survived a SteamOS preview update and the
overlay behavior for a sandboxed non-Steam app is confirmed. The store's testing bar is a real
commitment; an unmaintained entry that breaks is worse than no entry.

---

## 8. If B is pursued

- Parse `--play <itemId>` and `runefin://play/<itemId>`; the single-instance path already forwards
  a second launch's arguments to the first. Route it to the item's detail or the player, with the
  same auth/session checks as the UI.
- Keep MPRIS as the transport/control API; add a small read-only local endpoint only if a
  companion needs "Continue watching" without a Jellyfin login. Bind to localhost/Unix socket,
  require a token, no CORS, no command execution.
- Test: second launch forwards once, no duplicate window, invalid ids fail cleanly, a play link
  during playback asks before replacing the current item.

This is useful beyond Decky: phone remotes, Home Assistant, a browser "Play on TV", and a future
Steam shelf all use the same seam.

---

## 9. Verification needed before any claim

- Steam Machine (SteamOS 3.10.x) + Decky stable/prerelease: install, QAM, uninstall.
- QAM/overlay while Runefin (Flatpak non-Steam shortcut) is running; if the overlay does not
  inject, the plugin is only usable before launch and after return.
- `AppLifetimeNotifications` for a Flatpak shortcut; temporary-shortcut cleanup on crash.
- MPRIS from the plugin's user context (same session D-Bus).
- A Steam client/SteamOS update: does the plugin survive, and does it fail visibly rather than
  hanging the UI?
- HDR: unchanged; the plugin must not claim HDR in Gaming Mode.

---

## Sources

- [decky-loader](https://github.com/SteamDeckHomebrew/decky-loader) — README, `backend/decky_loader/injector.py`, `main.py`, `plugin.py`, `sandboxed_plugin.py`, `dist/plugin_loader-release.service`.
- [decky-installer](https://github.com/SteamDeckHomebrew/decky-installer) — `cli/install_release.sh` (`.cef-enable-remote-debugging`, root unit).
- [Decky wiki](https://github.com/SteamDeckHomebrew/wiki) — plugin development, route patching, env vars, review/testing.
- [decky-frontend-lib](https://github.com/SteamDeckHomebrew/decky-frontend-lib) / `@decky/ui` — `SteamClient.Apps` (`AddShortcut`, `RunGame`, lifetime notifications), `collectionStore`, `GameSessions`.
- Plugins: [Desktop Apps](https://github.com/allmazz/decky-desktop-apps), [Museck](https://github.com/Nezreka/Museck), [Junk-Store](https://github.com/ebenbruyns/junkstore), [HDR Auto Pilot](https://github.com/Tri70n/hdr-auto-pilot), [NonSteamLaunchers](https://github.com/moraroy/NonSteamLaunchersDecky), [Deck Shelves](https://github.com/santojon/Deck-Shelves).
- Steam Machine compatibility: decky-loader [#945](https://github.com/SteamDeckHomebrew/decky-loader/issues/945), [#964](https://github.com/SteamDeckHomebrew/decky-loader/issues/964), [#988](https://github.com/SteamDeckHomebrew/decky-loader/issues/988), [#1011](https://github.com/SteamDeckHomebrew/decky-loader/issues/1011), [PR #962](https://github.com/SteamDeckHomebrew/decky-loader/pull/962).
- HDR/gamescope: [SteamOS #2166](https://github.com/ValveSoftware/SteamOS/issues/2166), `gamescope/src/Apps/gamescopectl.cpp`, `steamcompmgr.cpp` (`hdr_enabled` convar).
- Runefin: `crates/runefin/src/mpris.rs` (transport/quit/raise), `scripts/steam-install.py` (tile/wrapper), `crates/runefin/src/main.rs` (single-instance argument forwarding).
