# RC1 Review — `zerodds-cdr`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md` (DoD + Forbidden-Tokens + Public-Strategy).
> **Layer:** 1 (Primitives)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public
>
> Track-Materialisierung via git: `git log docs/release/rc1-reviews/cdr.md`.

---

## 1 Purpose

XCDR1/XCDR2 Encoder/Decoder + KeyHash + PL_CDR1 Member-Codec; implementiert OMG XTypes 1.3 §7.4 Wire-Format vollständig. Pure-Rust, `no_std`-tauglich (mit opt-in `alloc`-Feature für composite/fixed/key_hash/struct_enc/xcdr1), `forbid(unsafe_code)`.

## 2 Public-Strategy

- **Marker:** 🌐 public
- **Begründung:** Layer-1-Primitive direkt unter Foundation. Wird von 11 Crates gegen-deps-t (qos, types, rtps, discovery, dcps, idl-rust, corba-{ior,iiop,giop,csiv2,cosnaming,rust}, ts-wasm). End-User können CDR-Encoding für eigene Topic-Types und Wire-Tooling direkt verwenden.

## 3 Content-Inventur

### 3.1 Module

```
src/
├── lib.rs           # Crate-Entry, Public-API-Aggregator, doctest
├── buffer.rs        # BufferReader / BufferWriter (alignment-tracking)
├── composite.rs     # CdrEncode/Decode für str / String / Vec / [T;N] / Option
├── encode.rs        # CdrEncode + CdrDecode Traits + primitive Impls
├── endianness.rs    # Endianness-Enum {Little, Big}
├── error.rs         # EncodeError + DecodeError mit Offset-Kontext
├── fixed.rs         # IDL fixed<P,S> Decimal-Type (BCD-Encoding)
├── key_hash.rs      # XTypes 1.3 §7.6.8 KeyHash (CDR_BE + MD5-Fallback)
├── struct_enc.rs    # XCDR2 Extensibility (final/appendable/mutable)
└── xcdr1.rs         # PL_CDR1 Member-Codec (XTypes 1.3 §7.4.1.2)
```

### 3.2 Public-API-Surface

```rust
// Buffer-I/O
pub struct BufferReader<'a>;
pub struct BufferWriter;        // alloc only
pub enum Endianness { Little, Big }

// Trait-Familie
pub trait CdrEncode;
pub trait CdrDecode: Sized;

// Composite-Impls (alloc only): for str / String / Vec<T> / [T;N] / Option<T>

// Errors
pub enum EncodeError { BufferFull, ValueOutOfRange, MissingNonOptionalMember };
pub enum DecodeError { UnexpectedEof, InvalidString, LengthExceeded,
                       InvalidEnum, InvalidBoolean, InvalidEncapsulation };

// Struct-Extensibility (alloc only)
pub fn encode_final / decode_final;
pub fn encode_appendable / decode_appendable;
pub struct MutableStructEncoder<'a>;
pub fn encode_mutable_member / encode_mutable_member_lc;
pub struct MutableMember<'a>;
pub fn read_mutable_member / read_all_mutable_members;
pub enum LengthCode;

// XCDR1 / PL_CDR1 (alloc only)
pub const PID_LIST_END: u16 = 0x3F02;
pub const PID_EXTENDED: u16 = 0x3F01;
pub const PID_EXTENDED_THRESHOLD: u32 = 0x3F00;
pub struct PlCdr1Member;
pub fn encode_pl_cdr1_member / write_pl_cdr1_sentinel;
pub fn read_pl_cdr1_member / read_all_pl_cdr1_members;

// Fixed-Decimal (alloc only)
pub struct Fixed<const P: u32, const S: u32>;

// KeyHash (alloc only)
pub fn compute_key_hash(holder: &[u8], max_size: usize) -> [u8; 16];
pub struct PlainCdr2BeKeyHolder;
pub const KEY_HASH_LEN: usize = 16;
```

### 3.3 Tests

- `cargo test -p zerodds-cdr`: ✅ alle Tests grün — 170 unit + 1 compliance_xcdr2 + 7 fuzz_smoke + 7 integration_topic + 15 proptest_roundtrip + 1 doc-test = **201 Tests**.
- Bench-Suite `encode_decode_hotpaths` (criterion).
- libFuzzer-Targets in `fuzz/` für PL_CDR1 + composite + struct_enc.

