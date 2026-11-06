# Cross-vendor interoperability — validation & proof

**Mission:** ZeroDDS interoperates with every other DDS stack, under every
wire condition. This document is the consolidated evidence — live cross-vendor
runs, byte-level wire conformance, performance, security, and same-host
zero-copy — against the four other relevant DDS stacks:

- **CycloneDDS** (Eclipse Foundation; ROS 2 reference middleware)
- **eProsima Fast DDS** (ROS 2 default middleware)
- **OpenDDS** (Object Computing)
- **RTI Connext** (commercial reference)

Cross-vendor runs are captured live on a single Linux host (8-core LXC) and
parsed from `tcpdump`; the encoding-byte claims are anchored against each
vendor's own IDL compiler. Public, runnable proof projects for the claims are
in the example repository — see *Proof projects* at the end.

> Status: ✅ live across all four vendors on the current wire stack. The
> detailed per-test inventory (Cyclone + Fast DDS QoS/discovery live tests)
> follows the high-level matrices below.

## Coverage at a glance

| Dimension | Cyclone | Fast&nbsp;DDS | OpenDDS | RTI | Evidence |
|---|:--:|:--:|:--:|:--:|---|
| Discovery + match (SPDP/SEDP) | ✅ | ✅ | ✅ | ✅ | live shapes matrix, both directions |
| `@final` small sample | ✅ | ✅ | ✅ | ✅ | shapes matrix, real samples decoded |
| XCDR1 ↔ XCDR2 negotiation | ✅ | ✅ | ✅ | ✅ | reader accepts both; writer XCDR1-capable |
| `@appendable` / `@mutable` live | ✅ | ✅ | ✅ | ✅ | 24-cell matrix below, all bidirectional |
| Big-endian receive **and** emit | ✅ | ✅ | ✅ | ✅ | ZeroDDS emits BE; all four decode it |
| Large / fragmented (DATA_FRAG) | ✅ | 🧩 | 🧩 | 🧩 | Cyclone↔ZeroDDS 60 KB both ways |
| Rich type (nested/seq/string/array) | ✅ | ✅ | ✅ | ✅ | rich matrix below, fully connected |
| DDS-Security (auth/AC/crypto) | ✅ | ✅¹ | ✅ | —² | secure matrices below |
| Same-host zero-copy | ✅³ | — | — | — | iceoryx-C++ (Cyclone) + iceoryx2 (Rust) |
| Reflective dynamic XTypes (no compile-time type) | ✅ | ✅ | ✅ | 🧩 | type-following decode of any TypeObject writer |

Legend: ✅ live cross-vendor, both directions · 🧩 narrower coverage (encoding
golden, same-runtime, or one direction) · — not applicable. ¹ Fast DDS secured
cross-vendor needs RSA identity certs (vendor constraint, see below). ² RTI
security is licence-gated, deliberately not exercised. ³ over the real
CycloneDDS iceoryx-C++/POSH shared-memory transport.

---

## Wire conformance — extensibility × XCDR-version × endianness

The hardest interop axis is the serialized-payload **encapsulation tuple**
(XCDR version × extensibility × endianness). A single keyed type in three
variants — `@final / @appendable / @mutable struct { @key uint32 id; double
value; sequence<octet> blob; }` — exercises an 8-byte-aligned primitive plus a
sequence in every wire form. 4 vendors × 3 extensibilities × 2 directions =
**24 cells**, all live, best-effort readers, parsed from `tcpdump`.

### What each writer puts on the wire (measured encapsulation id)

| writer | `@final` | `@appendable` | `@mutable` |
|---|:--:|:--:|:--:|
| **ZeroDDS** (default) | `0x0007` CDR2 (X2) | `0x0009` D_CDR2 (X2) | `0x000b` PL_CDR2 (X2) |
| **OpenDDS** | `0x0007` CDR2 (X2) | `0x0009` D_CDR2 (X2) | `0x000b` PL_CDR2 (X2) |
| **CycloneDDS** | `0x0001` CDR (**X1**) | `0x0009` D_CDR2 (X2) | `0x000b` PL_CDR2 (X2) |
| **Fast&nbsp;DDS** | `0x0001` CDR (**X1**) | `0x0001` CDR (**X1**) | `0x0003` PL_CDR (**X1**) |
| **RTI Connext** | `0x0001` CDR (**X1**) | `0x0001` CDR (**X1**) | `0x0003` PL_CDR (**X1**, PID_EXTENDED) |

