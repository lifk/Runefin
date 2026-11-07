//! Parsing tests against bodies copied from a real server (10.10.x, 2026).
//! These run with no network: the fixture is the contract.

use super::*;

const ITEMS: &str = r#"{
  "Items": [
    {
      "Id": "a1b2c3",
      "Name": "Blade Runner 2049",
      "Type": "Movie",
      "ProductionYear": 2017,
      "RunTimeTicks": 97800000000,
      "Overview": "A young blade runner's discovery…",
      "CommunityRating": 8.0,
      "OfficialRating": "R",
      "Genres": ["Science Fiction", "Drama"],
      "ImageTags": { "Primary": "9f2c" },
      "BackdropImageTags": ["77aa"],
      "PrimaryImageAspectRatio": 0.666,
      "UserData": {
        "PlaybackPositionTicks": 450000000,
        "PlayCount": 0,
        "IsFavorite": false,
        "Played": false,
        "UnplayedItemCount": 0
      }
    },
    {
      "Id": "d4e5f6",
      "Name": "The One Where It Rains",
      "Type": "Episode",
      "SeriesName": "Some Show",
      "SeriesId": "999",
      "SeriesPrimaryImageTag": "bb22",
      "SeasonId": "888",
      "IndexNumber": 7,
      "ParentIndexNumber": 2,
      "RunTimeTicks": 2700000000,
      "ImageTags": { "Primary": "aa11" }
    }
  ],
  "TotalRecordCount": 2,
  "StartIndex": 0
}"#;

#[test]
fn a_real_items_body_parses_and_keeps_total_count() {
    let page: ItemsResult = serde_json::from_str(ITEMS).expect("the captured body parses");
    assert_eq!(page.total_record_count, 2);
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].name, "Blade Runner 2049");
    assert_eq!(page.items[0].kind, "Movie");
}

#[test]
fn ticks_become_milliseconds_at_ten_thousand_per_ms() {
    let page: ItemsResult = serde_json::from_str(ITEMS).expect("the captured body parses");
    // 97_800_000_000 ticks = 9_780_000 ms = 2 h 43 m.
    assert_eq!(page.items[0].runtime_ms(), Some(9_780_000));
}

#[test]
fn a_started_unfinished_item_is_resumable_with_its_position() {
    let page: ItemsResult = serde_json::from_str(ITEMS).expect("the captured body parses");
    let movie = &page.items[0];
    let ud = movie.user_data.as_ref().expect("the movie has watch state");
    assert_eq!(ud.position_ms(), Some(45_000));
    assert!(ud.is_resumable());
}

#[test]
fn an_item_that_never_started_is_not_resumable() {
    let page: ItemsResult = serde_json::from_str(ITEMS).expect("the captured body parses");
    let episode = &page.items[1];
    // No UserData at all: a card must not be badged with a progress bar.
    assert!(episode.user_data.is_none());
    assert!(
        !episode
            .user_data
            .as_ref()
            .map(UserData::is_resumable)
            .unwrap_or(false)
    );
}

#[test]
fn a_session_carries_its_device_and_now_playing_item() {
    let body = r#"{
      "Id": "session-1",
      "UserId": "u1",
      "UserName": "luis",
      "Client": "Jellyfin Web",
      "DeviceName": "living-room",
      "DeviceId": "dev-2",
      "IsActive": true,
      "NowPlayingItem": { "Id": "item-1", "Name": "Dune", "Type": "Movie" },
      "PlayState": { "PositionTicks": 450000000, "IsPaused": true, "CanSeek": true }
    }"#;
    let session: SessionInfo = serde_json::from_str(body).expect("the session parses");
    assert_eq!(session.device_name, "living-room");
    assert_eq!(
        session
            .now_playing_item
            .as_ref()
            .map(|item| item.name.as_str()),
        Some("Dune")
    );
    assert_eq!(
        session.play_state.as_ref().map(|state| state.is_paused),
        Some(true)
    );
}

#[test]
fn episodes_carry_their_season_and_number() {
    let page: ItemsResult = serde_json::from_str(ITEMS).expect("the captured body parses");
    assert_eq!(page.items[1].episode_label().as_deref(), Some("S2E07"));
    assert_eq!(page.items[1].series_name.as_deref(), Some("Some Show"));
    // The series poster's tag is what an episode card fetches instead of its own still.
    assert_eq!(
        page.items[1].series_primary_image_tag.as_deref(),
        Some("bb22")
    );
}

