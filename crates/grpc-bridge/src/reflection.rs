// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! gRPC Server Reflection (`grpc.reflection.v1.ServerReflection` and
//! `grpc.reflection.v1alpha.ServerReflection`).
//!
//! Spec: `zerodds-grpc-bridge-1.0.md` §4.6; the protocol is
//! `grpc/reflection/v1/reflection.proto` (grpc/grpc-proto), whose `v1alpha`
//! twin has the same messages and field numbers.
//!
//! `ServerReflectionInfo` is a bidirectional stream: every
//! `ServerReflectionRequest` gets one `ServerReflectionResponse`, which
//! names the request's `host` as `valid_host` and repeats the request as
//! `original_request`. The requests:
//!
//! * `list_services` — the services of the topics and the reflection
//!   service in both names.
//! * `file_by_filename`, `file_containing_symbol` — the
//!   `FileDescriptorProto` of the file: per package the messages of §4.2 and
//!   the package's topic services ([`crate::service_gen::proto_file`]), or
//!   the reflection protocol itself.
//! * `all_extension_numbers_of_type` — a message of a file has no
//!   extensions: its name and no number.
//! * `file_containing_extension` — no file declares an extension.
//!
//! An unknown file, symbol or type is an `error_response` with `NOT_FOUND`
//! (5), as grpc-go answers it; the stream stays open.

use alloc::string::String;
use alloc::vec::Vec;

use crate::descriptor::{FieldDesc, FieldType, FileDesc, MessageDesc, MethodDesc, ServiceDesc};
use crate::messages::{DecodeError, Field, fields, put_bytes, utf8};
use crate::service_gen::ServiceCatalog;

/// Reflection service path, the same value as in `grpcurl -plaintext localhost:N list`.
pub const REFLECTION_PATH: &str = "/grpc.reflection.v1alpha.ServerReflection/ServerReflectionInfo";

/// The `v1alpha` reflection service name.
pub const REFLECTION_SERVICE_V1ALPHA: &str = "grpc.reflection.v1alpha.ServerReflection";

/// The `v1` reflection service name.
///
/// grpc/grpc-proto promoted the service out of `v1alpha` and both names
/// are served side by side by every current implementation. A client
/// asks for `v1` first: grpcurl 1.9.3 opens
/// `/grpc.reflection.v1.ServerReflection/ServerReflectionInfo` and only
/// falls back to `v1alpha` on `UNIMPLEMENTED`.
pub const REFLECTION_SERVICE_V1: &str = "grpc.reflection.v1.ServerReflection";

/// Whether a `:path` service name addresses the reflection service, in
/// either of its two names.
#[must_use]
pub fn is_reflection_service(service: &str) -> bool {
    service == REFLECTION_SERVICE_V1 || service == REFLECTION_SERVICE_V1ALPHA
}

/// Status code (NOT_FOUND).
pub const STATUS_NOT_FOUND: i32 = 5;

/// What a `ServerReflectionRequest` asks (its `message_request` oneof).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestKind {
    /// `file_by_filename` (3).
    FileByFilename(String),
    /// `file_containing_symbol` (4).
    FileContainingSymbol(String),
    /// `file_containing_extension` (5): `containing_type`, `extension_number`.
    FileContainingExtension(String, i32),
    /// `all_extension_numbers_of_type` (6).
    AllExtensionNumbersOfType(String),
    /// `list_services` (7).
    ListServices,
}

/// One `ServerReflectionRequest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// `host` (1).
    pub host: String,
    /// The request; `None` when the oneof is not set.
    pub kind: Option<RequestKind>,
    /// The octets of the request, repeated as `original_request`.
    pub raw: Vec<u8>,
}

