// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-bench-suite` with `zerodds-c-api`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-c-api` (as `zerodds`; its library does
//! not, the binary `src/bin/roundtrip_1us.rs` does, in its DCPS modes):
//! - `create_dcps_runtime`: `ffi_runtime_config(vec![2, 0])` (XCDR2, then
//!   XCDR1, beneath the standard files), the `RuntimeConfig` it returns
//!   with `user_transport` set and `user_transports` cleared, and
//!   `runtime_create_with_config(domain, config)` with the `--dcps-domain`
//!   of the command line.
//! - `run_pong_dcps` / `run_ping_dcps` and their listener variants:
//!   `zerodds_runtime_wait_for_peers`, `zerodds_writer_create` /
//!   `zerodds_reader_create` (best effort, the type `RoundtripBytes`),
//!   `zerodds_writer_wait_for_matched` / `zerodds_reader_wait_for_matched`,
//!   `zerodds_writer_write`, `zerodds_reader_take` (no `out_repr`; `0` with
//!   a non-NULL buffer is a sample) with `zerodds_buffer_free` of the
//!   buffer and its length, `zerodds_reader_set_data_callback` with an
//!   `extern "C"` callback (`ZeroDdsDataCallback`) and with `None` to clear
//!   it, the three `zerodds_*_destroy` calls; the handle types
//!   `ZeroDdsRuntime` and `ZeroDdsWriter`.
//!
//! What it relies on and is checked here:
//! - `ffi_runtime_config` gives the bench's offer `[XCDR2, XCDR1]` where
//!   neither the standard files nor the environment set
//!   `wire.data_representation_offer`, and their offer where they do.
//! - `runtime_create_with_config` refuses a domain that has no port before
//!   it starts anything.
//! - The status code the bench compares against: `0` is OK, every error is
//!   negative.
//! - The callback type of the listener variants, `None` as the cleared
//!   callback, and the signatures of the calls on a running runtime.
//!
//! Oracles: the rustdoc of `ffi_runtime_config` ("the caller's
//! byte-encoding profile as the fallback offer. Explicit file
//! representation remains authoritative") and of
//! `runtime_create_with_config` ("# Errors Invalid domain"); DDS-XTypes 1.3
//! §7.6.3.1.1 (`XCDR_DATA_REPRESENTATION` = 0, `XCDR2_DATA_REPRESENTATION`
//! = 2); DDSI-RTPS 2.5 §9.6.2.2 (the discovery port `PB + DG * domainId +
//! d0`, PB 7400, DG 250, is a 16-bit UDP port);
//! `docs/specs/zerodds-c-api-1.0.md` §8 (status codes); the rustdoc of
//! `zerodds_reader_set_data_callback` ("`callback = NULL` clears an
//! existing listener") and the Rust reference on `Option<extern "C" fn>`
//! (a nullable function pointer).
//!
//! Every call on a runtime, writer or reader needs a started runtime, which
//! binds sockets; the bench itself runs them on two hosts. Their rustdoc
//! requires a valid handle, so they are checked here for their signature
//! only, through a function pointer of the type the bench calls them with.
//! Not checked here: the encapsulation header the writer stamps, the
//! buffer `take` hands out, the matching, the callback's samples. No socket
//! is opened.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use core::ffi::{c_char, c_int, c_void};
use core::mem::size_of;

use zerodds::{ZeroDdsDataCallback, ZeroDdsReader, ZeroDdsRuntime, ZeroDdsStatus, ZeroDdsWriter};
use zerodds_dcps::runtime::{ConfigSource, RuntimeConfig};

/// DDS-XTypes 1.3 §7.6.3.1.1: `XCDR_DATA_REPRESENTATION`.
const XCDR: i16 = 0;
/// DDS-XTypes 1.3 §7.6.3.1.1: `XCDR2_DATA_REPRESENTATION`.
const XCDR2: i16 = 2;

/// The callback shape of `run_pong_dcps_listener` / `run_ping_dcps_listener`.
extern "C" fn on_sample(
    _user_data: *mut c_void,
    _payload: *const u8,
    _len: usize,
    _representation: u8,
    _big_endian: u8,
) {
}

