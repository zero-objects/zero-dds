// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Deep example (async): the same sensor-telemetry flow, but the subscriber does
//! not own the run-loop. `AsyncReader` spawns a std thread that forwards decoded
//! bodies over an `mpsc` channel; the consumer blocks on `recv` — the idiomatic
//! std concurrency model (no async runtime). Every field is decoded. A watchdog
//! ends the run with an error when the samples do not arrive within
//! [`TIMEOUT`], instead of an endless wait in `recv`.
//!
//! Run: `cargo run -p zerodds-endpoint-rust --example example_async`

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::expect_used)]

use std::sync::mpsc;
use std::time::Duration;

use zerodds_endpoint_rust::{
    AsyncReader, Client, MemTransport, OBJK_DATAWRITER, Reading, xrce_object_id,
};
use zerodds_wire::Endianness;

/// How long the subscriber waits for all samples.
const TIMEOUT: Duration = Duration::from_secs(10);

fn main() {
    let total: u32 = 5;
    let transport = MemTransport::new();
    let mut client = Client::new(transport.clone(), xrce_object_id(0x001, OBJK_DATAWRITER));
    for i in 0..total {
        let r = Reading {
            id: 0x2000 + i,
            value: 100.0 - (i as f32),
            label: format!("sensor-{i:02}"),
        };
        client
            .write(&r.marshal(Endianness::Little))
            .expect("sample fits the 16-bit submessage_length");
    }

    let (done, finished) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        if finished.recv_timeout(TIMEOUT) == Err(mpsc::RecvTimeoutError::Timeout) {
            eprintln!("incomplete: not all {total} samples within {TIMEOUT:?}");
            std::process::exit(1);
        }
    });

    let reader = AsyncReader::start(transport);
    for got in 0..total {
        let body = reader.recv();
        let r = Reading::decode(&body);
        println!(
            "async reading {got}: id=0x{:x} value={:.1} label=\"{}\"",
            r.id, r.value, r.label
        );
    }
    reader.stop();
    let _ = done.send(());
    println!("ALL OK");
}
