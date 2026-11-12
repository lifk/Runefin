# Loudness normalisation and local intro detection — plan

**Status:** loudness shipped 2026-11-12 (wired, tested); the intro detector shipped as a tested
module but **not offered in the app** — see "As shipped" below for why. Both reuse the subtitle
sync's audio probe
(`crates/runefin/src/audio_probe.rs`): decode a window of an item's direct stream at 8 kHz mono on
a worker, with cancel and progress.

## 1. Loudness normalisation (ReplayGain's job, in a client)

**Problem.** A television mixes to −24 LUFS, a Blu-ray to −27, a web rip somewhere else. At night
the reader turns the volume to the quietest film and the next one wakes the house. The server
sends no loudness tags; the ReplayGain plugin writes them, and it needs the files.

**Design.**

- Measure, once per item, the *gated RMS* of a 90 s window from about 40 % into the file: frames
  quieter than −60 dBFS are silence and are excluded, so a quiet passage does not read as a quiet
  film. `loudness::measure(samples, rate) -> Option<Measurement>` returns dBFS.
- Gain = `TARGET_DBFS − measured`, clamped to ±12 dB, where the target is −23 dBFS RMS (the level a
  −23 LUFS EBU R128 mix sits at, measured as RMS after gating).
- Cache `~/.cache/runefin/loudness.json`: `{item id: {db, gain, saved}}`. Written by a worker,
  read at session start; a missing entry is measured in the background and applied when it lands,
  in the same session.
- Apply as a multiplier on the player's volume: `player.set_volume(in_app_volume * gain)`, so the
  reader's own volume and mute still act on top of it.
- Setting: **Settings → Playback → Loudness**, off by default. Off, nothing is measured and nothing
  is applied.
- Failure is silence: no audio, no measurement, no gain — never a guess.

**Tests.** Synthetic tones at −20 and −30 dBFS → gains differ by 10 dB and both land on the
target; silence and a DC offset read `None`; the cache round-trips and ignores a foreign version.

## 2. Local intro detection (Intro Skipper plugin's job, in a client)

**Problem.** The Media Segments plugins that mark intros run on the server, from audio
fingerprints of several episodes. A server without them has no intro to skip, and the app already
knows how to skip a segment.

**Design.** An intro is the same audio, at about the same offset, in every episode of a series.

- Take two episodes of one series (the playing one and its neighbour) and decode their first six
  minutes.
- Build 1 s frames of the band-limited, modulated envelope (the subtitle sync's features: a score
  under dialogue must not count as speech, and vice versa).
- Cross-correlate the two envelopes over ±25 s. The best shift is the offset between the episodes'
  intros; along that shift, a contiguous run of frames whose correlation stays above a threshold
  (≥ 10 s long) is the shared span — the intro.
- Confidence is the mean correlation inside the span and its length; below 0.5 over 10 s there is
  no answer. The span is stored per series in `~/.cache/runefin/intros.json` with the episode pair
  that produced it.
- Playback: the stored span is added to the session's segments exactly like a server `Intro`
  segment, so the existing skip pill, `S`, and the "Always" mode act on it unchanged. Detection is
  offered as a row in the player's gear sheet ("Find intro") for an episode whose series has none,
  and only when a neighbouring episode exists.
- **Ask by default.** A detected intro is offered, not taken: the failure mode of a wrong skip is
  worse than of a wrong offset, so `skip_segments = Always` still requires the reader to turn it on.

**Risks.** Animes and sitcoms that open cold every episode have no shared intro; a show whose
intro moves (a different length in some episodes) yields the modal span only. Both are read as "no
answer" more often than as a wrong one, and the tests pin that.

**Tests.** Two synthetic episodes with a shared 30 s span at the same offset and different content
elsewhere → the span is found within 1 s; two unrelated windows → none; a span shorter than the
minimum → none. A live check (ignored) prints what two real episodes of a series produce.

## Order

1. `loudness` module, its cache and the setting — small, self-contained, useful immediately.
2. `intro_detect` module and its tests, then the live check against a real series.
3. Wire detection to the gear row and the stored span into the player's segments.
4. If the live check is convincing, offer automatic detection on the series page ("Find intros
   for this series").

## As shipped

**Loudness.** `crates/runefin/src/loudness.rs` measures gated RMS (100 ms frames, frames under
−60 dBFS are silence and are excluded) and turns it into a gain against −23 dBFS, clamped to
±12 dB. `state/in_app_state.rs` measures **three windows** (at a fifth, a half and four fifths of
the run, 60 s each) and takes the median, because one window can land on a quiet scene; the gain is
applied on top of the reader's volume through one helper, so the volume control, the sleep fade and
the OSD all agree. Measured against this server: *101 Dalmatas* −23.8 dBFS (+0.8 dB), an anime
episode −25.9 (+2.9), *28 Days Later* −50.7 on its three windows — an exceptionally quiet mix,
which clamps at +12 dB and still plays quieter than the others. That is ReplayGain's own
limitation, not a defect of the measurement. The cache is
`~/.cache/runefin/loudness.json`; Settings → Playback → **Loudness** is off by default.

The probe also had a bug the live checks caught: its `audioconvert` downmix to **mono** divided by
the channel count, so a 7.1 film measured 18 dB below ffmpeg's stereo decode of the same file. It
now decodes stereo and averages the two channels itself, and a test pins that a file URL decodes
to the same samples ffmpeg wrote.

**Intro data from the server first.** Jellyfin already has this: `GET /MediaSegments/{itemId}`
returns the item's segments, and the app has asked for them at every playback start since the
player was rewritten — the "Skip intro" / "Skip credits" pill (Ask, five seconds' lead), the `S`
key and the Always mode all read that list (`state/in_app_state.rs`, `apply_in_app_segments`).
What the server cannot do on its own is *produce* them: the OpenAPI document for 12.4 exposes only
`GET /MediaSegments/{itemId}`, so a client cannot publish segments, and detection comes from a
provider plugin. On this server the list is empty for every item tried (`[]` for an episode with
five chapters and a film with nine), and the plugin catalogue carries no intro detector — only
"Local Intros" (pre-rolls) and "Chapter Segments Provider", which produced nothing here. The
practical route is the third-party Intro Skipper plugin, added through its own plugin repository
in the server's dashboard; once its segments exist the app skips with no change. That is why the
local detector is a **fallback for servers without a provider**, not the main path.

The local detector (`crates/runefin/src/intro_detect.rs`) builds band-limited envelopes (200 ms
frames), normalised and clamped to ±3σ, and looks for the shift and the longest stretch where two
episodes stay correlated (an "audible" gate keeps two silences from counting). Five synthetic
tests pass: a shared 30 s span at the same offset is found within a second, a span at a different
offset is still found, unrelated windows and a 4 s sting are refused.

Against two real episodes of a series on this server it finds **nothing**: the candidate is a 3 s
run at 192 s. The level contours of the two episodes differ across the whole window (their mixes
are not the same), so an envelope comparison cannot see the title sequence. The fix is a spectral
fingerprint — a short FFT per frame, hashed, as the Intro Skipper plugin does with chromaprint —
not a better envelope. The module therefore ships **unwired**: no button that would usually say
"no intro found", but the maths, its tests and an ignored live check
(`RUNEFIN_PROBE_SERIES=<id>`) that prints the candidate and the verdict on demand.
