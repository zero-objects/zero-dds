// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Every discovery announcement four DDS stacks actually sent, read and
//! written back.
//!
//! `tests/fixtures/vendor-discovery/*.frames` hold RTPS datagrams off
//! the wire — Eclipse Cyclone DDS, RTI Connext, eProsima Fast DDS and
//! OpenDDS, each announcing its own participant and its own endpoints
//! through its own shipped tooling. Nothing in them was written by this
//! tree, which is the whole point: a round trip against our own encoder
//! proves only that the encoder and the decoder agree with each other,
//! and this crate has just produced an example of exactly that — its
//! writer set the DATA `D` flag implicitly and its reader never looked,
//! so neither side could see the flag was missing.
//!
//! What is checked, per announcement:
//!
//! * the datagram decodes and the announcement's `BuiltinTopicData`
//!   parses;
//! * re-encoding what we parsed and parsing that again gives the same
//!   value, so nothing is read into a field that the encoder then writes
//!   differently;
//! * **per parameter id, the value we write equals the value the vendor
//!   wrote.** This is the assertion that can see something our own
//!   round trip cannot: a PID we read loosely and re-emit in a
//!   different form is a PID a peer will read differently than we meant.
//!
//! What is *not* asserted is byte-identity of the whole parameter list.
//! DDSI-RTPS 2.5 §9.4.2.11 fixes neither the order of the parameters nor
//! which optional ones are present, and the four vendors disagree on
//! both. Comparing the lists as octet strings would fail on a difference
//! the specification permits and hide the ones it does not.
//!
//! ## The parameters we do not write back
//!
//! A vendor sends parameters this tree has no field for. Each is listed
//! per vendor below with what it is, so that a PID that *starts* being
//! dropped fails here instead of being noticed on a wire run.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
use zerodds_rtps::parameter_list::ParameterList;
use zerodds_rtps::participant_data::ParticipantBuiltinTopicData;
use zerodds_rtps::publication_data::PublicationBuiltinTopicData;
use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
use zerodds_rtps::wire_types::EntityId;

/// The encapsulation header every `*BuiltinTopicData` payload starts
/// with (§10.2).
const ENCAPSULATION_HEADER_LEN: usize = 4;

fn fixture(vendor: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/vendor-discovery")
        .join(format!("{vendor}.frames"))
}

fn hex_to_bytes(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "a hex datagram has an even length");
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

/// Which builtin writer an announcement came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Announcement {
    Participant,
    Publication,
    Subscription,
}

fn announcement_kind(writer: EntityId) -> Option<Announcement> {
    if writer == EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER
        || writer == EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER
    {
        Some(Announcement::Participant)
    } else if writer == EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER
        || writer == EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER
    {
        Some(Announcement::Publication)
    } else if writer == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER
        || writer == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER
    {
        Some(Announcement::Subscription)
    } else {
        None
    }
}

/// Splits a PL_CDR payload into its parameters, keyed by PID with the
/// must-understand bit cleared — §9.4.2.11.2 makes that bit the
/// sender's demand and not part of the identity.
fn parameters(payload: &[u8]) -> BTreeMap<u16, Vec<Vec<u8>>> {
    let little_endian = match [payload[0], payload[1]] {
        [0x00, 0x03] => true,
        [0x00, 0x02] => false,
        other => panic!("not a PL_CDR encapsulation: {other:02x?}"),
    };
    let list = ParameterList::from_bytes(&payload[ENCAPSULATION_HEADER_LEN..], little_endian)
        .expect("the vendor's parameter list parses");
    let mut out: BTreeMap<u16, Vec<Vec<u8>>> = BTreeMap::new();
    for p in &list.parameters {
        out.entry(p.id & !0x4000).or_default().push(p.value.clone());
    }
    out
}

/// What a vendor wrote for one PID and we do not, and the reverse.
type ValueDelta = (Vec<Vec<u8>>, Vec<Vec<u8>>);

/// Which announcements carry a dropped PID, and the octets it carried.
type DropSite = (BTreeSet<Announcement>, Vec<u8>);

/// One announcement's worth of evidence.
struct Checked {
    kind: Announcement,
    /// PIDs the vendor sent and we do not write back, with the octets
    /// it sent — a policy announced at its default is a different
    /// finding from one that carries a value.
    dropped: BTreeMap<u16, Vec<u8>>,
    /// PIDs both sides write, where the values differ: what the vendor
    /// wrote and we do not, and what we write and the vendor did not.
    diverged: BTreeMap<u16, ValueDelta>,
}

