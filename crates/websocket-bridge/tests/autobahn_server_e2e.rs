// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The Autobahn|TestSuite against the server role of this crate's
//! connection layer.
//!
//! The suite's `fuzzingclient` (the container `crossbario/autobahn-testsuite`)
//! is the other side: for each of its cases it opens a connection to the
//! server under test, sends what the case is about — valid and invalid
//! frames, fragments, UTF-8, close codes, large messages — and judges what
//! comes back and how the connection ends. The server here is the echo
//! server the suite asks for, built the way the daemon builds its
//! connections: `handshake::parse_client_request` and
//! `build_server_response`, `permessage_deflate::accept_offers`, and
//! `connection::{Receiver, Sender}` in [`Role::Server`]. It echoes every
//! message, answers Ping and Close, and fails the connection with the
//! receiver's close code on invalid data. The verdict of each case is the
//! suite's (`index.json` of the run: `behavior` for what the server sent,
//! `behaviorClose` for how the connection closed), so a test here passes
//! when the suite says the behaviour is right, not when this crate does.
//!
//! The suite starts that mode from its command line only. The container of
//! the weekly job therefore runs a control script beside the fuzzing server
//! (ci/integration-weekly.yml, `.autobahn-service`): a request
//! `GET /run?agent=…&url=…&cases=…` on its port 9002 runs the fuzzing client
//! against the server at `url` and answers with the `index.json` of the
//! run. Its address is `ZERODDS_AUTOBAHN_FUZZINGCLIENT` (`host:port`,
//! default `127.0.0.1:9002`). The echo server listens on every interface;
//! the address the suite reaches it under is the local address of the
//! connection to the control port, or `ZERODDS_AUTOBAHN_ECHO_HOST`.
//!
//! The tests are `#[ignore]`d; the weekly job
//! `integration-bridges-vendor-autobahn` runs them with the container as its
//! service (ci/integration_scenarios.py). Without the suite they fail.

#![cfg(feature = "std")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use zerodds_websocket_bridge::connection::{Event, Limits, Receiver, Sender};
use zerodds_websocket_bridge::handshake::{
    build_server_response, parse_client_request, render_server_response,
};
use zerodds_websocket_bridge::masking::Role;
use zerodds_websocket_bridge::permessage_deflate::{accept_offers, render_accept};

/// The name the suite files this server's results under.
const AGENT: &str = "zerodds-server";

/// The largest opening handshake the echo server reads.
const MAX_REQUEST_HEAD: usize = 16 * 1024;

fn control() -> String {
    std::env::var("ZERODDS_AUTOBAHN_FUZZINGCLIENT").unwrap_or_else(|_| "127.0.0.1:9002".to_string())
}

/// One connection of the suite: the opening handshake as the daemon answers
/// it, then the echo server until the connection ends.
fn serve(mut stream: TcpStream) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    let mut head = Vec::new();
    let mut octet = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut octet) {
            Ok(1) if head.len() < MAX_REQUEST_HEAD => head.push(octet[0]),
            _ => return,
        }
    }
    let Ok(text) = std::str::from_utf8(&head) else {
        return;
    };
    let request = match parse_client_request(text) {
        Ok(request) => request,
        Err(e) => {
            let _ = stream.write_all(e.render_error_response().as_bytes());
            return;
        }
    };
    let mut response = build_server_response(&request);
    let mut receiver = Receiver::new(Role::Server, Limits::default());
    let mut sender = Sender::server();
    // RFC 7692 §7.1: the first offer this server serves is agreed to.
    if let Some(params) = accept_offers(&request.extensions) {
        response.extensions.push(render_accept(&params));
        receiver = receiver.with_permessage_deflate(&params);
        sender = sender.with_permessage_deflate(&params);
    }
    if stream
        .write_all(render_server_response(&response).as_bytes())
        .is_err()
    {
        return;
    }

    let mut buf = Vec::new();
    let mut chunk = vec![0u8; 16 * 1024];
    'connection: loop {
        let event = loop {
            match receiver.feed(&mut buf) {
                Ok(Some(event)) => break Ok(event),
                Ok(None) => {}
                Err(failure) => break Err(failure),
            }
            match stream.read(&mut chunk) {
                // The suite dropped the connection, which some of its
                // cases do.
                Ok(0) | Err(_) => break 'connection,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
        };
        let wire = match event {
            Ok(Event::Message(m)) => sender.message(m.is_text, &m.payload).ok(),
            Ok(Event::Ping(data)) => sender.pong(&data).ok(),
            Ok(Event::Pong(_)) => continue,
            Ok(Event::Close(body)) => {
                if let Ok(Some(wire)) = sender.close_received(body.as_ref()) {
                    let _ = stream.write_all(&wire);
                }
                break;
            }
            Err(failure) => {
                if let Some(wire) = sender.fail(&failure) {
                    let _ = stream.write_all(&wire);
                }
                break;
            }
        };
        match wire {
            Some(wire) if stream.write_all(&wire).is_ok() => {}
            _ => break,
        }
    }
    // RFC 6455 §7.1.1: the server closes the TCP connection first. It ends
    // its sending direction and reads what the client still sends, at most
    // two seconds, so the Close frame is not lost to a reset.
    let _ = stream.shutdown(Shutdown::Write);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    while matches!(stream.read(&mut chunk), Ok(n) if n > 0) {}
}

