# Why artwork went missing — 2026-11-26

The live 4K pass showed letter placeholders on cards whose art the server has, and the same
cards came and went. Three independent causes, all fixed together.

## 1. A card only ever asked for `Primary`

`home::poster` returned the series poster for an episode, otherwise only the item's
`ImageTags.Primary`. Plenty of films and collections have no Primary: the server returns
`Thumb`, `Backdrop` and `Logo` only. 28 Days Later is one — `Thumb` and `Backdrop`, no
`Primary` — so its card could only ever be the letter. The same applied to episode rows whose
only still is a `Thumb`/`Backdrop`.

**Fix:** `home::poster_source` walks SeriesPrimary → Primary → Thumb → Backdrop and returns the
kind with the id/tag; `home::own_image_source` does Primary → Thumb → Backdrop for an episode's
own still. Every view key and every request uses the same source and kind. A `Thumb`/`Backdrop`
is 16:9 and the card crops it to 2:3; that is a better outcome than a letter, and the server
has no poster to show.

## 2. The 4K cache was too small for the screen it was drawing

The 2x width cap did not exist: at a 3x window scale a 360 px poster became 1080 px. A decoded
1080×1620 poster is 6.99 MB base RGBA, ~8.9 MB with the mip estimate, so the 256 MB decoded
cache held **28** posters. Home requests every rail (60–150 images); Detail requests a poster,
a backdrop, 12 similar, a season's episodes and 12 people (~230–260 MB). `trim_image_cache`
then evicted the screen's own images.

Worse, `current_images` did not survive a visit:

- `image_request` returned `None` for a key already in `requested_images`, so returning Home
  never re-marked the loaded posters as current.
- `poll_ingest` clears `current_images` on every route change; the next pass could not rebuild
  it for already-requested keys.
- `trim_image_cache` was allowed to evict a current key once every resident key was current.

The result was art that loaded, was evicted, and then could not be requested again because it
was still in `requested_images` — silently a letter until a cache clear.

**Fix:**
- The image width multiplier caps at **2x**: 720 px for a 4K poster (a 176 pt card is 528
  physical px), 2560 for the Home backdrop, 3200 for the detail backdrop.
- The decoded cache is **768 MB**, which holds a 4K Home at the new widths.
- `image_request` always returns the key; `send_images` marks every key current before the
  already-requested check.
- Each screen's request pass now **replaces** `current_images`, so it pins exactly the keys that
  screen draws (Library pins its mounted rows, not every row ever visited).
- `trim_image_cache` skips current keys. If the pinned set alone is over budget it stays over
  budget rather than blanking the visible screen.

Budget at the new widths: 720×1080 poster ~3.95 MB, 1280×720 16:9 ~5.6 MB, 2560×1440 Home
backdrop ~14.7 MB, 3200×1800 detail backdrop ~23 MB.

## 3. A bad body was cached forever

`load_or_fetch` accepted any non-empty body from the server or disk and wrote it to the cache.
A 200 with an HTML error page or a truncated JPEG therefore became a permanent entry: Rune's
decoder failed, the card showed a letter, and every later visit read the same bytes. The
fallback URL was only tried on an **empty** body, not on a 404/5xx or an undecodable body.

**Fix:** the worker validates before caching and before returning disk bytes (a full decode for
posters, a header check for a trickplay sprite); a bad disk entry is deleted and fetched again;
the fallback is tried on any first failure; a `Msg::Image` with no image cache clears its
request so a later pass retries. A 0-byte resume backdrop now falls back to a real poster/still.

## 4. Still to do

- A negative cache for 404/400 per session so a permanently missing image is not retried on
  every pass.
- A bounded retry with backoff for a transient failure, instead of waiting for the next screen
  pass.
- A static colour/blur placeholder for the in-flight state; the letter is still what a card
  shows until the bytes arrive.
- Request the now-playing poster when playback starts from a card (no detail page had fetched
  it), and retire a Rune decode handle that fails after a successful fetch.
- Cancel stale queue work on a route/scale change; four workers still fetch old-scale images
  whose message is then dropped.

The Jellyfin client comparison that informed this: Jellyfin Web, Swiftfin, Streamyfin,
Wholphin and Findroid all use an ordered candidate list and a fallback path; Swiftfin removes a
failed source and retries the next, and Wholphin's documented art fix is a cache clear — the
same class of bug we just removed the root cause for.
