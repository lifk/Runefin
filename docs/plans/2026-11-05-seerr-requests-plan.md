# Seerr companion — implementation plan

**Status:** **implemented 2026-11-05** (S0–S3). The recommended decisions were accepted. The only
unshipped item is S2's optional `DELETE`/Undo and a full keyboard-only accessibility pass; S4
remains out of scope.

**Baseline:** commit `d5b6bea` (2026-11-05). The app runs against Jellyfin **12.4.0**. After
S0–S3: **22 Seerr API tests, 42 Jellyfin API tests, 287 app tests** (21 ignored) and **106 scenes /
114 captures**. The Seerr contract was verified against `seerr-team/seerr` `develop` on
2026-11-05; the published OpenAPI is stale.

**Live instance, S0.0 captured 2026-11-05:** `https://jellyseerr.example`, Jellyseerr
**3.5.1** (`applicationTitle`), API v1, Jellyfin-backed (`mediaServerType: 2`). Measured: 458
requests (144 processing, 299 available, 0 pending), an admin API key, and **no title/poster on
request rows**. Radarr/Sonarr sync was off at capture; S0.0 **enabled it**, and the first live
`downloadStatus` rows are four completed episodes of one season pack (same `downloadId`, same
43 GB). The full measurement is in the discovery §2.8; the consequences are the summary N+1 and
the unique-download aggregate below.

**Discovery:** [`../research/08-seerr-client.md`](../research/08-seerr-client.md). This plan does
not re-argue the contract; it turns it into work.

**Principle:** Seerr is an optional companion service. One Jellyfin media server, one window. With
`seerr_url` empty the app is exactly today's app: no nav entry, no polling, no changed snapshot, no
dependency on a running Seerr. With it configured, the Jellyfin path is still unchanged.

**Decisions:** SD1–SD11 were accepted as recommended: API key, one Discover page with
Discover/Requests tabs, films + seasons, 5 s visible-only polling, TMDB posters, all requests the
key can see, the Jellyfin detail integration, no 4K/admin/issues/deletion, one Jellyfin media
server plus an optional companion, and no `notify-send` in v1.

---

## 0. Outcome and non-goals

### 0.1 The user-visible outcome

A signed-in viewer can:

1. Open **Discover** from the sidebar (only when Seerr is configured) and see trending films,
   trending series, and a search field over Seerr's TMDB catalogue.
2. Open a Seerr title and read its poster, backdrop, year, overview, score and availability state.
3. Request a film, or choose seasons of a series (All / individual seasons), and get the server's
   own answer for approval, quota, permission and duplicate cases.
4. Open **Requests** (Active by default: pending + processing + partial) and see each request's
   request status and media status, plus byte-accurate download progress **when Seerr's queue sync
   is on**. On the live instance both sync flags are off, so the honest row is
   `Processing · Waiting for a download`; the same code shows bars when sync is enabled.
5. Open an **Available** request in Jellyfin, and request missing seasons from a Jellyfin series
   detail page (S3).

And, when Seerr is absent, misconfigured or down:

6. The rest of the app behaves exactly as it does today. No new nav item, no background error
   banner, no blocked Home, no failed playback.

### 0.2 Non-goals

- A local download queue or offline files (Q9). Seerr's "download progress" is the Radarr/Sonarr
  queue as Seerr reports it, not files stored by Runefin.
- Direct Radarr/Sonarr clients, queue management or deletion.
- Seerr administration, users, permissions, settings, logs, jobs, override rules.
- Issues and comments.
- 4K requests (`is4k`, `status4k`, `downloadStatus4k`).
- Watchlist sync, Seerr's watchlist rows, Trakt.
- Notifications while the page is closed; `notify-send` (SD11).
- A second Jellyfin server, a merged Jellyfin/Seerr catalogue, or multi-server search.
- A web view, an embedded Seerr UI, or a second window.

---

## 1. S0.0 — live capture (done 2026-11-05)

S0.0 ran read-only against `https://jellyseerr.example` with an admin key. It exists because
§2.7 of the discovery is real, and it found two things that change the plan:

1. **A request row has no title or poster.** `GET /request` gives the status, media status and
   `downloadStatus`, but the media row only has `tmdbId`/`mediaType`. Titles, posters, backdrops
   and years come from `/movie/{tmdbId}` or `/tv/{tmdbId}`. The Requests page is therefore a
   bounded N+1: fetch summaries only for the visible rows and cache them by `(mediaType, tmdbId)`.
2. **Sync was off and is now on.** `downloadStatus` was empty on all 144 processing requests
   because Radarr and Sonarr both had `syncEnabled: false`. S0.0 enabled both through the settings
   API (the PUT body must omit the read-only `id`) and ran the `download-sync` job. The tracker
   now returns four **completed** episode rows for one season pack; there is no in-progress row,
   so the mid-progress bar stays fixture-tested. The season pack repeats the same `downloadId` and
   the same 43 GB size on every episode, so aggregate by unique download, not by row
   (otherwise 172 GB instead of 43 GB).

**Measured:** Jellyseerr 3.5.1, `mediaServerType: 2` (Jellyfin); 458 requests (295 movie / 163 tv),
0 pending, 443 approved, 144 processing, 299 available, 14 completed; 50 recent rows returned
statuses 2/5 and media statuses 3/4/5/7; `canRemove: true`; `partialRequestsEnabled: true`;
`enableSpecialEpisodes: false`; `movie4kEnabled`/`series4kEnabled: false`; user `displayName`
`"example-user"`, `username: null`, permissions 2. Full measurement: discovery §2.8.

**Calls (read-only; reproduced for the next capture):**

