// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Per-peer RTPS behaviour: the axes on which measured implementations
//! disagree about a **submessage**, as opposed to about the CDR inside it.
//!
//! # Why this is not `zerodds_ast_layout::LayoutKey`
//!
//! `LayoutKey` answers "how is this value serialised" — encoding, byte
//! order, length codes, delimiters, `wstring` framing, PL_CDR1 headers,
//! parameter-id flags, enumeration width. Every one of its fields is a
//! decision about the octets of a sample or a key holder, it is chosen per
//! **type**, and it is chosen at code-generation time.
//!
//! The axes here are none of those things. They decide which submessage
//! flags a discovery DATA carries and which inline-QoS parameters go with
//! it — not how a value is encoded but which shape of message names it.
//! They are chosen per **peer**, at run time, because a peer is what has a
//! vendor.
//!
//! So the two tables sit apart, and their **defaults differ in kind** — a
//! reader who sees them next to each other has to be able to tell why:
//!
//! * `LayoutKey`'s default is `zerodds_ast_layout::Dialect::Specification`:
//!   the clause-by-clause reading of XTypes 1.3, which no measured
//!   implementation produces in full. There the specification does pick a
//!   value, and a dialect is the deliberate choice to depart from it.
//! * Here the specification does **not** pick a value.
//!   §9.4.5.3 gives `D` and `K` two mutually exclusive
//!   readings of `serializedPayload` and permits either to be clear;
//!   §9.6.3.9 puts the dispose/unregister semantics in `PID_STATUS_INFO`,
//!   which both readings carry. A `K=1` DATA with a serialized key and a
//!   `K=0` DATA with `PID_KEY_HASH` alone are therefore **both** conforming
//!   dispose markers, and no clause prefers one. There is no
//!   "specification reading" to default to.
//!
//! That is why the default here is named for what it does —
//! [`RtpsDialect::MaximallyInteroperable`] — and not `Specification`. It is
//! not a clause; it is the measured form that every peer accepts, and the
//! name has to say so rather than borrow authority the clauses do not give.
//!
//! [`SpdpResponseTargets`] is the one axis here where a clause does derive a
//! value, and the default departs from it deliberately. §8.5.2 and §8.5.4
//! build the builtin endpoints of a discovered participant from the locator
//! lists its `SPDPdiscoveredParticipantData` announced; the address a
//! datagram arrived from is not one of those lists, and no clause names it.
//! So [`SpdpResponseTargets::AnnouncedLocatorsOnly`] is the clause-derived
//! value — and it does not reach OpenDDS 3.34, which announces its rtps_udp
//! transport port there and keeps its SPDP reader on a port it announces
//! nowhere. The default is the wider form for the same reason as everywhere
//! else in this table: it is the one every measured implementation accepts.
//! The clause-derived value keeps its place as the other value on the axis,
//! and the axis says which is which.
//!
//! # Configured, never inferred
//!
//! Every RTPS header carries a `vendorId`, so a writer could switch form by
//! itself. It does not. A dialect is the deliberate choice to carry a
//! peer's defect while talking to it, and switching on a wire field would
//! make that choice without anyone taking it — and would make the octets on
//! the wire depend on a value a remote can set. The per-participant default
//! applies to every peer; an override is per peer and always explicit.
//!
//! An application may name a vendor instead of a peer
//! (`DcpsRuntime::set_vendor_rtps_behaviour` in `zerodds-dcps`): then the
//! application takes the choice, for every participant that announces that
//! `vendorId`. The OMG dds-rtps test client does so for the GROUP wire form
//! ([`GroupWireForm`]); the runtime's defaults never do.
//!
//! This covers the behaviours of this table. Elsewhere the stack does read a
//! peer's `vendorId`, each time for a field or a format whose meaning the
//! vendor fixes rather than the specification:
//!
//! - vendor-specific PIDs (the `0x8000` range) are read as eProsima's only
//!   from a Fast DDS or ZeroDDS participant (`DcpsRuntime::speaks_eprosima_pids`);
//!   DDSI-RTPS 2.5 §9.6.2.2.1 makes their meaning the vendor's;
//! - the authentication request NUL-terminates `c.dsign_algo` and
//!   `c.kagree_algo` for an OpenDDS peer, which compares them with `sizeof`
//!   (`crates/discovery/src/security/stack.rs`);
//! - a ROS 2 service correlates a request from Cyclone DDS by its
//!   in-payload header, which `rmw_cyclonedds_cpp` writes instead of a
//!   related sample identity (`crates/rmw-zerodds-shim/src/services.rs`).

use crate::wire_types::Guid;