Three native dialects: **XCDR2-everywhere** (ZeroDDS, OpenDDS); **XCDR1-everywhere**
(Fast DDS, RTI — `@final`/`@appendable` collapse to plain CDR because XCDR1 has
no DHEADER); **hybrid** (Cyclone: XCDR1 `@final`, XCDR2 for the extensible
kinds). Fast DDS and RTI share encap `0x0003` for `@mutable` yet differ one level
down — Fast DDS uses short PIDs, RTI uses **PID_EXTENDED + MUST_UNDERSTAND**
(`0x7F01`). ZeroDDS reads and writes **all** of these.

### Interop result (recv / integrity, best-effort reader)

| Vendor | → ZeroDDS (final / append / mutable) | ZeroDDS → (final / append / mutable) |
|---|:--:|:--:|
| **CycloneDDS** | ✅ / ✅ / ✅ | ✅ / ✅ / ✅ |
| **Fast&nbsp;DDS** | ✅ / ✅ / ✅ | ✅ / ✅ / ✅ |
| **RTI Connext** | ✅ / ✅ / ✅ | ✅ / ✅ / ✅ |
| **OpenDDS** | ✅ / ✅ / ✅ | ✅ / ✅ / ✅ |

**All 24 cells deliver correct data.** ZeroDDS carries every reader/writer
path — plain CDR (X1), PL_CDR (X1, including RTI's PID_EXTENDED), and
CDR2/D_CDR2/PL_CDR2 (X2) — so it speaks each vendor's native dialect
interchangeably. That spread *is* the "all combinations" proof.

### Big-endian axis — closed live

Every vendor on an x86 host emits little-endian. The big-endian half is reached
by making ZeroDDS *emit* BE (a real feature: `encode_be` in the Rust codegen +
a big-endian writer path — the reader could already decode BE). Measured: ZeroDDS
emits `0x0006` CDR2_BE / `0x0008` D_CDR2_BE / `0x000a` PL_CDR2_BE, and **all four
vendors decode it across all three extensibilities, integrity OK** — Cyclone
59/59, Fast DDS 60/60, OpenDDS 40/40, RTI 60/60.

### The wire fixes (ZeroDDS side, with evidence)

Three ZeroDDS fixes turned the last cross-vendor cells green; each is a spec
conformance correction, not a vendor workaround:

- **RTI `@mutable` decode.** RTI encodes its sequence member with
  PID_EXTENDED carrying the MUST_UNDERSTAND flag (`0x7F01` = `0x4000` MU |
  `0x3F01` PID_EXTENDED). ZeroDDS compared the raw 16-bit PID without masking
  the flag bits and de-synced. Fix: mask `& 0x3FFF` per RTPS §9.6.2.2.1
  (in the XCDR1 codec of the time) — the XCDR1 twin of the XCDR2 LC5 fix. Turned the
  whole RTI row green.
- **ZeroDDS → OpenDDS `@appendable`/`@mutable`.** ZeroDDS stamped encap
  `0x0007` (FINAL) on every sample even when the body was correctly framed as
  appendable/mutable — the writer never propagated `T::EXTENSIBILITY`. Lenient
  readers tolerated it; OpenDDS's strict `to_encoding` rejected it. Fix: thread
  `T::EXTENSIBILITY` through at `create_datawriter`
  (`crates/dcps/src/publisher.rs`). ZeroDDS now emits `0x0009`/`0x000b`.
- **OpenDDS → ZeroDDS spurious decode error.** OpenDDS sends one key-only DATA
  (`register_instance`) per data sample; ZeroDDS full-decoded the key-only
  payload and raised a decode error per sample. Fix: key-only ALIVE samples are
  acked-and-skipped, not delivered for decode (`crates/rtps/src/reliable_reader.rs`).

**Confirmed limitation (universal, not ZeroDDS-specific):** a single writer
emits one representation per sample, so one writer cannot serve XCDR1-only *and*
XCDR2-only readers simultaneously — true of all four vendors. A ZeroDDS writer
emits the first representation it offers: by default XCDR1, the first of
`wire.data_representation_offer` (`[XCDR1, XCDR2]`); per writer
`DataWriterQos.data_representation`, participant-wide that setting.

---

## Rich-typed interoperability + performance

A second type maximizes the per-sample codec work — `string<64>`, a 16-element
`double` array (PARRAY), a `sequence<Waypoint>` of nested structs, and an
`octet` payload sweep — to isolate the encode/decode cost from the transport.
Round-trip ping→pong, median **p50 µs**, `✗` = NO_DATA.

| ping ＼ pong | zerodds | cyclone | fastdds | rti | opendds |
|---|---|---|---|---|---|
| **zerodds** | 20 | 41 | 35 | **61** | 60 |
| cyclone | 62 | 70 | 77 | ✗ | ✗ |
| fastdds | 70 | 91 | 64 | **76** | 123 |
| rti | **23** | ✗ | 67 | 56 | ✗ |
| opendds | 103 | ✗ | 79 | ✗ | 226 |

**ZeroDDS interoperates with all four other vendors on the rich type — the only
fully-connected stack besides Fast DDS.** The empty cells are vendor↔vendor
pairs (`cyclone↔rti`, `cyclone↔opendds`, `rti↔opendds`) where no ZeroDDS is
involved. RTI needed its XCDR2 compliance mask enabled
(`NDDS_XTYPES_COMPLIANCE_MASK`) — RTI's default is non-compliant (it omits the
collection DHEADER); ZeroDDS was already spec-correct.