```sh
curl -s "$SEERR/api/v1/status?checkUpdateAvailable=false"
curl -s -H "X-Api-Key: $KEY" "$SEERR/api/v1/auth/me"
curl -s -H "X-Api-Key: $KEY" "$SEERR/api/v1/discover/trending?mediaType=all&timeWindow=week"
curl -s -H "X-Api-Key: $KEY" "$SEERR/api/v1/search?query=dune&page=1"
curl -s -H "X-Api-Key: $KEY" "$SEERR/api/v1/request?take=50&skip=0&filter=all&sort=modified&sortDirection=desc"
curl -s -H "X-Api-Key: $KEY" "$SEERR/api/v1/request/count"
```

**Fixtures (S0's first task):** `crates/seerr-api/tests/fixtures/` with `status.json`,
`auth-me.json`, `discover.json`, `search.json`, `requests.json` (live-shaped, no download rows),
`requests-downloading.json` (synthetic, for the bars), `requests-pending.json`,
`requests-available.json`, `request-count.json`, `tv-detail.json`, `movie-detail.json`. The raw
responses stay in `/tmp`; the committed files are redacted (no key, email, `serviceUrl`,
`mediaUrl`, `jellyfinMediaId`, `jellyfinUserId`, avatar or hostname) and slimmed to the fields the
app reads plus a few unknown ones. Mark the synthetic download fixture in its first key or a
sibling `.md` note so nobody mistakes it for live evidence.

**Done when:** done for the capture; the fixture files land with S0 and `git grep` finds no secret
or host.

---

## 2. Architecture

### 2.1 New crate `crates/seerr-api`

```
crates/seerr-api/
  Cargo.toml          serde, serde_json, ureq 3.5; [lints] workspace = true
  src/lib.rs          pub mod client; pub mod models;
  src/client.rs       blocking Client, Error, normalise_base_url, fetch_image
  src/models.rs       wire types and the two status enums
  src/models/tests.rs captured-body parsing
  src/client/tests.rs URL building, error mapping
  tests/live.rs       ignored; SEERR_URL / SEERR_API_KEY
```

**Client surface:**

```rust
pub struct Client { /* base, api_key, user_id, agent */ }

impl Client {
    pub fn new(base_url: &str, api_key: impl Into<String>) -> Self;
    pub fn with_user(mut self, user_id: i64) -> Self;       // X-API-User
    pub fn status(&self) -> Result<Status>;
    pub fn public_settings(&self) -> Result<PublicSettings>;
    pub fn auth_me(&self) -> Result<User>;
    pub fn search(&self, query: &str, page: u32) -> Result<SearchResults>;
    pub fn discover_trending(&self, page: u32) -> Result<DiscoverPage>;
    pub fn discover_movies(&self, page: u32) -> Result<DiscoverPage>;
    pub fn discover_tv(&self, page: u32) -> Result<DiscoverPage>;
    pub fn movie(&self, tmdb_id: u64) -> Result<MovieDetails>;
    pub fn tv(&self, tmdb_id: u64) -> Result<TvDetails>;
    pub fn requests(&self, page: RequestPage) -> Result<RequestsPage>;
    pub fn request_count(&self) -> Result<RequestCounts>;
    pub fn create_request(&self, request: CreateRequest) -> Result<MediaRequest>;
    pub fn delete_request(&self, request_id: u64) -> Result<()>;
}

pub fn fetch_image(url: &str) -> Result<Vec<u8>, String>;
```

- One `ureq::Agent` per `Client`, shared by clones; `User-Agent: Runefin/<version>`,
  `Accept: application/json`, `X-Api-Key`, optional `X-API-User`.
- `normalise_base_url`: trim whitespace and trailing `/`, add `http://` when no scheme. Do not
  strip a path: a Seerr behind a reverse proxy may live under `/seerr`.
- `Error { Status { code, message }, Transport(String), Decode(String) }` with:
  - `user_message()`: 401 "Seerr needs an API key."; 403 the server's message (the Settings probe
    overrides this to "Seerr rejected the API key." when `/auth/me` is 403); 404 "Seerr does not
    know that request."; 500 on `/movie`/`/tv` "Seerr could not load that title."; 409 "Already
    requested."; 429 "Seerr is busy; try again shortly."; other 5xx "Seerr had a problem.";
    transport "Can't reach Seerr."; decode "Seerr sent something unexpected."
  - `is_transient()`: transport and 5xx only. A 500 from a bad TMDB id is retried once but the
    detail screen keeps the page and shows the message; it never claims Seerr is down.
  - `is_auth()`: 401, plus 403 from `/auth/me` (the probe's call site knows which endpoint it
    called).
- The error reader takes `message`, then `error`, then the status code. The live probes pin the
  three shapes: wrong key → 403 `{status, error}`; no key → 401 `{message, errors}`; unknown
  title → 500 `{message}`.
- `fetch_image` is a plain `ureq::get` with no auth, for TMDB's CDN.

**Models:** every field the UI does not strictly need is `Option`/`#[serde(default)]`; unknown
fields are ignored; unknown enum numbers become `Unknown` rather than failing the page.
`MediaStatus` and `MediaRequestStatus` pin the numeric values from §2.5 of the discovery. `PageInfo`
carries `page`, `pages`, `results` (total), `pageSize`; a `Paginated<T>` has `pageInfo` and
`results`. A `DiscoverItem` merges `MovieResult`/`TvResult`: `id`, `media_type`, `title` or `name`,
`release_date` or `first_air_date`, `poster_path`, `backdrop_path`, `overview`, `vote_average`,
`media_info`. `DetailData` is an enum over `MovieDetails`/`TvDetails`. `TvDetails.seasons` is a
`Vec<Season>` with `season_number`, `name`, `episode_count`, `air_date`. `RequestPage` is the
query value (`take`, `skip`, `filter`, `media_type`); the wire response is
`Paginated<MediaRequest>`, whose `page_info.results` is the total count, not the page length. The
app stores the merged rows as a `Vec<MediaRequest>` so there is no second `results` to confuse.
`User` prefers `display_name` then
`jellyfin_username` then `email`; `username` is often null. `MediaRequest` has **no title,
poster or year** — do not derive one from `service_url`. `MediaSummary` is the slim
`/movie/{id}` or `/tv/{id}` view the Requests list needs: `title` or `name`, `year`,
`poster_path`, `backdrop_path`, `overview`, `runtime` or `number_of_seasons`, and `seasons` for
TV. Models that travel in `Msg` derive `Debug, Clone, PartialEq, Eq`; the rest only need
`Deserialize`.

