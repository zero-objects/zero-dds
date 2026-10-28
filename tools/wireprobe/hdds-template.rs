// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The program `wireprobe-hdds` builds around one sample.
//!
//! `hddsgen gen rust` writes the type; `wireprobe-fill --vendor hdds`
//! writes the expression that constructs it. Nothing here decides any
//! wire question — the octets come from the generated `encode_*`
//! method, which is HDDS's own.

#![allow(dead_code, unused_imports, clippy::all)]

include!("generated.rs");

/// One encode attempt, by method name.
///
/// `max_*_size` is HDDS's own upper bound for the type. Where it is
/// absent for a shape, a fixed buffer would silently truncate, so the
/// encode is retried at a larger size instead and only a genuine
/// refusal is reported.
macro_rules! attempt {
    ($sample:expr, $method:ident) => {{
        let mut capacity = 4096usize;
        loop {
            let mut buffer = vec![0u8; capacity];
            match $sample.$method(&mut buffer) {
                Ok(written) => {
                    let mut hex = String::with_capacity(written * 2);
                    for byte in &buffer[..written] {
                        hex.push_str(&format!("{byte:02x}"));
                    }
                    break Ok(hex);
                }
                Err(error) => {
                    let message = format!("{error:?}");
                    if message.contains("BufferTooSmall") && capacity < (1 << 24) {
                        capacity *= 4;
                        continue;
                    }
                    break Err(message);
                }
            }
        }
    }};
}

fn main() {
    let sample = include!("fill.inc");
    // `drive.inc` is written by the driver: either one `attempt!` whose
    // hex goes to standard output, or the four `--matrix` rows. This
    // file names no encoding of its own.
    include!("drive.inc")
}
