# SteamOS media integration plan

**2026-11-17.** Companion to `docs/research/21-steamos-media-integration-2026-11-17.md`. Ordered by
user-visible value and risk. The target is a polished Steam Machine media app, not a replacement
OS.

## Steps

- [x] 1. Steam entry installer + wrapper + generated artwork (stable appid, one tile, `--big-screen`, uninstall) — host fixture tested; Steam Machine validation pending
- [ ] 2. Session detection + fullscreen at create + per-mode geometry (`SteamTenfoot`, gamescope, `SteamDeck`, `--desktop` opt-out)
- [ ] 3. Couch first run: auto-discovery, single-server prefill, Quick Connect above credentials, Steam+X hint
- [ ] 4. Input: focus gate under overlay/OSK, last-active pad, trigger seek/right-stick volume, virtual-pad de-dupe
- [ ] 5. Durable quit and second launch: stop report flush, early signal install, instance per display, flag forwarding
- [ ] 6. CEC: media-key handling or cecd transport mapping; fix the docs
- [ ] 7. Audio routing: device row, sink-loss reselect, passthrough requirements in diagnostics
- [ ] 8. Suspend/resume: `PrepareForSleep`, pause/report, pipeline teardown, pad/audio re-enumeration, re-seek
- [ ] 9. Diagnostics + update notice + “repair Steam entry”
- [ ] 10. Desktop media profile script: default Desktop Mode, autostart Big Screen, CEC/power helper, `--restore`
- [ ] 11. Distribution: lower `--require-version`, vendor sources, Flathub submission
- [ ] 12. Hardware verification on Steam Machine/Deck: Gaming Mode, overlay/OSK, suspend, HDMI, CEC, 4K
- [ ] 13. Experimental only: HDR in Desktop Mode, custom gamescope session doc, Steam store evaluation

## Detail

The full feasibility matrix, code references, sources and the “what Valve must add” list are in
`docs/research/21-steamos-media-integration-2026-11-17.md`.

Principles:

- No `/usr`, `/etc`, `devkit-steam`, Steam UI injection, or per-item shortcuts.
- The app owns the 10-foot UI, input, OSK hint, audio and suspend contract; Steam owns the shell.
- One stable identity; a wrapper is the launch path so a Steam environment regression cannot
  brick the tile.
- Desktop Mode is a supported optional media profile, not the default; a custom session is
  documented as unsupported.
- Never promise HDR, 24p, TrueHD or overlay behavior that the platform does not expose; diagnose
  and fall back honestly.
