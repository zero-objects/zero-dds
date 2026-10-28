// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Pure-Rust mapping between DDS topics and Zenoh key expressions, and
//! between the DDS wire (DDSI-RTPS) sample encoding and the byte layout
//! `zenoh-plugin-dds` puts on a Zenoh key. Lives without the `zenoh` dep so
//! the workspace CI can test the bridge logic without external deps.
//!
//! # Wire compatibility with `zenoh-plugin-dds` / `zenoh-bridge-dds`
//!
//! Read against tag `1.10.1` of
//! <https://github.com/eclipse-zenoh/zenoh-plugin-dds> (current release as of
//! this crate's writing; published 2026-09-07).
//!
//! * **Key expression**: `<scope>/<partition>/<topic_name>`, both `<scope>`
//!   and `<partition>` optional — `zenoh-plugin-dds/src/lib.rs` fn
//!   `topic_to_keyexpr` (around line 770). `<scope>` defaults to *empty*
//!   (config key `scope`, CLI `-s/--scope`; `zenoh-plugin-dds/src/config.rs`
//!   line 32: `pub scope: Option<OwnedKeyExpr>`, no default value function ⇒
//!   `None`). There is no fixed `"dds"` segment in the data key expression —
//!   `KE_PREFIX_DDS = "dds"` (`lib.rs` line 118) is only used inside the
//!   plugin's *admin space* path (`@/<zenoh-id>/dds/...`), never in the
//!   routed data key. [`key_expr_for_topic`] mirrors this: `scope` is `None`
//!   by default in this crate's daemon config, matching the plugin's default
//!   layout exactly so the two bridges interoperate out of the box; the old
//!   ZeroDDS-only convention of always prefixing with `"dds"` is still
//!   reachable by setting `scope: "dds"` explicitly — it is the same
//!   parameter, just a non-default value, not a separate code path.
//! * **Payload**: the *full* CDR blob, including the 4-byte RTPS
//!   encapsulation header (representation identifier + options,
//!   DDSI-RTPS 2.5 §10.5 Table 10.3) — i.e. exactly the bytes CycloneDDS's
//!   `ddsi_serdata` carries and exactly what sits in the RTPS DATA
//!   submessage's `serializedPayload`. Confirmed by
//!   `zenoh-plugin-dds/src/dds_mgt.rs` (`DDSRawSample::data_as_slice`,
//!   `impl From<DDSRawSample> for ZBytes`, around lines 169-217 — note the
//!   *unused* sibling `payload_as_slice` which strips the header is dead
//!   code, not what is actually published) and
//!   `zenoh-plugin-dds/src/route_zenoh_dds.rs` fn `do_route_data` (around
//!   line 364), which feeds the raw Zenoh payload bytes straight into
//!   `ddsi_serdata_from_ser_iov` with no header stripped off first. ZeroDDS'
//!   own raw-sample channel (`zerodds_dcps::runtime::UserSample::Alive`)
//!   carries the body *without* that header (by design, so every bridge
//!   daemon can treat it as opaque bytes); [`encapsulation_header`] /
//!   [`decode_encapsulation_header`] add/remove it at the Zenoh boundary so
//!   the bytes actually on the wire match the plugin's.
//! * **`allow`/`deny`**: regular expressions matched against the *derived*
//!   key expression (after scope/partition are applied) —
//!   `zenoh-plugin-dds/src/lib.rs` fn `is_allowed` (around line 365). This
//!   crate uses `*`/`?` glob patterns instead of regex (the convention this
//!   workspace's other allow/deny lists use, e.g.
//!   `crates/routing-service/src/config.rs` `glob_match`) matched against the
//!   same derived key expression — see [`topic_allowed`].
//! * **Reliability / congestion**: the plugin always declares its Zenoh
//!   `Publisher` reliable at the transport level
//!   (`route_dds_zenoh.rs` around line 162) and instead chooses
//!   `CongestionControl::Block` (backpressure) vs `::Drop` from
//!   `reliable_routes_blocking` (config default `true`) *and* whether the
//!   DDS writer itself is `RELIABLE` — `lib.rs` around lines 806-810. See
//!   [`congestion_control`].
//! * **TRANSIENT_LOCAL / late joiners**: the plugin checks only
//!   `DurabilityKind::TRANSIENT_LOCAL` (not `TRANSIENT`/`PERSISTENT`) —
//!   `zenoh-plugin-dds/src/qos_helpers.rs` fn `is_transient_local`. See
//!   [`is_transient_local`]. The daemon module implements the
//!   `PublicationCache`/`FetchingSubscriber` mechanics this drives
//!   (`crates/zenoh-bridge/src/daemon/server.rs`).

