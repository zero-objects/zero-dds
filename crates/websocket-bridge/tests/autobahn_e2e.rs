// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The Autobahn|TestSuite against this crate's connection layer.
//!
//! The suite's `fuzzingserver` (the container `crossbario/autobahn-testsuite`,
//! whose default command is that mode on port 9001) is the other side: for
//! each of its cases it sends what the case is about — valid and invalid
//! frames, fragments, UTF-8, close codes, large messages,
//! `permessage-deflate` with each parameter — and judges what comes back.
//! The client here is the echo client the suite asks for, built from
//! `handshake`, `permessage_deflate` and `connection::{Receiver, Sender}`:
//! it echoes every message, answers Ping and Close, and fails the
//! connection with the receiver's close code on invalid data. The verdict
//! of each case is read back from the suite (`getCaseStatus`), so a test
//! here passes when the suite says the behaviour is right, not when this
//! crate does.
//!
//! The address of the suite is `ZERODDS_AUTOBAHN_FUZZINGSERVER`
//! (`host:port`, default `127.0.0.1:9001`). The tests are `#[ignore]`d;
//! the weekly jobs `integration-bridges-vendor-autobahn` (the cases 1 to 10
//! and 13), `integration-bridges-vendor-autobahn-deflate-a` (12.1, 12.2,
//! 12.4) and `integration-bridges-vendor-autobahn-deflate-b` (12.3, 12.5)
//! run them with the container as their service
//! (ci/integration_scenarios.py, ci/integration-weekly.yml). Without the
//! suite they fail.
//!
//! The cases 12 and 13 send a thousand messages each; in a test build
//! without optimizations the DEFLATE of this side makes a group of them
//! take minutes, so the jobs build the target in the cargo profile
//! `corpus` (a test build with optimizations). The tests are cut so that
//! each stays within the 25 s of a test: measured in that profile on the
//! Linux test host, the longest (12.3.16 alone) took 12.5 s, the 517 cases
//! together 456 s.

#![cfg(feature = "std")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use zerodds_websocket_bridge::connection::{Event, Limits, Receiver, Sender};
use zerodds_websocket_bridge::generate_masking_key;
use zerodds_websocket_bridge::handshake::{
    client_key, render_client_request_with_extensions, validate_server_response_with_extensions,
};
use zerodds_websocket_bridge::masking::Role;
use zerodds_websocket_bridge::permessage_deflate::{
    DeflateOffer, EXTENSION_NAME, accept_response, render_offer,
};

/// The name the suite files this client's results under.
const AGENT: &str = "zerodds";

fn suite() -> String {
    std::env::var("ZERODDS_AUTOBAHN_FUZZINGSERVER").unwrap_or_else(|_| "127.0.0.1:9001".to_string())
}

/// A connection to the suite after the opening handshake.
struct Connection {
    stream: TcpStream,
    receiver: Receiver,
    sender: Sender,
    buf: Vec<u8>,
}

