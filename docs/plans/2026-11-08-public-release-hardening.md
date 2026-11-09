# Public release hardening — execution plan

**Status:** published, 2026-11-08. The public tree is `lifk/Runefin@ee9ae08` (one initial
commit) and the pinned fork is `lifk/rune#runefin/player@5da4bbf6`. `v0.1.0` is tagged with the
Flatpak bundle attached. Remaining work is Flathub and release signing, not the publication
gate.
**Goal:** a public repository that anyone can clone, audit, build and install without access to
the current private network, and without exposing private infrastructure, identities or secrets.

**Rule:** do not create the public repository or push the current history until every P0 item
below is complete. Once a private commit reaches GitHub, caching and forks make it very hard to
remove.

## Progress log

### Applied 2026-11-08 (`ebb7435`)

- **P0.1 (tree side).** `crates/runefin/Cargo.toml` and `Cargo.lock` now name
  `https://github.com/lifk/rune` at the same rev; a local `git config url.<path>.insteadOf`
  resolves it to the fork during development. The build cannot work for anyone else until that
  repository exists.
- **P0.3.** Every private host, SSH clone URL, live server name and private LAN address in the
  current tree is replaced: `Cargo.toml`, `README.md`, `docs/engineering.md` (formerly
  `HANDOFF.md`), `docs/design-player.md`, `docs/research/06`, `docs/research/08`, both Seerr
  plans, the Flatpak metadata and scripts, and the demo/test fixtures. Fixture addresses now use
  the documentation ranges (`192.0.2.0/24`, `198.51.100.0/24`) and the demo server is `example`.
- **P0.4.** `quest.luis.home.Runefin` → `io.github.lifk.Runefin` in the manifest, metainfo,
  desktop entries, icon files, `.flatpakref` generator, deploy/serve scripts, README and features.
  AppStream developer id `io.github.lifk`, screenshots on `raw.githubusercontent.com`.
- **P0.5.** `LICENSE` (MIT, Luis Jurado), `THIRD_PARTY_NOTICES.md` (Rune, wgpu-external-frame,
  Inter and IBM Plex Mono, the crate tree, project assets, system tools), asset provenance, and a
  licence/disclaimer section in the README.
- **P0.6.** `.hermes-tmp.nY4mrt` removed; `.gitignore` covers `.env`, keys, `.cargo/config.toml`
  and the Flatpak/dist output. No credential, key or private host exists in the tree.
- **P1.1.** README rewritten for a stranger (install, requirements, big screen, development,
  licence, disclaimer); `CONTRIBUTING.md` and `SECURITY.md` added; `HANDOFF.md` became a public
  `docs/engineering.md` with the machine-specific and private-network notes removed.
- **P1.2.** `.github/workflows/ci.yml`: fmt, clippy, three test suites, release build on
  Ubuntu 24.04 with GStreamer and lavapipe, plus a job that fails on private hosts, private git
  sources and a missing licence/notice file, and runs gitleaks.
- **P1.3.** Token exposure through stream URLs, `mpv` arguments and logs is documented in
  `SECURITY.md`, with the config mode and the redaction rule for reports. Error messages now
  pass through `jellyfin_api::client::redact`, which replaces every `api_key`/`token` query
  value with `<redacted>`; two tests pin it.
- **P1.4.** Packaging URLs, app ID, metainfo and the Flatpak README moved to GitHub; no bundle,
  repository or signing key is tracked.

### Verified locally

`scripts/check.sh` passes (42 Jellyfin, 25 Seerr, 312 app tests, 21 ignored, release build);
`scripts/snapshots.sh` regenerates 150 captures, 85 of them changed only because the demo server
name and the fixture addresses changed.

### Published 2026-11-08

- `https://github.com/lifk/rune` — public, default branch `runefin/player` at `5da4bbf6`, the
  exact rev the app pins. The fork's other branches stay in the private mirror.
- `https://github.com/lifk/Runefin` — public, `main` is the single initial commit `ee9ae08`,
  authored `Luis Jurado <lifk@users.noreply.github.com>`; no private object was pushed.
- **Identity incident and fix.** The first publication carried the private development
  identity in the author field of two commits (the sync commits, not the initial one) and of the fork's eleven
  commits. Both histories were rewritten to `lifk@users.noreply.github.com`, the two
  repositories were deleted and recreated rather than force-pushed, and the pre-rewrite hashes
  now answer 422. Trees are byte-identical; only the identities and the pin changed.
- Repository settings: description, homepage, ten topics, MIT recognised, issues on, wiki and
  projects off, vulnerability alerts and Dependabot security updates on, `main` protected
  against force-push and deletion.
- The first CI run passed: both jobs green, including the clean
  `cargo build --release --locked` on a fresh Ubuntu 24.04 runner and the publication checks.