### 2.2 Configuration

Add to `Config` (after `omdb_api_key`, before the subtitle block):

```rust
#[serde(default)] pub seerr_url: String,
#[serde(default)] pub seerr_api_key: Option<String>,
#[serde(default)] pub seerr_user_id: Option<i64>,
#[serde(default)] pub seerr_user_name: Option<String>,
```

- `Config::sign_out` keeps them: a Jellyfin sign-out is a different session. The Settings **Clear**
  action removes the key; replacing it zeroizes the old value.
- The key is written with the existing 0600, atomic `save()`.
- Tests: `seerr_config_round_trips`, `an_older_file_without_seerr_fields_still_loads`,
  `a_jellyfin_sign_out_keeps_the_seerr_key`.
- `State::save_preferences` writes the three fields from the Settings buffers (on close, not on
  every keystroke).
- `State::from_config` initializes the buffers and `SeerrState`.

### 2.3 State, worker and polling

**`SeerrState` (new `state/seerr_state.rs`):**

```rust
pub enum SeerrTab { Discover, Requests }
pub enum RequestsFilter { Active, All }

pub struct SeerrState {
    pub tab: SeerrTab,
    pub filter: RequestsFilter,   // default Active
    pub requests: Vec<MediaRequest>,
    pub counts: Option<RequestCounts>,
    pub loading: bool,
    pub error: Option<String>,
    pub last_loaded: Option<Instant>,
    pub in_flight: bool,
    pub discover: DiscoverPage,
    pub discover_loading: bool,
    pub discover_error: Option<String>,
    pub query: TextEdit,
    pub search: SearchResults,
    pub searching: bool,
    pub search_error: Option<String>,
    pub detail: Option<SeerrDetailState>,
    /// Title/poster/year for the request rows; keyed by `(media_type, tmdb_id)` and
    /// fetched lazily, because `GET /request` has none of it.
    pub summaries: HashMap<(String, u64), MediaSummary>,
    pub summaries_loading: bool,
    pub summaries_error: Option<String>,
}

pub struct SeerrDetailState {
    pub media_type: String,        // "movie" | "tv"
    pub tmdb_id: u64,
    pub details: Option<DetailData>,
    pub selected_seasons: HashSet<i64>,
    pub loading: bool,
    pub error: Option<String>,
    pub requesting: bool,
}
```

**`State` fields:**

```rust
pub seerr: SeerrState,
seerr_probe: Latest<Result<seerr_api::Status, String>>,
seerr_requests: Latest<Result<Vec<seerr_api::MediaRequest>, String>>,
seerr_counts: Latest<Result<seerr_api::RequestCounts, String>>,
seerr_discover: Latest<Result<seerr_api::DiscoverPage, String>>,
seerr_search: Latest<Result<seerr_api::SearchResults, String>>,
seerr_detail_load: Latest<Result<seerr_api::DetailData, String>>,
seerr_summaries: Latest<Result<Vec<((String, u64), MediaSummary)>, String>>,
seerr_return: Route,
```

`State` has no `Default`, so every literal must be completed when the fields are added:
`State::from_config`, the `#[cfg(test)] fn state()` helper in `state.rs`, and any fixture
constructor in `main.rs` that builds a `State` literally. The `setup` waker list grows by the seven
new `Latest` fields.

- **Reads** (`status`, requests, counts, discover, search, detail) go through `Latest`. `Search`
  requests use the same 150 ms debounce and two-character minimum as `search_state.rs`; a new
  keystroke cancels the previous call.
- **The Requests filter defaults to Active**: fetch `filter=pending` and `filter=processing` and
  merge (two calls), or `filter=all` for the All chip. The merged list keeps the server's
  `modified` order; the active set is 144 on the live instance, so the first `take=50` is the
  actionable page. The tab/sidebar badge counts pending + processing.
- **Request summaries** are the N+1 the live capture exposed: after a `/request` page lands, fetch
  `/movie/{id}` or `/tv/{id}` for the rows in view whose `(media_type, tmdb_id)` is missing from
  `summaries`. One `Latest` closure uses `std::thread::scope` with at most four concurrent
  requests and returns the batch; the cache lives for the session. Do **not** refetch summaries on
  the 5 s poll. A failed summary leaves the row as `TMDB <id>` + status and retries on demand.
- **Writes** (`create_request`, optional `delete_request`) go through `Worker::spawn` with
  `Msg::SeerrRequested(Result<MediaRequest, String>)` and
  `Msg::SeerrDeleted(Result<(), String>)`, like `Msg::Favorite`.
- `setup` adds `set_waker` for every `Latest` field and, when `seerr_url` + key are present and the
  user is signed in, requests the count once (no timer) so the sidebar badge can appear.
- `poll` takes each `Latest` and sets `changed = true` only when an answer lands; `poll_seerr`
  starts a poll only when **all** of these hold:
  - `is_signed_in()`, `seerr_url` and key are non-empty;
  - `route == Route::Seerr` and `seerr.tab == SeerrTab::Requests`;
  - `!seerr.in_flight`;
  - `last_loaded` is `None` or at least 5 s old.
  The timer itself returns `false`; the answer sets `in_flight = false`, updates `last_loaded`,
  and returns `true`.
- Leaving the route, opening the player, signing out, or clearing the key cancels the in-flight
  request (`Latest::cancel()`), sets `in_flight = false`, and stops the timer.
- The request and count are fetched beside each other in one worker function so a row's badge and
  the tab count never disagree.