/// Where a dispose/unregister marker names its instance.
///
/// All three values were measured, each from one implementation deleting a
/// `DataReader` on an isolated domain, read off the DATA its
/// `ENTITYID_BUILTIN_SUBSCRIPTIONS_WRITER` sent with
/// `PID_STATUS_INFO = 0x00000003`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DisposeKeyForm {
    /// `K=1`, `serializedPayload` holds the key, and the inline QoS carries
    /// `PID_STATUS_INFO` **only** — no `PID_KEY_HASH`.
    ///
    /// Cyclone DDS 11.0.1 sends this: flags `0x0b`, `octetsToNextHeader` 60,
    /// key = `PL_CDR_LE` + `PID_ENDPOINT_GUID` + `PID_SENTINEL`.
    ///
    /// A receiver that reads only `PID_KEY_HASH` learns nothing from this
    /// form, which is why [`crate::parameter_list::sedp_endpoint_key_pl_cdr_le`]
    /// exists on the read side too.
    KeyInPayload,

    /// `K=1` with the key in `serializedPayload` **and** `PID_KEY_HASH` in
    /// the inline QoS — both identities on the wire at once.
    ///
    /// OpenDDS 3.34.0 sends this: flags `0x0b`, inline QoS
    /// `PID_STATUS_INFO` + `PID_KEY_HASH`, same twenty-eight key octets as
    /// Cyclone.
    ///
    /// The default, and the reason is the measurement rather than a clause:
    /// it is the only one of the three forms that satisfies a receiver
    /// reading either identity, so all three measured implementations accept
    /// it. See this module's header for why that does not make it a
    /// "specification" default.
    #[default]
    KeyInPayloadWithHash,

    /// `K=0`, no `serializedPayload` at all, instance named by
    /// `PID_KEY_HASH` alone.
    ///
    /// Fast DDS 3.6.1 sends this: flags `0x03`, `octetsToNextHeader` 52.
    ///
    /// Also what a writer falls back to when it holds no key serialization
    /// — see [`DisposeKeyForm::with_key_available`]. `K=1` over zero octets
    /// is not among the values here, because it is not a form any
    /// implementation sends and Cyclone's `validate_Data`
    /// (`ddsi_receive.c:432`) discards the whole datagram that carries it.
    HashOnly,
}

impl DisposeKeyForm {
    /// True when the form puts the key in `serializedPayload`, i.e. sets `K`.
    #[must_use]
    pub const fn sets_key_flag(self) -> bool {
        matches!(self, Self::KeyInPayload | Self::KeyInPayloadWithHash)
    }

    /// True when the form puts `PID_KEY_HASH` in the inline QoS.
    #[must_use]
    pub const fn emits_key_hash(self) -> bool {
        matches!(self, Self::KeyInPayloadWithHash | Self::HashOnly)
    }

    /// The form actually usable when the caller has `key_available` key
    /// octets or does not.
    ///
    /// A caller without a key serialization cannot send one, and
    /// [`Self::KeyInPayload`] would then name the instance **nowhere**: no
    /// payload to read a key from and no `PID_KEY_HASH` either. That is a
    /// marker a receiver cannot act on, so it degrades to
    /// [`Self::HashOnly`], which still carries the hash. The degradation is
    /// deliberate and narrow — it never drops an identity, it picks the one
    /// that is present.
    #[must_use]
    pub const fn with_key_available(self, key_available: bool) -> Self {
        if key_available { self } else { Self::HashOnly }
    }
}

/// Whether every datagram states its destination `GuidPrefix`.
///
/// This axis was in the tree before this module was, as
/// `ReliableWriter::set_emit_info_dst(bool)` — a boolean named after the
/// submessage it emits and justified by the name of one vendor's endpoint.
/// It is a dialect axis and belongs in the same table as the others, with a
/// name that says what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DirectedAddressing {
    /// No `INFO_DST`. The receiver resolves the submessage by its
    /// `readerId` alone, and `Receiver.destGuidPrefix` stays
    /// `GUIDPREFIX_UNKNOWN` (§8.3.8.8).
    #[default]
    ByEntityIdOnly,

    /// Every datagram opens with `INFO_DST` carrying the target's twelve
    /// prefix octets.
    ///
    /// Required by a receiver that matches a directed submessage on the
    /// full GUID rather than on the `readerId`: Cyclone's filtered
    /// `VolatileSecure` reader does, and without the prefix it resolves
    /// `dst = 0:0:0:<eid>`, matches nothing, and never advances
    /// `wn->last_seq` — after which `ddsi_reorder_nackmap` sits on a
    /// `maxseq - 0` request that neither side can close.
    PrefixEveryDatagram,
}