extern crate alloc;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_qos::{DurabilityKind, ReliabilityKind};

/// Default Zenoh key-expression scope: none, matching `zenoh-plugin-dds`'s
/// own default (`scope: Option<OwnedKeyExpr>` with no default value
/// function). The historic ZeroDDS-only convention of a mandatory `"dds"`
/// scope remains reachable by setting `scope: Some("dds")` explicitly.
pub const DEFAULT_SCOPE: Option<&str> = None;

/// A DDS topic name, partition, or Zenoh scope segment is not a valid Zenoh
/// key-expression segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidKeyExprSegment {
    /// The rejected segment.
    pub segment: String,
    /// Why it was rejected.
    pub reason: &'static str,
}

impl core::fmt::Display for InvalidKeyExprSegment {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "`{}` is not a valid Zenoh key-expression segment: {}",
            self.segment, self.reason
        )
    }
}

/// Validates one key-expression segment (a scope, a partition, or a topic
/// name once mapped onto Zenoh). Zenoh key expressions reserve `*` and `**`
/// as wildcards, `?`/`#` for selectors, and forbid empty segments; a DDS
/// topic or partition name containing one of these would, published
/// verbatim, either fail to parse or silently mean something else (a
/// wildcard subscription instead of one topic) — so a name that does not
/// pass this check is refused rather than mangled into a different, and
/// possibly colliding, key expression. The caller (the daemon's topic
/// registration path) turns a refusal into an observability event and a
/// counter, never a dropped-without-a-trace topic.
///
/// # Errors
/// [`InvalidKeyExprSegment`] naming the offending segment and why.
pub fn validate_key_expr_segment(segment: &str) -> Result<(), InvalidKeyExprSegment> {
    if segment.is_empty() {
        return Err(InvalidKeyExprSegment {
            segment: segment.to_string(),
            reason: "empty segments are not allowed",
        });
    }
    for c in segment.chars() {
        if matches!(c, '*' | '?' | '#' | '$' | '/') {
            return Err(InvalidKeyExprSegment {
                segment: segment.to_string(),
                reason: "contains a Zenoh-reserved character (one of `*?#$/`)",
            });
        }
    }
    Ok(())
}

/// Maps a DDS topic name (with an optional partition and an optional scope)
/// to a Zenoh key expression, exactly as `zenoh-plugin-dds` does (see the
/// module docs for the source citation): `<scope>/<partition>/<topic>`, with
/// `<scope>` and `<partition>` each optional.
///
/// # Examples
/// ```
/// use zerodds_zenoh_bridge::key_expr_for_topic;
/// assert_eq!(key_expr_for_topic(None, "", "Chatter").unwrap(), "Chatter");
/// assert_eq!(
///     key_expr_for_topic(None, "robot1", "Sensor").unwrap(),
///     "robot1/Sensor"
/// );
/// assert_eq!(
///     key_expr_for_topic(Some("dds"), "", "Chatter").unwrap(),
///     "dds/Chatter"
/// );
/// ```
///
/// # Errors
/// [`InvalidKeyExprSegment`] when `scope`, `partition` (non-empty), or
/// `topic` is not a valid Zenoh key-expression segment.
pub fn key_expr_for_topic(
    scope: Option<&str>,
    partition: &str,
    topic: &str,
) -> Result<String, InvalidKeyExprSegment> {
    validate_key_expr_segment(topic)?;
    if !partition.is_empty() {
        validate_key_expr_segment(partition)?;
    }
    if let Some(scope) = scope {
        validate_key_expr_segment(scope)?;
    }
    Ok(match (scope, partition.is_empty()) {
        (Some(scope), true) => format!("{scope}/{topic}"),
        (Some(scope), false) => format!("{scope}/{partition}/{topic}"),
        (None, true) => topic.to_string(),
        (None, false) => format!("{partition}/{topic}"),
    })
}

