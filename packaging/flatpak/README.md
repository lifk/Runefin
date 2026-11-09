# Runefin Flatpak

The Steam Machine package. Flatpak is SteamOS's supported way to install software: it lives on
the writable partition, survives OS updates and opens from Discover without root.

## Build

Needs `flatpak`, the Flathub remote and `org.flatpak.Builder`:

```sh
flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install flathub org.flatpak.Builder
packaging/flatpak/build.sh
```

The script builds the app against Freedesktop 26.08 and writes:

- `dist/Runefin.flatpak` — the offline bundle;
- `dist/flatpak-repo/` — a repository to host for updates.

It uses the checkout as its source. The build has network access because Cargo fetches the
pinned Rune fork from GitHub; a Flathub submission should replace that with `cargo vendor`
sources and drop the grant.

To sign a repository for a hosted `.flatpakref`, create a release key and pass its ID, the public
key file and the host-visible keyring:

```sh
gpg --quick-gen-key "Runefin Releases" ed25519 sign 1y
key=$(gpg --list-secret-keys --with-colons | awk -F: '/^sec:/{print $5; exit}')
gpg --export --armor "$key" > runefin.gpg
GNUPGHOME="$HOME/.gnupg" \
FLATPAK_GPG_KEY="$key" FLATPAK_GPG_KEY_FILE="$PWD/runefin.gpg" \
FLATPAK_REPO_URL="https://example.invalid/runefin/flatpak-repo" \
packaging/flatpak/build.sh
```

The builder app has the host home mounted; keep `GNUPGHOME` under your home (not `/tmp`, which
is private to the app). Without these variables the bundle still installs, but a hosted remote
is unsigned.

## Install

```sh
flatpak install --user dist/Runefin.flatpak
flatpak run io.github.lifk.Runefin
```

For the TV:

```sh
flatpak run io.github.lifk.Runefin --big-screen
```

The package exports **Runefin** and **Runefin Big Screen** desktop entries. The user config lives
at `~/.var/app/io.github.lifk.Runefin/config/runefin/config.toml`.

## Sending and updating

### One machine, over SSH (recommended first)

Enable SSH in SteamOS Developer settings, then from this checkout:

```sh
packaging/flatpak/deploy.sh deck@steam-machine
```

The first run copies `dist/flatpak-repo/` to
`~/.local/share/runefin-repo` on the machine, adds the `runefin` remote and
installs the app. Later runs sync the new commit and call `flatpak update`.
No web server and no root are involved. The Steam shortcut keeps working.

If the app was installed from a `.flatpak` bundle earlier, uninstall that copy
once (`flatpak uninstall --user io.github.lifk.Runefin`); its app data stays
under `~/.var/app/`, and the next deploy installs from the repository.

### Fast iteration over the LAN

```sh
packaging/flatpak/serve.sh
```

It serves the repository on port 8000. On the Steam Machine, add the printed
`http://<host>:8000/` URL as a `runefin` remote once and install. Rebuild with
`packaging/flatpak/build.sh`; the next `flatpak update --user` sees the new
commit. For a permanent repository, put the same directory behind HTTPS
(Caddy, nginx, Forgejo Pages or another static host) instead of the development
HTTP server. A Caddy site is one line:

```caddyfile
runefin.example.invalid {
    root * /srv/runefin
    file_server
}
```

### One-click repository

Host `dist/flatpak-repo/` at a stable HTTPS URL, set that URL when building, and
publish the generated `dist/Runefin.flatpakref`:

```sh
FLATPAK_REPO_URL=https://example.invalid/runefin/flatpak-repo \
FLATPAK_GPG_KEY=<key-id> FLATPAK_GPG_KEY_FILE=$PWD/runefin.gpg \
GNUPGHOME=$HOME/.gnupg packaging/flatpak/build.sh
```

Discover opens the `.flatpakref`; later builds are picked up by Discover or
`flatpak update`. Sign before publishing: the installer accepts an unsigned
bundle, but a hosted remote should be verified.

### Bundle fallback

For a machine without SSH or a repository:

```sh
flatpak install --user --reinstall Runefin.flatpak
```