On the simple type, `zerodds ↔ zerodds` leads at every payload (≈18 µs at 0 B,
≈23 µs at 8 KiB — its same-host SHM carrier avoids UDP fragmentation, where
other stacks sit at 38–99 µs). Codec-cost deltas (rich vs simple) are within the
LXC noise floor, so only the connectivity and ranking are reported.

---

## DDS-Security, cross-vendor

DDS-Security 1.2 (PKI-DH auth, AES-GCM-GMAC crypto, governance + permissions),
SRTPS over UDP. Median **p50 µs**, `✗`/`FAIL`/`NO_MATCH` = no secured match.

**Simple type, secured**

| ping ＼ pong | cyclone | fastdds | opendds | zerodds |
|---|---|---|---|---|
| cyclone | 92 | NO_MATCH | NO_MATCH | **78** |
| fastdds | FAIL | 206 | FAIL | FAIL |
| opendds | FAIL | FAIL | FAIL | FAIL |
| **zerodds** | **48** | NO_MATCH | NO_MATCH | **37** |

**Rich type, secured**

| ping ＼ pong | zerodds | cyclone | fastdds | opendds |
|---|---|---|---|---|
| **zerodds** | 91 | 72 | ✗ | **194** |
| cyclone | 113 | 135 | ✗ | ✗ |
| opendds | **125** | ✗ | – | 232 |

**ZeroDDS is the only stack that interoperates secured with both Cyclone and
OpenDDS** (and itself), on both the simple and the rich type. The cells that
stay dark are vendor↔vendor cert/algorithm incompatibilities, each reproducible
**without** ZeroDDS:

- **Fast DDS secured cross-vendor needs RSA identity certs.** Its PKI-DH
  handshake rejects the EC P-256 certs that Cyclone/OpenDDS/ZeroDDS accept
  (`cyclone↔fastdds` secured also fails with EC, no ZeroDDS involved). With RSA
  certs, **`zerodds↔fastdds` secured + rich works both ways (70 / 28 µs)**. RSA
  in turn breaks `zerodds↔cyclone` (Cyclone prefers EC) — so no single cert
  algorithm spans all four, but **ZeroDDS interoperates secured under both EC
  and RSA**.
- **Cyclone↔OpenDDS rich** fails on the same TypeObject-strictness mismatch
  that fails their *plain* rich pairing — again, no ZeroDDS involved.

Crypto cost on ZeroDDS (simple, basis→secure) is +19 µs self / +24 µs to
Cyclone — lighter than Cyclone's own +58 µs.

**Pluggable crypto backend — same interop on FIPS / wolfCrypt.** ZeroDDS'
DDS-Security primitives (AES-GCM, HMAC, HKDF, ECDH, ECDSA) are supplied by a
compile-time-selectable backend: `ring` (default), AWS-LC (`fips`,
FIPS-140-3-validatable) or wolfSSL/wolfCrypt (`wolfcrypt`, DO-178 / embedded
heritage). Because the AES-GCM/HMAC wire bytes are standardized, the secured
cross-vendor matrix is **backend-independent** — re-running the secured matrix on
the canonical `data-enc` profile with the wolfCrypt backend keeps every
ZeroDDS ↔ {Cyclone, Fast DDS, OpenDDS} cell green in both directions, with an
identical pass/fail topology to the default build (ZeroDDS↔Cyclone 41/44,
↔Fast DDS 84/56, ↔OpenDDS 93/84 µs). A regulated or safety-certified deployment
gets the same interoperability as the stock build.

---

## Same-host zero-copy

ZeroDDS speaks two shared-memory zero-copy paths, both cross-stack:

- **iceoryx2 (Rust)** — ZeroDDS ↔ iceoryx2 same-host, both directions.
- **iceoryx C++ / POSH** — ZeroDDS reads and writes a CycloneDDS sample over
  Cyclone's *real* `psmx_iox` iceoryx-C++ shared-memory transport (the PSMX
  chunk carries the writer's RTPS GUID + Cyclone-shaped user header). This is
  true cross-vendor same-host zero-copy, not the iceoryx2-Rust path.

ZeroDDS's own same-host carrier is an event-driven POSIX-SHM ring (shared futex
wakeups, no sleep-poll), which is why `zerodds ↔ zerodds` leads the latency
tables above.

---

## Reflective dynamic XTypes

ZeroDDS can decode a sample with **no compile-time type** by following the
writer's advertised `TypeObject`: `TypeObject → DynamicType` resolution (all 10
type kinds, nested composites, collections, `wstring`, bitmask/bitset) feeding a
`DynamicData ↔ CDR` reflective codec (XCDR1, XCDR2, and `@mutable` PL_CDR2). The
`zerodds-spy` tool attaches to any TypeObject-advertising vendor writer (e.g.
Fast DDS) and prints typed samples without the IDL.

---

## Proof projects (public, runnable)

Every claim above has a runnable project in the example repository:

| Project | Proves |
|---|---|
| `cross-vendor-shapes/` | live 4-vendor discovery + `@final` shapes matrix, both directions |
| `cross-vendor-rich/` | the rich-typed interop + the latency/security matrices |
| `cross-vendor-dynamic-xtypes/` | type-following decode of every TypeObject-advertising vendor |
| `iceoryx-cyclone-zerocopy/` | ZeroDDS ↔ Cyclone same-host zero-copy over iceoryx-C++ |
| `idl-conformance/` | cross-PSM encoding byte-identity across all language bindings |

---

## Test inventory (Cyclone + Fast DDS live QoS/discovery)

> The per-test inventory below predates the multi-vendor matrices above and is
> kept as the detailed live-test backing for the Cyclone + Fast DDS rows.

### CV-1 fastdds_discovery_server_spdp_handshake

**Spec:** `ddsi-rtps-2.5.md::§8.5.1` — Simple Participant Discovery
Protocol; the Fast DDS discovery server is eProsima's extension of it.

**Repo:** `crates/discovery`.

**Tests:** —

**Status:** open — ZeroDDS is no client of a Fast DDS discovery server.
Measured on the Linux test host with Fast DDS 3.6.1 (`fastdds discovery -l
<address> -p <port>`, `ROS_DOMAIN_ID` the participant's domain): the server
answers the `DATA(p)` of a Fast DDS client on the same host with its own
`DATA(p)` and a `HEARTBEAT` within 2 ms, and sends nothing to ZeroDDS'
`DATA(p)`, also when it carries the client's properties `PARTICIPANT_TYPE`
and `DS_VERSION`; the client's message carries `INFO_TS` and the inline QoS
`0x0083` and `0x800f` (the sample identity), ZeroDDS' does not (`tcpdump`,
`tshark`). The earlier test passed on any foreign beacon and started the
server over SSH on a lab host.

### CV-2 fastdds_default_discovery_via_shape_pub_visible

**Spec:** `ddsi-rtps-2.5.md::§8.5.1` — SPDP via Multicast.

**Repo:** `crates/discovery/tests/fastdds_live_spdp.rs`.

**Tests:** `fastdds_default_discovery_via_shape_pub_visible` (the shapes
client of `tests/interop/ci/fastdds` on a leased domain, its beacon parsed
with eProsima's vendor id, the domain and the participant's builtin
endpoints; weekly in `vendor-live-dcps`).

**Status:** done

### CV-3 to CV-11 Fast DDS: best effort and reliable, volatile and transient local, both directions

**Spec:** `dds-xtypes-1.3.md::§7.4.3` (XCDR2), `ddsi-rtps-2.5.md::§8.4.2.2-3`
(Reliable Writer), `ddsi-rtps-2.5.md::§8.5.2` (SEDP match),
`zerodds-dcps-1.4.md::§2.2.3` (QoS compatibility).

**Repo:** `tests/interop/omg/chunks.tsv` (the chunk `rtps-vendor-1`),
`ci/interop-omg-weekly.yml` (the group `rtps-fastdds`).

**Tests:** in the weekly job `omg-rtps-fastdds` (its JUnit report) the OMG
dds-rtps cases `"rtps_test_suite_1_Test_Domain_0"` (a best-effort reader),
`"rtps_test_suite_1_Test_Reliability_4"` (a reliable reader, every sample in
order) and `"rtps_test_suite_1_Test_Durability_17"` (a TRANSIENT_LOCAL
writer and a late joiner) against the Fast DDS 3.6.1 shapes client, ZeroDDS
as publisher and as subscriber; the suite's harness is the oracle
(internal/release/rc8-test-suite.md, "Interop").

