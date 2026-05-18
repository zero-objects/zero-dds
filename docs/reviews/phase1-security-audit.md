# Phase-1 Security-Audit — ZeroDDS

**Datum:** 2026-04-20
**Scope:** Hygiene-Audit nach Phase-1-Abschluss (WP 1.1–1.5). DDS-Security-Plugins
sind WP 2+ (siehe `memory/project_security_posture.md`) und aus diesem Audit
ausgeklammert.
**Methode:** Read-only-Review. Supply-Chain (cargo-audit, cargo-deny),
`#![forbid(unsafe_code)]`-Inventar, Untrusted-Input-Decode-Pfade, DoS-Caps,
Kryptographie-Hygiene, Timing-Attack-Oberflaeche.

## Ueberblick Findings

| # | Severity | Kategorie | Titel | Fundort |
|---|----------|-----------|-------|---------|
| 1 | Medium | Supply-Chain | `cargo-audit` im Dev-Toolkit nicht installiert | host env / CI |
| 2 | Medium | Supply-Chain | `cargo-deny` im Dev-Toolkit nicht installiert | host env / CI |
| 3 | Low | Supply-Chain | `windows-sys` Duplicate-Skip kann maskieren | `deny.toml:45` |
| 4 | Low | Supply-Chain | 55 External Deps (davon 15+ proc-macro-nahe), Dep-Baum moderat | `cargo tree` |
| 5 | High | DoS | `TypeLookupReply::decode_from` unbounded `Vec::with_capacity(n)` | `crates/types/src/type_lookup.rs:102` |
| 6 | Medium | DoS | `Vec<T>::decode` nutzt `len.min(remaining)` aber kein `safe_capacity` | `crates/cdr/src/composite.rs:113` |
| 7 | Low | DoS | `ParameterList::from_bytes` unbegrenzte Parameter-Anzahl (Byte-gekappt) | `crates/rtps/src/parameter_list.rs:146` |
| 8 | Info | DoS | `serialized_payload = body[pos..].to_vec()` kopiert Rest ungekappt | `submessages.rs:511, 1047` |
| 9 | Info | unsafe-Inventar | 0 `unsafe`-Bloecke im produktiven Code | alle 26 Crates |
| 10 | Info | unsafe-Inventar | `forbid/deny(unsafe_code)` konsistent auf allen 26 `lib.rs` | alle Crates |
| 11 | Info | Crypto | MD5 dokumentiert als Spec-konform (nicht Security) | `types/src/hash.rs:66`, `type_object/common.rs:33` |
| 12 | Info | Crypto | Keine self-rolled Crypto-Primitiven | Grep-bestaetigt |
| 13 | Info | Timing | Kein Secret/Token-Handling in Phase 1 → keine Timing-Oberflaeche | `crates/security/src/lib.rs` = Stub |
| 14 | Low | Process | `SECURITY.md` mit Platzhalter `security@example.invalid` | `SECURITY.md:12` |
| 15 | Low | Test-Coverage | Fuzzing nur RTPS (`datagram`, `fragment_assembler`, `submessage_decoders`); CDR + TypeObject ungefuzzt | `crates/rtps/fuzz/fuzz_targets/` |

## Details zu High/Medium Findings

### #5 High — TypeLookupReply unbounded `with_capacity`

```rust
// crates/types/src/type_lookup.rs:101-102
let n = r.read_u32()? as usize;
let mut types = Vec::with_capacity(n);   // <-- n kann u32::MAX sein
```
Ein boeswilliger Peer kann im `getTypes`-Reply `n = 0xFFFF_FFFF` ankuendigen.
Auf 64-bit-Zielen reserviert `Vec::with_capacity` anteilig
`n * sizeof(ReplyTypeObject)` → OOM. Fix: bestehenden Helper
`type_object::common::safe_capacity(n, 1, r.remaining())` nutzen
(wie bereits in `type_object/common.rs:147` und `type_identifier/mod.rs:512`).
Konsistent mit Review-Findings #9/#10 aus WP 1.5.

### #6 Medium — CDR-Generic `Vec<T>::decode`

