# Cross-Vendor DDS-Security Interop Matrix — Reference

Source-of-truth reference for the **secured** cross-vendor interop work (analogous to
the perf-compare docs). Consumed by the documentation agent to produce the public
security-comparison page. Covers: the timing matrix, every ZeroDDS wire/protocol fix
per vendor, the full catalog of cross-vendor wire incompatibilities, and the external
vendor/OMG tickets & bugs that make certain interop combinations impossible.

- Scope: ZeroDDS ↔ {Eclipse Cyclone DDS, eProsima Fast DDS, OCI OpenDDS}, DDS-Security 1.2
- Roundtrip bench (`tests/perf/dds-roundtrip-bench`), request/echo, payload 64 B, p50 µs
- 13 governance profiles × vendor pairs × both directions; loopback on codepit (Debian 13)
- Vendor versions: Cyclone (5-vendor stack), Fast DDS, OpenDDS tag `DDS-3.34.0`
- ZeroDDS vendorId `0x01F0` (OMG registration pending), Cyclone `0x0110`, Fast DDS `0x010F`, OpenDDS `0x0103`

> **Reproduction (repo-relativ, ein Befehl).** Harness
> `tests/perf/dds-roundtrip-bench/security-matrix/run_deep_matrix.sh` — keine
> `/root/bench-security`-Hardcodes mehr; alle Pfade aus der Script-Location
> abgeleitet (`gen.sh` erzeugt den cert-Tree, `gen_profile.sh` baut XSD-konforme
> governance + per-vendor CMS-signierte permissions, `deep_matrix.sh` faehrt
> Profil × Vendor-Paar). `deep_matrix.sh` setzt `ZERODDS_SECURE_SPDP=1` jetzt
> automatisch **nur** fuer den fastdds-Peer (FastDDS gated Token-Reziprokation
> auf 0xff0101). **2026-06-10 live re-verifiziert (codepit, `data-enc`):
> ZeroDDS↔{Cyclone 45/50, Fast DDS 72/72, OpenDDS 92/108 µs} alle GRUEN beide
> Richtungen** — die roten Zellen sind ausschliesslich Fremd↔Fremd-Paare
> (cyclone↔fastdds/opendds-Vendor-Loecher, kein ZeroDDS-Bug).
> Asset gotchas that otherwise SEC_FAIL every vendor: CMS signer must be the **permissions-CA
> directly** (`-signer permissions_ca.pem`, not the authority EE → Cyclone `PKCS7_get0_signers:
> signer not found`); CMS MIME `smime -sign -text` for **all** vendors incl. OpenDDS; governance
> XSD 20170901 strict indented sequence. Fast DDS interop requires `ZERODDS_SECURE_SPDP=1`
> (default off); `cap_prof.sh` must set `ZERODDS_BENCH_SECURITY=1` for the fastdds/opendds case
> (a config bug once made all "fastdds secured" captures actually PLAIN fastdds).

---

## 1. Governance profiles (the 13)

| Profile | rtps | metadata | data | discovery | liveliness | join/read/write AC |
|---|---|---|---|---|---|---|
| common-subset | NONE | NONE | ENCRYPT | NONE | NONE | off |
| data-enc | NONE | NONE | ENCRYPT | NONE | NONE | off |
| data-sign | NONE | NONE | **SIGN** | NONE | NONE | off |
| meta-data-enc | NONE | ENCRYPT | ENCRYPT | NONE | NONE | off |
| meta-sign-data | NONE | SIGN | ENCRYPT | NONE | NONE | off |
| liv-data-enc | NONE | NONE | ENCRYPT | NONE | ENCRYPT | off |
| disc-meta-data | NONE | ENCRYPT | ENCRYPT | ENCRYPT | NONE | off |
| disc-data-enc | NONE | NONE | ENCRYPT | ENCRYPT | NONE | off |
| rtps-enc | ENCRYPT | NONE | NONE | NONE | NONE | off |
| rtps-sign-data | **SIGN** | NONE | ENCRYPT | NONE | NONE | off |
| all-enc | ENCRYPT | ENCRYPT | ENCRYPT | ENCRYPT | ENCRYPT | **on** |
| all-sign | SIGN | SIGN | **SIGN** | SIGN | SIGN | **on** |
| sros2-full | ENCRYPT | ENCRYPT | ENCRYPT | ENCRYPT | ENCRYPT | **on** |