### Publishing a later commit

Development now happens directly on GitHub. `origin` is `https://github.com/lifk/Runefin` in
both checkouts; the pre-rewrite history survives only as the `private-archive` branch of the old
private remote (`forgejo`), which is never pushed to `origin`:

```sh
git push origin main     # after scripts/check.sh is green
```

### Verified after publication

- A fresh clone of `https://github.com/lifk/Runefin` builds with `cargo build --release
  --locked` (Rune fetched from `https://github.com/lifk/rune` at the pinned rev), no private
  network and no local override.
- The Flatpak rebuilt under `io.github.lifk.Runefin` against the public Rune commit, installed
  side by side with the old `quest.luis.home.Runefin`, and rendered a frame; its config lives
  under `~/.var/app/io.github.lifk.Runefin/`.

### Released

- Tag `v0.1.0` on `ee9ae08` and a GitHub release with notes and
  `Runefin-0.1.0-x86_64.flatpak` (10.7 MB) built from the repinned tree.
- Tag `v0.1.1` on `41ee306` with the playback pacing fix: `Runefin-0.1.1-x86_64.flatpak`
  (10.7 MB), checksum-verified against the local build, installed over 0.1.0 under the same ID.

### Still required

- Adopt a release signing key before publishing a hosted repository (the bundle works unsigned).
- P2: Flathub, update hosting, clone-size review.

---

## 0. Publication gate

The repo is publishable only when all of these are true:

- [ ] A clean clone on a machine with no access to the private network builds with
      `cargo build --release --locked`.
- [ ] `gitleaks detect --source . --log-opts=--all` reports nothing.
- [ ] A history and current-tree search finds no private hostnames, private SSH URLs, author
      emails or live credentials.
- [ ] `LICENSE`, `THIRD_PARTY_NOTICES`, the public README and the disclaimer are present.
- [ ] The app ID, Flatpak metadata and every URL point at the public GitHub identity.
- [ ] CI is green on the public repository, including a fresh build and the headless tests.

---

## 1. Decisions needed first

These block P0. The plan can be partially prepared, but the public repo should not be created
before they are answered.

| Decision | Recommended default |
|---|---|
| GitHub owner and repository name | `<owner>/Runefin`; use a user or project organisation you control |
| Flatpak/AppStream app ID | `io.github.<owner>.Runefin`; change now, before any users have an app ID and config path |
| Licence holder | “Copyright (c) 2026 <name or organisation>” in the MIT licence |
| Rune strategy | Publish the fork publicly as a mirror and pin the exact commit; vendor only if the mirror cannot be maintained |
| History strategy | Fresh public repo or `git filter-repo`; never push the current history unchanged |
| Release signing | A project release key kept out of the repo; public key only in the `.flatpakref` |

Do not rename the app ID after a public release: Flatpak config and update identity live under
`~/.var/app/<app-id>` and in the remote metadata.

---

## 2. P0 — blockers before creating the public repo

### P0.1 Publish the Rune source

The app currently pins a private Forgejo fork in `crates/runefin/Cargo.toml` and `Cargo.lock`.

- [ ] Publish the `runefin/player` fork on GitHub (or a public mirror), including its `LICENSE`
      and the `wgpu-external-frame` MIT/Apache notices.
- [ ] Keep the exact commit named by the pin.
- [ ] Update `crates/runefin/Cargo.toml` to the public URL and exact rev.
- [ ] Regenerate `Cargo.lock`; verify it contains no private git URL.
- [ ] Keep the private mirror as a development remote if useful; it must not be a build
      dependency of the public tree.
- [ ] Test `cargo build --release --locked` from a clean temporary `HOME` with the private
      network blocked.

If the fork is not public, the alternative is to vendor the patched Rune source with its
licence. That makes the repository much larger and moves maintenance burden onto the app; the
public fork is the cleaner option.

### P0.2 Sanitise the history, not just the current files

The current history contains private repository URLs, private LAN/server names and author
emails that belong to a private domain. Deleting them in a new commit leaves them in every
existing clone and in GitHub's object database.

- [ ] Choose a fresh/recreated public repository **or** rewrite every commit with
      `git filter-repo`.
- [ ] Replace private host suffixes and SSH paths with `example.com` / `user@example.com`.
- [ ] Rewrite author emails to a GitHub noreply address; keep the project identity consistent.
- [ ] Remove internal-only files or replace them with public handoff notes.
- [ ] Run the secret scan over every ref and tag, not only `main`.
- [ ] Keep the original private repository as the archival copy; do not mirror its history.

Recommended: a fresh public repository with a single reviewed initial commit. It is the only
way to be certain that old objects cannot leak; the private repo remains the historical record.

