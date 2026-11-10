# Snapshot captures

Eight views are committed here. Each is rendered by `scripts/snapshots.sh` from a named
`AppSpec` scene, headlessly, at a fixed virtual time and scale — the same renderer the window
uses, so a diff in the image is a diff in the UI. Personal live captures are never committed:
they contain the real session's ids and artwork.

| File | Scene | Size | What a reviewer reads it for |
|---|---|---|---|
| `home.png` | `home` | 1280×800 at 2× | the desktop shell, rails and cards |
| `detail.png` | `detail` | 1280×800 at 2× | hero, badges, actions and the season strip |
| `search.png` | `search` | 1280×800 at 2× | grouped results across movies, shows and people |
| `live-tv.png` | `live-tv` | 1280×800 at 2× | channels, now/next and progress |
| `settings.png` | `settings` | 1280×800 at 2× | the settings index and rows |
| `in-app-osd.png` | `in-app-osd` | 1280×800 at 2× | the player chrome and timeline |
| `home-big.png` | `home` | 1920×1080 at 1.5× | the 10-foot home; linked from the README |
| `library-big.png` | `library` | 1920×1080 at 1.5× | the 10-foot library; linked from AppStream |

These eight are the screens a change is most likely to touch, plus the two frames the README and
the AppStream metadata link to. The rest of the catalogue still exists — it is rendered on your
machine, not stored in the repository.

## Rendering

```sh
scripts/snapshots.sh              # the eight above, in place; commit intended diffs
scripts/snapshots.sh --all        # every scene into /tmp/runefin-snapshots
scripts/snapshots.sh --all DIR    # every scene into DIR
target/debug/runefin --list-scenes
```

`--all` covers the connect states, narrow windows (800×600), the 4K big-screen frames, the
player's subtitle/trickplay/up-next states, the Seerr pages, and the synthetic contrast art
(`home-art`, `home-art-light`, `detail-art`). Use it whenever a change touches a screen that is
not in the table, and look at the frames it writes.

**Rule.** A change that can move pixels renders the affected scenes with `--all` and inspects
them. If one of the eight committed views changes, commit it in the same change and say why.

## Reproducibility

Committed frames are **AMD/RADV-rendered**. A lavapipe re-render changes the antialiasing even
when the UI did not, so render every scene with one ICD and name it in the commit. The big-screen
frames use the 1280×720 design height: `*-big.png` is 1.5× (exactly 1920×1080) and `*-big-4k.png`
at 3× is 3840×2160, which is how the same layout is checked at both output sizes.

There is intentionally no committed frame for MPRIS (a bus service, not a surface), for the HDR
tone curve (GPU math pinned by CPU tests), or for a fullscreen window (the window manager owns
the mode).
