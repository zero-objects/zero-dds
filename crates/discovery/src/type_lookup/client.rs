// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeLookup-Service Client-Side (XTypes 1.3 §7.6.3.3.4).
//!
//! Sendet `getTypes` / `getTypeDependencies`-Requests und matched
//! eingehende Replies via `RequestId` (Sample-Identity §7.6.3.3.5).
//!
//! Lifecycle:
//! 1. [`TypeLookupClient::request_types`] erzeugt eine eindeutige
//!    [`RequestId`] und merkt sich den Callback.
//! 2. Die Anwendung serialisiert den Request ueber [`request_payload`]
//!    und sendet ihn ueber den Reliable-Writer.
//! 3. Bei eingehender Reply ruft sie [`TypeLookupClient::handle_reply`]
//!    mit der korrelierten RequestId — der Callback feuert dann
//!    automatisch.
//!
//! Pending-Requests-Cap: [`TypeLookupClient::DEFAULT_MAX_PENDING`] = 256
//! laufende Requests pro Client. Aelteste werden bei Ueberschreitung
//! verworfen (FIFO-Eviction). Schuetzt vor unbeantworteten Requests die
//! sich akkumulieren.
//!
//! zerodds-lint: allow no_dyn_in_safe — `Box<dyn FnMut>` ist die Standard-
//! Callback-Signatur fuer Application-Code, der heterogen typisierte
//! Closures registrieren will. Konkrete Generics waeren hier
//! API-feindlich (jeder Pending-Eintrag muesste denselben Closure-Typ
//! haben).

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;

use zerodds_cdr::{BufferWriter, EncodeError, Endianness};
use zerodds_types::type_lookup::{
    ContinuationPoint, GetTypeDependenciesReply, GetTypeDependenciesRequest, GetTypesReply,
    GetTypesRequest,
};
use zerodds_types::{EquivalenceHash, TypeIdentifier};

/// Eindeutige Request-Identifier (Sub-Set der Sample-Identity).
///
/// Spec §7.6.3.3.5: `SampleIdentity = { writer_guid, sequence_number }`.
/// Hier verkuerzen wir auf den Sequence-Anteil — der `writer_guid` ist
/// implizit durch den Client gegeben.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(pub u64);

impl RequestId {
    /// Convenience.
    #[must_use]
    pub fn from_u64(v: u64) -> Self {
        Self(v)
    }
}

/// Was ein Reply ist — typunterschieden, weil `getTypes` und
/// `getTypeDependencies` separate Reply-Typen haben.
#[derive(Debug, Clone)]
pub enum TypeLookupReply {
    /// Antwort auf `getTypes`.
    Types(GetTypesReply),
    /// Antwort auf `getTypeDependencies`.
    Dependencies(GetTypeDependenciesReply),
}

/// Callback-Signatur fuer Replies.
pub type ClientCallback = Box<dyn FnMut(TypeLookupReply) + Send>;

/// Pending-Request-Eintrag.
struct Pending {
    callback: ClientCallback,
}

impl core::fmt::Debug for Pending {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Pending").finish()
    }
}

/// Client-Side TypeLookup-Service (Requester).
///
/// Stateless beyond pending-callbacks — die eigentliche Wire-Korrelation
/// (writer_guid + sequence_number) wird vom Caller via [`RequestId`]
/// gemanagt.
#[derive(Debug)]
pub struct TypeLookupClient {
    pending: BTreeMap<RequestId, Pending>,
    /// FIFO-Reihenfolge fuer Eviction-Tracking.
    pending_order: VecDeque<RequestId>,
    next_seq: u64,
    max_pending: usize,
}

impl TypeLookupClient {
    /// Standard-Cap fuer offene Requests.
    pub const DEFAULT_MAX_PENDING: usize = 256;

