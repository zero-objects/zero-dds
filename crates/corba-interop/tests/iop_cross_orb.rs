// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! CORBA 3.3 Part 2 §7.7.2 and §7.8 against JacORB 3.9.
//!
//! JacORB's `IOP::Codec` is the oracle of ZeroDDS's: the octets of
//! `encode` and `encode_value` under `ENCODING_CDR_ENCAPS` 1.2 for a
//! struct, a sequence, a union, a `wstring` and a `long long` agree octet
//! for octet, and ZeroDDS decodes JacORB's; both refuse an encoding
//! format other than `ENCODING_CDR_ENCAPS`, and JacORB's 1.0 codec
//! encodes a `wstring` where §7.8.1.1 asks for `InvalidTypeForEncoding`,
//! which ZeroDDS raises. A JacORB client interceptor
//! sends a service context outside the OMG range; the ZeroDDS server
//! interceptor sees it, the call succeeds, and the reply context of the
//! ZeroDDS interceptor reaches JacORB's.
//!
//! JacORB 3.9 and its Java 8 JDK come from
//! `tests/integration/provision.sh jacorb`; the job
//! `integration-bridges-vendor-corba` runs the tests (`#[ignore]` in the
//! crate tier, which has no JacORB).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use zerodds_corba_ccm::iop_codec::{CodecError, CodecFactory, Encoding, UnknownEncoding};
use zerodds_corba_ccm::portable_interceptor::{
    InitializerError, OrbInitInfo, OrbInitializer, PointResult, RequestInfo,
    ServerRequestInterceptor, register_orb_initializer,
};
use zerodds_corba_interop::runtime::{CorbaServer, orb_init, stringify_object_ref};
use zerodds_corba_rt::{AnyValue, CorbaAny, TypeCode, UnionLabel, UnionMember, WString};
use zerodds_corba_rust::SkeletonResult;

#[path = "support/jacorb.rs"]
mod jacorb;

fn sources() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("competitors/jacorb/iop")
}

