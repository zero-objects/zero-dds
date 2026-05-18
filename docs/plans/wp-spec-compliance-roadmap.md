# ZeroDDS v1.0 — Master-Roadmap zur vollstaendigen OMG-Spec-Compliance

**Stand:** 2026-04-25
**Scope:** v1.0 = full spec compliance ueber alle 13 OMG-Specs (kein Cherrypicking, kein "out-of-scope").
**Quellen:** Open-Listen aus `docs/spec-coverage/*.open.md` (13 Specs).
**Status-Dashboard:** `docs/plans/PROGRESS.md` (Live-Tracker fuer Done/Offen pro Cluster).

**Legende Status-Spalte:** ✅ done auf main · 🟡 partial · ⏳ next · — offen.

---

## 0. Lese-Anleitung

Diese Roadmap konsolidiert die Open-Items aus den 13 Open-Listen zu **Cross-Spec-Clustern**. Pro Cluster:
- involvierte Specs/Sections,
- Pflicht-Charakter (mandatory|conditional|optional),
- Interop-Risiko (blocker|degraded|none),
- Aufwand (S=1-3 Tage, M=1-2 Wochen, L=3-4 Wochen, XL=2+ Monate),
- Cross-Spec-Dependencies,
- Sicherheits-Risiko (kritisch|hoch|mittel|niedrig|n/a).

Cluster sind in **7 Phasen** gruppiert. Reihenfolge ist topologisch: Wire-Grundlagen → DCPS-API-Vollstaendigkeit → Security-Hardening → Type-System-Vervollstaendigung → Sprach-Bindings → Erweiterungen → DDS-XML-Konfiguration. Phase 7 (XML) kann frueh parallel laufen, hat aber keine Vorbedingungen aus dem Wire-Stack.

---

## 1. Phasen-Uebersicht (1-Blick)

| Phase | Titel | Inhalt | Cluster | Aufwand | Status | Bedingungen |
|---|---|---|---|---|---|---|
| **1** | Wire-Grundlagen | RTPS-Hardening, XCDR2-Encoding-Stack, KeyHash, MustUnderstand, HeaderExtension, GroupInfo | C1.1 - C1.10 | ~3-4 PM | ✅ done | keine |
| **2** | DCPS-API-Vollstaendigkeit | Listener, WaitSet, Conditions, SampleInfo, Instance-Lifecycle, Builtin-Topic-Reader, set_qos, find_topic | C2.1 - C2.10 | ~4-5 PM | ✅ done | Phase 1 |
| **3** | Security-Hardening | PKI-Handshake-Voll, Permissions-CA-Sig, Wire-Crypto-Konflikte, Stateless/Volatile-Topics, PSK-Builtins, Secure-Discovery | C3.1 - C3.9 | ~4-5 PM | ✅ done | Phase 1, teilweise Phase 2 |
| **4** | Type-System-Vervollstaendigung | DynamicType-API, TypeLookup-Service, alle Annotations, XML-Schema-Loader (XTypes-XSD), Builtin-Types | C4.1 - C4.7 | ~3-4 PM | ✅ done | Phase 1, Phase 2 (Listener) |
| **5** | Sprach-Bindings (parallel zu Phase 4) | IDL4-CPP, IDL4-CSharp, IDL4-Java, DDS-PSM-CXX, DDS-Java-PSM | C5.1 - C5.5 | ~9-12 PM (parallel) | ✅ alle 5 Cluster done (C5.5 Cross-Vendor 2026-04-26) | Phase 2 (DCPS-API stabil) |
| **6** | Erweiterungen | DDS-RPC, DDS-XRCE | C6.1, C6.2 | ~5-7 PM | — offen | Phase 1-4 |
| **7** | DDS-XML-Konfiguration | XML-Loader fuer QoS-Profile, Domains, Participants, Applications, Types, Samples | C7.A - C7.D | ~1-2 PM | ✅ done | Cluster A (well-formed-Parser) |

Hinweis: Alle Phase-2-/-3-/-4-Folge-Stufen (C2.2-b/-c, C3.4-c, C4.2-b,
C4.4-b, C4.5-b) sind 2026-04-26 abgeschlossen. Phasen 2/3/4 jetzt voll
konsolidiert.

**Gesamt v1.0:** ca. **30-40 PM** (12-16 Personenmonate sequentiell pro Phase + parallele Sprach-Bindings + Erweiterungen). Mit 3-4 Devs und sauberer Parallelisierung (Phase 5 parallel zu 4, Phase 7 parallel ab Phase 2): **~12-15 Kalendermonate**.

**Cyclone-Live-Interop ist der Acceptance-Test fuer Phase 1.** Alle weiteren Phasen werden zusaetzlich gegen Cyclone (und FastDDS, wo verfuegbar) gehaertet.

---

## 2. Cross-Spec-Verzahnungen (Topologie)

Diese Verzahnungen treiben die Reihenfolge:

1. **PL_CDR2/DHEADER/EMHEADER-Encoding-Stack** (xtypes §7.4.3) ist Voraussetzung fuer
   - **RTPS-KeyHash-Berechnung** (rtps §9.6.4.3, §9.6.4.8)
   - **Security AES-GCM-Key-Derivation + AAD** (security §10.5.3, §8.1)
   - **DCPS InstanceHandle-Sample-Routing** (dcps §2.2.2.5)
   - → Cluster **C1.2** (XCDR2-Stack) muss VOR C1.5 (KeyHash), C3.6 (Crypto-Wire), C2.4 (SampleInfo).

2. **TypeLookup-Service** (xtypes §7.6.3.3) braucht
   - **SEDP-Builtin-Endpoints** (rtps §8.5.4) ✓ (done)
   - **TypeObject-Hash** (xtypes §7.3.4) ✓ (done)
   - **Secure TypeLookup-Topics** (security §6.1, §7.5.5)
   - → Cluster **C4.2** Phase 4, mit Security-Hook aus Phase 3.

3. **PKI-Handshake mit hash_c1/c2** (security §10.3.4) braucht
   - **Cert-Bind** (security §7.4.3)
   - **DCPSParticipantStatelessMessage-Topic** (security §7.5.3)
   - **DCPSParticipantVolatileMessageSecure-Topic** (security §7.5.4)
   - → alles Phase-3-Cluster, intern reihenfolge-abhaengig.

4. **Builtin-Annotations** (xtypes §7.3.1) braucht
   - **IDL-Annotations-Apply-Logik** (idl-4.2 §1.10, §1.13)
   - **TypeObject-Generation** (xtypes §7.3) ✓ (done in WP 1.5)
   - → Cluster **C4.5** (XTypes-Annotations) hat IDL-Voraussetzung in **C4.6**.

5. **Sprach-Bindings (Phase 5)** brauchen alle:
   - **DCPS-Public-API stabil** (Phase 2 abgeschlossen)
   - **TypeObject + DynamicType-API** (Phase 4 fuer DynamicData)
   - **Codegen-Templates-Engine** (Sub-WP, parallelisierbar pro Sprache)

6. **DDS-RPC** (rpc) braucht
   - **IDL-RPC-Annotations** (idl-4.2 §1.10, §6, §7)
   - **RTPS-SubmessageExtension-Plugin** (Phase-2-Architektur, siehe Memory)
   - → Phase 6, nach Phase 4 (Type-System).

7. **DDS-XRCE** (xrce) braucht
   - **DDS-XML 1.0-Parser** (Phase 7) fuer XML-Profile
   - **XCDR2** (Phase 1) ✓
   - → Phase 6, sub-Phasen XRCE-Wire-Lite → XRCE-Reliable → XRCE-XML.

---

## 3. Phase 1 — Wire-Grundlagen (alles was Cross-Vendor blockiert)

**Ziel:** ZeroDDS spricht Wire-byte-identisch zu Cyclone DDS / FastDDS auf RTPS-2.5-Niveau, inkl. XCDR2 und allen Pflicht-PIDs. Acceptance: Live-Discovery + Pub/Sub gegen Cyclone fuer alle XCDR2-Topics.

### Cluster-Tabelle

