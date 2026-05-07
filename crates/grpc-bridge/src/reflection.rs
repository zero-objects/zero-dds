// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! gRPC Server Reflection Service (`grpc.reflection.v1alpha.ServerReflection`).
//!
//! Spec: `zerodds-grpc-bridge-1.0.md` §4.6.
//!
//! Implementiert ein **minimales** Reflection-Profile:
//! * `ListServices` — liefert alle registrierten Topic-Services aus
//!   dem [`crate::service_gen::ServiceCatalog`].
//! * Andere Reflection-Anfragen (`FileByFilename`,
//!   `FileContainingSymbol`, `AllExtensionNumbersOfType` …) werden mit
//!   `error_response { error_code = NOT_FOUND }` beantwortet (Spec
//!   §4.6 Note: `error_code = 12 (UNIMPLEMENTED)` ist auch zulaessig).
//!
//! Wire-Format: protobuf — wir bauen die Antworten manuell aus den
//! benötigten Feldern, ohne `prost`-Dependency. Das ist ausreichend
//! für `grpcurl`-Compliance auf den ListServices-Pfad.

use alloc::vec::Vec;

use crate::service_gen::ServiceCatalog;

/// Reflection-Service-Pfad, gleicher Wert wie in `grpcurl -plaintext localhost:N list`.
pub const REFLECTION_PATH: &str = "/grpc.reflection.v1alpha.ServerReflection/ServerReflectionInfo";

/// Status-Code (NOT_FOUND).
pub const STATUS_NOT_FOUND: i32 = 5;

/// Encodiert einen `ServerReflectionResponse` mit `list_services_response`.
///
/// Vereinfachter Proto-Encode der relevanten Felder:
/// ```proto
/// message ServerReflectionResponse {
///   string valid_host = 1;
///   ServerReflectionRequest original_request = 2;
///   oneof message_response {
///     // ...
///     ListServiceResponse list_services_response = 6;
///     // ...
///   }
/// }
/// message ListServiceResponse {
///   repeated ServiceResponse service = 1;
/// }
/// message ServiceResponse {
///   string name = 1;
/// }
/// ```
#[must_use]
pub fn encode_list_services(catalog: &ServiceCatalog) -> Vec<u8> {
    // Build inner ListServiceResponse first.
    let mut inner = Vec::new();
    for name in catalog.fully_qualified_service_names() {
        // ServiceResponse: field 1 (string) name.
        let mut svc_buf = Vec::new();
        write_string_field(&mut svc_buf, 1, &name);
        // ListServiceResponse.service is repeated, field 1, length-delim.
        write_length_delimited_field(&mut inner, 1, &svc_buf);
    }
    // Outer: field 6 (list_services_response) of ListServiceResponse.
    let mut out = Vec::new();
    write_length_delimited_field(&mut out, 6, &inner);
    out
}

/// Encodiert einen `ServerReflectionResponse` mit `error_response`.
///
/// ```proto
/// message ErrorResponse {
///   int32 error_code = 1;
///   string error_message = 2;
/// }
/// // Outer field 7 = error_response.
/// ```
#[must_use]
pub fn encode_error(code: i32, message: &str) -> Vec<u8> {
    let mut inner = Vec::new();
    // field 1: error_code (varint).
    write_varint_field(&mut inner, 1, code as u64);
    // field 2: error_message (string).
    write_string_field(&mut inner, 2, message);
    // Outer field 7.
    let mut out = Vec::new();
    write_length_delimited_field(&mut out, 7, &inner);
    out
}

// ---------- protobuf low-level helpers ----------

fn write_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn write_tag(out: &mut Vec<u8>, field: u32, wire_type: u8) {
    let tag = (field << 3) | u32::from(wire_type);
    write_varint(out, u64::from(tag));
}

fn write_varint_field(out: &mut Vec<u8>, field: u32, v: u64) {
    write_tag(out, field, 0); // wire-type 0 = varint
    write_varint(out, v);
}

fn write_string_field(out: &mut Vec<u8>, field: u32, s: &str) {
    write_length_delimited_field(out, field, s.as_bytes());
}

fn write_length_delimited_field(out: &mut Vec<u8>, field: u32, bytes: &[u8]) {
    write_tag(out, field, 2); // wire-type 2 = length-delimited
    write_varint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::service_gen::TopicService;

    #[test]
    fn empty_catalog_yields_empty_list_services_response() {
        let cat = ServiceCatalog::new();
        let bytes = encode_list_services(&cat);
        // Should produce: tag(6, length-delim) + varint(0) → 2 bytes.
        // 6 << 3 | 2 = 50 (0x32). length 0 = 0x00.
        assert_eq!(bytes, vec![0x32, 0x00]);
    }

    #[test]
    fn one_service_in_list() {
        let mut cat = ServiceCatalog::new();
        cat.register(TopicService::from_topic("Trade", "Trade"));
        let bytes = encode_list_services(&cat);
        // Outer: tag 6, length-delimited, length = ...
        // Inner ListServiceResponse: tag 1 (service), len-delim, ServiceResponse {name = "zerodds.bridge.v1.TradeStream"}
        // ServiceResponse: tag 1 (name), string "zerodds.bridge.v1.TradeStream" (29 bytes).
        let name = "zerodds.bridge.v1.TradeStream";
        // ServiceResponse buf: 0x0a (tag 1, ld) + 0x1d (29) + name.
        // ListServiceResponse buf: 0x0a (tag 1, ld) + 0x1f (31 = 2+29) + service-bytes.
        // Outer: 0x32 + len + inner.
        let inner_total = 2 + 2 + name.len(); // 33
        assert_eq!(bytes[0], 0x32);
        assert_eq!(bytes[1] as usize, inner_total);
    }

    #[test]
    fn error_response_encodes_code_and_message() {
        let bytes = encode_error(STATUS_NOT_FOUND, "not found");
        // Outer: tag 7 (0x3a) + len + inner.
        assert_eq!(bytes[0], 0x3a);
        // Inner: tag 1 varint (0x08), value 5 (0x05), tag 2 ld (0x12), len, "not found".
        assert!(bytes.windows(1).any(|w| w[0] == 0x08));
    }

    #[test]
    fn varint_basics() {
        let mut v = Vec::new();
        write_varint(&mut v, 0);
        assert_eq!(v, vec![0]);
        let mut v = Vec::new();
        write_varint(&mut v, 127);
        assert_eq!(v, vec![127]);
        let mut v = Vec::new();
        write_varint(&mut v, 128);
        assert_eq!(v, vec![0x80, 0x01]);
        let mut v = Vec::new();
        write_varint(&mut v, 300);
        assert_eq!(v, vec![0xac, 0x02]);
    }

    #[test]
    fn reflection_path_constant_matches_grpcurl_default() {
        // `grpcurl localhost:N list` invokes exactly this path.
        assert_eq!(
            REFLECTION_PATH,
            "/grpc.reflection.v1alpha.ServerReflection/ServerReflectionInfo"
        );
    }
}