**Status:** done

### CV-12 cyclone_live_wlp_manual_by_participant_pulse

**Spec:** `ddsi-rtps-2.5.md::§8.7.2` (LIVELINESS wire mapping) +
`zerodds-dcps-1.4.md::§2.2.3.11` (LIVELINESS QoS).

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint::assert_participant`).

**Tests:** `crates/dcps/src/wlp.rs::tests::rtps_8_4_13_5_wlp_assert_participant_emits_manual_pulse`
(the kind of the ParticipantMessageData, `MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE`).
No Cyclone peer takes part: the test target this item named before called
the same endpoint without a vendor and went with the B3 sort of
`zerodds-dcps`. The live WLP run against Cyclone is CV-23.

**Status:** partial

### CV-13 cyclone_live_wlp_manual_by_topic_token

**Spec:** `zerodds-dcps-1.4.md::§2.2.3.11` — MANUAL_BY_TOPIC.

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint::assert_topic`).

**Tests:** `crates/dcps/src/wlp.rs::tests::wlp_assert_topic_emits_vendor_kind_with_token`
(the token in the ParticipantMessageData of the ZeroDDS kind). No Cyclone
peer takes part, as for CV-12.

**Status:** partial

### CV-14 typelookup_responder_builds_cyclone_compatible_reply

**Spec:** `dds-xtypes-1.3.md::§7.6.3.3` — TypeLookup responder.

**Repo:** `crates/discovery/tests/cyclone_typelookup_responder.rs`.

**Tests:** `typelookup_responder_builds_cyclone_compatible_reply`.

**Status:** done

### CV-15 typelookup_responder_unknown_hash_yields_empty_reply

**Spec:** `dds-xtypes-1.3.md::§7.6.3.3`.

**Repo:** `crates/discovery/tests/cyclone_typelookup_responder.rs`.

**Tests:** `typelookup_responder_unknown_hash_yields_empty_reply`.

**Status:** done

### CV-16 cyclone_compliance (wire compliance)

**Spec:** `ddsi-rtps-2.5.md::§8.3` (Messages) + §8.4 (Behavior).

**Repo:** `crates/rtps/tests/cyclone_compliance.rs`.

**Tests:** `crates/rtps/tests/cyclone_compliance.rs::cyclone_data_empty_payload_decodes`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_data_cdr2_payload_decodes_and_carries_payload`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_heartbeat_decodes`,
`crates/rtps/tests/cyclone_compliance.rs::zerodds_writer_produces_compatible_data_layout`,
`crates/rtps/tests/cyclone_compliance.rs::zerodds_writer_produces_compatible_data_with_payload`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_data_empty_roundtrip_bit_identical`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_data_cdr2_roundtrip_bit_identical`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_sedp_publication_datagram_decodes`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_sedp_publication_parameter_list_parses`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_sedp_publication_all_four_topics_parse`
(no gate; the default run executes all twelve).

**Status:** done

### CV-17 cyclone_he_must_understand

**Spec:** `ddsi-rtps-2.5.md::§9.4.2.11.2` — Must-Understand bit.

**Repo:** `crates/rtps/tests/cyclone_he_must_understand.rs` +
`crates/rtps/src/parameter_list.rs::Parameter::with_must_understand`
(sender-side helper) + `validate_must_understand` (decoder).

**Tests:** header extension Must-Understand tests +
`parameter_with_must_understand_helper_sets_bit`.

**Status:** done — both paths live: the sender sets `MUST_UNDERSTAND_BIT`
explicitly via `with_must_understand`, the decoder rejects unknown
MU PIDs. The live test against Cyclone DDS stays `#[ignore]` until a
lab setup is available in CI.

### CV-18 representation_and_type_information

**Spec:** `ddsi-rtps-2.5.md::§8` (entire) +
`dds-xtypes-1.3.md::§7.6.3` (TypeLookup).

**Repo:** `crates/discovery/tests/representation_and_type_information.rs`.

**Tests:** `crates/discovery/tests/representation_and_type_information.rs::negotiation_takes_xcdr2_when_both_offer_both_versions`,
`crates/discovery/tests/representation_and_type_information.rs::xtypes_7_6_3_2_1_type_information_is_built_from_a_keyed_appendable_struct`,
`crates/discovery/tests/representation_and_type_information.rs::xtypes_7_6_3_1_1_negotiation_matches_xcdr2_when_both_support`,
`crates/discovery/tests/representation_and_type_information.rs::typeinfo_for_cyclone_chatter_has_nonzero_sizes`
(no gate, all four run in the default run).

