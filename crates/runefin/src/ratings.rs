//! External ratings. Jellyfin stores TMDb in `CommunityRating` and Rotten Tomatoes in
//! `CriticRating`; IMDb is not part of its schema, so the app optionally asks OMDb for it with
//! the IMDb id the server already knows. No key means no external request: the other two scores
//! still render and the IMDb chip becomes a link.

use jellyfin_api::Client;

/// `https://www.omdbapi.com/?i=tt…&apikey=…`, parsed down to `imdbRating`.
pub fn imdb_rating(client: &Client, key: &str, imdb_id: &str) -> Result<Option<f64>, String> {
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("The OMDb key has an unexpected shape.".into());
    }
    if !imdb_id.starts_with("tt") || !imdb_id.chars().all(|c| c == 't' || c.is_ascii_digit()) {
        return Err("The server's IMDb id has an unexpected shape.".into());
    }
    let url = format!("https://www.omdbapi.com/?i={imdb_id}&apikey={key}&plot=short&r=json");
    let bytes = client
        .fetch_bytes(&url)
        .map_err(|error| error.user_message())?;
    Ok(parse_imdb_rating(&bytes))
}

/// A small parser kept separate so the network test is not the only test of the response shape.
pub fn parse_imdb_rating(bytes: &[u8]) -> Option<f64> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if value.get("Response").and_then(|v| v.as_str()) == Some("False") {
        return None;
    }
    value
        .get("imdbRating")
        .and_then(|v| v.as_str())
        .filter(|rating| *rating != "N/A")
        .and_then(|rating| rating.parse::<f64>().ok())
        .filter(|rating| (0.0..=10.0).contains(rating))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_omdb_answer_becomes_a_score_or_no_score() {
        assert_eq!(
            parse_imdb_rating(br#"{"Response":"True","imdbRating":"8.3"}"#),
            Some(8.3)
        );
        assert_eq!(
            parse_imdb_rating(br#"{"Response":"True","imdbRating":"N/A"}"#),
            None
        );
        assert_eq!(
            parse_imdb_rating(br#"{"Response":"False","Error":"Invalid API key!"}"#),
            None
        );
        assert_eq!(parse_imdb_rating(b"not json"), None);
    }
}