fn check(kind: Announcement, payload: &[u8]) -> Checked {
    let theirs = parameters(payload);
    // Parse, re-encode, parse again: our own closure has to hold before
    // a comparison against a vendor means anything.
    let ours_payload = match kind {
        Announcement::Participant => {
            let a = ParticipantBuiltinTopicData::from_pl_cdr_le(payload)
                .expect("the participant announcement parses");
            let bytes = a.to_pl_cdr_le();
            let b = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes)
                .expect("our own encoding parses");
            assert_eq!(a, b, "participant announcement is not round-trip stable");
            bytes
        }
        Announcement::Publication => {
            let a = PublicationBuiltinTopicData::from_pl_cdr_le(payload)
                .expect("the publication announcement parses");
            let bytes = a.to_pl_cdr_le().expect("our encoder writes it");
            let b = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes)
                .expect("our own encoding parses");
            assert_eq!(a, b, "publication announcement is not round-trip stable");
            bytes
        }
        Announcement::Subscription => {
            let a = SubscriptionBuiltinTopicData::from_pl_cdr_le(payload)
                .expect("the subscription announcement parses");
            let bytes = a.to_pl_cdr_le().expect("our encoder writes it");
            let b = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes)
                .expect("our own encoding parses");
            assert_eq!(a, b, "subscription announcement is not round-trip stable");
            bytes
        }
    };
    let ours = parameters(&ours_payload);

    let mut dropped = BTreeMap::new();
    let mut diverged = BTreeMap::new();
    for (pid, their_values) in &theirs {
        match ours.get(pid) {
            None => {
                dropped.insert(*pid, their_values.first().cloned().unwrap_or_default());
            }
            Some(our_values) => {
                // A PID may repeat (§9.4.2.11, the locator lists). The
                // order within one PID is the sender's, so the
                // comparison is on the set.
                let theirs_set: BTreeSet<&Vec<u8>> = their_values.iter().collect();
                let ours_set: BTreeSet<&Vec<u8>> = our_values.iter().collect();
                if theirs_set != ours_set {
                    let missing: Vec<Vec<u8>> = theirs_set
                        .difference(&ours_set)
                        .map(|v| (*v).clone())
                        .collect();
                    let added: Vec<Vec<u8>> = ours_set
                        .difference(&theirs_set)
                        .map(|v| (*v).clone())
                        .collect();
                    diverged.insert(*pid, (missing, added));
                }
            }
        }
    }
    Checked {
        kind,
        dropped,
        diverged,
    }
}

/// Reads every announcement of a vendor's capture.
fn walk(vendor: &str) -> Vec<Checked> {
    let path = fixture(vendor);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "no capture at {}: {e} — an absent fixture passes every assertion \
             it is not run against",
            path.display()
        )
    });
    let mut out = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let datagram = hex_to_bytes(line);
        let parsed = decode_datagram(&datagram).unwrap_or_else(|e| {
            panic!(
                "{}:{}: a datagram {vendor} sent does not decode: {e}",
                path.display(),
                line_no + 1
            )
        });
        for frame in parsed.submessages {
            let ParsedSubmessage::Data(d) = frame.submessage else {
                continue;
            };
            let Some(kind) = announcement_kind(d.writer_id) else {
                continue;
            };
            // A key-only DATA is the endpoint's departure and carries no
            // announcement — the D flag, which this crate started
            // reading in the same change that added this file.
            if !d.data_flag || d.serialized_payload.len() < ENCAPSULATION_HEADER_LEN {
                continue;
            }
            out.push(check(kind, &d.serialized_payload));
        }
    }
    assert!(
        !out.is_empty(),
        "{} carries no announcement",
        path.display()
    );
    out
}