| Cluster | Specs/§ | Pflicht | Risiko | Aufwand | Sec-Risk | Deps | Status |
|---|---|---|---|---|---|---|---|
| **C1.1** RTPS-Header & ProtocolVersion 2.5 | rtps §8.3.3.1.2 | mandatory | blocker | S | n/a | — | ✅ |
| **C1.2** XCDR2-Encoding-Stack vollstaendig (LC=6/7, D_CDR_BE/LE, PID_IGNORE) | xtypes §7.4.3.4.2, §7.4.1.2.1; rtps §10 | mandatory | blocker | M | n/a | — | ✅ |
| **C1.3** RTPS-HeaderExtension + Checksum | rtps §8.3.3.2, §9.4.5.2, §9.4.2.15, §3 (CRC-32C/CRC-64/MD5) | mandatory | degraded (forward-compat) | L | mittel | — | ✅ |
| **C1.4** Must-Understand-Bit + PID-Reject-Logik | rtps §9.4.2.11.2 | mandatory | degraded | S | niedrig | — | ✅ |
| **C1.5** KeyHash-Berechnung XCDR2 + Inline-QoS | rtps §9.6.4.3; security §7.4.4 | mandatory | blocker | M | niedrig | C1.2 | ✅ |
| **C1.6** GAP/HEARTBEAT GroupInfo + filteredCount + GSN-Felder | rtps §8.3.8.4.2, §8.3.8.6.2, §8.4.9.2.7 | mandatory | degraded | M | n/a | — | ✅ |
| **C1.7** InfoSource/InfoReply Decoder + Receiver-State-Update | rtps §8.3.8.9.4, §8.3.8.10.4 | mandatory | degraded | S | n/a | — | ✅ |
| **C1.8** Reliable HEARTBEAT FinalFlag-Default + Pre-Emptive ACKNACK | rtps §8.4.9.2.7, §8.4.2.3.4 | mandatory | blocker (timing-bug) | S | n/a | — | ✅ |
| **C1.9** Builtin-Endpoint-Set Bits + availableBuiltinEndpoints (PARTICIPANT_MESSAGE 10/11, TOPICS 28/29, Security 16-27) | rtps §9.3.2.12; security §7.4.7.1 | mandatory | blocker | M | hoch (Sec-Discovery) | — | ✅ |
| **C1.10** ParticipantMessageData / WLP / MANUAL_*-Liveliness Wire | rtps §8.4.13, §9.6.3.1; dcps §2.2.3.11 | mandatory | degraded | M | n/a | C1.9 | ✅ |

**Phase-1-Aufwand:** ~3-4 PM. Acceptance: Cyclone-Live-Discovery + Cyclone-Live-Pub/Sub fuer XCDR2-Topics gruen (ueber WP 0.6 Interop-Harness erweitert).

### WP-Sketches

#### WP 1.A — XCDR2-Encoding-Stack vollstaendig (C1.2)
**Specs:** xtypes §7.4.3.4.2 (LC=6/7), §7.4.1.2.1 (PID_IGNORE), rtps §10 (Encapsulation 0x0014/0x0015 D_CDR_BE/LE)
**Pflicht/Risiko:** mandatory, blocker
**Aufwand:** M
**Deliverables:**
- LC=6/7 Encoder-Pfad fuer prim-Arrays mit Length-Multiplikator
- D_CDR_BE/LE (0x0014/0x0015) Delimited XCDR2 fuer APPENDABLE
- PID_IGNORE 0x3F03 Decoder-Skip-Pfad mit Cyclone-Roundtrip-Test
- Test-Vektoren gegen Cyclone fuer LC=6/7 + APPENDABLE + PID_IGNORE
**DoD:**
- [ ] cargo test gruen
- [ ] byte-identischer Cyclone-Roundtrip fuer 5+ Type-Kategorien
- [ ] CI: 99% Branch-Coverage auf neuem Code
**Dependencies:** —

#### WP 1.B — KeyHash-Berechnung XCDR2 + Inline-QoS (C1.5)
**Specs:** rtps §9.6.4.3; security §7.4.4
**Pflicht/Risiko:** mandatory, blocker (Wire-Routing + Security-Persistence)
**Aufwand:** M
**Deliverables:**
- PLAIN_CDR2-BE-Serialization mit member-Reorder by memberId
- KeyHash-Inline-QoS-Wiring im DCPS-Encrypt-Pfad (Pflicht bei Encrypted Data, MD5 fallback)
- Cyclone-Roundtrip-Test fuer XCDR2-keyed-Topic
**DoD:**
- [ ] byte-identischer KeyHash gegen Cyclone fuer 3+ keyed Types
- [ ] Persistence-Service-Routing-Test (auch ohne Persistence-Service: KeyHash-Field korrekt)
**Dependencies:** WP 1.A (XCDR2-Stack)

#### WP 1.C — RTPS-HeaderExtension + Checksum (C1.3)
**Specs:** rtps §8.3.3.2, §9.4.5.2, §9.4.2.15
**Pflicht/Risiko:** mandatory, degraded (Forward-Compat + DoS-Protection)
**Aufwand:** L
**Deliverables:**
- HeaderExtension-Submessage Encode/Decode (alle Flags: Endianness, Length, Timestamp, U/W-Extension, Checksum, Parameters)
- CRC-32C, CRC-64/XZ, MD5-128 als Crypto-Primitive in `crates/foundation`
- Receiver-State-Update §8.3.7.4 (clockSkewDetected, etc.)
- Must-Understand-Bit-Reject in HE (whole-message-reject vs. submessage-skip)
**DoD:**
- [ ] CRC-Vektoren gegen RFC 4960 / ECMA-182 / RFC 1321
- [ ] Cyclone-Negative-Test (HE mit unbekannter PID + Must-Understand → reject ganze Message)
**Dependencies:** —

#### WP 1.D — Builtin-Endpoint-Set + WLP (C1.9, C1.10)
**Specs:** rtps §9.3.2.12, §8.4.13; security §7.4.7.1; dcps §2.2.3.11
**Pflicht/Risiko:** mandatory, blocker (Cyclone-Match-Verlust ohne diese Bits)
**Aufwand:** M
**Deliverables:**
- Bits 10/11 PARTICIPANT_MESSAGE_{R,W} announcen + matchen
- Bits 28/29 TOPICS_{ANN,DET} announcen
- Bits 16-27 Security-Discovery-Endpoints (Cross-Crate-Audit zw. rtps + security-rtps)
- ParticipantMessageData Wire-Encoding §9.6.3.1
- MANUAL_BY_PARTICIPANT/TOPIC Liveliness assert_liveliness-Wiring
**DoD:**
- [ ] WLP-Match mit Cyclone-LIVELINESS=AUTOMATIC und MANUAL_BY_PARTICIPANT
- [ ] Secure-Discovery-Bits konsistent mit security-rtps-Crate
**Dependencies:** —

#### WP 1.E — RTPS-Reliable-Hardening + GroupInfo (C1.6, C1.7, C1.8)
**Specs:** rtps §8.4.9.2.7, §8.3.8.4.2, §8.3.8.6.2, §8.3.8.9.4, §8.3.8.10.4
**Pflicht/Risiko:** mandatory, blocker fuer Cross-Vendor
**Aufwand:** M
**Deliverables:**
- HEARTBEAT FinalFlag=NOT_SET als Default fuer periodische HBs
- GAP filteredCount + gapStartGSN/gapEndGSN
- HEARTBEAT GroupInfo (currentGSN/firstGSN/lastGSN/writerSet)
- InfoSource + InfoReply Decoder + Receiver-State-Update
- ChangeKind ALIVE_FILTERED
**DoD:**
- [ ] Cyclone-Reliable-Test ueber 30% Packet-Loss durchgaengig gruen
- [ ] Pre-Emptive ACKNACK akzeptiert (kein crash)
**Dependencies:** —

#### WP 1.F — Phase-1-Hygiene (C1.1, C1.4)
**Specs:** rtps §8.3.3.1.2, §9.4.2.11.2
**Pflicht/Risiko:** mandatory, low-effort
**Aufwand:** S
**Deliverables:**
- ProtocolVersion 2.5 als Default in Header
- Must-Understand-Bit-Reject-Logic in `parameter_list.rs`
- Time-Konstanten TIME_ZERO/INVALID/INFINITE expose
- Locator-Konstanten LOCATOR_KIND_RESERVED + UDPv6 (Stub)
**DoD:**
- [ ] Cyclone-Header-Diff-Test
**Dependencies:** —

---

## 4. Phase 2 — DCPS-API-Vollstaendigkeit

**Ziel:** Vollstaendige Pflicht-API-Surface aus DDS-DCPS 1.4 §2.2 — set_qos/Listener/WaitSet/Conditions/SampleInfo-Statechart/Instance-Lifecycle/Builtin-Topic-Reader/find_topic. Acceptance: Cyclone-Apps die Listener+WaitSet+SampleInfo nutzen, sind portabel.

