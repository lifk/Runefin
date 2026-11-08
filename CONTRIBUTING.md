# Contributing

Thanks for looking. Runefin is a small, opinionated codebase; these rules keep it that way.

## Before you start

- Open an issue for anything larger than a fix, so the approach is agreed before the code.
- Read [`docs/design.md`](docs/design.md) and [`docs/design-player.md`](docs/design-player.md).
  If the design document and the code disagree, the document wins until it is amended in the same
  change.
- Run `scripts/check.sh` before you push. It must be green.

## Build

```sh
scripts/install-hooks.sh   # once: the pre-commit hook refuses tokens and private hosts
cargo build -p runefin
cargo test -p jellyfin-api
cargo test -p seerr-api
cargo test -p runefin -- --test-threads=1
```

On Debian/Ubuntu the build needs `libxkbcommon-dev libudev-dev libvulkan1`; the player needs the
GStreamer 1.28 development packages. Without a GPU,
`VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json` runs the tests and snapshots on Mesa's
lavapipe.

## The rules

1. **Never block the UI thread.** No network, disk or process work in a handler or `poll`. Use
   `rune::Latest` for request/response work and the `setup` waker for long-lived workers.
   `poll` returns `true` only when something visible changed.
2. **Every interactive element has a stable `.key(..)`.** Keys are how tests, scripted input and
   `--tree` address elements. Renaming one re-renders the snapshots in the same commit.
3. **A change that can move pixels ships with a snapshot.** Run `scripts/snapshots.sh` before and
   after and commit the PNGs; an intended diff says why in the commit.
4. **No `todo!()`, `unimplemented!()`, `dbg!()` or `unwrap()` outside `#[cfg(test)]`.** The
   workspace lints deny them. Do not silence them with `#[allow]`.
5. **No new dependency without a line in `docs/design.md`.** Nothing that pulls an async runtime,
   a web view or a second text/render stack.
6. **The Rune rev stays pinned** in `crates/runefin/Cargo.toml`: a commit of the fork's
   `runefin/player` branch, never a branch name.

## Tests

- `cargo test -p jellyfin-api`: parsing and URL tests, no network.
- `cargo test -p runefin`: `Runtime` drives real input through the real layout and springs.
  Assert on `rt.state`, `rt.find(key)` and `rt.presented(key)` — never on colours.
- No test hits the network. Mock at the `Client` boundary. The `--ignored` live tests need your
  own server and are never part of the default run.

## Snapshots

`scripts/snapshots.sh` renders every scene at a fixed virtual time and scale. Committed images are
AMD/RADV-rendered; render all of them with one ICD and say which produced the new ones. Do not mix
ICDs in one change: the antialiasing differs even when the UI does not.

## Commits and pull requests

- Keep a commit to one idea, and write the message in the imperative mood.
- Describe the user-visible effect, not the file you touched.
- Say in the pull request what you verified (`scripts/check.sh`, a snapshot diff, a manual run) and
  what you could not verify.

## Licence

By contributing you agree that your work is licensed under the [MIT licence](LICENSE).