/// Starts the echo server on every interface and returns its port.
fn echo_server() -> u16 {
    let listener = TcpListener::bind("0.0.0.0:0").expect("bind the echo server");
    let port = listener.local_addr().expect("local_addr").port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || serve(stream));
        }
    });
    port
}

/// The suite's verdict on one case.
#[derive(Debug)]
struct Verdict {
    id: String,
    /// What the server sent.
    behavior: String,
    /// How the connection closed.
    behavior_close: String,
}

/// The string value of `key` in a flat JSON object.
fn json_string(json: &str, key: &str) -> String {
    let at = json
        .find(&format!("\"{key}\""))
        .unwrap_or_else(|| panic!("no {key} in {json}"));
    let rest = &json[at + key.len() + 2..];
    let start = rest.find('"').expect("a string value") + 1;
    let end = start + rest[start..].find('"').expect("the end of the string");
    rest[start..end].to_string()
}

/// The verdicts of an `index.json`:
/// `{"<agent>": {"<case id>": {"behavior": …, "behaviorClose": …, …}, …}}`.
/// The objects of the third level are the cases; the key before each is its
/// id. No string of the file holds a brace or an escaped quote (agent name,
/// case ids, verdicts, file names).
fn verdicts(index: &str) -> Vec<Verdict> {
    let mut found = Vec::new();
    let mut depth = 0usize;
    let mut case_start = 0usize;
    for (at, c) in index.char_indices() {
        match c {
            '{' => {
                depth += 1;
                if depth == 3 {
                    case_start = at;
                }
            }
            '}' => {
                if depth == 3 {
                    let object = &index[case_start..=at];
                    // The key of the object: the last string before it.
                    let before = index[..case_start].trim_end().trim_end_matches(':');
                    let before = before.trim_end().trim_end_matches('"');
                    let id = before.rsplit('"').next().expect("a case id").to_string();
                    found.push(Verdict {
                        id,
                        behavior: json_string(object, "behavior"),
                        behavior_close: json_string(object, "behaviorClose"),
                    });
                }
                depth = depth.checked_sub(1).expect("balanced braces");
            }
            _ => {}
        }
    }
    found
}

/// Has the suite's fuzzing client run the cases of `patterns` (`"1.*"`,
/// `"9.1.*"`) against the server at `port` and returns the `index.json` of
/// the run.
fn fuzz(patterns: &[&str], port: u16) -> String {
    let addr = control();
    // The container needs a moment after its start.
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut stream = loop {
        match TcpStream::connect(&addr) {
            Ok(s) => break s,
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(e) => panic!("no Autobahn fuzzingclient control port at {addr}: {e}"),
        }
    };
    // The address the suite reaches this process under: the one this
    // process reaches the suite from.
    let host = std::env::var("ZERODDS_AUTOBAHN_ECHO_HOST").unwrap_or_else(|_| {
        stream
            .local_addr()
            .expect("the local address")
            .ip()
            .to_string()
    });
    stream
        .set_read_timeout(Some(Duration::from_secs(300)))
        .expect("timeout");
    let request = format!(
        "GET /run?agent={AGENT}&url=ws://{host}:{port}&cases={} HTTP/1.0\r\nHost: {addr}\r\n\r\n",
        patterns.join(",")
    );
    stream.write_all(request.as_bytes()).expect("the request");
    let mut answer = String::new();
    stream
        .read_to_string(&mut answer)
        .unwrap_or_else(|e| panic!("the control port sent no whole answer: {e}"));
    let (head, body) = answer
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("no HTTP answer: {answer}"));
    let status = head.lines().next().unwrap_or_default();
    assert!(status.contains(" 200 "), "{status}: {body}");
    body.to_string()
}