### Cluster-Tabelle

| Cluster | Specs/§ | Pflicht | Risiko | Aufwand | Sec-Risk | Deps | Status |
|---|---|---|---|---|---|---|---|
| **C2.1** Entity-Lifecycle: set_qos + enable + get_status_condition + get_instance_handle | dcps §2.2.2.1.1 | mandatory | degraded | L | n/a | — | ✅ |
| **C2.2** Status-Strukturen vollstaendig (13 Structs) + Listener-Hierarchie (5 Listener-Traits + Bubble-Up) | dcps §2.2.4.1, §2.2.2.2.3, §2.2.2.4.3-4, §2.2.2.5.6-7 | mandatory | degraded | XL | n/a | C2.1 | ✅\* |
| **C2.3** WaitSet + Conditions (StatusCondition/GuardCondition/ReadCondition/QueryCondition) | dcps §2.2.2.1.6 ff., §2.2.2.5.3.5 ff. | mandatory | degraded | L | n/a | C2.2 | ✅ |
| **C2.4** SampleInfo-Statechart komplett + Instance-Lifecycle (register/unregister/dispose/get_key_value/lookup_instance) | dcps §2.2.2.5.1, §2.2.2.4.2.5-14 | mandatory | blocker | XL | n/a | C1.5 (KeyHash) | ✅ |
| **C2.5** TopicDescription + find_topic + lookup_topicdescription | dcps §2.2.2.3.1, §2.2.2.2.1.11-12 | mandatory | degraded | M | n/a | — | ✅ |
| **C2.6** Builtin-Topic-Reader (DCPSParticipant/Topic/Publication/Subscription) | dcps §2.2.5; rtps §8.5 | mandatory | degraded | M | n/a | C2.2 | ✅ |
| **C2.7** ignore_* + delete_contained_entities + get_discovered_* | dcps §2.2.2.2.1.14-18, §2.2.2.2.1.27-30 | mandatory | degraded | M | n/a | C2.6 | ✅ |
| **C2.8** QoS-Vollstaendigkeit: PRESENTATION + LATENCY_BUDGET + LIVELINESS-MANUAL + TIME_BASED_FILTER + PARTITION-Wildcard + HISTORY/RES_LIMITS-Konsistenz + EXCLUSIVE-OWNERSHIP-Resolution | dcps §2.2.3.6, §2.2.3.8-13, §2.2.3.18-19, §2.2.3.23 | mandatory | degraded - blocker | L | n/a | C1.10 | ✅ |
| **C2.9** Coherent-Sets + Subscriber begin/end_access + wait_for_historical_data + suspend/resume_publications | dcps §2.2.2.4.1.8-11, §2.2.2.5.2.8-11, §2.2.2.5.3.32; rtps §8.7.5/6 (PID_COHERENT_SET) | mandatory (Presentation-driven) | degraded | M | n/a | C2.8 | ✅ |
| **C2.10** Phase-1-Hygiene: get_current_time, InstanceHandle/HANDLE_NIL, write-RES_LIMITS-Block, NOT_ALLOWED_BY_SECURITY, IDL-PSM-Konstanten | dcps §2.2.2.2.1.32, §2.3.3, §2.2.2.4.2.11; security §7.3.25 | mandatory | none/degraded | M | niedrig | — | ✅ |

\* C2.2 — Status-Strukturen + Listener-Traits sind als Standalone-Module gemerged. Listener-Slot-Integration in den Entities + Bubble-Up DR→Sub→DP ist als Folge-Stufe in Phase-3-bundle deferred.

**Phase-2-Aufwand:** ~4-5 PM.

### WP-Sketches (Auswahl)

#### WP 2.A — Listener-Hierarchie + Status-Strukturen (C2.2)
**Specs:** dcps §2.2.4.1, §2.2.2.{2,4,5}.3
**Pflicht/Risiko:** mandatory, degraded
**Aufwand:** XL
**Deliverables:**
- 13 Status-Strukturen (SAMPLE_LOST, SAMPLE_REJECTED, INCOMPATIBLE_QOS, …) mit total_count + total_count_change + Reset-Semantik
- 5 Listener-Traits (TopicListener, PublisherListener, DataWriterListener, SubscriberListener, DataReaderListener) + DomainParticipantListener als Aggregat
- Bubble-Up DR→Sub→DP wenn lokaler Listener None oder mask mismatch
- Re-Entrancy: Listener-Aufruf darf keinen Entity-Lock halten
**DoD:**
- [ ] alle Statuses durch echten Status-Quell-Pfad gefuellt (keine Counter-Stubs)
- [ ] Cyclone-Equivalent-Tabellen-Test (z.B. dds_get_subscription_matched_status diff)
**Dependencies:** C2.1

#### WP 2.B — SampleInfo + Instance-Lifecycle (C2.4)
**Specs:** dcps §2.2.2.5.1, §2.2.2.4.2.5-14
**Pflicht/Risiko:** mandatory, blocker
**Aufwand:** XL
**Deliverables:**
- Reader-side `BTreeMap<InstanceHandle, InstanceState>` mit alle 11 SampleInfo-Felder
- Statechart ALIVE/NOT_ALIVE_DISPOSED/NOT_ALIVE_NO_WRITERS × NEW/NOT_NEW
- sample_state-Tracking pro DR + view_state + generation_counts
- DdsType erweitert um `fn key(&self) -> Option<Vec<u8>>;`
- DATA(D=1) dispose, DATA(U=1) unregister, kombiniert D|U
- ALIVE→NOT_ALIVE_NO_WRITERS-Detection via Liveliness
**DoD:**
- [ ] Industrial-Telemetrie-Use-Case mit dispose roundtrip gruen
- [ ] Cyclone-Compatibility: DDS_NOT_ALIVE_DISPOSED korrekt propagiert
**Dependencies:** C1.5 (KeyHash), C2.2 (Listener)

#### WP 2.C — WaitSet + Conditions (C2.3)
**Specs:** dcps §2.2.2.1.6 ff., §2.2.2.5.3.5 ff.
**Pflicht/Risiko:** mandatory, degraded
**Aufwand:** L
**Deliverables:**
- WaitSet mit Mutex+Condvar
- StatusCondition pro Entity (alle Entities)
- GuardCondition (Boolean Trigger)
- ReadCondition + QueryCondition (mit Annex-B-SQL)
- read/take_w_condition + read/take_instance + read/take_next_instance
**DoD:**
- [ ] `wait(timeout)` korrekt-blockierend, Trigger-Setter wakt condvar
- [ ] Condition-Matrix-Test gegen Annex-B SQL-Subset
**Dependencies:** C2.2

#### WP 2.D — Builtin-Topic-Reader + ignore_* (C2.6, C2.7)
**Specs:** dcps §2.2.5, §2.2.2.2.1.14-30
**Pflicht/Risiko:** mandatory, degraded
**Aufwand:** M
**Deliverables:**
- Wrapper-Subscriber `get_builtin_subscriber()` mit 4 Builtin-DataReader
- Discovery-Cache zugaenglich
- ignore_participant/topic/publication/subscription mit Filter im Discovery-Pipeline
- delete_contained_entities rekursiv mit PRECONDITION_NOT_MET
- get_discovered_participants/_data/_topics/_topic_data
**DoD:**
- [ ] ddsperf/dds_ps gegen ZeroDDS-Builtin-Topics laeuft
**Dependencies:** C2.2

#### WP 2.E — QoS-Vollstaendigkeit (C2.8) + Coherent (C2.9)
**Specs:** dcps §2.2.3.{6,8,9,11,12,13,18-19,23}, §2.2.2.4.1.8-11, §2.2.2.5.2.8-11
**Pflicht/Risiko:** mandatory, degraded-blocker (Ownership=blocker fuer Redundanz)
**Aufwand:** L (C2.8) + M (C2.9)
**Deliverables:**
- PRESENTATION (access_scope INSTANCE/TOPIC/GROUP + coherent + ordered + PID_PRESENTATION)
- EXCLUSIVE-OWNERSHIP-Resolution (highest-strength-Tracking + Liveliness-driven failover)
- LATENCY_BUDGET, MANUAL_*-LIVELINESS, TIME_BASED_FILTER, PARTITION-Wildcard (POSIX fnmatch)
- HISTORY/RES_LIMITS-Konsistenz-Checks
- begin/end_coherent_changes + Publisher-Sequence-Counter + PID_COHERENT_SET
- begin/end_access + wait_for_historical_data + suspend/resume_publications
**DoD:**
- [ ] Cyclone-Cross-Vendor mit OWNERSHIP=EXCLUSIVE failover < 1s
- [ ] Coherent-Set atomic-Update-Test
**Dependencies:** C1.10 (Liveliness-Wire)

