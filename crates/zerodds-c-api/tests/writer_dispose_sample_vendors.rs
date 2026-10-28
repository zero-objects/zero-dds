// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds_writer_dispose_sample` / `_unregister_sample` /
//! `_unregister_with_dispose_sample` against readers of other vendors
//! (D10, the vendor half of `writer_dispose_sample.rs`).
//!
//! `writer_dispose_sample.rs` proves the mechanism on a same-runtime
//! reader: the `_sample` functions derive the KeyHash and the canonical
//! key holder from the sample. What a foreign reader does with the DATA
//! they send is what this file measures. The reader runs as a child
//! process and reports the last instance state it saw per instance; three
//! instances go through the three `_sample` functions, and a fourth
//! through the `PID_KEY_HASH`-only `zerodds_writer_dispose`. That one goes
//! out with the `serializedKey` too: the writer recorded the instance's key
//! when it wrote the instance, and a key within 16 octets can also be read
//! back from the hash (DDSI-RTPS 2.5 §9.6.4.8). A `PID_KEY_HASH`-only
//! dispose is what Cyclone 11.0.1, Fast DDS 3.6.1 and OpenDDS 3.35 did not
//! act on (`Test_FinalInstanceState_1`: 0/4, with `serializedKey` 4/4).
//!
//! Two tests, both live against another vendor, so `#[ignore]` like the
//! other cross-vendor tests (`crates/ros2-rmw/tests/cross_vendor.rs`), run
//! with `--ignored`:
//!
//! - Cyclone 11 through its Python binding, on a host where
//!   `python3 -c 'import cyclonedds'` works (`PYTHON` names another
//!   interpreter); a missing binding fails the run.
//! - Any reader program that speaks the protocol of
//!   `zerodds-test-vendor-reader`, named by `ZERODDS_DISPOSE_READER` (a
//!   command line; the protocol arguments are appended).
//!   `tools/dispose-reader/build.sh` builds the Fast DDS, the OpenDDS and
//!   the OpenSplice one and prints the value to set; unset, the test fails.
//!   OpenSplice 6.9 has no XCDR2, and `zerodds_runtime_create` offers XCDR2
//!   first, which a reader without `DataRepresentation` does not match
//!   (XTypes 1.3 §7.6.3.1.1): against it the run sets
//!   `ZERODDS_DATA_REPR_OFFER=XCDR1`. The body of two `long`s is the same
//!   in both.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::collections::BTreeMap;
use std::ffi::CString;
use std::time::Duration;

use zerodds::{
    ZeroDdsStatus, zerodds_runtime_create, zerodds_runtime_destroy, zerodds_writer_create_typed,
    zerodds_writer_destroy, zerodds_writer_dispose, zerodds_writer_dispose_sample,
    zerodds_writer_unregister_sample, zerodds_writer_unregister_with_dispose_sample,
    zerodds_writer_wait_for_matched, zerodds_writer_write,
};
use zerodds_test_vendor_reader::{
    VendorReader, keyed_point_a as sample, keyed_point_a_key_hash, keyed_point_a_type_object,
    reader_command,
};

const OK: i32 = ZeroDdsStatus::Ok as i32;

