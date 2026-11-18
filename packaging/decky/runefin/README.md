# Runefin — Decky companion (experimental)

A small optional plugin for [Decky Loader](https://github.com/SteamDeckHomebrew/decky-loader)
that puts Runefin in the Steam **Quick Access Menu**:

- **Open Runefin** / **Show Runefin** — launches the host installer's stable Steam tile, or
  creates one from `~/.local/share/runefin/runefin-steam.sh` if it is missing.
- **Play/Pause, Previous, Next, Stop** — through Runefin's MPRIS service
  (`org.mpris.MediaPlayer2.runefin`), the same controls a desktop media widget uses.
- **Quit to Steam** — MPRIS `Quit`, which runs Runefin's normal shutdown (stop report, save,
  release the idle inhibitor), then Steam returns.
- **Add to Media collection** — creates (or reuses) a user "Media" collection and adds the Runefin tile, through Steam's own `collectionStore`; no system collection is touched.

It has **no root flag**, no Steam route patches, no filesystem writes, and makes no HDR claim.
HDR remains a Desktop Mode feature (KWin); a Decky plugin cannot give a native Wayland app an
HDR surface in Gaming Mode. See
[`docs/research/22-decky-plugin-architecture-2026-11-18.md`](../../../docs/research/22-decky-plugin-architecture-2026-11-18.md).

## Requirements

- Decky Loader installed on the Steam Machine (it is third-party and unsupported by Valve).
- Runefin installed with the host installer (`scripts/steam-install.py`), which writes the
  wrapper path above. The plugin can launch an existing tile without the wrapper, but the
  wrapper is the durable launch path.
- Steam in Gaming Mode or Desktop Mode; the QAM panel itself is a Steam UI surface.

## Install

Copy this directory to the Decky plugin folder and restart Decky/Steam:

```sh
mkdir -p ~/homebrew/plugins
cp -r packaging/decky/runefin ~/homebrew/plugins/Runefin
sudo systemctl restart plugin_loader   # or reboot / restart Decky from its settings
```

`dist/index.js` is committed, so Node is not needed to install. To build from source:

```sh
cd packaging/decky/runefin
corepack pnpm install
pnpm exec tsc --noEmit
pnpm build
```

The plugin uses `@decky/api` and `@decky/ui`; the bundle includes `@decky/api` (LGPL-2.1) and
imports Decky's UI library at runtime. Runefin's plugin code is MIT, like the app.

## Uninstall

Remove `~/homebrew/plugins/Runefin` and restart Decky. The Steam tile and wrapper belong to the
host installer; remove them with `scripts/steam-install.py --uninstall` if wanted.

## Limitations

- **Experimental and unsupported.** Decky injects into Steam's CEF UI; Steam client updates can
  break it (see issues #964, #988 and #962 in the Decky repository). This plugin keeps its surface
  small (QAM only, no route patches) to reduce that risk, but cannot eliminate it.
- **The Steam overlay may not appear over a non-Steam Flatpak.** If it does not, the panel is
  usable before launching Runefin and after quitting it, not during playback. That is the same
  uncertainty as the OSK/F12 behavior in the host installer README.
- **No "Continue watching" list.** Runefin's own Home already has it; duplicating a Jellyfin
  library in a QAM plugin is the kind of code and auth surface this deliberately avoids.
- **No HDR toggle.** Gamescope's HDR does not reach a native Wayland app; the plugin will not
  pretend it does. HDR output is Desktop Mode + KWin.
- Not submitted to the Decky store. The store's testing bar (SteamOS preview, update survival,
  overlay behavior) has not been met here.

## Files

```
plugin.json       Decky metadata (api_version 1, no root flag)
package.json      build metadata
main.py           MPRIS/installation backend (host user)
src/index.tsx     QAM panel
rollup.config.js  @decky/rollup
dist/index.js     committed build; rebuild after changing src/
```