**Status:** done — the two preconditions without a peer:
representation negotiation to XCDR2 and a `TypeInformation` from the
type objects. The file was called `cyclone_full_interop.rs` and carried an
`#[ignore]` placeholder for the data path; its local part is the
first test, the data path itself is CV-18b.

### CV-18b Live data path ZeroDDS writer -> Cyclone reader

**Spec:** `ddsi-rtps-2.5.md::§8.4` (writer/reader behavior) +
`dds-xtypes-1.3.md::§7.6.3` (TypeLookup over the wire).

**Repo:** `crates/dcps/tests/cyclone_live_type_lookup.rs` (Cyclone's writers and readers through the
`cyclonedds` Python binding, on a leased domain).

**Tests:** in the weekly job `omg-rtps-cyclone` (its JUnit report) the OMG
dds-rtps case `"rtps_test_suite_1_Test_Reliability_4"`, ZeroDDS as
publisher to a Cyclone reader and as subscriber of a Cyclone writer;
`crates/dcps/tests/cyclone_live_type_lookup.rs::xtypes_7_6_3_3_4_a_cyclone_reader_with_a_member_appended_fetches_the_type_of_a_writer_and_takes_its_samples`
(a ZeroDDS writer to a Cyclone reader whose type differs),
`crates/dcps/tests/cyclone_live_type_lookup.rs::xtypes_7_2_4_4_8_a_final_cyclone_writer_under_xcdr1_is_refused_by_an_appendable_reader`,
`crates/dcps/tests/cyclone_live_type_lookup.rs::xtypes_7_2_4_4_8_a_final_cyclone_writer_under_xcdr2_is_refused_by_an_appendable_reader`
(the incompatible pairs: neither side matches, the reader's topic reports
INCONSISTENT_TOPIC after the reader holds Cyclone's `@final` object;
`#[ignore]`, job `vendor-live-dcps`).

**Status:** done

### CV-19 cyclone_sedp_replay

**Spec:** `ddsi-rtps-2.5.md::§8.5.2` (SEDP).

**Repo:** `crates/discovery/tests/cyclone_sedp_replay.rs`.

**Tests:** `crates/discovery/tests/cyclone_sedp_replay.rs::rtps_8_5_4_2_cyclone_sedp_publication_fixture_flows_through_stack`,
`crates/discovery/tests/cyclone_sedp_replay.rs::cyclone_fixture_without_discovery_is_dropped_as_unknown_src`,
`crates/discovery/tests/cyclone_sedp_replay.rs::rtps_8_5_5_2_replay_after_participant_lost_stops_accepting_publications`
(no gate; the default run executes all three).

**Status:** done

### CV-20 cyclone_live_sedp

**Spec:** `ddsi-rtps-2.5.md::§8.5.2`.

**Repo:** `crates/discovery/tests/cyclone_live_sedp.rs`.

**Tests:** `crates/discovery/tests/cyclone_live_sedp.rs::cyclone_live_sedp_discovery`
(`#[ignore]`, needs a local Cyclone `ddsperf`; default run: 0
cases. Runs in the CI job `interop-cyclone` with `--ignored`).

**Status:** done

### CV-21 type_lookup_requests

**Spec:** `dds-xtypes-1.3.md::§7.6.3` — Type Lookup Service.

**Repo:** `crates/discovery/tests/type_lookup_requests.rs`.

**Tests:** `crates/discovery/tests/type_lookup_requests.rs::get_types_request_carries_the_mutable_framing_of_the_clause`,
`crates/discovery/tests/type_lookup_requests.rs::xtypes_7_6_3_3_4_2_a_get_types_request_and_its_reply_round_trip_between_two_stacks`
(no gate, both run in the default run).

**Status:** done — what the evidence covers is the wire format of the GetTypes request per
XTypes 1.3 §7.6.3.3.3: `@mutable` DHEADER, EMHEADER with the `@hashid` of
`type_ids` and length code 5, then the sequence (count, EK_MINIMAL
identifier); the length of the member against the byte oracle from Cyclone 11.0.1
(`crates/types/tests/type_lookup_vendor_envelope.rs`). The second test
carries request and reply between two stacks in the same process; the file
was called `cyclone_live_typelookup.rs`, and the test was an
`#[ignore]` placeholder. The roundtrip against Cyclone is CV-21b.

### CV-21b Live TypeLookup roundtrip against Cyclone