/// What the reader `which` reports while the writer is alive, by instance
/// id, after instance 7 went through `_dispose_sample`, 8 through
/// `_unregister_sample`, 9 through `_unregister_with_dispose_sample` and 10
/// through the hash-only `zerodds_writer_dispose`.
fn run_against(which: &str, topic: &str) -> BTreeMap<String, String> {
    let lease = zerodds_test_vendor_reader::lease_domain();
    let domain = lease.id();
    let (command, label) = reader_command(which);
    let mut reader = VendorReader::start(command, &label, domain, topic);

    let type_object = keyed_point_a_type_object();
    let topic_c = CString::new(topic).unwrap();
    let type_name_c = CString::new("KeyedPointA").unwrap();
    let while_alive;
    // SAFETY: the runtime and the writer come from the calls right above
    // their use and are destroyed exactly once at the end of this block;
    // every buffer handed over lives across the call it is handed to.
    unsafe {
        let runtime = zerodds_runtime_create(domain as u32);
        assert!(!runtime.is_null(), "runtime create failed");
        let writer = zerodds_writer_create_typed(
            runtime,
            topic_c.as_ptr(),
            type_name_c.as_ptr(),
            1,
            1,
            type_object.as_ptr(),
            type_object.len(),
        );
        assert!(!writer.is_null(), "typed keyed writer create failed");
        assert_eq!(
            zerodds_writer_wait_for_matched(writer, 1, 8_000),
            OK,
            "the ZeroDDS writer did not match the {label} reader on domain {domain}"
        );
        reader.wait_matched();

        for id in [7, 8, 9, 10] {
            let s = sample(id, 100 + id);
            assert_eq!(zerodds_writer_write(writer, s.as_ptr(), s.len()), OK);
        }
        assert!(
            reader.wait_seen(&[7, 8, 9, 10], Duration::from_secs(10)),
            "the {label} reader did not get the four samples"
        );

        let s = sample(7, 107);
        assert_eq!(
            zerodds_writer_dispose_sample(writer, s.as_ptr(), s.len()),
            OK
        );
        let s = sample(8, 108);
        assert_eq!(
            zerodds_writer_unregister_sample(writer, s.as_ptr(), s.len()),
            OK
        );
        let s = sample(9, 109);
        assert_eq!(
            zerodds_writer_unregister_with_dispose_sample(writer, s.as_ptr(), s.len()),
            OK
        );
        let key_hash = keyed_point_a_key_hash(10);
        assert_eq!(zerodds_writer_dispose(writer, key_hash.as_ptr()), OK);
        // The reliable writer keeps repairing until the reader has it all.
        std::thread::sleep(Duration::from_millis(1500));
        while_alive = reader.snapshot();

        zerodds_writer_destroy(writer);
        zerodds_runtime_destroy(runtime);
    }
    // The deleted writer's unregister has a moment to arrive.
    std::thread::sleep(Duration::from_millis(500));
    let _ = reader.finish();
    println!(
        "{label}: hash-only zerodds_writer_dispose, instance 10, writer alive: {}",
        while_alive.get("10").map_or("not seen", String::as_str)
    );
    while_alive
}

/// The states the four calls have to leave, the writer alive.
fn assert_the_calls_took_effect(label: &str, while_alive: &BTreeMap<String, String>) {
    for (id, want) in [
        ("7", "NOT_ALIVE_DISPOSED"),
        ("8", "NOT_ALIVE_NO_WRITERS"),
        ("9", "NOT_ALIVE_DISPOSED"),
        ("10", "NOT_ALIVE_DISPOSED"),
    ] {
        assert_eq!(
            while_alive.get(id).map(String::as_str),
            Some(want),
            "instance {id} at the {label} reader, writer alive (all: {while_alive:?})"
        );
    }
}

#[test]
#[ignore = "live against Eclipse Cyclone DDS: needs python3 with the cyclonedds binding; run with --ignored"]
fn a_cyclone_reader_sees_the_instance_states_the_sample_functions_send() {
    let states = run_against("cyclone", "CFfiDisposeSampleCycloneTopic");
    assert_the_calls_took_effect("cyclone", &states);
}

/// Measured on the Linux test host with Fast DDS 3.6.1 and OpenDDS 3.34
/// (`tools/dispose-reader/`); the C-FFI calls were measured against Cyclone
/// only before.
#[test]
#[ignore = "live against another vendor: set ZERODDS_DISPOSE_READER (tools/dispose-reader/build.sh); run with --ignored"]
fn a_foreign_reader_sees_the_instance_states_the_sample_functions_send() {
    let states = run_against("env", "CFfiDisposeSampleForeignTopic");
    assert_the_calls_took_effect("foreign", &states);
}