#[test]
fn image_tags_are_read_per_kind_and_absent_kinds_stay_absent() {
    let page: ItemsResult = serde_json::from_str(ITEMS).expect("the captured body parses");
    let movie = &page.items[0];
    assert_eq!(movie.image_tag(ImageKind::Primary), Some("9f2c"));
    assert_eq!(movie.image_tag(ImageKind::Backdrop), Some("77aa"));
    // Never invented: a missing tag means no image request at all.
    assert_eq!(movie.image_tag(ImageKind::Logo), None);
}

#[test]
fn people_arrive_with_their_role_and_portrait_tag() {
    let body = r#"{
      "Id":"show-1","Name":"Severance","Type":"Series",
      "People":[
        {"Id":"p1","Name":"Adam Scott","Role":"Mark Scout","Type":"Actor","PrimaryImageTag":"face-1"},
        {"Id":"p2","Name":"Dan Erickson","Role":"Creator","Type":"Writer"}
      ]
    }"#;
    let item: Item = serde_json::from_str(body).expect("the detail body parses");
    assert_eq!(item.people.len(), 2);
    assert_eq!(item.people[0].name, "Adam Scott");
    assert_eq!(item.people[0].role.as_deref(), Some("Mark Scout"));
    assert_eq!(item.people[0].primary_image_tag.as_deref(), Some("face-1"));
    assert_eq!(item.people[1].role.as_deref(), Some("Creator"));
    assert!(item.people[1].primary_image_tag.is_none());
}

#[test]
fn chapters_parse_their_start_and_label() {
    let body = r#"{"Id":"e1","Name":"Episode","Type":"Episode",
      "Chapters":[{"StartPositionTicks":0,"Name":"Cold open"},
                  {"StartPositionTicks":3000000000,"Name":"The turn"}]}"#;
    let item: Item = serde_json::from_str(body).expect("the chapter body parses");
    assert_eq!(item.chapters.len(), 2);
    assert_eq!(item.chapters[0].name, "Cold open");
    assert_eq!(item.chapters[1].start_ms(), 300_000);
}

#[test]
fn trickplay_picks_the_640_set_and_maps_time_to_its_tile_cell() {
    // The server nests widths under the item id, even when the request is for one item.
    let body = r#"{"Id":"e1","Name":"Episode","Type":"Episode","Trickplay":{"e1":{
      "320":{"Width":320,"Height":180,"TileWidth":10,"TileHeight":10,
             "ThumbnailCount":400,"Interval":10000,"Bandwidth":2000},
      "640":{"Width":640,"Height":360,"TileWidth":10,"TileHeight":10,
             "ThumbnailCount":156,"Interval":10000,"Bandwidth":8000}}}}"#;
    let item: Item = serde_json::from_str(body).expect("the trickplay body parses");
    let (width, info) = item.trickplay_manifest().expect("a manifest is chosen");
    assert_eq!(width, 640, "the sharper set wins when the server has it");
    assert_eq!(info.thumbnails_per_tile(), 100);
    // 615 s is thumbnail 61 in a 10-column sprite: the seventh cell of its first row.
    assert_eq!(info.thumbnail_at(615_000), 61);
    assert_eq!(info.tile_of(61), 0);
    assert_eq!(info.cell_of(61), (1, 6));
    // 1,560 s is thumbnail 156, past the last generated thumbnail: clamp, never ask for it.
    assert_eq!(info.thumbnail_at(1_560_000), 155);
    assert_eq!(info.tile_of(155), 1);
    // Row is inside the tile: 155 is the sixth row of tile 1, not the sixteenth of the sprite.
    assert_eq!(info.cell_of(155), (5, 5));
    assert_eq!(info.cell_of(100), (0, 0));
    assert_eq!(info.cell_of(105), (5, 0));
}