---

## 2. Status & timing matrix (p50 µs, both directions where green)

`A→B` = A is ping (initiator), B is pong (replier). Numbers are representative p50 from
codepit loopback after all ZeroDDS fixes landed (`feat/secured-rtps-enc-interop`).

### ZeroDDS ↔ OpenDDS — **9/13 green (the ZeroDDS maximum)**
| Profile | opendds→zerodds | zerodds→opendds | Status |
|---|---|---|---|
| common-subset | 101 | 93 | ✅ |
| data-enc | 96 | 94 | ✅ |
| meta-data-enc | 123 | 97 | ✅ |
| meta-sign-data | 104 | 127 | ✅ |
| liv-data-enc | 86 | 84 | ✅ |
| disc-meta-data | 124 | 99 | ✅ |
| rtps-enc | 92 | 99 | ✅ (new) |
| rtps-sign-data | 93 | 117 | ✅ (new) |
| disc-data-enc | 86 | 80 | ✅ (new) |
| data-sign | — | — | ❌ OpenDDS limit (DDSSEC12-59) |
| all-sign | — | — | ❌ OpenDDS limit (DDSSEC12-59) |
| all-enc | — | — | ❌ OpenDDS self-rejects full-AC (its literal Table-63 read); ZeroDDS joins — see Cyclone/FastDDS all-enc |
| sros2-full | — | — | ❌ OpenDDS self-rejects full-AC (its literal Table-63 read); ZeroDDS joins — see Cyclone/FastDDS |

### ZeroDDS ↔ Cyclone — **13/13 green, regression-free throughout**
Representative p50: 41–62 µs both directions across all profiles (e.g. rtps-enc 41/51,
disc-data-enc 41/49, common-subset 44/55, rtps-sign-data 56/62). Cyclone ignores the
`0xff0101` secure-SPDP channel, so `ZERODDS_SECURE_SPDP=1` is a clean additive guard.

### ZeroDDS ↔ Fast DDS — **green** (secure-SPDP channel, now auto-enabled per-peer)
Representative p50: 72–123 µs (2026-06-10 re-verify `data-enc` 72/72; earlier sweep
common-subset 80/97, disc-data-enc 93/94, disc-meta-data 109/100, all-enc 109/123,
rtps-sign-data 80/117). Fast DDS gates token reciprocation on ZeroDDS' reliable
`0xff0101` secure-SPDP channel, so it needs `ZERODDS_SECURE_SPDP=1`.
> `deep_matrix.sh` setzt `ZERODDS_SECURE_SPDP=1` jetzt automatisch **nur** wenn der
> ZeroDDS-Prozess gegen einen fastdds-Peer laeuft (global waere es fuer
> rtps-enc/discovery=NONE spec-falsch). Damit ist Fast DDS↔ZeroDDS direkt in der
> generischen Matrix gruen, ohne separate Harness.

### Same-vendor diagonal (sanity baseline, common-subset, p50 µs)
| | self |
|---|---|
| ZeroDDS | 31–47 |
| Cyclone | 64 |
| Fast DDS | 111 |
| OpenDDS | 200–224 |

### Cyclone ↔ Fast DDS — **NO_MATCH under every profile** (external vendor hole, see §5)

---

## 3. ZeroDDS fixes per vendor (wire/protocol, spec-grounded)

All on `feat/secured-rtps-enc-interop`; each is spec-grounded and verified regression-free
against the other vendors.

