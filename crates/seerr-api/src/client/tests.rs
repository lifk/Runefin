use super::*;

#[test]
fn a_bare_host_gets_a_scheme_and_no_trailing_slash() {
    assert_eq!(
        normalise_base_url(" 198.51.100.2:5055/ "),
        "http://198.51.100.2:5055"
    );
    assert_eq!(
        normalise_base_url("https://jellyseerr.example/"),
        "https://jellyseerr.example"
    );
    assert_eq!(
        normalise_base_url("https://example.test/seerr/"),
        "https://example.test/seerr"
    );
}

#[test]
fn a_client_builds_the_api_path_under_any_base_path() {
    let client = Client::new("https://example.test/seerr/", "key");
    assert_eq!(client.base_url(), "https://example.test/seerr");
    assert_eq!(
        client.url("/api/v1/status"),
        "https://example.test/seerr/api/v1/status"
    );
}

#[test]
fn the_optional_user_header_is_stored() {
    let client = Client::new("https://example.test", "key").with_user(7);
    assert_eq!(client.user_id, Some(7));
}

#[test]
fn queries_are_percent_encoded() {
    assert_eq!(urlencode("dune part two"), "dune%20part%20two");
    assert_eq!(urlencode("a&b=c"), "a%26b%3Dc");
    assert_eq!(urlencode("safe-._~"), "safe-._~");
}

#[test]
fn a_wrong_key_is_a_403_and_a_missing_key_is_a_401() {
    let forbidden: Error = Error::Status {
        code: 403,
        body: include_str!("../../tests/fixtures/error-403.json").to_string(),
    };
    assert_eq!(
        forbidden.user_message(),
        "You do not have permission to access this endpoint"
    );
    assert!(!forbidden.is_transient());
    assert!(!forbidden.is_auth());

    let missing: Error = Error::Status {
        code: 401,
        body: include_str!("../../tests/fixtures/error-401.json").to_string(),
    };
    assert_eq!(missing.user_message(), "Seerr needs an API key.");
    assert!(missing.is_auth());
}

#[test]
fn a_missing_title_keeps_the_server_message() {
    let error: Error = Error::Status {
        code: 500,
        body: include_str!("../../tests/fixtures/error-500.json").to_string(),
    };
    assert_eq!(error.user_message(), "Unable to retrieve movie.");
    assert!(error.is_transient());
}

#[test]
fn a_duplicate_request_is_a_normal_answer() {
    let error = Error::Status {
        code: 409,
        body: "{}".into(),
    };
    assert_eq!(error.user_message(), "Already requested.");
    assert!(!error.is_transient());
}

#[test]
fn transport_and_decode_errors_have_distinct_messages() {
    assert_eq!(
        Error::Transport("dns".into()).user_message(),
        "Can't reach Seerr."
    );
    assert_eq!(
        Error::Decode("bad json".into()).user_message(),
        "Seerr sent something unexpected."
    );
    assert!(Error::Transport("dns".into()).is_transient());
}

#[test]
fn the_server_message_falls_back_from_message_to_error() {
    assert_eq!(
        server_message(r#"{"message":"nope"}"#).as_deref(),
        Some("nope")
    );
    assert_eq!(
        server_message(r#"{"error":"refused"}"#).as_deref(),
        Some("refused")
    );
    assert_eq!(server_message("not json"), None);
}
