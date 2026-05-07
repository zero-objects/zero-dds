// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeLookup-Service — XTypes 1.3 §7.6.3.3.4.
//!
//! ## Module
//!
//! - [`TypeLookupStack`]: Single-Object-Helper (Request-Builder +
//!   Reply-Parser + Registry-Auflösung).
//! - [`server::TypeLookupServer`]: server-side Handler mit Pagination.
//! - [`client::TypeLookupClient`]: client-side Correlation-Table mit
//!   pending-Request-Cap.
//! - [`endpoints::TypeLookupEndpoints`]: 4 Builtin-Endpoint-GUIDs
//!   (TL_SVC_REQ/REPLY × WRITER/READER) + service-instance-name
//!   Formatter.
//!
//! ## Layer-Boundary an DCPS
//!
//! Dieses Modul liefert die **wire-format-vollständigen** Primitives:
//! Request-/Reply-Bytes, Server-Pagination, Client-Correlation. Die
//! Instantiierung der vier Reliable-Writer/Reader-Pairs auf den
//! `TL_SVC_*`-GUIDs liegt im DCPS-Layer
//! (`crates/dcps/src/runtime.rs` Builtin-Endpoint-Spawn-Pfad), analog
//! zu SEDP-Builtin-Endpoints.

pub mod client;
pub mod endpoints;
pub mod server;

pub use client::{
    ClientCallback, RequestId, TypeLookupClient, TypeLookupReply, hashes_to_minimal_ids,
    request_dependencies_payload, request_types_payload,
};
pub use endpoints::{
    TYPELOOKUP_TOPIC_PREFIX, TypeLookupEndpoints, format_service_instance_name,
    format_service_instance_name_short,
};
pub use server::TypeLookupServer;

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, EncodeError, Endianness};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};
use zerodds_types::error::TypeCodecError;
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_lookup::{
    ContinuationPoint, GetTypeDependenciesReply, GetTypeDependenciesRequest, GetTypesReply,
    GetTypesRequest, ReplyTypeObject,
};
use zerodds_types::type_object::TypeObject;
use zerodds_types::{EquivalenceHash, TypeIdentifier};

/// TypeLookup-Stack fuer einen lokalen Participant.
#[derive(Debug)]
pub struct TypeLookupStack {
    /// Eigener Prefix (fuer Request-/Reply-GUIDs).
    pub local_prefix: GuidPrefix,
    /// Registry von bereits empfangenen TypeObjects.
    pub registry: TypeRegistry,
    /// Zaehler fuer Request-Sequenzen (fuer Sample-Identity).
    next_request_seq: u64,
}

impl TypeLookupStack {
    /// Konstruiert einen leeren Stack.
    #[must_use]
    pub fn new(local_prefix: GuidPrefix) -> Self {
        Self {
            local_prefix,
            registry: TypeRegistry::new(),
            next_request_seq: 1,
        }
    }

    /// GUID des Request-Writers (von dem wir senden).
    #[must_use]
    pub fn request_writer_guid(&self) -> Guid {
        Guid::new(self.local_prefix, EntityId::TL_SVC_REQ_WRITER)
    }

    /// GUID des Reply-Readers (auf dem wir empfangen).
    #[must_use]
    pub fn reply_reader_guid(&self) -> Guid {
        Guid::new(self.local_prefix, EntityId::TL_SVC_REPLY_READER)
    }

    /// Baut einen `getTypes`-Request fuer die angegebenen Hashes.
    /// Return: request-Payload-Bytes + neue Sequence-ID.
    ///
    /// # Errors
    /// `EncodeError` bei zu grossen Listen.
    pub fn make_get_types_request(
        &mut self,
        hashes: &[EquivalenceHash],
        minimal: bool,
    ) -> Result<(Vec<u8>, u64), EncodeError> {
        let seq = self.next_request_seq;
        self.next_request_seq = self.next_request_seq.saturating_add(1);

        let type_ids: Vec<TypeIdentifier> = hashes
            .iter()
            .map(|h| {
                if minimal {
                    TypeIdentifier::EquivalenceHashMinimal(*h)
                } else {
                    TypeIdentifier::EquivalenceHashComplete(*h)
                }
            })
            .collect();
        let req = GetTypesRequest { type_ids };
        let mut w = BufferWriter::new(Endianness::Little);
        req.encode_into(&mut w)?;
        Ok((w.into_bytes(), seq))
    }

