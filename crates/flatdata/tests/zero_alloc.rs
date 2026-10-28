// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Spec §11.3 — zero heap allocation in the zero-copy write hot path.
//!
//! A counting global allocator (this test binary only) counts the heap blocks
//! the measuring thread allocates during a steady-state run of
//! `FlatWriter::write` (reserve + commit into a pre-allocated slot). The claim:
//! a `write_flat` does no heap allocation — the whole point of the SHM
//! zero-copy path.
//!
//! Only the measuring thread counts. `write` runs entirely on the calling
//! thread, and the test harness allocates on its own threads while a test
//! runs: counted globally (with `dhat`), the window caught 5 blocks / 948 bytes
//! in one of three runs on the Linux test host at a load of 70, at an unchanged path.
//!
//! Run: `cargo test -p zerodds-flatdata --test zero_alloc`

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

use zerodds_flatdata::{FlatStruct, FlatWriter, InMemorySlotAllocator};

/// `System`, counting the blocks and bytes a thread allocates while it is
/// inside [`allocations_of`]. A realloc counts as a block: it may move.
struct ThreadCounting;

thread_local! {
    /// `Some((blocks, bytes))` while this thread is counted.
    static COUNT: Cell<Option<(u64, u64)>> = const { Cell::new(None) };
}

fn note(bytes: usize) {
    // `try_with`: an allocation during thread teardown, after the slot is
    // gone, is not a measurement.
    let _ = COUNT.try_with(|c| {
        if let Some((blocks, total)) = c.get() {
            c.set(Some((blocks + 1, total + bytes as u64)));
        }
    });
}

// SAFETY: every call is forwarded to `System` with the caller's arguments;
// `note` only touches a const-initialised thread-local `Cell`, which does not
// allocate.
unsafe impl GlobalAlloc for ThreadCounting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        // SAFETY: forwarded unchanged; the caller upholds `alloc`'s contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        // SAFETY: as in `alloc`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size);
        // SAFETY: as in `alloc`; `ptr` came from this allocator, which is
        // `System` underneath.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: as in `realloc`.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOC: ThreadCounting = ThreadCounting;

/// Runs `f` with this thread counted; returns the blocks and bytes it
/// allocated. Other threads, counted or not, do not add to it.
fn allocations_of(f: impl FnOnce()) -> (u64, u64) {
    COUNT.with(|c| c.set(Some((0, 0))));
    f();
    COUNT.with(|c| c.take()).unwrap_or((0, 0))
}

#[derive(Copy, Clone)]
#[repr(C)]
struct Pose1k {
    bytes: [u8; 1024],
}

// SAFETY: repr(C) + Copy + 'static, single [u8; 1024] field.
unsafe impl FlatStruct for Pose1k {
    const TYPE_HASH: [u8; 16] = [0x42; 16];
}

/// The counter sees an allocation of the measuring thread, so a zero below
/// is a measurement and not a counter that never counts.
#[test]
fn the_counter_counts_the_measuring_thread() {
    let (blocks, bytes) = allocations_of(|| {
        let v: Vec<u8> = Vec::with_capacity(4096);
        std::hint::black_box(&v);
    });
    assert_eq!(blocks, 1);
    assert_eq!(bytes, 4096);
}

#[test]
fn write_flat_is_heap_allocation_free() {
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

    let (blocks, bytes) = allocations_of(|| {
        for _ in 0..N {
            writer.write(&payload).expect("write");
        }
    });
    assert_eq!(
        blocks, 0,
        "write_flat must be heap-allocation-free: {blocks} blocks / {bytes} bytes \
         allocated across {N} writes (reserve_slot + commit_slot copy into a \
         pre-allocated slot — no heap)"
    );
}
