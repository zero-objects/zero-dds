// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-flatdata`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-flatdata` (`src/flatdata_integration.rs`,
//! `src/subscriber.rs`, with `flatdata-integration`): a `SlotBackend`, through
//! `reserve_slot` with the mask of the active readers, `commit_slot`,
//! `read_slot`, `mark_read` and the `SlotError` they return; `FlatStruct`
//! for the sample's octets; `SlotHandle` as the token between them.
//!
//! Oracle: zerodds-flatdata-1.0 §2 (the slot header: sequence number,
//! sample size, reader mask), §4.1 (reserve, write, commit; the reader reads
//! without a copy and gives the slot back), §5.1 (a slot is recyclable when
//! every active reader's bit is set; a full cache has no slot to reserve),
//! §1.1 (`as_bytes` is the `#[repr(C)]` memory of the value, `WIRE_SIZE` its
//! size).
#![allow(clippy::unwrap_used, clippy::panic, unsafe_code)]

use zerodds_flatdata::{FlatStruct, InMemorySlotAllocator};

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(C)]
struct Pose {
    x: i32,
    y: i32,
}

// SAFETY: repr(C), Copy, 'static, two i32 without padding.
unsafe impl FlatStruct for Pose {
    const TYPE_HASH: [u8; 16] = [7; 16];
}

#[test]
fn flatdata_4_1_a_committed_sample_is_read_back_and_carries_its_size_and_sequence_number() {
    let backend = InMemorySlotAllocator::new(1, 2, 64);
    let pose = Pose { x: 3, y: -4 };
    let handle = backend.reserve_slot(0b1).unwrap();
    let sequence = backend.commit_slot(handle, pose.as_bytes()).unwrap();
    let (header, bytes) = backend.read_slot(handle).unwrap();
    assert_eq!(header.sample_size as usize, Pose::WIRE_SIZE);
    assert_eq!(header.sequence_number, sequence);
    assert_eq!(bytes, pose.as_bytes());
}

#[test]
fn flatdata_5_1_the_slot_comes_back_when_every_active_reader_has_read_it() {
    let backend = InMemorySlotAllocator::new(1, 1, 64);
    let handle = backend.reserve_slot(0b11).unwrap();
    backend.commit_slot(handle, &[1, 2, 3, 4]).unwrap();
    assert!(backend.reserve_slot(0b11).is_err(), "the only slot is held");
    backend.mark_read(handle, 0).unwrap();
    assert!(
        backend.reserve_slot(0b11).is_err(),
        "reader 1 has not read it"
    );
    backend.mark_read(handle, 1).unwrap();
    assert!(backend.reserve_slot(0b11).is_ok(), "both have read it");
}

#[test]
fn flatdata_4_1_a_sample_larger_than_the_slot_is_refused() {
    let backend = InMemorySlotAllocator::new(1, 1, 8);
    let handle = backend.reserve_slot(0b1).unwrap();
    assert!(backend.commit_slot(handle, &[0; 9]).is_err());
}