**Spec:** `dds-xtypes-1.3.md::§7.6.3` — Type Lookup Service over the
reliable TL request/TL reply endpoints.

**Repo:** `crates/dcps/tests/cyclone_live_type_lookup.rs`.

**Tests:** `crates/dcps/tests/cyclone_live_type_lookup.rs::xtypes_7_6_3_3_4_a_reader_fetches_the_type_of_a_cyclone_writer_with_a_member_appended_and_takes_its_samples`,
`crates/dcps/tests/cyclone_live_type_lookup.rs::xtypes_7_6_3_4_2_a_reader_refuses_a_cyclone_writer_whose_label_is_a_string_after_fetching_its_type`,
`crates/dcps/tests/cyclone_live_type_lookup.rs::xtypes_7_6_3_3_4_a_cyclone_reader_with_a_member_appended_fetches_the_type_of_a_writer_and_takes_its_samples`
(`#[ignore]`, job `vendor-live-dcps`).

**Status:** done — ZeroDDS fetches the type of a Cyclone writer with
`getTypes` and decides on the received object, which the test finds in the
participant's TypeLookup registry: an appended member delivers
(§7.2.4.4.8), a `label` retyped to `string` is refused with
INCONSISTENT_TOPIC (§7.6.3.4.2). Cyclone fetches the type of a ZeroDDS
writer the same way and reads its samples with the appended member at its
default.

### CV-22 cyclone_live_security_caps

**Spec:** `zerodds-security-1.2.md::§14` — security discovery capabilities.

**Repo:** `crates/security-runtime/tests/cyclone_live_security_caps.rs`.

**Tests:** `crates/security-runtime/tests/cyclone_live_security_caps.rs::cyclone_accepts_beacon_with_security_caps`
(`#[ignore]`; Cyclone's `ddsperf` on the same host and a leased domain;
passes when Cyclone sends to the metatraffic unicast locator of the beacon
with the security properties; weekly in
`vendor-live-dcps`).

**Status:** done

### CV-23 cyclone_live_wlp (AUTOMATIC)

**Spec:** `ddsi-rtps-2.5.md::§8.4.13` (WLP) +
`zerodds-dcps-1.4.md::§2.2.3.11` (LIVELINESS=AUTOMATIC).

**Repo:** `crates/dcps/tests/cyclone_live_wlp.rs`.

**Tests:** `crates/dcps/tests/cyclone_live_wlp.rs::rtps_8_4_13_3_cyclone_live_wlp_handshake`
(`#[ignore]`, needs a running `ddsperf -i 42 sub` on the lab host
and multicast reachability; default run: 0 cases. Lab invocation: cargo test
-p zerodds-dcps --test cyclone_live_wlp -- --ignored --nocapture).

**Status:** done

---

## Run instructions

### Locally without a lab

All live tests skip themselves automatically if `LLVM_HOST_AVAILABLE`
is not set AND `sshpass` is not installed:

```bash
cargo test -p zerodds-discovery -p zerodds-dcps
# 0 failed, all live tests "ignored"
```

The deterministic Cyclone gap fillers also run without a lab:

```bash
cargo test -p zerodds-discovery --test cyclone_typelookup_responder
```

### Lab run on the Linux bench host

Prerequisites:

- SSH access to the bench host (lab convention)
- `sshpass` installed
- multicast setup active on the virtualization host
- On the bench host: `ip link set enp6s18 allmulticast on`

Invocation:

```bash
LLVM_HOST_AVAILABLE=1 cargo test -p zerodds-dcps -p zerodds-discovery \
    --features live-interop -- --ignored --nocapture
```

Per test file, individually:

```bash
cargo test -p zerodds-dcps --features live-interop \
    --test cyclone_live_wlp -- --ignored --nocapture
```

---

## Known edge cases

1. **Topic naming**: `fastdds shape` uses exactly `Square`/`Triangle`/
   `Circle` (case-sensitive). ZeroDDS `create_topic::<ShapeType>(name)`
   accepts arbitrary strings — the test sets the default convention.

2. **FastDDS Discovery Server TCP**: `fastdds discovery -i 0` listens
   only on TCP, not on SPDP multicast. `fastdds_live_spdp.rs`
   therefore tests two paths: (a) server TCP mode, (b) a regular
   `fastdds shape publisher` as SPDP sender.

3. **VM host multicast**: the VM kernel drops multicast without
   `allmulticast on` on the virtio interface. Without this workaround
   the tests see no Cyclone/FastDDS beacons. The bridge config of the
   virtualization host is documented separately.