- A late detail answer is applied only when its `(media_type, tmdb_id)` still matches the open
  `Route::SeerrDetail`; keep `seerr_detail_for: Option<(String, u64)>` and discard the rest, the
  same way `detail_similar_for` guards the Jellyfin detail page.
- A 5 s poll is at most three HTTP calls (pending + processing + count when Active; requests +
  count when All). No poll from Home, Library, Detail, Search or the player.

**`Msg` additions** in `net.rs`:

```rust
SeerrRequested(Result<seerr_api::MediaRequest, String>),
SeerrDeleted(Result<(), String>),
```

`apply` handles both: on success, replace/refresh the matching request, set `in_flight = false`,
refresh the requests page and counts, and show a toast. On 409, show "Already requested" and
refresh. On 403, show the server's message. On failure, keep the previous rows.

### 2.4 Routes, screens, sidebar and keys

**`Route` additions:**

```rust
Seerr,
SeerrDetail { media_type: String, tmdb_id: u64 },
```

- `screens.rs::view`: `Route::Seerr => seerr::seerr_screen(s)`;
  `Route::SeerrDetail { .. }` resolves `s.seerr.detail`, falling back to the Seerr page.
- `on_key`: `Esc` on `Route::Seerr` goes home (add it to the existing
  `Route::Remote | Route::LiveTv | Route::Settings` arm); `Esc` on `Route::SeerrDetail` calls
  `back_from_seerr_detail()`. `Ctrl+F` on `Route::Seerr` focuses the Seerr search instead of
  opening Jellyfin search; `Ctrl+K` stays the global Jellyfin search. No new global key.
- `remember_place`: do **not** persist Seerr. A restart returns Home, so a crash-safe restore never
  depends on a second service. `Route::Seerr` falls into the existing `_ => home` arm.
- `State::sign_out`: cancel the seven `Latest` loads and reset `seerr` to default, but keep the
  config URL/key.

**Screens:**

- `screens/seerr.rs` — one `seerr_screen(s)` with the shared shell (`sidebar`, separator,
  scrollable content). Header: back, **Discover**, segmented tabs, **Active/All** chip and
  **Refresh** on the Requests tab.
- `screens/seerr_detail.rs` — `seerr_detail_screen(s, detail)`: backdrop, poster, title, year,
  runtime/seasons, overview, genres, score, availability banner, request action/season list.
- Reuse `theme::CARD_W`, `CARD_H`, `CARD_GAP`, `theme::GUTTER`, `type_::*()`, `theme::colors()`.
  No new card geometry, no new motion values.
- Rows use the table in §3 and the progress helper in §3.2. Paint the status row first from
  `/request`; fill the title, poster and year when the summary lands. Before that, the row says
  `TMDB <id>` with the status — never a blank card and never a fabricated title from
  `service_url`.

**Proposed keys** (stable; the snapshot tests address them):

```
seerr-screen, seerr-back, seerr-title
seerr-tab-discover, seerr-tab-requests, seerr-refresh
seerr-filter-active, seerr-filter-all
seerr-search, seerr-clear-search
seerr-row-trending, seerr-row-movies, seerr-row-tv
seerr-card-<movie|tv>-<tmdb-id>
seerr-detail, seerr-detail-back, seerr-detail-request
seerr-season-<n>, seerr-season-all
seerr-request-<id>, seerr-poster-<id>, seerr-progress-<id>
seerr-episodes-<id>, seerr-open-<id>
seerr-empty, seerr-error, seerr-retry, seerr-unconfigured, seerr-setup
nav-seerr, nav-seerr-badge
```

**Sidebar (`screens.rs`):** after the Playlists block, before the `sidebar-nav-scroll` child:

```rust
if !s.config.seerr_url.is_empty() {
    nav = nav.child(
        nav_item("Discover", "nav-seerr", matches!(s.route, Route::Seerr | Route::SeerrDetail { .. }),
                 t, &theme::icon::COMPASS, collapsed)
            .on_press(|s: &mut State| s.open_seerr(SeerrTab::Discover)),
    );
}
```

A small `nav_item_badged` helper (or an optional parameter) appends a pill with the pending +
processing count when expanded; collapsed mode shows the icon only. Add
`theme::icon::COMPASS` (a simple 20×20 stroke SVG) in `theme.rs`; a new icon is a pixel change and
gets the normal snapshot review.

### 2.5 Images from TMDB

`net.rs` currently has `ImageRequest { client, key, url }`; `image.rs::load_or_fetch(client, url,
key)` takes a Jellyfin `Client`.

- Add `ImageSource { Jellyfin(Client), Public }` in `net.rs`.
- Keep `request_image(client, key, url, priority)` as the Jellyfin convenience wrapper; add
  `request_public_image(key, url, priority)`.
- `image::load_or_fetch(source, url, key)` dispatches: Jellyfin → `client.fetch_bytes`, Public →
  `seerr_api::fetch_image`.
- Cache key: `image::key(&s.config.seerr_url, &format!("tmdb-{id}"), "Poster", Some(poster_path),
  342)`; backdrop uses `"Backdrop"`, `backdrop_path`, `780`. The `poster_path` is the tag, so new
  artwork is a new file; the Seerr URL is the "server" part, so two instances do not collide.
- TMDB URLs: `https://image.tmdb.org/t/p/w342{poster_path}` and `w780{backdrop_path}`. No key.
- A failed TMDB fetch is an image failure only; the card's initial-letter fallback remains.

### 2.6 Settings

A new card between **Metadata** and **Storage** in `screens/settings.rs`:

```
Requests (Seerr)                    [Connected · Seerr 2.x · luis]
Seerr server   [http://198.51.100.2:5055]        [Test]
API key        [••••••••••••••••]            [Clear]
User id        [1] (optional; attributes requests to a Seerr user)
                                             [Open Discover]
```

