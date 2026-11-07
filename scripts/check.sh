#!/usr/bin/env bash
# Everything that must pass before a commit. Run from anywhere.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p jellyfin-api
cargo test -p seerr-api
cargo test -p runefin -- --test-threads=1
# The shipped artifact: a scene or fixture behind cfg(debug_assertions) compiles in tests and
# fails here, and only here, before release.
cargo build --release -p runefin