### 3.4 Coherence-Audit (Public-API × Cross-Crate × Spec)

| Public-Item | Spec-Anker | External Production-Refs | Klassifikation | Decision |
|---|---|---|---|---|
| `BufferReader` / `BufferWriter` | XTypes 1.3 §7.4 (alle) | 6 + 6 (rtps, dcps, types, idl-rust, corba-*, etc.) | CONNECTED | — |
| `Endianness` | XTypes 1.3 §7.4.1.1 RepresentationIdentifier | 41 Files | CONNECTED | — |
| `CdrEncode` / `CdrDecode` | XTypes 1.3 §7.4 trait family | 5 + 4 (idl-rust codegen + dcps + types) | CONNECTED | — |
| `EncodeError` / `DecodeError` | (interner Error-Pfad) | 6 + 14 | CONNECTED | — |
| `composite`-Impls (str/Vec/[T;N]/Option) | XTypes 1.3 §7.4.4 | indirekt via CdrEncode/CdrDecode | CONNECTED | — |
| `struct_enc::{encode_final,decode_final}` | XTypes 1.3 §7.4.2.1 Plain CDR2 | indirekt via codegen-output | CONNECTED via codegen | — |
| `struct_enc::{encode_appendable,decode_appendable}` | XTypes 1.3 §7.4.2.2 Delimited CDR2 | idl-rust emittiert in den codegen-Output | CONNECTED via codegen | — |
| `struct_enc::MutableStructEncoder` + `read_mutable_member` | XTypes 1.3 §7.4.2.4 PL_CDR2 | idl-rust emittiert in den codegen-Output | CONNECTED via codegen | — |
| `struct_enc::{encode_mutable_member,encode_mutable_member_lc,read_all_mutable_members,MutableMember,LengthCode}` | XTypes 1.3 §7.4.2.4 EMHEADER + LC0–LC7 | 0 direkte Production-Refs (intern verwendet) | OPTIONAL-HOOK | doc-as-hook (granulare API für handcodierte XCDR2-Pfade) |
| `xcdr1::*` (gesamtes Modul, 7 pub-Items) | XTypes 1.3 §7.4.1.2 PL_CDR1 | 0 (Tests + Fuzz nutzen es; keine production-Crate) | SPEC-MANDATED-OPEN | doc-as-hook (siehe F-CDR-1) |
| `key_hash::compute_key_hash` + `PlainCdr2BeKeyHolder` + `KEY_HASH_LEN` + `keyhash_cdr2_be` | XTypes 1.3 §7.6.8 | dcps re-exportiert + nutzt | CONNECTED | — |
| `fixed::Fixed<P,S>` | IDL 4.2 §7.4.13 | indirekt via idl-rust codegen | CONNECTED via codegen | — |
| `padding_for(pos, alignment)` (CDR-Padding-Helper) | XTypes 1.3 §7.4.4 (Member-Alignment) | 0 ext direkt | VENDOR-EXTENSION (Public-Library-API für handcodierte XCDR-Pfade — analog `struct_enc`-Familie) | doc-as-hook |

### 3.4.1 Sweep-Verifikation (§1.5b Pass 2)

`/tmp/zerodds-audit/cdr.tsv` enthält 32 Public-Items. Alle in der Tabelle
oben durch Family-Rows abgedeckt. **0 DEAD.**

## 4 Wiring

### 4.1 Dependencies

```toml
[dependencies]
zerodds-foundation = { path = "../foundation", default-features = false, features = ["alloc"] }

[dev-dependencies]
proptest = { workspace = true }
criterion = { workspace = true }
```

### 4.2 Dependents

