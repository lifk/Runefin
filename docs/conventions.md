# Conventions

Rules that keep the codebase reviewable when several hands — human and agent — touch it. They
exist because Rune's author enforces the same ones upstream.

## Layout

- `crates/jellyfin-api`: HTTP and wire types only. No Rune imports, no UI decisions, no `unsafe`.
- `crates/runefin`: the app. `screens/` builds `El`; `state.rs` holds state; `net.rs` owns every
  call that can block.
- `docs/`: `design.md` (the contract), `research/` (the evidence), `snapshots/` (committed PNGs).

## Hard rules

1. **Never block the UI thread.** No network, disk or process work inside a handler or `poll`.
   Those go through `rune::Latest` or a worker holding the `setup` waker. `poll` returns `true`
   only when something visible changed. Rune's debug builds warn past 8 ms — a warning is a bug.
2. **Every interactive element has a stable `.key(..)`.** Keys are how tests, scripted input and
   the `--tree` dump address elements. Renaming one is a breaking change for the snapshot suite:
   rename it in the same commit and re-render.
3. **A change that can move pixels renders a snapshot.** Run `scripts/snapshots.sh` before and
   after; commit the PNGs. An intended diff says why in the commit message.
4. **No `todo!()`, `unimplemented!()`, `dbg!()`, or `unwrap()` outside `#[cfg(test)]`.** Denied by
   the workspace lints. Do not silence with `#[allow]` — finish the branch or delete the call.
5. **No new dependency without a line in `docs/design.md`.** Especially anything pulling an async
   runtime, a web view, or a second text/render stack.
6. **The Rune rev is pinned.** Bump it as a standalone commit: update the rev in
   `crates/runefin/Cargo.toml`, run `scripts/check.sh`, render `scripts/snapshots.sh`, include
   the PNG diffs. Never bump inside a feature change.

## Build and shape

- **`scripts/guardrails.sh` runs first in `scripts/check.sh`.** It checks that the `rune` rev and
  the `accesskit_atspi_common` patch name the same commit, that `crates/runefin/src` has no
  `use crate::*`, and that no file grows past its line ceiling. Files over 1,500 lines are listed
  in `scripts/file-size-exceptions.txt`; lower a ceiling when a file shrinks, and use
  `scripts/guardrails.sh --update` only when a file legitimately grew.
- **Local profiles:** `dev-fast` for the edit-compile-run loop, `release-quick` and
  `release-thin` for optimized local builds. The shipped artifact still uses `release` in CI;
  do not package a local profile.
- **Back/Escape closes the topmost overlay.** `crates/runefin/src/overlay.rs` owns the order;
  a new dialog, picker, sheet or player panel adds a variant there, a close arm in
  `state/overlay_state.rs`, and a case in `tv_reach_tests` when it takes focus. Do not add another
  `if s.something_open && key.key == Key::Escape` to `on_key`.

## State and the frame

- **`State::poll` is four phases** in `crates/runefin/src/state/poll.rs`: Ingest (inputs and
  answers), Derive (state and timers), React (follow-ups and machines) and Persist (external
  observers). Add a per-frame block to the phase whose inputs it needs; keep the phases
  contiguous so the order the app has always run stays visible.
- **A new state cluster is a struct on `State`.** Group fields that change together
  (`Toast`, `DownloadsState`, ...) and wrap the hot ones in `rune::Tracked`. Read them like the
  value (`self.toast.message.as_ref()`); write through the cell (`*self.toast.message = ...`).
  A `rune::region` around a screen can then subscribe to exactly the fields it read, but only
  add the region once every read inside it is `Tracked` or an explicit `region_with` input.

## Releases

- **A push to `main` that bumps the version publishes.** The `release` job in CI runs after the
  checks pass on a push to `main`:
  it builds the Flatpak, signs it, replaces the `gh-pages` repository, tags the commit if no tag
  exists, and attaches the bundle to the GitHub release. Pushing a `v*` tag does the same for an
  already-tagged commit; a version that already has a release is skipped, so a
  docs or CI-only push releases nothing.
- **The version is part of the change.** Bump `Cargo.toml`, add the metainfo entry and
  `docs/releases/<version>.md`, and re-render `docs/snapshots/settings.png` (its About row prints
  the version) in the same commit.
- **`scripts/publish-flatpak.sh` is for local testing**, never for the public artifact: it
  publishes whatever is in the worktree, signed by whatever keyring is on this machine.
- **The release key is a secret, not a file.** CI reads `FLATPAK_GPG_KEYRING`; only the public
  half lives in `packaging/flatpak/runefin-releases.gpg`. Losing the private half means every
  client must re-add the remote.

## Testing

- `cargo test -p jellyfin-api`: parsing and URL tests, no network. A new endpoint gets a test with
  a captured body.
- `cargo test -p seerr-api`: the optional Seerr companion's wire types and URL/error tests, no
  network. Its ignored `tests/live.rs` is read-only and opt-in with `SEERR_URL`/`SEERR_API_KEY`.
- `cargo test -p runefin`: `Runtime` drives real input through the real layout and springs at a
  fixed step. Assert on `rt.state`, `rt.find(key)` and `rt.presented(key)` — not on colours.
- `tv_reach_tests` walks every screen, overlay and player layer with the D-pad and fails if any
  visible focusable is unreachable from the first focus (`crates/runefin/src/tv_reach.rs`). The
  overlays covered include the card menu, pre-play sheet, playlist/collection/cast pickers, the
  new-playlist prompt, still-watching and resume-conflict cards, the shortcut sheet, online
  subtitle search and the shelf; the same walk covers the Seerr discover (explore, browse and
  requests), Seerr detail, person and remote screens. A new screen or panel gets a case there; a panel passes its
  layer key as the scope.
- Anything needing a server is mocked at the `Client` boundary. No default test hits the network;
  the ignored `tests/live.rs` checks are opt-in and run only when their `JELLYFIN_*` or `SEERR_*`
  variables are set.

## Style

- Comments explain *why*. Delete any comment that restates the code.
- Names are the app's words: `library`, `item`, `resume`, `handoff` — not `data`, `info`, `mgr`.
- Errors are values the UI can render (`Error::user_message()`), not strings formatted at the call
  site.