/// Minimal `*`/`?` glob match (`*` = any run incl. empty, `?` = one char) —
/// the same convention `crates/routing-service/src/config.rs::glob_match`
/// uses. `zenoh-plugin-dds` matches `allow`/`deny` with full regular
/// expressions against the derived key expression (see the module docs);
/// this crate uses glob patterns against the same derived key expression,
/// consistent with every other allow/deny list in this workspace.
#[must_use]
pub fn glob_match(pattern: &str, text: &str) -> bool {
    /// zerodds-lint: recursion-depth 256 (glob backtracking; bounded by pattern + text length)
    fn m(p: &[u8], t: &[u8]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some(b'*') => m(&p[1..], t) || (!t.is_empty() && m(p, &t[1..])),
            Some(b'?') => !t.is_empty() && m(&p[1..], &t[1..]),
            Some(&c) => t.first() == Some(&c) && m(&p[1..], &t[1..]),
        }
    }
    m(pattern.as_bytes(), text.as_bytes())
}

/// Whether `key_expr` is allowed to be routed, per `zenoh-plugin-dds`'s
/// `is_allowed` rule (`lib.rs` around line 374): allowed unless `deny`
/// matches and `allow` (if non-empty) does not; an empty `allow` list means
/// "everything allowed" (not "nothing"), matching the plugin's `None` case.
#[must_use]
pub fn topic_allowed(key_expr: &str, allow: &[String], deny: &[String]) -> bool {
    let allowed = allow.is_empty() || allow.iter().any(|p| glob_match(p, key_expr));
    let denied = deny.iter().any(|p| glob_match(p, key_expr));
    allowed && !denied
}

/// Zenoh reliability equivalent of a DDS reliability QoS. The plugin always
/// declares its Zenoh `Publisher`/`Subscriber` reliable at the transport
/// level (`route_dds_zenoh.rs` around line 162); this exists for the
/// `Reliable`/`BestEffort` distinction ZeroDDS' own reader/writer QoS still
/// carries end to end, independent of the Zenoh transport setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZenohReliability {
    /// Reliable end-to-end (Zenoh: `Reliability::Reliable`).
    Reliable,
    /// Best-Effort (Zenoh: `Reliability::BestEffort`).
    BestEffort,
}

/// Zenoh `CongestionControl` equivalent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZenohCongestion {
    /// Block (backpressure) — used for a `RELIABLE` DDS writer when
    /// `reliable_routes_blocking` is set (the default).
    Block,
    /// Drop — used for `BEST_EFFORT` writers, or for `RELIABLE` writers when
    /// `reliable_routes_blocking` is disabled.
    Drop,
}

/// Maps a DDS reliability QoS to its Zenoh equivalent.
#[must_use]
pub fn dds_reliability_to_zenoh(reliability: ReliabilityKind) -> ZenohReliability {
    match reliability {
        ReliabilityKind::Reliable => ZenohReliability::Reliable,
        ReliabilityKind::BestEffort => ZenohReliability::BestEffort,
    }
}

