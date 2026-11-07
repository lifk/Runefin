# Runefin vs other Jellyfin clients — parity audit, 2026-11-04

> **Historical audit.** The live server was upgraded from Jellyfin 10.11.2 to **12.4.0** on
> 2026-11-05 (8 libraries, 3 live tests), and Phase 2b then shipped collection add/sort, playlist
> drag, live reminders, text size and shortcut remapping. The matrix below records the audit as
> performed and is not rewritten; [`../features.md`](../features.md) is the current inventory.

Method: the live server (`jellyfin.example`, Jellyfin **10.11.2**, 7 libraries, 4 live
channels) driven by the app, the ignored live tests, and this repository's own tests and
snapshots. The comparison set is **Jellyfin Web 10.11**, **Findroid** (Android), **Streamyfin**
(iOS/Android), **Jellyfin Media Player** (desktop, mpv) and **Moonfin 3.0** (Linux/Flutter).
Statuses are about *this* client against *this* server, not about what the protocol allows.

## 1. Capability matrix

Legend: **Done** = shipped, tested and exercised against the live server. **Partial** = shipped
with a named limitation. **Blocked** = the server or the host cannot do it here. **Out of scope**
= a deliberate product decision (one server, one window, no web view, no second render stack).

| Area | Jellyfin Web | Findroid | Streamyfin | Media Player | Runefin | Evidence |
|---|---|---|---|---|---|---|
| Server probe + password sign-in | yes | yes | yes | yes | **Done** | `tests::pressing_connect_*`, live `--test live` auth |
| Quick Connect | yes | yes | yes | yes | **Done** | `state::quick_state` tests, live code exchange |
| LAN server discovery | no | yes | yes | no | **Done** | UDP 7359 handshake, loopback responder test; blocked on this subnet |
| Session persistence, 0600 | web storage | keystore | keystore | config | **Done** | `config` round-trip tests |
| Home: Continue Watching, one card per series | yes | yes | yes | yes | **Done** | `home-*.png`, `merge_resume` tests |
| Home: Next Up | yes | yes | yes | yes | **Done** | streamed beside first paint; live 12 rows |
| Home: Recently Added rails | yes | yes | yes | yes | **Done** | `rail-recent-*` snapshots |
| Home: Collections rail | yes | partial | yes | yes | **Done** | `rail-collections`, streamed (46 ms) |
| Libraries: virtualised grid/list, A–Z, sort, direction, filters, genres | yes | yes | yes | yes | **Done** | 20 000-item frame test, `library-*.png` |
| Watched badge on cards and rows | yes | yes | yes | yes | **Done** | `is_watched` tests, `library.png` |
| Quick play from a poster | yes | no | no | yes | **Done** | hover play button, test proves it does not navigate |
| Search: whole server + this library, recents, grouped, see-all | yes | yes | yes | yes | **Done** | `search-*.png`, `Ctrl+K`/`Ctrl+F` tests |
| Detail: seasons, episodes, next-up, start over | yes | yes | yes | yes | **Done** | `detail*.png`, play/start-over tests |
| Cast & crew, person filmographies | yes | partial | yes | yes | **Done** | `person.png`, live credits |
| "More like this" | yes | yes | yes | yes | **Done** | `detail-similar.png`, live 13 recs |
| Collections browse | yes | yes | yes | yes | **Done** | `collection.png`, box-set detail |
| Playlists: list, play, add, create, reorder, remove | yes | partial | yes | yes | **Done** | `playlist*.png`, live create/add/move/read/remove/delete |
| Favourites and watched (toggle, immediate local update) | yes | yes | yes | yes | **Done** | item-menu tests, live write round-trip |
| Item context menu (right click / Menu key) | web has one | long-press | long-press | yes | **Done** | `card-menu.png`; add-to-playlist, watch state, resume removal |
| Direct play (MKV/MP4/HEVC/AV1 containers via GStreamer) | via browser | ExoPlayer | AVPlayer | mpv | **Done** | live 4K/1080p/H.264 probes |
| Zero-copy DMA-BUF, 4K HDR10 PQ | n/a | n/a | n/a | mpv | **Done** | 256 ms start, 24 fps, 232 MB |
| Subtitles: SRT/VTT/ASS, inline bold/italic/colour, track/delay/size | yes | yes | yes | yes | **Done** | subtitle snapshots, live SRT/ASS |
| Subtitle appearance: font, colour, outline, weight, position, opacities | web | partial | partial | yes | **Done** | `in-app-subtitle-style.png`, Settings rows |
| Online subtitle search + download | plugin | yes | yes | yes | **Done** | `in-app-subtitle-search.png`, live 2 results |
| ASS/SSA layout and style | web | partial | partial | yes | **Done** | requested as ASS: alignment, `\pos`, size, colour/opacity, bold/italic, outline/shadow and rotation; non-uniform scale approximated |
| PGS/VobSub/DVB image subtitles | web | yes | yes | yes | **Done** | `in-app-image-subtitle.png`; live PGS and VobSub MKS (1920×1080 bitmap), DVB validated on a generated MPEG-TS |
| IMDb / TMDb / RT scores | plugin | partial | yes | partial | **Done** | live detail IMDb 7.5, TMDb 7.5, RT 87%; public IMDb dataset |
| Trailers | web embed | browser | yes | yes | **Done** | local trailers in-app; YouTube via optional yt-dlp downloads into the cache and plays in-app, browser fallback |
| Audio track menu, preferred language | yes | yes | yes | yes | **Done** | `in-app-audio.png`, language preference |
| Chapters: ticks, list with stills, jump | yes | yes | yes | yes | **Done** | `in-app-chapters.png`; live chapter image fetched (5 KB) |
| Trickplay scrub preview + chapter label | yes | yes | yes | yes | **Done** | `in-app-trickplay.png` |
| Skip intro/credits segments | yes | yes | yes | partial | **Done** | `in-app-intro.png`, `S` key test |
| Up next + auto-roll + play-next queue | yes | yes | yes | yes | **Done** | queue tests, live S01E01→E02 |
| Play all / shuffle (library, season, collection, TV shuffle) | yes | yes | yes | yes | **Done** | live random episodes, `library-shows.png` |
| Playback speed, volume, mute, fit/fill | yes | yes | yes | yes | **Done** | OSD tests, remembered volume |
| Stats overlay (sink, codec, range, frames) | web (dev) | no | partial | mpv | **Done** | `in-app-stats.png` |
| Buffering indicator | yes | yes | yes | yes | **Done** | `in-app-buffering.png`, live no false positive |
| Mini player while browsing | web | no | no | yes | **Done** | `in-app-mini.png` |
| Idle/sleep inhibitor during playback | web | OS | OS | mpv | **Done** | live `idle inhibitor: true`, released |
| Live TV channels + guide + zapping + now/next | yes | yes | yes | yes | **Done** | `live-guide.png`, `in-app-live.png`, live tuner |
| Recordings browsing | yes | yes | yes | yes | **Done** | Recordings library opens as a library |
| Recording timers | yes | partial | no | partial | **Blocked** | server answers HTTP 500 from the Xtream provider |
| Transcode quality / bitrate cap | yes | yes | yes | yes | **Partial** | the server transcodes; the client direct-plays only until P4 adds PlaybackInfo + a quality menu |
| Remote control of other sessions | yes | no | no | no | **Done** | `remote.png`, session commands |
| Cast an item to another session | yes | no | no | no | **Done** | item menu `Play on…`, answer 204 from the server; playback needs a second device |
| SyncPlay | yes | no | no | no | **Out of scope** | needs a WebSocket client; no dependency for it (design rule 5) |
| Music / playlists of music | yes | yes | yes | yes | **Out of scope** | the server has no music libraries |
| Photos / books | yes | partial | partial | yes | **Out of scope** | no such libraries on the server |
| Downloads / offline | web | yes | yes | no | **Planned** | Q9: Settings storage path, queue, offline playback; not implemented yet |
| Multi-server aggregation | no | yes | yes | no | **Out of scope** | product is one server, one window |
| MPRIS / media keys | browser | no | no | yes | **Done** | zbus on the session bus; status, metadata, position, play/pause/seek/next/previous, fullscreen |
| Fullscreen window | browser | yes | yes | yes | **Done** | `F11` / OSD chip; Rune fork window API, `in-app-osd.png` |

