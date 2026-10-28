//! Exact installed Cyclone descriptor serialization, captured 2026-09-12.
//! Provenance and reproducible C helper: internal/release/evidence/
//! rc7-recovery-2026-09-10/root/cyclone-typelookup-oracle*.
#![allow(clippy::unwrap_used)]

use zerodds_types::framing::{xcdr2_reader, xcdr2_writer};
use zerodds_types::type_lookup::*;

#[test]
fn all_four_service_envelopes_match_cyclone_serializer() {
    let fixtures: Vec<Vec<u8>> = include_str!("fixtures/cyclone-typelookup-11.log")
        .lines()
        .map(|line| {
            let (_, hex) = line.split_once('=').unwrap();
            (0..hex.len())
                .step_by(2)
                .map(|offset| u8::from_str_radix(&hex[offset..offset + 2], 16).unwrap())
                .collect()
        })
        .collect();
    for (index, call) in [
        TypeLookupCall::GetTypes(GetTypesRequest::default()),
        TypeLookupCall::GetTypeDependencies(GetTypeDependenciesRequest::default()),
    ]
    .into_iter()
    .enumerate()
    {
        let request = TypeLookupRequest {
            header: RequestHeader {
                writer_guid: [0x5a; 16],
                sequence_number: 0x0102_0304_0506_0708,
                instance_name: "dds.builtin.TOS.000000000000000000000000".into(),
            },
            data: call,
        };
        let mut writer = xcdr2_writer();
        request.encode_into(&mut writer).unwrap();
        assert_eq!(writer.finish(), fixtures[index * 2]);
        let mut reader = xcdr2_reader(&fixtures[index * 2]);
        assert_eq!(
            TypeLookupRequest::decode_from(&mut reader).unwrap(),
            request
        );
        assert_eq!(reader.remaining(), 0);

        let reply = TypeLookupReplyMessage {
            header: ReplyHeader {
                writer_guid: request.header.writer_guid,
                sequence_number: request.header.sequence_number,
                remote_ex: 0,
            },
            ret: if index == 0 {
                TypeLookupReturn::GetTypes(GetTypesResult::default())
            } else {
                TypeLookupReturn::GetTypeDependencies(GetTypeDependenciesResult::default())
            },
        };
        let mut writer = xcdr2_writer();
        reply.encode_into(&mut writer).unwrap();
        assert_eq!(writer.finish(), fixtures[index * 2 + 1]);
        let mut reader = xcdr2_reader(&fixtures[index * 2 + 1]);
        assert_eq!(
            TypeLookupReplyMessage::decode_from(&mut reader).unwrap(),
            reply
        );
        assert_eq!(reader.remaining(), 0);
    }
}
