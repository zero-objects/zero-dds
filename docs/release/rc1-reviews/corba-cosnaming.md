# RC1 Review — `zerodds-corba-cosnaming`

> **Layer:** 8 (CORBA-Stack, Tier-C) | **Reviewer:** claude | **Public-Strategy:** 🌐 public
> **Status:** ✅ rc1-ready

## 1 Purpose

OMG CosNaming 1.3 (`formal/2004-10-03`) — voller Naming-Service-Stack: NamingContext + NamingContextExt In-Memory-Implementation mit allen 5 Spec-Exception-Klassen, Stringified-Name (§2.4) und corbaname-URL-Scheme (§2.5).

## 2-3 Inhalt

- 5 src-Files (lib + context, error, name, stringified).
- **25 Unit-Tests + 1 Doc-Test grün.**

## 3.4 Coherence-Audit (Cross-Crate × Spec)

**Verifizierung:** `rg 'zerodds_corba_cosnaming' --type rust crates/ -g '!crates/corba-cosnaming/**'` → 0 externe Konsumenten heute (Hosting-Anwendungen referenzieren via Naming-Server-Bootstrap).

| Item-Familie | Spec-Anker | External Production-Refs | Klassifikation |
|---|---|---|---|
| `Name` / `NameComponent` | CosNaming 1.3 §2.2 | 0 (Caller-Layer Naming-Server) | OPTIONAL-HOOK |
| `NamingContext` / `Binding` / `BindingType` | §2.2 + §2.3 | 0 | OPTIONAL-HOOK |
| `ObjectRef` mit IOR-Inhalt | §2.2 + CORBA P2 §13.6 | corba-ior::Ior (Inhalt) | CONNECTED (intern) |
| `NamingError` / `NotFoundReason` (5 Exception-Klassen) | §2.2 + §2.3 (Exception-IDLs) | intern | OPTIONAL-HOOK |
| `name_to_string` / `string_to_name` | §2.4 Stringified-Name | 0 | OPTIONAL-HOOK |

**Klassifikation:** Naming-Service ist Spec-MUST-Service-Implementation fuer hosting-Anwendungen — externe Production-Refs entstehen ueber Naming-Server-Bootstrap. ObjectRef-IOR-Inhalt CONNECTED zu corba-ior.

## 6 Cleanup

- **Forbidden:** 0.
- **Sprint-Marker:** 0.
- **TODO/FIXME/Stub:** 0.

## 7 Cleanup-Actions

1. Cargo.toml: publish=true + Metadata komplett.
2. lib.rs: SPDX bereits da + Doc-Test (`NameComponent`-Konstruktion).
3. SPDX auf alle 5 src-Files.
4. README + CHANGELOG.
5. Mirror unter `github/crates/corba-cosnaming/`.
6. `website/docs/corba-cosnaming.md`.
7. `github/Cargo.toml` + CHANGELOG.md ergaenzt.

## §1.13 Drei-Punkte-Kohärenz

- **(a) Wire + Semantik kohärent:** NamingContext-Operationen in §2.2-Spec abgebildet (Bind/Rebind/Resolve/Unbind/BindContext/NewContext/Destroy/ListBindings). Stringified-Name escaped korrekt `/`, `.` und `\` gemaess §2.4.
- **(b) Wire-up:** OPTIONAL-HOOK extern (Naming-Server-Bootstrap); CONNECTED intern via ObjectRef-IOR-Inhalt.
- **(c) Getestet:** 25 Unit-Tests (Bind/Resolve/Unbind-Roundtrips + alle 5 Exceptions + Stringified-Name-Codec + Bind-Context-Hierarchie) + 1 Doc-Test.

## 10-12 Gates

- `cargo test -p zerodds-corba-cosnaming`: ✅ 25 unit + 1 doc.
- `cargo clippy -p zerodds-corba-cosnaming --tests -- -D warnings`: ✅.

## RC1-DoD-Status

- §1.1 Cargo.toml ✅
- §1.2 lib.rs Crate-Header mit Doc-Test ✅
- §1.3 README ✅
- §1.4 CHANGELOG ✅
- §1.5b Coherence-Audit: ✅ (1 CONNECTED-intern + 4 OPTIONAL-HOOK)
- §1.6 Spec-Coverage: ✅ (CosNaming 1.3 §2.2 + §2.3 + §2.4 + §2.5)
- §1.7 Forbidden-Sweep ✅
- §1.8 License-Header ✅ (alle 5 Files SPDX)
- §1.9 Tests/Lints/Doc ✅
- §1.10 Review-Doc ✅
- §1.12 Public-Mirror ✅
- §1.13 Inline-Deferral-Sweep ✅; Drei-Punkte-Liste ✅; Wire-up: OPTIONAL-HOOK + CONNECTED intern.

**Crate-Version:** `1.0.0-rc.1` | **Status:** ✅ rc1-ready | **Sign-off:** claude