/// PIDs a vendor sends that this tree does not write back, per vendor.
///
/// Each was read off the capture and then looked up, and each is here
/// with the reason it is not a defect. A PID in the capture and not in
/// this list fails the test, which is the point: the list is the record
/// of what was examined, and a new drop has not been.
fn expected_drops(vendor: &str) -> BTreeSet<u16> {
    // Each entry was read off the capture, looked up in the vendor's own
    // header, and written down with the reason it is not a defect. The
    // name is the vendor's; the octets are what the capture carried.
    let list: &[u16] = match vendor {
        "cyclone" => &[
            // PID_PROTOCOL_VERSION `02 05 00 00`, on the SEDP
            // announcements. §9.6.3.2's `DiscoveredWriterData` is
            // `DDS::PublicationBuiltinTopicData` + `WriterProxy` and has no
            // such field, and Table 9.19 maps the parameter to
            // `ParticipantProxy::protocolVersion` alone. It arrives in the
            // RTPS header of the datagram that carries the announcement
            // (§8.3.3.1), which is where this tree reads it.
            //
            // `PID_VENDORID` (0x0016) is the same case in Table 9.19 and is
            // *not* in this list: Cyclone, Fast DDS and RTI all state their
            // vendor on the endpoint announcement, and this tree writes back
            // the vendor the announcement states — that value is what says
            // whose a vendor-specific parameter is (Table 9.6), so dropping
            // it would drop the attribution of 0x8002 with it.
            0x0015,
            // PID_HISTORY / PID_RESOURCE_LIMITS (`ddsi__protocol.h:252`,
            // `:253`). DDS 1.4 §2.2.3 makes neither a requested/offered
            // policy — they bound a local cache and take no part in
            // matching, so a peer that read them could not act on them.
            0x0040, 0x0041,
            // PID_ADLINK_PARTICIPANT_VERSION_INFO (`:344`) and
            // PID_ADLINK_ENTITY_FACTORY (`:350`) — another vendor's
            // extensions, in the vendor-specific range.
            0x8007, 0x800c,
            // PID_CYCLONE_RECEIVE_BUFFER_SIZE (`:362`) — Cyclone's own
            // socket sizing, announced so a Cyclone peer can match it.
            0x8019,
        ],
        "rti" => &[
            // PID_SEND_QUEUE_SIZE / PID_RECV_QUEUE_SIZE, both `ff ff ff
            // ff`. `mig_rtps.h:2298` and `:2312` name them
            // `_DEPRECATED`, and RTI sends the unlimited sentinel.
            0x0013, 0x0018,
            // PID_PROTOCOL_VERSION `02 05 00 00` — as for Cyclone, the
            // RTPS header carries it. RTI's `PID_VENDOR_ID`
            // `01 01 00 00` is read and written back unchanged.
            0x0015,
            // PID_PARTITION on the *participant*. DDS 1.4 §2.2.3.13
            // makes PARTITION a publisher/subscriber policy, and §8.5.4
            // carries it on the endpoint announcements, where this tree
            // does read it.
            0x0029,
            // PID_GROUP_ENTITY_ID (`mig_rtps.h:1753`) — the publisher's
            // or subscriber's entity id. §8.5.4.4's `DiscoveredWriterData`
            // has `group_guid`; the entity id alone is RTI's split of it.
            0x0053,
            // PID_TYPE_CONSISTENCY (`:1849`) — XTypes 1.3 §7.6.3.4's
            // `TypeConsistencyEnforcementQosPolicy`. Now read and
            // written back on *subscriptions*
            // (`SubscriptionBuiltinTopicData::type_consistency`), where
            // the policy belongs: §7.6.3.4 gives it to the DataReader
            // and the writer's half of §7.6.3.4.2 needs it. RTI writes
            // it on the publication too, where the policy has no
            // meaning and this tree carries no field for it, so it is
            // still listed here.
            0x0074,
            // PID_BUILTIN_ENDPOINT_QOS (`:1870`) — §9.3.2.13's
            // best-effort-participant-message bit.
            0x0077,
            // PID_AVAILABLE_BUILTIN_ENDPOINTS_EX (`:1895`) — RTI's
            // extension of the 32-bit builtin-endpoint set, sent as 0.
            0x1007,
            // PID_PRODUCT_VERSION (`:1924`) `07 07 00 00` — Connext
            // 7.7.0's own version.
            0x8000,
            // PID_TYPECODE (`:1955`) — the CORBA TypeCode of the topic
            // type. XTypes 1.3 replaced it with `TypeObject`, which this
            // tree reads from PID_TYPE_INFORMATION.
            0x8004,
            // PID_EXPECTS_VIRTUAL_HB (`:1990`), PID_TRANSPORT_INFO_LIST
            // (`:2057`), PID_ENDPOINT_PROPERTY_CHANGE_EPOCH (`:2100`),
            // PID_REACHABILITY_LEASE_DURATION (`:2107`),
            // PID_VENDOR_BUILTIN_ENDPOINT_MASK (`:2114`) and
            // PID_RTI_DOMAIN_ID (`:2035`) — RTI's own extensions.
            0x8009, 0x8010, 0x8015, 0x8016, 0x8017, 0x800f,
            // PID_ENTITY_VIRTUAL_GUID (`mig_rtps.h:1939`) — a 16-octet
            // GUID under the parameter id this tree calls
            // `PID_ZERODDS_TYPE_ID`. Table 9.6 hands 0x8000..0xffff to
            // every vendor at once — "Vendor-specific ParameterId. Will
            // not be recognized by other vendors' implementations." — so
            // the id alone names no parameter, and which one it is follows
            // from the vendor the announcement states. Under RTI's
            // `01 01 00 00` it is RTI's virtual GUID: not read (the type
            // identity of a foreign announcement comes from
            // PID_TYPE_INFORMATION) and not written back, because writing
            // it would offer RTI's parameter id carrying this tree's
            // `TypeIdentifier`.
            0x8002,
        ],
        "fastdds" => &[
            // PID_TIME_BASED_FILTER, at its default (`00…00`, a zero
            // separation). §9.6.3.1: a policy absent from an announcement
            // is that policy at its default, so writing the default back
            // and omitting it say the same thing to a peer. The same for
            // PID_PARTITION, PID_USER_DATA, PID_GROUP_DATA,
            // PID_TOPIC_DATA and PID_EXPECTS_INLINE_QOS, each of which
            // Fast DDS announces as an empty sequence or a zero.
            0x0004, 0x0029, 0x002c, 0x002d, 0x002e, 0x0043,
            // PID_PROTOCOL_VERSION `02 02 00 00` — the RTPS header
            // carries it. Fast DDS's `PID_VENDOR_ID` `01 0f 00 00` is read
            // and written back unchanged.
            0x0015,
            // PID_DURABILITY_SERVICE on a *Subscription* — DDS 1.4
            // §2.2.3.5 defines the policy on `DataWriterQos` only, but Fast
            // DDS 3.6.1 sends it on its reader announcement too (measured:
            // an all-default `DurabilityServiceQosPolicy`). `Publication`
            // now decodes and writes this PID back (the standalone
            // Durability-Service daemon, ADR 0009, reads a writer's value
            // from it to derive its retention contract) — only the
            // subscription side still drops it, which this entry covers.
            0x001e,
            // PID_LIVELINESS, PID_DEADLINE and PID_LIFESPAN at their
            // infinite default (`ff ff ff 7f ff ff ff ff`). This tree leaves
            // the three out at their default, as Cyclone, RTI and OpenDDS
            // do (`PublicationBuiltinTopicData::parameters`).
            0x001b, 0x0023, 0x002b,
            // PID_TYPE_CONSISTENCY — as for RTI; the subscription's
            // copy is read and written back, the publication's is not.
            0x0074,
            // PID_KEY_HASH on the SEDP announcement itself: the MD5 of
            // the endpoint GUID that is the instance key of the builtin
            // topic. §9.6.3.3 makes it redundant with PID_ENDPOINT_GUID,
            // which this tree reads, and it is the writer's to compute
            // rather than to copy — `crates/wire/src/digest.rs`.
            0x0070,
        ],
        "opendds" => &[
            // PID_PARTICIPANT_MANUAL_LIVELINESS_COUNT (`RtpsCore.idl:288`)
            // — the participant's manual-liveliness counter, sent as 0 —
            // and PID_BUILTIN_ENDPOINT_QOS (`:322`, also 0) are read and
            // written back (`ParticipantBuiltinTopicData`).
            // PID_PARTICIPANT_BUILTIN_ENDPOINTS (`:289`) `3f fc 00 00` —
            // §9.3.2.12's *deprecated* twin of PID_BUILTIN_ENDPOINT_SET
            // (0x0058), which OpenDDS sends beside it and this tree
            // reads.
            0x0044,
            // PID_OPENDDS_PARTICIPANT_FLAGS — `PID_OPENDDS_BASE + 5`
            // (`:341`, `:346`), so 0xb005. OpenDDS's own extension.
            0xb005,
        ],
        _ => &[],
    };
    list.iter().copied().collect()
}

