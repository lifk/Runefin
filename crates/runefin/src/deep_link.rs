//! `runefin://` links: the app's one external entry point.
//!
//! The system opens a link by starting the app with it as an argument (and `single_instance`
//! hands a later launch's link to the running process, which is what makes a media remote
//! useful). Keep the grammar deliberately small:
//!
//! * `runefin://item/<id>` opens the item's page.
//! * `runefin://play/<id>` starts the item, or opens its page when it is not playable (a series
//!   or a folder).
//!
//! IDs are Jellyfin's GUIDs. Anything else — another scheme, host, path shape, query that would
//! make an ID ambiguous — is refused rather than guessed at.

/// A parsed link. The ID is a Jellyfin item id, never a URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeepLink {
    /// Show the item's page.
    Item(String),
    /// Start the item, or show its page when it has no playable media source.
    Play(String),
}

impl DeepLink {
    /// Parse a link from the system. `None` for anything outside the grammar above.
    pub fn parse(url: &str) -> Option<Self> {
        let rest = url.strip_prefix("runefin://")?;
        let (host, path) = rest.split_once('/')?;
        let id = path.split(['?', '#']).next()?.trim();
        if !valid_id(id) {
            return None;
        }
        match host {
            "item" => Some(Self::Item(id.to_owned())),
            "play" => Some(Self::Play(id.to_owned())),
            _ => None,
        }
    }
}

/// A Jellyfin id: hex digits and hyphens, short enough to be an id and not a path.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_and_play_links_parse() {
        let id = "8f2a1c4e5b6d7f8091a2b3c4d5e6f708";
        assert_eq!(
            DeepLink::parse(&format!("runefin://item/{id}")),
            Some(DeepLink::Item(id.into()))
        );
        assert_eq!(
            DeepLink::parse(&format!("runefin://play/{id}")),
            Some(DeepLink::Play(id.into()))
        );
    }

    #[test]
    fn a_query_or_fragment_after_the_id_is_not_part_of_it() {
        let id = "abcdef0123456789";
        assert_eq!(
            DeepLink::parse(&format!("runefin://play/{id}?from=decky")),
            Some(DeepLink::Play(id.into()))
        );
        assert_eq!(
            DeepLink::parse(&format!("runefin://item/{id}#top")),
            Some(DeepLink::Item(id.into()))
        );
    }

    #[test]
    fn only_the_documented_hosts_and_ids_are_accepted() {
        for url in [
            "",
            "https://example.test/runefin://play/abc",
            "runefin:/play/abc",
            "runefin://play",
            "runefin://play/",
            "runefin://play/../etc/passwd",
            "runefin://play/abc/def",
            "runefin://play/%2e%2e",
            "runefin://open/abc",
            "RUNEFIN://play/abc",
            "runefin://play/xyz",
        ] {
            assert_eq!(DeepLink::parse(url), None, "{url:?} must not parse");
        }
    }

    #[test]
    fn hyphens_and_uppercase_hex_are_valid_ids() {
        assert_eq!(
            DeepLink::parse("runefin://play/8F2A1C4E-5B6D-7F80-91A2-B3C4D5E6F708"),
            Some(DeepLink::Play(
                "8F2A1C4E-5B6D-7F80-91A2-B3C4D5E6F708".into()
            ))
        );
    }
}