impl DirectedAddressing {
    /// True when a datagram opens with `INFO_DST`.
    #[must_use]
    pub const fn emits_info_dst(self) -> bool {
        matches!(self, Self::PrefixEveryDatagram)
    }

    /// The value the migrated `set_emit_info_dst(bool)` maps to.
    #[must_use]
    pub const fn from_emit_info_dst(emit: bool) -> Self {
        if emit {
            Self::PrefixEveryDatagram
        } else {
            Self::ByEntityIdOnly
        }
    }
}

/// Where the directed SPDP response to a newly discovered participant goes.
///
/// A participant that hears a beacon it has not heard before answers with its
/// own beacon straight away, instead of leaving the peer to wait for the next
/// periodic multicast — up to `spdp_period`, which is 5 s here and longer than
/// the deadline a first endpoint match usually gets. This axis is the address
/// that answer is sent to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SpdpResponseTargets {
    /// The locators the peer announced as its metatraffic unicast locators,
    /// and nothing else.
    ///
    /// The clause-derived value: §8.5.2/§8.5.4 configure the builtin endpoints
    /// of a discovered participant from the locator lists in its
    /// `SPDPdiscoveredParticipantData`, and those lists are the whole of the
    /// addressing mechanism the specification defines.
    ///
    /// Also the value for a deployment where the source address is not the
    /// peer at all: a beacon relayed by a discovery server carries the
    /// relay's address, and a peer behind a NAT carries the translation's.
    /// Answering there sends a datagram to a third party.
    ///
    /// Cyclone DDS 11.0.1 and Fast DDS 3.6.1 are reached by it — both read
    /// SPDP on the locator they announce, measured 26/26 in both directions
    /// before this axis existed.
    AnnouncedLocatorsOnly,

    /// Those locators **and** the address the beacon arrived from.
    ///
    /// The default, and the reason is a measurement rather than a clause —
    /// see the module header for why that is the rule in this table and not
    /// the exception.
    ///
    /// OpenDDS 3.34.0 needs it. It announces its rtps_udp transport port as
    /// `PID_METATRAFFIC_UNICAST_LOCATOR` and keeps its SPDP reader on the
    /// well-known discovery port, which it announces in no locator list. An
    /// SPDP DATA that lands on the transport port instead is dropped there
    /// under `rtps_protection != NONE`: `RtpsUdpReceiveStrategy::check_encoded`
    /// demands full-message protection from every sender outside
    /// `RtpsUdpDataLink::separate_message` (`RtpsUdpDataLink.cpp:2555`), and
    /// that list holds the stateless and the volatile writer, not the SPDP
    /// writer, because in OpenDDS SPDP never travels the data link at all.
    /// SPDP is exempt from RTPS message protection (DDS-Security 1.2
    /// §7.5.7, §7.5.12) and goes out plain, so the answer is discarded
    /// without a trace and the peer waits for the periodic beacon.
    ///
    /// Measured on the Linux test host, governance `rtps-sign-data`, OpenDDS→ZeroDDS,
    /// 5 runs: 0/5 with [`Self::AnnouncedLocatorsOnly`], 5/5 with this value.
    /// The address the beacon came from is the socket the peer announced on,
    /// which is its SPDP plane whether or not it says so.
    ///
    /// The extra datagram is additive: the announced locators keep their
    /// copy, so a peer that does read SPDP there is unaffected — Cyclone
    /// 26/26 and Fast DDS 26/26 with this value.
    #[default]
    AnnouncedLocatorsAndBeaconSource,
}

impl SpdpResponseTargets {
    /// True when the address the beacon arrived from is a target.
    #[must_use]
    pub const fn answers_beacon_source(self) -> bool {
        matches!(self, Self::AnnouncedLocatorsAndBeaconSource)
    }
}

/// Whether a GROUP-scope publisher's writer puts `PID_GROUP_COHERENT_SET`
/// on the DATA of a group coherent set.
///
/// DDSI-RTPS 2.5 §8.7.6 requires it: "All Data submessages and the first
/// DataFrag submessage belonging to the same group coherent set shall have
/// three in-line QoS parameters", `PID_GROUP_COHERENT_SET` among them, and
/// the end-of-coherent-set DATA carries it. Measured on the OMG dds-rtps
/// suite (`Test_CoherentSets_12`, `_18`, `_21`, `--access-scope g`): with it,
/// HDDS 1.4.8 delivers `_12` and `_21` wrong and Connext 7.7.0 delivers
/// `_18` and `_21` nothing; without it all three pass with both. Connext
/// uses the parameter in its own group-coherent scheme, together with
/// `PID_ORIGINAL_WRITER_INFO` and vendor parameters it alone sends. CoreDX
/// 6.15.3 passes either way. A ZeroDDS subscriber needs it to know a group
/// coherent set's members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GroupCoherentSetPid {
    /// Sent, as §8.7.6 requires. The default.
    #[default]
    Sent,
    /// Left out.
    Omitted,
}

