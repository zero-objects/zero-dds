// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-c-api` with `zerodds-flatdata`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-flatdata` (`src/shm_loan_ffi.rs`, with
//! `flatdata-loan`): `PosixSlotAllocator::create` for a writer's segment and
//! `attach` for a reader on the same host, the `SlotBackend` operations on
//! both mappings, `ShmLocator` with `fnv1a_32` of the host id, written into
//! the locator the C API hands out, and `SlotHandle`.
//!
//! Oracle: zerodds-flatdata-1.0 §3.1 (the locator: hostname hash, uid,
//! slot count, slot size as little-endian `u32`, then the segment path as a
//! CDR string), §4.1 (what the owner commits, a second mapping reads), §2
//! (slot size 16 + sample size, padded to 64); FNV-1a 32 bit
//! (draft-eastlake-fnv §6, the reference vectors `""` 0x811C9DC5, `"a"`
//! 0xE40C292C, `"foobar"` 0xBF9CF968).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_flatdata::{PosixSlotAllocator, ShmLocator, SlotBackend, fnv1a_32};

#[test]
fn the_host_hash_is_fnv1a_32_of_its_reference_vectors() {
    assert_eq!(fnv1a_32(b""), 0x811C_9DC5);
    assert_eq!(fnv1a_32(b"a"), 0xE40C_292C);
    assert_eq!(fnv1a_32(b"foobar"), 0xBF9C_F968);
}

#[test]
fn flatdata_3_1_the_locator_writes_its_four_words_and_the_path() {
    let locator = ShmLocator {
        hostname_hash: 0x0102_0304,
        uid: 7,
        slot_count: 4,
        slot_size: 128,
        segment_path: "/zd".into(),
    };
    let octets = locator.to_bytes_le().unwrap();
    assert_eq!(
        octets,
        [
            4, 3, 2, 1, 7, 0, 0, 0, 4, 0, 0, 0, 128, 0, 0, 0, 4, 0, 0, 0, b'/', b'z', b'd', 0
        ]
    );
    assert_eq!(ShmLocator::from_bytes_le(&octets).unwrap(), locator);
}

#[test]
fn flatdata_4_1_what_the_owner_commits_an_attached_mapping_reads() {
    let flink = std::env::temp_dir().join(format!("capi-contract-flatdata-{}", std::process::id()));
    let owner = PosixSlotAllocator::create(flink.clone(), 4, 64).unwrap();
    let reader = PosixSlotAllocator::attach(flink.clone()).unwrap();
    assert_eq!(
        SlotBackend::slot_total_size(&owner),
        128,
        "16 + 64, padded to 64"
    );
    let handle = owner.reserve_slot(0b1).unwrap();
    owner.commit_slot(handle, &[9, 8, 7]).unwrap();
    let (_, bytes) = reader.read_slot(handle).unwrap();
    assert_eq!(bytes, [9, 8, 7]);
    drop(reader);
    drop(owner);
    let _ = std::fs::remove_file(&flink);
}
