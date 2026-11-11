//! URL and header tests. No network: everything here is what would go on the wire.

use super::*;

#[test]
fn a_bare_host_gets_http_and_no_trailing_slash() {
    assert_eq!(
        normalise_base_url("192.0.2.10:8096/"),
        "http://192.0.2.10:8096"
    );
    assert_eq!(
        normalise_base_url("  jellyfin.lan  "),
        "http://jellyfin.lan"
    );
}

#[test]
fn an_explicit_scheme_is_kept_even_when_it_is_https() {
    assert_eq!(
        normalise_base_url("https://media.example.com/jellyfin/"),
        "https://media.example.com/jellyfin"
    );
}

#[test]
fn the_authorization_header_is_the_quoted_pair_list_jellyfin_parses() {
    let c = Client::new("http://localhost:8096", "dev-1").with_session("tok", "user-1");
    assert_eq!(
        c.authorization(),
        format!(
            "MediaBrowser Client=\"Runefin\", Device=\"{}\", DeviceId=\"dev-1\", Version=\"{}\", Token=\"tok\"",
            device_name(),
            env!("CARGO_PKG_VERSION")
        )
    );
}

#[test]
fn quotes_in_a_token_cannot_break_the_header() {
    let c = Client::new("http://localhost:8096", "dev-1").with_session("a\"b", "user-1");
    assert!(c.authorization().contains("Token=\"a\\\"b\""));
}

#[test]
fn a_library_query_names_its_parent_types_and_page() {
    let q = ItemQuery::in_library("lib-9").to_query();
    assert!(q.contains("parentId=lib-9"));
    assert!(q.contains("recursive=true"));
    assert!(q.contains("limit=60"));
    assert!(q.contains("startIndex=0"));
    // Without these two the server omits watch state and posters lose their progress bars.
    assert!(q.contains("fields=UserData"));
    assert!(q.contains("enableUserData=true"));
}

#[test]
fn a_search_query_escapes_the_term() {
    let mut q = ItemQuery::search("example film & co");
    q.limit = 40;
    let s = q.to_query();
    assert!(s.contains("searchTerm=example%20film%20%26%20co"));
    // Values are percent-encoded, so commas travel as %2C and the server decodes them.
    assert!(s.contains("includeItemTypes=Movie%2CSeries%2CEpisode%2CBoxSet%2CPerson"));
}

#[test]
fn deep_library_filters_go_on_the_wire() {
    let q = ItemQuery {
        include_types: vec!["Movie"],
        recursive: true,
        years: vec![2024, 2025],
        min_community_rating: Some(7.5),
        audio_languages: vec!["eng".into()],
        subtitle_languages: vec!["spa".into()],
        has_subtitles: true,
        is_4k: true,
        sort_by: Some(SortBy::Runtime),
        descending: true,
        ..ItemQuery::default()
    }
    .to_query();
    assert!(q.contains("years=2024%2C2025"), "{q}");
    assert!(q.contains("minCommunityRating=7.5"), "{q}");
    assert!(q.contains("audioLanguages=eng"), "{q}");
    assert!(q.contains("subtitleLanguages=spa"), "{q}");
    assert!(q.contains("HasSubtitles"), "{q}");
    assert!(q.contains("Is4K"), "{q}");
    assert!(q.contains("sortBy=Runtime"), "{q}");
}

#[test]
fn studio_tag_and_series_status_filters_go_on_the_wire() {
    let q = ItemQuery {
        include_types: vec!["Movie", "Series"],
        recursive: true,
        studio_ids: vec!["studio-1".into(), "studio-2".into()],
        tags: vec!["noir".into()],
        series_status: Some("Continuing".into()),
        ..ItemQuery::default()
    }
    .to_query();
    assert!(q.contains("studioIds=studio-1%2Cstudio-2"), "{q}");
    assert!(q.contains("tags=noir"), "{q}");
    assert!(q.contains("seriesStatus=Continuing"), "{q}");
}

#[test]
fn a_recently_played_query_filters_and_sorts_by_date_played() {
    let q = ItemQuery {
        include_types: vec!["Movie", "Episode"],
        recursive: true,
        played_only: true,
        sort_by: Some(SortBy::DatePlayed),
        descending: true,
        limit: 24,
        ..ItemQuery::default()
    }
    .to_query();
    assert!(q.contains("filters=IsPlayed"));
    assert!(q.contains("sortBy=DatePlayed"));
    assert!(q.contains("sortOrder=Descending"));
    assert!(q.contains("includeItemTypes=Movie%2CEpisode"));
}

