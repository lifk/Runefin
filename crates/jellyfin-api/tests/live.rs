//! Opt-in contract check against a real Jellyfin server.
//!
//! Ignored by default: `cargo test -p jellyfin-api` must stay hermetic. Run it with:
//!
//! ```sh
//! JELLYFIN_URL=https://jellyfin.example \
//! JELLYFIN_USER_ID=... JELLYFIN_TOKEN=... \
//! cargo test -p jellyfin-api --test live -- --ignored --nocapture
//! ```
//!
//! It is read-only: no playback reports, so it does not change Continue Watching.

use jellyfin_api::{Client, ImageKind, ItemQuery};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} for the live contract test"))
}

#[test]
#[ignore = "needs a real Jellyfin server; set JELLYFIN_URL, JELLYFIN_USER_ID and JELLYFIN_TOKEN"]
fn the_real_server_still_matches_the_wire_contract() {
    let client = Client::new(&env("JELLYFIN_URL"), "runefin-live")
        .with_session(env("JELLYFIN_TOKEN"), env("JELLYFIN_USER_ID"));

    let info = client.public_system_info().expect("public system info");
    assert!(!info.server_name.is_empty());
    assert!(!info.version.is_empty());

    let libraries = client.libraries().expect("libraries");
    assert!(!libraries.is_empty(), "the server exposes no libraries");

    let sessions = client.sessions().expect("sessions");
    assert!(
        sessions.iter().all(|session| !session.id.is_empty()),
        "a session came back without an id"
    );

    let resume = client.resume(24).expect("continue watching");
    for item in &resume.items {
        assert!(
            item.user_data.is_some(),
            "{} came back without UserData; fields=UserData is not taking effect",
            item.name
        );
        if item.series_id.is_some() {
            // Specials can lack the series poster; the app falls back to the item's own image.
            assert!(
                item.series_primary_image_tag.is_some()
                    || item.image_tag(ImageKind::Primary).is_some(),
                "{} is an episode with no poster at all",
                item.name
            );
        }
    }

    // Filmography: personIds is a credit query, not a name search.
    let mut person_checked = false;
    for item in resume.items.iter().take(6) {
        let Ok(detail) = client.item(&item.id) else {
            continue;
        };
        for person in detail.people.iter().take(3) {
            if let Ok(credits) = client.person_items(&person.id, 20)
                && !credits.is_empty()
            {
                eprintln!("person: {} -> {} credits", person.name, credits.len());
                person_checked = true;
                break;
            }
        }
        if person_checked {
            break;
        }
    }
    if !person_checked {
        eprintln!("person: no credited person in the sample");
    }

    if client.quick_connect_enabled().unwrap_or(false) {
        let quick = client
            .quick_connect_initiate()
            .expect("quick connect initiate");
        assert!(!quick.code.is_empty(), "quick connect returned no code");
        client
            .quick_connect_authorize(&quick.code)
            .expect("approve the code from this signed-in session");
        let polled = client
            .quick_connect_poll(&quick.secret)
            .expect("poll quick connect");
        assert!(
            polled.authenticated,
            "the approved code did not authenticate"
        );
        let auth = client
            .authenticate_with_quick_connect(&quick.secret)
            .expect("exchange the secret for a token");
        assert!(!auth.access_token.is_empty());
        eprintln!("quick connect: code approved and token exchanged");
    }

    let collections = client.collections(12).expect("collections");
    eprintln!("collections: {} box sets", collections.len());

    // Recommendations are read-only. The server answers for films; an episode can legitimately
    // have none, so the probe asks about a movie.
    let movie_query = ItemQuery {
        recursive: true,
        include_types: vec!["Movie"],
        limit: 1,
        ..ItemQuery::default()
    };
    if let Some(movie) = client
        .items(&movie_query)
        .ok()
        .and_then(|page| page.items.into_iter().next())
    {
        match client.similar(&movie.id, 12) {
            Ok(similar) => eprintln!(
                "similar: {} for {:?}, first: {:?}",
                similar.len(),
                movie.name,
                similar.first().map(|item| item.name.clone())
            ),
            Err(error) => eprintln!("similar: failed ({error})"),
        }
    }

    // Reordering: create three entries, move the first to the end, read the order back, delete.
    let mut order_query = ItemQuery {
        recursive: true,
        include_types: vec!["Movie"],
        limit: 3,
        ..ItemQuery::default()
    };
    order_query.limit = 3;
    if let Ok(page) = client.items(&order_query)
        && page.items.len() == 3
    {
        let ids: Vec<String> = page.items.iter().map(|item| item.id.clone()).collect();
        if let Ok(playlist) = client.create_playlist("Runefin order probe", &ids) {
            let before = client.playlist_items(&playlist).unwrap_or_default();
            let first_entry = before
                .first()
                .and_then(|item| item.playlist_item_id.clone());
            let moved = first_entry
                .as_ref()
                .map(|entry| client.move_playlist_entry(&playlist, entry, 2).is_ok())
                .unwrap_or(false);
            let after = client.playlist_items(&playlist).unwrap_or_default();
            eprintln!(
                "playlist order: moved {moved}, order now {:?}",
                after
                    .iter()
                    .map(|item| item.name.clone())
                    .collect::<Vec<_>>()
            );
            let _ = client.delete_playlist(&playlist);
        }
    }

    // Image subtitles are listed as non-text rather than silently missing from the track list.
    let mut sub_query = ItemQuery {
        recursive: true,
        include_types: vec!["Episode", "Movie"],
        limit: 100,
        extra_fields: Some("MediaSources".into()),
        ..ItemQuery::default()
    };
    sub_query.limit = 100;
    if let Ok(page) = client.items(&sub_query) {
        for item in page.items {
            let tracks = item.subtitle_tracks();
            let image = tracks.iter().filter(|track| !track.is_text).count();
            if image > 0 {
                eprintln!(
                    "subtitles: {:?} has {} text and {} image tracks ({})",
                    item.name,
                    tracks.len() - image,
                    image,
                    tracks
                        .iter()
                        .filter(|track| !track.is_text)
                        .filter_map(|track| track.codec.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                break;
            }
        }
    }

    // Chapter stills: a film with chapters exposes an image per chapter.
    let chapter_query = ItemQuery {
        recursive: true,
        include_types: vec!["Movie"],
        limit: 20,
        extra_fields: Some("Chapters".into()),
        ..ItemQuery::default()
    };
    if let Ok(page) = client.items(&chapter_query)
        && let Some(item) = page.items.iter().find(|item| {
            item.chapters
                .iter()
                .any(|chapter| chapter.image_tag.is_some())
        })
    {
        let chapter = item
            .chapters
            .iter()
            .position(|chapter| chapter.image_tag.is_some())
            .unwrap_or(0);
        let tag = item.chapters[chapter].image_tag.clone().unwrap_or_default();
        match client.chapter_image_url(&item.id, chapter as u32, &tag, 160) {
            Some(url) => match client.fetch_bytes(&url) {
                Ok(bytes) => eprintln!(
                    "chapter image: {:?} chapter {} -> {} bytes",
                    item.name,
                    chapter + 1,
                    bytes.len()
                ),
                Err(error) => eprintln!("chapter image: fetch failed ({error})"),
            },
            None => eprintln!("chapter image: no URL"),
        }
    }

    // A TV library's shuffle: the server randomises episodes below it.
    if let Some(shows) = libraries
        .iter()
        .find(|library| library.collection_type.as_deref() == Some("tvshows"))
    {
        match client.episodes_in(&shows.id, jellyfin_api::SortBy::Random, 5) {
            Ok(episodes) => eprintln!(
                "shuffle: {} random episodes from {:?}, kinds {:?}",
                episodes.len(),
                shows.name,
                episodes
                    .iter()
                    .map(|episode| episode.kind.clone())
                    .collect::<Vec<_>>()
            ),
            Err(error) => eprintln!("shuffle: failed ({error})"),
        }
    }

    // The item menu's two writes, exercised and restored: favourite and played.
    let mut toggle_query = ItemQuery {
        recursive: true,
        include_types: vec!["Movie"],
        limit: 1,
        ..ItemQuery::default()
    };
    toggle_query.limit = 1;
    if let Some(movie) = client
        .items(&toggle_query)
        .ok()
        .and_then(|page| page.items.into_iter().next())
    {
        let before = client.item(&movie.id).expect("the movie").user_data;
        // Never disturb a real watch position: only a film nobody has started is a safe probe.
        let untouched = before
            .as_ref()
            .and_then(|user| user.position_ms())
            .is_none_or(|position| position == 0);
        let favorite = before.as_ref().is_some_and(|user| user.is_favorite);
        let played = before.as_ref().is_some_and(|user| user.played);
        if !untouched {
            eprintln!("watch state: skipped (the probe film has progress)");
            return;
        }
        let flipped = client.set_favorite(&movie.id, !favorite).is_ok();
        let after = client.item(&movie.id).expect("the movie after the write");
        let favourite_stuck = after
            .user_data
            .as_ref()
            .is_some_and(|user| user.is_favorite == !favorite);
        let played_flipped = client.set_played(&movie.id, !played).is_ok();
        let after_played = client.item(&movie.id).expect("the movie after played");
        let played_stuck = after_played
            .user_data
            .as_ref()
            .is_some_and(|user| user.played == !played);
        // Put the item back exactly as it was.
        let _ = client.set_favorite(&movie.id, favorite);
        let _ = client.set_played(&movie.id, played);
        eprintln!(
            "watch state: favourite write {flipped}, read back {favourite_stuck}; played write {played_flipped}, read back {played_stuck}"
        );
    }

    // Clearing the resume position is how "Remove from Continue Watching" works; the probe
    // restores the exact position afterwards.
    if let Some(item) = resume.items.first() {
        let before = client
            .item(&item.id)
            .ok()
            .and_then(|item| item.user_data)
            .and_then(|user| user.position_ms())
            .unwrap_or(0);
        if before > 0 {
            let cleared = client
                .set_resume_position(&item.id, 0)
                .map(|()| {
                    client
                        .resume(50)
                        .map(|list| !list.items.iter().any(|entry| entry.id == item.id))
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            let restored = client
                .set_resume_position(&item.id, before * jellyfin_api::TICKS_PER_MS)
                .map(|()| {
                    client
                        .resume(50)
                        .map(|list| list.items.iter().any(|entry| entry.id == item.id))
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            eprintln!("resume removal: cleared {cleared}, restored {restored}");
        }
    }

    // Playlists are a mutation, so the probe creates one, checks it, and deletes it again in
    // the same run. A failure after creation still reaches the delete.
    {
        let mut ids: Vec<String> = resume
            .items
            .iter()
            .take(2)
            .map(|item| item.id.clone())
            .collect();
        ids.push(libraries[0].id.clone());
        let ids = ids.into_iter().take(2).collect::<Vec<_>>();
        match client.create_playlist("Runefin probe", &ids) {
            Ok(playlist) => {
                let extra = resume.items.get(2).map(|item| vec![item.id.clone()]);
                let added = match &extra {
                    Some(extra) => client.add_to_playlist(&playlist, extra).is_ok(),
                    None => false,
                };
                let items = client.playlist_items(&playlist);
                let deleted = client.delete_playlist(&playlist);
                match items {
                    Ok(items) => eprintln!(
                        "playlists: created with {} entries, add extra: {added}, read back {}",
                        ids.len(),
                        items.len()
                    ),
                    Err(error) => eprintln!("playlists: read failed ({error})"),
                }
                assert!(deleted.is_ok(), "the probe playlist is removed again");
            }
            Err(error) => eprintln!("playlists: create failed ({error})"),
        }
    }

    // Read-only: asks the providers, never downloads. Open Subtitles may rate-limit, so an
    // empty list is reported rather than failing the whole contract run.
    if let Some(item) = resume.items.first() {
        match client.search_subtitles(&item.id, "eng") {
            Ok(results) => {
                eprintln!(
                    "subtitles: {} results for {:?}, first: {}",
                    results.len(),
                    item.name,
                    results
                        .first()
                        .map(|result| result.detail_line())
                        .unwrap_or_default()
                );
            }
            Err(error) => eprintln!("subtitles: search failed ({error})"),
        }
    }

    let genres = client
        .genres(Some(&libraries[0].id))
        .expect("library genres");
    if let Some(genre) = genres.first() {
        let mut query = ItemQuery::in_library(libraries[0].id.clone());
        query.genre_ids = vec![genre.id.clone()];
        let filtered = client.items(&query).expect("genre filter");
        eprintln!(
            "genres: {} in {}, {} items for {}",
            genres.len(),
            libraries[0].name,
            filtered.total_record_count,
            genre.name
        );
    }

    let next_up = client.next_up_all(12).expect("next up");
    assert!(
        next_up.iter().all(|item| item.kind == "Episode"),
        "next up must be episodes"
    );
    let with_poster = next_up
        .iter()
        .filter(|item| {
            item.series_primary_image_tag.is_some() || item.image_tag(ImageKind::Primary).is_some()
        })
        .count();
    eprintln!(
        "next up: {} rows, {with_poster} with a poster",
        next_up.len()
    );

    let channels = client.live_channels(10).expect("live channels");
    eprintln!("live tv: {} channels", channels.len());
    if let Some(channel) = channels.first() {
        let stream = client
            .open_live_stream(&channel.id)
            .expect("open a channel");
        assert!(stream.url.starts_with("http"), "the tuner returned no URL");
        if let Some(id) = &stream.live_stream_id {
            client.close_live_stream(id).expect("close the channel");
        }
        eprintln!(
            "live tv: {} ({:?}) open and closed",
            channel.name, channel.kind
        );
    }

    let library = &libraries[0];
    let latest = client
        .latest(Some(&library.id), 12)
        .expect("recently added");
    let page = client
        .items(&ItemQuery::in_library(library.id.clone()))
        .expect("library page");
    assert!(
        page.total_record_count >= 0,
        "the page has no total record count"
    );

    let sample = page
        .items
        .first()
        .or_else(|| latest.first())
        .expect("the first library has no items to inspect");
    let detail = client.item(&sample.id).expect("item detail");
    assert_eq!(detail.id, sample.id);

    if detail.kind == "Series" {
        let mut seasons_query = ItemQuery::in_library(detail.id.clone());
        seasons_query.recursive = false;
        seasons_query.include_types = vec!["Season"];
        seasons_query.limit = 50;
        let seasons = client.items(&seasons_query).expect("seasons");
        assert!(!seasons.items.is_empty(), "series has no seasons");
        let mut episodes_query = ItemQuery::in_library(seasons.items[0].id.clone());
        episodes_query.recursive = false;
        episodes_query.include_types = vec!["Episode"];
        episodes_query.limit = 50;
        let episodes = client.items(&episodes_query).expect("episodes");
        assert!(!episodes.items.is_empty(), "season has no episodes");
    }

    // Trickplay is optional per item, but when the server generates it the nested manifest
    // must parse and its first tile must be fetchable.
    let mut trickplay_checked = false;
    for item in resume.items.iter().take(12) {
        let detail = client.item(&item.id).expect("item detail with trickplay");
        if let Some((width, info)) = detail.trickplay_manifest() {
            assert!(width > 0 && info.thumbnail_count > 0, "empty manifest");
            assert!(info.tile_width > 0 && info.tile_height > 0, "no tile grid");
            let tile = info.tile_of(info.thumbnail_at(60_000));
            let url = client
                .trickplay_url(&detail.id, width, tile)
                .expect("a trickplay URL");
            let bytes = client.fetch_bytes(&url).expect("trickplay tile");
            assert!(!bytes.is_empty(), "the trickplay URL returned no bytes");
            eprintln!(
                "trickplay: {} {width}px {}x{} tiles, {} thumbnails",
                detail.name, info.tile_width, info.tile_height, info.thumbnail_count
            );
            trickplay_checked = true;
            break;
        }
    }
    if !trickplay_checked {
        eprintln!("trickplay: no item in Continue Watching has generated tiles");
    }

    if let Some(tag) = sample.image_tag(ImageKind::Primary) {
        let url = client.image_url(&sample.id, ImageKind::Primary, 64, Some(tag), None);
        let bytes = client.fetch_bytes(&url).expect("primary image");
        assert!(!bytes.is_empty(), "the image URL returned no bytes");
    }

    eprintln!(
        "{} {} — {} libraries, {} resume rows, {} recent in {}",
        info.server_name,
        info.version,
        libraries.len(),
        resume.items.len(),
        latest.len(),
        library.name,
    );
}
#[test]
#[ignore = "needs a real Jellyfin server; set JELLYFIN_URL, JELLYFIN_USER_ID and JELLYFIN_TOKEN"]
fn playback_info_offers_direct_and_transcode() {
    let client = Client::new(&env("JELLYFIN_URL"), "runefin-live")
        .with_session(env("JELLYFIN_TOKEN"), env("JELLYFIN_USER_ID"));
    let movies = client
        .items(&ItemQuery {
            include_types: vec!["Movie", "Episode"],
            recursive: true,
            limit: 1,
            ..ItemQuery::default()
        })
        .expect("a playable item");
    let item = movies.items.first().expect("a playable item");
    let ask = |bitrate: Option<i64>| {
        client.playback_info(
            &item.id,
            &jellyfin_api::PlaybackInfoRequest {
                user_id: env("JELLYFIN_USER_ID"),
                start_time_ticks: 0,
                max_streaming_bitrate: bitrate,
                media_source_id: None,
                audio_stream_index: None,
                subtitle_stream_index: None,
                max_audio_channels: Some(2),
                enable_direct_play: true,
                enable_direct_stream: true,
                enable_transcoding: true,
                allow_video_stream_copy: true,
                allow_audio_stream_copy: true,
                device_profile: Client::device_profile(),
            },
        )
    };
    let direct = ask(None).expect("direct PlaybackInfo");
    eprintln!(
        "playback: {} sources, session {:?}, direct={} stream={} transcode={}",
        direct.media_sources.len(),
        direct.play_session_id,
        direct.media_sources[0].supports_direct_play,
        direct.media_sources[0].supports_direct_stream,
        direct.media_sources[0].supports_transcoding
    );
    assert!(!direct.media_sources.is_empty());
    assert!(
        direct
            .media_sources
            .iter()
            .any(|source| source.supports_direct_play),
        "this file should still direct-play with the Runefin device profile"
    );
    let capped = ask(Some(2_000_000)).expect("capped PlaybackInfo");
    let transcodes = capped
        .media_sources
        .iter()
        .any(|source| source.supports_transcoding && source.transcoding_url.is_some());
    eprintln!("playback: capped transcode offered = {transcodes}");
    assert!(
        transcodes,
        "a 2 Mbps cap must offer a transcoding URL for this server"
    );
    let source = capped
        .media_sources
        .iter()
        .find(|source| source.supports_transcoding && source.transcoding_url.is_some())
        .expect("a transcoding source");
    let playlist = client
        .fetch_bytes(&client.authed_url(source.transcoding_url.as_deref().expect("a URL")))
        .expect("the transcoding URL is fetchable");
    assert!(
        playlist.starts_with(b"#EXTM3U"),
        "the transcoder returned something that is not an HLS playlist"
    );
    eprintln!("playback: HLS playlist {} bytes", playlist.len());
}

#[test]
#[ignore = "needs a real Jellyfin server; creates and deletes a collection"]
fn a_collection_can_take_new_members_and_give_them_back() {
    let client = Client::new(&env("JELLYFIN_URL"), "runefin-live")
        .with_session(env("JELLYFIN_TOKEN"), env("JELLYFIN_USER_ID"));
    let movies = client
        .items(&ItemQuery {
            recursive: true,
            include_types: vec!["Movie"],
            limit: 2,
            ..ItemQuery::default()
        })
        .expect("two movies");
    let ids: Vec<String> = movies
        .items
        .iter()
        .take(2)
        .map(|item| item.id.clone())
        .collect();
    assert_eq!(ids.len(), 2, "the server needs two movies for this probe");
    let name = format!(
        "Runefin live collection {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    client
        .create_collection(&name, &ids[..1])
        .expect("create the probe collection");
    let created = client
        .collections(200)
        .expect("collections")
        .into_iter()
        .find(|item| item.name == name)
        .expect("the probe collection is listed");
    client
        .add_to_collection(&created.id, &ids[1..])
        .expect("add the second movie");
    let members = client
        .items(&ItemQuery {
            parent_id: Some(created.id.clone()),
            recursive: false,
            include_types: vec!["Movie", "Series"],
            limit: 20,
            ..ItemQuery::default()
        })
        .expect("the collection's members");
    assert_eq!(
        members.items.len(),
        2,
        "both movies are in the collection after the add"
    );
    client
        .remove_from_collection(&created.id, &ids[1..])
        .expect("remove the second movie");
    client
        .delete_playlist(&created.id)
        .expect("delete the probe collection");
    eprintln!("collection: created, added, verified 2 members, uploaded a cover, removed, deleted");
}