/// Congestion control for a `put` re-publishing a DDS sample onto Zenoh —
/// `zenoh-plugin-dds/src/lib.rs` around lines 806-810: `Block` iff the DDS
/// writer is `RELIABLE` *and* `reliable_routes_blocking` is set (default
/// `true`); `Drop` otherwise. Blocking without limit on a slow/absent Zenoh
/// subscriber is the trade-off a `RELIABLE` writer accepts by declaring
/// itself reliable in the first place; a best-effort writer never blocks.
#[must_use]
pub fn congestion_control(reliable: bool, reliable_routes_blocking: bool) -> ZenohCongestion {
    if reliable && reliable_routes_blocking {
        ZenohCongestion::Block
    } else {
        ZenohCongestion::Drop
    }
}

/// Whether a DDS durability QoS needs the TRANSIENT_LOCAL / late-joiner
/// path (a `PublicationCache` on the DDS→Zenoh side, a `FetchingSubscriber`
/// on the Zenoh→DDS side). Mirrors `zenoh-plugin-dds/src/qos_helpers.rs`
/// fn `is_transient_local` exactly: only `TRANSIENT_LOCAL` counts, not the
/// stronger `TRANSIENT`/`PERSISTENT` (those need a durability *service*
/// ZeroDDS' own `durability-service` crate provides, not a Zenoh history
/// cache).
#[must_use]
pub fn is_transient_local(durability: DurabilityKind) -> bool {
    matches!(durability, DurabilityKind::TransientLocal)
}

// ---------------------------------------------------------------------------
// RTPS encapsulation header (DDSI-RTPS 2.5 §10.5, Table 10.3) — added at the
// DDS→Zenoh boundary and removed at the Zenoh→DDS boundary so the bytes
// actually put on a Zenoh key match `zenoh-plugin-dds`'s (see the module
// docs). ZeroDDS' own `UserSample::Alive.payload` never carries this header
// — it is purely an RTPS wire-framing concern the DCPS runtime otherwise
// adds/strips on the DDS side.
// ---------------------------------------------------------------------------

/// The extensibility of the topic's root type (XTypes 1.3 §7.2.2), which
/// picks the representation identifier: `Final`/`Appendable` are plain CDR
/// in XCDR1 (`Appendable`'s trailing-field growth needs no header bit of its
/// own there); `Appendable` in XCDR2 is `D_CDR2` (delimited, `DHEADER`
/// inside the body); `Mutable` is always a parameter-list encoding
/// (`PL_CDR`/`PL_CDR2`). A raw byte-forwarding daemon without a compiled
/// type cannot discover this from the wire alone (`representation` on
/// `crate::mapping`'s `UserSample::Alive` records only XCDR1 vs XCDR2 and
/// byte order, not the header family) — the daemon config names it per
/// topic (`qos.extensibility`, default `Final`, matching ZeroDDS' own writer
/// default; see `crates/zenoh-bridge/src/daemon/config.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Extensibility {
    /// Plain CDR/CDR2, no growth.
    #[default]
    Final,
    /// Plain CDR (XCDR1) / delimited `D_CDR2` (XCDR2, `DHEADER`).
    Appendable,
    /// Parameter-list `PL_CDR`/`PL_CDR2`.
    Mutable,
}

/// Representation identifiers, DDSI-RTPS 2.5 §10.5 Table 10.3.
pub mod representation_id {
    /// `CDR_BE` — XCDR1, plain, big-endian.
    pub const CDR_BE: u16 = 0x0000;
    /// `CDR_LE` — XCDR1, plain, little-endian.
    pub const CDR_LE: u16 = 0x0001;
    /// `PL_CDR_BE` — XCDR1, parameter list, big-endian.
    pub const PL_CDR_BE: u16 = 0x0002;
    /// `PL_CDR_LE` — XCDR1, parameter list, little-endian.
    pub const PL_CDR_LE: u16 = 0x0003;
    /// `CDR2_BE` — XCDR2, plain, big-endian.
    pub const CDR2_BE: u16 = 0x0006;
    /// `CDR2_LE` — XCDR2, plain, little-endian.
    pub const CDR2_LE: u16 = 0x0007;
    /// `D_CDR2_BE` — XCDR2, delimited, big-endian.
    pub const D_CDR2_BE: u16 = 0x0008;
    /// `D_CDR2_LE` — XCDR2, delimited, little-endian.
    pub const D_CDR2_LE: u16 = 0x0009;
    /// `PL_CDR2_BE` — XCDR2, parameter list, big-endian.
    pub const PL_CDR2_BE: u16 = 0x000a;
    /// `PL_CDR2_LE` — XCDR2, parameter list, little-endian.
    pub const PL_CDR2_LE: u16 = 0x000b;
}