#[test]
fn only_transport_and_server_errors_are_retried() {
    assert!(Error::Transport("reset".into()).is_transient());
    assert!(
        Error::Status {
            code: 503,
            body: String::new()
        }
        .is_transient()
    );
    assert!(
        !Error::Status {
            code: 404,
            body: String::new()
        }
        .is_transient()
    );
    assert!(!Error::Decode("bad".into()).is_transient());
}

#[test]
fn a_playback_info_request_carries_the_policy_and_device_profile() {
    let request = PlaybackInfoRequest {
        user_id: "user".into(),
        start_time_ticks: 720_000_000,
        max_streaming_bitrate: Some(8_000_000),
        media_source_id: Some("4k".into()),
        audio_stream_index: Some(1),
        subtitle_stream_index: Some(2),
        max_audio_channels: Some(2),
        enable_direct_play: true,
        enable_direct_stream: true,
        enable_transcoding: true,
        allow_video_stream_copy: true,
        allow_audio_stream_copy: true,
        device_profile: Client::device_profile(),
    };
    let json = serde_json::to_value(&request).expect("the request serialises");
    assert_eq!(json["UserId"], "user");
    assert_eq!(json["StartTimeTicks"], 720_000_000);
    assert_eq!(json["MaxStreamingBitrate"], 8_000_000);
    assert_eq!(json["MediaSourceId"], "4k");
    assert_eq!(json["EnableDirectPlay"], true);
    assert_eq!(json["DeviceProfile"]["Name"], "Runefin");
    let profiles = json["DeviceProfile"]["DirectPlayProfiles"]
        .as_array()
        .expect("direct play profiles");
    assert!(profiles.iter().any(|profile| {
        profile["Container"]
            .as_str()
            .is_some_and(|containers| containers.contains("mkv"))
    }));
}

#[test]
fn a_playback_info_response_chooses_direct_stream_and_transcode_urls() {
    let response: PlaybackInfoResponse = serde_json::from_value(serde_json::json!({
        "PlaySessionId": "session-1",
        "MediaSources": [{
            "Id": "4k",
            "Name": "4K HDR10",
            "Container": "mkv",
            "Bitrate": 31_000_000,
            "SupportsDirectPlay": true,
            "SupportsDirectStream": true,
            "SupportsTranscoding": true,
            "DirectStreamUrl": "/Videos/item/stream?static=false",
            "TranscodingUrl": "/Videos/item/master.m3u8?api_key=token",
            "TranscodingSubProtocol": "hls",
            "TranscodingContainer": "ts",
            "PlaySessionId": "session-2"
        }]
    }))
    .expect("the response parses");
    assert_eq!(response.play_session_id.as_deref(), Some("session-1"));
    assert_eq!(response.media_sources[0].id, "4k");
    assert!(response.media_sources[0].supports_direct_play);
    assert!(response.media_sources[0].direct_stream_url.is_some());
    assert!(response.media_sources[0].transcoding_url.is_some());
}

#[test]
fn a_server_relative_url_gets_the_client_base() {
    let client = Client::new("https://media.example", "device");
    assert_eq!(
        client.absolute_url("/Videos/1/master.m3u8"),
        "https://media.example/Videos/1/master.m3u8"
    );
    assert_eq!(
        client.absolute_url("https://cdn.example/x.m3u8"),
        "https://cdn.example/x.m3u8"
    );
}

#[test]
fn a_direct_stream_can_name_its_media_source() {
    let client = Client::new("https://media.example", "device").with_session("token", "user");
    let url = client
        .direct_stream_url_with_source("example 2", Some("4k source"))
        .expect("a URL");
    assert!(url.contains("mediaSourceId=4k%20source"), "{url}");
    assert!(url.contains("/Videos/example%202/stream"), "{url}");
}

#[test]
fn a_transfer_to_another_session_carries_the_start_position() {
    let path = Client::play_on_session_path_at("session 1", &["a b".into()], 720_000_000);
    assert!(path.contains("/Sessions/session%201/Playing"), "{path}");
    assert!(path.contains("playCommand=PlayNow"));
    assert!(path.contains("itemIds=a%20b"));
    assert!(path.contains("startPositionTicks=720000000"));
}

#[test]
fn a_letter_jump_asks_the_server_to_start_with_that_letter() {
    let mut q = ItemQuery::in_library("lib-9");
    q.name_starts_with = Some("S".into());
    assert!(q.to_query().contains("nameStartsWith=S"));
}