---

## 5. Phase 3 — Security-Hardening

**Ziel:** DDS-Security 1.2 vollstaendig und Cross-Vendor-interop (Cyclone-Security, FastDDS-Security). Sicherheits-kritische Pfade (Handshake, Permissions-Sig, Crypto-Wire) gehaertet. Acceptance: ZeroDDS-Security-Live-Handshake gegen Cyclone DDS Security.

### Cluster-Tabelle

| Cluster | Specs/§ | Pflicht | Risiko | Aufwand | Sec-Risk | Deps | Status |
|---|---|---|---|---|---|---|---|
| **C3.1** Vollstaendiger PKI-Handshake (hash_c1/c2 + cert-bind + signature + dh1/dh2 + challenge1/2) | sec §10.3.4, §7.4.3 | M-B | blocker | L | **kritisch** | C1.9 | ✅ done |
| **C3.2** Permissions-CA-Signature-Validation (S/MIME, RFC 5751) + Permissions-XML-Vollstaendigkeit (partitions, domain-filter, data_tags, relay) | sec §10.4.1.1, §10.4.1.3 | M-B | blocker | L | **kritisch** | — | 🟡 part |
| **C3.3** Wire-Crypto-Konflikte beheben: CryptoAlgorithmId-Mapping (§8.1 Tab.22) ✅ + class_id-Versionierung `:1.2` ✅ + KeyMaterial-Wire-Format (master_salt, sender_key_id) + Spec-konforme session_key-Derivation (HMAC-SHA256) + TransformKind im SEC_PREFIX | sec §10.5.2-3, §10.3.2.1, §10.4, §7.3.20 | M-B | **kritisch** | M | **kritisch** | C1.2 | 🟡 part |
| **C3.4** DCPSParticipantStatelessMessage-Topic (Auth-Handshake-Wire) + DCPSParticipantVolatileMessageSecure (Crypto-Token-Exchange) | sec §7.5.3, §7.5.4 | M-B | **kritisch** | L | **kritisch** | C1.9, C3.1 | 🟡 part |
| **C3.5** SPDP/SEDP Discovery-Erweiterungen: IdentityToken/PermissionsToken im Announce + Algorithm-Info-PIDs (0x1010-0x1013) + 18 Secure-Builtin-Endpoint-EntityIds + DCPSParticipantsSecure/PublicationsSecure/SubscriptionsSecure Topics | sec §7.5.1.4-8, §7.4.7.1, §7.3.11-15, §7.3.4-5 | M-B | **kritisch** | L | **kritisch** | C1.9 | ✅ done |
| **C3.6** SecureRTPSPrefixSubMsg + SecureRTPSPostfixSubMsg (SRTPS) + AAD im AES-GCM + encode/decode_rtps_message | sec §7.4.6.6, §7.4.7.8/9, §8.1, §10.5.3 | M-B (rtps_protection_kind != NONE) | mittel | L | mittel | C1.3 (HE) | — |
| **C3.7** Plugin-Vollstaendigkeit: alle CryptoKeyFactory/KeyExchange/Transform-Methoden + AccessControl-check_create_*/check_remote_* + Authentication-get_identity_token/return_*-Familie + ValidationResult Pending/Failed | sec §9.3.2 (Tab.29-31), §9.4.2 (Tab.39), §9.5.1 (Tab.42-44) | M-B | mittel | L | mittel | C3.1, C3.3 | — |
| **C3.8** PSK-Profil komplett (Auth+Access+Crypto+SRTPS-PreSharedKeyFlag) | sec §10.7-10.9 | optional (PSK-Profile) | none | L | niedrig | C3.6 | ✅ done |
| **C3.9** OCSP/CRL-Live-Checking + AuthenticationListener.on_revoke_identity + identity_status_token | sec §10.3.3.2, §9.3.2 (Tab.31), §7.5.1.6 | conditional (wenn enabled) | niedrig | M | hoch | C3.1 | — |

**Phase-3-Aufwand:** ~4-5 PM.

### WP-Sketches (Auswahl)

#### WP 3.A — PKI-Handshake-Vollstaendigkeit (C3.1)
**Specs:** security §10.3.4, §7.4.3 (Cert-Bind), §10.3.2 (Token-Strukturen)
**Pflicht/Risiko:** M-B, blocker, **kritisches Sicherheits-Risiko**
**Aufwand:** L
**Deliverables:**
- IdentityToken (general + PSK), PermissionsToken, AuthRequestMessageToken, HandshakeRequest/Reply/FinalMessageToken mit allen Spec-Feldern (c.id, c.perm, c.pdata, dsign_algo, kagree_algo, hash_c1, dh1, challenge1, ocsp_status, signature)
- hash_c1/c2 berechnung ueber Cert-Properties
- ECDHE-P256 mit Spec-konformer Wahl (zusaetzlich zu X25519)
- challenge1/2 (256-bit Random)
- signature ueber gesammelte Inhalte (RSASSA-PSS / ECDSA-P256)
- GUID-zu-Identity-Bindung (Anti-Squatter)
- AuthenticatedPeerCredentialToken
**DoD:**
- [ ] Live-Handshake gegen Cyclone-DDS-Security gruen
- [ ] MitM-Negative-Test (gefaelschtes Cert wird abgelehnt)
- [ ] GUID-Squatter-Test
**Dependencies:** C1.9 (Secure-Discovery-Bits)

#### WP 3.B — Permissions-CA-Sig + Permissions-XML-Voll (C3.2)
**Specs:** security §10.4.1.1, §10.4.1.3
**Pflicht/Risiko:** M-B, blocker, **kritisches Sicherheits-Risiko**
**Aufwand:** L
**Deliverables:**
- S/MIME-Parser fuer Permissions-Document-Signatur (RFC 5751)
- CA-Signature-Validation gegen `dds.sec.access.permissions_ca`
- Permissions-XML: `<partitions>`-Filter pro Grant + Domain-Filter + `<relay>` + `<data_tags>`
- `dds.sec.access.governance` + `dds.sec.access.permissions_ca` Properties
**DoD:**
- [ ] Cyclone-signed Permissions-XML wird akzeptiert
- [ ] gefaelschte (unsignierte) Permissions wird abgelehnt
**Dependencies:** —

#### WP 3.C — Wire-Crypto-Konflikte (C3.3)
**Specs:** security §10.5.2-3, §8.1 (Tab.22), §7.3.20, §10.3.2.1
**Pflicht/Risiko:** M-B, **kritisch** (Cross-Vendor-Wire-Compat)
**Aufwand:** M
**Deliverables:**
- BREAKING: CryptoAlgorithmId-Mapping korrigieren (0x01=AES128+GMAC, 0x02=AES128+GCM, 0x03=AES256+GMAC, 0x04=AES256+GCM)
- class_id-Versionierung: "DDS:Auth:PKI-DH:1.2", "DDS:Access:Permissions:1.2", "DDS:Crypto:AES-GCM-GMAC:1.2"
- KeyMaterial_AES_GCM_GMAC Wire: master_salt(32) + sender_key_id(4) + master_key + receiver_specific_key_id + receiver_key
- session_key-Derivation: HMAC-SHA256(master_sender_key, "SessionKey" || master_salt || session_id)
- TransformKind im SEC_PREFIX (echte Werte statt 16 byte all-null)
- CryptoToken im DataHolder-Format mit `binary_property "dds.cryp.keymat"`
- Property-URI-Schemas (`file:`, `data:`, `pkcs11:`)
**DoD:**
- [ ] Cyclone-Decrypt-Test gegen ZeroDDS-Encrypt gruen
- [ ] AES-GCM-Wire-Vektoren byte-identisch zur Spec
**Dependencies:** C1.2 (XCDR2-Stack)

