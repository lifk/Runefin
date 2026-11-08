use super::*;

fn parse<T: serde::de::DeserializeOwned>(text: &str) -> T {
    serde_json::from_str(text).expect("fixture parses")
}

#[test]
fn the_status_and_public_settings_parse() {
    let status: Status = parse(include_str!("../../tests/fixtures/status.json"));
    assert_eq!(status.version, "3.5.1");
    assert_eq!(status.commit_tag.as_deref(), Some("local"));

    let public: PublicSettings = parse(
        r#"{
          "initialized": true,
          "applicationTitle": "Jellyseerr",
          "mediaServerType": 2,
          "partialRequestsEnabled": true,
          "enableSpecialEpisodes": false,
          "movie4kEnabled": false,
          "series4kEnabled": false
        }"#,
    );
    assert!(public.initialized);
    assert_eq!(public.application_title.as_deref(), Some("Jellyseerr"));
    assert_eq!(public.media_server_type, Some(2));
    assert_eq!(public.partial_requests_enabled, Some(true));
    assert_eq!(public.enable_special_episodes, Some(false));
}

#[test]
fn a_user_prefers_the_display_name_because_username_is_null() {
    let user: User = parse(include_str!("../../tests/fixtures/auth-me.json"));
    assert_eq!(user.display_name(), "example-user");
    assert_eq!(user.permissions, 2);
    assert_eq!(user.request_count, 3);

    let fallback: User = parse(r#"{"id": 2, "email": "only@example.invalid"}"#);
    assert_eq!(fallback.display_name(), "only@example.invalid");
}

#[test]
fn search_and_discover_use_the_page_total_fields() {
    let search: SearchResults = parse(include_str!("../../tests/fixtures/search.json"));
    assert_eq!(search.total_results, 3);
    assert_eq!(search.results.len(), 3);
    assert_eq!(search.results[0].title(), "Example Film");
    assert_eq!(search.results[0].year(), Some("2021"));
    assert!(search.results[0].is_movie());
    assert!(search.results[1].is_tv());
    assert!(search.results[2].media_info.is_none());

    let discover: SearchResults = parse(include_str!("../../tests/fixtures/discover.json"));
    assert_eq!(discover.total_pages, 1001);
    assert_eq!(discover.results.len(), 2);
    assert!(discover.results[0].media_info.is_none());
    assert_eq!(
        discover.results[1].media_info.as_ref().map(|m| m.status),
        Some(3)
    );
}

#[test]
fn detail_pages_expose_the_title_poster_and_media_state() {
    let movie: MovieDetails = parse(include_str!("../../tests/fixtures/movie-detail.json"));
    assert_eq!(movie.title.as_deref(), Some("Example Film"));
    assert_eq!(movie.year(), Some("2021"));
    assert_eq!(movie.runtime, Some(155));
    assert_eq!(
        movie.media_info.as_ref().map(|m| m.media_status()),
        Some(MediaStatus::Available)
    );

    let tv: TvDetails = parse(include_str!("../../tests/fixtures/tv-detail.json"));
    assert_eq!(tv.name.as_deref(), Some("Example Show"));
    assert_eq!(tv.seasons.len(), 1);
    assert_eq!(tv.seasons[0].season_number, 1);
    assert_eq!(tv.seasons[0].episode_count, 12);
    assert_eq!(
        tv.media_info.as_ref().map(|m| m.media_status()),
        Some(MediaStatus::PartiallyAvailable)
    );
}

#[test]
fn a_person_carries_biography_and_credit_facts() {
    let person: PersonDetails = parse(include_str!("../../tests/fixtures/person.json"));
    assert_eq!(person.name.as_deref(), Some("Cailee Spaeny"));
    assert_eq!(person.birthday.as_deref(), Some("1998-07-24"));
    assert_eq!(person.deathday, None);
    assert_eq!(
        person.place_of_birth.as_deref(),
        Some("Springfield, Missouri, USA")
    );
    assert!(
        person
            .biography
            .as_deref()
            .unwrap_or("")
            .contains("American actress")
    );
    assert_eq!(
        person.profile_path.as_deref(),
        Some("/6gUzyMnwo7TjZA7E1Amd1GI8JGw.jpg")
    );

    let credits: PersonCredits = parse(include_str!("../../tests/fixtures/person-credits.json"));
    assert_eq!(credits.cast.len(), 3);
    assert_eq!(credits.crew.len(), 1);
    assert_eq!(credits.cast[0].title(), "Alien: Romulus");
    assert_eq!(credits.cast[0].year(), Some("2024"));
    assert!(credits.cast[0].is_movie());
    assert_eq!(credits.cast[2].title(), "Devs");
    assert_eq!(credits.cast[2].year(), Some("2020"));
    assert!(credits.cast[2].is_tv());
    assert_eq!(credits.all().count(), 4, "cast and crew, both media types");
}

#[test]
fn a_discover_query_carries_only_the_filters_we_use() {
    let mut query = DiscoverQuery::new(2);
    query.genre = Some(878);
    query.studio = Some(41077);
    query.sort_by = Some("vote_average".into());
    query.from = Some("2020-01-01".into());
    query.to = Some("2029-12-31".into());
    assert_eq!(
        query.query_string("primaryReleaseDate"),
        "page=2&genre=878&studio=41077&sortBy=vote_average&primaryReleaseDateGte=2020-01-01&primaryReleaseDateLte=2029-12-31"
    );
    assert_eq!(DiscoverQuery::new(1).query_string("firstAirDate"), "page=1");
}

#[test]
fn a_detail_carries_genres_tagline_and_credits() {
    let movie: MovieDetails = parse(include_str!("../../tests/fixtures/movie-detail.json"));
    assert_eq!(
        movie.tagline.as_deref(),
        Some("Beyond fear, destiny awaits.")
    );
    assert_eq!(movie.genres.len(), 2);
    assert_eq!(movie.genres[0].name.as_deref(), Some("Science Fiction"));
    assert_eq!(movie.credits.cast.len(), 2);
    assert_eq!(
        movie.credits.cast[0].name.as_deref(),
        Some("Timothée Chalamet")
    );
    assert_eq!(movie.credits.director(), Some("Denis Villeneuve"));
    assert_eq!(movie.credits.creator(), None);
}

#[test]
fn request_pages_carry_page_info_and_the_rows() {
    let page: Paginated<MediaRequest> = parse(include_str!("../../tests/fixtures/requests.json"));
    assert_eq!(page.page_info.results, 3);
    assert_eq!(page.results.len(), 3);
    assert_eq!(page.results[0].request_status(), RequestStatus::Approved);
    assert_eq!(page.results[0].media_status(), MediaStatus::Processing);
    assert_eq!(
        page.results[0]
            .requested_by
            .as_ref()
            .map(User::display_name),
        Some("example-user")
    );
    assert_eq!(page.results[1].media.download_status.len(), 3);
    assert_eq!(
        page.results[2].media.jellyfin_media_id.as_deref(),
        Some("item-456")
    );
}

#[test]
fn a_season_pack_is_counted_once_by_download_id() {
    let page: Paginated<MediaRequest> = parse(include_str!("../../tests/fixtures/requests.json"));
    let downloads = &page.results[1].media.download_status;
    assert_eq!(downloads.len(), 3, "three episode rows");
    let (total, left, percent) = aggregate(downloads);
    assert_eq!(total, 43_063_640_191, "the pack size, not 3 × the pack");
    assert_eq!(left, 0);
    assert_eq!(percent, Some(1.0));
    assert!(all_completed(downloads));
}

#[test]
fn a_downloading_movie_reports_the_byte_percentage() {
    let page: Paginated<MediaRequest> = parse(include_str!(
        "../../tests/fixtures/requests-downloading.json"
    ));
    let movie = &page.results[0].media.download_status[0];
    assert_eq!(movie.download_id, "movie-1");
    assert_eq!(movie.percent(), Some(0.6));
    assert!(!movie.is_completed());
}

#[test]
fn a_series_aggregates_by_unique_download_not_by_episode_row() {
    let page: Paginated<MediaRequest> = parse(include_str!(
        "../../tests/fixtures/requests-downloading.json"
    ));
    let downloads = &page.results[1].media.download_status;
    assert_eq!(downloads.len(), 3, "two rows of pack-2 and one of pack-3");
    let (total, left, percent) = aggregate(downloads);
    assert_eq!(total, 86_127_280_382, "pack-2 once plus pack-3 once");
    assert_eq!(left, 40_000_000_000);
    let percent = percent.expect("known sizes");
    assert!((percent - 0.5356).abs() < 0.001, "got {percent}");
    assert!(!all_completed(downloads));
}

#[test]
fn zero_or_impossible_sizes_never_produce_a_percentage() {
    let item = DownloadingItem {
        title: "unknown".into(),
        status: "downloading".into(),
        size: 0,
        size_left: 10,
        time_left: None,
        estimated_completion_time: None,
        download_id: "x".into(),
        media_type: None,
        external_id: None,
        episode: None,
    };
    assert_eq!(item.percent(), None);
    assert_eq!(aggregate(&[item]), (0, 10, None));
}

#[test]
fn media_and_request_status_numbers_are_pinned() {
    for (number, status) in [
        (1, MediaStatus::Unknown),
        (2, MediaStatus::Pending),
        (3, MediaStatus::Processing),
        (4, MediaStatus::PartiallyAvailable),
        (5, MediaStatus::Available),
        (6, MediaStatus::Blocklisted),
        (7, MediaStatus::Deleted),
        (99, MediaStatus::Unknown),
    ] {
        assert_eq!(MediaStatus::from_i64(number), status);
    }
    for (number, status) in [
        (1, RequestStatus::Pending),
        (2, RequestStatus::Approved),
        (3, RequestStatus::Declined),
        (4, RequestStatus::Failed),
        (5, RequestStatus::Completed),
        (99, RequestStatus::Unknown),
    ] {
        assert_eq!(RequestStatus::from_i64(number), status);
    }
    assert!(MediaStatus::Available.is_available());
    assert!(!MediaStatus::Processing.is_available());
}

#[test]
fn request_count_parses_and_active_is_pending_plus_processing() {
    let counts: RequestCounts = parse(include_str!("../../tests/fixtures/request-count.json"));
    assert_eq!(counts.total, 3);
    assert_eq!(counts.active(), 2);
}

#[test]
fn the_create_body_omits_seasons_for_a_movie_and_sorts_tv_seasons() {
    let movie = serde_json::to_value(CreateRequest::movie(438631)).expect("serialises");
    assert_eq!(
        movie,
        serde_json::json!({"mediaType": "movie", "mediaId": 438631})
    );

    let all = serde_json::to_value(CreateRequest::tv(196950, Seasons::all())).expect("serialises");
    assert_eq!(
        all,
        serde_json::json!({"mediaType": "tv", "mediaId": 196950, "seasons": "all"})
    );

    let chosen = serde_json::to_value(CreateRequest::tv(196950, Seasons::numbers([3, 1, 3, 2])))
        .expect("serialises");
    assert_eq!(
        chosen,
        serde_json::json!({"mediaType": "tv", "mediaId": 196950, "seasons": [1, 2, 3]})
    );
}

#[test]
fn an_episode_label_is_zero_padded() {
    let episode = EpisodeRef {
        season_number: 1,
        episode_number: 2,
        absolute_episode_number: None,
        id: None,
    };
    assert_eq!(episode.label(), "S01E02");
}
