// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The wait of a DDS reader of the test for the daemon's DataWriter to match
// it, by a sample that arrives (`#[path]` include).

use std::sync::mpsc::Receiver;

use zerodds_dcps::runtime::UserSample;

/// What [`publish_until_received`] did. Each test file that includes this
/// module reads the fields it needs, not all of them.
#[allow(dead_code)]
pub struct Received {
    /// The calls of `publish` it made.
    pub calls: i64,
    /// The CDR bytes of the first sample that arrived.
    pub payload: Vec<u8>,
    /// Its representation identifier.
    pub representation_identifier: [u8; 2],
}

/// [`zerodds_test_domains::publish_until_received`] on a reader of the
/// runtime: `publish` (one call, one sample of the daemon's DataWriter)
/// until a sample reaches `rx`, the samples of the later calls taken. The
/// next sample the daemon writes has the sequence number `calls + 1`.
pub fn publish_until_received(rx: &Receiver<UserSample>, publish: impl FnMut()) -> Received {
    let (calls, (payload, representation_identifier)) =
        zerodds_test_domains::publish_until_received(
            rx,
            |sample| match sample {
                UserSample::Alive {
                    payload,
                    source_sequence_number,
                    representation_identifier,
                    ..
                } => Some((
                    source_sequence_number,
                    (payload.to_vec(), representation_identifier),
                )),
                _ => None,
            },
            publish,
        );
    Received {
        calls,
        payload,
        representation_identifier,
    }
}
