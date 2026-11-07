//! YouTube trailers without a web view. `yt-dlp` (if installed) downloads a combined 1080p
//! MP4 into the trailer cache; the in-app player opens that file, so playback stays in the
//! same window and the video path. Without `yt-dlp` or `ffmpeg`, the caller falls back to the
//! browser. Nothing here runs on the UI thread.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Only provider URLs that `yt-dlp` understands and that the app can treat as a trailer.
pub fn is_youtube(url: &str) -> bool {
    url.starts_with("https://www.youtube.com/")
        || url.starts_with("https://youtube.com/")
        || url.starts_with("https://youtu.be/")
}

/// `$XDG_CACHE_HOME/runefin/trailers`, or `~/.cache/runefin/trailers`.
pub fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("runefin").join("trailers"))
}

/// A stable file name per provider URL, so a second play is instant.
pub fn cache_path(url: &str) -> Option<PathBuf> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    Some(cache_dir()?.join(format!("{:016x}.mp4", hasher.finish())))
}

/// Download a YouTube trailer, or return the cached file. The first call on a new trailer takes
/// a few seconds; it runs on a worker.
pub fn download(url: &str) -> Result<PathBuf, String> {
    if !is_youtube(url) {
        return Err("This trailer is not a YouTube URL.".into());
    }
    let path = cache_path(url).ok_or("No cache directory for trailers.")?;
    if let Ok(meta) = std::fs::metadata(&path)
        && meta.len() > 0
    {
        return Ok(path);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("Could not create the trailer cache: {error}"))?;
    }
    let temporary = path.with_extension("part.mp4");
    let _ = std::fs::remove_file(&temporary);
    let output = Command::new("yt-dlp")
        .args([
            "--no-playlist",
            "--no-warnings",
            "--merge-output-format",
            "mp4",
            "-f",
            "bestvideo[height<=1080]+bestaudio/best[height<=1080]/best",
            "-o",
        ])
        .arg(&temporary)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                "yt-dlp is not installed. Install it to watch YouTube trailers in the app."
                    .to_string()
            } else {
                format!("yt-dlp could not start: {error}")
            }
        })?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&temporary);
        let message = String::from_utf8_lossy(&output.stderr);
        let line = message.lines().last().unwrap_or("no detail").trim();
        return Err(format!("yt-dlp could not download this trailer: {line}"));
    }
    // `-o` names the output exactly; some yt-dlp versions still append the merged extension.
    let produced = if temporary.exists() {
        temporary
    } else {
        path.with_extension("part.mp4")
    };
    if !produced.exists() {
        return Err("yt-dlp finished without a trailer file.".into());
    }
    std::fs::rename(&produced, &path)
        .map_err(|error| format!("Could not finish the trailer download: {error}"))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_provider_urls_take_the_downloader_path() {
        assert!(is_youtube("https://www.youtube.com/watch?v=abc"));
        assert!(is_youtube("https://youtu.be/abc"));
        assert!(!is_youtube("https://example.com/trailer.mp4"));
        assert!(!is_youtube("file:///tmp/trailer.mp4"));
    }

    #[test]
    #[ignore = "needs yt-dlp in PATH and network; downloads a real trailer"]
    fn a_youtube_trailer_downloads_into_the_cache() {
        let path = download("https://www.youtube.com/watch?v=mWEhfF27O0c").expect("trailer");
        let size = std::fs::metadata(&path).expect("file").len();
        assert!(size > 1_000_000, "trailer is {size} bytes");
        eprintln!("trailer: {} ({} bytes)", path.display(), size);
    }

    #[test]
    #[ignore = "needs the cached trailer file and a GPU"]
    fn a_cached_trailer_starts_quickly() {
        let path = cache_path("https://www.youtube.com/watch?v=mWEhfF27O0c").expect("cache");
        assert!(
            path.exists(),
            "run the download test first: {}",
            path.display()
        );
        let surface = rune::NativeSurface::new();
        let started = std::time::Instant::now();
        let player =
            crate::player::Player::start(&format!("file://{}", path.display()), surface, None)
                .expect("the cached trailer plays");
        let elapsed = started.elapsed();
        eprintln!("trailer start: {} ms", elapsed.as_millis());
        player.stop();
        assert!(
            elapsed.as_secs_f64() < 2.0,
            "trailer start took {elapsed:?}"
        );
    }

    #[test]
    fn the_cache_name_is_stable_per_url() {
        let a = cache_path("https://youtu.be/abc").expect("cache");
        let b = cache_path("https://youtu.be/abc").expect("cache");
        let c = cache_path("https://youtu.be/def").expect("cache");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.extension().and_then(|e| e.to_str()), Some("mp4"));
    }
}
