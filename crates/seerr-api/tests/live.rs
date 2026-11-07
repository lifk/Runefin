//! Opt-in live checks against a real Seerr/Jellyseerr instance.
//!
//! Run only when `SEERR_URL` and `SEERR_API_KEY` are set:
//!
//! ```sh
//! SEERR_URL=https://jellyseerr.example SEERR_API_KEY=... \
//!   cargo test -p seerr-api --test live -- --ignored --nocapture
//! ```
//!
//! Every test here is read-only. The write test belongs in S2, behind `SEERR_ALLOW_WRITE`.

use seerr_api::Client;

fn client() -> Client {
    let url = std::env::var("SEERR_URL").expect("SEERR_URL is set for the live test");
    let key = std::env::var("SEERR_API_KEY").expect("SEERR_API_KEY is set for the live test");
    Client::new(&url, key)
}

#[test]
#[ignore = "needs SEERR_URL and SEERR_API_KEY"]
fn a_live_seerr_answers_status_and_auth() {
    let client = client();
    let status = client.status().expect("status");
    assert!(!status.version.is_empty(), "the instance reports a version");
    let user = client.auth_me().expect("auth/me");
    assert!(
        !user.display_name().is_empty(),
        "the key resolves to a user"
    );
    eprintln!(
        "Seerr {} · {} · {} requests",
        status.version,
        user.display_name(),
        user.request_count
    );
}

#[test]
#[ignore = "needs SEERR_URL and SEERR_API_KEY"]
fn a_live_request_page_parses() {
    let page = client().requests("all", 0, 5).expect("requests");
    assert!(page.page_info.results >= 0);
    for request in &page.results {
        // The row has no title; this is the summary N+1, not a parse failure.
        assert!(request.id > 0);
        assert!(!request.media_type.is_empty());
    }
    if let Some(request) = page.results.first()
        && let Some(tmdb_id) = request.media.tmdb_id
    {
        let client = client();
        let summary = match request.media_type.as_str() {
            "movie" => client.movie(tmdb_id).map(seerr_api::MediaSummary::from),
            "tv" => client.tv(tmdb_id).map(seerr_api::MediaSummary::from),
            other => panic!("unexpected media type {other}"),
        }
        .expect("a request summary");
        assert!(!summary.title.is_empty(), "the summary carries a title");
        eprintln!("summary: {}", summary.title);
    }
    let processing = client().requests("processing", 0, 2).expect("processing");
    assert!(processing.results.len() <= 2);
    let pending = client().requests("pending", 0, 2).expect("pending");
    assert!(pending.results.len() <= 2);
    let counts = client().request_count().expect("request count");
    assert!(counts.total >= counts.active());
    eprintln!(
        "{} request rows, {} total, {} processing, {} pending",
        page.results.len(),
        page.page_info.results,
        processing.results.len(),
        pending.results.len()
    );
}

#[test]
#[ignore = "needs network access to the TMDB image CDN"]
fn a_live_tmdb_image_fetches() {
    let bytes =
        seerr_api::fetch_image("https://image.tmdb.org/t/p/w342/d5NXSklXo0qyIYkgV94XAgMIckC.jpg")
            .expect("a TMDB poster");
    assert!(bytes.len() > 1000);
    assert_eq!(&bytes[..2], &[0xFF, 0xD8], "JPEG magic");
    eprintln!("{} image bytes", bytes.len());
}

#[test]
#[ignore = "needs SEERR_URL and SEERR_API_KEY, plus SEERR_DUPLICATE_MOVIE_ID"]
fn a_live_duplicate_movie_is_a_409() {
    let Ok(tmdb_id) = std::env::var("SEERR_DUPLICATE_MOVIE_ID") else {
        eprintln!("SEERR_DUPLICATE_MOVIE_ID is not set; skipping the duplicate probe");
        return;
    };
    let tmdb_id = tmdb_id.parse::<u64>().expect("a numeric TMDB id");
    let result = client().create_request(&seerr_api::CreateRequest::movie(tmdb_id));
    match result {
        Err(seerr_api::Error::Status { code: 409, body }) => {
            assert!(
                body.contains("already"),
                "the 409 body names the duplicate: {body}"
            );
            eprintln!("duplicate probe returned 409: {body}");
        }
        other => panic!("expected 409, got {other:?}"),
    }
}

#[test]
#[ignore = "needs SEERR_URL and SEERR_API_KEY"]
fn a_live_catalogue_page_parses() {
    let page = client().discover_trending(1).expect("trending");
    assert!(!page.results.is_empty(), "trending returns rows");
    for item in page.results.iter().take(3) {
        assert!(!item.media_type.is_empty());
        assert!(!item.title().is_empty());
    }
    let search = client().search("dune", 1).expect("search");
    assert!(!search.results.is_empty(), "search returns rows");
    eprintln!(
        "{} trending rows, {} search rows",
        page.results.len(),
        search.results.len()
    );
}

#[test]
#[ignore = "needs SEERR_URL and SEERR_API_KEY"]
fn a_live_person_has_credits_and_a_film_has_a_cast() {
    let person = client().person(1683343).expect("person");
    assert!(person.name.is_some(), "the person has a name");
    let credits = client().person_credits(1683343).expect("combined credits");
    assert!(!credits.cast.is_empty(), "combined credits return cast");
    assert!(
        credits.cast.iter().any(|credit| credit.is_movie()),
        "a film credit is present"
    );
    let movie = client().movie(945961).expect("movie");
    assert!(
        !movie.credits.cast.is_empty(),
        "the movie detail carries its cast"
    );
    eprintln!(
        "{} person credits, {} film cast members",
        credits.cast.len(),
        movie.credits.cast.len()
    );
}