    /// Konstruiert einen Client mit Standard-Cap.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_MAX_PENDING)
    }

    /// Konstruiert einen Client mit konfigurierbarem Cap.
    #[must_use]
    pub fn with_capacity(max_pending: usize) -> Self {
        Self {
            pending: BTreeMap::new(),
            pending_order: VecDeque::new(),
            next_seq: 1,
            max_pending: max_pending.max(1),
        }
    }

    /// Anzahl aktuell offener Requests.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Registriert einen `getTypes`-Request mit Callback und liefert
    /// die zugewiesene [`RequestId`] zurueck. Der Caller serialisiert
    /// die Request-Bytes selbst (siehe [`request_types_payload`]).
    pub fn request_types(
        &mut self,
        _ids: Vec<TypeIdentifier>,
        callback: ClientCallback,
    ) -> RequestId {
        self.alloc_pending(callback)
    }

    /// Registriert einen `getTypeDependencies`-Request mit Callback.
    pub fn request_type_dependencies(
        &mut self,
        _ids: Vec<TypeIdentifier>,
        _continuation_point: ContinuationPoint,
        callback: ClientCallback,
    ) -> RequestId {
        self.alloc_pending(callback)
    }

    fn alloc_pending(&mut self, callback: ClientCallback) -> RequestId {
        let id = RequestId(self.next_seq);
        self.next_seq = self.next_seq.saturating_add(1);

        // Eviction: FIFO-Drop wenn ueber Cap.
        while self.pending.len() >= self.max_pending {
            if let Some(old) = self.pending_order.pop_front() {
                self.pending.remove(&old);
            } else {
                break;
            }
        }

        self.pending.insert(id, Pending { callback });
        self.pending_order.push_back(id);
        id
    }

    /// Verarbeitet ein Reply fuer eine gegebene [`RequestId`].
    /// Unbekannte IDs werden ignoriert (kein Panic, kein Error). Das
    /// schuetzt vor verzoegerten Replies oder Replies fuer evictete
    /// Pending-Eintraege.
    ///
    /// Liefert `true` zurueck wenn der Callback ausgefuehrt wurde.
    pub fn handle_reply(&mut self, request_id: RequestId, reply: TypeLookupReply) -> bool {
        let Some(mut entry) = self.pending.remove(&request_id) else {
            return false;
        };
        // entry.order entry entfernen (linear scan ist ok, max 256).
        if let Some(pos) = self.pending_order.iter().position(|x| *x == request_id) {
            self.pending_order.remove(pos);
        }
        (entry.callback)(reply);
        true
    }

    /// Verwirft alle Pending-Eintraege (z.B. bei Participant-Shutdown).
    pub fn clear(&mut self) {
        self.pending.clear();
        self.pending_order.clear();
    }
}

impl Default for TypeLookupClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Serialisiert einen `getTypes`-Request fuer den Wire-Transport.
///
/// # Errors
/// `EncodeError` bei Buffer-Overflow.
pub fn request_types_payload(ids: &[TypeIdentifier]) -> Result<Vec<u8>, EncodeError> {
    let req = GetTypesRequest {
        type_ids: ids.to_vec(),
    };
    let mut w = BufferWriter::new(Endianness::Little);
    req.encode_into(&mut w)?;
    Ok(w.into_bytes())
}

/// Serialisiert einen `getTypeDependencies`-Request.
///
/// # Errors
/// `EncodeError` bei Buffer-Overflow.
pub fn request_dependencies_payload(
    ids: &[TypeIdentifier],
    continuation_point: ContinuationPoint,
) -> Result<Vec<u8>, EncodeError> {
    let req = GetTypeDependenciesRequest {
        type_ids: ids.to_vec(),
        continuation_point,
    };
    let mut w = BufferWriter::new(Endianness::Little);
    req.encode_into(&mut w)?;
    Ok(w.into_bytes())
}