#### WP 3.D — Stateless/Volatile-Topics (C3.4)
**Specs:** security §7.5.3, §7.5.4
**Pflicht/Risiko:** M-B, **kritisch**
**Aufwand:** L
**Deliverables:**
- DCPSParticipantStatelessMessage Best-Effort StatelessWriter/Reader (Sequenz-Predict-Robustheit)
- DCPSParticipantVolatileMessageSecure Reliable Stateful (VOLATILE)
- ParticipantGenericMessage-Format
- Auth-Handshake durch Stateless-Topic
- Crypto-Token-Exchange durch Volatile-Topic
**DoD:**
- [ ] vollstaendiger Handshake-Roundtrip gegen Cyclone (Wire-Capture)
**Dependencies:** C1.9 (Builtin-Endpoints), C3.1 (Handshake-Logik)

#### WP 3.E — Discovery-Erweiterungen Security (C3.5)
**Specs:** security §7.5.1.4-8, §7.4.7.1
**Pflicht/Risiko:** M-B, **kritisch**
**Aufwand:** L
**Deliverables:**
- IdentityToken + PermissionsToken in SPDP-Announce
- ParticipantSecurityDigitalSignatureAlgorithmInfo (PID 0x1010), KeyEstablishmentInfo (0x1011), SymmetricCipherInfo (0x1012-0x1013)
- 18 Secure-Builtin-Endpoint-EntityIds (Phase 1 hat Bits in C1.9 angekuendigt; jetzt die Endpoints)
- DCPSParticipantsSecure / PublicationsSecure / SubscriptionsSecure Topics
- DCPSParticipantMessageSecure (wenn is_liveliness_protected)
**DoD:**
- [ ] Cyclone-Live-Discovery zeigt Security-PIDs korrekt
- [ ] is_discovery_protected=true → Topic-Names leaken nicht
**Dependencies:** C1.9, C3.4

#### WP 3.F — SRTPS-Wrapping + RTPS-Header-AAD (C3.6)
**Specs:** security §7.4.6.6, §7.4.7.8/9, §8.1
**Pflicht/Risiko:** M-B (wenn rtps_protection_kind != NONE)
**Aufwand:** L
**Deliverables:**
- SecureRTPSPrefixSubMsg (0x33) + SecureRTPSPostfixSubMsg (0x34)
- AdditionalAuthenticatedDataFlag, PreSharedKeyFlag
- RTPS-Header AAD im AES-GCM (`Aad::from(rtps_header_bytes)`)
- encode_rtps_message + decode_rtps_message
- preprocess_secure_submsg + getrennte decode-Pfade DataWriter vs DataReader
**DoD:**
- [ ] Discovery-Header authentisiert
- [ ] Cyclone-rtps_protection_kind=ENCRYPT roundtrip gruen
**Dependencies:** C1.3 (HeaderExtension)

#### WP 3.G — Plugin-Vollstaendigkeit (C3.7)
**Specs:** security §9.3.2, §9.4.2, §9.5.1
**Pflicht/Risiko:** M-B, mittel
**Aufwand:** L
**Deliverables:**
- Authentication: get_identity_token, get_identity_status_token, set_permissions_credential_and_token, get_authenticated_peer_credential_token, set_listener, return_*-Familie
- AccessControl: check_create_participant/topic, check_remote_participant/topic, check_local_datawriter_register/dispose_instance + remote-Pendants, get_permissions_token, get_permissions_credential_token, get_*_sec_attributes
- CryptoKeyFactory: register/unregister_matched_remote_data{writer,reader}
- CryptoKeyExchange: create_local_data{writer,reader}_crypto_tokens + set_remote_* + return_crypto_tokens
- CryptoTransform: getrennte encode/decode_data{writer,reader}_submessage + preprocess_secure_submsg
- ValidationResult_t: PendingRetry + Failed
- SecureSubmessageCategory_t Routing (INFO/DATAWRITER/DATAREADER)
- SecurityException.minor_code
- DDS-Return-Code NOT_ALLOWED_BY_SECURITY
**DoD:**
- [ ] Per-Endpoint-Keys statt nur Per-Participant
- [ ] Memory-Leak-Test (return_*-Methoden geben Slots frei)
**Dependencies:** C3.1, C3.3

---

## 6. Phase 4 — Type-System-Vervollstaendigung

**Ziel:** DDS-XTypes 1.3 vollstaendig: DynamicData/DynamicTypeBuilder-API, TypeLookup-Service, alle Annotations + IDL-Apply-Logik, XML-Schema-Loader (Annex A), Builtin-Types. Acceptance: dynamische Tools (logger/monitor) funktionieren ueber DynamicType-API; Cyclone-TypeLookup-Service interop.

### Cluster-Tabelle

| Cluster | Specs/§ | Pflicht | Risiko | Aufwand | Deps |
|---|---|---|---|---|---|
| **C4.1** DynamicTypeBuilder + DynamicType + DynamicData (komplettes API-Set §7.5.2) | xtypes §7.5.2.* | mandatory | degraded | XL | C2.2 (Listener-Framework) | ✅ done (Foundation) |
| **C4.2** TypeLookup-Service vollstaendig + complete_to_minimal-Mapping + Service-Instance-Name-Format + Secure TypeLookup-Topics | xtypes §7.6.3.3, §7.6.3.3.4; sec §6.1, §7.5.5 | mandatory | degraded | L | C3.5 | 🟡 part — Server+Client+Registry+Pagination+ServiceName done; DCPS-Hot-Path-Trigger + Reliable-Wiring + complete_to_minimal-Pair-Tabelle Folgestufe |
| **C4.3** XML-Schema-Loader Annex A + create_type_w_uri / create_type_w_document | xtypes Annex A, §7.5.2 | mandatory (DynamicType-Pflicht) | degraded | L | C7.A (xml-Foundation) | 🟡 part — URI-Loader + Strict/Lax done; TypeObject-Bridge nach C4.1 |
| **C4.4** TypeConsistencyEnforcement: TryConstruct-Apply (DISCARD/USE_DEFAULT/TRIM) + @ignore_literal_names + @non_serialized-Compat-Filter + @data_representation-Mask-Match + Single-Inheritance-Edge-Cases | xtypes §7.2.4.4.7-8, §7.2.2.7, §7.3.1.2.1.13-14 | mandatory | degraded | M | — |
| **C4.5** Builtin-Annotations + Apply-Logik durch IDL: @default(value=...), @verbatim-PlacementKind, @topic-Lookup, @hashid-Voll, @bit_bound/@position-Bitmask | xtypes §7.2.2.4.4.4.9, §7.3.1.2.1; idl §1.10, §1.13, §6, §7 | mandatory | degraded | M | C4.6 | ✅ done (Bridge IDL-`Lowered` → DynamicType-Descriptor via `apply_to_member`/`apply_to_type`; @key/@id/@optional/@must_understand/@external/@default/@nested/@extensibility/@final/@appendable/@mutable/@position/@default_literal voll; @autoid/@verbatim/@unit/@hashid/@bit_bound als Passthrough-Report) |
| **C4.6** IDL 4.2 Spec-Treue: Konstanten-Evaluator + String-Concat + Identifier-Case-Insensitive + Name-Resolver-Voll + Forward-Decl-Check + Anon-Types-AST + Annotation-Decl + Union-Validierung + Bitfield/Bitmask-Validierung + Profile-Selektor + Preprocessor-Hardening | idl Phase-1: 1.1-1.13, 4, 5, 6, 7, 10 | mandatory | correctness/blocker | XL | — | ✅ done (Phase-1) |
| **C4.7** Builtin-Types + Sample-XML-Codec + INCONSISTENT_TOPIC-Listener-Callback + lookup_topicdescription mit index | xtypes §7.6.5, Annex E, §7.6.4.2, §7.6.3.4.2 | mandatory | degraded | M | C4.1 | 🟡 part — TryConstruct-Apply done; Builtin-Types + Sample-XML-Codec + INCONSISTENT_TOPIC-Listener offen |

**Phase-4-Aufwand:** ~3-4 PM.

### WP-Sketches (Auswahl)

#### WP 4.A — DynamicType + DynamicData Voll-Stack (C4.1)
**Specs:** xtypes §7.5.2.*
**Pflicht/Risiko:** mandatory, degraded
**Aufwand:** XL
**Deliverables:**
- DynamicTypeBuilderFactory, DynamicTypeBuilder, DynamicType, DynamicTypeMember, MemberDescriptor, TypeDescriptor, AnnotationDescriptor
- DynamicData, DynamicDataFactory, DynamicTypeSupport, DynamicDataWriter/Reader
- loan_value/return_loaned_value (max 1 outstanding loan, Rust-Lifetime/RefCell)
- Promotions-Tabelle (Int8→{Int16/32/64,Float32/64/128}, Char8→{...}, Boolean-Promotions) im Get/Set
- get_item_count je TypeKind (Bitmask: Set-Flags-Count)
- member_by_name Konsistenz-Checks pro TypeKind
**DoD:**
- [ ] generischer DDS-logger (read alle Topics dynamisch) funktioniert
- [ ] Rust-Borrow-Sicherheit fuer Loan-Pattern beweisbar
**Dependencies:** C2.2