/// Reads a `ServerReflectionRequest` (proto3: unknown fields skipped, the
/// last member of the oneof wins).
///
/// # Errors
/// A message that is not proto3 wire format.
pub fn decode_request(msg: &[u8]) -> Result<Request, DecodeError> {
    let mut host = String::new();
    let mut kind = None;
    for f in fields(msg) {
        match f? {
            (1, Field::Len(v)) => host = utf8(v)?,
            (3, Field::Len(v)) => kind = Some(RequestKind::FileByFilename(utf8(v)?)),
            (4, Field::Len(v)) => kind = Some(RequestKind::FileContainingSymbol(utf8(v)?)),
            (5, Field::Len(v)) => {
                let mut containing = String::new();
                let mut number = 0i32;
                for g in fields(v) {
                    match g? {
                        (1, Field::Len(t)) => containing = utf8(t)?,
                        // int32: the low 32 bits of the varint.
                        #[allow(clippy::cast_possible_truncation)]
                        (2, Field::Varint(n)) => number = n as i32,
                        _ => {}
                    }
                }
                kind = Some(RequestKind::FileContainingExtension(containing, number));
            }
            (6, Field::Len(v)) => kind = Some(RequestKind::AllExtensionNumbersOfType(utf8(v)?)),
            (7, Field::Len(_)) => kind = Some(RequestKind::ListServices),
            _ => {}
        }
    }
    Ok(Request {
        host,
        kind,
        raw: msg.to_vec(),
    })
}

/// The files and services the reflection service answers about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reflection {
    files: Vec<FileDesc>,
    services: Vec<String>,
}

impl Reflection {
    /// The reflection of `catalog`: the file of each package of its
    /// services, and the reflection protocol in its two packages.
    #[must_use]
    pub fn of(catalog: &ServiceCatalog) -> Self {
        let mut files = catalog.proto_files();
        let mut services = catalog.fully_qualified_service_names();
        for package in ["grpc.reflection.v1", "grpc.reflection.v1alpha"] {
            files.push(reflection_file(package));
            services.push(format!("{package}.ServerReflection"));
        }
        Self { files, services }
    }

    /// The fully-qualified services `list_services` names.
    #[must_use]
    pub fn services(&self) -> &[String] {
        &self.services
    }

    /// The file descriptors served.
    #[must_use]
    pub fn files(&self) -> &[FileDesc] {
        &self.files
    }

    /// The `ServerReflectionResponse` to `req`. `None` when the request
    /// names nothing (the oneof is not set): grpc-go ends the stream with
    /// `INVALID_ARGUMENT` then.
    #[must_use]
    pub fn respond(&self, req: &Request) -> Option<Vec<u8>> {
        let kind = req.kind.as_ref()?;
        let mut out = Vec::new();
        put_bytes(&mut out, 1, req.host.as_bytes());
        put_len_always(&mut out, 2, &req.raw);
        let file = |f: &FileDesc| {
            // FileDescriptorResponse { repeated bytes file_descriptor_proto = 1 }
            let mut r = Vec::new();
            put_len_always(&mut r, 1, &f.encode());
            r
        };
        match kind {
            RequestKind::ListServices => {
                let mut list = Vec::new();
                for name in &self.services {
                    let mut svc = Vec::new();
                    put_bytes(&mut svc, 1, name.as_bytes());
                    put_len_always(&mut list, 1, &svc);
                }
                put_len_always(&mut out, 6, &list);
            }
            RequestKind::FileByFilename(name) => {
                match self.files.iter().find(|f| f.name == *name) {
                    Some(f) => put_len_always(&mut out, 4, &file(f)),
                    None => not_found(&mut out, &format!("file not found: {name}")),
                }
            }
            RequestKind::FileContainingSymbol(symbol) => {
                let symbol = symbol.strip_prefix('.').unwrap_or(symbol);
                match self.files.iter().find(|f| f.declares(symbol)) {
                    Some(f) => put_len_always(&mut out, 4, &file(f)),
                    None => not_found(&mut out, &format!("symbol not found: {symbol}")),
                }
            }
            RequestKind::AllExtensionNumbersOfType(name) => {
                let name = name.strip_prefix('.').unwrap_or(name);
                if self.files.iter().any(|f| f.has_message(name)) {
                    // ExtensionNumberResponse { base_type_name = 1 }, no numbers.
                    let mut r = Vec::new();
                    put_bytes(&mut r, 1, name.as_bytes());
                    put_len_always(&mut out, 5, &r);
                } else {
                    not_found(&mut out, &format!("type not found: {name}"));
                }
            }
            RequestKind::FileContainingExtension(containing, number) => {
                not_found(&mut out, &format!("no extension {number} of {containing}"))
            }
        }
        Some(out)
    }
}