fn spec<'a>(name: &'a str, main: &'a str, props: &'a [&'a str]) -> jacorb::Spec<'a> {
    jacorb::Spec {
        name,
        idl: vec![sources().join("iop.idl")],
        java: vec![
            sources().join("CodecDump.java"),
            sources().join("ContextClient.java"),
        ],
        main,
        props,
        ior_to_file: false,
        stdout_prefix: "",
    }
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// The values `CodecDump` encodes, as ZeroDDS anys.
fn values() -> Vec<(&'static str, CorbaAny)> {
    let union_tc = TypeCode::Union {
        repo_id: "IDL:U:1.0".into(),
        name: "U".into(),
        discriminator: Box::new(TypeCode::Long),
        default_index: -1,
        members: vec![
            UnionMember {
                label: UnionLabel::Long(1),
                name: "n".into(),
                member_type: TypeCode::Long,
            },
            UnionMember {
                label: UnionLabel::Long(2),
                name: "s".into(),
                member_type: TypeCode::String(0),
            },
        ],
    };
    vec![
        (
            "point",
            CorbaAny(AnyValue::Struct {
                repo_id: "IDL:Point:1.0".into(),
                name: "Point".into(),
                members: vec![
                    ("x".into(), AnyValue::Long(7)),
                    ("label".into(), AnyValue::Str("p".into())),
                ],
            }),
        ),
        (
            "shorts",
            CorbaAny(AnyValue::Seq {
                element: TypeCode::Short,
                items: vec![AnyValue::Short(-1), AnyValue::Short(2)],
            }),
        ),
        (
            "union",
            CorbaAny(AnyValue::Union {
                type_code: union_tc,
                discriminator: UnionLabel::Long(2),
                value: Some(Box::new(AnyValue::Str("two".into()))),
            }),
        ),
        (
            "wstring",
            CorbaAny(AnyValue::WStr(WString::from("\u{e4}\u{20ac}"))),
        ),
        (
            "longlong",
            CorbaAny(AnyValue::LongLong(0x0102_0304_0506_0708)),
        ),
    ]
}

#[test]
#[ignore = "needs JacORB 3.9 (integration-bridges-vendor-corba)"]
fn corba_p2_7_8_codec_octets_agree_with_jacorb() {
    let out = jacorb::run(&spec("iop-jacorb-codec", "CodecDump", &[]));
    assert!(out.contains("DONE"), "{out}");
    let lines: BTreeMap<&str, &str> = out.lines().filter_map(|l| l.split_once('=')).collect();
    let codec = CodecFactory
        .create_codec(Encoding::cdr_encaps(1, 2))
        .unwrap();
    for (name, any) in values() {
        let theirs = unhex(lines[format!("{name} encode").as_str()]);
        let theirs_value = unhex(lines[format!("{name} value").as_str()]);
        assert_eq!(codec.encode(&any).unwrap(), theirs, "{name}: encode");
        assert_eq!(
            codec.encode_value(&any).unwrap(),
            theirs_value,
            "{name}: encode_value"
        );
        assert_eq!(codec.decode(&theirs).unwrap(), any, "{name}: decode");
        assert_eq!(
            codec
                .decode_value(&theirs_value, &any.0.type_code())
                .unwrap(),
            any,
            "{name}: decode_value"
        );
    }
    // §7.8.1.1: "this exception is raised if the encoding is
    // ENCODING_CDR_ENCAPS version 1.0 and a type that does not exist in
    // that version, such as wstring, is passed to the operation".
    // JacORB 3.9's 1.0 codec encodes the wstring (measured); ZeroDDS
    // raises InvalidTypeForEncoding as the text says.
    assert_eq!(lines["wstring_1_0"], "encoded");
    let wide = values().remove(3).1;
    assert_eq!(
        CodecFactory
            .create_codec(Encoding::cdr_encaps(1, 0))
            .unwrap()
            .encode(&wide)
            .unwrap_err(),
        CodecError::InvalidTypeForEncoding
    );
    assert_eq!(lines["format_1"], "UnknownEncoding");
    assert_eq!(
        CodecFactory.create_codec(Encoding {
            format: 1,
            major_version: 1,
            minor_version: 2,
        }),
        Err(UnknownEncoding)
    );
}

const VENDOR_CTX: u32 = 0x4a43_0001;
const REPLY_CTX: u32 = 0x4a43_0002;

struct Recorder(Arc<Mutex<Vec<Vec<u8>>>>);

impl ServerRequestInterceptor for Recorder {
    fn name(&self) -> &str {
        "zerodds-vendor-context"
    }
    fn receive_request_service_contexts(&self, info: &mut RequestInfo) -> PointResult {
        if let Ok(data) = info.get_request_service_context(VENDOR_CTX) {
            self.0.lock().unwrap().push(data.to_vec());
        }
        Ok(())
    }
    fn send_reply(&self, info: &mut RequestInfo) -> PointResult {
        info.add_reply_service_context(REPLY_CTX, b"from-zerodds".to_vec(), false)?;
        Ok(())
    }
}

struct Init(Arc<Mutex<Vec<Vec<u8>>>>);

impl OrbInitializer for Init {
    fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), InitializerError> {
        if info.orb_id() == "iop-jacorb-context" {
            info.add_server_request_interceptor(Arc::new(Recorder(self.0.clone())))?;
        }
        Ok(())
    }
}

#[test]
#[ignore = "needs JacORB 3.9 (integration-bridges-vendor-corba)"]
fn corba_p2_7_7_2_a_jacorb_vendor_context_reaches_the_zerodds_interceptor() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    register_orb_initializer(Arc::new(Init(seen.clone())));
    let server = CorbaServer::new();
    server.set_orb(orb_init(&mut Vec::new(), "iop-jacorb-context").unwrap());
    server.register(b"Echo", |op, body, _e| match op {
        "ping" => SkeletonResult::Reply(body.bytes().to_vec()),
        _ => SkeletonResult::BadOperation,
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let ior = stringify_object_ref(
        "IDL:Echo:1.0",
        "127.0.0.1",
        acceptor.listen_addr().port(),
        b"Echo",
    );
    let prop = format!("zerodds.ior={ior}");
    let out = jacorb::run(&spec(
        "iop-jacorb-context",
        "ContextClient",
        &[prop.as_str()],
    ));
    acceptor.shutdown();
    assert!(out.contains("PING=hi"), "{out}");
    assert!(out.contains("REPLY_CTX=from-zerodds"), "{out}");
    assert!(out.contains("ALL OK"), "{out}");
    assert_eq!(*seen.lock().unwrap(), [b"from-jacorb".to_vec()]);
}