#[test]
fn trickplay_falls_back_to_the_widest_set_deterministically() {
    let body = r#"{"Id":"e1","Name":"Episode","Type":"Episode","Trickplay":{"e1":{
      "160":{"Width":160,"Height":90,"TileWidth":5,"TileHeight":5,"ThumbnailCount":9,
             "Interval":10000},
      "480":{"Width":480,"Height":270,"TileWidth":5,"TileHeight":5,"ThumbnailCount":9,
             "Interval":10000}}}}"#;
    let item: Item = serde_json::from_str(body).expect("the trickplay body parses");
    let (width, _) = item.trickplay_manifest().expect("a manifest is chosen");
    assert_eq!(width, 480);
}

#[test]
fn media_sources_expose_every_subtitle_track_and_mark_the_bitmap_ones() {
    let body = r#"{"Id":"e1","Name":"Episode","Type":"Episode","MediaSources":[{
      "Id":"source-1","Container":"mkv","Size":123456789,"MediaStreams":[
        {"Index":-1,"Type":"Video","Codec":"hevc","Width":3840,"Height":2160,
         "VideoRange":"HDR10","Profile":"Main 10"},
        {"Index":0,"Type":"Subtitle","Codec":"subrip","Language":"eng",
         "DisplayTitle":"English - SUBRIP - External","IsExternal":true,
         "IsTextSubtitleStream":true},
        {"Index":1,"Type":"Subtitle","Codec":"pgssub","Language":"eng",
         "DisplayTitle":"English - PGS","IsTextSubtitleStream":false},
        {"Index":2,"Type":"Audio","Codec":"opus","Language":"jpn","Channels":2,
         "DisplayTitle":"Japanese - OPUS - Stereo"},
        {"Index":4,"Type":"Audio","Codec":"aac","Language":"eng","Channels":6,
         "DisplayTitle":"English - AAC - 5.1"},
        {"Index":3,"Type":"Subtitle","Codec":"ass","DisplayTitle":"",
         "IsTextSubtitleStream":true}
      ]},{"Id":"source-2","Container":"mp4","MediaStreams":[
        {"Index":0,"Type":"Audio","Codec":"aac","Language":"spa"},
        {"Index":1,"Type":"Subtitle","Codec":"subrip","Language":"spa",
         "IsTextSubtitleStream":true}
      ]}]}"#;
    let item: Item = serde_json::from_str(body).expect("the media source body parses");
    let source = item.primary_media_source().expect("a source");
    assert_eq!(source.id, "source-1");
    assert_eq!(item.media_sources.len(), 2, "versions stay separate");
    assert_eq!(source.size, Some(123_456_789));
    let tracks = item.subtitle_tracks();
    assert_eq!(tracks.len(), 3, "bitmap tracks are listed, not dropped");
    assert_eq!(tracks[0].index, 0);
    assert_eq!(tracks[0].display_title, "English - SUBRIP - External");
    assert!(tracks[0].is_external);
    assert!(tracks[0].is_text);
    assert_eq!(tracks[1].index, 1);
    assert_eq!(tracks[1].codec.as_deref(), Some("pgssub"));
    assert!(!tracks[1].is_text, "PGS cannot be drawn as text");
    assert_eq!(tracks[2].index, 3);
    assert_eq!(
        tracks[2].display_title, "Subtitle 4",
        "a missing title falls back"
    );
}

#[test]
fn iso_timestamps_round_trip_across_leap_days() {
    assert_eq!(iso_seconds("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(
        iso_seconds("2026-11-03T22:00:00.0000000Z"),
        Some(1_793_743_200)
    );
    assert_eq!(iso_seconds("2024-02-29T12:34:56Z"), Some(1_709_210_096));
    assert_eq!(iso_from_seconds(1_793_743_200), "2026-11-03T22:00:00Z");
    assert_eq!(iso_from_seconds(1_709_210_096), "2024-02-29T12:34:56Z");
    assert_eq!(iso_seconds("not a date"), None);
}

#[test]
fn media_segments_parse_intro_and_outro_spans() {
    let segment: MediaSegment = serde_json::from_str(
        r#"{"Id":"s1","ItemId":"e1","Type":"Intro","StartTicks":0,"EndTicks":900070000}"#,
    )
    .expect("the segment parses");
    assert_eq!(segment.kind, "Intro");
    assert_eq!(segment.start_ms(), 0);
    assert_eq!(segment.end_ms(), 90_007);
}

#[test]
fn the_chapter_under_the_playhead_is_the_last_one_that_started() {
    let item: Item = serde_json::from_str(
        r#"{"Id":"i","Name":"n","Type":"Movie","Chapters":[
            {"StartPositionTicks":0,"Name":"Cold open"},
            {"StartPositionTicks":600000000,"Name":"Act one"}]}"#,
    )
    .expect("the item parses");
    assert_eq!(
        item.chapter_at(0).map(|chapter| chapter.name.as_str()),
        Some("Cold open")
    );
    assert_eq!(
        item.chapter_at(59_999).map(|chapter| chapter.name.as_str()),
        Some("Cold open")
    );
    assert_eq!(
        item.chapter_at(60_000).map(|chapter| chapter.name.as_str()),
        Some("Act one")
    );
}

