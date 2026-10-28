// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Fuzz target for the model snapshot loader.
//!
//! Spec §9 and §10: arbitrary bytes must yield a typed error, never a
//! panic. A snapshot is untrusted input — it crosses process boundaries
//! and lands in a build cache — so the loader is held to the same bar as
//! a wire decoder.
#![no_main]

use libfuzzer_sys::fuzz_target;
use zerodds_ast::Ast;

fuzz_target!(|data: &[u8]| {
    // Whatever the bytes are, this returns.
    if let Ok(model) = Ast::from_snapshot(data) {
        // A model that loaded must also be usable: re-serialising it
        // must not panic, and the result must load again.
        if let Ok(bytes) = model.to_snapshot() {
            let reloaded = Ast::from_snapshot(&bytes);
            assert!(
                reloaded.is_ok(),
                "a model that serialised did not load back"
            );
        }
    }
});