#[test]
fn xtypes_7_6_3_1_1_the_bench_offer_is_the_fallback_beneath_the_standard_files() {
    let offer = vec![XCDR2, XCDR];
    // The offer the standard layers name, if any: the host's files and
    // environment are the bench's too.
    let expected = ConfigSource::from_standard_paths()
        .map_err(|e| e.to_string())
        .and_then(|source| {
            let config = RuntimeConfig::from_source(&source).map_err(|e| e.to_string())?;
            Ok(match source.var("wire.data_representation_offer") {
                None => offer.clone(),
                Some(_) => config.data_representation_offer,
            })
        });
    match (zerodds::ffi_runtime_config(offer), expected) {
        (Ok(config), Ok(expected)) => assert_eq!(config.data_representation_offer, expected),
        (Err(_), Err(_)) => {}
        (got, expected) => panic!("{got:?} against {expected:?}"),
    }
}

#[test]
fn rtps_9_6_2_2_a_domain_without_a_discovery_port_is_refused_before_the_runtime_starts() {
    // 7400 + 250 * domain is no 16-bit port for either domain.
    for domain in [u32::MAX, 1 << 31] {
        if let Ok(rt) = zerodds::runtime_create_with_config(domain, RuntimeConfig::default()) {
            // SAFETY: a runtime this call created, destroyed once.
            unsafe { zerodds::zerodds_runtime_destroy(rt) };
            panic!("a runtime on domain {domain}");
        }
    }
}

#[test]
fn c_api_8_ok_is_0_and_the_errors_are_negative() {
    assert_eq!(ZeroDdsStatus::Ok as c_int, 0);
    for (status, code) in [
        (ZeroDdsStatus::Error, -1),
        (ZeroDdsStatus::BadHandle, -2),
        (ZeroDdsStatus::Timeout, -4),
        (ZeroDdsStatus::PreconditionNotMet, -5),
        (ZeroDdsStatus::BadParameter, -6),
        (ZeroDdsStatus::NoData, -7),
        (ZeroDdsStatus::OutOfResources, -8),
    ] {
        assert_eq!(status as c_int, code, "{status:?}");
    }
}

#[test]
fn the_listener_callback_and_its_none_are_a_nullable_c_function_pointer() {
    let _: ZeroDdsDataCallback = on_sample;
    // `None` crosses as NULL, as `zerodds_reader_set_data_callback` reads it.
    assert_eq!(
        size_of::<Option<ZeroDdsDataCallback>>(),
        size_of::<*const c_void>()
    );
}

#[test]
fn the_calls_on_a_running_runtime_keep_the_signatures_the_bench_calls() {
    let _: fn(Vec<i16>) -> Result<RuntimeConfig, String> = zerodds::ffi_runtime_config;
    let _: fn(u32, RuntimeConfig) -> Result<*mut ZeroDdsRuntime, String> =
        zerodds::runtime_create_with_config;
    let _: unsafe extern "C" fn(*mut ZeroDdsRuntime, c_int, u64) -> c_int =
        zerodds::zerodds_runtime_wait_for_peers;
    let _: unsafe extern "C" fn(
        *mut ZeroDdsRuntime,
        *const c_char,
        *const c_char,
        c_int,
    ) -> *mut ZeroDdsWriter = zerodds::zerodds_writer_create;
    let _: unsafe extern "C" fn(
        *mut ZeroDdsRuntime,
        *const c_char,
        *const c_char,
        c_int,
    ) -> *mut ZeroDdsReader = zerodds::zerodds_reader_create;
    let _: unsafe extern "C" fn(*mut ZeroDdsWriter, c_int, u64) -> c_int =
        zerodds::zerodds_writer_wait_for_matched;
    let _: unsafe extern "C" fn(*mut ZeroDdsReader, c_int, u64) -> c_int =
        zerodds::zerodds_reader_wait_for_matched;
    let _: unsafe extern "C" fn(*mut ZeroDdsWriter, *const u8, usize) -> c_int =
        zerodds::zerodds_writer_write;
    let _: unsafe extern "C" fn(*mut ZeroDdsReader, *mut *mut u8, *mut usize, *mut u8) -> c_int =
        zerodds::zerodds_reader_take;
    let _: unsafe extern "C" fn(*mut u8, usize) = zerodds::zerodds_buffer_free;
    let _: unsafe extern "C" fn(
        *mut ZeroDdsReader,
        Option<ZeroDdsDataCallback>,
        *mut c_void,
    ) -> c_int = zerodds::zerodds_reader_set_data_callback;
    let _: unsafe extern "C" fn(*mut ZeroDdsWriter) = zerodds::zerodds_writer_destroy;
    let _: unsafe extern "C" fn(*mut ZeroDdsReader) = zerodds::zerodds_reader_destroy;
    let _: unsafe extern "C" fn(*mut ZeroDdsRuntime) = zerodds::zerodds_runtime_destroy;
}