    /// Verarbeitet ein empfangenes `getTypes`-Reply und tragt alle
    /// enthaltenen TypeObjects in die Registry ein.
    ///
    /// # Errors
    /// Decode-Fehler oder Hash-Berechnung scheitert.
    pub fn handle_get_types_reply(&mut self, bytes: &[u8]) -> Result<usize, TypeCodecError> {
        let mut r = BufferReader::new(bytes, Endianness::Little);
        let reply = GetTypesReply::decode_from(&mut r)?;
        let mut count = 0;
        for item in reply.types {
            match item {
                ReplyTypeObject::Minimal(m) => {
                    let hash = zerodds_types::compute_hash(&TypeObject::Minimal(m.clone()))?;
                    self.registry.insert_minimal(hash, m);
                    count += 1;
                }
                ReplyTypeObject::Complete(c) => {
                    let hash = zerodds_types::compute_hash(&TypeObject::Complete(c.clone()))?;
                    self.registry.insert_complete(hash, c);
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    /// Baut einen `getTypeDependencies`-Request.
    ///
    /// # Errors
    /// Encode.
    pub fn make_get_dependencies_request(
        &self,
        hashes: &[EquivalenceHash],
        cont: ContinuationPoint,
    ) -> Result<Vec<u8>, EncodeError> {
        let req = GetTypeDependenciesRequest {
            type_ids: hashes
                .iter()
                .map(|h| TypeIdentifier::EquivalenceHashMinimal(*h))
                .collect(),
            continuation_point: cont,
        };
        let mut w = BufferWriter::new(Endianness::Little);
        req.encode_into(&mut w)?;
        Ok(w.into_bytes())
    }

    /// Decoded ein getTypeDependencies-Reply (fuer Tests / Caller).
    ///
    /// # Errors
    /// Decode.
    pub fn parse_dependencies_reply(
        &self,
        bytes: &[u8],
    ) -> Result<GetTypeDependenciesReply, TypeCodecError> {
        let mut r = BufferReader::new(bytes, Endianness::Little);
        GetTypeDependenciesReply::decode_from(&mut r)
    }

    /// Responder-Helper: baut einen `getTypes`-Reply aus der Registry.
    /// Unbekannte Hashes werden ausgelassen.
    ///
    /// # Errors
    /// Encode.
    pub fn build_get_types_reply(
        &self,
        hashes: &[EquivalenceHash],
        minimal: bool,
    ) -> Result<Vec<u8>, EncodeError> {
        let mut types = Vec::new();
        for h in hashes {
            if minimal {
                if let Some(m) = self.registry.get_minimal(h) {
                    types.push(ReplyTypeObject::Minimal(m.clone()));
                }
            } else if let Some(c) = self.registry.get_complete(h) {
                types.push(ReplyTypeObject::Complete(c.clone()));
            }
        }
        let reply = GetTypesReply { types };
        let mut w = BufferWriter::new(Endianness::Little);
        reply.encode_into(&mut w)?;
        Ok(w.into_bytes())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_types::builder::TypeObjectBuilder;
    use zerodds_types::{MinimalTypeObject, PrimitiveKind};

    fn sample_struct() -> MinimalTypeObject {
        MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| m)
                .build_minimal(),
        )
    }

    #[test]
    fn make_and_parse_get_types_roundtrip() {
        let mut stack = TypeLookupStack::new(GuidPrefix::from_bytes([1; 12]));
        let hash = zerodds_types::compute_minimal_hash(&sample_struct()).unwrap();
        let (bytes, seq) = stack.make_get_types_request(&[hash], true).unwrap();
        assert_eq!(seq, 1);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = GetTypesRequest::decode_from(&mut r).unwrap();
        assert_eq!(decoded.type_ids.len(), 1);
    }

    #[test]
    fn responder_round_trip_via_registry() {
        let mut responder = TypeLookupStack::new(GuidPrefix::from_bytes([2; 12]));
        let m = sample_struct();
        let hash = zerodds_types::compute_minimal_hash(&m).unwrap();
        responder.registry.insert_minimal(hash, m);

        let reply_bytes = responder.build_get_types_reply(&[hash], true).unwrap();

        let mut requester = TypeLookupStack::new(GuidPrefix::from_bytes([3; 12]));
        let n = requester.handle_get_types_reply(&reply_bytes).unwrap();
        assert_eq!(n, 1);
        assert!(requester.registry.get_minimal(&hash).is_some());
    }

    #[test]
    fn request_seq_increments() {
        let mut s = TypeLookupStack::new(GuidPrefix::from_bytes([0; 12]));
        let (_, s1) = s.make_get_types_request(&[], true).unwrap();
        let (_, s2) = s.make_get_types_request(&[], true).unwrap();
        assert_eq!(s1, 1);
        assert_eq!(s2, 2);
    }
}