### P0.3 Remove private infrastructure from the current tree

Replace rather than delete the useful documentation.

| Location | What to replace |
|---|---|
| `Cargo.toml` | `repository = "https://github.com/<owner>/Runefin"` |
| `crates/runefin/Cargo.toml` | public Rune URL and rev |
| `Cargo.lock` | regenerate; no private git source |
| `README.md` | app ID, install commands, no private host |
| `docs/HANDOFF.md` | private repo links, SSH clone commands, live server names; make a public engineering note or remove it |
| `docs/design-player.md` | private repo table and any private host |
| `docs/research/06…`, `docs/research/08…`, related plans | live Jellyfin/Jellyseerr hostnames and LAN addresses |
| `packaging/flatpak/*` | app ID, homepage, bug tracker, screenshot URLs, build/deploy paths |
| `docs/features.md` | Flatpak config path after the app-ID change |

Use `example.com`, `user@host` and `demo` names. Do not keep a "sanitised" private hostname that
still identifies the network.

### P0.4 Move the app identity to GitHub

- [ ] Rename `quest.luis.home.Runefin` to `io.github.<owner>.Runefin` everywhere.
- [ ] Rename the desktop files, icon files and metainfo XML to match.
- [ ] Update Flatpak `finish-args`, build commands, `.flatpakref`, `serve.sh`, `deploy.sh` and
      README examples.
- [ ] Update the documented config path from
      `~/.var/app/quest.luis.home.Runefin/...` to the new app ID.
- [ ] Re-run `flatpak-builder-lint manifest` and AppStream validation.
- [ ] Rebuild and reinstall the Flatpak once under the new ID; verify the old and new app data
      paths do not collide.

This is a breaking change only for test installs today; after a public release it would be a
user-visible migration.

### P0.5 Licence, notices and assets

- [ ] Add a root `LICENSE` (MIT) with the chosen copyright holder.
- [ ] Add `THIRD_PARTY_NOTICES` covering Rune (MIT), Inter and IBM Plex Mono (OFL), the vendored
      `wgpu-external-frame` (MIT/Apache), and the Rust dependency tree. Generate with
      `cargo-about` or maintain a reviewed list.
- [ ] State the licence of `Assets/Icon.png`, `Assets/Logo_with_name.png` and the demo subtitle
      image; confirm they are original or properly licensed.
- [ ] Note that GStreamer, mpv, ffmpeg and yt-dlp are system/optional tools; if a release ever
      bundles them, add their notices and source-offer obligations.
- [ ] Add the README licence section and a short “not affiliated with Jellyfin, Valve/Steam or
      the Rune project” disclaimer.
- [ ] Check for copied GPL code from Jellyfin/Moonfin/Jellyfin Media Player; the research notes
      say the work is behavioural only, but the public audit should confirm it.

### P0.6 Secret and artefact hygiene

- [ ] Remove the empty tracked `.hermes-tmp.nY4mrt`.
- [ ] Add `.env`, local config, keys, `dist/`, `.flatpak-build*`, `.flatpak-builder/`, `target/`
      and generated bundles to `.gitignore`; verify none are tracked.
- [ ] Confirm no token, API key, GPG private key, SSH key or live server address exists in the
      current tree or history.
- [ ] Check the Flatpak repository/bundle is untracked; never commit a release key or a bundle.
- [ ] Run `gitleaks` with `--log-opts=--all`; fix or rotate anything found.
- [ ] Search for the private host fragments and author emails with `rg`/`git grep`; run it in the
      clean clone too.

---

## 3. P1 — before the first public tag or release

### P1.1 Public README and docs

- [ ] Rewrite the README for a stranger: what Runefin is, screenshots, requirements, build,
      run, Flatpak install, controller/CEC, known limits, licence, disclaimer.
- [ ] Remove internal commentary about the development machine, the private server, agent
      workflow, and private repositories.
- [ ] Keep the design/feature/research docs that add value; sanitise internal references.
- [ ] Add a `CONTRIBUTING.md`, issue/PR templates, and a short support policy.
- [ ] Add `SECURITY.md` with a private reporting route and supported version policy.
- [ ] Add a `CODE_OF_CONDUCT.md` if the repository accepts outside contributions.

### P1.2 CI and reproducible builds

- [ ] Add GitHub Actions for `cargo fmt --check`, `cargo clippy`, both test suites and a clean
      release build.
- [ ] Install `libgstreamer1.0-dev`, `libxkbcommon-dev`, `libudev-dev`, Vulkan/Mesa and lavapipe
      in CI; do not make live-server tests run by default.
- [ ] Cache Cargo, but verify a cold build in a clean runner.
- [ ] Add a job that checks `Cargo.lock` has no private sources and that the app metadata URLs
      point at the public repository.
