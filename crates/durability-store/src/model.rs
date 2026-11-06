// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Data model at the store boundary: samples, selectors, paginated pages.

use alloc::string::String;
use alloc::vec::Vec;
use std::time::SystemTime;

/// A stored durability sample. The `(instance_key, sequence)` pair is the
/// stable ordering key within a topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurabilitySample {
    /// Topic name.
    pub topic: String,
    /// Instance KeyHash (RTPS §9.6.4.8, 16 bytes).
    pub instance_key: [u8; 16],
    /// Writer-assigned monotonic sequence number.
    pub sequence: u64,
    /// Encoded payload (XCDR2 serialized body).
    pub payload: Vec<u8>,
    /// The representation identifier of the encapsulation header `payload`
    /// came under (DDSI-RTPS 2.5 §10.5, DDS-XTypes 1.3 §7.6.3.1.2 Table 60):
    /// the XCDR version, the format (plain, delimited, parameter list) and
    /// the byte order of the body. The replay writer sends the body under
    /// the same header, so that a late joiner reads it as it was written.
    /// A store keeps the CDR forms of Table 60, whose first octet is `0x00`
    /// (see [`identifier_columns`]).
    pub representation_identifier: [u8; 2],
    /// Source/creation wall-clock timestamp.
    pub created_at: SystemTime,
    /// GUID of the writer that originally emitted this sample (16 bytes). With
    /// `source_sequence` this is the globally-unique source identity a
    /// durability service uses to dedup a writer's history against its live
    /// stream, and across a service restart (O2 P5). All-zero when unknown.
    pub source_guid: [u8; 16],
    /// The source writer's RTPS sequence number for this sample (DDSI-RTPS
    /// §8.3.5.4). `-1` when the ingest path had no source sequence (intra-runtime
    /// or pre-P5 stored samples), in which case it does not participate in
    /// source-identity dedup.
    pub source_sequence: i64,
}

/// Pagination cursor: the `(instance_key, sequence)` of the last sample of a
/// [`Page`]. The next page starts strictly after this key in
/// `(instance_key, sequence)` lexicographic order.
pub type Cursor = ([u8; 16], u64);

/// A read selector for [`crate::DurabilityStore::query`]. All filters are
/// optional and combine with AND. `after` + `limit` drive pagination.
///
/// An empty selector (`Selector::default()`) plus paging therefore yields the
/// full contracted history for a topic, ordered by `(instance_key, sequence)`.
#[derive(Debug, Clone, Default)]
pub struct Selector {
    /// Restrict to a single instance.
    pub instance_key: Option<[u8; 16]>,
    /// Inclusive lower sequence bound.
    pub seq_from: Option<u64>,
    /// Inclusive upper sequence bound.
    pub seq_to: Option<u64>,
    /// Inclusive lower `created_at` bound.
    pub time_from: Option<SystemTime>,
    /// Inclusive upper `created_at` bound.
    pub time_to: Option<SystemTime>,
    /// Return only samples strictly after this `(instance_key, sequence)`.
    pub after: Option<Cursor>,
    /// Maximum samples in the returned page. `None` = adapter default.
    pub limit: Option<usize>,
}

impl Selector {
    /// Builds the selector for the next page after a returned [`Cursor`],
    /// preserving the filter predicate but advancing the pagination anchor.
    #[must_use]
    pub fn after_cursor(&self, cursor: Cursor) -> Self {
        Self {
            after: Some(cursor),
            ..self.clone()
        }
    }