/// Builds the 4-byte RTPS encapsulation header (2-byte representation
/// identifier, big-endian on the wire, + 2-byte representation options,
/// always `0x0000` here — this crate never sets the XCDR2 `DHEADER` capacity
/// hint) for a body of the given XCDR version, byte order and extensibility.
#[must_use]
pub fn encapsulation_header(xcdr2: bool, big_endian: bool, ext: Extensibility) -> [u8; 4] {
    use representation_id::{
        CDR_BE, CDR_LE, CDR2_BE, CDR2_LE, D_CDR2_BE, D_CDR2_LE, PL_CDR_BE, PL_CDR_LE, PL_CDR2_BE,
        PL_CDR2_LE,
    };
    let id = match (xcdr2, ext, big_endian) {
        (false, Extensibility::Final | Extensibility::Appendable, true) => CDR_BE,
        (false, Extensibility::Final | Extensibility::Appendable, false) => CDR_LE,
        (false, Extensibility::Mutable, true) => PL_CDR_BE,
        (false, Extensibility::Mutable, false) => PL_CDR_LE,
        (true, Extensibility::Final, true) => CDR2_BE,
        (true, Extensibility::Final, false) => CDR2_LE,
        (true, Extensibility::Appendable, true) => D_CDR2_BE,
        (true, Extensibility::Appendable, false) => D_CDR2_LE,
        (true, Extensibility::Mutable, true) => PL_CDR2_BE,
        (true, Extensibility::Mutable, false) => PL_CDR2_LE,
    };
    let [hi, lo] = id.to_be_bytes();
    [hi, lo, 0x00, 0x00]
}

/// The RTPS encapsulation header a decoded [`representation_id`] value
/// resolves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedHeader {
    /// XCDR2 (`true`) vs XCDR1 (`false`).
    pub xcdr2: bool,
    /// Big-endian (`true`) vs little-endian (`false`).
    pub big_endian: bool,
    /// The extensibility family the identifier implies.
    pub extensibility: Extensibility,
}

/// [`decode_encapsulation_header`] refused `bytes`: shorter than 4 bytes, or
/// a representation identifier that is not one of DDSI-RTPS 2.5 Table 10.3's
/// CDR/CDR2/PL_CDR/PL_CDR2/D_CDR2 values (XML representations `0x0004`/
/// `0x0005` are valid RTPS but carry no CDR body this bridge can forward as
/// DDS payload bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidEncapsulationHeader;

impl core::fmt::Display for InvalidEncapsulationHeader {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("not a valid RTPS encapsulation header (DDSI-RTPS 2.5 §10.5 Table 10.3)")
    }
}