impl Connection {
    /// Opens `path` on the suite, offering `permessage-deflate` with
    /// `client_max_window_bits` (so the suite may limit this client's
    /// window, which its cases 13.x do).
    fn open(path: &str) -> Self {
        let addr = suite();
        // The container needs a moment after its start.
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut stream = loop {
            match TcpStream::connect(&addr) {
                Ok(s) => break s,
                Err(e) if Instant::now() < deadline => {
                    let _ = e;
                    std::thread::sleep(Duration::from_millis(250));
                }
                Err(e) => panic!("no Autobahn fuzzingserver at {addr}: {e}"),
            }
        };
        stream.set_nodelay(true).expect("nodelay");
        stream
            .set_read_timeout(Some(Duration::from_secs(60)))
            .expect("timeout");

        let mut nonce = [0u8; 16];
        for chunk in nonce.chunks_mut(4) {
            chunk.copy_from_slice(&generate_masking_key().expect("os random source"));
        }
        let key = client_key(&nonce);
        let offer = DeflateOffer {
            client_max_window_bits: Some(None),
            ..DeflateOffer::default()
        };
        let offered = render_offer(&offer);
        let request =
            render_client_request_with_extensions(&addr, path, &key, &[], &[offered.as_str()]);
        stream.write_all(request.as_bytes()).expect("send request");
        let mut head = Vec::new();
        let mut octet = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            match stream.read(&mut octet) {
                Ok(1) => head.push(octet[0]),
                other => panic!("{path}: the handshake response ended early: {other:?}"),
            }
        }
        let head = String::from_utf8(head).expect("an HTTP head");
        let answer =
            validate_server_response_with_extensions(&head, &key, &[], false, &[offered.as_str()])
                .unwrap_or_else(|e| panic!("{path}: {e}: {head}"));
        let mut receiver = Receiver::new(Role::Client, Limits::default());
        let mut sender = Sender::client();
        if let Some(element) = answer
            .extensions
            .iter()
            .find(|e| e.to_ascii_lowercase().starts_with(EXTENSION_NAME))
        {
            let params = accept_response(&offer, element)
                .unwrap_or_else(|e| panic!("{path}: {e}: {element}"));
            receiver = receiver.with_permessage_deflate(&params);
            sender = sender.with_permessage_deflate(&params);
        }
        Self {
            stream,
            receiver,
            sender,
            buf: Vec::new(),
        }
    }

    /// The next event; `Ok(None)` when the suite closed the connection or
    /// sent nothing for the length of the read timeout.
    fn next(&mut self) -> Result<Option<Event>, zerodds_websocket_bridge::Failure> {
        let mut chunk = vec![0u8; 16 * 1024];
        loop {
            if let Some(event) = self.receiver.feed(&mut self.buf)? {
                return Ok(Some(event));
            }
            if self.receiver.is_closed() {
                return Ok(None);
            }
            match self.stream.read(&mut chunk) {
                Ok(0) | Err(_) => return Ok(None),
                Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
            }
        }
    }

    /// Reads until the suite closes the TCP connection (RFC 6455 §7.1.1:
    /// the server closes first), at most two seconds.
    fn linger(&mut self) {
        let _ = self.stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut chunk = [0u8; 4096];
        while matches!(self.stream.read(&mut chunk), Ok(n) if n > 0) {}
    }

    /// The echo client of the suite: every message back as it came, a Pong
    /// for a Ping, the closing handshake, and the receiver's Close frame
    /// for invalid data.
    fn echo(mut self) {
        loop {
            let wire = match self.next() {
                Ok(Some(Event::Message(m))) => self.sender.message(m.is_text, &m.payload).ok(),
                Ok(Some(Event::Ping(data))) => self.sender.pong(&data).ok(),
                Ok(Some(Event::Pong(_))) => continue,
                Ok(Some(Event::Close(body))) => {
                    if let Ok(Some(wire)) = self.sender.close_received(body.as_ref()) {
                        let _ = self.stream.write_all(&wire);
                    }
                    break;
                }
                Ok(None) => return,
                Err(failure) => {
                    if let Some(wire) = self.sender.fail(&failure) {
                        let _ = self.stream.write_all(&wire);
                    }
                    break;
                }
            };
            match wire {
                Some(wire) if self.stream.write_all(&wire).is_ok() => {}
                _ => break,
            }
        }
        self.linger();
    }

    /// The one text message a control path of the suite answers with.
    fn answer(mut self) -> String {
        let text = match self.next() {
            Ok(Some(Event::Message(m))) if m.is_text => {
                String::from_utf8(m.payload).expect("UTF-8")
            }
            other => panic!("the suite sent no answer: {other:?}"),
        };
        if let Ok(Some(Event::Close(body))) = self.next()
            && let Ok(Some(wire)) = self.sender.close_received(body.as_ref())
        {
            let _ = self.stream.write_all(&wire);
        }
        self.linger();
        text
    }
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

/// The cases a test runs: every case of a group of the suite (`"7"`:
/// 7.x.y, `"9.1"`: 9.1.x), or the cases `first..=last` of a subgroup
/// (`"12.2"`, 1, 8: 12.2.1 to 12.2.8).
struct Cases {
    group: &'static str,
    range: Option<(u32, u32)>,
}

const fn all(group: &'static str) -> Cases {
    Cases { group, range: None }
}

const fn cases(group: &'static str, first: u32, last: u32) -> Cases {
    Cases {
        group,
        range: Some((first, last)),
    }
}

impl Cases {
    fn holds(&self, id: &str) -> bool {
        let Some(rest) = id
            .strip_prefix(self.group)
            .and_then(|r| r.strip_prefix('.'))
        else {
            return false;
        };
        match self.range {
            None => true,
            Some((first, last)) => rest.parse::<u32>().is_ok_and(|n| first <= n && n <= last),
        }
    }
}

/// The cases the suite itself files as `INFORMATIONAL`: they have no right
/// answer (a Close after the closing handshake, close codes of the ranges
/// 5000 and 65535). Every other case has to be `OK`.
const INFORMATIONAL: [&str; 3] = ["7.1.6", "7.13.1", "7.13.2"];

fn case_id(case: usize) -> String {
    let info = Connection::open(&format!("/getCaseInfo?case={case}")).answer();
    json_string(&info, "id")
}

