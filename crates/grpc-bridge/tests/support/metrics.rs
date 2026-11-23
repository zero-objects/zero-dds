// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The Prometheus page of a running daemon (spec §8.2), for the e2e tests
// that read it (`#[path]` include).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// The response to `GET /metrics` on the admin port `port`, head and body.
pub fn scrape(port: u16) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(b"GET /metrics HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut body = String::new();
    let _ = s.read_to_string(&mut body);
    body
}

/// The value of the sample line that starts with `series`.
pub fn metric(body: &str, series: &str) -> Option<f64> {
    body.lines()
        .find(|l| l.starts_with(series))
        .and_then(|l| l.rsplit(' ').next())
        .and_then(|v| v.parse().ok())
}