/// Parses the leading 4-byte RTPS encapsulation header off a Zenoh payload
/// (see the module docs: `zenoh-plugin-dds` always includes it). Returns the
/// decoded header and the body that follows (`&bytes[4..]`).
///
/// # Errors
/// `InvalidEncapsulationHeader` — see its docs.
pub fn decode_encapsulation_header(
    bytes: &[u8],
) -> Result<(DecodedHeader, &[u8]), InvalidEncapsulationHeader> {
    use representation_id::{
        CDR_BE, CDR_LE, CDR2_BE, CDR2_LE, D_CDR2_BE, D_CDR2_LE, PL_CDR_BE, PL_CDR_LE, PL_CDR2_BE,
        PL_CDR2_LE,
    };
    if bytes.len() < 4 {
        return Err(InvalidEncapsulationHeader);
    }
    let id = u16::from_be_bytes([bytes[0], bytes[1]]);
    let decoded = match id {
        CDR_BE => DecodedHeader {
            xcdr2: false,
            big_endian: true,
            extensibility: Extensibility::Final,
        },
        CDR_LE => DecodedHeader {
            xcdr2: false,
            big_endian: false,
            extensibility: Extensibility::Final,
        },
        PL_CDR_BE => DecodedHeader {
            xcdr2: false,
            big_endian: true,
            extensibility: Extensibility::Mutable,
        },
        PL_CDR_LE => DecodedHeader {
            xcdr2: false,
            big_endian: false,
            extensibility: Extensibility::Mutable,
        },
        CDR2_BE => DecodedHeader {
            xcdr2: true,
            big_endian: true,
            extensibility: Extensibility::Final,
        },
        CDR2_LE => DecodedHeader {
            xcdr2: true,
            big_endian: false,
            extensibility: Extensibility::Final,
        },
        D_CDR2_BE => DecodedHeader {
            xcdr2: true,
            big_endian: true,
            extensibility: Extensibility::Appendable,
        },
        D_CDR2_LE => DecodedHeader {
            xcdr2: true,
            big_endian: false,
            extensibility: Extensibility::Appendable,
        },
        PL_CDR2_BE => DecodedHeader {
            xcdr2: true,
            big_endian: true,
            extensibility: Extensibility::Mutable,
        },
        PL_CDR2_LE => DecodedHeader {
            xcdr2: true,
            big_endian: false,
            extensibility: Extensibility::Mutable,
        },
        _ => return Err(InvalidEncapsulationHeader),
    };
    Ok((decoded, &bytes[4..]))
}

/// Topic map: bidirectional mapping DDS topic ↔ Zenoh KeyExpr.
#[derive(Debug, Default, Clone)]
pub struct TopicMap {
    entries: Vec<TopicMapEntry>,
}

/// A single topic entry.
#[derive(Debug, Clone)]
pub struct TopicMapEntry {
    /// DDS-Topic-Name.
    pub topic: String,
    /// DDS-Type-Name.
    pub type_name: String,
    /// Zenoh-Key-Expression.
    pub key_expr: String,
    /// DDS-Reliability.
    pub reliability: ReliabilityKind,
    /// DDS-Durability.
    pub durability: DurabilityKind,
    /// Whether this route forwards DDS → Zenoh.
    pub dds_to_zenoh: bool,
    /// Whether this route forwards Zenoh → DDS.
    pub zenoh_to_dds: bool,
    /// Whether the topic is keyed (has one or more `@key` fields) — a real
    /// assertion (an explicit `topics:` entry, or a resolved auto-discovered
    /// type) or this bridge's own GUID-entity-kind fallback for an
    /// auto-discovered topic whose type did not resolve (DDSI-RTPS 2.5
    /// §9.3.1.2 Table 9.1, see `crate::daemon::server::note_discovery`).
    /// Read by the daemon's `/catalog` endpoint — this is the live
    /// per-topic record it renders from, so an auto-discovered topic shows
    /// up there the same as an explicitly configured one.
    pub keyed: bool,
}

impl TopicMap {
    /// New, empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an entry.
    pub fn add(&mut self, entry: TopicMapEntry) {
        self.entries.push(entry);
    }

    /// Marks further directions of an existing entry as bridged; a
    /// direction already set stays set. `false` when no entry has `topic`.
    pub fn widen(&mut self, topic: &str, dds_to_zenoh: bool, zenoh_to_dds: bool) -> bool {
        match self.entries.iter_mut().find(|e| e.topic == topic) {
            Some(e) => {
                e.dds_to_zenoh |= dds_to_zenoh;
                e.zenoh_to_dds |= zenoh_to_dds;
                true
            }
            None => false,
        }
    }

