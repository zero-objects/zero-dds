// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `wireprobe-zerodds-c-gen` — the command around
//! [`zerodds_wireprobe_c_gen`].
//!
//! The generation itself is the library's, because the batch driver
//! `wireprobe-c-matrix` calls it a hundred thousand times and a process
//! launch per call costs more than the call.

fn main() {
    zerodds_wireprobe_c_gen::cli_main();
}
