// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]
//! The types of DDS-XRCE 1.0 Annex A that carry no `@extensibility`
//! annotation (`Time_t`, `ResultStatus`, `Property`, `TransportLocator*`,
//! `ACKNACK_Payload`, ...).
//!
//! DDS-XTypes 1.3 §7.3.1.2.1.8: a type whose extensibility is not given
//! "shall be considered appendable"; an APPENDABLE type has a DHEADER in
//! XCDR2. The default of this crate follows that.
//!
//! The datagrams eProsima Micro-XRCE-DDS 3.0.1 sent and received
//! (`tools/wire-capture/xrce-sessions/`) have none: a `STATUS` payload of
//! 6 octets, an `ACKNACK` of 5. That form is `UnannotatedTypes::Final`, a
//! named runtime setting. The readers accept both forms in either setting.

use std::fs;
use std::path::PathBuf;

use zerodds_xrce::encoding::{Endianness, UnannotatedTypes, with_unannotated_types};
use zerodds_xrce::object_info::{BaseObjectReply, BaseObjectRequest, ResultStatus};
use zerodds_xrce::submessages::timestamp::TimePoint;
use zerodds_xrce::submessages::{AckNackPayload, Message, SubmessageId};

fn datagrams(file: &str) -> Vec<Vec<u8>> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/wire-capture/xrce-sessions");
    fs::read_to_string(path.join(file))
        .unwrap()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| {
            (0..l.len() / 2)
                .map(|i| u8::from_str_radix(&l[2 * i..2 * i + 2], 16).unwrap())
                .collect()
        })
        .collect()
}

#[test]
fn the_reference_implementations_octets_are_read_in_the_default_setting() {
    let mut statuses = 0;
    let mut acknacks = 0;
    for file in ["agent-to-client.hex", "client-to-agent.hex"] {
        for bytes in datagrams(file) {
            let msg = Message::decode(&bytes).unwrap();
            for sm in &msg.submessages {
                match sm.header.submessage_id {
                    SubmessageId::Status => {
                        statuses += 1;
                        assert_eq!(sm.body.len(), 6, "captured STATUS has no DHEADER");
                        let e = Endianness::Little;
                        BaseObjectReply::decode_with(&sm.body, e).unwrap();
                    }
                    SubmessageId::AckNack => {
                        acknacks += 1;
                        assert_eq!(sm.body.len(), 5, "captured ACKNACK has no DHEADER");
                        AckNackPayload::decode_body(&sm.body, Endianness::Little).unwrap();
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(statuses > 0 && acknacks > 0, "{statuses} {acknacks}");
}

#[test]
fn the_default_is_appendable_with_a_dheader() {
    assert_eq!(
        zerodds_xrce::encoding::unannotated_types(),
        UnannotatedTypes::Appendable
    );
    // ResultStatus: DHEADER 2 + 2 members.
    assert_eq!(ResultStatus::ok().encode().len(), 6);
    let reply = BaseObjectReply {
        related_request: BaseObjectRequest::default(),
        result: ResultStatus::ok(),
    };
    // BaseObjectRequest (4) + ResultStatus (6).
    assert_eq!(reply.encode().len(), 10);
    let ack = AckNackPayload {
        first_unacked_seq_num: 1,
        nack_bitmap: [0, 0],
        stream_id: 0x80,
    };
    assert_eq!(
        ack.encode_body(Endianness::Little).unwrap(),
        [5, 0, 0, 0, 1, 0, 0, 0, 0x80]
    );
    let mut body = Vec::new();
    TimePoint {
        seconds: 1,
        nanoseconds: 2,
    }
    .encode_into(&mut body, Endianness::Little);
    assert_eq!(body, [8, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0]);
}

#[test]
fn the_final_setting_writes_the_reference_implementations_octets() {
    with_unannotated_types(UnannotatedTypes::Final, || {
        assert_eq!(ResultStatus::ok().encode().len(), 2);
        let reply = BaseObjectReply {
            related_request: BaseObjectRequest::default(),
            result: ResultStatus::ok(),
        };
        assert_eq!(reply.encode().len(), 6);
        let ack = AckNackPayload {
            first_unacked_seq_num: 1,
            nack_bitmap: [0, 0],
            stream_id: 0x80,
        };
        assert_eq!(
            ack.encode_body(Endianness::Little).unwrap(),
            [1, 0, 0, 0, 0x80]
        );
        let mut body = Vec::new();
        TimePoint {
            seconds: 1,
            nanoseconds: 2,
        }
        .encode_into(&mut body, Endianness::Little);
        assert_eq!(body, [1, 0, 0, 0, 2, 0, 0, 0]);
    });
}

#[test]
fn both_forms_are_read_in_either_setting() {
    let reply = BaseObjectReply {
        related_request: BaseObjectRequest::default(),
        result: ResultStatus::ok(),
    };
    let appendable = reply.encode();
    let final_form = with_unannotated_types(UnannotatedTypes::Final, || reply.encode());
    for setting in [UnannotatedTypes::Appendable, UnannotatedTypes::Final] {
        with_unannotated_types(setting, || {
            for bytes in [&appendable, &final_form] {
                let (got, _) = BaseObjectReply::decode_with(bytes, Endianness::Little).unwrap();
                assert_eq!(got, reply);
            }
        });
    }
}
