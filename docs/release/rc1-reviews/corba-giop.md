# RC1 Review — `zerodds-corba-giop`

> **Layer:** 8 (CORBA-Stack, Tier-A) | **Reviewer:** claude | **Public-Strategy:** 🌐 public
> **Status:** ✅ rc1-ready

## 1 Purpose

OMG CORBA 3.3 Part 2 §15 General Inter-ORB Protocol (GIOP) Wire-Codec — voller Stack mit allen 8 Message-Types fuer GIOP 1.0, 1.1 und 1.2 inkl. Bidirectional-GIOP. `no_std + alloc`, CDR-1 via `zerodds-cdr`.

## 2-3 Inhalt

- 17 src-Files (lib + 16 Module: cancel_request, close_connection, codec, error, flags, fragment, header, locate_reply, locate_request, message_error, message_type, reply, request, service_context, target_address, version).
- 0 tests-Files (Tests inline).
- **70 Tests grün** (69 unit + 1 doc).

## 3.4 Coherence-Audit (Cross-Crate × Spec)

**Verifizierung:** `rg 'zerodds_corba_giop' --type rust crates/ -g '!crates/corba-giop/**'` → 0 externe Konsumenten heute (corba-iiop / corba-dds-bridge sind Tier-B/C-pending).

| Item-Familie | Spec-Anker | External Production-Refs | Klassifikation |
|---|---|---|---|
| `MessageHeader` / `Version` / `Flags` / `MessageType` / `MAGIC` | §15.4.1 | 0 (Tier-B-Konsumenten pending) | OPTIONAL-HOOK (Wire-Codec-Surface fuer corba-iiop-Acceptor) |
| 8 Message-Types (`Request`/`Reply`/`CancelRequest`/`LocateRequest`/`LocateReply`/`CloseConnection`/`MessageError`/`Fragment`) | §15.4.2-§15.4.9 | 0 | OPTIONAL-HOOK |
| `ReplyStatusType` (6 Statuses) / `LocateStatusType` | §15.4.3 / §15.4.6 | 0 | OPTIONAL-HOOK |
| `ServiceContext` / `ServiceContextList` / `ServiceContextTag` | §15.5 | 0 | OPTIONAL-HOOK (Spec-MAY Plugin-Hook fuer Service-Context-Implementierungen) |
| `TargetAddress` / `ObjectKey` (GIOP 1.2) | §15.4 | 0 | OPTIONAL-HOOK |
| `Message`-Enum + `encode_message` / `decode_message` | §15.4 Top-Level-Codec | 0 | OPTIONAL-HOOK (Wire-Roundtrip-Surface) |
| `GiopError` / `GiopResult` | §15 | 0 | OPTIONAL-HOOK |

**Klassifikation:** corba-giop ist eine reine Wire-Codec-Bibliothek — Spec-MUST-Surface fuer hosting-Anwendungen oder Tier-B-Konsumenten. Externe Production-Refs werden bei der corba-iiop-RC1-Review (Task #23) hinzukommen, wenn der Acceptor das `decode_message`/`encode_message` einhaengt. Aktuell als OPTIONAL-HOOK klassifiziert (Wire-Codec-Plugin-API), nicht DEAD-as-whole-crate, weil die Crate Spec-mandatorische Wire-Format-Implementation ist.

## 6 Cleanup

- **Forbidden:** 0.
- **Sprint-Marker:** 0.
- **TODO/FIXME/Stub:** 0.

## 7 Cleanup-Actions

1. Cargo.toml: publish=true + Metadata komplett.
2. lib.rs: SPDX + Doc-Test (MAGIC_BYTES + Version).
3. SPDX auf alle 17 src-Files.
4. README + CHANGELOG.
5. Mirror unter `github/crates/corba-giop/`.
6. `website/docs/corba-giop.md`.
7. `github/Cargo.toml` + CHANGELOG.md ergaenzt.

## §1.13 Drei-Punkte-Kohärenz

- **(a) Wire + Semantik kohärent:** GIOP 1.0/1.1/1.2-Versions-Quirks korrekt abgebildet (Spec §15.4.1-§15.4.9). 1.0 mit `requesting_principal`, 1.1 mit Fragment-Flag, 1.2 mit umorganisiertem Header + TargetAddress + 8-Byte-Body-Alignment + Fragment fuer alle Types + BiDir-GIOP via ServiceContext.
- **(b) Wire-up mit allen Modulen:** OPTIONAL-HOOK extern (Tier-B-Konsumenten kommen). Intern voll integriert (`encode_message`/`decode_message` + 8 Message-Types + ServiceContext + TargetAddress alle untereinander gewired).
- **(c) Getestet:** 69 Unit-Tests (per-Message Roundtrips + Header-Codec + Version-Quirks + Fragment-Reassembly + ServiceContext-Tags) + 1 Doc-Test.

## 10-12 Gates

- `cargo test`: ✅ 70 (69 unit + 1 doc).
- `cargo clippy --tests -- -D warnings`: ✅.
- `cargo fmt --check`: ✅.
- `cargo doc --no-deps`: ✅.

## RC1-DoD-Status

- §1.1 Cargo.toml ✅
- §1.2 lib.rs Crate-Header ✅ (mit Doc-Test)
- §1.3 README ✅
- §1.4 CHANGELOG ✅
- §1.5b Coherence-Audit: ✅ (Wire-Codec OPTIONAL-HOOK fuer corba-iiop-Acceptor)
- §1.6 Spec-Coverage: ✅ (`corba-3.3.md` Part 2 §15 + §15.4.x + §15.5 referenziert)
- §1.7 Forbidden-Sweep ✅
- §1.8 License-Header ✅ (alle 17 Files SPDX)
- §1.9 Tests/Lints/Doc ✅
- §1.10 Review-Doc ✅
- §1.12 Public-Mirror ✅ (github/crates + website/docs)
- §1.13 Inline-Deferral-Sweep ✅; Drei-Punkte-Liste ✅; Wire-up-Status: OPTIONAL-HOOK fuer Tier-B-Konsumenten.

**Crate-Version:** `1.0.0-rc.1` | **Status:** ✅ rc1-ready | **Sign-off:** claude