#[test]
fn a_remote_subtitle_parses_its_provider_facts() {
    let body = r#"[{"ThreeLetterISOLanguageName":"eng","Id":"abc_srt-eng-1",
        "ProviderName":"Open Subtitles","Name":"Severance.S02E04.1080p.WEB-DL",
        "Format":"srt","Author":"tedi","Comment":"Retail NF",
        "DateCreated":"2023-09-29T17:01:03.0000000Z","CommunityRating":3.0,
        "FrameRate":23.976,"DownloadCount":64498,"IsHashMatch":true,
        "Forced":false,"HearingImpaired":true,"AiTranslated":false,
        "MachineTranslated":false}]"#;
    let results: Vec<RemoteSubtitle> = serde_json::from_str(body).expect("results parse");
    let result = &results[0];
    assert_eq!(result.language.as_deref(), Some("eng"));
    assert_eq!(result.id, "abc_srt-eng-1");
    assert!(result.is_hash_match);
    assert!(result.hearing_impaired);
    assert_eq!(
        result.detail_line(),
        "Open Subtitles · 64k downloads · ★ 3.0 · exact file · SDH · SRT"
    );
    assert_eq!(result.credit_line().as_deref(), Some("tedi · Retail NF"));
}

#[test]
fn a_sparse_remote_subtitle_does_not_print_empty_separators() {
    let result: RemoteSubtitle =
        serde_json::from_str(r#"{"Id":"x","Name":"no metadata","ProviderName":"Open Subtitles"}"#)
            .expect("the sparse result parses");
    assert_eq!(result.detail_line(), "Open Subtitles");
    assert_eq!(result.credit_line(), None);
}

#[test]
fn unknown_fields_from_a_newer_server_are_ignored() {
    let body = r#"{"Id":"x","Name":"Y","Type":"Movie","SomeFieldFrom2027":true,
                   "UserData":{"PlaybackPositionTicks":0,"BrandNewKey":{"a":1}}}"#;
    let item: Item = serde_json::from_str(body).expect("the item body parses");
    assert_eq!(item.name, "Y");
}

#[test]
fn a_stop_report_is_its_own_payload_and_carries_the_final_position() {
    let mut stop = PlaybackStop::new("abc", 600_000);
    stop.play_session_id = Some("ps-1".into());
    let json = serde_json::to_string(&stop).expect("the stop report serialises");
    assert!(json.contains("\"PositionTicks\":6000000000"));
    assert!(json.contains("\"Failed\":false"));
    // PlaybackStopInfo has no IsPaused/CanSeek; sending them would be noise the server ignores.
    assert!(!json.contains("IsPaused"));
    assert!(!json.contains("CanSeek"));
}

#[test]
fn a_report_serialises_with_jellyfin_field_names() {
    let report = PlaybackReport::direct_play("abc", 1_500, true);
    let json = serde_json::to_string(&report).expect("the progress report serialises");
    assert!(json.contains("\"ItemId\":\"abc\""));
    assert!(json.contains("\"PositionTicks\":15000000"));
    assert!(json.contains("\"IsPaused\":true"));
    assert!(json.contains("\"PlayMethod\":\"DirectPlay\""));
    assert!(!json.contains("PlaySessionId"), "absent fields are omitted");

    let mut negotiated = PlaybackReport::direct_play("abc", 1_500, false);
    negotiated.play_method = "Transcode";
    negotiated.play_session_id = Some("session-1".into());
    let json = serde_json::to_string(&negotiated).expect("the negotiated report serialises");
    assert!(json.contains("\"PlayMethod\":\"Transcode\""));
    assert!(json.contains("\"PlaySessionId\":\"session-1\""));
}