#### WP 4.B — TypeLookup-Service voll (C4.2)
**Specs:** xtypes §7.6.3.3
**Pflicht/Risiko:** mandatory, degraded
**Aufwand:** L
**Deliverables:**
- complete_to_minimal-Mapping-Field in TypeLookup_getTypes_Out
- Service-Instance-Name `dds.builtin.TOS.<16-hex GUID>`
- Secure TypeLookup-Topics (§7.5.5)
**DoD:**
- [ ] Cyclone-TypeLookup-Roundtrip
**Dependencies:** C3.5 (Secure-Discovery)

#### WP 4.C — IDL 4.2 Spec-Treue (C4.6)
**Specs:** idl-4.2 Phase-1-Items
**Pflicht/Risiko:** mandatory, correctness/blocker fuer Codegen
**Aufwand:** XL
**Deliverables:**
- Konstanten-Evaluator (Type-Promotion, Octet 0..255, Enum-Resolution, Fixed-Point 62-Digit-Doppel-Precision)
- String-Concat + Escape-Decoding
- Identifier-Case-Insensitive + Escape-Identifier-Praefix
- Name-Resolver Voll + Module-Reopen + Diamond-Inheritance
- Forward-Decl-Completion-Check
- Anonymous Sequence/String/Array in Member-Position (Rules 216/217)
- `>>`-Whitespace-Disambiguation
- Bitfield-Size-Validierung + @bit_bound/@position-Validierung
- User-Defined `@annotation` Declarations (Rules 218-223)
- Union-case_label distinct-Check
- Native-Type
- Annotation-Multi-Definition-Konsistenz
- Profile-Selektor (Plain DDS / Extensible / CORBA)
- Preprocessor: Backslash-Newline, #error/#warning (Phase-1 subset)
**DoD:**
- [ ] DDS-XTypes-konforme TypeObjects fuer alle Spec-Beispiele
- [ ] CORBA-Konstrukte werden im DDS-Profil abgelehnt mit klarer Fehlermeldung
**Dependencies:** —