- State buffers: `seerr_url: TextEdit`, `seerr_key: TextEdit` (`.secure()`),
  `seerr_user: TextEdit` (numeric). They live in `SeerrState` so the screen stays a pure read.
- **Test** builds a client from the buffers (not the saved config), calls `/status` then `/auth/me`
  through a `Latest`, and renders `Connected · Seerr <version> · <username>` or the error line.
  It never blocks the UI and never changes the saved config.
- **Clear** empties the key buffer and `config.seerr_api_key` (zeroize), cancels the Seerr loads,
  resets the page state, and leaves the URL.
- **Open Discover** opens `Route::Seerr` even when the URL/key are empty, so the setup card is
  reachable while the sidebar entry stays hidden. The scene for the unconfigured state uses the
  same route.
- The settings search `shown` filter must include "Requests (Seerr)", "Seerr server", "API key",
  "User id". A query for "seerr" shows the card and hides unrelated rows.
- `save_preferences` writes the buffers on close, like the OMDb key.
- Snapshot `settings-seerr`: set `settings_query = "Seerr"` so the filtered card is at the top,
  then render 1280×900. Keep `settings-keyboard` as it is.
- Copy: "The key is stored in this file with mode 0600 and is admin-level. Use HTTPS when the host
  is not on a trusted LAN. Seerr is not required." The live instance calls itself **Jellyseerr**;
  the card title says "Requests (Seerr / Jellyseerr)" so both names are findable.

### 2.7 Accessibility and text size

- Use `type_::body()`, `type_::page()`, `type_::label()`, `type_::caption()`; never the raw
  `TextStyle` constants, or the 85–130 % preference will not apply.
- Every interactive element: `.key(..)`, `focusable(true)`, `focus_ring(2.0, 2.0, theme::FOCUS)`,
  pointer cursor, hover/press wash.
- Progress is `42% · 3.1 of 7.4 GB · 12 min left` plus the bar. Status chips carry text; colour is
  a second signal, never the only one.
- Test the page at 130 % (`set_text_scale(1.3)`) at 800×600; the request rows wrap, the header
  wraps, and no text clips.
- Add the page to `docs/accessibility.md`'s audit list and its "not yet audited" note if the
  keyboard pass is incomplete.

---

## 3. Data model and UI state machine

### 3.1 Request/media → row state

| Request status | Media status | Row | Action |
|---|---|---|---|
| PENDING | PENDING | **Awaiting approval** | none; no bar |
| APPROVED | PROCESSING | **Downloading** if `downloadStatus` else **Waiting for a download** | none; bar/indeterminate |
| APPROVED | PROCESSING/PARTIAL | every unique download is `completed` | **Downloaded · waiting for the library**; no live bar |
| APPROVED | PARTIALLY_AVAILABLE | **Partially available** + `n of m seasons` | **Request missing seasons** only in S3 |
| APPROVED | AVAILABLE | **Available** | **Open in Jellyfin** |
| DECLINED | any | **Declined** | none |
| FAILED | any | **Failed** | none; the copy names Radarr/Sonarr |
| COMPLETED | AVAILABLE | **Completed** | **Open in Jellyfin** |
| any | BLOCKLISTED | **Not requestable** | none; the detail explains |
| any | DELETED | **Removed from the library** | may request again if the server allows |

Unknown enum values render as `Unknown` and never crash the page.

### 3.2 Progress arithmetic

Pure helpers on `DownloadingItem`. **Group by `downloadId` first**: a season pack repeats the same
id and the same `size` on every episode row, and summing rows overcounts (measured live: four
43 GB rows for one 43 GB pack).

```rust
pub fn aggregate(items: &[DownloadingItem]) -> (u64, u64, Option<f64>) {
    let mut seen = HashSet::new();
    let unique: Vec<&DownloadingItem> = items
        .iter()
        .filter(|i| i.download_id.is_empty() || seen.insert(i.download_id.as_str()))
        .collect();
    let total: u64 = unique.iter().map(|i| i.size).sum();
    let left: u64 = unique.iter().map(|i| i.size_left).sum();
    let percent = (total > 0 && left <= total).then(|| (total - left) as f64 / total as f64);
    (total, left, percent)
}

pub fn all_completed(items: &[DownloadingItem]) -> bool {
    // same unique-by-download_id pass; true when every unique row is "completed" or size_left == 0
}
```

- Aggregate by bytes per **unique download**, never by averaging percentages or summing episode
  rows.
- `size == 0` or `size_left > size` → indeterminate. Never print `NaN` or a negative percent.
- If every unique download is `completed` (`size_left == 0`) and the media is still
  `PROCESSING`/`PARTIALLY_AVAILABLE`, the row says **Downloaded · waiting for the library** and does
  not draw a live bar.
- Format bytes in binary units with one decimal (`3.1 GB`), the same in every row.
- `timeLeft` is rendered when it matches `^\d{2}:\d{2}:\d{2}$`; otherwise the ETA is used; if both
  are missing, omit the ETA rather than guessing.
- A download group is `Season 1 pack · 43 GB · completed` with episode labels beneath it; a film
  group omits the episode list. Do not repeat the pack size on every episode.

### 3.3 TV season selection

- `TvDetails.seasons` is the source; season 0 (specials) is visible but off by default; a season
  with `episode_count == 0` is disabled.
- "All seasons" sends `seasons: "all"`; otherwise `seasons: [1, 3, 5]` sorted ascending.
- A season already `AVAILABLE` in `mediaInfo` is marked "In library" and cannot be selected.
- `PARTIALLY_AVAILABLE` uses the server's `media.seasons` status when present; if not, only the
  overall state is claimed.
- The POST body always includes `mediaType`, `mediaId`; `is4k` is omitted.

---

## 4. Work breakdown

### S0.0 — live capture (done 2026-11-05)

