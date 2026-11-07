# Packaging for the Steam Machine — plan

**Status:** implemented for a local/user Flatpak, 2026-11-07. A hosted repository and Flathub
submission remain distribution steps. Implements the evidence in
[`research/11-steam-machine-packaging.md`](../research/11-steam-machine-packaging.md).

**Definition of done.** Runefin installs on a Steam Machine without root, through Discover or
one command, launches from Steam with `--big-screen`, sees the controller, plays through the
runtime's GStreamer/VA-API stack, and uses the same desktop and Big Screen layouts as the
development build.

---

## 1. Decision

**Flatpak**, with the Freedesktop 26.08 runtime.

- It is the format SteamOS supports through Discover and Flathub.
- It pins GStreamer 1.28, Mesa/VA-API, ffmpeg, fonts and udev instead of relying on the
  immutable host image.
- It gives `/dev/input` through `--device=input` for the native gamepad.
- A single self-hosted repository can auto-update; a `.flatpak` bundle works offline.

Not AppImage: it would still need host GStreamer/ffmpeg, has no Discover presence and no update
channel. Not AppImage or pacman in v1.

## 2. Package

`packaging/flatpak/io.github.lifk.Runefin.yml`:

- `io.github.lifk.Runefin` application ID;
- `org.freedesktop.Platform//26.08` / `org.freedesktop.Sdk//26.08`;
- `org.freedesktop.Sdk.Extension.rust-stable` for the build;
- Wayland/X11, PulseAudio, network, DRI and input; MPRIS name ownership; notification and portal
  talk names; PipeWire runtime directory;
- two desktop entries — **Runefin** and **Runefin Big Screen** (`--big-screen`);
- AppStream metadata and 128/256/512 px icons derived from `Assets/Icon.png`.

The manifest builds from the checkout with `cargo build --release --locked`. Build-time network
is enabled for the first package because the Rune fork is private; a public release should
replace this with `cargo vendor` sources and disable network in the sandbox.

## 3. Build and install

```sh
packaging/flatpak/build.sh
# dist/Runefin.flatpak
flatpak install --user dist/Runefin.flatpak
```

The same script writes `dist/flatpak-repo/` for a hosted remote and an optional GPG signature.
`FLATPAK_GPG_KEY` selects the signing key.

## 4. Steam Machine

1. Install the bundle or the repository (Discover accepts the `.flatpakref`).
2. In Desktop Mode, add **Runefin Big Screen** as a non-Steam game (Steam lists the exported
   desktop file), or add a shortcut whose command is
   `flatpak run io.github.lifk.Runefin --big-screen`.
3. Leave Steam Input off so the physical pad reaches `gilrs`; if it is on, use a standard
   Xbox-layout virtual pad.
4. Launch from Gaming Mode. `--big-screen` already requests fullscreen, the 720 pt design height
   and controller focus.

## 5. Verified

- Built with `org.flatpak.Builder` 1.4.12 on Freedesktop SDK 26.08.
- Installed as a user bundle and launched headlessly: a 1080p Big Screen snapshot is identical
  to the native frame.
- Runtime check: `ldd` clean; `playbin`, H.264/HEVC decoders, VA-API, Matroska and MP4 demuxers
  present; `/dev/input/event*` and `/dev/dri` visible in the sandbox.
- Permissions are `devices=dri;input`, not `all`.
- Bundle 11 MB; installed app 36.5 MB.
- `flatpak-builder-lint manifest` is clean; AppStream validation passes. The repo linter only
  asks for Flathub-mirrored screenshots, which a self-hosted repo cannot provide.
- A signed `.flatpakref` install was exercised end to end with a throwaway key; the bundle
  installs both signed and unsigned.
- `packaging/flatpak/deploy.sh` syncs a file repository over SSH; `serve.sh` serves the same
  repository over HTTP. `flatpak remote-ls` against the HTTP repo resolves the app.

## 6. Follow-ups

- Choose the permanent host (Caddy/nginx, Forgejo Pages, or the SSH file repo) and publish a
  signed `Runefin.flatpakref`; auto-updates then work in Discover.
- Generate `cargo-sources.json`, remove the build-time network grant, and publish the Rune fork
  publicly before a Flathub submission. The current manifest deliberately uses the checkout and
  network because the fork is private.
- Decide whether to bundle `yt-dlp` (trailers) and `mpv` (last-resort fallback). The runtime
  already has ffmpeg; trailers currently fall back to the browser.
- Optional AppImage for non-Flatpak desktops, using host GStreamer and the same desktop entries.