/// The cases the suite itself files as `INFORMATIONAL`: they have no right
/// answer (a Close after the closing handshake, close codes of the ranges
/// 5000 and 65535). Every other case has to be `OK`.
const INFORMATIONAL: [&str; 3] = ["7.1.6", "7.13.1", "7.13.2"];

/// Runs the cases of `groups` (`"1"`: 1.x.y, `"9.1"`: 9.1.x) against the
/// echo server and asserts the suite's verdict on each: `OK` for what the
/// server sent and for how the connection closed, or `INFORMATIONAL` for
/// the cases of [`INFORMATIONAL`]. Everything else, `NON-STRICT`, `FAILED`,
/// `UNIMPLEMENTED`, `WRONG CODE` and `UNCLEAN` among it, fails the test.
fn run(groups: &[&str]) {
    let port = echo_server();
    let patterns: Vec<String> = groups.iter().map(|g| format!("{g}.*")).collect();
    let patterns: Vec<&str> = patterns.iter().map(String::as_str).collect();
    let index = fuzz(&patterns, port);
    let verdicts = verdicts(&index);
    // The suite ran cases of every group, and of no other.
    for group in groups {
        let prefix = format!("{group}.");
        assert!(
            verdicts.iter().any(|v| v.id.starts_with(&prefix)),
            "no case of {group} in the report: {index}"
        );
    }
    let mut wrong = Vec::new();
    for v in &verdicts {
        assert!(
            groups
                .iter()
                .any(|group| v.id.starts_with(&format!("{group}."))),
            "the report holds the case {} outside {groups:?}",
            v.id
        );
        let informational = INFORMATIONAL.contains(&v.id.as_str());
        let accepted = |verdict: &str| match verdict {
            "OK" => true,
            "INFORMATIONAL" => informational,
            _ => false,
        };
        if !accepted(&v.behavior) || !accepted(&v.behavior_close) {
            wrong.push(format!("{}: {} / {}", v.id, v.behavior, v.behavior_close));
        }
    }
    eprintln!(
        "autobahn, server role: {} cases, {} not accepted",
        verdicts.len(),
        wrong.len()
    );
    assert!(wrong.is_empty(), "{wrong:?}");
}

#[test]
fn the_verdicts_of_an_index_are_read_per_case() {
    let index = r#"{
   "zerodds-server": {
      "1.1.1": {
         "behavior": "OK",
         "behaviorClose": "OK",
         "duration": 2,
         "remoteCloseCode": 1000,
         "reportfile": "zerodds_server_case_1_1_1.json"
      },
      "7.1.6": {"behavior": "INFORMATIONAL", "behaviorClose": "UNCLEAN", "duration": 1,
                "remoteCloseCode": null, "reportfile": "zerodds_server_case_7_1_6.json"}
   }
}"#;
    let found = verdicts(index);
    assert_eq!(found.len(), 2);
    assert_eq!(
        (
            found[0].id.as_str(),
            found[0].behavior.as_str(),
            found[0].behavior_close.as_str()
        ),
        ("1.1.1", "OK", "OK")
    );
    assert_eq!(
        (
            found[1].id.as_str(),
            found[1].behavior.as_str(),
            found[1].behavior_close.as_str()
        ),
        ("7.1.6", "INFORMATIONAL", "UNCLEAN")
    );
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingclient; runs in integration-bridges-vendor-autobahn"]
fn ws_5_autobahn_server_1_framing_and_2_pings() {
    run(&["1", "2"]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingclient; runs in integration-bridges-vendor-autobahn"]
fn ws_5_2_autobahn_server_3_reserved_bits_and_4_opcodes() {
    run(&["3", "4"]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingclient; runs in integration-bridges-vendor-autobahn"]
fn ws_5_4_autobahn_server_5_fragmentation() {
    run(&["5"]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingclient; runs in integration-bridges-vendor-autobahn"]
fn ws_8_1_autobahn_server_6_utf8() {
    run(&["6"]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingclient; runs in integration-bridges-vendor-autobahn"]
fn ws_7_autobahn_server_7_close_handling() {
    run(&["7"]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingclient; runs in integration-bridges-vendor-autobahn"]
fn ws_10_4_autobahn_server_9_message_sizes_1_to_4() {
    run(&["9.1", "9.2", "9.3", "9.4"]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingclient; runs in integration-bridges-vendor-autobahn"]
fn ws_10_4_autobahn_server_9_message_sizes_5_to_8_and_10() {
    run(&["9.5", "9.6", "9.7", "9.8", "10"]);
}