- [x] Create `crates/seerr-api/tests/fixtures/` and capture the bodies listed in §1. The raw
  responses stayed in `/tmp` and were deleted; the committed fixtures are slim and redacted.
- [x] Redact names and hosts; verify no key is committed.
- [x] Record the Seerr version, `Sync Enabled` state, and the shape differences in the brief.
- [x] Enable `syncEnabled` on both integrations and run `download-sync`; capture the first
  completed season-pack rows.

**Done when:** `jq` parses every file, the measured shapes are written down, and
`git grep -i "$KEY"` finds nothing.

### S0 — crate, config, probe, Settings (done 2026-11-05)

- [x] Add `crates/seerr-api` to `Cargo.toml` members; create the crate with `ureq`/`serde` and
  workspace lints.
- [x] Write the redacted fixtures: the live-shaped set plus the clearly marked synthetic
  `requests-downloading.json`.
- [x] Implement `Client`, `Error`, `normalise_base_url`, `fetch_image`, and the models/enums.
- [x] Parse the fixtures; add the client URL/error tests (22 tests).
- [x] Add the four `Config` fields and the lifecycle tests.
- [x] Add `SeerrState`, the probe `Latest`, and the Settings buffers. The write messages and
  `Worker` methods land with S2, where they are first used.
- [x] Add the Settings card (URL, key, user id, Test, Clear), the search filter, and save-on-close.
- [x] Add `scripts/check.sh`'s `cargo test -p seerr-api` line.
- [x] Add the design.md dependency line (§6) and the §7.2 seam; `features.md` marks S0 only.

**Tests:** 22 crate tests, the config round-trip/sign-out test, `settings_search_finds_the_seerr_card`,
`a_403_on_auth_says_the_key_was_rejected`, `a_transport_error_says_seerr_is_unreachable`,
`clearing_the_key_empties_the_buffer_and_config`, `save_preferences_writes_the_seerr_fields_from_the_buffers`.

**Snapshot:** `settings-seerr` (1280×2600, the card in context); `settings-keyboard` re-rendered.

**Done when:** a live URL + key shows **Connected · Seerr 3.5.1 · <user>**; a wrong key says the
key was rejected; an unreachable URL says "Can't reach Seerr"; the key is not in any snapshot,
toast or log; with the key cleared the app is unchanged.

### S1 — Requests page, read-only (2–3 days)

- [x] `Route::Seerr`, `SeerrTab`, `SeerrState`, the seven `Latest` fields, `open_seerr`,
  `close_seerr`, `poll_seerr`, `apply_seerr_page`.
- [x] Request-summary loader: cache by `(media_type, tmdb_id)`, fetch only the rows in view, at
  most four concurrent calls via `std::thread::scope`, never on the 5 s poll.
- [x] `RequestsFilter::{Active, All}` (default Active) and the Active/All chip; Active merges
  `filter=pending` + `filter=processing`, All uses `filter=all`.
- [x] `screens/seerr.rs`: header, tabs (Requests active; Discover implemented), the
  Active/All chip, refresh, list, status chips, progress rows, episode disclosure,
  `Open in Jellyfin`. The row paints status first and fills title/poster/years from the summary;
  before it arrives, `TMDB <id>`.
- [x] Sidebar `nav-seerr` + badge helper + `COMPASS` icon; hidden when `seerr_url` is empty.
- [x] `on_key` Esc/`Ctrl+F`; `State::sign_out` reset; `remember_place` leaves Home.
- [x] Image plumbing: `ImageSource::Public`, `request_public_image`, `load_or_fetch` dispatch.
- [x] Poll only while visible, one in flight, 5 s; cancel on leave.
- [x] States: unconfigured, loading, empty, error, stale, permission.
- [x] Add `demo::seerr_requests()` fixtures and the scenes.

**Tests:** `seerr_progress_uses_bytes_not_percentages`,
`a_pending_request_has_no_progress_bar`, `an_unconfigured_seerr_hides_the_nav_entry`,
`a_seerr_error_keeps_the_last_answer`, `requests_poll_only_while_visible`,
`a_request_row_paints_before_its_summary`, `summaries_are_cached_and_not_refetched_on_poll`,
`a_summary_failure_leaves_the_tmdb_id`, `a_season_pack_is_counted_once_by_download_id`,
`a_completed_season_pack_says_downloaded`, `the_requests_page_defaults_to_active`,
`active_merges_pending_and_processing`, `the_seerr_page_addresses_every_control`,
`the_seerr_page_survives_130_percent_text`.

**Snapshots:** `requests`, `requests-live`, `requests-empty`, `requests-error`,
`requests-unconfigured`, `requests-narrow`. `requests` uses the synthetic download fixture;
`requests-live` uses the live-shaped no-download fixture and proves the "Waiting for a download"
state.

**Done when:** on the live instance an active request shows its title/poster from the summary; a
request with no sync rows says `Waiting for a download`; the completed season pack says
`Downloaded` and counts 43 GB once, not 172 GB; the synthetic mid-progress fixture shows a correct
byte percentage; a PENDING request shows approval, not 0 %; killing Seerr leaves Jellyfin browsing
and playback untouched.

### S2 — Discover and request creation (3–4 days)

- [x] `seerr_discover` (trending + movies + tv, one combined answer, 20 each), the search `Latest`
  with 150 ms debounce and two characters, and `Route::SeerrDetail`.
- [x] `screens/seerr.rs` Discover tab: search field, three rows, availability chips, progress chip,
  cards; `screens/seerr_detail.rs`: backdrop, poster, meta, overview, availability, request action,
  season picker, `Open in Jellyfin`.
- [x] `Worker::create_seerr_request` + `Msg::SeerrRequested`; duplicate/403/quota handling;
  refresh the requests page and the detail's `media_info` on success.
- [ ] Optional `DELETE /request/{id}` undo for 8 s when the server allows it. — not shipped in this pass.
- [x] `demo::seerr_discover()`, `demo::seerr_tv_detail()`, `demo::seerr_search()`.

