// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-replay` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-dcps`, in `src/main.rs`, `InjectState`
//! (`replay --inject [--inject-domain N] [--representation H]`):
//! - `RuntimeConfig::from_standard_paths`, an error is the error
//!   "runtime config: …".
//! - `runtime::DcpsRuntime::start(domain, prefix, config)`,
//!   `discovered_participants` (waits for one peer, at most 5 s),
//!   `register_user_writer` with `zerodds_cli_common::reliable_raw_writer_config`
//!   once per recorded topic, `user_writer_matched_count` (waits for one
//!   reader, at most 2 s), `set_user_writer_representation_identifier` with
//!   the identifier a sample was recorded with, `write_user_sample` with the
//!   recorded body, `await_user_writer_acknowledged` per writer before the
//!   end (at most 5 s), and `shutdown` in `Drop`.
//!
//! What it relies on and is checked here:
//! - `set_user_writer_representation_identifier` takes the CDR identifiers
//!   of DDS-XTypes 1.3 Table 60 and refuses another one with
//!   `BadParameter` before it changes the writer (its rustdoc), so the tool
//!   reports a capture with such an identifier instead of sending it under
//!   a wrong header. Checked with a writer id the runtime does not know:
//!   the refusal comes first.
//! - The signatures of the calls on a running runtime.
//!
//! The injection itself needs a bound runtime; it runs in
//! `inject_under_the_recorded_identifier.rs`.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use zerodds_dcps::DdsError;
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

/// What `register_user_reader` returns: the reader and its samples.
type ReaderHandle = zerodds_dcps::Result<(EntityId, Receiver<UserSample>)>;

#[test]
fn the_runtime_calls_the_tool_makes_have_their_signatures() {
    let _start: fn(i32, GuidPrefix, RuntimeConfig) -> zerodds_dcps::Result<Arc<DcpsRuntime>> =
        DcpsRuntime::start;
    let _writer: fn(&DcpsRuntime, UserWriterConfig) -> zerodds_dcps::Result<EntityId> =
        DcpsRuntime::register_user_writer;
    let _reader: fn(&DcpsRuntime, UserReaderConfig) -> ReaderHandle =
        DcpsRuntime::register_user_reader;
    let _identifier: fn(&DcpsRuntime, EntityId, [u8; 2]) -> zerodds_dcps::Result<()> =
        DcpsRuntime::set_user_writer_representation_identifier;
    let _write: fn(&DcpsRuntime, EntityId, Vec<u8>) -> zerodds_dcps::Result<()> =
        DcpsRuntime::write_user_sample;
    let _matched: fn(&DcpsRuntime, EntityId) -> usize = DcpsRuntime::user_writer_matched_count;
    let _acknowledged: fn(&DcpsRuntime, EntityId, Instant) -> bool =
        DcpsRuntime::await_user_writer_acknowledged;
    let _shutdown: fn(&DcpsRuntime) = DcpsRuntime::shutdown;
    assert!(RuntimeConfig::from_standard_paths().is_ok() || cfg!(not(unix)));
}

/// 0x0004 is XML in DDS-XTypes 1.3 Table 60, no CDR form: the runtime
/// refuses the identifier before it looks for the writer, so the tool
/// reports it instead of sending a body under it.
#[test]
fn xtypes_7_6_3_1_2_an_identifier_of_no_cdr_form_is_refused() {
    let lease = zerodds_test_domains::lease_domain();
    let rt = DcpsRuntime::start(
        lease.id(),
        GuidPrefix::from_bytes([0x5f; 12]),
        RuntimeConfig::default(),
    )
    .unwrap();
    let unknown = EntityId::user_writer_with_key([0x00, 0x00, 0x7f]);
    let refused = rt.set_user_writer_representation_identifier(unknown, [0x00, 0x04]);
    rt.shutdown();
    assert!(
        matches!(refused, Err(DdsError::BadParameter { .. })),
        "{refused:?}"
    );
}
