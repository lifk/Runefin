//! Opt-in performance probe against a real Jellyfin server.
//!
//! ```sh
//! JELLYFIN_URL=... JELLYFIN_USER_ID=... JELLYFIN_TOKEN=... \
//!   cargo test -p jellyfin-api --test perf -- --ignored --nocapture
//! ```
//!
//! Prints median / p95 / min / max milliseconds per call and payload sizes. Times include the
//! reverse proxy and TLS because that is what the app actually pays; keep-alive is reused by the
//! client between runs, as in the app.

use std::time::{Duration, Instant};

use jellyfin_api::{Client, ImageKind, ItemQuery};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} for the perf probe"))
}

fn bench<T>(label: &str, runs: usize, mut f: impl FnMut() -> T) -> T {
    // A warm-up hides one-time TLS and DNS from the reported numbers.
    let _ = f();
    let mut times = Vec::with_capacity(runs);
    let mut last = None;
    for _ in 0..runs {
        let started = Instant::now();
        last = Some(f());
        times.push(started.elapsed());
    }
    times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let median = ms(times[times.len() / 2]);
    let p95 = ms(times[((times.len() as f64 * 0.95).ceil() as usize - 1).min(times.len() - 1)]);
    eprintln!(
        "{label:<28} median {median:7.1} ms  p95 {p95:7.1} ms  min {:7.1}  max {:7.1}",
        ms(times[0]),
        ms(*times.last().expect("runs > 0"))
    );
    last.expect("runs > 0")
}
#[test]
#[ignore = "needs a real Jellyfin server; set JELLYFIN_URL, JELLYFIN_USER_ID and JELLYFIN_TOKEN"]
fn live_api_performance() {
    let base = env("JELLYFIN_URL");
    let user = env("JELLYFIN_USER_ID");
    let token = env("JELLYFIN_TOKEN");
    let client = Client::new(&base, "runefin-perf").with_session(token, user);

    let libraries = bench("libraries", 7, || client.libraries().expect("libraries"));
    eprintln!("libraries                   {} rows", libraries.len());
    let resume = bench("resume", 7, || client.resume(24).expect("resume"));
    eprintln!("resume                      {} rows", resume.items.len());
    let latest = bench("latest (first library)", 7, || {
        client
            .latest(libraries.first().map(|library| library.id.as_str()), 12)
            .expect("latest")
    });
    eprintln!("latest                      {} rows", latest.len());
    let next_up = bench("next up", 7, || client.next_up_all(12).expect("next up"));
    eprintln!("next up                     {} rows", next_up.len());

    let sample = resume
        .items
        .first()
        .or_else(|| latest.first())
        .expect("a resume or latest item");
    let detail = bench("item detail", 7, || {
        client.item(&sample.id).expect("detail")
    });
    eprintln!(
        "detail                      {} chapters, {} people, {} sources",
        detail.chapters.len(),
        detail.people.len(),
        detail.media_sources.len()
    );
    bench("search", 5, || {
        client.items(&ItemQuery::search("the")).expect("search")
    });

    let poster_id = detail
        .series_id
        .as_deref()
        .unwrap_or(&detail.id)
        .to_string();
    let poster_tag = detail
        .series_primary_image_tag
        .as_deref()
        .or_else(|| detail.image_tag(ImageKind::Primary))
        .expect("a poster tag");
    let poster_url = client.image_url(&poster_id, ImageKind::Primary, 360, Some(poster_tag), None);
    let poster = bench("poster 360", 5, || {
        client.fetch_bytes(&poster_url).expect("poster")
    });
    eprintln!("poster bytes                {} KB", poster.len() / 1024);

    if let Some(tag) = detail.image_tag(ImageKind::Backdrop) {
        let url = client.image_url(&detail.id, ImageKind::Backdrop, 1280, Some(tag), None);
        let bytes = bench("backdrop 1280", 3, || {
            client.fetch_bytes(&url).expect("backdrop")
        });
        eprintln!("backdrop bytes              {} KB", bytes.len() / 1024);
    }

    if let Some((width, info)) = detail.trickplay_manifest() {
        let tile = info.tile_of(info.thumbnail_at(60_000));
        if let Some(url) = client.trickplay_url(&detail.id, width, tile) {
            let bytes = bench("trickplay tile", 3, || {
                client.fetch_bytes(&url).expect("trickplay")
            });
            eprintln!(
                "trickplay                   {width}px {tile} tile, {} KB",
                bytes.len() / 1024
            );
        }
    }

    // Prefer an external text track: the Frieren ASS track converts to a 24 MB SRT, which is a
    // real but unrepresentative subtitle payload for a first track.
    let mut subtitle_job = None;
    for item in resume.items.iter().take(8) {
        let Ok(detail) = client.item(&item.id) else {
            continue;
        };
        let Some(source) = detail
            .primary_media_source()
            .map(|source| source.id.clone())
        else {
            continue;
        };
        if let Some(track) = detail
            .subtitle_tracks()
            .into_iter()
            .find(|track| track.is_external)
            .or_else(|| detail.subtitle_tracks().into_iter().next())
            && let Some(url) = client.subtitle_url(&detail.id, &source, track.index)
        {
            subtitle_job = Some((url, track.display_title));
            break;
        }
    }
    if let Some((url, title)) = subtitle_job {
        let bytes = bench("subtitle SRT", 3, || {
            client.fetch_bytes(&url).expect("subtitle")
        });
        eprintln!(
            "subtitle                    {title}, {} KB",
            bytes.len() / 1024
        );
    }

    let channels = bench("live channels", 5, || {
        client.live_channels(120).expect("channels")
    });
    eprintln!("channels                    {} rows", channels.len());
    if !channels.is_empty() {
        let ids: Vec<String> = channels.iter().map(|channel| channel.id.clone()).collect();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        bench("live guide window", 3, || {
            client
                .live_guide(&ids, now - 3600, now + 3 * 3600)
                .expect("guide")
        });
        bench("live open + close", 3, || {
            let stream = client
                .open_live_stream(&channels[0].id)
                .expect("open channel");
            if let Some(id) = &stream.live_stream_id {
                client.close_live_stream(id).expect("close channel");
            }
        });
    }
}