**Tests:** `the_person_result_is_filtered`, `a_tv_request_sends_the_chosen_seasons`,
`a_duplicate_request_says_already_requested`, `a_quota_error_keeps_the_server_message`,
`an_available_season_cannot_be_requested`, `a_missing_title_500_keeps_the_page`,
`leaving_the_tab_cancels_the_search`.

**Snapshots:** `seerr`, `seerr-search`, `seerr-detail-tv`, `seerr-requested`.

**Done when:** a film and a chosen season appear in Seerr's own UI and in Requests; a duplicate is
friendly; a quota/permission failure is the server's message; no request is offered for something
already available.

### S3 — one product (2 days)

- [x] Sidebar count refreshed on open and after a write (no background timer).
- [x] `Open in Jellyfin` via `media_info.jellyfin_media_id`, with a toast fallback when the id is
  not on this server.
- [x] Jellyfin series detail: a quiet **Request missing seasons** action when `provider_ids["Tmdb"]`
  exists and the Jellyfin seasons are a strict subset of `TvDetails.seasons`; post only the
  difference.
- [x] A poll that sees `AVAILABLE` while the page is open shows one toast, not one per poll.
- [ ] Accessibility pass: 130 %, keyboard-only, focus rings, text + bar. — 130 % test shipped; full keyboard-only pass still open.
- [x] Docs and counts.

**Tests:** `a_missing_season_posts_only_the_difference`, `an_available_request_opens_the_detail`,
`an_available_transition_toasts_once`, `the_seerr_key_is_not_written_to_a_toast`.

**Snapshots:** `detail-seerr`, `requests-available`, `seerr-130`.

**Done when:** a series missing season 3 requests exactly season 3; an available request opens the
Jellyfin page; an app with no Seerr URL renders the same snapshots as before the project.

---

## 5. Test matrix

| Layer | Where | What |
|---|---|---|
| Wire parsing | `crates/seerr-api/src/models/tests.rs` | 10 captured bodies; enum values; `pageInfo.results` is total; unknown fields are ignored. |
| URL/error | `crates/seerr-api/src/client/tests.rs` | base normalisation (scheme, trailing slash, path); `X-Api-Key`/`X-API-User`; 401/403/409/429/5xx mapping; image URL. |
| Pure app logic | `crates/runefin/src/state/seerr_state.rs` tests | progress percent/aggregate, season selection, state derivation. |
| Summaries | `crates/runefin/src/state/seerr_state.rs` tests | cache key, visible-only batch, no refetch on poll, failure fallback. |
| UI/state | `crates/runefin/src/main.rs` tests | `Runtime` finds keys; nav visibility; error keeps data; poll visibility; 130 %; duplicate message; title after summary. |
| Snapshots | `scripts/snapshots.sh` | 13–14 scenes; `requests` uses the synthetic download fixture, `requests-live` uses the live-shaped no-download fixture. |
| Live read-only | `crates/seerr-api/tests/live.rs` | `/status`, `/auth/me`, discover, requests; ignored unless `SEERR_URL`/`SEERR_API_KEY`. |
| Live write | same, `SEERR_ALLOW_WRITE=1` | create a known request, read it, delete it; skipped by default. |

**No default test hits the network.** The `runefin` tests apply fixture values through
`State`/`Msg`, never a real client.

---

## 6. Snapshot matrix

| Scene | Size | Proves |
|---|---|---|
| `seerr` | 1280×800 | Discover: nav entry, three rows, cards, availability chips. |
| `seerr-search` | 1280×800 | Search results, person filtered, query visible. |
| `seerr-detail-tv` | 1280×800 | Season picker, All/individual, request action, backdrop. |
| `seerr-requested` | 1280×800 | Post-request state: toast, disabled action, availability chip. |
| `requests` | 1280×800 | Synthetic downloading film and a series with two episodes; bars and aggregate. |
| `requests-live` | 1280×800 | Live-shaped rows: processing with no download rows, "Waiting for a download". |
| `requests-empty` | 1280×800 | Empty copy and a Discover link. |
| `requests-error` | 1280×800 | Unreachable banner + Retry, last answer kept. |
| `requests-unconfigured` | 1280×800 | Setup card, no nav entry in the shell. |
| `requests-narrow` | 800×600 | Rows wrap at the smallest window. |
| `settings-seerr` | 1280×900 | Card + Test/Clear, filtered by the settings search. |
| `seerr-130` | 1280×800 | 130 % text; no clipping in header, rows or chips. |
| `detail-seerr` | 1280×800 | Jellyfin series detail with "Request missing seasons" (S3). |
| `requests-available` | 1280×800 | Available row + Open in Jellyfin (S3). |

After rendering, update:

- `docs/snapshots/README.md`: new rows and the `94 scenes / 101 captures` total.
- `docs/HANDOFF.md` §6 and §1 counts.
- `docs/accessibility.md`: the audit list and the 101-snapshot count.
- `README.md` if it states a capture count (it does not today; keep it that way).

---

## 7. Documentation and counts