/// Whether a GROUP-scope publisher's writer puts the group information of
/// DDSI-RTPS 2.5 §8.7.5/§8.7.6 on the wire.
///
/// The specification's form numbers every change of the publisher's writers
/// with `PID_GROUP_SEQ_NUM` and `PID_WRITER_GROUP_INFO`, marks a group
/// coherent set with `PID_GROUP_COHERENT_SET`, closes it with an
/// end-of-coherent-set DATA from every writer of the publisher, and adds the
/// GroupInfoFlag with the group sequence numbers to HEARTBEAT and GAP.
///
/// In the published OMG dds-rtps run of 2026-09-30, Connext 7.7.0, HDDS 1.4.8
/// and zzdds 0.3.2 as subscribers take the GROUP-scope coherent and ordered
/// cases from HDDS and zzdds, which send each writer's coherent set as at
/// TOPIC scope and none of the group information (measured 2026-10-10:
/// `PID_COHERENT_SET`, `PID_KEY_HASH`, no `PID_GROUP_SEQ_NUM`, no
/// GroupInfoFlag). [`Self::CoherentSetsOnly`] is that form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GroupWireForm {
    /// The group information of §8.7.5/§8.7.6. The default.
    #[default]
    Specification,
    /// Each writer's coherent set as at TOPIC scope: `PID_COHERENT_SET` on
    /// its samples, an end-of-coherent-set DATA without in-line QoS from a
    /// writer that wrote into the set, and no group sequence numbers on
    /// DATA, HEARTBEAT or GAP. A subscriber of the peer orders by writer,
    /// not by group.
    CoherentSetsOnly,
}

/// How the end-of-coherent-set DATA of a writer reaches a peer.
///
/// DDSI-RTPS 2.5 Table 9.22 ends a writer's coherent set with a DATA
/// without data and without in-line QoS (Example 3), which ZeroDDS sends.
/// An InterCOM 4.3.1 reader does not take that DATA: it requests the number
/// again and again and delivers nothing after it, also with an INFO_TS
/// before it. HDDS 1.4.8 answers that request with a GAP for the number,
/// which InterCOM takes (measured on OMG dds-rtps Test_CoherentSets_10:
/// InterCOM acknowledges HDDS's sets set by set, and stayed at ZeroDDS's
/// first end). InterCOM sends no coherent set itself (no
/// `PID_COHERENT_SET`). [`Self::Gap`] gives such a peer the number as a GAP:
/// it learns that the number carries no sample, and its set ends where it
/// ends one without the marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EndOfSetForm {
    /// The DATA of Table 9.22 Example 3. The default.
    #[default]
    Data,
    /// A GAP for the number of the end-of-coherent-set DATA.
    Gap,
}

/// Whether the HEARTBEAT of a writer with group information carries
/// `writerSet` and `secureWriterSet` (DDSI-RTPS 2.5 Table 8.44, §9.4.2.19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HeartbeatGroupDigests {
    /// Sent after `lastGSN`, as the PIM and §8.7.5 ask. The default.
    #[default]
    Sent,
    /// Left out: the form of the figure of §9.4.5.7, for a peer that does not
    /// skip the trailing octets of a HEARTBEAT by its `submessageLength`.
    Omitted,
}

/// The RTPS-side behaviour table: one value per axis.
///
/// The counterpart of `zerodds_ast_layout::LayoutKey`, for submessages
/// instead of for CDR. Held per participant as a default and overridable
/// per matched peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RtpsBehaviour {
    /// Where a dispose/unregister marker names its instance.
    pub dispose_key_form: DisposeKeyForm,
    /// Whether every datagram states its destination `GuidPrefix`.
    pub directed_addressing: DirectedAddressing,
    /// Where the directed SPDP response to a new participant goes.
    pub spdp_response_targets: SpdpResponseTargets,
    /// Whether the DATA of a group coherent set carry
    /// `PID_GROUP_COHERENT_SET`.
    pub group_coherent_set: GroupCoherentSetPid,
    /// Whether a HEARTBEAT with group information carries `writerSet` and
    /// `secureWriterSet`.
    pub heartbeat_group_digests: HeartbeatGroupDigests,
    /// Whether a GROUP-scope publisher's writers send the group information
    /// at all.
    pub group_wire_form: GroupWireForm,
    /// How the end-of-coherent-set DATA of a writer reaches the peer.
    pub end_of_set_form: EndOfSetForm,
}

