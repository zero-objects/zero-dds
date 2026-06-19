// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Spec §11.3 — zero heap allocation in the zero-copy write hot path.
//!
//! Uses `dhat` as the global allocator (this test binary only) to count heap
//! blocks allocated during a steady-state run of `FlatWriter::write` (reserve +
//! commit into a pre-allocated slot). The claim: a `write_flat` does no heap
//! allocation — the whole point of the SHM zero-copy path.
//!
//! Run: `cargo test -p zerodds-flatdata --test zero_alloc`

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;

use zerodds_flatdata::{FlatStruct, FlatWriter, InMemorySlotAllocator};

// dhat installs itself as the global allocator for this test binary so it can
// account every heap block.
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[derive(Copy, Clone)]
#[repr(C)]
struct Pose1k {
    bytes: [u8; 1024],
}

// SAFETY: repr(C) + Copy + 'static, single [u8; 1024] field.
unsafe impl FlatStruct for Pose1k {
    const TYPE_HASH: [u8; 16] = [0x42; 16];
}

#[test]
fn write_flat_is_heap_allocation_free() {
    let _profiler = dhat::Profiler::builder().testing().build();

    // Setup allocates (slots, Arc, writer) — done BEFORE the measured window.
    // Enough slots that the writes below never hit NoFreeSlot (so no recycling
    // and no read() — read() copies out a Vec and is intentionally excluded).
    const N: usize = 1000;
    let alloc = Arc::new(InMemorySlotAllocator::new(0, N + 16, Pose1k::WIRE_SIZE));
    let writer = FlatWriter::<Pose1k>::new(Arc::clone(&alloc), 0b1);
    let payload = Pose1k {
        bytes: [0xAA; 1024],
    };
    // Warm up one write so any one-time lazy allocation is outside the window.
    writer.write(&payload).expect("warmup write");

    let before = dhat::HeapStats::get();
    for _ in 0..N {
        writer.write(&payload).expect("write");
    }
    let after = dhat::HeapStats::get();

    let blocks = after.total_blocks - before.total_blocks;
    let bytes = after.total_bytes - before.total_bytes;
    assert_eq!(
        blocks, 0,
        "write_flat must be heap-allocation-free: {blocks} blocks / {bytes} bytes \
         allocated across {N} writes (reserve_slot + commit_slot copy into a \
         pre-allocated slot — no heap)"
    );
}