11 Production-Crates: `zerodds-qos`, `zerodds-types`, `zerodds-rtps`, `zerodds-discovery`, `zerodds-dcps`, `zerodds-idl-rust`, `dds-corba-{ior,iiop,giop,csiv2,cosnaming,rust}`, `zerodds-ts-wasm`.

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std`   | ✅ | std-Re-Exports, implies `alloc` |
| `alloc` | ✅ | aktiviert `composite`/`fixed`/`key_hash`/`struct_enc`/`xcdr1` |
| `safety`| ❌ | Reserved für Safety-Class-Hardening (Phase-2) |

## 5 Spec-Relevanz

- **Spec(s):** OMG XTypes 1.3 §7.4 (komplett), §7.6.8 KeyHash; OMG IDL 4.2 §7.4.13 fixed; DDSI-RTPS 2.5 §10 Wire-Encapsulation; RFC 1321 (MD5 via foundation).
- **Coverage-Doc:** `docs/spec-coverage/dds-xtypes-1.3.md` (78+ done / 0 partial / 0 open).
- **Abgedeckte §-Sektionen:** §7.4.1.1 (RepresentationIdentifier), §7.4.1.2 (PL_CDR1), §7.4.2 (Plain/Delimited/PL CDR2), §7.4.4 (Composite Types), §7.4.5 (Extensibility), §7.6.8 (KeyHash).

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

```bash
rg -g '!target/' -i \
  -e 'llvm@llvm' -e 'sandra-kessler' -e 'fishermen21' \
  -e '/Users/sandrakessler' -e 'PDE-Spec' -e 'zero_concept' \
  -e 'zero-principle' -e 'Ghost-Inject' -e 'R-09[7-9]' \
  -e 'R-10[0-4]' -e 'R-110' -e '\bseesaw\b' \
  crates/cdr