impl RtpsBehaviour {
    /// The default table — see [`RtpsDialect::MaximallyInteroperable`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            dispose_key_form: DisposeKeyForm::KeyInPayloadWithHash,
            directed_addressing: DirectedAddressing::ByEntityIdOnly,
            spdp_response_targets: SpdpResponseTargets::AnnouncedLocatorsAndBeaconSource,
            group_coherent_set: GroupCoherentSetPid::Sent,
            heartbeat_group_digests: HeartbeatGroupDigests::Sent,
            group_wire_form: GroupWireForm::Specification,
            end_of_set_form: EndOfSetForm::Data,
        }
    }

    /// Builder: sets [`Self::dispose_key_form`].
    #[must_use]
    pub const fn with_dispose_key_form(mut self, form: DisposeKeyForm) -> Self {
        self.dispose_key_form = form;
        self
    }

    /// Builder: sets [`Self::directed_addressing`].
    #[must_use]
    pub const fn with_directed_addressing(mut self, a: DirectedAddressing) -> Self {
        self.directed_addressing = a;
        self
    }

    /// Builder: sets [`Self::spdp_response_targets`].
    #[must_use]
    pub const fn with_spdp_response_targets(mut self, t: SpdpResponseTargets) -> Self {
        self.spdp_response_targets = t;
        self
    }

    /// Builder: sets [`Self::heartbeat_group_digests`].
    #[must_use]
    pub const fn with_heartbeat_group_digests(mut self, d: HeartbeatGroupDigests) -> Self {
        self.heartbeat_group_digests = d;
        self
    }

    /// Builder: sets [`Self::group_coherent_set`].
    #[must_use]
    pub const fn with_group_coherent_set(mut self, g: GroupCoherentSetPid) -> Self {
        self.group_coherent_set = g;
        self
    }

    /// Builder: sets [`Self::group_wire_form`].
    #[must_use]
    pub const fn with_group_wire_form(mut self, f: GroupWireForm) -> Self {
        self.group_wire_form = f;
        self
    }

    /// Builder: sets [`Self::end_of_set_form`].
    #[must_use]
    pub const fn with_end_of_set_form(mut self, f: EndOfSetForm) -> Self {
        self.end_of_set_form = f;
        self
    }
}

/// A named set of axis values, one per measured implementation.
///
/// Deliberately **separate** from `zerodds_ast_layout::Dialect`: that one
/// names wire-format dialects, this one names discovery-behaviour dialects,
/// and an implementation can occupy an axis in one and none in the other.
/// One list would blur that. The spellings are kept in step by
/// `tests/dialect_no_drift.rs`, which fails when a name here has no
/// counterpart there or names a different version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RtpsDialect {
    /// Every axis at the value that the largest number of measured
    /// implementations accept — for the dispose marker, the one form all
    /// three accept because it carries both identities.
    ///
    /// The default. It is **not** a reading of the specification, and it
    /// does not claim to be: on this module's axes the clauses permit
    /// several forms and prefer none, so there is no clause to default to.
    /// See the module header.
    #[default]
    MaximallyInteroperable,

    /// Eclipse Cyclone DDS 11.0.1.
    ///
    /// Sends its SEDP dispose as [`DisposeKeyForm::KeyInPayload`] — key in
    /// the payload, no `PID_KEY_HASH`.
    Cyclone,

    /// eProsima Fast DDS 3.6.1.
    ///
    /// Sends its SEDP dispose as [`DisposeKeyForm::HashOnly`] — no payload,
    /// `PID_KEY_HASH` alone.
    FastDds,

    /// OpenDDS 3.34.0.
    ///
    /// Sends its SEDP dispose as [`DisposeKeyForm::KeyInPayloadWithHash`] —
    /// both identities, which is what makes that form the default.
    OpenDds,
}

