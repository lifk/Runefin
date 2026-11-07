//! The mpv handoff: spawn the player, speak its JSON IPC, and turn its events into the handful
//! of facts the session needs. Nothing here touches the UI thread; `net.rs` owns the reader
//! thread and `state.rs` owns the session whose reports go back to Jellyfin.
//!
//! On Wayland mpv cannot be embedded in our window (mpv #9654), so this is the separate-window
//! fallback described in `design.md`; the in-app DMA-BUF player is the M5a fork track.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::net::{Failure, Stage};

#[derive(Debug, Clone)]
pub struct PlayRequest {
    pub binary: String,
    pub url: String,
    pub start_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackEvent {
    Position(i64),
    Duration(i64),
    Paused(bool),
    Ended,
    Exited,
}

pub struct PlaybackHandle {
    child: Child,
    writer: UnixStream,
    pub reader: Option<UnixStream>,
    socket: PathBuf,
}

impl PlaybackHandle {
    /// Send one JSON command to mpv. Errors mean mpv has gone away; the reader thread will
    /// report the exit too.
    pub fn send(&mut self, value: serde_json::Value) -> bool {
        let mut line = value.to_string();
        line.push('\n');
        self.writer.write_all(line.as_bytes()).is_ok()
    }

    pub fn quit(&mut self) {
        let _ = self.send(serde_json::json!({ "command": ["quit"] }));
        let _ = self.child.kill();
    }
}

impl Drop for PlaybackHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// Spawn mpv, wait for its control socket, and subscribe to the properties the session tracks.
pub fn start(request: &PlayRequest) -> Result<PlaybackHandle, Failure> {
    let socket = std::env::temp_dir().join(format!(
        "runefin-mpv-{}-{}.sock",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_file(&socket);

    let mut command = Command::new(&request.binary);
    command
        .arg("--no-terminal")
        .arg("--really-quiet")
        .arg("--force-window=yes")
        .arg("--keep-open=no")
        .arg(format!("--input-ipc-server={}", socket.display()));
    if request.start_ms > 0 {
        command.arg(format!("--start={:.3}", request.start_ms as f64 / 1000.0));
    }
    command.arg(&request.url);
    command.stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = command.spawn().map_err(|error| Failure {
        stage: Stage::Player,
        message: if error.kind() == std::io::ErrorKind::NotFound {
            "mpv isn't installed. Install mpv to play this item.".into()
        } else {
            format!("mpv couldn't start: {error}")
        },
    })?;

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut stream = None;
    while Instant::now() < deadline {
        match UnixStream::connect(&socket) {
            Ok(connected) => {
                stream = Some(connected);
                break;
            }
            Err(_) => {
                if child.try_wait().ok().flatten().is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    let Some(stream) = stream else {
        let _ = child.kill();
        return Err(Failure {
            stage: Stage::Player,
            message: "mpv exited before opening its control socket.".into(),
        });
    };
    let mut writer = stream.try_clone().map_err(|error| Failure {
        stage: Stage::Player,
        message: format!("mpv control socket: {error}"),
    })?;

    for (id, property) in [
        (1, "time-pos"),
        (2, "duration"),
        (3, "pause"),
        (4, "eof-reached"),
    ] {
        let command = serde_json::json!({
            "command": ["observe_property", id, property],
        });
        let mut line = command.to_string();
        line.push('\n');
        if let Err(error) = writer.write_all(line.as_bytes()) {
            let _ = child.kill();
            return Err(Failure {
                stage: Stage::Player,
                message: format!("mpv control socket: {error}"),
            });
        }
    }

    Ok(PlaybackHandle {
        child,
        writer,
        reader: Some(stream),
        socket,
    })
}

/// One line of mpv's JSON IPC, or `None` for events the session does not care about.
pub fn parse_event(line: &str) -> Option<PlaybackEvent> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("event")?.as_str()? == "end-file" {
        return Some(PlaybackEvent::Ended);
    }
    if value.get("event")?.as_str()? != "property-change" {
        return None;
    }
    let name = value.get("name")?.as_str()?;
    let data = value.get("data")?;
    match name {
        "time-pos" => data
            .as_f64()
            .map(|seconds| PlaybackEvent::Position((seconds * 1000.0) as i64)),
        "duration" => data
            .as_f64()
            .map(|seconds| PlaybackEvent::Duration((seconds * 1000.0) as i64)),
        "pause" => data.as_bool().map(PlaybackEvent::Paused),
        "eof-reached" if data.as_bool() == Some(true) => Some(PlaybackEvent::Ended),
        _ => None,
    }
}

/// Read mpv's events until the socket closes. Runs on the handoff worker, never the UI thread.
pub fn watch(stream: UnixStream, mut emit: impl FnMut(PlaybackEvent)) {
    let reader = BufReader::new(stream);
    for line in reader.lines().map_while(Result::ok) {
        if let Some(event) = parse_event(&line) {
            emit(event);
        }
    }
    emit(PlaybackEvent::Exited);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::mpsc;

    #[test]
    fn a_fake_player_opens_its_socket_and_its_events_are_read() {
        let dir = std::env::temp_dir().join(format!("runefin-fake-mpv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fake player dir");
        let script = dir.join("fake-mpv.py");
        std::fs::write(
            &script,
            r#"#!/usr/bin/env python3
import json, os, socket, sys
path = next(a.split('=', 1)[1] for a in sys.argv if a.startswith('--input-ipc-server='))
try:
    os.unlink(path)
except FileNotFoundError:
    pass
server = socket.socket(socket.AF_UNIX)
server.bind(path)
server.listen(1)
conn, _ = server.accept()
f = conn.makefile('rwb', buffering=0)
def send(value):
    f.write((json.dumps(value) + '\n').encode())
send({"event": "property-change", "name": "duration", "data": 120.0})
send({"event": "property-change", "name": "time-pos", "data": 42.0})
send({"event": "property-change", "name": "pause", "data": False})
while True:
    line = f.readline()
    if not line or b'"quit"' in line:
        break
send({"event": "end-file", "reason": "eof"})
f.close()
conn.close()
server.close()
try:
    os.unlink(path)
except FileNotFoundError:
    pass
"#,
        )
        .expect("fake player script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("fake player executable");

        let request = PlayRequest {
            binary: script.to_string_lossy().into_owned(),
            url: "fake".into(),
            start_ms: 1000,
        };
        let mut handle = start(&request).expect("fake player starts");
        let reader = handle.reader.take().expect("the fake player has a reader");
        let (sender, receiver) = mpsc::channel();
        let reader_thread = std::thread::spawn(move || {
            watch(reader, |event| {
                let _ = sender.send(event);
            });
        });
        let mut events = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline
            && !(events.contains(&PlaybackEvent::Position(42_000))
                && events.contains(&PlaybackEvent::Duration(120_000)))
        {
            if let Ok(event) = receiver.recv_timeout(Duration::from_millis(100)) {
                events.push(event);
            }
        }
        assert!(
            events.contains(&PlaybackEvent::Position(42_000)),
            "{events:?}"
        );
        assert!(
            events.contains(&PlaybackEvent::Duration(120_000)),
            "{events:?}"
        );
        handle.quit();
        drop(handle);
        let _ = reader_thread.join();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn property_changes_become_the_few_events_the_session_needs() {
        assert_eq!(
            parse_event(r#"{"event":"property-change","name":"time-pos","data":42.5}"#),
            Some(PlaybackEvent::Position(42_500))
        );
        assert_eq!(
            parse_event(r#"{"event":"property-change","name":"pause","data":true}"#),
            Some(PlaybackEvent::Paused(true))
        );
        assert_eq!(
            parse_event(r#"{"event":"property-change","name":"eof-reached","data":true}"#),
            Some(PlaybackEvent::Ended)
        );
        assert_eq!(
            parse_event(r#"{"event":"end-file","reason":"eof"}"#),
            Some(PlaybackEvent::Ended)
        );
        assert_eq!(
            parse_event(r#"{"event":"property-change","name":"ignore","data":1}"#),
            None
        );
    }
}
