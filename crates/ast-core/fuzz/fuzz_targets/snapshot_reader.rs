// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Fuzz target for the snapshot reader.
//!
//! Spec §9 and §10: malformed input must produce a typed error, never a
//! panic. Every reader is driven over arbitrary bytes; the target fails
//! only if one of them panics or the reader walks past its own buffer.
#![no_main]

use libfuzzer_sys::fuzz_target;
use zerodds_ast_core::SnapshotReader;

fuzz_target!(|data: &[u8]| {
    // The header path: rejects anything that is not a real snapshot.
    let _ = SnapshotReader::with_header(data);

    // The raw path: drive every reader until the buffer is exhausted.
    // Each call must return, never panic, and must never advance the
    // offset past the input length.
    let mut reader = SnapshotReader::new(data);
    let len = data.len();
    loop {
        let before = reader.offset();
        assert!(before <= len, "reader advanced past its own buffer");

        if reader.remaining() == 0 {
            break;
        }

        let _ = reader.bool();
        let _ = reader.u8();
        let _ = reader.u16();
        let _ = reader.u32();
        let _ = reader.u64();
        let _ = reader.i64();
        let _ = reader.i128();
        let _ = reader.string();
        let _ = reader.byte_vec();
        let _ = reader.node_id();

        // A round that consumed nothing means every reader failed; the
        // buffer cannot make further progress and the loop would spin.
        if reader.offset() == before {
            break;
        }
    }
});
