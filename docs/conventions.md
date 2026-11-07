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
3. **A change that can move pixels ships with a snapshot.** Run `scripts/snapshots.sh` before and
   after; commit the PNGs. An intended diff says why in the commit message.
4. **No `todo!()`, `unimplemented!()`, `dbg!()`, or `unwrap()` outside `#[cfg(test)]`.** Denied by
   the workspace lints. Do not silence with `#[allow]` — finish the branch or delete the call.
5. **No new dependency without a line in `docs/design.md`.** Especially anything pulling an async
   runtime, a web view, or a second text/render stack.
6. **The Rune rev is pinned.** Bump it as a standalone commit: update the rev in
   `crates/runefin/Cargo.toml`, run `scripts/check.sh`, render `scripts/snapshots.sh`, include
   the PNG diffs. Never bump inside a feature change.

## Testing

- `cargo test -p jellyfin-api`: parsing and URL tests, no network. A new endpoint gets a test with
  a captured body.
- `cargo test -p seerr-api`: the optional Seerr companion's wire types and URL/error tests, no
  network. Its ignored `tests/live.rs` is read-only and opt-in with `SEERR_URL`/`SEERR_API_KEY`.
- `cargo test -p runefin`: `Runtime` drives real input through the real layout and springs at a
  fixed step. Assert on `rt.state`, `rt.find(key)` and `rt.presented(key)` — not on colours.
- Anything needing a server is mocked at the `Client` boundary. No default test hits the network;
  the ignored `tests/live.rs` checks are opt-in and run only when their `JELLYFIN_*` or `SEERR_*`
  variables are set.

## Style

- Comments explain *why*. Delete any comment that restates the code.
- Names are the app's words: `library`, `item`, `resume`, `handoff` — not `data`, `info`, `mgr`.
- Errors are values the UI can render (`Error::user_message()`), not strings formatted at the call
  site.
