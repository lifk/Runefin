//! LAN server discovery: the same UDP handshake Jellyfin's own clients use.
//!
//! A client broadcasts `who is JellyfinServer?` to port 7359 and every server on the subnet
//! answers with a small JSON document. It is the fastest way to fill the connect screen, and it
//! needs no dependency: `std::net::UdpSocket` and `serde_json` are already here.

use std::collections::HashSet;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// The port Jellyfin's discovery service listens on.
pub const DISCOVERY_PORT: u16 = 7359;

/// One answer from the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredServer {
    pub id: String,
    pub name: String,
    /// A URL that can be typed into the Server field as-is.
    pub address: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Reply {
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    endpoint_address: Option<String>,
}

/// Broadcast discovery on the LAN.
pub fn discover(timeout: Duration) -> Vec<DiscoveredServer> {
    discover_to(
        &[SocketAddr::from(([255, 255, 255, 255], DISCOVERY_PORT))],
        timeout,
    )
}

/// The same handshake against explicit targets, which is what the test drives.
pub fn discover_to(targets: &[SocketAddr], timeout: Duration) -> Vec<DiscoveredServer> {
    let Ok(socket) = UdpSocket::bind(("0.0.0.0", 0)) else {
        return Vec::new();
    };
    let _ = socket.set_broadcast(true);
    let _ = socket.set_read_timeout(Some(Duration::from_millis(120)));
    for target in targets {
        let _ = socket.send_to(b"who is JellyfinServer?", target);
    }
    let deadline = Instant::now() + timeout;
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    let mut buffer = [0u8; 4096];
    while Instant::now() < deadline {
        match socket.recv_from(&mut buffer) {
            Ok((len, from)) => {
                if let Some(server) = parse_reply(&buffer[..len], from)
                    && seen.insert((server.id.clone(), server.address.clone()))
                {
                    found.push(server);
                }
            }
            // A quiet subnet is normal: keep listening until the deadline, then answer with
            // whatever arrived.
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }
    }
    found.sort_by_key(|server| server.name.to_lowercase());
    found
}

fn parse_reply(bytes: &[u8], from: SocketAddr) -> Option<DiscoveredServer> {
    let reply: Reply = serde_json::from_slice(bytes).ok()?;
    let address = reply
        .address
        .or(reply.endpoint_address)
        .filter(|address| !address.trim().is_empty())
        .map(|address| address.trim().trim_end_matches('/').to_string())
        .unwrap_or_else(|| format!("http://{}:8096", from.ip()));
    Some(DiscoveredServer {
        id: reply.id.unwrap_or_else(|| address.clone()),
        name: reply
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| address.clone()),
        address,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    /// A tiny stand-in for Jellyfin's discovery service, on a loopback port.
    #[test]
    fn the_handshake_reads_name_and_address() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("a responder");
        let port = server.local_addr().expect("an address").port();
        let responder = std::thread::spawn(move || {
            let mut buffer = [0u8; 256];
            let Ok((len, from)) = server.recv_from(&mut buffer) else {
                return;
            };
            assert_eq!(&buffer[..len], b"who is JellyfinServer?");
            let _ = server.send_to(
                br#"{"Address":"http://192.0.2.49:8096","Id":"abc","Name":"example","EndpointAddress":null}"#,
                from,
            );
        });
        let targets = [SocketAddr::from((Ipv4Addr::LOCALHOST, port))];
        let found = discover_to(&targets, Duration::from_millis(800));
        responder.join().expect("the responder thread");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "example");
        assert_eq!(found[0].address, "http://192.0.2.49:8096");
        assert_eq!(found[0].id, "abc");
    }

    /// A diagnostic, not a gate: an empty answer means the server is on another subnet, has
    /// discovery disabled, or the network drops broadcasts. The loopback handshake above is the
    /// protocol test.
    #[test]
    #[ignore = "needs Jellyfin's UDP discovery service on this network"]
    fn the_lan_answers_the_broadcast() {
        let found = discover(Duration::from_millis(900));
        eprintln!("discovered on this network: {found:?}");
    }

    #[test]
    fn a_garbage_answer_is_ignored_and_a_missing_address_falls_back_to_the_sender() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("a responder");
        let port = server.local_addr().expect("an address").port();
        let responder = std::thread::spawn(move || {
            let mut buffer = [0u8; 256];
            let Ok((_, from)) = server.recv_from(&mut buffer) else {
                return;
            };
            let _ = server.send_to(b"not json", from);
            let _ = server.send_to(br#"{"Id":"xyz","Name":"attic"}"#, from);
        });
        let targets = [SocketAddr::from((Ipv4Addr::LOCALHOST, port))];
        let found = discover_to(&targets, Duration::from_millis(800));
        responder.join().expect("the responder thread");
        assert_eq!(found.len(), 1, "the garbage datagram produced no server");
        assert_eq!(found[0].name, "attic");
        assert!(
            found[0].address.starts_with("http://127.0.0.1:"),
            "the sender's address is the fallback: {}",
            found[0].address
        );
    }
}
