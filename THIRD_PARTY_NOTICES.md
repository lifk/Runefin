# Third-party notices

Runefin itself is MIT-licensed (see [LICENSE](LICENSE)). A binary distribution also contains or
links the work below, and the notices in this file must ship with it.

## Rune

- Project: Rune — https://github.com/anomalyco/rune
- Runefin builds against a fork, https://github.com/lifk/rune, branch `runefin/player`, pinned to
  one commit in `crates/runefin/Cargo.toml`.
- Licence: MIT — Copyright (c) 2026 Anomaly.
- The fork adds the player surface, DMA-BUF import, HDR tone mapping and the big-screen input and
  fullscreen APIs, and is distributed under the same MIT terms.

## wgpu-external-frame

- Source: vendored inside the Rune fork (`vendor/wgpu-external-frame`).
- Licence: MIT OR Apache-2.0.
- It imports DMA-BUF (and, on other platforms, IOSurface) frames into wgpu as external textures.
  Runefin's zero-copy player path depends on it.

## Fonts embedded by Rune's `embedded-fonts` feature

| Font | Licence | Upstream |
|---|---|---|
| Inter (Regular, Medium, SemiBold) | SIL Open Font License 1.1 | https://github.com/rsms/inter |
| IBM Plex Mono (Regular) | SIL Open Font License 1.1 | https://github.com/IBM/plex |

The OFL permits bundling and redistribution with a application; it requires the licence text and
the reserved font names to be preserved. The full OFL text is in the Rune repository and in each
font's upstream distribution.

## Rust crates

The dependency tree is made up of permissively licensed crates. As of this commit every crate in
`Cargo.lock` is under MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unicode-3.0, BSL-1.0 or
CDLA-Permissive-2.0 (the last two are the `windows`/`webpki-roots` data crates), and none is
copyleft at the crate level. Regenerate the exact list before a release:

```sh
cargo install cargo-about   # once
cargo about generate about.hbs > third-party-licences.html
cargo deny check licenses   # optional policy check
```

Notable direct dependencies and their licences:

| Crate | Licence | Role |
|---|---|---|
| `gstreamer`, `gstreamer-*` (0.25) | MIT | GStreamer bindings for the in-app player |
| `gilrs` | MIT OR Apache-2.0 | Gamepad input |
| `image` | MIT OR Apache-2.0 | Image decode |
| `libbitsub-core` | MIT | PGS/VobSub bitmap subtitle decode |
| `serde`, `serde_json`, `toml` | MIT OR Apache-2.0 | Config and wire types |
| `ureq`, `rustls`, `webpki-roots` | MIT OR Apache-2.0, CDLA-Permissive-2.0 | Blocking HTTP |
| `zbus`, `tokio` | MIT | MPRIS and a small runtime for the D-Bus listener |
| `zeroize` | MIT OR Apache-2.0 | Wipes the session token from the heap |

## Project artwork

`Assets/Icon.png`, `Assets/Logo_with_name.png`, the icons under `packaging/flatpak/icons/` and
the demo fixtures under `crates/runefin/assets/` were produced for this project and are released
under the same MIT terms as the code. They contain no third-party artwork.

## System and optional tools

Runefin does not bundle these; it uses them when the user already has them:

- GStreamer, its plugins and the codecs they load (LGPL-2.1-or-later and per-plugin licences)
- `mpv` (GPL-2.0-or-later), used only as a playback fallback
- `ffmpeg`, used for bitmap subtitle extraction and trailer muxing
- `yt-dlp` (Unlicense), used only when the user asks for a provider trailer

If a future release bundles any of them, this file must be updated with their notices and, for
GPL/LGPL works, the corresponding source offer.

## Media

The screenshots in `docs/snapshots/` are rendered from synthetic fixtures in the test scenes, not
from a real library. Where a screenshot shows a title, the title is fixture data.