#[test]
fn sorting_descending_says_so() {
    let mut q = ItemQuery::in_library("lib-1");
    q.sort_by = Some(SortBy::DateCreated);
    q.descending = true;
    let s = q.to_query();
    assert!(s.contains("sortBy=DateCreated"));
    assert!(s.contains("sortOrder=Descending"));
}

#[test]
fn an_image_url_carries_the_tag_so_it_can_be_cached_forever() {
    let c = Client::new("http://localhost:8096", "dev-1").with_session("tok", "user-1");
    let url = c.image_url("item-1", ImageKind::Primary, 400, Some("9f2c"), None);
    assert_eq!(
        url,
        "http://localhost:8096/Items/item-1/Images/Primary?maxWidth=400&quality=90&tag=9f2c&api_key=tok"
    );
}

#[test]
fn an_image_url_without_a_tag_still_loads() {
    let c = Client::new("http://localhost:8096", "dev-1").with_session("tok", "user-1");
    let url = c.image_url("item-1", ImageKind::Backdrop, 1280, None, None);
    assert!(!url.contains("tag="));
    assert!(url.contains("api_key=tok"));
}

#[test]
fn the_stream_url_mpv_gets_is_static_direct_play() {
    let c = Client::new("http://localhost:8096", "dev-1").with_session("tok", "user-1");
    assert_eq!(
        c.direct_stream_url("item-1")
            .expect("a session has a stream URL"),
        "http://localhost:8096/Videos/item-1/stream?static=true&api_key=tok&UserId=user-1"
    );
}

#[test]
fn a_subtitle_url_names_the_source_and_the_stream_index() {
    let c = Client::new("http://localhost:8096", "dev-1").with_session("tok", "user-1");
    assert_eq!(
        c.subtitle_url("item-1", "source-1", 3)
            .expect("a session has a subtitle URL"),
        "http://localhost:8096/Videos/item-1/source-1/Subtitles/3/Stream.srt?api_key=tok"
    );
}

#[test]
fn a_client_without_a_session_has_no_stream_url() {
    let c = Client::new("http://localhost:8096", "dev-1");
    assert_eq!(c.direct_stream_url("item-1"), None);
}

#[test]
fn an_expired_session_asks_for_a_reconnect_and_nothing_else_does() {
    let expired = Error::Status {
        code: 401,
        body: String::new(),
    };
    let offline = Error::Transport("refused".into());
    assert!(expired.needs_reconnect());
    assert!(!offline.needs_reconnect());
    assert_eq!(
        expired.user_message(),
        "Your session expired. Sign in again."
    );
    assert_eq!(offline.user_message(), "Can't reach the server.");
}

#[test]
fn an_error_message_never_carries_the_session_token() {
    // Transport errors quote the request URL; Jellyfin's stream URLs carry the token in it.
    let transport = Error::Transport(
        "http://host/Videos/i/stream?static=true&api_key=secret123&UserId=u-1 failed".into(),
    );
    let text = transport.to_string();
    assert!(!text.contains("secret123"), "{text}");
    assert!(text.contains("api_key=<redacted>"), "{text}");
    assert!(text.contains("UserId=u-1"), "{text}");

    let status = Error::Status {
        code: 400,
        body: "GET /Items?api_key=secret123 returned 400".into(),
    };
    assert!(!status.to_string().contains("secret123"));
}

#[test]
fn redaction_leaves_ordinary_text_alone() {
    assert_eq!(redact("can't reach the server"), "can't reach the server");
    assert_eq!(redact("?userId=1&limit=2"), "?userId=1&limit=2");
    assert_eq!(redact("?ApiKey=abc"), "?ApiKey=<redacted>");
}

#[test]
fn the_device_profile_declines_dolby_vision_and_still_allows_hdr10() {
    let profile = Client::device_profile();
    let hevc = profile["CodecProfiles"]
        .as_array()
        .expect("codec profiles")
        .iter()
        .find(|entry| entry["Codec"] == "hevc")
        .expect("an hevc profile");
    let conditions = hevc["Conditions"].as_array().expect("conditions");
    let declined: Vec<&str> = conditions
        .iter()
        .filter(|condition| condition["Condition"] == "NotEquals")
        .filter_map(|condition| condition["Value"].as_str())
        .collect();
    assert!(declined.contains(&"DOVIWithHDR10"), "{declined:?}");
    assert!(declined.contains(&"DOVI"), "{declined:?}");
}
