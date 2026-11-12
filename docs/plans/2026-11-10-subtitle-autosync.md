# Subtitle auto-sync — feasibility and plan

**Status:** shipped 2026-11-12 (0.1.36 cycle, post-release). What differs from this plan is
recorded at the end, under "As shipped".
**Question:** can Runefin align a subtitle track to the audio automatically, the way Bazarr
does?

## What Bazarr actually does

Bazarr syncs files on the server: it calls [ffsubsync](https://github.com/smacke/ffsubsync),
which extracts the reference audio with ffmpeg, runs a voice-activity detector (webrtcvad), builds
a speech mask from the subtitle cues, cross-correlates the two, and rewrites the subtitle with the
estimated lag (and optionally a frame-rate ratio). It knows nothing about the player.

We cannot call that from a client without the media file, but we do not need to: the app already
has the stream URL, a GStreamer stack, the parsed cues and a per-track delay. A constant offset is
the common case, and it can be estimated from a few minutes of audio.

## What "sync" can mean here

| Level | What it fixes | Cost |
|---|---|---|
| **Manual delay** (exists) | a known constant offset | done |
| **Auto offset** | an unknown constant offset, the usual Bazarr case | this plan |
| **Auto offset + drift** | a frame-rate mismatch (`23.976` vs `25`) | same machinery, three windows, linear fit |
| **Rewriting the file** | sharing the corrected subtitle with other clients | out of scope; we adjust at playback |

Image subtitles (PGS/VobSub/DVB) cannot be re-timed this way: their cues are bitmaps served by the
server. The action is hidden for them.

## Design

1. **Reference audio, on a worker.**
   Build a short-lived pipeline on the item's stream URL, independent of the playing one:
   `souphttpsrc → decodebin → audioconvert → audio/x-raw,format=S16LE,channels=1,rate=8000 →
   appsink`. Pull 8–10 minutes around the playhead (or from the start when near it). GStreamer is
   already a dependency; no new crate, no ffmpeg.
2. **Voice activity.** 20 ms frames, RMS energy against an adaptive noise floor (10th percentile
   over a sliding window) plus a zero-crossing gate, producing a binary mask. This is the
   webrtcvad shape without the model; if accuracy disappoints, decoding a second band (100–4000 Hz
   energy) is a cheap improvement.
3. **Subtitle mask.** Cues are already text-time intervals. Mark a frame as speech when a cue
   covers it, with a small lead/trail dilation (±80 ms) to absorb cue padding.
4. **Correlation.** For lags from −60 s to +60 s in 20 ms steps, compute agreement between the
   shifted subtitle mask and the audio mask; normalise by the overlap. The peak is the offset. A
   coarse pass at 1 s over ±10 min runs first when the playhead is near the start and the file is
   long.
5. **Confidence.** Peak over mean in a window around zero. Below ~1.5, report "couldn't align" and
   leave the delay untouched; a wrong offset is worse than none.
6. **Apply.** Set `Subtitles::delay_ms` (already used by the overlay and the ±control), toast
   `Synced: +1.8 s`, and remember the previous value for an undo. Optionally persist per item and
   track in the config so a re-open keeps it.
7. **Drift, only if asked.** Split the window into thirds, estimate each, and fit a line; a slope
   far from 1 means a frame-rate mismatch. Do not enable by default: three times the audio and a
   worse failure mode than a constant offset.

## Where it lives

- `crates/runefin/src/subtitle_sync.rs`: masks, correlation, confidence, with unit tests on
  synthetic masks (injected offset, noise, silence). No I/O.
- `crates/runefin/src/audio_probe.rs` or a function in `player.rs`: the temporary decode to PCM,
  returning `Vec<i16>`; runs on `Latest`/a worker, never the UI thread.
- `state/in_app_state.rs`: a `sync` request, its result, the confidence and the undo.
- `screens/in_app.rs`: a **Sync to audio** row in the subtitle panel next to Delay, with progress
  and the result; hidden for image tracks and for live TV.

## UX

- Panel row: `Sync to audio` → `Syncing… (18 s)` → `Synced +1.8 s` or `No clear match`.
- A second press re-runs; the delay control still allows manual correction afterwards.
- Keyboard: no new global shortcut; the panel row is enough.
- The toast carries the number, so a user can undo with the existing delay steps.

## Risks and limits

- **Silence or music** in the window: no match. Mitigate by sampling the middle of the file rather
  than the first minutes, and by reporting low confidence.
- **Cue padding**: subtitle start/end rarely match speech exactly; the dilation and the 20 ms grid
  absorb most of it, but the result is typically ±0.2 s, not sample-accurate.
- **Server transcode**: the stream may be an HLS transcode; decoding it on a second connection
  costs server CPU. Cap the probe at ~10 minutes and cancel it when the panel closes.
- **Licensing**: doing the VAD ourselves keeps ffsubsync out; calling it as an optional external
  tool would add a GPL dependency and a spawn path, like mpv.
- **Not a fix for broken cues**: a subtitle with progressive drift or duplicated lines cannot be
  repaired by an offset.

## Prototype order

1. Pure `subtitle_sync` module with synthetic tests — a day, no app changes.
2. The audio probe on a local file, offline (ignored test): decode, VAD, compare against a known
   offset.
3. Wire the worker and the panel row; one manual pass on a real episode with a deliberately
   shifted SRT.
4. Persist the per-item offset and add the undo.

Rough effort: 2–3 days for steps 1–3 in a focused pass, plus tests and one live check.

## What I would not do

- Bundle ffsubsync or webrtcvad as a dependency.
- Rewrite the subtitle on the server (needs file access and a write path the API does not offer).
- Claim frame-rate sync before a fixture with a 23.976/25 mismatch proves the drift fit.

## As shipped (2026-11-12)

- `crates/runefin/src/subtitle_sync.rs`: the envelope is **band-limited to 300–3400 Hz** (a pair of
  second-order biquads) and then **modulated**: each 20 ms frame's level minus the level around it
  over a second. Both steps came out of measuring against this library, and both were necessary:
  a plain RMS envelope put the correlation peak at a spurious −27.8 s on a music-heavy episode,
  and the band limit alone left a −21 s peak on a film. With the modulation feature, five windows
  of *28 Days Later* agreed on −0.1…−0.4 s (confidence 0.22–0.40, contrast 0.05–0.17) where the
  same windows had scattered from −0.7 s to +0.0 s before.
- The cue mask keeps 40 ms of dilation, the search is a 100 ms coarse grid over ±30 s refined to
  20 ms, and a result must reach confidence 0.2 with a ±1 s contrast of 0.05. The signs are pinned
  by tests: a positive result is how much *later* the cues should play, i.e. what the delay control
  takes.
- **Two windows, and disagreement is no answer**: the worker decodes from five seconds before the
  playhead and, if that window is unsure, another 240 s further on; a strong first answer
  (confidence ≥ 0.35) skips the second. `subtitle_sync::settle` takes the more confident and
  returns `None` when the two differ by more than 800 ms.
- `crates/runefin/src/audio_probe.rs`: a private playbin of the file's **direct stream** — audio
  only, `sync=false`, mono S16 at 8 kHz — with a 60 s deadline, a cancel flag, a seconds-decoded
  counter, and the window's start taken from the first buffer's PTS rather than the seek that was
  asked for. Tested end to end against a generated WAV, no server: the probe decodes it and the
  aligner recovers a known offset (and keeps item time for a window that starts mid-file).
- **A live check, kept as an ignored test** (`a_real_track_lines_up_with_real_audio` with
  `RUNEFIN_PROBE_ITEM`, `RUNEFIN_PROBE_SRT`, `RUNEFIN_PROBE_START_MS`): it decodes through the
  app's own probe and prints the alignment. On this server it reports −0.40 s and −0.32 s on two
  windows of *28 Days Later* (a track that is slightly early, which is what the estimator is for),
  and refuses the two windows of a music-heavy anime episode it was tried on. That is the intended
  shape: useful where there is dialogue, silent where there is not.
- `state/in_app_state.rs`: `sync_in_app_subtitles` (file URL, window from the playhead, cues,
  worker), `apply_subtitle_sync` (delay, toast, and the three outcomes: applied, no clear match,
  error), cancellation when the panel closes, another track is chosen or playback stops.
- `screens/in_app.rs`: a **Sync to audio** row under Delay and Size, with `Listening… N s` while it
  runs and the verdict after; hidden for bitmap tracks and live TV.
- Tests: 5 synthetic-mask tests, 2 probe tests, 2 app tests (the row and its outcomes; the row's
  absence for live/bitmap).

Left out of this pass, in the order they would help: persisting the offset per item and track, a
second frequency band for the envelope, drift (three windows and a line fit), and asking the
probe's pipeline for a transcode URL when the file itself is not reachable.