| File | Change | When |
|---|---|---|
| `docs/design.md` | §1 boundary wording ("one Jellyfin media server + optional companion"). §6 architecture: `crates/seerr-api`, `screens/seerr.rs`. §7 heading back to "Designed-for extensions"; add §7.2 "Optional Seerr companion (proposed)". §8: M8 row. §10: second-service risk. §11: a Seerr question. §0: D9. §12 stays Jellyfin-only. | S0/S1 |
| `docs/features.md` | Insert **§13 Requests and discovery (Seerr)** before Limits; renumber Limits to §14; update the README anchor; add the limits rows (no 4K, no notifications when closed, no admin); add the config fields to §12. | S1–S3 |
| `README.md` | Status/feature list, optional requirement, live test env, "one Jellyfin media server + optional companion" boundary, anchor `#14-limits…`. | S1/S3 |
| `docs/HANDOFF.md` | §1 state-of-play row, §5 decision, §6 final state, counts, live test. | S1–S3 |
| `docs/accessibility.md` | Seerr in the audit list, 130 % and keyboard pass, snapshot count. | S3 |
| `docs/conventions.md` | The `SEERR_*` live test note beside the `JELLYFIN_*` one. | S0 |
| `docs/snapshots/README.md` | New scene rows and counts. | each phase |
| `docs/research/07…` | Leave historical; the Seerr brief is `08`, not a new 07 entry. | — |
| `docs/plans/2026-11-05-implementation-plan.md` | Add an optional Phase 5 cross-reference to this plan. | S0 |
| `scripts/check.sh` | `cargo test -p seerr-api`. | S0 |
| `scripts/snapshots.sh` | New scenes at the end; no reordering of existing renders. | each phase |

**Count rule:** every test or capture added changes the numbers in `features.md`, `HANDOFF.md`,
`accessibility.md` and `snapshots/README.md`. Update them in the same commit; a stale count is a
docs bug.

---

## 8. Commit sequence

1. **`test(seerr): capture the live contract`** — S0.0 fixtures, redacted, plus the brief's note.
   No app code. `cargo test` is unchanged.
2. **`feat(seerr): add the API crate and connection settings`** — S0. `scripts/check.sh` gains the
   crate; `settings-seerr` snapshot; design §6/§7.2.
3. **`feat(seerr): show requests and download progress`** — S1. Requests scenes; nav; image source;
   design §8 M8; features/README rows.
4. **`feat(seerr): discover and request titles`** — S2. Discover/search/detail scenes; request
   write path; duplicate/quota tests.
5. **`feat(seerr): open available requests and request missing seasons`** — S3. Jellyfin detail
   integration; badge; accessibility pass; final counts.

Each commit: `scripts/check.sh`, `scripts/snapshots.sh`, the PNG diffs, and a message that says
which decision it implements. Do not bump the Rune rev in any of them.

---

## 9. Risks and mitigations

| Risk | Mitigation |
|---|---|
| The OpenAPI is stale and types drop `downloadStatus`. | S0.0 captures live bodies; every unneeded field is optional; fixtures pin the shape. |
| The API key is admin-level. | 0600, zeroize, no logs/toasts, HTTPS note, explicit Clear; SD1's cookie path is the alternative. |
| Radarr/Sonarr sync is off, so there is no progress. | Confirmed live, then enabled: both flags were false, S0.0 turned them on, and the tracker returned completed season-pack rows. The row says "Waiting for a download" when there are no rows, "Downloaded" when every unique download is complete, and never 0 %. |
| Request rows have no title/poster, so the list is an N+1. | Fetch `/movie`/`/tv` summaries for visible rows only, cache by `(media_type, tmdb_id)`, four concurrent, never on the poll; paint status first. |
| TMDB is blocked on the LAN. | The image fails alone; the letter fallback keeps the page usable; no retry storm. |
| Polling adds load or piles up. | Visible-only, 5 s, one in flight, `Latest` cancellation, no background timer. |
| Settings is already dense. | Searchable card, compact rows, `settings-seerr` filtered capture; no new top-level nav unless configured. |
| Duplicate requests are noisy. | 409 is a normal outcome: "Already requested", refresh the row. |
| TV season arithmetic is wrong. | Post only the chosen/difference set; specials off by default; test the body, not just the UI. |
| A second service weakens the "one server" story. | SD10: one Jellyfin media server + one optional companion; no catalogue merge. |
| Counts and audits go stale. | The count rule in §7 and the same-commit checklist. |
| Scope creep into admin/4K/notifications. | SD4, SD9, SD11; the non-goals list; a PR that adds them is a different project. |

---

## 10. Definition of done

- With no `seerr_url`, `scripts/snapshots.sh` produces the same images as `d5b6bea`, the nav has no
  Discover entry, and no Seerr request is made.
- With a configured, reachable Seerr, Settings tests the key; Discover shows trending/films/series;
  a film and a chosen season can be requested; Requests shows each row's title/poster from the
  summary and its status, with byte-accurate progress when sync is on, `Downloaded` for a completed
  season pack counted once, and `Waiting for a download` when it is off; an available row opens the
  Jellyfin detail.
- A wrong key, an unreachable host, a 403 quota, a 409 duplicate, a pending approval and a
  sync-off instance each have a distinct, honest state.
- Seerr being down never blocks sign-in, Home, library, detail, search or playback.
- `cargo test -p seerr-api`, `cargo test -p runefin`, `scripts/check.sh` and
  `scripts/snapshots.sh` are green; the new tests are in the counts.
- No secret appears in the tree, a log, a toast or a snapshot.
- The accessibility pass covers the page at 100 % and 130 %, with a keyboard-only walk and the
  text + bar progress.

---

## 11. Open questions

1. **Answered by S0.0:** the live instance is Jellyseerr 3.5.1 at `jellyseerr.example`,
   Jellyfin-backed, with an admin API key. Should the docs keep the hostname or use
   `jellyseerr.example` in the committed fixtures?
2. **Sync Enabled is false on both Radarr and Sonarr**, so there are no live download rows. Do
   you want to enable it (Seerr → Settings → Services) so S1 can prove the bars against real
   traffic? The client code does not change either way.
3. Is there more than one Seerr user? The key sees all requests and the instance shows one
   `displayName` (`example-user`); the requester label and `X-API-User` only earn their place if
   there is a second user.
4. Do you want S1 (Requests first, read-only proof) or S2 (Discover first, immediate requesting)?
   The plan recommends S1.
5. Is "Request missing seasons" on a Jellyfin series (S3) worth doing now, or should the app stay
   two separate pages until the core is used?
6. `enableSpecialEpisodes` is false on this instance. Hide season 0 in the picker, or show it
   disabled with an explanation?