impl RtpsDialect {
    /// Configuration spelling. Shares the vocabulary of
    /// `zerodds_ast_layout::Dialect::as_str` wherever the two name the same
    /// implementation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MaximallyInteroperable => "maximally-interoperable",
            Self::Cyclone => "cyclone",
            Self::FastDds => "fastdds",
            Self::OpenDds => "opendds",
        }
    }

    /// Parses a configuration value. An unknown value is `None` rather than
    /// the default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "maximally-interoperable" => Some(Self::MaximallyInteroperable),
            "cyclone" => Some(Self::Cyclone),
            "fastdds" => Some(Self::FastDds),
            "opendds" => Some(Self::OpenDds),
            _ => None,
        }
    }

    /// The implementation version whose octets this dialect reproduces, or
    /// `None` for [`Self::MaximallyInteroperable`], which reproduces no
    /// single implementation.
    #[must_use]
    pub const fn measured_version(self) -> Option<&'static str> {
        match self {
            Self::MaximallyInteroperable => None,
            Self::Cyclone => Some("11.0.1"),
            Self::FastDds => Some("3.6.1"),
            Self::OpenDds => Some("3.34.0"),
        }
    }

    /// True for the default (see [`Self::MaximallyInteroperable`]).
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::MaximallyInteroperable)
    }

    /// Every dialect, for tests and for a configuration front end that
    /// wants to list them.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::MaximallyInteroperable,
            Self::Cyclone,
            Self::FastDds,
            Self::OpenDds,
        ]
    }

    /// The axis values this dialect stands for.
    #[must_use]
    pub const fn behaviour(self) -> RtpsBehaviour {
        let form = match self {
            Self::MaximallyInteroperable | Self::OpenDds => DisposeKeyForm::KeyInPayloadWithHash,
            Self::Cyclone => DisposeKeyForm::KeyInPayload,
            Self::FastDds => DisposeKeyForm::HashOnly,
        };
        // Cyclone and Fast DDS read SPDP on the locator they announce, so
        // naming them is naming the narrower form; OpenDDS does not, and the
        // default carries the wider one for its sake.
        let spdp = match self {
            Self::MaximallyInteroperable | Self::OpenDds => {
                SpdpResponseTargets::AnnouncedLocatorsAndBeaconSource
            }
            Self::Cyclone | Self::FastDds => SpdpResponseTargets::AnnouncedLocatorsOnly,
        };
        RtpsBehaviour::new()
            .with_dispose_key_form(form)
            .with_spdp_response_targets(spdp)
    }
}

/// Per-participant default plus per-peer overrides.
///
/// The default applies to every matched peer. An override is explicit and
/// keyed by the peer's reader GUID, or by its participant's `GuidPrefix`
/// for every reader of that participant, so one participant can serve a
/// Cyclone peer and a Fast DDS peer at the same time, each in its own form,
/// without a second participant — and without inferring anything from a
/// `vendorId` a remote controls. A reader GUID's override goes before its
/// participant's.
#[derive(Debug, Clone, Default)]
pub struct BehaviourTable {
    default: RtpsBehaviour,
    overrides: alloc::vec::Vec<(Guid, RtpsBehaviour)>,
    participants: alloc::vec::Vec<(crate::wire_types::GuidPrefix, RtpsBehaviour)>,
}

impl BehaviourTable {
    /// A table whose default is `default` and which has no overrides.
    #[must_use]
    pub const fn new(default: RtpsBehaviour) -> Self {
        Self {
            default,
            overrides: alloc::vec::Vec::new(),
            participants: alloc::vec::Vec::new(),
        }
    }

    /// The participant-wide default.
    #[must_use]
    pub const fn default_behaviour(&self) -> RtpsBehaviour {
        self.default
    }

    /// Replaces the participant-wide default. Peers that carry an explicit
    /// override keep it.
    pub fn set_default(&mut self, behaviour: RtpsBehaviour) {
        self.default = behaviour;
    }

    /// Sets the behaviour for one peer, replacing any earlier override.
    pub fn set_for_peer(&mut self, peer: Guid, behaviour: RtpsBehaviour) {
        if let Some(slot) = self.overrides.iter_mut().find(|(g, _)| *g == peer) {
            slot.1 = behaviour;
        } else {
            self.overrides.push((peer, behaviour));
        }
    }

    /// Drops one peer's override, so the default applies again.
    pub fn clear_for_peer(&mut self, peer: Guid) {
        self.overrides.retain(|(g, _)| *g != peer);
    }

    /// Sets the behaviour for every reader of the participant `prefix`,
    /// replacing any earlier one for it.
    pub fn set_for_participant(
        &mut self,
        prefix: crate::wire_types::GuidPrefix,
        behaviour: RtpsBehaviour,
    ) {
        if let Some(slot) = self.participants.iter_mut().find(|(p, _)| *p == prefix) {
            slot.1 = behaviour;
        } else {
            self.participants.push((prefix, behaviour));
        }
    }

    /// The behaviour for `peer`: its override, its participant's, or the
    /// default.
    #[must_use]
    pub fn for_peer(&self, peer: Guid) -> RtpsBehaviour {
        if let Some((_, b)) = self.overrides.iter().find(|(g, _)| *g == peer) {
            return *b;
        }
        self.participants
            .iter()
            .find(|(p, _)| *p == peer.prefix)
            .map_or(self.default, |(_, b)| *b)
    }