That is manual; the repository path is what makes updates one command.

`flatpak update` checks the repository for a newer commit; it does not push.
Discover's update setting or a manual `flatpak update --user` applies it, and a
running app needs a restart afterwards. The runtime is shared, so a rebuild of
Runefin alone is small.

Updates are deltas against the existing commit, so only differences are
transferred; the Freedesktop runtime is shared with other Flatpak apps. Bump
`Cargo.toml`/the metainfo release when you want the version shown in Discover
to change, and restart the app after an update.

## Steam Machine

1. Install the bundle (or a hosted `.flatpakref` through Discover).
2. Desktop Mode → Steam → **Add a Non-Steam Game** → choose **Runefin Big Screen** from the
   application list. If it is not listed, add a shortcut with the command
   `flatpak run io.github.lifk.Runefin --big-screen`.
3. Leave Steam Input **off** for the native gamepad path. If it is on, use a standard
   Xbox-layout virtual pad.
4. Start it from Gaming Mode. Big Screen requests fullscreen and focuses the first item.

## TV remote (HDMI-CEC)

SteamOS 3.8.1+ forwards HDMI-CEC remote buttons from Gamescope into SteamOS as
keyboard input; no `/dev/cec` permission is needed inside the Flatpak. Enable
**Remote control input** in SteamOS Settings → Display / Power → HDMI CEC and
turn on CEC on the TV.

SteamOS's `cecd` maps Select/OK to **Enter**. Runefin's big-screen cards now
take Return through `focus_highlight`, but a few controls (notably some detail
actions) still only activate on **Space**. The mapping below sends Select to
Space so every control works, and adds the transport keys.

On the Steam Machine, with no checkout needed:

```sh
curl -fsSLO https://raw.githubusercontent.com/lifk/Runefin/main/packaging/flatpak/setup-cecd.sh
less setup-cecd.sh          # read it first; it only touches ~/.config/cecd
bash setup-cecd.sh --dry-run
bash setup-cecd.sh
```

From a checkout, `packaging/flatpak/setup-cecd.sh` is the same script. It:

- adds only the keys that are missing and keeps any value you already chose;
  `--force` replaces differing values as well;
- writes a timestamped backup next to the file before changing it;
- refuses a config with two `[mappings]` tables, an inline `mappings = { … }` or
  `[[mappings]]`, since cecd cannot read those;
- warns when `cecd` is not installed or the machine is not SteamOS, and still
  writes the file for when it is;
- restarts the `cecd` user service when it exists.

It maps OK to Space, Back to Esc, the remote's Menu key to Runefin's context
menu, and play/pause, skip and channel buttons to the keys the player already
uses. `packaging/flatpak/cecd.toml` is the same mapping as a reference file.

A TV remote is reported as a keyboard, not a gamepad. Steam Input is not
involved; the physical controller keeps working alongside it.

## Permissions

`--device=input` exposes `/dev/input` for `gilrs`; `--device=dri` exposes the GPU; network is
enabled for Jellyfin and image fetches; Wayland/X11 and PulseAudio are the display and sound
sockets. MPRIS owns `org.mpris.MediaPlayer2.runefin`, and notifications/portal talk names are
allowed. The package does **not** grant `--device=all`.

## Runtime limits

The Freedesktop runtime includes GStreamer 1.28, VA-API, ffmpeg, `notify-send` and `xdg-open`.
It does not include `yt-dlp` or `mpv`: YouTube trailers fall back to the browser, and the
GStreamer player is the primary path with no final mpv handoff. Both can be bundled later.

## Hosting a repository

`dist/flatpak-repo/` is a normal Flatpak repository. Host it over HTTPS and publish a
`.flatpakref`:

```ini
[Flatpak Ref]
Title=Runefin
Name=io.github.lifk.Runefin
Branch=stable
Url=https://example.invalid/runefin/flatpak-repo
IsRuntime=false
```

Discover opens that file; `flatpak install --user <url>/Runefin.flatpakref` is the terminal
equivalent. Sign the repository with a release key (`FLATPAK_GPG_KEY=... packaging/flatpak/build.sh`)
before treating it as public.