/// A LEN field written also when empty (a set message field).
fn put_len_always(out: &mut Vec<u8>, number: u32, v: &[u8]) {
    crate::messages::put_varint(out, (u64::from(number) << 3) | 2);
    crate::messages::put_varint(out, v.len() as u64);
    out.extend_from_slice(v);
}

/// `error_response` (7) with `NOT_FOUND`.
fn not_found(out: &mut Vec<u8>, message: &str) {
    put_len_always(out, 7, &encode_error_body(STATUS_NOT_FOUND, message));
}

/// `ErrorResponse { int32 error_code = 1; string error_message = 2; }`.
fn encode_error_body(code: i32, message: &str) -> Vec<u8> {
    let mut inner = Vec::new();
    #[allow(clippy::cast_sign_loss)]
    crate::messages::put_uint(&mut inner, 1, i64::from(code) as u64);
    put_bytes(&mut inner, 2, message.as_bytes());
    inner
}

/// Encodes a `ServerReflectionResponse` with `list_services_response` and
/// nothing else: the services of `catalog`.
#[must_use]
pub fn encode_list_services(catalog: &ServiceCatalog) -> Vec<u8> {
    let mut list = Vec::new();
    for name in catalog.fully_qualified_service_names() {
        let mut svc = Vec::new();
        put_bytes(&mut svc, 1, name.as_bytes());
        put_len_always(&mut list, 1, &svc);
    }
    let mut out = Vec::new();
    put_len_always(&mut out, 6, &list);
    out
}

/// Encodes a `ServerReflectionResponse` with `error_response` and nothing
/// else.
#[must_use]
pub fn encode_error(code: i32, message: &str) -> Vec<u8> {
    let mut out = Vec::new();
    put_len_always(&mut out, 7, &encode_error_body(code, message));
    out
}