### vs OpenDDS (8 fixes → 9/13)
| # | Fix | Why / spec |
|---|---|---|
| 1 | SEDP/WLP outbound SRTPS-wrap | §8.4.2.4 Table 27 `is_rtps_protected`; OpenDDS `check_encoded`/`separate_message()` drops plain SEDP under rtps_protection |
| 2 | Volatile-Reader ACKNACK/NACK_FRAG Kx submessage-protection | OpenDDS `check_encoded` 2nd gate (`is_submessage_protected`) drops clear ACKNACK from `ff0202c4` |
| 3 | Event-driven directed SPDP response to new peers | §8.5.3; closes a discovery-cadence gap |
| 4 | **SRTPS decode `octetsToNextHeader==0` = to-end-of-message** | RTPS §8.3.3.2.3; OpenDDS sets `otn=0` on the final `SRTPS_POSTFIX`; ZeroDDS read body-len 0 → "erwarte SRTPS_POSTFIX(16)" → dropped all of OpenDDS's SRTPS |
| 5 | per-submessage SEC decode `otn==0` | same defect in `parse_secure_submessages::read_submsg` (SEC_POSTFIX) |
| 6 | No ParticipantCryptoToken when `rtps_protection=NONE` | OpenDDS `Spdp.cpp:1966` `crypto_handle_==NIL` → rejects participant tokens; sends only endpoint tokens |
| 7 | `data_protection` as a true `max` FLOOR | `level = reader_lv.max(gov_data_level)`; OpenDDS marks encrypted payload with the **N-flag** (NonStandardPayload, DATA flags `0x15`); ZeroDDS emitted `0x05` plaintext → OpenDDS `decode_serialized_payload=0`, no echo |
| 8 | None-arm data_protection FLOOR (user-writer, no locator-resolved reader) | §9.5.3.3.1 payload layer is target-independent (local writer key) |
| (+) | **Grant-based `check_create_participant` (§8.4.2.9.3)** | full-AC governance (all-enc/sros2-full) **is** joinable with a matching permissions grant — Cyclone/FastDDS do exactly this; removes a ZeroDDS over-strictness bug that denied every fully-locked domain unconditionally. (OpenDDS reads Table 63 literally and self-rejects — its own stance, not binding.) |

Auth-layer (earlier, also vs OpenDDS): ONELINE subject names (`subject_oneline`, OpenDDS compares `dds.ca.sn` by string), **NUL-terminated** `c.dsign_algo`/`c.kagree_algo` (OpenDDS `DiffieHellman::factory` needs NUL — opposite of Fast DDS), cert trailing-NUL strip (`cid_to_der`), data_protection FLOOR when OpenDDS omits `PID_ENDPOINT_SECURITY_INFO` in SEDP.

### vs Cyclone (→ 13/13)
| Fix | Why / spec |
|---|---|
| `hash_c1` optional in handshake request/reply | §9.3.2.3.1 — `hash_c1` is OPTIONAL; Cyclone/Fast DDS omit it → responder must recompute from (c.id,c.perm,c.pdata,c.dsign_algo,c.kagree_algo); present→tamper-check |
| ECDH-P256 (`prime256v1`) as Kx default | §9.3.2.3.1; X25519 is a non-spec extension → opt-in |
| `IS_KEY_PROTECTED` at `data==ENCRYPT` (not metadata) | §10.4.1.2.6 (data=ENCRYPT→payload+key; SIGN→payload only) |
| GMAC SRTPS: omit SEC_BODY (cyclone convention) | §9.5.3.3.4.4; plaintext-as-AAD, INFO_SRC in protected body |
| INFO_DST routing fix; SEDP plain under `discovery=NONE` | mirror Cyclone's "whole discovery plane plaintext under rtps=ENCRYPT + discovery=NONE" |
| Volatile writer full-submessage protection (DATA+HEARTBEAT+GAP) | §8.4.2.4 — all writer submessages Kx-protected, else "clear submsg from protected src" |