/// PIDs where the value we write back differs from the vendor's for a
/// reason that was looked up, with what the difference is.
///
/// This table is thinner than [`expected_drops`] on purpose. A PID both
/// sides write with different octets is the case a peer reads
/// differently than we meant, so each entry has to be a difference the
/// specification permits — or a defect, and then it says so.
fn expected_divergences(vendor: &str) -> BTreeSet<u16> {
    let list: &[u16] = match vendor {
        "rti" => &[
            // PID_DEFAULT_UNICAST_LOCATOR / PID_METATRAFFIC_UNICAST_LOCATOR
            // with `kind = 0x01000000` — `NDDS_TRANSPORT_CLASSID_SHMEM`,
            // RTI's shared-memory transport. §9.4.2.4.1 gives the kind
            // no meaning outside the vendor that assigned it, and a
            // segment of RTI's shared memory is not addressable from
            // here. The UDPv4 locators of the same PID are read and
            // written back unchanged; only the SHMEM one is absent.
            0x0031, 0x0032,
            // PID_DATA_REPRESENTATION. RTI writes twelve octets where
            // XTypes 1.3 §7.6.3.1.1 defines eight — a one-element
            // `sequence<DataRepresentationId_t>` and then four more.
            // This tree writes the sequence the specification defines.
            0x0073,
            // PID_PROPERTY_LIST (`mig_rtps.h:1725`) — RTI's
            // `dds.sys_info.*` strings: hostname, pid, executable path,
            // license terms. RTI writes the `sequence<Property_t>` of
            // DDSI-RTPS 2.5 Table 9.18; this tree reads it and writes the
            // `PropertyQosPolicy` of DDS-Security 1.2 §7.5.1.4, the same
            // properties and an empty `binary_value` behind them.
            0x0059,
        ],
        "fastdds" => &[
            // PID_DEFAULT_UNICAST_LOCATOR / PID_UNICAST_LOCATOR with
            // `kind = 16` — eProsima's `LOCATOR_KIND_SHM`. As for RTI's
            // SHMEM locator: the UDPv4 entries of the same PID come back
            // unchanged and the shared-memory one does not.
            0x0031, 0x002f,
            // PID_PROPERTY_LIST — "PARTICIPANT_TYPE"="SIMPLE", the
            // discovery-server role Fast DDS announces for its own
            // topology. As for RTI: the Table 9.18 form is read and the
            // DDS-Security form written back.
            0x0059,
        ],
        _ => &[],
    };
    list.iter().copied().collect()
}