```

Treffer: **keine**.

### 6.2 Soft-Review-Treffer

```bash
rg -i -e 'TODO\b' -e 'FIXME\b' -e 'XXX\b' -e '\bhack\b' crates/cdr
```

Treffer: **keine**.

### 6.3 Tech-Debt + Dead Code

- **F-CDR-1**: `xcdr1`-Modul (gesamte 7 pub-Items) hat 0 Production-Refs — `zerodds-rtps::parameter_list` hat einen parallelen PL_CDR-Encoder/Decoder mit anderem Sentinel (`PID_SENTINEL=0x0001` für DDSI-RTPS §9.4.2.11) während `zerodds-cdr::xcdr1` den XTypes-1.3-Sentinel `PID_LIST_END=0x3F02` (§7.4.1.2.4) implementiert. Es ist also **keine** Code-Duplikation, sondern zwei spec-distinkte Wire-Formate. **Klassifikation:** SPEC-MANDATED-OPEN. **Decision:** behalten + dokumentiert als Public-API für XTypes-konforme `@mutable`-Struct-Serialisierung außerhalb des RTPS-Pfads (z.B. XML-mapped, GIOP-Service-Context-PL_CDR1, custom transports). Code ist via 6 Unit-Tests + 2 libFuzzer-Targets abgedeckt.

- **F-CDR-2**: `LengthCode`, `MutableMember`, `read_all_mutable_members`, `encode_mutable_member` (low-level), `encode_mutable_member_lc`, `encode_final`, `decode_final` haben jeweils 0 direkte Production-Refs außerhalb der Crate. Sie sind die granulare API-Schicht unter `MutableStructEncoder` (das verbunden ist via codegen). **Klassifikation:** OPTIONAL-HOOK. **Decision:** behalten + als handcoded-XCDR2-Hook dokumentiert. Alle Items sind Spec-§7.4.2.4-EMHEADER-LC0..LC7-Primitives.

### 6.4 Public-API-Leaks

- Keine `pub use crate::*;` glob re-exports gefunden.
- Keine ungewollt `pub` markierten Helper.

## 7 Cleanup-Actions

1. `Cargo.toml`: ergänzt um `homepage`, `documentation`, `readme`, `keywords`, `categories`; `description` ausgebaut; `publish = false` → `publish = true` (Public-Strategy 🌐).
2. SPDX-License-Header in alle 9 `src/*.rs`-Files eingefügt (war komplett fehlend).
3. `src/lib.rs` Crate-Header erweitert: Spec-Block, Schichten-Position, vollständige Public-API-Aufzählung, doctest-Beispiel (Compile-Korrektheit verifiziert).
4. `CHANGELOG.md` neu angelegt mit `[1.0.0-rc.1]`-Initial-Release-Eintrag (Spec-Referenzen + Public-API + Implementierung + Feature-Flags).
5. `cargo doc`-Warnings (unclosed HTML-Tags durch `<T>`-Generics in markdown) durch backtick-quoting behoben.

## 8 Spec-Doc-Updates

`docs/spec-coverage/dds-xtypes-1.3.md` ist bereits voll grün (78 done / 0 partial / 0 open), keine Änderung nötig. Cross-Lang-Audit (cpp/csharp/java/rust/ts) bereits sauber (siehe `corba-rust`-Review-Commit `e0a53d7`).

## 9 Doc-Artefacts

- [x] `Cargo.toml`-Metadata vollständig
- [x] `lib.rs`-Crate-Header mit Safety-Class + Spec-Ref + Layer + API-Aufzählung
- [x] `README.md` (existiert, 119 Zeilen mit Quickstart + Extensibility-Beispiel)
- [x] `CHANGELOG.md` mit `[1.0.0-rc.1]`-Entry
- [x] doc-tested Code-Example (1 doctest grün)

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-cdr               # ✅ 201 Tests grün (170 + 1 + 7 + 7 + 15 + 1)
cargo clippy -p zerodds-cdr --all-targets -- -D warnings   # ✅ clean
cargo fmt -p zerodds-cdr -- --check     # ✅ clean
cargo doc -p zerodds-cdr --no-deps      # ✅ clean (warnings behoben)
cargo build -p zerodds-cdr --no-default-features            # ✅ no_std builds
cargo build -p zerodds-cdr --no-default-features --features alloc  # ✅
cargo run --bin zerodds-lint -- check   # ✅ workspace-weit 0 errors / 0 warnings
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README.md
- [x] §1.4 CHANGELOG.md mit RC1-Entry
- [x] §1.5 Public-API-Audit (siehe §3.2)
- [x] §1.5b Coherence-Audit (Tabelle in §3.4 ausgefüllt; F-CDR-1 + F-CDR-2 sind dokumentiert + behalten als SPEC-MANDATED-OPEN bzw. OPTIONAL-HOOK)
- [x] §1.6 Spec-Coverage-Update (XTypes 1.3 bereits voll grün)
- [x] §1.7 Forbidden-Token-Sweep
- [x] §1.8 License-Header pro File
- [x] §1.9 Tests + Lints + Doc-Build grün
- [x] §1.10 Review-Doc ausgefüllt
- [x] §1.11 Tracker auf ✅
- [x] Findings-Tracker `RC1_FINDINGS.md` mit F-CDR-1 + F-CDR-2 aktualisiert

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1` (via `version.workspace = true`)
- **Reviewer-Sign-off:** Claude
- **Tracker-Eintrag aktualisiert:** ✅

---

## Addendum — XCDR2-Bindings-Conformance (post initial RC1)

Nach dem initialen RC1-Sign-off wurde die Crate um Bindings-Conformance-
Funktionen erweitert. Alle Aenderungen sind additive und brechen keine
existing Public-API.

### Neue Tests

- `tests/xcdr2_wire_vectors.rs` — 16 Tests gegen `zerodds-xcdr2-bindings-conformance-1.0` §6 V-1..V-12 byte-genau.
- `tests/xcdr2_cross_vendor_fixtures.rs` — 15 Tests (14 spec-derived Fixtures + V-2 echte Cyclone-DDS-0.10.2-Capture via tcpdump auf llvm-Testbed).
- `tests/serde_bridge.rs` — 3 Tests fuer das neue Optional-Feature.

### Neue Public-Items

- `pub mod serde_bridge` (Feature `serde-bridge` only):
  - `encode_via_serde<T: Serialize>` / `decode_via_serde<T: DeserializeOwned>`
  - `decoded_json_repr` Debug-Helper
- Spec-Anker: `zerodds-xcdr2-rust-1.0` §11.3.

### Neue Cargo.toml-Felder

- Feature `serde-bridge` (optional, default-OFF) — pulls in `serde` + `serde_json`.

### Public-Mirror-Sync

- `github/crates/cdr/` synchronisiert (Cargo.toml + src/serde_bridge.rs +
  3 neue tests/-Files).
- `github/crates/cdr-derive/` neu (Layer 1.6 Companion-Crate).
- `website/docs/cdr.md` Public-API-Liste um `serde_bridge` und Wire-
  Vector-Test-Counts erweitert.
- `website/spec-coverage/zerodds-xcdr2-rust-1.0.md` neu (Coverage-
  Public-Mirror).

### Test-Tally (post-Addendum)

- Cargo: 232 Tests (incl. 16 wire-vectors + 15 cross-vendor + 3 serde).
- Forbidden-Token-Sweep: 0 Treffer.
- Inline-Deferral-Marker: 0 Treffer.
- License-Header SPDX: 12/12 src-Files.
- Cargo-Pflichtfelder: 14/14.
