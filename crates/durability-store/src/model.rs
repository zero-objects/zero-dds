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
    /// XCDR representation of `payload` from the encapsulation header (RTPS 2.5
    /// §10.5): `0` = XCDR1 (CDR/PL_CDR), `1` = XCDR2 (CDR2/D_CDR2/PL_CDR2). The
    /// replay writer re-declares it so a late-joiner reads the body with the
    /// right alignment.
    pub representation: u8,
    /// Wire byte order of `payload`: `false` = little-endian, `true` =
    /// big-endian. A big-endian peer's stored sample holds big-endian body
    /// bytes; the replay writer must emit a matching `_BE` encap header
    /// (otherwise a late-joiner mis-decodes). Defaults to little-endian.
    pub big_endian: bool,
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
        if let Some(k) = self.instance_key {
            if sample.instance_key != k {
                return false;
            }
        }
        if let Some(lo) = self.seq_from {
            if sample.sequence < lo {
                return false;
            }
        }
        if let Some(hi) = self.seq_to {
            if sample.sequence > hi {
                return false;
            }
        }
        if let Some(t0) = self.time_from {
            if sample.created_at < t0 {
                return false;
            }
        }
        if let Some(t1) = self.time_to {
            if sample.created_at > t1 {
                return false;
            }
        }
        // Pagination: strictly after `(instance_key, sequence)`.
        if let Some((ck, cs)) = self.after {
            if (sample.instance_key, sample.sequence) <= (ck, cs) {
                return false;
            }
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
