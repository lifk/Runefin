//! Image bytes on disk and in Rune's decoder.
//!
//! Rune's `ImageCache` decodes on its own worker, but it does not fetch: on native, a URL is a
//! local path. The app therefore fetches the server's image URL on its image worker, keys the
//! file by the server's immutable image tag, and hands the bytes to `ImageCache::load`. A second
//! visit to a screen reads the disk and never touches the network.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use crate::net::ImageSource;

/// The blurred poster placeholder size from design §5. Small enough to decode cheaply and
/// upscale into a soft wash behind the full image while it fades in.
pub const PLACEHOLDER_WIDTH: u32 = 27;
pub const PLACEHOLDER_HEIGHT: u32 = 17;

/// The cache key for one image. The tag is what makes the server's bytes immutable, so a tag
/// change is a new file and a missing tag is not cached under the old one.
pub fn key(server_url: &str, item_id: &str, kind: &str, tag: Option<&str>, width: u32) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    server_url.hash(&mut hasher);
    item_id.hash(&mut hasher);
    tag.hash(&mut hasher);
    let tag = tag.map_or_else(|| "untagged".to_string(), |tag| format!("t{}", safe(tag)));
    format!(
        "{:016x}-{}-{kind}-{tag}-{width}",
        hasher.finish(),
        safe(item_id)
    )
}

/// `$XDG_CACHE_HOME/runefin/images`, or `~/.cache/runefin/images`.
pub fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("runefin").join("images"))
}

/// `$XDG_CACHE_HOME/runefin/images/<key>.img`, or `~/.cache/runefin/images/<key>.img`.
pub fn cache_path(key: &str) -> Option<PathBuf> {
    Some(cache_dir()?.join(format!("{key}.img")))
}

/// Remove every cached poster. Runs on the image worker, never the UI thread.
pub fn clear_cache() -> Result<usize, String> {
    let Some(dir) = cache_dir() else {
        return Err("no cache directory".into());
    };
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(format!("{}: {error}", dir.display())),
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "img")
            && std::fs::remove_file(entry.path()).is_ok()
        {
            removed += 1;
        }
    }
    Ok(removed)
}

/// Downscale fetched poster bytes to the 27×17 placeholder on the image worker. A decode
/// failure is not an image failure: the card keeps its initial-letter fallback.
pub fn placeholder(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use ::image::ImageDecoder;
    let error = |e: ::image::ImageError| e.to_string();
    let mut reader = ::image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = ::image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits.clone());
    let mut decoder = reader.into_decoder().map_err(error)?;
    limits.reserve(decoder.total_bytes()).map_err(error)?;
    decoder.set_limits(limits).map_err(error)?;
    let image = ::image::DynamicImage::from_decoder(decoder).map_err(error)?;
    let image = image.into_rgba8();
    let small = ::image::imageops::resize(
        &image,
        PLACEHOLDER_WIDTH,
        PLACEHOLDER_HEIGHT,
        ::image::imageops::FilterType::Triangle,
    );
    Ok(small.into_raw())
}

/// Disk first, then the source. The caller runs this on the image worker; nothing here blocks
/// the UI thread. Jellyfin images use the session client; TMDB posters are public.
pub fn load_or_fetch(
    source: &ImageSource,
    url: &str,
    fallback: Option<&str>,
    key: &str,
) -> Result<Vec<u8>, String> {
    let path = cache_path(key);
    if let Some(path) = &path
        && let Ok(bytes) = std::fs::read(path)
        && !bytes.is_empty()
    {
        return Ok(bytes);
    }
    let fetch = |url: &str| -> Result<Vec<u8>, String> {
        match source {
            ImageSource::Jellyfin(client) => client.fetch_bytes(url).map_err(|e| e.user_message()),
            ImageSource::Public => seerr_api::fetch_image(url),
        }
    };
    let mut bytes = fetch(url);
    if bytes.as_ref().is_ok_and(|bytes| bytes.is_empty())
        && let Some(fallback) = fallback
    {
        bytes = fetch(fallback);
    }
    let bytes = bytes?;
    if bytes.is_empty() {
        return Err("the server returned an empty image".into());
    }
    if let Some(path) = path {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // A failed cache write is not worth failing the image for; it will be fetched next time.
        let _ = std::fs::write(&path, &bytes);
        trim_cache(400);
    }
    Ok(bytes)
}

/// Keep the poster cache bounded. Named for what it does, not for a timer: it runs after a
/// write, on the image worker, and deletes the least recently modified files first.
fn trim_cache(max_entries: usize) {
    let Some(dir) = cache_dir() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "img")
        })
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect();
    if files.len() <= max_entries {
        return;
    }
    files.sort_by_key(|(modified, _)| *modified);
    let excess = files.len() - max_entries;
    for (_, path) in files.into_iter().take(excess) {
        let _ = std::fs::remove_file(path);
    }
}

fn safe(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_placeholder_is_a_tiny_rgba_image() {
        let image = ::image::RgbaImage::from_pixel(60, 90, ::image::Rgba([20, 40, 80, 255]));
        let mut png = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut png, ::image::ImageFormat::Png)
            .expect("encode png");
        let rgb = placeholder(png.get_ref()).expect("decode placeholder");
        assert_eq!(
            rgb.len(),
            (PLACEHOLDER_WIDTH * PLACEHOLDER_HEIGHT * 4) as usize
        );
        assert_eq!(&rgb[..4], &[20, 40, 80, 255]);
    }

    #[test]
    fn the_key_is_stable_and_separates_servers_items_and_tags() {
        let a = key("http://a", "item-1", "Primary", Some("tag-1"), 360);
        assert_eq!(a, key("http://a", "item-1", "Primary", Some("tag-1"), 360));
        assert_ne!(a, key("http://b", "item-1", "Primary", Some("tag-1"), 360));
        assert_ne!(a, key("http://a", "item-1", "Primary", Some("tag-2"), 360));
        assert_ne!(a, key("http://a", "item-1", "Primary", Some("tag-1"), 720));
    }

    #[test]
    fn a_key_without_a_tag_does_not_collide_with_a_tagged_one() {
        assert_ne!(
            key("http://a", "item-1", "Primary", None, 360),
            key("http://a", "item-1", "Primary", Some("no-tag"), 360)
        );
    }

    #[test]
    fn the_cache_path_ends_in_img_and_stays_under_the_cache_dir() {
        let path = cache_path("abc-1").expect("HOME is set in the test environment");
        assert!(
            path.ends_with("runefin/images/abc-1.img"),
            "{}",
            path.display()
        );
    }
}
