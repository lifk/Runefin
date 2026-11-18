# Steam entry (host installer)

A Flatpak has no post-install hook and its sandbox cannot write Steam's `shortcuts.vdf`. The
Steam tile therefore has to be created by a small host-side script. That is what
`scripts/steam-install.py` does:

```sh
python3 scripts/steam-install.py --dry-run
python3 scripts/steam-install.py
# later
python3 scripts/steam-install.py --uninstall
```

It installs the Flatpak if needed, writes a wrapper and a host `.desktop` launcher, asks the
running Steam client to add that launcher, reads the assigned appid back from
`userdata/<id>/config/shortcuts.vdf`, and copies generated portrait/header/hero/logo/icon art into
`config/grid`.

`--decky` also copies the optional Decky QAM companion into `~/homebrew/plugins/Runefin` when
Decky is already installed. It never installs or enables Decky, and the plugin is unsupported:
it adds an Open/transport/Quit panel and a "Media" collection, not HDR. `--uninstall` removes it
along with the wrapper, launcher and art. See
[`packaging/decky/runefin/README.md`](../decky/runefin/README.md).

The wrapper is the durable launch path, not a detail:

```sh
unset LD_LIBRARY_PATH LD_PRELOAD
exec /usr/bin/flatpak run --branch=stable --arch=x86_64 io.github.lifk.Runefin --big-screen "$@"
```

Steam can leak its runtime's `LD_LIBRARY_PATH`/`LD_PRELOAD` into `flatpak run`; a wrapper that
clears them survives that class of Steam client regression. The wrapper also logs to
`~/.var/app/io.github.lifk.Runefin/launch.log`.

Rules the script follows:

- never root; Steam and the app are per-user
- never edit `/usr`, `/etc`, `devkit-steam` or another shortcut
- never rewrite `shortcuts.vdf`; Steam owns it. The script uses Steam's running-client add and
  reads the result back
- one shortcut named Runefin; no per-film entries, no mutable appid
- generated artwork only; no SteamGridDB assets are bundled
- `--dry-run`, `--no-install`, `--no-steam`, `--decky`, `--art-dir`, `--ref` for controlled runs
- `--uninstall` removes the wrapper, launcher and art; the Steam entry is removed from Steam's UI

Requirements: Steam is running (Desktop Mode is fine), a user account has logged into Steam once,
and Python 3.10+ (SteamOS ships it). Artwork downloads from this repository if the script is run
outside a checkout; `--art-dir packaging/steam/art` uses a local copy.

What it cannot do: create the Flatpak entry as part of `flatpak install` (no such hook), make
Steam's overlay/OSK work inside a Flatpak sandbox, or guarantee the tile's appid if the user later
renames the shortcut or changes its command. Overlay/OSK/screenshot behavior must be verified on
the Steam Machine. A Flatpak Steam client has a private `/tmp`; if the running-client add does
not appear, add the launcher from Steam's UI and rerun the script to install the artwork.
