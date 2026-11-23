// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The output of a broker process the vendor tests start (`#[path]`
//! include of `rabbitmq.rs`, and of `brokerj.rs` and `artemis.rs` in
//! `zerodds-amqp-endpoint`).

use std::io::{BufRead, BufReader};
use std::process::Child;
use std::sync::mpsc;

/// The lines `child` writes on stdout and on stderr, in one channel: a
/// thread per stream reads it until the child closes it. The streams are
/// taken from `child`.
pub fn child_lines(child: &mut Child) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel::<String>();
    for stream in [
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
    }
    rx
}