- [ ] Decide whether snapshot rendering is a CI job; it needs a GPU or lavapipe and should not
      make basic PRs fail.
- [ ] Protect `main`: no direct force-push, required checks before release tags.

### P1.3 Security and token handling

- [ ] Audit every place a Jellyfin token enters a URL, log, process argument, screenshot or
      error message. In particular the direct-stream `api_key` URL and mpv handoff.
- [ ] Add log/error redaction and a rule that issue reports must not include tokens or private
      URLs.
- [ ] Document the config file mode (`0600`), where it lives and how to revoke a session.
- [ ] Keep the “one Jellyfin server, one user” security model explicit; this is not a
      multi-tenant server.

### P1.4 Packaging and release infrastructure

- [ ] Rebuild the Flatpak metadata, icons, desktop files, `.flatpakref` and repository URLs under
      the new app ID and GitHub owner.
- [ ] Host the Flatpak repository at a public HTTPS URL (GitHub Pages/releases or a static host);
      do not require the private Forgejo.
- [ ] Sign the repository with a release key; publish only the public key in the `.flatpakref`.
- [ ] Do not commit `dist/`, the `.flatpak` bundle, the Flatpak repo or signing material.
- [ ] Run `flatpak-builder-lint manifest`, AppStream validation and a clean install from the
      public bundle/repository.
- [ ] Decide whether the first release is bundle-only, repository-only or both.

### P1.5 Repository metadata

- [ ] Description, topics, homepage, issue URL and licence are set on GitHub.
- [ ] Default branch is `main`; the sanitised history has no private refs/tags.
- [ ] Branch and tag rules are in place before the first release.
- [ ] Enable GitHub secret scanning, Dependabot alerts and private vulnerability reporting.
- [ ] The README makes clear that Runefin is an unofficial client and that the user supplies the
      Jellyfin server and any media.

---

## 4. P2 — after the public repository exists

- [ ] Flathub submission, once the public Rune source and release key policy are settled.
- [ ] Public release hosting and update channel; verify a fresh user can install and update.
- [ ] Review snapshot storage (currently ~29 MB) if clone size becomes a problem; do not move
      the visual contract out of the repo without a replacement review path.
- [ ] Add a public roadmap that keeps the private research notes out.
- [ ] Revisit app naming/trademark if a conflict is found.
- [ ] Add reproducible-build attestations or signed release checks if distribution grows.

---

## 5. Verification commands

Run these in the sanitised clone, not the private checkout:

```sh
# Build and tests without the private network
cargo build --release --locked
cargo test -p jellyfin-api
cargo test -p seerr-api
cargo test -p runefin
scripts/check.sh

# Dependencies and licences
cargo tree --locked
cargo metadata --locked >/dev/null

# No private sources or credentials in the tree
rg -n "forgejo|home\\.luis|jellyfin\\.home|jellyseerr\\.home|hermes@|luis@" .
gitleaks detect --source . --log-opts=--all

# Flatpak metadata
flatpak run --command=flatpak-builder-lint org.flatpak.Builder \
  manifest packaging/flatpak/quest.luis.home.Runefin.yml
flatpak run --command=flatpak-builder-lint org.flatpak.Builder \
  appstream packaging/flatpak/quest.luis.home.Runefin.metainfo.xml

# Check a clean clone, with a temporary HOME and no access to the private LAN
git clone <public-url> /tmp/runefin-public
cd /tmp/runefin-public
cargo test -p runefin
```

The exact linter command changes after the app-ID migration; keep it in the release checklist.

---

## 6. Risks that remain after the checklist

| Risk | Mitigation |
|---|---|
| A private URL survives in an old commit | Recreate the repo from a reviewed initial commit; scan every ref, not just `main`. |
| The public Rune fork drifts | Pin an exact rev, keep the fork's `runefin/player` branch documented, and test the public clone in CI. |
| App-ID rename breaks an existing install | Do it before the first public release; document the migration path. |
| Third-party licence obligation missed | `cargo-about`/manual notices, asset provenance and a binary notice file. |
| Tokens leak through URLs/logs | Redaction, header-based streaming where possible, no token in issue templates. |
| Trademark complaint | Unofficial-client disclaimer, no Jellyfin/Valve logos, project-name check. |
| Snapshot/repo size grows | Keep the current PNGs; revisit LFS only if clone size becomes a practical problem. |

## 7. Out of scope for this plan

- Publishing the app on Flathub before P0/P1.
- Changing the product scope, adding features, or redesigning the UI.
- Renaming “Runefin” unless a trademark check requires it.
- Migrating the private Forgejo issue history; a fresh public issue tracker is fine.
