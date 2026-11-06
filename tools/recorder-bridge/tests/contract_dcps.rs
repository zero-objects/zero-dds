// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-recorder-bridge` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-dcps` (`src/main.rs`):
//! - `RuntimeConfig::from_standard_paths`, `runtime::DcpsRuntime::start(domain,
//!   prefix, config)`, `register_user_reader` with
//!   `zerodds_cli_common::raw_reader_config_typed` per `--topic NAME:TYPE`,
//!   and `shutdown`, also when a reader cannot be created.
//! - `runtime::UserSample`: `Alive { payload, writer_guid,
//!   representation_identifier, .. }` is recorded with the identifier,
//!   `Lifecycle { kind, .. }` as one or two entries by its `ChangeKind`
//!   (see `contract_rtps.rs`), `CoherentSetEnd` is not recorded.
//!
//! What it relies on and is checked here:
//! - `representation_identifier` of a live sample is the identifier of its
//!   encapsulation header (DDSI-RTPS 2.5 §10.5; rustdoc of
//!   `UserSample::Alive`), two octets.
//! - The reader config of the tool asks for RELIABLE: a monitor takes the
//!   samples a reliable writer repairs (DDS 1.4 §2.2.3.14 makes BEST_EFFORT
//!   the DataReader default).
//! - The signatures of the calls on a running runtime.
//!
//! The capture needs a bound runtime; the calls and the sample variants are
//! checked at compile time. No socket is opened.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample};
use zerodds_rtps::history_cache::ChangeKind;
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

/// What the tool takes out of a sample, as it does.
fn identifier_of(sample: &UserSample) -> Option<[u8; 2]> {
    match sample {
        UserSample::Alive {
            representation_identifier,
            ..
        } => Some(*representation_identifier),
        UserSample::Lifecycle { kind, .. } => {
            let _disposed = matches!(
                kind,
                ChangeKind::NotAliveDisposed | ChangeKind::NotAliveDisposedUnregistered
            );
            None
        }
        UserSample::CoherentSetEnd { .. } => None,
    }
}

/// What `register_user_reader` returns: the reader and its samples.
type ReaderHandle = zerodds_dcps::Result<(EntityId, Receiver<UserSample>)>;

#[test]
fn the_runtime_calls_the_tool_makes_have_their_signatures() {
    let _start: fn(i32, GuidPrefix, RuntimeConfig) -> zerodds_dcps::Result<Arc<DcpsRuntime>> =
        DcpsRuntime::start;
    let _reader: fn(&DcpsRuntime, UserReaderConfig) -> ReaderHandle =
        DcpsRuntime::register_user_reader;
    let _shutdown: fn(&DcpsRuntime) = DcpsRuntime::shutdown;
    let _take: fn(&UserSample) -> Option<[u8; 2]> = identifier_of;
}

#[test]
fn dcps_2_2_3_14_the_tools_reader_asks_for_reliable() {
    let cfg = zerodds_cli_common::raw_reader_config_typed("T", "N");
    assert!(cfg.reliable);
    assert!(!UserReaderConfig::default().reliable);
}