/// The id of every case, by its number less one. The suite numbers its
/// cases 1 to `count` and names each by an id (`getCaseInfo`); the list is
/// read once and kept in the target's temporary directory for the other
/// tests of the run, each of which is a process of its own. [`run`] checks
/// the id of each case it runs against the suite, so a list of another
/// suite is noticed and read again.
fn case_ids(count: usize, refresh: bool) -> Vec<String> {
    let file = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "autobahn-cases-{}-{count}.txt",
        suite().replace(|c: char| !c.is_ascii_alphanumeric(), "_")
    ));
    if !refresh && let Ok(text) = std::fs::read_to_string(&file) {
        let ids: Vec<String> = text.lines().map(str::to_string).collect();
        if ids.len() == count {
            return ids;
        }
    }
    let ids: Vec<String> = (1..=count).map(case_id).collect();
    // Written whole or not at all: another test may read it meanwhile.
    let part = file.with_extension(format!("{}.part", std::process::id()));
    if std::fs::write(&part, ids.join("\n")).is_ok() {
        let _ = std::fs::rename(&part, &file);
    }
    ids
}

/// Runs the cases of `select` and asserts the suite's verdict on each:
/// `OK`, or `INFORMATIONAL` for the cases of [`INFORMATIONAL`]. Everything
/// else, `NON-STRICT`, `FAILED` and `UNIMPLEMENTED` among it, fails the
/// test.
fn run(select: &[Cases]) {
    let count: usize = Connection::open("/getCaseCount")
        .answer()
        .trim()
        .parse()
        .expect("the case count");
    assert!(count >= 500, "the suite has {count} cases");
    let mut ids = case_ids(count, false);
    let chosen = |ids: &[String]| -> Vec<usize> {
        (1..=count)
            .filter(|case| select.iter().any(|s| s.holds(&ids[case - 1])))
            .collect()
    };
    if chosen(&ids)
        .iter()
        .any(|case| case_id(*case) != ids[case - 1])
    {
        ids = case_ids(count, true);
    }
    let chosen = chosen(&ids);
    // A range names its cases one by one; the suite has to have them all.
    for s in select {
        let found = chosen.iter().filter(|c| s.holds(&ids[**c - 1])).count();
        match s.range {
            Some((first, last)) => assert_eq!(
                found,
                usize::try_from(last - first + 1).expect("a case count"),
                "the cases {first} to {last} of {}",
                s.group
            ),
            None => assert!(found > 0, "no case of {} in the suite", s.group),
        }
    }
    let mut wrong = Vec::new();
    for case in &chosen {
        let id = &ids[case - 1];
        Connection::open(&format!("/runCase?case={case}&agent={AGENT}")).echo();
        let status =
            Connection::open(&format!("/getCaseStatus?case={case}&agent={AGENT}")).answer();
        let behavior = json_string(&status, "behavior");
        let accepted = match behavior.as_str() {
            "OK" => true,
            "INFORMATIONAL" => INFORMATIONAL.contains(&id.as_str()),
            _ => false,
        };
        if !accepted {
            wrong.push(format!("{id}: {behavior}"));
        }
    }
    // The report of the run, in the container under /reports/clients.
    let mut report = Connection::open(&format!("/updateReports?agent={AGENT}"));
    while let Ok(Some(_)) = report.next() {}
    eprintln!(
        "autobahn: {} cases, {} not accepted",
        chosen.len(),
        wrong.len()
    );
    assert!(wrong.is_empty(), "{wrong:?}");
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_5_autobahn_1_framing_and_2_pings() {
    run(&[all("1"), all("2")]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_5_2_autobahn_3_reserved_bits_and_4_opcodes() {
    run(&[all("3"), all("4")]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_5_4_autobahn_5_fragmentation() {
    run(&[all("5")]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_8_1_autobahn_6_utf8() {
    run(&[all("6")]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_7_autobahn_7_close_handling() {
    run(&[all("7")]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_10_4_autobahn_9_message_sizes_1_to_4() {
    run(&[all("9.1"), all("9.2"), all("9.3"), all("9.4")]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_10_4_autobahn_9_message_sizes_5_to_8_and_10() {
    run(&[all("9.5"), all("9.6"), all("9.7"), all("9.8"), all("10")]);
}

// The cases 12.x.y and 13.x.y: permessage-deflate (RFC 7692). 12.x: five
// kinds of payload with the default parameters; 13.x: seven combinations
// of window bits and context takeover. y is the message size, 16 octets
// to 128 KiB, a thousand messages each.

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-a"]
fn ws_9_autobahn_12_1_cases_1_to_10() {
    run(&[cases("12.1", 1, 10)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-a"]
fn ws_9_autobahn_12_1_cases_11_to_15() {
    run(&[cases("12.1", 11, 15)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-a"]
fn ws_9_autobahn_12_1_cases_16_to_18() {
    run(&[cases("12.1", 16, 18)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-a"]
fn ws_9_autobahn_12_2_cases_1_to_8() {
    run(&[cases("12.2", 1, 8)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-a"]
fn ws_9_autobahn_12_2_case_9() {
    run(&[cases("12.2", 9, 9)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-a"]
fn ws_9_autobahn_12_2_case_10() {
    run(&[cases("12.2", 10, 10)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-a"]
fn ws_9_autobahn_12_2_cases_11_to_13() {
    run(&[cases("12.2", 11, 13)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-a"]
fn ws_9_autobahn_12_2_case_14() {
    run(&[cases("12.2", 14, 14)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-a"]
fn ws_9_autobahn_12_2_case_15() {
    run(&[cases("12.2", 15, 15)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-a"]
fn ws_9_autobahn_12_2_case_16() {
    run(&[cases("12.2", 16, 16)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-a"]
fn ws_9_autobahn_12_2_case_17() {
    run(&[cases("12.2", 17, 17)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-a"]
fn ws_9_autobahn_12_2_case_18() {
    run(&[cases("12.2", 18, 18)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_3_cases_1_to_8() {
    run(&[cases("12.3", 1, 8)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_3_case_9() {
    run(&[cases("12.3", 9, 9)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_3_case_10() {
    run(&[cases("12.3", 10, 10)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_3_cases_11_to_13() {
    run(&[cases("12.3", 11, 13)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_3_case_14() {
    run(&[cases("12.3", 14, 14)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_3_case_15() {
    run(&[cases("12.3", 15, 15)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_3_case_16() {
    run(&[cases("12.3", 16, 16)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_3_case_17() {
    run(&[cases("12.3", 17, 17)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_3_case_18() {
    run(&[cases("12.3", 18, 18)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-a"]
fn ws_9_autobahn_12_4_cases_1_to_10() {
    run(&[cases("12.4", 1, 10)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-a"]
fn ws_9_autobahn_12_4_cases_11_to_15() {
    run(&[cases("12.4", 11, 15)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-a"]
fn ws_9_autobahn_12_4_cases_16_to_18() {
    run(&[cases("12.4", 16, 18)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_5_cases_1_to_8() {
    run(&[cases("12.5", 1, 8)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_5_case_9() {
    run(&[cases("12.5", 9, 9)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_5_case_10() {
    run(&[cases("12.5", 10, 10)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_5_cases_11_to_13() {
    run(&[cases("12.5", 11, 13)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_5_case_14() {
    run(&[cases("12.5", 14, 14)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_5_case_15() {
    run(&[cases("12.5", 15, 15)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_5_case_16() {
    run(&[cases("12.5", 16, 16)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_5_case_17() {
    run(&[cases("12.5", 17, 17)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn-deflate-b"]
fn ws_9_autobahn_12_5_case_18() {
    run(&[cases("12.5", 18, 18)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_1_cases_1_to_10() {
    run(&[cases("13.1", 1, 10)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_1_cases_11_to_15() {
    run(&[cases("13.1", 11, 15)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_1_cases_16_to_18() {
    run(&[cases("13.1", 16, 18)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_2_cases_1_to_10() {
    run(&[cases("13.2", 1, 10)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_2_cases_11_to_15() {
    run(&[cases("13.2", 11, 15)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_2_cases_16_to_18() {
    run(&[cases("13.2", 16, 18)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_3_cases_1_to_10() {
    run(&[cases("13.3", 1, 10)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_3_cases_11_to_15() {
    run(&[cases("13.3", 11, 15)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_3_cases_16_to_18() {
    run(&[cases("13.3", 16, 18)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_4_cases_1_to_10() {
    run(&[cases("13.4", 1, 10)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_4_cases_11_to_15() {
    run(&[cases("13.4", 11, 15)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_4_cases_16_to_18() {
    run(&[cases("13.4", 16, 18)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_5_cases_1_to_10() {
    run(&[cases("13.5", 1, 10)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_5_cases_11_to_15() {
    run(&[cases("13.5", 11, 15)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_5_cases_16_to_18() {
    run(&[cases("13.5", 16, 18)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_6_cases_1_to_10() {
    run(&[cases("13.6", 1, 10)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_6_cases_11_to_15() {
    run(&[cases("13.6", 11, 15)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_6_cases_16_to_18() {
    run(&[cases("13.6", 16, 18)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_7_cases_1_to_10() {
    run(&[cases("13.7", 1, 10)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_7_cases_11_to_15() {
    run(&[cases("13.7", 11, 15)]);
}

#[test]
#[ignore = "needs the Autobahn|TestSuite fuzzingserver; runs in integration-bridges-vendor-autobahn"]
fn ws_9_autobahn_13_7_cases_16_to_18() {
    run(&[cases("13.7", 16, 18)]);
}