    /// Number of peers with an explicit override.
    #[must_use]
    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    #[test]
    fn a_readers_override_goes_before_its_participants() {
        use crate::wire_types::{EntityId, GuidPrefix};
        let prefix = GuidPrefix::from_bytes([3; 12]);
        let one = Guid::new(prefix, EntityId::user_reader_with_key([0, 0, 1]));
        let other = Guid::new(prefix, EntityId::user_reader_with_key([0, 0, 2]));
        let elsewhere = Guid::new(
            GuidPrefix::from_bytes([4; 12]),
            EntityId::user_reader_with_key([0, 0, 1]),
        );
        let omitted = RtpsBehaviour::new().with_group_coherent_set(GroupCoherentSetPid::Omitted);
        let mut t = BehaviourTable::new(RtpsBehaviour::new());
        t.set_for_participant(prefix, omitted);
        t.set_for_peer(one, RtpsBehaviour::new());
        assert_eq!(
            t.for_peer(one).group_coherent_set,
            GroupCoherentSetPid::Sent
        );
        assert_eq!(
            t.for_peer(other).group_coherent_set,
            GroupCoherentSetPid::Omitted
        );
        assert_eq!(
            t.for_peer(elsewhere).group_coherent_set,
            GroupCoherentSetPid::Sent
        );
    }

    use super::*;
    use crate::wire_types::{EntityId, GuidPrefix};

    fn peer(n: u8) -> Guid {
        Guid::new(
            GuidPrefix::from_bytes([n; 12]),
            EntityId::user_reader_with_key([0xA0, 0xB0, n]),
        )
    }

    /// The default is the union form, and it is the union form because it is
    /// the only value that both sets the K flag and emits the hash.
    #[test]
    fn default_carries_both_identities() {
        let b = RtpsBehaviour::default();
        assert_eq!(b.dispose_key_form, DisposeKeyForm::KeyInPayloadWithHash);
        assert!(b.dispose_key_form.sets_key_flag());
        assert!(b.dispose_key_form.emits_key_hash());
        assert_eq!(RtpsDialect::default(), RtpsDialect::MaximallyInteroperable);
        assert!(RtpsDialect::default().is_default());
    }

    /// Each measured implementation's form, as read off its own wire.
    #[test]
    fn each_dialect_reproduces_its_measured_form() {
        assert_eq!(
            RtpsDialect::Cyclone.behaviour().dispose_key_form,
            DisposeKeyForm::KeyInPayload
        );
        assert_eq!(
            RtpsDialect::FastDds.behaviour().dispose_key_form,
            DisposeKeyForm::HashOnly
        );
        assert_eq!(
            RtpsDialect::OpenDds.behaviour().dispose_key_form,
            DisposeKeyForm::KeyInPayloadWithHash
        );
    }

    /// Cyclone's form names the instance only in the payload, so without a
    /// key it would name it nowhere. It degrades to the form that still
    /// carries an identity, never to one that carries none.
    #[test]
    fn a_form_without_a_key_still_names_the_instance() {
        for form in [
            DisposeKeyForm::KeyInPayload,
            DisposeKeyForm::KeyInPayloadWithHash,
            DisposeKeyForm::HashOnly,
        ] {
            let usable = form.with_key_available(false);
            assert!(
                !usable.sets_key_flag(),
                "{form:?} without a key must not set K over zero octets"
            );
            assert!(
                usable.emits_key_hash(),
                "{form:?} without a key must still carry PID_KEY_HASH — a \
                 marker naming no instance is one a receiver cannot act on"
            );
        }
        for form in [
            DisposeKeyForm::KeyInPayload,
            DisposeKeyForm::KeyInPayloadWithHash,
        ] {
            assert_eq!(form.with_key_available(true), form);
        }
    }

    /// The SPDP axis: the default answers the beacon source as well, because
    /// that is the value every measured implementation is reached by; the
    /// clause-derived value answers only the announced locators.
    #[test]
    fn the_spdp_response_default_answers_the_beacon_source() {
        assert_eq!(
            RtpsBehaviour::default().spdp_response_targets,
            SpdpResponseTargets::AnnouncedLocatorsAndBeaconSource
        );
        assert!(SpdpResponseTargets::default().answers_beacon_source());
        assert!(!SpdpResponseTargets::AnnouncedLocatorsOnly.answers_beacon_source());
        // OpenDDS is the implementation the wider form exists for, so its
        // dialect must not be the one that drops it.
        assert!(
            RtpsDialect::OpenDds
                .behaviour()
                .spdp_response_targets
                .answers_beacon_source(),
            "OpenDDS is reached only on the beacon source — measured 0/5 vs 5/5"
        );
        for d in [RtpsDialect::Cyclone, RtpsDialect::FastDds] {
            assert_eq!(
                d.behaviour().spdp_response_targets,
                SpdpResponseTargets::AnnouncedLocatorsOnly,
                "{d:?} is reached on the locator it announces"
            );
        }
    }

