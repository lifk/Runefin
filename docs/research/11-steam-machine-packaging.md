# Packaging Runefin for a Steam Machine — evidence and recommendation

**Status:** analysis, 2026-11-07. Companion to
[`plans/2026-11-07-steam-machine-packaging.md`](../plans/2026-11-07-steam-machine-packaging.md).

**Scope.** Which package format makes Runefin easiest to install on a Steam Machine, and what
that format has to provide for playback, a controller and Big Screen. It is not a general Linux
packaging survey.

---

## 1. What the Steam Machine gives us

SteamOS is an immutable, Arch-based image. Valve's supported way to add software is **Flatpak
through Discover**, with Flathub configured by default; Flatpaks live on the writable partition
and survive OS updates. Installing native Arch packages means unlocking the read-only root, which
is the path the OS explicitly discourages. Steam itself has no store entry for a private app; a
Steam shortcut launches whatever command it is given.

The practical install surface is therefore:

- **Flatpak** — Discover search/install, no root, updates through Discover, one application ID.
- A **`.flatpak` bundle** — copy one file and run `flatpak install --user`.
- A shared Flatpak repository — a `.flatpakref` opens in Discover and auto-updates.
- An **AppImage** — a file plus a shortcut, but no store, no updates, and host libraries.
- A native **pacman** package — root unlock and OS-update risk; not a candidate.

## 2. Why the app is a Flatpak fit

Runefin is one ELF binary with a small set of well-known runtime libraries:

- GStreamer 1.28 (app, base, video, allocators) for the in-app player;
- Vulkan through Rune/wgpu, with Mesa's RADV on a Steam Machine;
- Wayland/X11, `libxkbcommon`, udev for the native gamepad listener;
- D-Bus for MPRIS and notifications;
- `ffmpeg` for PGS/VobSub/DVB subtitle extraction;
- optional `yt-dlp` and `mpv` for trailers and the last-resort playback fallback.

The Freedesktop 26.08 runtime supplies GStreamer **1.28.6**, `codecs-extra`, Mesa/VA-API
extensions, ffmpeg 8.1.3, `notify-send` and `xdg-open`. Our build was verified inside the
sandbox: `playbin`, `avdec_h264`, `avdec_h265`, `vah264dec`, `vah265dec`, `matroskademux` and
`qtdemux` all resolve; `ldd /app/bin/runefin` reports no missing libraries. A 1080p Big Screen
snapshot rendered by the Flatpak build is identical to the native one.

The runtime does **not** include `yt-dlp`, `mpv` or `systemd-inhibit`:

- trailers fall back to opening the URL in the browser, which the app already supports;
- the GStreamer player is the primary path; the final mpv handoff is unavailable;
- idle inhibition is best-effort and may not suppress the display.

Those are acceptable for a first package on an AMD Steam Machine. Bundling yt-dlp (or mpv) is a
later, additive change; it does not affect the install format.

## 3. Controller inside the sandbox

`gilrs` reads `/dev/input`; it does not need `/dev/uinput` or raw USB. Flatpak exposes input
devices with `--device=input` (Flatpak 1.15.6+). Our installed package reports
`devices=dri;input` and lists `/dev/input/event*` inside the sandbox. Deliberately **not**
`--device=all`: the app has no need for raw USB, KVM or other host devices.

Steam Input should stay off for the native path. If it is on, it must present a standard
Xbox-layout virtual pad; `gilrs` then sees that pad through `/dev/input`.

## 4. Format comparison

| Format | Install on SteamOS | Updates | Runtime fit | Verdict |
|---|---|---|---|---|
| Self-hosted Flatpak repo | Add one `.flatpakref`, or `flatpak remote-add` once | Discover/`flatpak update` | Pinned Freedesktop runtime; GStreamer/VA-API included | **Recommended now** |
| Flathub | One click in Discover | Automatic | Same runtime | Best end state; needs a public Rune source and review |
| `.flatpak` bundle | Copy one 11 MB file, one command | Manual reinstall | Same | Best offline fallback |
| AppImage | Download, chmod, shortcut | Manual | Host GStreamer/ffmpeg; hard to guarantee | Fallback for non-Flatpak desktops |
| Native pacman | Unlock root | Tied to OS | Host libs; breakage risk | No |
| Steam-only | Still needs a binary | None | Whatever the shortcut points at | Shortcut layer, not packaging |

Measured on this machine (2026-11-07):

- `org.flatpak.Builder` 1.4.12 with Freedesktop SDK 26.08;
- build time 1m12s once the runtime/SDK were present;
- bundle **11 MB**, installed app **36.5 MB**;
- runtime download is shared with other Flathub apps.

## 5. Repository or Flathub?

Flathub is the easiest for a stranger, but it requires public, reproducible sources. Runefin
currently pins Rune at a private Forgejo git URL; Flathub builders cannot reach it. A
self-hosted repository is the correct first release:

1. build the bundle and repository with `packaging/flatpak/build.sh`;
2. host `dist/flatpak-repo/` over HTTPS (Forgejo releases/static hosting is enough);
3. publish `Runefin.flatpakref`, which Discover opens directly;
4. sign the summary with a release GPG key before calling it public.

Once the Rune fork has a public mirror and the tree can be vendored, the same manifest can be
submitted to Flathub; the application ID and build commands do not need to change.

## 6. Decision

Use **Flatpak** as the Steam Machine package. Ship a self-hosted repository plus an offline
`.flatpak` bundle, with the desktop entries and controller permission already in the manifest.
Keep AppImage out of v1; it would duplicate the dependency work without giving Discover or
updates.

## Sources

- Valve, [SteamOS](https://store.steampowered.com/steamos) (immutable, Desktop Mode).
- GamingOnLinux, [How to install extra software on SteamOS and Steam Deck](https://www.gamingonlinux.com/guides/view/how-to-install-extra-software-apps-and-games-on-steamos-and-steam-deck) (Flatpak/Flathub/Discover).
- Flatpak, [Sandbox permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html) (`--device=input`, since 1.15.6).
- Flatpak, [Extensions](https://docs.flatpak.org/en/latest/extension.html) (`codecs-extra`, VA-API extensions).
- freedesktop-sdk, [26.08 release](https://gitlab.com/freedesktop-sdk/freedesktop-sdk/-/releases) (GStreamer freeze and runtime contents).
- Local verification: `packaging/flatpak/build.sh`, `flatpak run … --snap`, runtime `gst-inspect`.