fn report(vendor: &str) {
    let checked = walk(vendor);
    let mut all_dropped: BTreeMap<u16, DropSite> = BTreeMap::new();
    let mut failures = Vec::new();
    for c in &checked {
        for (pid, value) in &c.dropped {
            let slot = all_dropped.entry(*pid).or_default();
            slot.0.insert(c.kind);
            slot.1 = value.clone();
        }
        for (pid, (missing, added)) in &c.diverged {
            if expected_divergences(vendor).contains(pid) {
                continue;
            }
            failures.push(format!(
                "{vendor} {:?}: PID 0x{pid:04x} — {vendor} wrote and we do not: \
                 {missing:02x?}; we write and {vendor} did not: {added:02x?}",
                c.kind
            ));
        }
    }
    let expected = expected_drops(vendor);
    let unexpected: BTreeSet<u16> = all_dropped
        .keys()
        .copied()
        .filter(|p| !expected.contains(p))
        .collect();

    let mut lines = Vec::new();
    if !failures.is_empty() {
        lines.push(format!("{} value divergences:", failures.len()));
        lines.extend(failures);
    }
    for pid in &unexpected {
        let (kinds, value) = &all_dropped[pid];
        lines.push(format!(
            "{vendor}: PID 0x{pid:04x} is sent on {kinds:?} as {value:02x?} and not \
             written back — look it up and record it in `expected_drops`, or write it"
        ));
    }
    assert!(
        lines.is_empty(),
        "{vendor}: {} announcements read\n{}",
        checked.len(),
        lines.join("\n")
    );
}

#[test]
fn cyclone_announcements_read_and_write_back() {
    report("cyclone");
}

#[test]
fn rti_announcements_read_and_write_back() {
    report("rti");
}

#[test]
fn fastdds_announcements_read_and_write_back() {
    report("fastdds");
}

#[test]
fn opendds_announcements_read_and_write_back() {
    report("opendds");
}
