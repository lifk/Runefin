# Runefin — rules for agents

Read `docs/design.md` before changing a screen. If the design doc and the code disagree, the doc
wins until it is amended in the same change. The full rationale for each rule lives in
`docs/conventions.md`; this file is the checklist.

## Layout

- `crates/jellyfin-api`: HTTP and wire types only. No Rune imports, no UI decisions, no `unsafe`.
- `crates/runefin`: the app. `screens/` builds `El`; `state.rs` holds state; `net.rs` owns every
  call that can block; `theme.rs` holds the token table.
- `docs/`: `design.md` (the contract), `research/` (the evidence), `conventions.md`, `snapshots/`.

## Hard rules

1. **Never block the UI thread.** No network, disk or process work inside a handler or `poll`.
   Use `rune::Latest` for request/response work and the `setup` waker for long-lived workers.
   `poll` returns `true` only when something visible changed.
2. **Every interactive element has a stable `.key(..)`.** Keys are how tests, scripted input and
   `--tree` address elements. Renaming one re-renders the snapshots in the same commit.
3. **A change that can move pixels renders a snapshot.** `scripts/snapshots.sh` for the eight
   committed views, and `scripts/snapshots.sh --all` for any other screen the change touches.
   Commit the core views when they change; an intended diff says why.
4. **No `todo!()`, `unimplemented!()`, `dbg!()`, or `unwrap()` outside `#[cfg(test)]`.** Denied by
   the workspace lints. Do not silence them with `#[allow]`.
5. **No new dependency without a line in `docs/design.md`.** Nothing that pulls an async runtime,
   a web view, or a second text/render stack.
6. **The Rune rev is pinned** in `crates/runefin/Cargo.toml` — a commit of our fork's
   `runefin/player` branch, never a branch name. Bump it as a standalone commit: run
   `scripts/upstream-sync.sh`, update the rev, run `scripts/check.sh`, render
   `scripts/snapshots.sh`, include the core-view PNG diffs.

## API truth

The server contract is Jellyfin **12.4.0**, verified against the OpenAPI document (see
`docs/design.md` §12). Two shapes that are easy to get wrong and are pinned by tests:

- `/Items` omits watch state unless you pass `fields=UserData`; `/UserItems/Resume` exists,
  `/Users/{userId}/Items/Resume` does not.
- Playback reporting is three calls with three different payloads; the stop payload has no
  `IsPaused`/`CanSeek` and must carry the final `PositionTicks`.

## Testing

- `cargo test -p jellyfin-api`: parsing and URL tests, no network.
- `cargo test -p runefin`: `Runtime` drives real input through the real layout and springs.
  Assert on `rt.state`, `rt.find(key)`, `rt.presented(key)` — never on colours.
- No test hits the network. Mock at the `Client` boundary.