## 2. Where Runefin is ahead

- **First paint**: home data in **~104 ms** (Next Up streamed, collections streamed); detail
  **~54 ms**; search **~66 ms**, with one shared HTTP connection pool for every clone of the
  client. Web and Flutter clients pay a larger startup and hydration cost.
- **Zero-copy 4K HDR10**: **256 ms** to first frame, **24 fps**, **232 MB** RSS, no CPU frame
  copies; seek **64–67 ms**.
- **IMDb/TMDb/RT at a glance**: TMDb and RT come from Jellyfin; IMDb comes from the public
  IMDb dataset (10 MB local index, no API key), so all three appear on one detail page.
- **Frame-budget regression test**: a 20 000-item grid must stay under 16 ms per frame.

## 3. Known gaps, in the order they would matter to a viewer

1. **DVB sample** — the server has no DVB track, so the path was validated against a DVB
   MPEG-TS generated from an on-server DVD track: ffmpeg transcodes it to DVD packets, the
   VobSub parser renders the cues. A live DVB tuner/recording would still be the final proof.
2. **Transcoding** — the server supports it (the official client transcodes here); the client is
   direct-play only until P4 adds PlaybackInfo and a quality menu. Keep direct play the default and
   transcode only for a chosen bitrate or unsupported codecs.
3. **Recording timers** — the Xtream provider returns 500 on `POST /LiveTv/Timers`; nothing to
   verify against until a supported tuner is attached.
4. **ASS transforms** — outline, shadow, rotation and scale render; a non-uniform
   `\fscx`/`\fscy` pair keeps its area because Rune scales uniformly, and 3D `\frx`/`\fry`
   are ignored.
5. **Provider trailers without yt-dlp** — the in-app path uses the optional external `yt-dlp`
   tool; when it is absent the trailer opens in the browser. No web view is involved.
6. **MPRIS signals** — properties and commands work; a change is published by the app's poll
   and read on demand, not pushed as `PropertiesChanged`. A desktop widget that caches without
   polling refreshes when it next asks.
7. **HDR passthrough** — HDR10/HLG is tone-mapped to the 8-bit sRGB window; passing PQ through
   unchanged would need an HDR surface and color space in Rune (M5d), not a flag.

## 4. Reproduce

```sh
scripts/check.sh            # fmt, clippy, 269 app (21 ignored) + 42 API tests, release build
scripts/snapshots.sh        # 94 scenes / 101 captures
# personal screenshots of the signed-in session, real artwork, never committed:
#   target/release/runefin --snap /tmp/home.png --live --at 30
# live, read-only except where noted
JELLYFIN_URL=... JELLYFIN_USER_ID=... JELLYFIN_TOKEN=... \
  cargo test -p jellyfin-api --test live -- --ignored --nocapture
cargo test -p runefin perf_live_pipeline -- --ignored --nocapture
RUNEFIN_PROBE_ITEM=<id> RUNEFIN_PROBE_RATE=1 \
  cargo test -p runefin state::in_app_state::tests::a_live_file_decodes -- --ignored --nocapture
```

The live contract test creates and deletes its own playlist; every other call is read-only except
the favourite/played probe, which restores the item's state before it returns.