#### WP 4.D — XML-Schema-Loader Annex A (C4.3)
**Specs:** xtypes Annex A
**Pflicht/Risiko:** mandatory (DynamicType voraussetzt), degraded
**Aufwand:** L
**Deliverables:**
- XML→TypeObject-Mapper
- create_type_w_uri / create_type_w_document (file://, XML-Type-Definition)
- alle Types aus Annex A (struct, enum, union, typedef, bitmask, bitset)
**DoD:**
- [ ] Cyclone-XML-Type-Files werden geladen → byte-identische TypeObjects
**Dependencies:** C7.A (well-formed XML-Parser)

#### WP 4.E — TryConstruct + Annotations-Apply (C4.4, C4.5)
**Specs:** xtypes §7.2.4.4.7-8, §7.2.2.7
**Pflicht/Risiko:** mandatory, degraded
**Aufwand:** M
**Deliverables:**
- TryConstruct-Bits-Apply: DISCARD/USE_DEFAULT/TRIM auf Sequenzen/Strings im Decoder
- @ignore_literal_names Apply-Logik
- @non_serialized-Compat-Filter
- @data_representation-Mask-Match (Tab.59)
- @default(value=...) Member-Construct
- @verbatim-PlacementKind
- @topic-Lookup
- Single-Inheritance Edge-Cases (gleicher Name oder ID in Base+Derived)
**Dependencies:** C4.6

---

## 7. Phase 5 — Sprach-Bindings (parallel zu Phase 4)

**Ziel:** Vollstaendige Sprach-Bindings fuer C++, C#, Java mit IDL-Codegen + DDS-PSM. Acceptance: Bestandscode-Migration von RTI/OpenDDS/Cyclone DDS C++/Java/C# moeglich; Cross-Vendor-Wire-Compat byte-identisch.

### Cluster-Tabelle

| Cluster | Specs | Pflicht | Risiko | Aufwand | Sec-Risk | Deps | Status |
|---|---|---|---|---|---|---|---|
| **C5.1** IDL4-CPP-Codegen + omg::types-Runtime-Header | idl4-cpp-1.0 | mandatory (fuer C++-Apps) | blocker (C++-Migration) | ~2-3 PM | n/a | C4.6 | ✅ done (C5.1-a Blocks A-E + C5.1-b Blocks F-H, 135+ Tests, `crates/idl-cpp/`) |
| **C5.2** DDS-PSM-CXX 1.0 (dds::core/domain/topic/pub/sub/qos/xtypes Namespaces + Reference/Value-Pattern + Listener/Status/Condition/WaitSet) | dds-psm-cxx-1.0 | mandatory | blocker | ~3-4 PM | n/a | C5.1, Phase 2 | ✅ done (Header-Skeleton + 5 Templates, 11 Integration-Tests, `crates/idl-cpp/src/psm_cxx.rs`); voller Reference-Pattern als Phase-2-Erweiterung |
| **C5.3** IDL4-CSharp-Codegen + Omg.Types-Runtime-Lib + DDS-Integration via FFI | idl4-csharp-1.0 | mandatory (fuer C#-Apps) | blocker | ~3-4 PM (12-16 Wo) | n/a | Phase 2, C4.6 | ✅ C5.3-a + C5.3-b done (144 Tests, ISequence/IBoundedSequence + 7 Annotations + ITopicType-Marker, `crates/idl-csharp/`); FFI-Live-Wiring Phase-6 |
| **C5.4** IDL4-Java-Codegen + org.omg.type-Runtime-JAR + Bitset/Bitmask + Annotations + Value-Types | idl4-java-1.0 | mandatory (fuer Java-Apps) | blocker | ~2-3 PM | mittel (verbatim-Code-Inject) | C4.6 | ✅ C5.4-a + C5.4-b done (154 Tests, Bitset/Bitmask via EnumSet, Multi-Inh via Companion-Interface, @value(N), 7 Annotations, TopicType, `crates/idl-java/`); JNI Phase-6 |
| **C5.5** DDS-Java-PSM 1.0 (org.omg.dds.* Package-Tree + ServiceEnvironment-SPI + Pure-Java-Implementation zu Rust-Core + Pub/Sub/Sample/Selector + DynamicType-Java-API) | zerodds-java-psm-1.0 | mandatory | blocker | ~3-4 PM | mittel (JNI-unsafe) | C5.4, Phase 2, C4.1 | offen |

**Phase-5-Aufwand parallel:** ~9-12 PM (3 Sprachen parallel mit 3 Devs ~3-4 Monate).

### Cross-Cutting Foundation

Alle Sprach-Bindings brauchen:
- **Codegen-Template-Engine** (Tera/MiniJinja) — als gemeinsamer Crate `idl-codegen-core` mit pro-Sprache-Templates.
- **FFI-Architektur-Entscheidung:** cxx-rs vs. autocxx vs. C-Shim (fuer C++/C#); JNI vs. Panama (fuer Java).
- **Sanitizer-CI** (ASan/MSan/TSan) fuer C++-Build.
- **Sprach-spezifische Test-Pipeline:** `dotnet build/test`, `mvn test`, `cmake+ctest`.

### WP-Sketches (Auswahl)

#### WP 5.A — IDL4-CPP + DDS-PSM-CXX Welle 1 (C5.1+C5.2 Phase 3a/3b/3c)
**Specs:** idl4-cpp-1.0, dds-psm-cxx-1.0 Block A-H
**Pflicht/Risiko:** mandatory, blocker
**Aufwand:** ~6-8 PM
**Deliverables:**
- omg::types Runtime-Header (header-only, alias auf std::*)
- Codegen-Skeleton: Names/Reserved/Modules/Constants/Basic-Types/Typedefs/struct/enum/array/sequence/string/map
- Union-Codegen mit Mehrfach-Case + Default + Discriminator-Param
- Annotations: @optional, @external, @bit_bound, @value, @default_literal, @cpp_mapping
- DDS-PSM-CXX Block A (Header-Layout) + B (Type-Mapping) + C (Reference/Value-Pattern) + D (Exception) + E (Time/Duration) + F (Status) + G (QoS-Policy+Traits) + H (Domain/Topic/Pub/Sub)
- C++11-Kompat (Block K), Concurrency/Reentrancy (Block L)
**DoD:**
- [ ] Cyclone-DDS-CXX-App kompiliert nach Header-Tausch
- [ ] TSan-Pipeline gruen
**Dependencies:** C4.6 (IDL-Voll)

#### WP 5.B — IDL4-CSharp + DDS-CSharp-Integration (C5.3 Phase 3.1-3.5)
**Specs:** idl4-csharp-1.0
**Pflicht/Risiko:** mandatory, blocker fuer .NET-Migration
**Aufwand:** ~3-4 PM
**Deliverables:**
- crates/idl-codegen-csharp + zerodds-cs-runtime/ (.NET Standard 2.0+)
- Naming-Engine + Tab. 8.1 + Reserved-Names
- Type-Mapping + Constructed Types + ISequence<T>-Runtime + Bitset/Bitmask
- Annotations + Standardized-Annotation-Impact
- Any + Interfaces + Exceptions + Valuetypes
- IDLC-Subcommand `idlc emit csharp`
- DDS-Integration via P/Invoke gegen Rust-Core
- NuGet-Packaging
**DoD:**
- [ ] RTI-Connext-CSharp-App-Migration: gleiche IDL → kompiliert mit ZeroDDS-CSharp-Stack
- [ ] NativeAOT-Pfad funktioniert
**Dependencies:** Phase 2 (DCPS), C4.6 (IDL-Voll)

#### WP 5.C — IDL4-Java + DDS-Java-PSM (C5.4+C5.5)
**Specs:** idl4-java-1.0, zerodds-java-psm-1.0 Cluster A-H + K-N
**Pflicht/Risiko:** mandatory, blocker
**Aufwand:** ~5-6 PM
**Deliverables:**
- Architektur-Spike: JNI vs. Panama (1-2 Tage Vorlauf)
- Runtime-JAR org.omg.type.* (Holder, BooleanSeq, etc.)
- IDL→Java-Codegen (Annotations, Bitset/Bitmask, Value-Types)
- DDS-Java-PSM: org.omg.dds.* Package-Tree
- ServiceEnvironment-SPI mit System-Property-Lookup
- DDSException-Hierarchie + ReturnCode_t-Mapping (TimeoutException checked, Rest unchecked)
- Value-Typen + QoS-DSL (withPolicy/withPolicies)
- Entity<E,Q,L> + Listener-Adapter + WaitSet/Condition
- DomainParticipantFactory + TypeSupport + Topic + ContentFilteredTopic
- Publisher/DataWriter<T> + Subscriber/Sample/DataReader<T>/Selector + Loan-Pattern via Closeable
- DynamicType-Java-API Bridge zu Rust
- omgdds.jar + omgdds_src.zip vendoring (third_party/omgdds_src/)
- Maven-Release-Pipeline + Cleaner statt finalize()
**DoD:**
- [ ] OpenSplice-Java-App kompiliert nach Re-Pointing
- [ ] try-with-resources funktioniert ueber alle Entities
**Dependencies:** Phase 2 (DCPS), C4.1 (DynamicType), C4.6 (IDL-Voll)

---

## 8. Phase 6 — Erweiterungen (DDS-RPC + DDS-XRCE)

**Ziel:** DDS-RPC 1.0 (Basic + Enhanced) und DDS-XRCE 1.0 (Wire-Lite + Reliable + XML).

### Cluster-Tabelle

| Cluster | Specs/§ | Pflicht | Risiko | Aufwand | Sec-Risk | Deps |
|---|---|---|---|---|---|---|
| **C6.1.A** RPC Common-Types + IDL-Annotations + Topic-Naming | rpc §7.5.1.1.1, §7.8.2, §7.3, §7.4 | M (Basic-Conformance) | none | M | n/a | C4.6 |
| **C6.1.B** RPC Basic-Codegen + Enhanced-Codegen + Discovery-Extensions (PIDs 0x0080-0x0083) + PID_RELATED_SAMPLE_IDENTITY | rpc §7.5.1.1, §7.5.1.2, §7.6.2, §7.8.2 | M / O (Enhanced) | mittel | L | n/a | C6.1.A, RTPS-SubmessageExtension-Plugin |
| **C6.1.C** RPC Requester/Replier-Runtime + QoS-Profile-Resolution | rpc §7.9, §7.10, §7.11 | M | hoch (Threading) | XL | n/a | C6.1.B, Phase 2 |
| **C6.1.D** RPC PSM-Bindings (C++/Java) | rpc §10, §11 | conditional | mittel | XL | n/a | C5.2, C5.5, C6.1.C |
| **C6.2.A** DDS-XRCE Wire-Lite (16 Submessages, Stream-Modell, RFC-1982 Serial-Number, XCDR2-Encoding, UDP-Mapping) | xrce §8, §11.1 | M (UDP-Conformance) | none | L | mittel (DTLS noetig) | C1.2 |
| **C6.2.B** DDS-XRCE Object-Model + Reliable-Stack (ACKNACK/HEARTBEAT/FRAGMENT/RESET/TIMESTAMP) + Continuous-Read | xrce §7, §8.4.14 | M | mittel | L | n/a | C6.2.A |
| **C6.2.C** DDS-XRCE XML/File-Configuration (REPRESENTATION_AS_XML_STRING, §9.3) | xrce §7.7.3, §9.3 | M (File-Config-Conformance) | niedrig | M | n/a | C6.2.B, Phase 7 |
| **C6.2.D** DDS-XRCE TCP/Serial Transports + TLS/DTLS | xrce §11.3, Annex C | M (TCP/Serial-Conformance) | mittel | L | mittel | C6.2.B |

**Phase-6-Aufwand:** ~5-7 PM (RPC ~3-4 PM + XRCE ~3-4 PM, parallelisierbar).

---

## 9. Phase 7 — DDS-XML-Konfiguration (parallel ab Phase 2)

**Ziel:** DDS-XML 1.0 Building Blocks vollstaendig: well-formed-Parser-Foundation, QoS-Library, Domains/Participants/Applications, Type-XML, Sample-XML.

**Strategie-Hinweis:** Kein Wire-Interop-Blocker, aber wichtigster Migrations-Hebel fuer RTI/FastDDS-Kunden. Cluster F (Foundation) sollte frueh in Phase 2 starten, da auch von Phase 4 (XML-Schema-Loader Annex A) und Phase 6 (XRCE XML-Config) gebraucht.

### Cluster-Tabelle

| Cluster | Specs/§ | Pflicht | Risiko | Aufwand | Deps | Status |
|---|---|---|---|---|---|---|
| **C7.A** Foundation: well-formed XML-Parser (`crates/xml/`), Element/Attribute-Value-Typen (boolean case-sensitive, hex-Long, LENGTH_UNLIMITED, DURATION_INFINITY), Octet-Sequences (dec/hex/Base64-B64) | zerodds-xml §7.1, §7.2.4.2 | mandatory (BB-atomar) | none | S | — | ✅ |
| **C7.B** Building Block QoS: `<qos_library>` + `<qos_profile>` + 22 Policies + Profile-Inheritance (`base_name`) + Topic-Filter (Glob) + Single-QoS-Shortcut | zerodds-xml §7.3.2 | mandatory (BB-atomar) | none | M | C7.A | ✅ done |
| **C7.C** Building Block Domains/Participants/Applications: `<domain_library>`, `<domain_participant_library>`, `<application_library>` + Inline-Entity-QoS + Participant-Inheritance + DCPS-Auto-Wire-up | zerodds-xml §7.3.4-6 | mandatory (BB-atomar) | none | M | C7.B, Phase 2 (DCPS-Factory) | ✅ done |
| **C7.D** Building Block Types (`<types>` mit struct/enum/union/typedef/bitmask/bitset) + Sample-XML-Codec (Building Block Data Samples) + optional XSD-1.1-Validator (Cluster A) | zerodds-xml §7.3.3, §7.3.7, §3 | mandatory (BB-atomar fuer Types); optional (XSD strict) | none | M-L | C7.A, C4.1 (XTypes-AST) | ✅ done |

**Phase-7-Aufwand:** ~1-2 PM. Unabhaengig parallelisierbar.

---

## 10. Topologische Reihenfolge (was-blockiert-was)

```
Phase 1 (Wire-Grundlagen)
  └─→ Phase 2 (DCPS-API)
        ├─→ Phase 3 (Security-Hardening)   [braucht C1.9, C1.3]
        ├─→ Phase 4 (Type-System)          [braucht C2.2 fuer Listener]
        │     └─→ Phase 5 (Sprach-Bindings) [braucht C4.6 IDL + C4.1 DynamicType]
        │           └─→ Phase 6.RPC-PSM    [braucht C5.2, C5.5]
        └─→ Phase 6.RPC-Runtime            [braucht Phase 2]
              └─→ Phase 6.XRCE             [braucht C1.2 XCDR2 + Phase 7 XML]

Phase 7 (DDS-XML)  [parallel ab Phase 2]
  └─→ Phase 4.XML-Schema-Loader (C4.3)
  └─→ Phase 6.XRCE.XML-Config (C6.2.C)
```

**Kritischer Pfad fuer v1.0:**
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 (parallel) → Phase 6.

**Cyclone-Live-Interop-Acceptance pro Phase:**
- Phase 1: Discovery + Pub/Sub XCDR2 byte-identisch
- Phase 2: SampleInfo, ddsperf/dds_ps, OWNERSHIP=EXCLUSIVE failover
- Phase 3: Live PKI-Handshake gegen Cyclone Security
- Phase 4: TypeLookup-Roundtrip + DynamicType-Logger
- Phase 5: C++/C#/Java-Apps von Cyclone/RTI portierbar
- Phase 6: RPC gegen RTI-Connext-RPC; XRCE gegen Micro-XRCE-DDS-Agent

---

## 11. Aufwands-Aggregat & Realistische Personenmonate

| Phase | Aufwand (PM, 1 Dev) | Aufwand (Kalender-Monate, 3-4 Devs) |
|---|---|---|
| 1 | 3-4 | 1-1.5 |
| 2 | 4-5 | 1.5-2 |
| 3 | 4-5 | 1.5-2 |
| 4 | 3-4 | 1-1.5 |
| 5 (parallel zu 4) | 9-12 | 2.5-3 |
| 6 | 5-7 | 2-2.5 |
| 7 (parallel ab 2) | 1-2 | 0.5 |
| **Summe v1.0** | **~30-40 PM** | **~10-12 Kalender-Monate** |

**Annahmen:**
- 3-4 Devs mit DDS/RTPS-Erfahrung
- Phase 5 startet parallel zu Phase 4 (1 Dev pro Sprache)
- Phase 7 laeuft als Hintergrund-Track (0.25 Dev) ab Phase 2
- Jede Phase mit Cyclone-Live-Interop-Tests im CI gehaertet

---

## 12. Risiko-Register (Top 10 fuer v1.0)

| # | Risiko | Wahrscheinlich | Impact | Mitigation |
|---|---|---|---|---|
| 1 | Cyclone-Wire-Drift (RTPS-2.5 → 2.6 mid-flight) | mittel | hoch | Wire-Capture in CI permanent gegen aktuelle Cyclone-Releases |
| 2 | DDS-Security-Cross-Vendor-Bugs (Cyclone-Sec hat eigene Eigenheiten) | hoch | hoch | Phase 3 mit echtem Cyclone-Security-Setup (Test-CA, Test-Permissions) |
| 3 | Sprach-Binding FFI-Crashes (C++ Reference/Value-Lifecycle) | hoch | mittel | TSan/ASan-CI ab Phase 5-Start; SAFETY-Comments-Pflicht |
| 4 | DynamicType-API Borrow-Sicherheit (Loan-Pattern) | mittel | mittel | Rust-Lifetime-Analyse + Property-Test |
| 5 | DDS-RPC Cross-Vendor-Validierung (Cyclone hat kein RPC) | hoch | mittel | RTI-Connext-Lizenz fuer CI; sonst Spec-Konformitaet ohne Live-Validierung |
| 6 | XRCE-Wire-Drift (Micro-XRCE-DDS Eigenheiten) | mittel | niedrig | Annex-B-Vektoren + Live-Test gegen eProsima-Agent |
| 7 | IDL-Parser-Performance (CST-exponentiell — bereits geloest, aber Phase 4 erweitert Grammatik) | niedrig | mittel | Memoization-Pass beibehalten, Coverage-Test mit grossen IDLs |
| 8 | Permissions-CA-S/MIME-Parser (RFC 5751-Edge-Cases) | mittel | hoch | `cms`-Crate evaluieren; Cyclone-signed Files als Fixtures |
| 9 | OCSP-Live-Stub vs. echter OCSP-Responder | niedrig | mittel | Lokaler OCSP-Mock (RFC 6960) im CI |
| 10 | Sprach-Binding-Aufwand wird unterschaetzt (idl4-cpp allein 30-40 Tage) | hoch | hoch | Phase 5 fruh anfangen, parallel zu Phase 4 |

---

## 13. Offene Strategie-Entscheidungen

1. **C++-Toolchain:** CMake vs. Bazel vs. Cargo+cc-rs — aktuelle Tendenz CMake mit FetchContent (siehe idl4-cpp-1.0 Open-List).
2. **Java-FFI:** JNI vs. Panama (Java 22+) — Architektur-Spike vor Phase 5 noetig.
3. **CryptoAlgorithmId-BREAKING-Change** (C3.3): Wann eingespielt? Vor erstem PKI-Live-Test, da sonst neuer Mismatch entsteht.
4. **DDS-RPC PSM-Defer:** PSM-Bindings (C6.1.D) on-demand beim ersten Pilot-Kunden, nicht spec-getrieben.
5. **Cluster J — Reflection-basierte Java-TypeRep** (zerodds-java-psm §8): Stretch-Goal Phase 7 oder n/a — IDL→Java-Codegen-Pfad (Cluster K) erfuellt Conformance §2 alleine.
6. **CORBA-Profile** (alle idl4-*-Specs): dauerhaft n/a — wird in v1.0 explizit dokumentiert, nicht implementiert.
7. **DynamicData-API in Phase 4 vs. Phase 5:** kann auch in Phase-5-Bindings als Phase-4-Voraussetzung liegen — aktuell als Phase-4-Cluster modelliert.

---

## 14. Was NICHT in v1.0 (explizit dokumentiert, nicht implementiert)

- **DURABILITY=TRANSIENT/PERSISTENT** (dcps §2.2.3.4) — Persistence-Service, Phase 5+
- **MultiTopic** (dcps §2.2.2.3.4) — Object-Model-Profile (DLRL-deprecated)
- **DLRL** — komplett, ersetzt durch X-Types
- **CORBA-Profile** in IDL4-Bindings (idl4-cpp Annex A.1, idl4-csharp 7.7-7.12 + A.1, idl4-java Annex A.1)
- **Annex C C++98/03** in idl4-cpp
- **Annex D Classic-Compiler** in idl4-cpp
- **DDS-XML 1.0 XSD-1.1-Validator strict** (defensiver Reader reicht)
- **DDS-RPC PSM-Bindings** (Block J) — defer bis konkreter Kunde
- **DDS-Security §11 Plugin Language Bindings** (Rust-only-Implementierung)
- **XRCE Federated/P2P-Deployments** (§10.4/§10.5) — Stretch
- **Plain Language Bindings C/C++/Java** in xtypes §7.5.1 (ueber idl4-* abgedeckt; XTypes-Spec selbst out-of-scope)

---

## 15. Naechste Aktionen

1. **Phase 1 starten:** WP 1.A (XCDR2-Stack vollstaendig) — 1-2 Wochen, blockiert die meisten anderen Phase-1-Cluster.
2. **Parallel:** WP 1.C (HeaderExtension+Checksum) und WP 1.D (Builtin-Endpoint-Set) — keine Deps zu 1.A.
3. **Phase-7-Foundation (Cluster C7.A) als Hintergrund-Task** — wird von Phase 4 und Phase 6 gebraucht.
4. **Architektur-Spike Java-FFI** — vor Phase 5 (kann jetzt schon laufen, 1-2 Tage Aufwand).
5. **CI-Erweiterung:** Cyclone-Live-Discovery-Test pro Phase als Pflicht-Gate.
6. **Sales-Material:** Migrations-Hebel-Dokumente aus Phase 5 (C++/C#/Java-Drop-in-Argumente) und Phase 7 (XML-Profile-Loader) parallel zur Implementierung schreiben.

---

**Master-Roadmap-Eigentuemer:** Architektur (Track-Lead).
**Naechstes Update:** Nach Abschluss Phase 1 (Cluster-Aufwaende kalibrieren mit Realdaten).