/// `grpc/reflection/<v1|v1alpha>/reflection.proto` in `package`: the
/// messages and the service of the reflection protocol.
#[must_use]
pub fn reflection_file(package: &str) -> FileDesc {
    use FieldType::{Bytes, Int32, String as Str};
    let one = FieldDesc::one;
    let msg = FieldDesc::msg;
    let m = |name, fields, oneofs| MessageDesc {
        name,
        fields,
        oneofs,
    };
    FileDesc {
        name: format!("{}/reflection.proto", package.replace('.', "/")),
        package: package.into(),
        messages: alloc::vec![
            m(
                "ServerReflectionRequest",
                alloc::vec![
                    one("host", 1, Str),
                    one("file_by_filename", 3, Str).in_oneof(0),
                    one("file_containing_symbol", 4, Str).in_oneof(0),
                    msg("file_containing_extension", 5, "ExtensionRequest").in_oneof(0),
                    one("all_extension_numbers_of_type", 6, Str).in_oneof(0),
                    one("list_services", 7, Str).in_oneof(0),
                ],
                alloc::vec!["message_request"],
            ),
            m(
                "ExtensionRequest",
                alloc::vec![
                    one("containing_type", 1, Str),
                    one("extension_number", 2, Int32)
                ],
                Vec::new(),
            ),
            m(
                "ServerReflectionResponse",
                alloc::vec![
                    one("valid_host", 1, Str),
                    msg("original_request", 2, "ServerReflectionRequest"),
                    msg("file_descriptor_response", 4, "FileDescriptorResponse").in_oneof(0),
                    msg(
                        "all_extension_numbers_response",
                        5,
                        "ExtensionNumberResponse"
                    )
                    .in_oneof(0),
                    msg("list_services_response", 6, "ListServiceResponse").in_oneof(0),
                    msg("error_response", 7, "ErrorResponse").in_oneof(0),
                ],
                alloc::vec!["message_response"],
            ),
            m(
                "FileDescriptorResponse",
                alloc::vec![one("file_descriptor_proto", 1, Bytes).repeated()],
                Vec::new(),
            ),
            m(
                "ExtensionNumberResponse",
                alloc::vec![
                    one("base_type_name", 1, Str),
                    one("extension_number", 2, Int32).repeated()
                ],
                Vec::new(),
            ),
            m(
                "ListServiceResponse",
                alloc::vec![msg("service", 1, "ServiceResponse").repeated()],
                Vec::new(),
            ),
            m(
                "ServiceResponse",
                alloc::vec![one("name", 1, Str)],
                Vec::new()
            ),
            m(
                "ErrorResponse",
                alloc::vec![one("error_code", 1, Int32), one("error_message", 2, Str)],
                Vec::new(),
            ),
        ],
        services: alloc::vec![ServiceDesc {
            name: "ServerReflection".into(),
            methods: alloc::vec![MethodDesc {
                name: "ServerReflectionInfo",
                input: "ServerReflectionRequest",
                output: "ServerReflectionResponse",
                client_streaming: true,
                server_streaming: true,
            }],
        }],
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::service_gen::TopicService;

    fn reflection() -> Reflection {
        let mut cat = ServiceCatalog::new();
        cat.register(TopicService::from_topic("Chat::Message", "Chat::Message"));
        let mut trade = TopicService::from_topic("Trade", "Trade");
        trade.package = "acme.trade.v2".into();
        cat.register(trade);
        Reflection::of(&cat)
    }

    /// A request: `host` and the oneof member.
    fn request(host: &str, member: u32, value: &[u8]) -> Vec<u8> {
        let mut r = Vec::new();
        put_bytes(&mut r, 1, host.as_bytes());
        put_len_always(&mut r, member, value);
        r
    }

    /// The top-level fields of a response: (number, octets).
    fn top(resp: &[u8]) -> Vec<(u32, Vec<u8>)> {
        fields(resp)
            .map(|f| match f.unwrap() {
                (n, Field::Len(v)) => (n, v.to_vec()),
                (n, _) => panic!("{n}"),
            })
            .collect()
    }

    /// The `FileDescriptorProto` octets of a `file_descriptor_response`.
    fn descriptor_of(resp: &[u8]) -> Vec<u8> {
        let t = top(resp);
        let (_, fdr) = t
            .iter()
            .find(|(n, _)| *n == 4)
            .expect("file_descriptor_response");
        let inner = top(fdr);
        assert_eq!(inner.len(), 1);
        inner[0].1.clone()
    }

    #[test]
    fn grpcbridge_4_6_a_request_is_read_by_its_kind() {
        let r = decode_request(&request("h", 3, b"a.proto")).unwrap();
        assert_eq!(r.host, "h");
        assert_eq!(r.kind, Some(RequestKind::FileByFilename("a.proto".into())));
        let r = decode_request(&request("", 4, b"x.Y")).unwrap();
        assert_eq!(
            r.kind,
            Some(RequestKind::FileContainingSymbol("x.Y".into()))
        );
        let mut ext = Vec::new();
        put_bytes(&mut ext, 1, b"x.Y");
        crate::messages::put_uint(&mut ext, 2, 77);
        let r = decode_request(&request("", 5, &ext)).unwrap();
        assert_eq!(
            r.kind,
            Some(RequestKind::FileContainingExtension("x.Y".into(), 77))
        );
        let r = decode_request(&request("", 6, b"x.Y")).unwrap();
        assert_eq!(
            r.kind,
            Some(RequestKind::AllExtensionNumbersOfType("x.Y".into()))
        );
        let r = decode_request(&request("", 7, b"*")).unwrap();
        assert_eq!(r.kind, Some(RequestKind::ListServices));
        // An empty `list_services` string still selects the member.
        let r = decode_request(&request("", 7, b"")).unwrap();
        assert_eq!(r.kind, Some(RequestKind::ListServices));
        assert_eq!(decode_request(&[]).unwrap().kind, None);
        assert!(decode_request(&[0x0a, 0x05, b'a']).is_err());
    }

    /// `list_services`: the topic services in their packages and the
    /// reflection service in both names; `valid_host` and
    /// `original_request` repeat the request.
    #[test]
    fn grpcbridge_4_6_list_services_names_the_packages_of_the_mappings() {
        let raw = request("example:50051", 7, b"");
        let resp = reflection()
            .respond(&decode_request(&raw).unwrap())
            .unwrap();
        let t = top(&resp);
        assert_eq!(t[0], (1, b"example:50051".to_vec()));
        assert_eq!(t[1], (2, raw));
        assert_eq!(t[2].0, 6);
        let names: Vec<String> = top(&t[2].1)
            .into_iter()
            .map(|(_, svc)| String::from_utf8(top(&svc)[0].1.clone()).unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "zerodds.chat.v1.ChatMessageStream",
                "acme.trade.v2.TradeStream",
                "grpc.reflection.v1.ServerReflection",
                "grpc.reflection.v1alpha.ServerReflection",
            ]
        );
    }

    #[test]
    fn grpcbridge_4_6_file_containing_symbol_returns_the_descriptor_of_the_service() {
        let r = reflection();
        for symbol in [
            "zerodds.chat.v1.ChatMessageStream",
            "zerodds.chat.v1.ChatMessageStream.Subscribe",
            ".zerodds.chat.v1.Sample",
            "zerodds.chat.v1.Sample.cdr_payload",
        ] {
            let resp = r
                .respond(&decode_request(&request("", 4, symbol.as_bytes())).unwrap())
                .unwrap();
            let fd = descriptor_of(&resp);
            assert_eq!(fd, r.files()[0].encode(), "{symbol}");
        }
        let resp = r
            .respond(&decode_request(&request("", 3, b"acme/trade/v2/bridge.proto")).unwrap())
            .unwrap();
        assert_eq!(descriptor_of(&resp), r.files()[1].encode());
        let resp = r
            .respond(
                &decode_request(&request("", 4, b"grpc.reflection.v1.ServerReflection")).unwrap(),
            )
            .unwrap();
        assert_eq!(
            descriptor_of(&resp),
            reflection_file("grpc.reflection.v1").encode()
        );
    }

    #[test]
    fn grpcbridge_4_6_an_unknown_symbol_is_an_error_response() {
        let r = reflection();
        for (member, value) in [
            (4, &b"zerodds.chat.v1.Nothing"[..]),
            (4, b"zerodds.bridge.v1.ChatMessageStream"),
            (3, b"no/such.proto"),
            (6, b"zerodds.chat.v1.ChatMessageStream"),
        ] {
            let resp = r
                .respond(&decode_request(&request("", member, value)).unwrap())
                .unwrap();
            let t = top(&resp);
            let (_, err) = t.iter().find(|(n, _)| *n == 7).expect("error_response");
            let first = fields(err).next().unwrap().unwrap();
            assert!(matches!(first, (1, Field::Varint(5))), "{value:?}");
        }
        // A message type has no extensions: its name, no numbers.
        let resp = r
            .respond(&decode_request(&request("", 6, b"zerodds.chat.v1.Sample")).unwrap())
            .unwrap();
        let t = top(&resp);
        let (_, ext) = t.iter().find(|(n, _)| *n == 5).expect("extension numbers");
        assert_eq!(top(ext), [(1, b"zerodds.chat.v1.Sample".to_vec())]);
        // No oneof member: no response, the stream ends.
        assert!(r.respond(&decode_request(&[]).unwrap()).is_none());
    }

    #[test]
    fn grpcbridge_4_6_empty_catalog_yields_empty_list_services_response() {
        let cat = ServiceCatalog::new();
        assert_eq!(encode_list_services(&cat), alloc::vec![0x32, 0x00]);
    }

    #[test]
    fn grpcbridge_4_6_error_response_encodes_code_and_message() {
        let bytes = encode_error(STATUS_NOT_FOUND, "not found");
        assert_eq!(bytes[0], 0x3a);
        assert_eq!(&bytes[2..4], &[0x08, 0x05]);
        assert_eq!(&bytes[4..6], &[0x12, 9]);
        assert_eq!(&bytes[6..], b"not found");
    }

    #[test]
    fn grpcbridge_4_6_reflection_path_constant_matches_grpcurl_default() {
        assert_eq!(
            REFLECTION_PATH,
            "/grpc.reflection.v1alpha.ServerReflection/ServerReflectionInfo"
        );
        assert!(is_reflection_service(REFLECTION_SERVICE_V1));
        assert!(is_reflection_service(REFLECTION_SERVICE_V1ALPHA));
        assert!(!is_reflection_service(
            "grpc.reflection.v2.ServerReflection"
        ));
    }
}