```rust
// crates/cdr/src/composite.rs:103-113
if len > reader.remaining() { return Err(...); }
let mut out = Vec::with_capacity(len);
```
Der `len > remaining()`-Check kappt auf Buffer-Groesse, aber die
`DECODE_PREALLOC_CAP`-Obergrenze (`4096` in `types/common.rs:573`) fehlt.
Bei einem 64 kB UDP-Payload allokiert ein `Vec<()>` oder `Vec<u8>`-Decode
64 kB Capacity — unkritisch. Bei groesseren Zielbuffern (TCP-Transport,
`MAX_FRAME_SIZE = 16 MiB`) wird es relevant. Empfehlung:
`safe_capacity`-Helper nach `zerodds-cdr` duplizieren oder in
`zerodds-foundation` zentralisieren.

### #7 Low — ParameterList-Parametergrenze

`from_bytes` laeuft bis Sentinel oder EOF. Jeder Parameter hat `u16` Length
(max 64 kB Value). Gesamtlaenge ist durch Input-Buffer gekappt, aber die
Anzahl der Parameter ist unbeschraenkt. Realistische SPDP/SEDP-Records
haben < 20 Parameter; ein Peer kann aber 10k zero-length-Parameter senden
→ `Vec<Parameter>` waechst. Schlagen `MAX_PARAMETERS_PER_LIST = 256` als
Hard-Cap vor, analog `MAX_PARTITIONS`.

### #8 Info — serialized_payload Rest-Kopie

`body[pos..].to_vec()` kopiert den Rest des Submessage-Bodies. Laenge ist
durch Submessage-Header (`u16 octets_to_next`) gekappt → <= 64 kB pro
Submessage. Kein akuter Fix noetig, aber als "designed-in Cap"
dokumentieren in `submessages.rs`.

## Supply-Chain (Details)

- **cargo-audit / cargo-deny nicht lokal installiert.** Empfehlung:
  `cargo install cargo-audit cargo-deny` in die Dev-Setup-Doku +
  `scripts/pre-commit` aufnehmen. GitLab-CI fuehrt `cargo deny check`
  bereits aus (siehe `memory/project_gitlab_ci_setup.md`).
- **deny.toml skipt `windows-sys`** wegen Transitive-Duplicate
  (0.52 via rustix, 0.61 via clap-Stack). Wir bauen nicht fuer Windows
  in Phase 1 — skip ist ok, aber Re-Evaluation vor dem ersten
  Windows-Target-Build.
- **Dep-Count:** 55 unique External Crates (davon proc-macro-Familie:
  `syn`, `quote`, `proc-macro2`, `serde_derive`, `thiserror-impl`,
  `clap_derive`). Krypto nur `md-5` (spec-notwendig), `digest`,
  `block-buffer`, `generic-array`, `typenum`, `crypto-common` —
  alles RustCrypto-Ecosystem, kein eigenes.

## Timing-Attacks

In Phase 1 gibt es kein Secret/Token-Handling — `crates/security/src/lib.rs`
ist ein Stub. Grep auf `== secret|== token|subtle::` findet nichts.
Status: **nichts zu flaggen**. Ab WP 2 (DDS-Security AccessControl Plugin)
muss jeder Token-Compare `subtle::ConstantTimeEq` nutzen.

## Empfehlungen (priorisiert)

1. **#5 Fix (High):** `safe_capacity` in `TypeLookupReply::decode_from`.
2. **#1/#2:** `cargo-audit` + `cargo-deny` ins Pre-Commit haengen.
3. **#6 Fix (Medium):** `safe_capacity`-Helper nach `zerodds-foundation`, dann
   `Vec<T>::decode` und weitere `decode_from`-Pfade konsolidiert
   absichern.
4. **#7/#8:** Expliziter `MAX_PARAMETERS_PER_LIST`-Cap + Dokumentation
   der impliziten Submessage-Caps.
5. **#15:** CDR- und TypeObject-Fuzz-Targets ergaenzen (WP 2-Backlog).
6. **#14:** `SECURITY.md`-Platzhalter vor public-Release ersetzen.

## Appendix: Tool-Output

### cargo audit

```
$ cargo audit
error: no such command: `audit`
```
Nicht im Toolchain-Image installiert. In CI via GitLab-Runner-Image
abgedeckt.

### cargo deny check

```
$ cargo deny check
error: no such command: `deny`
```
Ebenfalls lokal fehlend. CI: gruen nach `windows-sys`-Skip-Eintrag.

### cargo tree (Summary)

```
$ cargo tree | wc -l
166
$ cargo tree --prefix=none | grep -v '^dds-' | sort -u | wc -l
55
```

Gesamte 166 Zeilen, 55 einzigartige External Crates. Moderater Footprint
fuer einen DDS-Stack.

**Wortanzahl (ohne Tabelle und Code-Bloecke):** ~540.
