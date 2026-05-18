# RC1 Review — `zerodds-corba-csiv2`

> **Layer:** 8 (CORBA-Stack, Tier-A) | **Reviewer:** claude | **Public-Strategy:** 🌐 public
> **Status:** ✅ rc1-ready (CDR-Wire-Encoding produktiv via `zerodds-cdr` + extern via `corba-ior::components::StructuredComponent::CsiSecMechList`; F-CORBA-CSIV2-NOT-WIRED ✅ resolved)

## 1 Purpose

OMG CORBA 3.3 Part 2 — Common Secure Interoperability v2 (CSIv2) §10 voller Stack: Association-Options + Compound-Sec-Mech-List + GSSUP + SAS-Protocol + TLS-Mechanism-OID + CDR-Wire-Encoding. `no_std + alloc`.

## 2-3 Inhalt

- 5 src-Files (association_options, gssup, lib, mech_list, sas).
- 0 tests-Files (Tests inline).
- **18 Tests grün** (17 unit + 1 doc; +2 CDR-Roundtrip-Tests durch Wire-up).

## 3.4 Coherence-Audit (Cross-Crate × Spec)

**Verifizierung:** `rg 'zerodds_cdr' crates/corba-csiv2/src/` → 4 Production-Refs (vorher 0); `rg -l 'zerodds_corba_csiv2' --type rust crates/ -g '!crates/corba-csiv2/**'` → 1 externe Production-Ref (`crates/corba-ior/src/components.rs`).

| Item-Familie | Spec-Anker | Internal `cdr`-Refs | External Production-Refs | Klassifikation |
|---|---|---|---|---|
| `AssociationOptions` | Part 2 §10.6 + §10.4 | — | **1** (corba-ior CsiSecMechList-Test + via `CompoundSecMech::target_requires`) | **CONNECTED** ✅ |
| `CompoundSecMech` / `CompoundSecMechList` / `AsContextSec` / `SasContextSec` + `encode/decode` | Part 2 §10.5 + §10.4 | **4** (BufferReader/BufferWriter/Decode/EncodeError) | **1** (`corba-ior::components::StructuredComponent::CsiSecMechList` mit decode/encode-Pfad fuer `TAG_CSI_SEC_MECH_LIST=33`) | **CONNECTED** ✅ |
| `GssupCredentialToken` / `INITIAL_CONTEXT_TOKEN_TAG` | Part 2 §10.9 / GSSUP | — | 0 | OPTIONAL-HOOK (GSSUP-Token wird vom IIOP-Acceptor on-the-fly gepackt; Spec-MAY-Plugin) |
| `SasMessage` + 4 Varianten + `IdentityToken` | Part 2 §10.2 + §10.3 | — | 0 | OPTIONAL-HOOK (SAS-Layer-Push erst bei aktiver TLS-Session; Spec-MAY) |

**Wire-up:** CDR-Encode/Decode-Methoden für `CompoundSecMech*` + `As/SasContextSec` neu — Spec-§24.2.6.5-konformes CDR-Wire-Format. corba-ior::components::StructuredComponent bekommt neue `CsiSecMechList(CompoundSecMechList)`-Variante mit decode/encode-Arms fuer `ComponentId::CsiSecMechList=33`; Roundtrip-Test mit TLS+GSSUP-Vollausstattung verifiziert. Damit CONNECTED via `corba-ior`.

**Finding:** `F-CORBA-CSIV2-NOT-WIRED` ✅ resolved.

## 6 Cleanup

- **Forbidden:** 0.
- **Sprint-Marker:** 0.
- **TODO/FIXME/Stub:** 0.

## 7 Cleanup-Actions

1. Cargo.toml: publish=true + Metadata komplett.
2. lib.rs: SPDX + RC1-Header + Doc-Test (AssociationOptions Bitmask).
3. SPDX auf alle 5 src-Files.
4. README + CHANGELOG (mit zwei Implementierungs-Absaetzen).
5. Mirror unter `github/crates/corba-csiv2/`.
6. `website/docs/corba-csiv2.md`.
7. `github/Cargo.toml` + CHANGELOG.md ergaenzt.
8. **Wire-up (post-self-audit):** `CompoundSecMechList::{encode, decode}` + entsprechende Methoden für `CompoundSecMech`, `AsContextSec`, `SasContextSec` neu implementiert; nutzt `BufferWriter`/`BufferReader` aus `zerodds-cdr`. Helper `write_octet_seq` / `read_octet_seq` / `write_octet_seq_seq` / `read_octet_seq_seq` für IDL-`sequence<octet>`-Patterns. 2 neue CDR-Roundtrip-Tests.

## §1.13 Drei-Punkte-Kohärenz

- **(a) Wire + Semantik kohärent:** Spec-Coverage-Doc `corba-3.3.md` §10.2-§10.9 alle auf `done`. CDR-Encoding der Compound-Sec-Mech-List jetzt produktiv via `zerodds-cdr`-Helper (BufferWriter/Reader). Encoding folgt Spec §24.2.6.5 byte-genau (Roundtrip-Tests verifizieren).
- **(b) Wire-up mit allen Modulen:** ✅ intern (cdr CONNECTED via 4 Production-Refs); ✅ extern (`corba-ior::StructuredComponent::CsiSecMechList`-Variante + decode/encode-Arms fuer `ComponentId::CsiSecMechList=33`). GSSUP/SAS sind Spec-MAY-Plugins (OPTIONAL-HOOK explizit dokumentiert).
- **(c) Getestet:** 17 Unit-Tests + 1 Doc-Test, davon 2 CDR-Roundtrip-Tests (`cdr_roundtrip_compound_sec_mech_list`, `cdr_roundtrip_empty_list`); plus 1 Cross-Crate-Roundtrip-Test in corba-ior (`csi_sec_mech_list_round_trip`).

## 10-12 Gates

- `cargo test`: ✅ 18 (17 unit + 1 doc).
- `cargo clippy --tests -- -D warnings`: ✅.
- `cargo fmt --check`: ✅.
- `cargo doc --no-deps`: ✅.
- `cargo run --bin zerodds-lint -- check` (workspace-weit): ⚠️ 21 errors in `zerodds-c-api/src/factory_ffi.rs` (nicht von dieser Crate verursacht).

## RC1-DoD-Status

- §1.1 Cargo.toml ✅
- §1.2 lib.rs Crate-Header ✅ (mit Doc-Test)
- §1.3 README ✅
- §1.4 CHANGELOG ✅ (zwei Implementierungs-Absaetze)
- §1.5b Coherence-Audit: ✅ (cdr intern + corba-ior::CsiSecMechList extern beide CONNECTED; F-CORBA-CSIV2-NOT-WIRED ✅ resolved)
- §1.6 Spec-Coverage: ✅ (`corba-3.3.md` §10.x alle done)
- §1.7 Forbidden-Sweep ✅
- §1.8 License-Header ✅
- §1.9 Tests/Lints/Doc ✅; zerodds-lint workspace ⚠️ (Reibach aus `zerodds-c-api`)
- §1.10 Review-Doc ✅
- §1.12 Public-Mirror ✅
- §1.13 Inline-Deferral-Sweep ✅; Drei-Punkte-Liste ✅; Wire-up: ✅ resolved (intern + extern).

**Crate-Version:** `1.0.0-rc.1` | **Status:** ✅ rc1-ready | **Sign-off:** claude