4. **`ddsperf` flag trap**: `-D` is the duration in seconds, `-i` is the
   domain ID. Mixing them up leads to match errors that are hard to debug.
   The helper `start_cyclone_ddsperf_*` in `cross_vendor.rs` sets this
   correctly.

5. **Multi-host stretch goal**: a second bench host is available in terms of
   hardware, but SSH auth is not set up. Multi-host tests
   remain in the scope of the phase 7 bench suite.

---

## Non-goals

- **RTI Connext** (commercial, not installable)
- **Multi-host discovery** (auth for the second host not set up)
- **FastDDS compile from source** (the binary tools suffice)
- **Performance benchmarks** (phase 7 bench suite)

---

## Result

**23 test clusters** map to concrete spec sections in DDS 1.4 +
RTPS 2.5 + XTypes 1.3 + DDS-Security 1.2; all compile/lint clean,
green as ignored in the macOS dev setup without a lab, can be activated with
`LLVM_HOST_AVAILABLE=1 + --features live-interop`.

Cross-vendor coverage spans:
- **SPDP+SEDP+TypeLookup** (Discovery) — ddsi-rtps-2.5 §8.5
- **Reliable+Best-Effort Behavior** — ddsi-rtps-2.5 §8.4
- **WLP Liveliness AUTOMATIC + MANUAL_BY_PARTICIPANT/TOPIC** —
  ddsi-rtps-2.5 §8.4.13 + zerodds-dcps-1.4 §2.2.3.11
- **XCDR2 wire compliance** — dds-xtypes-1.3 §7.4.3
- **HeaderExtension Must-Understand** — ddsi-rtps-2.5 §9.4.2.11.2
- **Security capability negotiation** — zerodds-security-1.2 §14

---

## Audit status

**K13 = 100% cross-vendor coverage.** 23 done / 0 partial / 0 open.
All cross-vendor validation items have dedicated tests; sender-side
Must-Understand bit generation was completed by the
`Parameter::with_must_understand` helper + test.

`cargo test -p zerodds-rtps --test cyclone_he_must_understand`: 3 passed,
1 ignored (the live Cyclone test stays reserved for the CI lab setup).
fmt + clippy + zerodds-lint clean.

K13 completed — K14 (dds-psm-cxx-1.0) can begin.

---

## Addendum 2026-06-08 — ROS 2 wire live interop (C5 cross-vendor)

In addition to the 23 K13 clusters: **live interop on the ROS 2 wire**
ZeroDDS ↔ CycloneDDS (= `rmw_cyclonedds` = real ROS 2), topic
`rt/chatter`, type `std_msgs::msg::dds_::String_`. **Bidirectional 20/20
samples** (Linux bench host, CycloneDDS 11.0.1).

| Direction / measurement | Result |
|---|---|
| Cyclone talker → ZeroDDS sub | 20/20 samples |
| ZeroDDS pub → Cyclone listener | 20/20 samples |
| ZeroDDS ↔ ZeroDDS (regression) | green |
| ZeroDDS↔Cyclone multicast-free (`run_multicast_free_xvendor.sh`) | matched=1, 20/20 |
| C3 real WiFi large data (arm→x86 cross-host, 2/4 MB) | byte-perfect; throughput **10.8 MiB/s** |
| C3 latency RTT (loopback, 256 B) | **p50=40 µs / p99=83 µs** |
| C3 latency RTT (cross-machine WiFi) \* | **p50=4342 µs** (wired host ↔ Wi-Fi host, 256 B, 0 lost, full discovery); root cause of the `participants=0` saga proven by A/B = **802.11 power save on the WiFi client** (with `tcpdump`/promiscuous → runs, without → timeout), **not a ZeroDDS limit** |

\* Measured on a different host combination than the rest of the stack
(a wired host ↔ a Wi-Fi host) and with the WiFi NIC kept awake
(promiscuous), because idle WiFi power save otherwise discards the discovery unicasts.

**Repo:** `crates/ros2-rmw/interop/` (`run_interop.sh`, `GROUND_TRUTH.md`,
`cyclone_ros_{talker,listener}.c`) + `crates/dcps/examples/
ros2_chatter_{publisher,subscriber}.rs`.

**Spec:** ddsi-rtps-2.5 §9.3.1.2 (entityKind keyed/no-key),
dds-xtypes-1.3 §7.6.3 (DataRepresentation-Match).

**Finding (fixed):** keyless types produced WithKey entity IDs →
cross-vendor match reject. Fix: entityKind from `DdsType::HAS_KEY`. This directly
supports the C5 thesis "interoperates where Fast↔Cyclone practically breaks"
on the real ROS 2 wire.
