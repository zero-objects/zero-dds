# RC1 Review — `zerodds-corba-iiop`

> **Layer:** 8 (CORBA-Stack, Tier-B) | **Reviewer:** claude | **Public-Strategy:** 🌐 public
> **Status:** ✅ rc1-ready

## 1 Purpose

OMG CORBA 3.3 Part 2 §14 + §15.7 + §15.9 — voller IIOP-TCP-Transport-Stack: ProfileBody (alle 4 Versionen 1.0-1.3 inkl. TaggedComponents), Connection / Connector / Acceptor mit thread-safer Connection-Reuse-Pool und Reconnect-Logik, Bidirectional-GIOP-Aushandlung.

## 2-3 Inhalt

- 8 src-Files (lib + acceptor, bidir, connection, connector, error, framing, profile_body).
- **24 Unit-Tests + 1 Doc-Test grün.**

## 3.4 Coherence-Audit (Cross-Crate × Spec)

**Verifizierung:** `rg 'zerodds_corba_iiop' --type rust crates/ -g '!crates/corba-iiop/**'` → corba-ior pflegt IIOP-ProfileBody als Standard-TaggedProfile-Inhalt.

| Item-Familie | Spec-Anker | External Production-Refs | Klassifikation |
|---|---|---|---|
| `IiopProfileBody` / `IiopVersion` / `TaggedComponent` | CORBA 3.3 P2 §15.7.2 | corba-ior::TaggedProfile (IIOP-Inhalt) | CONNECTED |
| `Connection` | §15.7 (Frame-Reader) | 0 (Caller-Layer-Hosting) | OPTIONAL-HOOK |
| `Connector` / `ConnectorConfig` | §15.7 (Client) | 0 | OPTIONAL-HOOK |
| `Acceptor` / `AcceptorConfig` | §15.7 (Server) | 0 | OPTIONAL-HOOK |
| `framing::{read_giop_message, write_giop_message}` | §15.4 (GIOP-Frame) | 0 (Hosting kann direkt corba-giop nutzen) | OPTIONAL-HOOK |
| `BiDirIiopServiceContext` / `BiDirIiopListenPoint` / `IIOP_BI_DIR_TAG` | §15.9 (Bidirectional-GIOP) | 0 | OPTIONAL-HOOK |
| `IiopError` | §15.7 | intern | OPTIONAL-HOOK |

**Klassifikation:** ProfileBody ist CONNECTED via corba-ior; Wire-/Connection-Surface ist OPTIONAL-HOOK fuer Caller-Layer-ORB-Hosting (Spec §15.7 Service-Implementation).

## 6 Cleanup

- **Forbidden:** 0.
- **Sprint-Marker:** 0.
- **TODO/FIXME/Stub:** 0.

## 7 Cleanup-Actions

1. Cargo.toml: publish=true + Metadata komplett.
2. lib.rs: SPDX bereits da + Doc-Test (`IiopVersion::V1_2`).
3. SPDX auf alle 8 src-Files.
4. README + CHANGELOG.
5. Mirror unter `github/crates/corba-iiop/`.
6. `website/docs/corba-iiop.md`.
7. `github/Cargo.toml` + CHANGELOG.md ergaenzt.

## §1.13 Drei-Punkte-Kohärenz

- **(a) Wire + Semantik kohärent:** ProfileBody-Encoder/Decoder respektiert IIOP-Versions-Quirks (1.0 ohne Components, ab 1.1 mit Components-Sequenz). Connection liest Frame-genau via 12-Byte-GIOP-Header.
- **(b) Wire-up:** ProfileBody CONNECTED zu corba-ior; Wire-Surface OPTIONAL-HOOK fuer Hosting.
- **(c) Getestet:** 24 Unit-Tests (ProfileBody-Roundtrips fuer alle 4 Versionen + TaggedComponent-Codec + Connection-Frame-Reader + Connector-Pool + Acceptor-Loop + BiDir-Aushandlung) + 1 Doc-Test.

## 10-12 Gates

- `cargo test -p zerodds-corba-iiop`: ✅ 24 unit + 1 doc.
- `cargo clippy -p zerodds-corba-iiop --tests -- -D warnings`: ✅.

## RC1-DoD-Status

- §1.1 Cargo.toml ✅
- §1.2 lib.rs Crate-Header mit Doc-Test ✅
- §1.3 README ✅
- §1.4 CHANGELOG ✅
- §1.5b Coherence-Audit: ✅ (1 CONNECTED + 6 OPTIONAL-HOOK)
- §1.6 Spec-Coverage: ✅ (CORBA 3.3 P2 §14 + §15.7 + §15.9)
- §1.7 Forbidden-Sweep ✅
- §1.8 License-Header ✅ (alle 8 Files SPDX)
- §1.9 Tests/Lints/Doc ✅
- §1.10 Review-Doc ✅
- §1.12 Public-Mirror ✅
- §1.13 Inline-Deferral-Sweep ✅; Drei-Punkte-Liste ✅; Wire-up: CONNECTED via corba-ior + OPTIONAL-HOOK fuer Hosting.

**Crate-Version:** `1.0.0-rc.1` | **Status:** ✅ rc1-ready | **Sign-off:** claude