    /// Every axis a dialect names has to survive being named together with
    /// the others — a `behaviour()` that set one axis and reset another
    /// would silently undo a value the caller asked for.
    #[test]
    fn a_dialect_names_every_axis_at_once() {
        for d in RtpsDialect::all() {
            let b = d.behaviour();
            assert_eq!(
                b.dispose_key_form,
                d.behaviour().dispose_key_form,
                "{d:?}: dispose form is not stable"
            );
            assert_eq!(
                b.directed_addressing,
                RtpsBehaviour::new().directed_addressing,
                "{d:?}: no dialect claims a directed-addressing value yet, so \
                 it must carry the default rather than a silent one"
            );
        }
    }

    /// The migrated axis maps both ways onto the boolean it replaces.
    #[test]
    fn directed_addressing_maps_onto_the_migrated_boolean() {
        assert!(!DirectedAddressing::default().emits_info_dst());
        assert!(DirectedAddressing::PrefixEveryDatagram.emits_info_dst());
        for emit in [false, true] {
            assert_eq!(
                DirectedAddressing::from_emit_info_dst(emit).emits_info_dst(),
                emit
            );
        }
    }

    /// A per-peer override applies to that peer and to no other.
    #[test]
    fn an_override_reaches_one_peer_only() {
        let mut t = BehaviourTable::new(RtpsBehaviour::default());
        assert_eq!(t.for_peer(peer(1)), RtpsBehaviour::default());
        t.set_for_peer(peer(1), RtpsDialect::Cyclone.behaviour());
        assert_eq!(
            t.for_peer(peer(1)).dispose_key_form,
            DisposeKeyForm::KeyInPayload
        );
        assert_eq!(
            t.for_peer(peer(2)).dispose_key_form,
            DisposeKeyForm::KeyInPayloadWithHash,
            "one peer's dialect must not reach another"
        );
        // Two peers of different vendors on one participant, which is the
        // reason the table is per peer at all.
        t.set_for_peer(peer(2), RtpsDialect::FastDds.behaviour());
        assert_eq!(
            t.for_peer(peer(1)).dispose_key_form,
            DisposeKeyForm::KeyInPayload
        );
        assert_eq!(
            t.for_peer(peer(2)).dispose_key_form,
            DisposeKeyForm::HashOnly
        );
        assert_eq!(t.override_count(), 2);
        // Setting the same peer again replaces rather than appends.
        t.set_for_peer(peer(2), RtpsDialect::OpenDds.behaviour());
        assert_eq!(t.override_count(), 2);
        t.clear_for_peer(peer(1));
        assert_eq!(t.for_peer(peer(1)), RtpsBehaviour::default());
        assert_eq!(t.override_count(), 1);
    }

    /// A new default does not silently take an overridden peer with it.
    #[test]
    fn a_new_default_leaves_overrides_alone() {
        let mut t = BehaviourTable::new(RtpsBehaviour::default());
        t.set_for_peer(peer(1), RtpsDialect::Cyclone.behaviour());
        t.set_default(RtpsDialect::FastDds.behaviour());
        assert_eq!(
            t.for_peer(peer(1)).dispose_key_form,
            DisposeKeyForm::KeyInPayload
        );
        assert_eq!(
            t.for_peer(peer(9)).dispose_key_form,
            DisposeKeyForm::HashOnly
        );
    }

    /// An unknown spelling is `None`, not the default.
    #[test]
    fn parse_round_trips_and_rejects_a_typo() {
        for d in RtpsDialect::all() {
            assert_eq!(RtpsDialect::parse(d.as_str()), Some(*d));
        }
        assert_eq!(RtpsDialect::parse("cyclonedds"), None);
        assert_eq!(RtpsDialect::parse(""), None);
    }

    /// Only the default names no implementation version.
    #[test]
    fn every_dialect_but_the_default_names_a_version() {
        for d in RtpsDialect::all() {
            assert_eq!(
                d.measured_version().is_none(),
                d.is_default(),
                "{d:?}: a dialect that reproduces an implementation must name \
                 its version, and the default must not pretend to"
            );
        }
    }
}