    /// Returns `true` if `sample` matches the filter predicate (ignores
    /// `after`/`limit`, which are pagination, not predicate).
    #[must_use]
    pub fn matches(&self, sample: &DurabilitySample) -> bool {
        if let Some(k) = self.instance_key
            && sample.instance_key != k
        {
            return false;
        }
        if let Some(lo) = self.seq_from
            && sample.sequence < lo
        {
            return false;
        }
        if let Some(hi) = self.seq_to
            && sample.sequence > hi
        {
            return false;
        }
        if let Some(t0) = self.time_from
            && sample.created_at < t0
        {
            return false;
        }
        if let Some(t1) = self.time_to
            && sample.created_at > t1
        {
            return false;
        }
        // Pagination: strictly after `(instance_key, sequence)`.
        if let Some((ck, cs)) = self.after
            && (sample.instance_key, sample.sequence) <= (ck, cs)
        {
            return false;
        }
        true
    }
}

/// One page of a paginated [`crate::DurabilityStore::query`] result. Samples
/// are ordered by `(instance_key, sequence)`. `next` is `Some` when more
/// samples may follow — feed it back via [`Selector::after_cursor`].
#[derive(Debug, Clone, Default)]
pub struct Page {
    /// The samples in this page, ordered by `(instance_key, sequence)`.
    pub samples: Vec<DurabilitySample>,
    /// Cursor to continue after, or `None` when the result is exhausted.
    pub next: Option<Cursor>,
}

/// Aggregate counters for a topic (diagnostics, limit accounting, tiering).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StoreStats {
    /// Number of stored samples for the topic.
    pub samples: usize,
    /// Number of distinct instances for the topic.
    pub instances: usize,
    /// Total payload bytes stored for the topic.
    pub bytes: u64,
}

/// The two columns a store keeps a CDR representation identifier in: the
/// second octet without its byte-order bit (`0x00` CDR, `0x02` PL_CDR, `0x06`
/// CDR2, `0x08` D_CDR2, `0x0a` PL_CDR2; DDS-XTypes 1.3 Table 60) and whether
/// the body is big endian (the bit clear). The first octet is `0x00` for
/// every CDR form and is not kept.
#[must_use]
pub fn identifier_columns(identifier: [u8; 2]) -> (u8, bool) {
    (identifier[1] & !0x01, identifier[1] & 0x01 == 0)
}

/// The representation identifier of [`identifier_columns`]. A store written
/// before the identifier was kept holds `1` for any XCDR2 body and `0` for
/// any XCDR1 body; `1` reads as CDR2 (`0x06`), `0` as CDR, which is how such
/// a sample was replayed then.
#[must_use]
pub fn identifier_from_columns(representation: u8, big_endian: bool) -> [u8; 2] {
    let kind = if representation == 1 {
        0x06
    } else {
        representation & !0x01
    };
    [0x00, kind | u8::from(!big_endian)]
}

#[cfg(test)]
mod tests {
    use super::{identifier_columns, identifier_from_columns};

    /// Every CDR form of DDS-XTypes 1.3 Table 60 in both byte orders comes
    /// back from its columns as it went in.
    #[test]
    fn xtypes_7_6_3_1_2_every_cdr_identifier_comes_back_from_its_columns() {
        for kind in [0x00, 0x02, 0x06, 0x08, 0x0a] {
            for identifier in [[0x00, kind], [0x00, kind | 0x01]] {
                let (representation, big_endian) = identifier_columns(identifier);
                assert_eq!(
                    identifier_from_columns(representation, big_endian),
                    identifier,
                    "{identifier:02x?}"
                );
            }
        }
        // CDR_BE is big endian, D_CDR2_LE little endian.
        assert_eq!(identifier_columns([0x00, 0x00]), (0x00, true));
        assert_eq!(identifier_columns([0x00, 0x09]), (0x08, false));
    }

    /// A store written before kept `0` for XCDR1 and `1` for XCDR2: those
    /// read as CDR and CDR2, in the byte order of their column.
    #[test]
    fn the_columns_of_an_earlier_store_read_as_cdr_and_cdr2() {
        assert_eq!(identifier_from_columns(0, false), [0x00, 0x01]);
        assert_eq!(identifier_from_columns(1, false), [0x00, 0x07]);
        assert_eq!(identifier_from_columns(1, true), [0x00, 0x06]);
    }
}