    /// Looks up an entry by DDS topic name.
    #[must_use]
    pub fn by_topic(&self, topic: &str) -> Option<&TopicMapEntry> {
        self.entries.iter().find(|e| e.topic == topic)
    }

    /// Looks up an entry by Zenoh key expression.
    #[must_use]
    pub fn by_key_expr(&self, key_expr: &str) -> Option<&TopicMapEntry> {
        self.entries.iter().find(|e| e.key_expr == key_expr)
    }

    /// Returns all entries.
    #[must_use]
    pub fn entries(&self) -> &[TopicMapEntry] {
        &self.entries
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn key_expr_no_scope_no_partition() {
        assert_eq!(key_expr_for_topic(None, "", "Chatter").unwrap(), "Chatter");
    }

    #[test]
    fn key_expr_no_scope_with_partition() {
        assert_eq!(
            key_expr_for_topic(None, "robot1", "Sensor").unwrap(),
            "robot1/Sensor"
        );
    }

    #[test]
    fn key_expr_with_scope_no_partition() {
        assert_eq!(
            key_expr_for_topic(Some("dds"), "", "Chatter").unwrap(),
            "dds/Chatter"
        );
    }

    #[test]
    fn key_expr_with_scope_and_partition() {
        assert_eq!(
            key_expr_for_topic(Some("dds"), "robot1", "Sensor").unwrap(),
            "dds/robot1/Sensor"
        );
    }

    #[test]
    fn key_expr_rejects_wildcard_topic_instead_of_mangling_it() {
        // The old behaviour silently rewrote `*`/`?`/`[`/`]` to `_`, which
        // risked two distinct DDS topics colliding on one Zenoh key. A name
        // that cannot become a valid key expression is refused instead.
        assert!(key_expr_for_topic(None, "", "Foo*Bar").is_err());
        assert!(key_expr_for_topic(None, "", "Topic?").is_err());
        assert!(key_expr_for_topic(None, "", "").is_err());
        assert!(key_expr_for_topic(Some("a/b"), "", "T").is_err());
    }

    #[test]
    fn glob_match_star_and_question() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("robot?/Sensor", "robot1/Sensor"));
        assert!(!glob_match("robot?/Sensor", "robot12/Sensor"));
        assert!(glob_match("robot*/*", "robot1/Sensor"));
    }

    #[test]
    fn topic_allowed_empty_allow_means_everything() {
        assert!(topic_allowed("any/key", &[], &[]));
    }

    #[test]
    fn topic_allowed_deny_wins_over_no_allow() {
        let deny = alloc::vec!["secret/*".to_string()];
        assert!(!topic_allowed("secret/key", &[], &deny));
        assert!(topic_allowed("public/key", &[], &deny));
    }

    #[test]
    fn topic_allowed_allow_list_restricts() {
        let allow = alloc::vec!["robot1/*".to_string()];
        assert!(topic_allowed("robot1/Sensor", &allow, &[]));
        assert!(!topic_allowed("robot2/Sensor", &allow, &[]));
    }

    #[test]
    fn topic_allowed_allow_and_deny_combine() {
        let allow = alloc::vec!["robot*/*".to_string()];
        let deny = alloc::vec!["*/Secret".to_string()];
        assert!(topic_allowed("robot1/Sensor", &allow, &deny));
        assert!(!topic_allowed("robot1/Secret", &allow, &deny));
        assert!(!topic_allowed("other/Sensor", &allow, &deny));
    }

    #[test]
    fn reliability_mapping() {
        assert_eq!(
            dds_reliability_to_zenoh(ReliabilityKind::Reliable),
            ZenohReliability::Reliable
        );
        assert_eq!(
            dds_reliability_to_zenoh(ReliabilityKind::BestEffort),
            ZenohReliability::BestEffort
        );
    }

    #[test]
    fn congestion_control_blocks_only_reliable_and_configured_blocking() {
        assert_eq!(congestion_control(true, true), ZenohCongestion::Block);
        assert_eq!(congestion_control(true, false), ZenohCongestion::Drop);
        assert_eq!(congestion_control(false, true), ZenohCongestion::Drop);
        assert_eq!(congestion_control(false, false), ZenohCongestion::Drop);
    }

    #[test]
    fn transient_local_matches_only_that_kind() {
        assert!(is_transient_local(DurabilityKind::TransientLocal));
        assert!(!is_transient_local(DurabilityKind::Volatile));
        assert!(!is_transient_local(DurabilityKind::Transient));
        assert!(!is_transient_local(DurabilityKind::Persistent));
    }

    #[test]
    fn topic_map_lookup() {
        let mut m = TopicMap::new();
        m.add(TopicMapEntry {
            topic: "Chatter".into(),
            type_name: "std_msgs::String".into(),
            key_expr: "Chatter".into(),
            reliability: ReliabilityKind::Reliable,
            durability: DurabilityKind::Volatile,
            dds_to_zenoh: true,
            zenoh_to_dds: true,
            keyed: false,
        });
        assert!(m.by_topic("Chatter").is_some());
        assert!(m.by_key_expr("Chatter").is_some());
        assert!(m.by_topic("Unknown").is_none());
    }

    #[test]
    fn encapsulation_header_round_trips_every_combination() {
        for xcdr2 in [false, true] {
            for big_endian in [false, true] {
                for ext in [
                    Extensibility::Final,
                    Extensibility::Appendable,
                    Extensibility::Mutable,
                ] {
                    let header = encapsulation_header(xcdr2, big_endian, ext);
                    let mut buf = header.to_vec();
                    buf.extend_from_slice(b"payload");
                    let (decoded, body) = decode_encapsulation_header(&buf).unwrap();
                    assert_eq!(decoded.xcdr2, xcdr2, "{ext:?} be={big_endian}");
                    assert_eq!(decoded.big_endian, big_endian, "{ext:?} xcdr2={xcdr2}");
                    // Final and Appendable share a wire form in XCDR1 (both
                    // plain CDR); the decode side cannot and need not tell
                    // them apart there.
                    if xcdr2 || ext != Extensibility::Appendable {
                        let expect = if !xcdr2 && ext == Extensibility::Appendable {
                            Extensibility::Final
                        } else {
                            ext
                        };
                        assert_eq!(decoded.extensibility, expect);
                    }
                    assert_eq!(body, b"payload");
                }
            }
        }
    }

    #[test]
    fn encapsulation_header_known_byte_values() {
        // DDSI-RTPS 2.5 Table 10.3, spot-checked against the constants.
        assert_eq!(
            encapsulation_header(false, true, Extensibility::Final),
            [0x00, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            encapsulation_header(false, false, Extensibility::Final),
            [0x00, 0x01, 0x00, 0x00]
        );
        assert_eq!(
            encapsulation_header(false, false, Extensibility::Mutable),
            [0x00, 0x03, 0x00, 0x00]
        );
        assert_eq!(
            encapsulation_header(true, false, Extensibility::Final),
            [0x00, 0x07, 0x00, 0x00]
        );
        assert_eq!(
            encapsulation_header(true, false, Extensibility::Appendable),
            [0x00, 0x09, 0x00, 0x00]
        );
        assert_eq!(
            encapsulation_header(true, false, Extensibility::Mutable),
            [0x00, 0x0b, 0x00, 0x00]
        );
    }

    #[test]
    fn decode_encapsulation_header_rejects_short_or_unknown() {
        assert!(decode_encapsulation_header(&[0x00, 0x01, 0x00]).is_err());
        assert!(decode_encapsulation_header(&[0xff, 0xff, 0x00, 0x00]).is_err());
    }
}