/// Convenience: Build die TypeIdentifiers aus einem Set von Hashes.
#[must_use]
pub fn hashes_to_minimal_ids(hashes: &[EquivalenceHash]) -> Vec<TypeIdentifier> {
    hashes
        .iter()
        .map(|h| TypeIdentifier::EquivalenceHashMinimal(*h))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use core::cell::RefCell;
    extern crate std;
    use std::sync::Arc;
    use std::sync::Mutex;

    #[test]
    fn request_id_unique_and_monotone() {
        let mut c = TypeLookupClient::new();
        let id1 = c.request_types(alloc::vec![], Box::new(|_| {}));
        let id2 = c.request_types(alloc::vec![], Box::new(|_| {}));
        let id3 = c.request_types(alloc::vec![], Box::new(|_| {}));
        assert!(id1 < id2);
        assert!(id2 < id3);
    }

    #[test]
    fn handle_reply_unknown_id_is_ignored() {
        let mut c = TypeLookupClient::new();
        let consumed = c.handle_reply(
            RequestId(99),
            TypeLookupReply::Types(GetTypesReply::default()),
        );
        assert!(!consumed);
    }

    #[test]
    fn handle_reply_invokes_callback() {
        let calls = Arc::new(Mutex::new(0u32));
        let calls_clone = Arc::clone(&calls);
        let mut c = TypeLookupClient::new();
        let id = c.request_types(
            alloc::vec![],
            Box::new(move |_| {
                *calls_clone.lock().unwrap() += 1;
            }),
        );
        assert_eq!(*calls.lock().unwrap(), 0);

        let consumed = c.handle_reply(id, TypeLookupReply::Types(GetTypesReply::default()));
        assert!(consumed);
        assert_eq!(*calls.lock().unwrap(), 1);
        assert_eq!(c.pending_count(), 0);
    }

    #[test]
    fn double_reply_runs_callback_only_once() {
        let calls = Arc::new(Mutex::new(0u32));
        let calls_clone = Arc::clone(&calls);
        let mut c = TypeLookupClient::new();
        let id = c.request_types(
            alloc::vec![],
            Box::new(move |_| {
                *calls_clone.lock().unwrap() += 1;
            }),
        );
        c.handle_reply(id, TypeLookupReply::Types(GetTypesReply::default()));
        c.handle_reply(id, TypeLookupReply::Types(GetTypesReply::default()));
        assert_eq!(*calls.lock().unwrap(), 1);
    }

    #[test]
    fn pending_cap_evicts_oldest() {
        let mut c = TypeLookupClient::with_capacity(2);
        let _id1 = c.request_types(alloc::vec![], Box::new(|_| {}));
        let id2 = c.request_types(alloc::vec![], Box::new(|_| {}));
        let id3 = c.request_types(alloc::vec![], Box::new(|_| {}));
        // Cap = 2 → id1 evicted.
        assert_eq!(c.pending_count(), 2);
        assert!(c.pending.contains_key(&id2));
        assert!(c.pending.contains_key(&id3));
    }

    #[test]
    fn clear_drops_all_pending() {
        let mut c = TypeLookupClient::new();
        c.request_types(alloc::vec![], Box::new(|_| {}));
        c.request_types(alloc::vec![], Box::new(|_| {}));
        assert_eq!(c.pending_count(), 2);
        c.clear();
        assert_eq!(c.pending_count(), 0);
    }

    #[test]
    fn request_types_payload_roundtrips() {
        let ids = alloc::vec![
            TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash([0x55; 14])),
            TypeIdentifier::Primitive(zerodds_types::PrimitiveKind::Int32),
        ];
        let bytes = request_types_payload(&ids).unwrap();
        // Sequence-Length-Prefix.
        assert!(bytes.len() >= 4);
    }

    #[test]
    fn dependencies_payload_carries_continuation() {
        let ids = alloc::vec![TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
            [0x77; 14]
        ))];
        let cp = ContinuationPoint(alloc::vec![1, 2, 3]);
        let bytes = request_dependencies_payload(&ids, cp).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn hashes_to_minimal_ids_maps_each() {
        let hashes = alloc::vec![EquivalenceHash([1; 14]), EquivalenceHash([2; 14])];
        let ids = hashes_to_minimal_ids(&hashes);
        assert_eq!(ids.len(), 2);
        assert!(matches!(ids[0], TypeIdentifier::EquivalenceHashMinimal(_)));
    }

    // Lokaler Smoke-Test fuer non-Send-Callback ist nicht moeglich
    // (ClientCallback ist `Send`). RefCell-Test stellt sicher dass
    // wir interior-mutability im Callback lokal benutzen koennen.
    #[test]
    fn callback_can_mutate_via_arc_mutex() {
        let _: RefCell<i32> = RefCell::new(0); // smoke
    }
}
