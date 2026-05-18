# RC1 Review — `zerodds-corba-ior`

> **Layer:** 8 (CORBA-Stack, Tier-C) | **Reviewer:** claude | **Public-Strategy:** 🌐 public
> **Status:** ✅ rc1-ready

## 1 Purpose

OMG CORBA 3.3 Part 2 §13.6 — voller IOR-Stack: IOR-Struct, alle Standard-Profile-Tags inkl. IIOP-ProfileBody (via `corba-iiop`), alle 32 Standard-TaggedComponents inkl. strukturierter Decoder fuer ORB_TYPE / CODE_SETS / ALTERNATE_IIOP_ADDRESS / SSL_SEC_TRANS / TLS_SEC_TRANS / RMI_CUSTOM_MAX_STREAM_FORMAT / JAVA_CODEBASE / CSI_SEC_MECH_LIST (via `corba-csiv2`), stringified-IOR (`IOR:hex`) bidirektional, `corbaloc:`/`corbaname:`-URL-Parser.

## 2-3 Inhalt

- 9 src-Files (lib + component_tags, components, error, ior, profile_tags, stringified, tagged_profile, url).
- **44 Unit-Tests + 1 Doc-Test grün.**

## 3.4 Coherence-Audit (Cross-Crate × Spec)

**Verifizierung:** `rg 'zerodds_corba_ior' --type rust crates/ -g '!crates/corba-ior/**'` → Konsumenten in corba-cosnaming (Object-Refs in Bindings) + corba-dds-bridge.

| Item-Familie | Spec-Anker | External Production-Refs | Klassifikation |
|---|---|---|---|
| `Ior` / `TaggedProfile` / `ProfileId` | §13.6.2 + §13.6.7.1 | corba-cosnaming::context::ObjectRef + corba-dds-bridge | CONNECTED |
| `ComponentId` + 32 Standard-Tags | §13.6.7.3 | intern (StructuredComponent-Decoder) | OPTIONAL-HOOK |
| `StructuredComponent::*` (8 strukturierte Decoder) | §13.6.7.3 + Vendor-Specs | intern | OPTIONAL-HOOK |
| `StructuredComponent::CsiSecMechList` | §10.2.7 CSIv2 | corba-csiv2::CompoundSecMechList (F-CORBA-CSIV2-NOT-WIRED ✅ resolved) | CONNECTED |
| `TaggedProfile::InternetIop` mit `IiopProfileBody` | §15.7.2 | corba-iiop::IiopProfileBody | CONNECTED |
| `from_stringified` / `to_stringified` / `STRINGIFIED_IOR_PREFIX` | §13.6.10 | 0 (Caller-Layer-Tools) | OPTIONAL-HOOK |
| `CorbalocAddress` / `CorbanameAddress` / `parse_corbaloc` / `parse_corbaname` | §13.6.10 | 0 | OPTIONAL-HOOK |
| `IorError` / `IorResult` | §13.6 | intern | OPTIONAL-HOOK |

**Klassifikation:** Mehrheit der CORE-Items CONNECTED via corba-cosnaming + corba-dds-bridge + corba-iiop + corba-csiv2; Stringified-IOR und URL-Parser sind Spec-MAY Tooling-Surfaces (OPTIONAL-HOOK).

## 6 Cleanup

- **Forbidden:** 0.
- **Sprint-Marker:** 0.
- **TODO/FIXME/Stub:** 0.

## 7 Cleanup-Actions

1. Cargo.toml: publish=true + Metadata komplett.
2. lib.rs: SPDX bereits da + Doc-Test (`Ior::default()` + `ProfileId::InternetIop.as_u32()`).
3. SPDX auf alle 9 src-Files.
4. README + CHANGELOG.
5. Mirror unter `github/crates/corba-ior/`.
6. `website/docs/corba-ior.md`.
7. `github/Cargo.toml` + CHANGELOG.md ergaenzt.

## §1.13 Drei-Punkte-Kohärenz

- **(a) Wire + Semantik kohärent:** IOR-CDR-Encapsulation respektiert Endian-Marker; alle 32 TaggedComponents typed-decodierbar; CsiSecMechList-Roundtrip via corba-csiv2.
- **(b) Wire-up:** CONNECTED via corba-cosnaming + corba-iiop + corba-csiv2 + corba-dds-bridge.
- **(c) Getestet:** 44 Unit-Tests (IOR-Roundtrip + alle 32 TaggedComponents + Stringified-IOR-Codec + corbaloc/corbaname-URL-Parser + IIOP-ProfileBody-Inhalt + CsiSecMechList-Roundtrip) + 1 Doc-Test.

## 10-12 Gates

- `cargo test -p zerodds-corba-ior`: ✅ 44 unit + 1 doc.
- `cargo clippy -p zerodds-corba-ior --tests -- -D warnings`: ✅.

## RC1-DoD-Status

- §1.1 Cargo.toml ✅
- §1.2 lib.rs Crate-Header mit Doc-Test ✅
- §1.3 README ✅
- §1.4 CHANGELOG ✅
- §1.5b Coherence-Audit: ✅ (3 CONNECTED + 5 OPTIONAL-HOOK)
- §1.6 Spec-Coverage: ✅ (CORBA 3.3 P2 §13.6 + §15.7.2 + §10)
- §1.7 Forbidden-Sweep ✅
- §1.8 License-Header ✅ (alle 9 Files SPDX)
- §1.9 Tests/Lints/Doc ✅
- §1.10 Review-Doc ✅
- §1.12 Public-Mirror ✅
- §1.13 Inline-Deferral-Sweep ✅; Drei-Punkte-Liste ✅; Wire-up: CONNECTED.

**Crate-Version:** `1.0.0-rc.1` | **Status:** ✅ rc1-ready | **Sign-off:** claude
