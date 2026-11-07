# Seerr as an optional second service — discovery

**Status:** discovery, implemented. **S0–S3 shipped 2026-11-05**; the recommended decisions were
accepted (API key, one Discover page with tabs, films + seasons, 5 s visible-only polling, no 4K,
no background notifications). **Reconciled** against the docs rewrite in `d5b6bea` ("docs: bring
the inventory and contracts up to date") and the code it describes. The phased plan is in
[`../plans/2026-11-05-seerr-requests-plan.md`](../plans/2026-11-05-seerr-requests-plan.md).

**Date:** 2026-11-05.

**Question.** Can Runefin become a Seerr client — discover and request films and series, and show
the download progress of those requests — without weakening the one-media-server, one-window
design?

**Answer.** Yes, if Seerr is treated as an **optional companion service** behind its own crate, its
own worker and one page. Runefin stays a single Jellyfin server and a single window; Seerr is not a
second library and is not aggregated into the Jellyfin catalogue. The Jellyfin path stays untouched
and byte-for-byte the same when Seerr is not configured. Nothing here needs an async runtime, a web
view, a second render stack or a new codec. The cost is a second failure domain, a second
credential, TMDB images fetched from a third host, and the honesty problem of showing progress that
Radarr/Sonarr may not be reporting.

### What changed since the first draft

The docs were rewritten in `d5b6bea` after this discovery was written. The Seerr contract in §2 is
unchanged; the integration and the acceptance surface are not. These are the changes that matter:

| Docs change (2026-11-05) | Impact on this discovery |
|---|---|
| The app ships against **Jellyfin 12.4.0**, with **42 API + 269 app tests** (21 ignored) and **94 scenes / 101 captures**. | §9's counts and the plan's `scripts/check.sh` line are updated; the new crate adds a third test command. |
| `design.md` §7 is now **"Designed-for extensions (now shipped)"**; §12 is the **Jellyfin** appendix. | Do not append Seerr endpoints to §12. Change §7's heading back to "Designed-for extensions" and add **§7.2 Optional Seerr companion (proposed)**; keep the Seerr contract in this brief. |
| Settings now has **Playback, Subtitles & audio, Keyboard, Metadata, Storage, History, Account**, text size 85–130 %, a notification opt-in and remappable shortcuts. | The Seerr card joins a dense, searchable Settings page; every row must be in the `shown` filter and the tall `settings` capture. |
| **Desktop notifications** cover live reminders only. | Seerr v1 must not use `notify-send`; it shows an in-app toast only while its page is open. Reusing the opt-in is a later, explicit claim change (SD11). |
| **Text size** scales every `type_` token from 85 % to 130 %; the `type_::` constants now have function wrappers. | The Seerr page must use the `type_::*()` functions and be checked at 130 %; raw `TextStyle` constants would not scale. |
| `accessibility.md` audits Phases 0–2 and **101 snapshots**; focus rings and stable keys are mandatory. | The Seerr page, request panel and progress rows enter the audit; progress is text + bar, never colour alone. |
| **Live tests** are opt-in and now documented as three `JELLYFIN_*` variables. | The Seerr live test is a separate ignored test with `SEERR_URL` / `SEERR_API_KEY`, not a hidden fourth dependency of the Jellyfin suite. |
| `docs/research/07` is marked **historical** in places. | The Seerr opportunity is this new brief, not an entry appended to `07`. |

The naming: the user says "Jellyseerr"; the merged project is **Seerr** (Overseerr + Jellyseerr),
and the live instance still calls itself Jellyseerr in `applicationTitle`. The API is `/api/v1`
and is compatible with existing Overseerr/Jellyseerr clients.

---

## 1. What Seerr is, and what it is not

Seerr is a request broker. It owns a TMDB catalogue, user accounts (Jellyfin/Emby/Plex or local),
and an approval policy; it does **not** store media. When a request is approved it tells Radarr or
Sonarr to find the title, and it reads the downloader queue back to report progress.

Consequences for this design:

- **Seerr is not a Jellyfin library.** A Seerr result is a TMDB id plus artwork, not a `jellyfin_api::Item`.
  It cannot be played, added to Continue Watching or opened by the existing detail screen.
- **Availability is a state machine, not a boolean.** A request can be pending approval, approved,
  downloading, partially available, available or failed. The UI must show which, and must not draw
  a progress bar for a request nobody has approved.
- **Progress is a proxy.** Seerr only has per-download rows when its Radarr/Sonarr integrations
  have **Sync Enabled**. Without that switch, Seerr still reports `PENDING`/`PROCESSING`/`AVAILABLE`
  but `downloadStatus` stays empty. That is a Seerr setting, not a bug in the client.
- **There is no documented push channel.** The official web client uses SWR revalidation; the
  package has no socket server. Polling while the page is visible is the supported pattern.

---

## 2. The contract, verified on 2026-11-05

Sources: the `seerr-api.yml` OpenAPI document and the server source on the `develop` branch
([repo](https://github.com/seerr-team/seerr)), plus the Seerr release announcement
([2026-02-10](https://docs.seerr.dev/blog/seerr-release/)). A local instance serves the same
document at `/api-docs`. The API root is `<seerr>/api/v1`.

### 2.1 Authentication

| Mode | Call | What is stored | Notes |
|---|---|---|---|
| **API key** | send `X-Api-Key: <key>` on every call | the key | The key is the instance API key from **Settings → General**. The server acts as the original administrator (user 1) unless `X-Api-User: <seerr user id>` is also sent. |
| Jellyfin login | `POST /auth/jellyfin` `{username, password, hostname?, serverType?}` | Express session cookie (`connect.sid`) | Returns a `User`; the cookie is the session. Jellyfin-backed instances use this. |
| Local login | `POST /auth/local` `{email, password}` | the same cookie | For Seerr instances with local accounts. |
| Who am I | `GET /auth/me` | — | Returns the `User`: `id`, `username`, `email`, `permissions`, `requestCount`. |

`X-API-Key` is case-insensitive HTTP; the server source reads `X-API-Key`. The key is a bearer
credential and must be stored like the Jellyfin token: `0600`, wiped on sign-out and on replace,
never printed.

### 2.2 Probe and status

| Call | Returns | Use |
|---|---|---|
| `GET /status` | `{version, commitTag, updateAvailable?, commitsBehind?, restartRequired?}` | Prove the URL is a Seerr, and record the version for diagnostics. |
| `GET /settings/public` | `{initialized, plexClientIdentifier}` | `initialized: false` means the instance has not finished setup; say so instead of showing an empty catalogue. |
| `GET /auth/me` | `User` | Prove the key/cookie is valid and learn the Seerr user id. |

Both `/status` and `/settings/public` are public (no auth). That makes a useful Settings probe:
confirm the URL first, then the key.

### 2.3 Catalogue

| Call | Query | Returns |
|---|---|---|
| `GET /search` | `query`, `page`, `language` | `{page, totalPages, totalResults, results[]}` where each result is `MovieResult`, `TvResult` or `PersonResult`. |
| `GET /discover/trending` | `page`, `language`, `mediaType=all|movie|tv`, `timeWindow=day|week` | the same result union |
| `GET /discover/movies` | `page`, `language` | `MovieResult[]` |
| `GET /discover/tv` | `page`, `language` | `TvResult[]` |
| `GET /discover/movies/upcoming` | `page`, `language` | films not released yet |
| `GET /discover/tv/upcoming` | `page`, `language` | series not started yet |
| `GET /movie/{tmdbId}` | — | `MovieDetails` (title, overview, runtime, genres, cast, `mediaInfo`) |
| `GET /tv/{tmdbId}` | — | `TvDetails` (name, overview, `seasons[]`, `mediaInfo`) |

A result carries the fields needed for a card and for the request decision:

```jsonc
{
  "id": 693134,                 // TMDB id — the mediaId used to request
  "mediaType": "movie",         // movie | tv | person
  "title": "Dune: Part Two",    // TvResult uses "name"
  "releaseDate": "2024-02-27",  // TvResult uses "firstAirDate"
  "posterPath": "/1pdfLvkbY9ohJlCjQH2CZjjYVvJ.jpg",
  "backdropPath": "/xOMo8BRK7PfcJv9JCnx7s5hj0PX.jpg",
  "overview": "…",
  "voteAverage": 8.2,
  "mediaInfo": {                // absent when nothing has ever been requested
    "id": 12,
    "tmdbId": 693134,
    "mediaType": "movie",
    "status": 5,
    "status4k": 1,
    "downloadStatus": [],
    "downloadStatus4k": []
  }
}
```

`posterPath` and `backdropPath` are **TMDB paths, not URLs**. Seerr does not proxy artwork; the
paths are fetched from TMDB's image CDN (`https://image.tmdb.org/t/p/w342{posterPath}`,
`w780{backdropPath}`), with no API key. That is a third host the image worker has to know about.

### 2.4 Requests and progress

| Call | Query | Returns |
|---|---|---|
| `GET /request` | `take`, `skip`, `filter=all|approved|available|pending|processing|unavailable|failed|deleted|completed`, `sort=added|modified`, `sortDirection=asc|desc`, `mediaType=all|movie|tv`, `requestedBy` | `{pageInfo, results: MediaRequest[]}` |
| `POST /request` | body below | `201` + the new `MediaRequest` |
| `GET /request/{id}` | — | one `MediaRequest` |
| `GET /request/count` | — | `{total, movie, tv, pending, approved, declined, processing, available, completed}` |
| `DELETE /request/{id}` | — | `204` (the "Undo" path, if implemented) |
| `GET /media` | `take`, `skip`, `filter=all|available|partial|allavailable|processing|pending|deleted`, `sort=added|modified|mediaAdded` | `{pageInfo, results: MediaInfo[]}` |

`POST /request` body:

```jsonc
{
  "mediaType": "movie",   // "movie" | "tv"
  "mediaId": 693134,      // TMDB id
  "seasons": "all",       // TV only: "all" or [1, 2, 5]
  "is4k": false           // omit in v1
}
```

`pageInfo` is `{page, pages, results, pageSize}`. **`results` is the total count, not the page
length** — a classic mistake that would break pagination. The page length is `results.length`.
The `MediaRequest` rows are what the Requests page needs; `/media` is only useful for a future
"everything available" browser.

### 2.5 The shapes that matter

`MediaRequest` (string keys as serialized):

```jsonc
{
  "id": 42,
  "status": 2,                    // 1 PENDING, 2 APPROVED, 3 DECLINED, 4 FAILED, 5 COMPLETED
  "type": "tv",                   // media type
  "is4k": false,
  "seasonCount": 2,
  "seasons": [{"id": 1, "seasonNumber": 1}, {"id": 2, "seasonNumber": 2}],
  "requestedBy": {"id": 1, "displayName": "luis", "jellyfinUsername": "luis",
                  "username": null, "email": "", "permissions": 2},
  "createdAt": "2026-11-05T09:12:00.000Z",
  "updatedAt": "2026-11-05T09:20:00.000Z",
  "canRemove": true,
  // No title, poster or year on the request or its media: fetch /movie/{tmdbId} or
  // /tv/{tmdbId} and cache the summary by (mediaType, tmdbId). See 2.8.
  "media": {
    "id": 12,
    "tmdbId": 1396,
    "tvdbId": 81189,
    "mediaType": "tv",
    "status": 3,                  // see MediaStatus below
    "status4k": 1,
    "jellyfinMediaId": "abc123",  // present once available on the Jellyfin side
    "downloadStatus": [
      {
        "mediaType": "tv",
        "externalId": 81189,
        "size": 7_400_000_000,
        "sizeLeft": 3_100_000_000,
        "status": "downloading",
        "timeLeft": "00:12:34",
        "estimatedCompletionTime": "2026-11-05T09:33:00.000Z",
        "title": "Breaking.Bad.S01E02.1080p.BluRay.x264",
        "downloadId": "…",
        "episode": {
          "seasonNumber": 1,
          "episodeNumber": 2,
          "absoluteEpisodeNumber": 2,
          "id": 105
        }
      }
    ],
    "downloadStatus4k": []
  }
}
```

`MediaStatus` — the numbers the source uses (`server/constants/media.ts`):

| Value | Name | Meaning in the UI |
|---:|---|---|
| 1 | `UNKNOWN` | nothing known; no request row |
| 2 | `PENDING` | requested, not approved yet |
| 3 | `PROCESSING` | approved and handed to Radarr/Sonarr; download may or may not be visible |
| 4 | `PARTIALLY_AVAILABLE` | some seasons/episodes are in the library |
| 5 | `AVAILABLE` | the library has it |
| 6 | `BLOCKLISTED` | Seerr refuses to request it |
| 7 | `DELETED` | removed from the library after being available |

The published OpenAPI description says "6 = DELETED" and omits `BLOCKLISTED`; the source is the
truth. Pin the numbers in tests.

`MediaRequestStatus` — the source has **five** values, while the OpenAPI description only documents
1–3:

| Value | Name |
|---:|---|
| 1 | `PENDING` |
| 2 | `APPROVED` |
| 3 | `DECLINED` |
| 4 | `FAILED` |
| 5 | `COMPLETED` |

`DownloadingItem` (only present when Seerr's Radarr/Sonarr sync is on):

| Field | Type | Use |
|---|---|---|
| `title` | string | release name; the row's second line |
| `status` | string | Radarr/Sonarr queue status (`downloading`, `queued`, `paused`, `completed`, …); treat unknown values as text |
| `size` | number | bytes total for the **download**; `0` means unknown, do not divide |
| `sizeLeft` | number | bytes remaining |
| `timeLeft` | string | `"00:12:34"`; render as-is when plausible |
| `estimatedCompletionTime` | ISO string | fallback ETA; ignore `0001-01-01…` |
| `downloadId` | string | identity of the download; a season pack repeats it on every episode row |
| `episode` | object? | only for TV: `seasonNumber`, `episodeNumber`, `absoluteEpisodeNumber`, `id` (the live object has many more fields; ignore them) |

Progress is `(size - sizeLeft) / size`, computed per **download**, not per row. Group the rows by
`downloadId` first: a season pack produces one row per episode with the same `downloadId` and the
same `size`, so summing rows would count the pack once per episode (measured live: four episode
rows of 43 GB each for one 43 GB pack — 172 GB instead of 43 GB). One download contributes its
`size`/`sizeLeft` once:

```
downloads = unique rows by downloadId
total = Σ download.size, left = Σ download.sizeLeft, percent = (total - left) / total
```

If `size` is 0 for every download, show an indeterminate bar and the status text; never invent a
percentage. If every download is `completed` (`sizeLeft: 0`) but the media is still
`PROCESSING`/`PARTIALLY_AVAILABLE`, say **Downloaded · waiting for the library** rather than
drawing a live bar.

### 2.6 Errors

Non-2xx responses are mostly `{ "message": "…", "errors": ["…"] }`; a few middleware paths return
`{ "status": 403, "error": "…" }`. The client should read `message`, then `error`, then fall back to
the HTTP code, and put a human line on screen. The cases the UI must distinguish:

| Status | Meaning | UI |
|---:|---|---|
| 401 | no credential at all (`cookie 'connect.sid' required`) | "Seerr needs an API key." + Settings link |
| 403 on `/auth/me` | wrong `X-Api-Key` (or a user with no access) | "Seerr rejected the API key." |
| 403 elsewhere | permission, quota, or 4K not allowed | show the server's `error`/`message`; it is specific |
| 404 | unknown request id, or wrong base path | "Seerr does not know that request." |
| 500 on `/movie` or `/tv` | TMDB could not resolve the id (confirmed live) | "Seerr could not load that title."; keep the page |
| 409 | duplicate request (`DuplicateMediaRequestError`) | "Already requested" + refresh the row |
| 429 | rate limit | back off; the poll should retry later |
| transport | Seerr or the network is down | banner + Retry; Jellyfin stays usable |

A wrong key is **403, not 401** on this instance: the auth middleware leaves `req.user` empty and
the route refuses with `{"status":403,"error":"You do not have permission to access this
endpoint"}`. The no-key case is 401 with a `cookie 'connect.sid' required` message. The Settings
probe must special-case 403 from `/auth/me` as a key problem.

### 2.7 Gaps in the published schema

These bite an implementer who generates types from `seerr-api.yml`:

- `MediaInfo` in the spec has only `id`, `tmdbId`, `tvdbId`, `status`, `requests`, `createdAt`,
  `updatedAt`. The runtime `Media` entity also serializes `status4k`, `downloadStatus`,
  `downloadStatus4k`, `mediaType`, `imdbId`, `jellyfinMediaId`, `serviceUrl`, `mediaUrl`,
  `lastSeasonChange`, `mediaAddedAt`, and the service ids. A 2026 schema update started adding
  them; the live server is ahead of the published document.
- `MediaRequest` in the spec omits `type`, `seasons`, `seasonCount` and `canRemove`; the entity has
  them.
- The `MediaStatus` and `MediaRequestStatus` descriptions are stale (see above).

Therefore: **capture fixtures from a live instance**, not from the OpenAPI, and pin every shape
the app reads. The `seerr-api` crate should parse unknown fields permissively and treat every
field the UI does not strictly need as optional.

### 2.8 Live check, 2026-11-05

The read-only capture was run against `https://jellyseerr.example` with an admin API key.
This is the evidence S0.0 asked for, and it changes two parts of the plan.

| Measurement | Live value |
|---|---|
| `/status` | `3.5.1`, `commitTag: "local"`, no update/restart |
| `/settings/public` | `applicationTitle: "Jellyseerr"`, `mediaServerType: 2` (Jellyfin), `initialized: true` |
| `/auth/me` | user 1, `displayName` / `jellyfinUsername` `"example-user"`, `username: null`, `email: "seerr@example.invalid"`, `permissions: 2` (admin), `requestCount: 458` |
| `/request/count` | total 458; movie 295; tv 163; pending 0; approved 443; declined 0; processing 144; available 299; completed 14 |
| `/request` (50 rows) | request statuses 2 and 5; media statuses 3, 4, 5, 7; `canRemove: true` |
| `downloadStatus` before sync | **empty on every row**, including all 144 processing requests |
| `downloadStatus` after sync | 4 completed episode rows for one season pack, all with the same `downloadId` and the same 43 GB size; no in-progress row at capture |
| Radarr/Sonarr `syncEnabled` | **`false` on both at capture; enabled during S0.0 (`true`/`true`)** |
| Public flags | `partialRequestsEnabled: true`; `enableSpecialEpisodes: false`; `movie4kEnabled: false`; `series4kEnabled: false`; `hideAvailable: false`; `localLogin: true`; `mediaServerLogin: true` |

**Finding 1 — a request row has no title or artwork.** `GET /request` returns the `MediaRequest`
and its `media` row, but `media` carries only `tmdbId`, `mediaType`, statuses, download rows,
service ids and `jellyfinMediaId`; it has no title, poster, backdrop or year. The `/movie/{id}`
and `/tv/{id}` responses do carry them, and their `mediaInfo` also embeds the request. So the
Requests page needs **one summary call per unique `(mediaType, tmdbId)`**, cached by that key.
This is the N+1 the plan must budget for: statuses paint from `/request`, titles/posters arrive
from the summaries, and the 5 s poll only refreshes the request rows + `/request/count`.

**Finding 2 — sync is now on, but the first live rows are completed.** `syncEnabled` was false on
both integrations at capture. S0.0 turned it on through `PUT /settings/radarr/0` and
`PUT /settings/sonarr/0` (the body must omit the read-only `id`) and ran the `download-sync` job.
The tracker then populated `downloadStatus` with four completed episode rows for one season pack.
There is no in-progress download at capture, so the mid-progress bar is still fixture-tested; the
100 %/completed path is live. The honest no-row state remains `Processing · Waiting for a download`
on an instance with sync off.

**Finding 3 — a season pack is one download, many rows.** Each episode row repeats the same
`downloadId` and the same `size`; summing rows overcounts (four 43 GB rows for one 43 GB pack).
Group by `downloadId` before summing or percentage. The `episode` object is also richer than the
source type: it carries `seriesId`, `tvdbId`, `episodeFileId`, `title`, `airDate`, `runtime`,
`overview`, `hasFile`, `monitored`, scene-number fields and `id`; only four are needed.

**Other live shapes worth pinning:**

- `requestedBy` and `modifiedBy` are full `User` objects; `username` is `null` and `displayName`
  (or `jellyfinUsername`) is the readable name. Use a `display_name()` fallback, not `username`.
- `RequestCounts` has no `failed` field: `total, movie, tv, pending, approved, declined,
  processing, available, completed`.
- A movie detail's `mediaInfo` includes `requests`; a TV detail's `mediaInfo` includes `requests`
  and `mediaUrl`; `jellyfinMediaId` is present once the item is available.
- TV `seasons[]` is `seasonNumber`, `name`, `episodeCount`, `airDate`; `enableSpecialEpisodes` is
  false, so hide season 0 on this instance.
- `partialRequestsEnabled` is true, so the season picker can post a subset.
- `mediaInfo.status4k: 1` and `downloadStatus4k: []` are present even though 4K is disabled.
- Error probes: a wrong key returns **403** `{"status":403,"error":"You do not have permission to
  access this endpoint"}`; no key returns **401** `{"message":"cookie 'connect.sid' required",
  "errors":[…]}`; an unknown `/movie/999999999` returns **500**
  `{"message":"Unable to retrieve movie."}`. A wrong key is not a 401, and a missing title is not
  a 404.
- `request?take=2` gives `pageInfo {pages: 229, pageSize: 2, results: 458, page: 1}`; an empty
  filter gives `pages: 0, results: 0`. Search/discover use `page`, `totalPages`, `totalResults`
  (not `pageInfo`).

Fixtures captured from this run must be redacted (user email, `serviceUrl`, `mediaUrl`, hostnames,
`jellyfinMediaId`, keys) before they are committed.

---

### 2.9 Person credits, and the join with the server (added 2026-11-06)

A person page needs the same person from both catalogues. The join is a TMDB id, and both sides
expose one:

| Need | Endpoint | Shape measured live |
|---|---|---|
| Seerr biography and facts | `GET /person/{tmdbId}` | `name`, `biography` (885 chars), `birthday`, `deathday`, `placeOfBirth`, `profilePath`, `knownForDepartment`; **no credits** |
| Seerr filmography | `GET /person/{tmdbId}/combined_credits` | `{ cast: [25], crew: [1] }`; each row has `id`, `mediaType`, `title`/`name`, `character`, `releaseDate`/`firstAirDate`, `posterPath`, `voteAverage` |
| Seerr title cast | `GET /movie/{tmdbId}`, `GET /tv/{tmdbId}` | `credits.cast` (15 for the film, 300 for the series), `credits.crew`, `genres`, `tagline` |
| Server person id | `GET /Items/{personId}` | `ProviderIds: {Tmdb, Imdb}` and the biography in `Overview` |
| Server person by name | `GET /Items?includeItemTypes=Person&searchTerm=…&fields=ProviderIds` | one row, `ProviderIds.Tmdb` present |
| Server filmography | `GET /Items?personIds=…&fields=ProviderIds` | each title carries `ProviderIds.Tmdb`, so Seerr credits already on the server can be filtered |

The person page therefore shows the server's `personIds` credits first and the Seerr
`combined_credits` minus the TMDB ids already in the library second. A Seerr cast member resolves
to the server's person by name search and a `ProviderIds.Tmdb` equality check; a mismatch is
treated as "not in your library" rather than guessed.

## 3. What it costs inside Runefin

The app already has the right seams; the work is mostly new files rather than changes to existing
ones.

| Seerr concern | Runefin seam | Change |
|---|---|---|
| HTTP + wire types | new `crates/seerr-api` | `ureq` + `serde`, no Rune, no async. `jellyfin-api` stays Jellyfin-only (AGENTS layout rule). |
| Blocking calls | `crates/runefin/src/net.rs` | a `seerr::Client`, `Msg` variants for writes, and `Latest` reads; one thread per call, as today. |
| Config | `crates/runefin/src/config.rs` | `seerr_url`, `seerr_api_key` (0600), optional `seerr_user_id`, `seerr_user_name`; cleared by an explicit Settings action, not by a Jellyfin sign-out. |
| Routes | `state.rs` `Route` | `Route::Seerr` (the tab is state) and `Route::SeerrDetail { media_type, tmdb_id }`. |
| Request summaries | `state/seerr_state.rs` | cache `(media_type, tmdb_id)` → title/poster/year; fetch only visible rows; never on the poll. |
| Screens | `screens/seerr.rs`, `screens/seerr_detail.rs` | same shell: sidebar + content; no new navigation model. |
| Sidebar | `screens.rs` | `nav-seerr` after Playlists and before the `TOOLS` label, shown only when `seerr_url` is set, with an active-request count and a new compass icon. |
| Settings | `screens/settings.rs` | a **Requests (Seerr)** card: URL, API key, optional user id, Test, Clear; included in the settings search; a tall capture. |
| Posters | `net.rs` `ImageRequest` + `image.rs` | the image request learns a public (unauthenticated) source; TMDB paths join the same disk cache. |
| Tests | `seerr-api` fixtures + `runefin` `Msg`-driven state tests | no network, by construction; `scripts/check.sh` gains `cargo test -p seerr-api`. |
| Snapshots | `main.rs` scenes + `scripts/snapshots.sh` | 13–14 new scenes; update `docs/snapshots/README.md`, `HANDOFF.md` and the 94/101 counts. |
| Docs | `design.md` §6/§7.2/§8/§10, `features.md`, `README.md`, `HANDOFF.md`, `accessibility.md`, `conventions.md` | the exact amendment list is in the plan. |

### 3.1 Why a new crate, not a module in `jellyfin-api`

`jellyfin-api` is deliberately "HTTP and wire types only" for **Jellyfin**. Seerr has a different
auth header, a different error body, different status enums and a different base path. Putting it
inside `jellyfin-api` would make that crate a two-server bag and weaken the rule that lets a
reviewer reason about it. A sibling crate mirrors the existing shape (`Client`, `Error`,
`normalise_base_url`, models, captured fixtures) with no extra dependency: `ureq` 3.5 and `serde`
are already in the workspace through `jellyfin-api`, and the new crate adds no runtime, web view
or render stack. `docs/design.md` §6 gets the line that rule 5 demands, §7 gets a proposed §7.2,
and §12 stays Jellyfin-only; the Seerr contract lives in this brief.

### 3.2 Configuration and secret handling

```toml
seerr_url = "http://198.51.100.2:5055"
seerr_api_key = "…"          # 0600; cleared in Settings or on replace
seerr_user_id = 1            # optional: send X-API-User for attribution
seerr_user_name = "luis"     # display only
```

The key is a bearer credential. It follows the existing token rules: the file is `0600`, written
atomically, and the string is zeroized when replaced or cleared. It never appears in a toast, a
log line or an error message. A Jellyfin sign-out is a different session and does **not** clear the
Seerr key; the Settings **Clear** action is the explicit removal path. If the URL is plain HTTP on
a LAN, the key travels in clear text, exactly like the Jellyfin token; the Settings copy says so
and recommends HTTPS when the host is not trusted.

### 3.3 Worker and polling

- One `seerr::Client` built on demand from the config, like `State::client()`.
- Reads use `rune::Latest` (debounce, cancellation, one wake per answer): search, Discover rows,
  the Requests page, the detail panel, and the request summaries (a batch of `/movie`/`/tv`
  calls cached by `(mediaType, tmdbId)`).
- Writes (create request, optional delete) use `Worker::spawn` + a `Msg`, like favourites and
  watched-state writes.
- The Requests page polls **only while it is visible**: `route == Seerr && tab == Requests`, one
  request in flight, 5 s between answers, stop when the route changes or the in-app player opens.
  The active view is `filter=pending` + `filter=processing` + `/request/count`; the All view is
  `/request?filter=all` + the count. The app does not expose window focus today; if it does later,
  pause on blur as an optimisation, not a correctness rule. The timer itself does not return
  `true` from `poll`; only a landed answer does.
- A slow Seerr must not pile up requests: `Latest` cancels the previous poll when a new one is
  requested, and an `in_flight` flag prevents a second thread while one is running.

### 3.4 Posters from TMDB

`net.rs`'s image worker currently takes a `jellyfin_api::Client` and calls `fetch_bytes`. Extend
`ImageRequest` with a source:

```rust
enum ImageSource {
    Jellyfin(jellyfin_api::Client), // existing path, auth header
    Public,                          // TMDB: https://image.tmdb.org/t/p/w342…
}
```

`image::load_or_fetch` dispatches on it. The disk key includes the source, the TMDB id, the kind,
the `posterPath` (which changes when the artwork changes) and the width, so a TMDB poster never
collides with a Jellyfin one. The placeholder pipeline (27×17 blur-up) is unchanged. A failed
TMDB fetch is an image failure only: the card keeps its initial-letter fallback.

No new crate: the public fetch is a small `ureq` call in `seerr-api` (which already depends on
`ureq`), called by the image worker. `crates/runefin` does not gain a direct HTTP dependency.

### 3.5 What does **not** change

- The Jellyfin client, auth, home, library, detail, player and playback reporting.
- The product boundary: **one Jellyfin media server and one window**; Seerr is a companion
  service, not a second library and not an aggregator. The docs' "one server" wording is amended
  to say exactly that (SD10).
- The navigation stack and focus model: `Route::Seerr` and `Route::SeerrDetail` are ordinary
  routes; the Seerr detail does not push `DetailState` and never becomes a Jellyfin item.
- The theme tokens, card geometry and motion ladder: same cards, same 2:3 poster, same springs.
  Use the `type_::*()` functions (not the raw `TextStyle` constants) so the 85–130 % text-size
  preference applies.
- The no-network test rule: the app tests never construct a real `seerr::Client`.
- The notification scope: v1 adds no `notify-send` path (SD11).

---

## 4. Proposed experience

### 4.1 Entry point

One sidebar entry, **Discover**, placed after the libraries, Watchlist and Playlists and before
the `TOOLS` label (the current order is Home, libraries, Live TV, Playlists, then Tools). It is
visible only when `seerr_url` is configured. It carries a small count badge when the server reports
pending + processing requests. Without Seerr configured the entry is absent, so the shell is
exactly today's shell. Because the sidebar entry is hidden when Seerr is not configured, the
Settings card carries an **Open Discover** action so the setup card is reachable.

The page uses the `type_::*()` functions and must render at 130 % text; the request rows flex-wrap
rather than clip, and the search field stays usable in a narrow window.

The page has two tabs (a segmented control in the header):

- **Discover** — search and the catalogue rows.
- **Requests** — the download/progress list, with the count on the tab.

The last tab is session state, not a route; `Esc` from the page returns to the previous route, as
every other page does.

### 4.2 Discover

- A search field with the app's standard 150 ms debounce and a two-character minimum; every
  keystroke cancels the in-flight request (`Latest` already does this).
- With an empty field: **Trending this week** (one row), **Films** and **Series** (one row each,
  capped at 20). Three rows, not an unbounded fan-out — same rule as Home.
- Each card shows the TMDB poster, title, year, and an availability chip derived from
  `mediaInfo.status`:
  - `Available` (5) — in the library; the card can offer **Open in Jellyfin** when
    `mediaInfo.jellyfinMediaId` is present.
  - `Partially available` (4) — some seasons are in; for a series, the request panel offers the
    missing seasons.
  - `Processing` (3) — approved and downloading; a compact progress chip if `downloadStatus` has
    rows.
  - `Pending` (2) — awaiting approval.
  - `Blocklisted` (6) — no Request action; say why from the chip.
  - absent/`UNKNOWN` — a normal **Request** card.
- Clicking a card opens the Seerr detail (a full page or a right-side panel; either is consistent
  with the existing preplay sheet). It shows backdrop, title, year, runtime/seasons, overview,
  genres, score and the availability state. It uses Seerr data only; it never fabricates a
  Jellyfin item.

### 4.3 Requesting

- **Film:** one primary **Request** button. On success the panel becomes "Requested · pending
  approval" or "Requested · approved", and the Requests tab gains a row. A duplicate (409) says
  "Already requested" and refreshes.
- **Series:** a season list from `TvDetails.seasons`, each with its own number, episode count and
  air date; **All seasons** is the default, individual seasons can be toggled, and season 0
  (specials) is hidden when `/settings/public` says `enableSpecialEpisodes: false` — it is false on
  the live instance — otherwise it is visible but off by default. `partialRequestsEnabled` is true
  on the live instance, so a subset is valid. The button sends `seasons: "all"` or the chosen
  numbers. If `mediaInfo.status` is `PARTIALLY_AVAILABLE`, the already-present seasons are marked
  and cannot be requested again; this is where the "request missing seasons" idea becomes real.
- **Feedback:** the button is disabled while the POST is in flight; success/failure is a toast
  with the server's own message on failure (quota, permission, duplicate). The app's undo culture
  suggests "Requested · Undo" for 8 s via `DELETE /request/{id}` when the key may delete; make it
  optional and hide it when the server refuses.

### 4.4 Requests and progress

Each row: poster, title, year, type chip, requester (when more than one), the request status, the
media status, and progress when Seerr has it.

**Default view: Active.** The live instance has 458 requests and only 144 are processing, so a
plain newest-first list buries the rows the page exists for. The Requests tab opens on
**Active** — pending approval + processing + partially available — with an **All** chip beside it.
Active is two calls (`filter=pending`, `filter=processing`) merged; All is one (`filter=all`).
The sidebar badge uses the same active count.

| Request status | Media status | What the row says |
|---|---|---|
| PENDING | PENDING | **Awaiting approval** — no progress bar. |
| APPROVED | PROCESSING | **Downloading** if `downloadStatus` is non-empty; otherwise **Waiting for a download** (indeterminate). |
| APPROVED | PARTIALLY_AVAILABLE | some seasons/episodes are in; progress for the rest; "2 of 5 seasons". |
| APPROVED | AVAILABLE | **Available** + **Open in Jellyfin**. |
| DECLINED | — | **Declined**; no progress. |
| FAILED | — | **Failed**; the row explains that Radarr/Sonarr could not get it. |
| COMPLETED | AVAILABLE | **Completed** (or Available; the server's status wins). |

TV rows group by `downloadId`: show the download's size once (`Season 1 pack · 43 GB ·
completed`) and list the episode labels under it. Do not print the pack size on every episode row;
the live season pack repeats the same 43 GB on each of its four episode rows.
A row with no `downloadStatus` and a `PROCESSING` status is not a failure: it is the honest
"Seerr has not reported queue rows" state, and the copy says that Seerr's sync may be off rather
than showing 0 %. On the live instance this is **every** processing row, because both sync flags
are false (§2.8); the bar is proved with a synthetic fixture until sync is enabled.

States the page must own: not configured (setup card), unreachable (banner + retry, last answer
kept), loading (skeleton rows with the final geometry), empty, permission error, and a stale
answer whose age is visible ("Updated 12 s ago").

**Progress is text as well as a bar.** The value reads `42% · 3.1 of 7.4 GB · 12 min left`; the
bar is the visual echo, and an indeterminate state says `Waiting for a download` rather than
drawing a zero. This is the WCAG-safe form the accessibility audit asks for, and it survives the
130 % text size.

**No desktop notification in v1.** The app's `notify-send` opt-in currently covers live reminders
only. Seerr shows an in-app toast when a request becomes available **while the page is open**, and
nothing when it is closed. Reusing the opt-in is SD11 and changes the `features.md` limits claim,
so it is not part of this project.

### 4.5 Later: the library detail page

A Jellyfin item already has `provider_ids["Tmdb"]`. In phase 3, a series detail page can show
**Request missing seasons** by comparing the Jellyfin seasons with `TvDetails.seasons` and
posting the missing numbers. An available request row can jump straight to the Jellyfin detail
with `mediaInfo.jellyfinMediaId` (falling back to the Seerr panel if the id is not on this server).
This is the feature that makes the two services feel like one product rather than two tabs.

---

## 5. Constraints and risks

1. **Two failure domains.** Seerr being down must never look like Jellyfin being down. Every
   Seerr call has its own banner, retry and empty state; the Jellyfin session, home and playback
   are untouched.
2. **A second credential.** The API key is admin-level. Storing it `0600` and never printing it is
   the minimum; a shared Seerr instance would ideally use a user session instead (SD1 option B).
3. **The schema lies.** The OpenAPI omits `downloadStatus` and the real status enums; types
   generated from it will silently drop progress. Fixtures come from a live instance.
4. **Progress depends on a Seerr setting.** No Radarr/Sonarr **Sync Enabled** means no
   `downloadStatus`. This is not hypothetical: on the live instance both flags are `false` and all
   144 processing requests have empty download rows. The UI must say "waiting for a download" and
   not 0 %. Enabling sync is a Seerr setting, not a client change.
5. **TMDB artwork is a third host.** If the LAN blocks `image.tmdb.org`, posters fail; the
   letter fallback keeps the page usable. There is no Seerr-proxied alternative.
6. **Approval and quota.** A request is not a download. `PENDING` can last forever, and 403/quota
   messages must be shown verbatim; the client must not auto-approve or bypass them.
7. **Duplicates.** 409 is a normal answer, not an error: refresh the row and say "already
   requested".
8. **TV seasons.** `"all"` includes future seasons in Seerr's model; specials (season 0) are a
   product decision; a partially available series needs per-season arithmetic, not a boolean.
9. **Polling load.** 5 s while visible is cheap; a background poll on every screen is not. No
   polling when the page is closed (SD5).
10. **Version drift.** The merge is recent and the schema is moving. Feature-detect with
    `/status` and keep every unneeded field optional; never assume a field exists.
11. **Scope creep.** Direct Radarr/Sonarr, Seerr admin, issues, comments, deletion, 4K, automatic
    approval and notifications are separate projects. None is needed to satisfy the request.
12. **Naming.** The merged product is Seerr; the user says "Jellyseerr", and the live instance's
    `applicationTitle` is "Jellyseerr". Use "Seerr" in the crate and docs, and
    "Requests (Seerr / Jellyseerr)" once in Settings so nobody hunts for the wrong name.
13. **The "one server" claim.** The current docs repeat "one server, one window". Seerr is a
    second *service*, not a second media server. Say "one Jellyfin media server plus one optional
    companion service" everywhere; do not let the feature drift into multi-server aggregation.
14. **Settings density.** Settings is already the densest screen and now has a Keyboard card and
    text scaling. The Seerr card must be searchable, compact and included in the tall capture;
    otherwise it adds a second screen of scroll to a page that already needs it.
15. **Notification scope.** `notify-send` is claimed for live reminders only. Do not quietly
    widen that claim; SD11 decides whether Seerr may use it, and the limits table must change in
    the same commit.
16. **Audit and counts.** Captures and test counts are stated in `features`, `HANDOFF`,
    `accessibility` and the snapshots README (`README` does not print a count). S0–S3 moved them
    to **114 captures, 287 app tests and 22 Seerr API tests**; every later scene and test changes
    them again. Update them in the same commit that adds the scenes.
17. **The request list is an N+1.** `GET /request` has no title, poster or year; each row needs a
    `/movie/{tmdbId}` or `/tv/{tmdbId}` summary. Fetch only the rows in view, cache by
    `(mediaType, tmdbId)`, and never refetch summaries on the 5 s poll. A 50-row first paint is
    50 extra calls if this is ignored.

---

## 6. What is deliberately not here

- Direct Radarr/Sonarr clients, API keys, queue management or deletion.
- Seerr administration: users, permissions, settings, logs, jobs, overrides.
- Issues and comments.
- 4K requests (`is4k`, `status4k`, `downloadStatus4k`) — the types can read them; the UI does not.
- Watchlist sync and Seerr "watchlist" rows.
- Notifications while the page is closed, or a `notify-send` path in v1.
- A second Jellyfin server, a merged Seerr/Jellyfin catalogue, or multi-server search.
- A web view, an embedded Seerr UI, or a second window.

---

## 7. Decisions I need from you

Recommendation is the default. "All recommended" builds to this.

| # | Question | Recommendation | What changes if you disagree |
|---|---|---|---|
| SD1 | How does Runefin authenticate to Seerr? | **Paste the instance API key in Settings** (`X-Api-Key`), stored `0600`; optional `X-API-User` for attribution. | Cookie sign-in via `/auth/jellyfin` (needs the username/password, session expiry) or `/auth/local` (email). More correct per-user, more moving parts. |
| SD2 | Where do requests live? | **One Discover page with Discover/Requests tabs**, sidebar entry only when configured, active count badge. | Two sidebar entries (Discover + Requests), or requests only from a detail page. |
| SD3 | What can be requested in v1? | **Films and series**, series with a season picker (All / chosen seasons). | Films only first; series later. |
| SD4 | 4K requests? | **Out of scope v1.** Read the fields; do not offer the toggle. | A 4K toggle doubles the status surface and the permission cases. |
| SD5 | Refresh model | **Poll 5 s while the page is visible**, one in flight, stop on leave; manual Refresh. | 15–30 s background polling plus a toast when something becomes available (more traffic, more noise). |
| SD6 | Posters | **Fetch TMDB images directly** through the existing image worker and disk cache. | Text-only cards, or add a proxy (Seerr does not provide one). |
| SD7 | Whose requests? | **All requests the key can see**, requester labelled when there is more than one; no per-user filter in v1. | `requestedBy=<seerr user id>` needs the id in Settings. |
| SD8 | Library detail integration | **Phase 3:** "Request missing seasons" on a Jellyfin series, "Open in Jellyfin" on an available request. | Keep Seerr in its own page forever. |
| SD9 | Scope guard | **No Radarr/Sonarr, no Seerr admin, no issues, no deletion, no auto-approve, no notifications when closed.** | Each is a separate project. |
| SD10 | Product framing | **One Jellyfin media server and one window; Seerr is an optional companion service.** No second library, no catalogue merge, no multi-server search. | Merging Seerr and Jellyfin into one catalogue is a different product and a much larger change. |
| SD11 | Notifications | **In-app toast only while the page is open; no `notify-send` in v1.** | Reuse the existing notification opt-in; this changes the `features.md` limits claim and needs a quiet-hours policy. |

---

## 8. Recommendation and route

Build it in four small phases plus a read-only capture step, each with a snapshot, a test and a
commit (the detailed plan is in
[`../plans/2026-11-05-seerr-requests-plan.md`](../plans/2026-11-05-seerr-requests-plan.md)):

0. **S0.0 — live capture.** Read `/status`, `/auth/me`, a Discover page and a request page from the
   real instance; save the bodies as fixtures. No code. This is where the schema lies are caught.
1. **S0 — crate, config, probe.** `crates/seerr-api`, the config fields, a Settings card with
   URL + key + Test, and the first fixtures. No new page yet.
2. **S1 — Requests page.** Read-only list with request/media statuses, byte-accurate progress,
   polling only while visible, and every state.
3. **S2 — Discover and request.** Search, three bounded rows, the Seerr detail/request panel,
   movie + TV season requests, duplicate/quota feedback.
4. **S3 — one product.** Sidebar count, "Open in Jellyfin", "Request missing seasons" on a
   Jellyfin series, a quiet availability toast while the page is open.

The docs rewrite adds no phase; it changes S0's Settings work (search, text scale, tall capture)
and S1's audit (text + bar, 130 %, focus). S0 and S1 are the proof: if the key, the probe and the
progress rows work against a live Seerr, the rest is surface. If they do not, the app is unchanged
and nothing was risked.

---

## 9. Evidence and tests

- **`cargo test -p seerr-api`** — captured fixtures for: `/status`, `/auth/me`, a search union, a
  Discover page, a TV detail with seasons, a request page with a movie downloading, a series with
  two episode downloads, a pending request, an available request, a 409 body, a 403 body. Pin the
  numeric status enums and `pageInfo.results` (total, not page length).
- **`cargo test -p runefin`** — drive `State` with `Latest`/`Msg` fixtures: progress arithmetic
  (including `size == 0`), the aggregate for a series **deduped by `downloadId`**, a completed
  season pack says `Downloaded`, the PENDING state has no bar, unconfigured hides the nav entry, an
  error keeps the last answer, a poll only fires while visible, the Seerr page is addressable at
  130 % text.
- **`scripts/check.sh`** — add `cargo test -p seerr-api` beside the two existing suites. The
  current script runs `cargo fmt`, `cargo clippy --workspace`, `jellyfin-api`, `runefin` and a
  release build; a workspace member without its own test line is a silent hole.
- **S0.0 live capture, done 2026-11-05** — Jellyseerr 3.5.1 at `jellyseerr.example`; 458
  requests, 144 processing, 299 available; admin key; `syncEnabled: false` on Radarr and Sonarr;
  no `downloadStatus` rows; request rows have no title or poster. See §2.8. Fixtures must be
  redacted before committing.
- **Progress bars** — no live row exists while sync is off, so the bar fixture is synthetic and
  follows the source `DownloadingItem`. The live test asserts statuses and the indeterminate
  state, not a percentage. Enabling sync in Seerr makes the same code show real bars.
- **Ignored live test** — `SEERR_URL` + `SEERR_API_KEY`, read-only: `/status`, `/auth/me`, one
  Discover page, one request page, and one summary call per media type. A write test is opt-in
  (`SEERR_ALLOW_WRITE=1`) and deletes the request it creates. This is a separate test target, not
  a fourth `JELLYFIN_*` variable.
- **Snapshots** — `discover`, `discover-search`, `requests`, `requests-empty`, `requests-error`,
  `requests-unconfigured`, `seerr-detail-tv`, `settings-seerr`; narrow variants for the request
  panel and the progress rows. S0–S3 moved the committed total to **106 scenes / 114 captures**,
  so the counts in `docs/snapshots/README.md`, `HANDOFF.md` and `accessibility.md` move.
- **Accessibility** — the page is part of the Phase 0–2 audit: every control has a stable `.key`,
  `focusable(true)` and the 2 px ring; the progress value is text + bar; the 130 % render and a
  keyboard-only pass are required before the "audited" claim.
- **No normal test hits the network**, and no test needs a Seerr instance. The person and cast
  probes are opt-in live tests (`a_live_person_has_credits_and_a_film_has_a_cast`).

---

## 10. References

- Seerr repository and OpenAPI: <https://github.com/seerr-team/seerr> (`seerr-api.yml`, `develop`,
  read 2026-11-05); live API docs at `<seerr>/api-docs`.
- Merge announcement: <https://docs.seerr.dev/blog/seerr-release/> (2026-02-10).
- Status enums and download rows: `server/constants/media.ts`, `server/entity/Media.ts`,
  `server/lib/downloadtracker.ts`.
- Auth: `server/middleware/auth.ts` (`X-API-Key`, `X-API-User`, session cookie),
  `server/routes/auth.ts`.
- Original API mirror: <https://api-docs.overseerr.dev/>.
- The docs baseline this brief was reconciled against: commit `d5b6bea`
  ("docs: bring the inventory and contracts up to date", 2026-11-05), which updated
  `docs/design.md`, `docs/features.md`, `docs/HANDOFF.md`, `docs/accessibility.md`,
  `docs/conventions.md` and `docs/snapshots/README.md`. The baseline at that commit was 42 API
  tests, 269 app tests (21 ignored), 94 scenes / 101 captures; S0–S3 add 22 Seerr API tests, 18 app
  tests and 11 scenes / 12 captures (106 / 114).
- Live instance, read-only capture 2026-11-05: `https://jellyseerr.example`, `/status`
  `3.5.1`, `applicationTitle` "Jellyseerr", `mediaServerType` 2 (Jellyfin), 458 requests, 144
  processing, 299 available, Radarr/Sonarr sync off, no download rows. See §2.8.