### vs Fast DDS (→ 13/13, on its harness)
| Fix | Why / ticket |
|---|---|
| **no-NUL** algo strings in hash_c/handshake tokens | Fast DDS recomputes hash_c2 over algo strings **without** `\0` (#3803) — opposite of OpenDDS; ZeroDDS build path emits/ hashes no-NUL (validate uses raw wire bytes → vendor-agnostic, Cyclone stays green) |
| `parse_srtps_body` forward-walk to `0x34` | Fast DDS appends a vendor submessage (`0x80`) after `SRTPS_POSTFIX`; last-24-byte assumption failed → SRTPS never decoded. Hybrid: fast-path last-24 (cyclone/zerodds), fallback walk |
| reliable **secure-SPDP channel `0xff0101`** (config-gated `enable_secure_spdp`) | Fast DDS announces+runs a reliable `ENTITYID_SPDP_RELIABLE_BUILTIN_PARTICIPANT_SECURE_WRITER/READER` and gates token reciprocation on it; reader is reliable + sends preemptive ACKNACK → needs writer resend (DATA SN=1 + HEARTBEAT) |
| **SEC-protect secure-SPDP** under `discovery_protection!=NONE` | Fast DDS SEC-wraps its `0xff0101` SPDP DATA when discovery=ENCRYPT; ZeroDDS sent it plain → rejected. per-endpoint crypto + token-exchange for `0xff0101` |
| P256 DH (avoids #3802); reply property order `(1,2)` | #3802 raw-vs-ASN.1 DH encoding; Fast DDS parses reply properties by-name (order tolerant) |

---

## 4. Cross-vendor wire-incompatibility catalog

The structural wire divergences that required per-vendor handling. Each is a place where
"the spec is ambiguous or silent and vendors chose differently."

| Topic | Cyclone | Fast DDS | OpenDDS | ZeroDDS handling |
|---|---|---|---|---|
| `octetsToNextHeader=0` on last SEC/SRTPS submessage | explicit length | explicit length | **otn=0** (to-end) | accept both (RTPS §8.3.3.2.3) |
| SRTPS_POSTFIX trailing bytes | last-24 | **+vendor submsg `0x80`** | otn=0/std | forward-walk to `0x34` |
| Encrypted payload marker | N-flag | N-flag | **N-flag (0x15)** | set N-flag (was missing → OpenDDS) |
| `c.dsign/kagree_algo` NUL | no-NUL | **no-NUL (#3803)** | **NUL required** | per-vendor: NUL for OpenDDS, no-NUL else |
| `dds.ca.sn`/`dds.cert.sn` subject format | lenient | lenient | **XN_FLAG_ONELINE** (`CN = …`) | emit ONELINE (universal) |
| cert bytes | std | std | **trailing NUL appended** | strip trailing NUL for parsing |
| `hash_c1` in handshake | omitted | omitted | present | optional + recompute |
| keymat serialization | cyclone fmt | — | XCDR1 BIG_ENDIAN | cyclone-format keymat |
| GMAC SRTPS SEC_BODY | omitted | omitted | — | omit (cyclone convention) |
| secure-SPDP `0xff0101` channel | not used | **reliable, required** | not used | config-gated `enable_secure_spdp` |
| secure-SPDP under discovery=ENCRYPT | n/a | **SEC-wrapped** | n/a | SEC-wrap when discovery≠NONE |
| `PID_ENDPOINT_SECURITY_INFO` in SEDP | present | present | **omitted** | data_protection FLOOR when absent |

---

## 5. External tickets / bugs that make interop impossible (not ZeroDDS)

These are the hard limits — documented with vendor/OMG source. ZeroDDS cannot fix them.

| ID | Where | Effect | Status / evidence |
|---|---|---|---|
| **OMG DDSSEC12-59** | DDS-Security spec / OpenDDS | `data_protection_kind=SIGN` (auth-only payload) is **unparseable as specified** — no length field before the variable CryptoFooter. OpenDDS `decode_serialized_payload` returns `[-3.3] Auth-only payload transformation not supported (DDSSEC12-59)`; **OpenDDS↔OpenDDS-self also fails**. Blocks `data-sign`, `all-sign`. | **Closed / Deferred** — raised by OpenDDS's own maintainer (OCI, A. Mitz) 2018, never fixed. `CryptoBuiltInImpl.cpp ~L2310`. https://issues.omg.org/issues/DDSSEC12-59 ; OpenDDS docs list it "not implemented". ZeroDDS-self does it (39/46 µs). |
| **OpenDDS full-AC self-reject** | OpenDDS `check_create_participant` only | OpenDDS reads Table 63 literally: a fully access-controlled governance (`enable_join_access_control=TRUE` + every topic read+write AC=TRUE, no exception) returns FALSE → "No governance exists for this domain", reproducible OpenDDS-self. This is **OpenDDS-specific**, not a universal spec verdict. | **Not binding on conformant peers.** §8.4.2.9.3 `check_create_participant` consults the *permissions grant*, not just governance topology: full-AC **is** joinable by a participant whose grant matches the domain. ZeroDDS joins it (grant-based gate); **Cyclone and Fast DDS join it too** (see all-enc 109/123 ZeroDDS↔FastDDS, SROS2 full-lockdown). Only the ZeroDDS↔OpenDDS cell is blocked, by OpenDDS's own stance. `AccessControlBuiltInImpl.cpp ~L281-348`. |
| **Cyclone #1547** | Cyclone DDS ↔ Fast DDS | Cyclone pub → Fast DDS sub secured fails: "Failed to convert octet sequence to ASN1 integer" (DH raw vs ASN.1). Makes **Cyclone↔Fast DDS NO_MATCH under every profile**. | **Open / needs-triage** since 2023-01-23 (still open 2026). Not a ZeroDDS bug. https://github.com/eclipse-cyclonedds/cyclonedds (issue #1547) |
| Fast DDS **#3802** | Fast DDS | DH-encoding raw vs ASN.1 (related to #1547). | ZeroDDS works around by using P256. |
| Fast DDS **#3803** | Fast DDS | algo strings without NUL terminator (hash_c recomputation). | ZeroDDS fixed on its side (no-NUL, spec-correct). |
| Fast DDS **#3804** | Fast DDS | "optional" reply fields treated as mandatory. | ZeroDDS emits hash_c1/c2/dh1 fully → satisfied. |
| ZeroDDS vendorId `0x01F0` | OMG | not yet OMG-registered (application pending). | Cosmetic; does not block interop. |

---

## 6. ZeroDDS config options (interop guards)

| Option | Env | Default | Effect |
|---|---|---|---|
| `enable_secure_spdp` | `ZERODDS_SECURE_SPDP=1` | off | Run the Fast DDS-style reliable secure-SPDP channel (`0xff0101`) + SEC-wrap it under discovery_protection. **On = Fast DDS interop; off = Cyclone/spec-pure.** Cyclone/OpenDDS ignore the channel (additive). |
| (auto) per-vendor NUL on algos | — | per remote vendorId | NUL-terminate `c.*_algo` only for OpenDDS (`VendorId::OPENDDS`), no-NUL for Cyclone/Fast DDS. |
| (auto) data_protection FLOOR | — | governance | encrypt user payload to governance `data_protection_kind` even when the peer omits per-endpoint security info / isn't locator-resolved. |

---

## 7. Bottom line

- **ZeroDDS ↔ Cyclone: 13/13.  ↔ Fast DDS: 13/13** (with `ZERODDS_SECURE_SPDP=1`).  **↔ OpenDDS: 9/13** = the maximum OpenDDS allows.
- Every one of the 13 OpenDDS cells is **green, an OpenDDS-side limitation (DDSSEC12-59), or an OpenDDS-specific self-reject (full-AC) that ZeroDDS itself joins**.
- The only un-achievable combinations are externally blocked: **OpenDDS data=SIGN** (OMG DDSSEC12-59, deferred) and **fully-locked governance vs OpenDDS** (OpenDDS self-rejects full-AC — its own literal Table-63 read, not binding; ZeroDDS/Cyclone/Fast DDS all join full-AC with a valid grant per §8.4.2.9.3), plus the **Cyclone↔Fast DDS** vendor hole (Cyclone #1547).
- ZeroDDS interoperates across more of the matrix than any pair of the three reference vendors interoperate with each other.
- Canonical create-participant access-control semantics (grant-based, §8.4.2.9.3): [`create-participant-access-control.md`](create-participant-access-control.md).
