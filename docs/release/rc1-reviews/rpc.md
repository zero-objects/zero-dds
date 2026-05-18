# RC1 Review — `zerodds-rpc`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 4 (Core Services)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public (im Mirror).

---

## 1 Purpose

OMG DDS-RPC 1.0 (`formal/16-12-04`) Request/Reply-Framework auf dem ZeroDDS-DCPS-Stack: Foundation (Common-Types + Topic-Naming + Service-Mapping + Codegen) plus Runtime (Requester/Replier/QoS-Profile + Endpoint-Builder).

## 2 Public-Strategy

**Marker:** 🌐 public (Tracker 4.8 🌐).
**Begruendung:** OMG-Standard-Spec, voll abdeckt; End-User-RPC-Apps brauchen das Crate direkt.
**Cargo-Manifest-Anomalie:** `publish = false` im Dev-Repo, weil `zerodds-rpc` transitiv ueber `zerodds-dcps` am Embargo-Crate `zerodds-inspect-endpoint` haengt. Der Public-Mirror unter `github/crates/rpc/` strippt die Embargo-Pfad-Dep transitiv und setzt `publish = true`. Analog zum bereits etablierten `zerodds-dcps`-Pattern.

## 3 Content-Inventur

```
src/
├── lib.rs            # Crate-Entry + Re-Exports
├── annotations.rs    # IDL @service/@oneway/@in/@out/@inout-Lowering
├── codegen.rs        # Request/Reply-Struct-Pairs + CallUnion (Spec §7.5.1)
├── common_types.rs   # RequestHeader/ReplyHeader/SampleIdentity/RemoteExceptionCode (Spec §7.5)
├── discovery_ext.rs  # PublicationBuiltinTopicDataExt + Service-Match (Spec §7.8.4)
├── endpoint.rs       # RpcEndpointBuilder + Requester/Replier-Endpoints
├── error.rs          # RpcError + RpcResult
├── evolution_rules.rs # Compatibility-Mappings (Spec §7.6.5)
├── function_call.rs  # FunctionStub/FunctionSkeleton + dispatch_request (Spec §7.7)
├── qos_profile.rs    # RpcQos (Spec §7.11) + XML-Profile-Resolution
├── replier.rs        # Replier<TIn, TOut> + ReplierHandler + FnHandler (Spec §7.10)
├── request_identity.rs # RequestIdentity (Spec §7.8.2)
├── requester.rs      # Requester<TIn, TOut> + tick-driven API (Spec §7.9)
├── rpc_hash.rs       # rpc_member_hash (Spec §7.5.4)
├── service_mapping.rs # ServiceDef/MethodDef/ParamDef + lower_service (Spec §7.4)
├── topic_naming.rs   # request_topic_name/reply_topic_name (Spec §7.8.2)
└── wire_codec.rs     # Request/Reply-Frame-Encoder/Decoder
```

17 src-Files, 5839 LOC, **180 Tests** (171 lib + 5 + 4 integration).

### Public-API

Siehe `lib.rs` — vollstaendig kuratierte `pub use`-Re-Exports nach Spec-Sektionen gruppiert.

### 3.4 Coherence-Audit

| Public-Item-Familie | Spec-Anker | External Production-Refs | Klassifikation | Decision |
|---|---|---|---|---|
| `RequestHeader` / `ReplyHeader` / `SampleIdentity` / `RemoteExceptionCode` | DDS-RPC 1.0 §7.5 | Requester/Replier/Endpoint (intern); end-user-Servicedefinitionen | CONNECTED | — |
| `topic_naming::*` | §7.8.2 | Endpoint-Builder; end-user-Code | CONNECTED | — |
| `annotations::*` | §7.3 | `service_mapping::lower_service` | CONNECTED | — |
| `service_mapping::*` | §7.4 | `codegen::build_*_pair` | CONNECTED | — |
| `codegen::*` | §7.5.1 | end-user-Codegen-Konsumenten (idl-rust IDL→Rust-Pipeline + Cross-PSM) | CONNECTED | — |
| `rpc_hash::rpc_member_hash` | §7.5.4 | `codegen` (intern fuer Member-Hash-Stempel auf Request/Reply-Strukturen) | CONNECTED | — |
| `Requester<TIn, TOut>` / `Replier<TIn, TOut>` | §7.9 / §7.10 | `endpoint` + end-user-Apps | CONNECTED | — |
| `RpcQos::*` | §7.11 | `requester` + `replier` (intern); end-user-XML-Profile-Pfade | CONNECTED | — |
| `wire_codec::*` | §7.5 + Inline-Layout | `requester` + `replier` (intern) | CONNECTED | — |
| `discovery_ext::*` | §7.8.4 | end-user Discovery-Match-Code; `service_matches_client` test | CONNECTED | — |
| `function_call::*` | §7.7 | `dispatch_request` von `Replier`; end-user-Skeleton-Adaption | CONNECTED | — |
| `evolution_rules::*` | §7.6.5 | end-user-Type-Compatibility-Tools | OPTIONAL-HOOK (Spec MAY) | document-as-hook |
| `request_identity::RequestIdentity` | §7.8.2 | `Replier::current_request_identity` | CONNECTED | — |
| `endpoint::{RpcEndpointBuilder, RequesterEndpoint, ReplierEndpoint}` | §7.9 + §7.10 | end-user-Apps | CONNECTED | — |
| `RpcError` / `RpcResult` | crate-internal | alle pub-Funktionen | CONNECTED | — |

Ergebnis: **0 ❌-Klassen offen.**

## 4 Wiring

### 4.1 Dependencies

```toml
[dependencies]
zerodds-dcps   = { path = "../dcps" }
zerodds-idl    = { path = "../idl" }
zerodds-qos    = { path = "../qos" }
zerodds-rtps   = { path = "../rtps" }
zerodds-types  = { path = "../types" }
zerodds-xml    = { path = "../xml" }
```

### 4.2 Dependents

End-User-RPC-Apps, `crates/rmw-zerodds-shim` (ROS-2-Service-Pfad), idl-cpp/idl-csharp/idl-java (Cross-PSM-Codegen-Konsumenten der Service-Mapping-Datenstrukturen — Spec §7.4 Pfad).

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | Threading + `Mutex`/`mpsc` fuer Runtime-Module |
| `alloc` | ✅ via std | `Vec`/`String` (Foundation-Module) |
| `safety` | ❌ | Reserve-Hook fuer extra Defensive-Checks |

## 5 Spec-Relevanz

- **Spec:** OMG DDS-RPC 1.0 (`formal/16-12-04`). Coverage-Doc `docs/spec-coverage/dds-rpc-1.0.md`.

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

```bash
rg -i -e 'llvm@llvm' -e 'sandra-kessler' -e 'fishermen21' \
  -e '/Users/sandrakessler' -e 'PDE-Spec' -e 'zero-principle' \
  crates/rpc/
```

Treffer: **0**.

### 6.2 Sprint-/Phase-Marker

Pre-Cleanup:
- `lib.rs:1` `Foundation-Stufe (C6.1.A)` + ganze `# Scope C6.1.A/C6.1.B/C6.1.C/C6.1.D`-Sektion mit Phase-Sprache.
- `qos_profile.rs:25` `Phase-7-XML-Loader`.
- `requester.rs:154` `In WP 1.5+ wird das die echte RTPS-GUID des Request-Writers.`

Post-Cleanup: **0**. lib.rs neu in Guardrails §1.2-Form (Safety-Class + Spec-Ref + Layer + Public-API-Aufzaehlung). Phase-Sprache durch fachliche Beschreibung der Module ersetzt.

### 6.3 Datums-Marker

CHANGELOG-Eintrag traegt Keep-a-Changelog-Datum (per Guardrails §2.1c erlaubt).

### 6.4 Soft-Review

Keine TODO/FIXME/HACK in src/.

### 6.5 Public-API-Leaks

Keine — alle Re-Exports sind explizit kuratiert.

### 6.6 Tech-Debt + Dead-Code

Keine.

## 7 Cleanup-Actions

1. **F-RPC-1** (resolved): Sprint-/Phase-Marker (`C6.1.A/B/C/D`, `Phase-7`, `WP 1.5+`) aus lib.rs/qos_profile.rs/requester.rs entfernt; lib.rs in Guardrails §1.2-Form.
2. **SPDX-Header** in 17 src-Files (lib + 16 weitere).
3. **Cargo.toml-Metadata**: `description` praezisiert; `homepage`/`documentation`/`readme`/`keywords`/`categories` ergaenzt; `publish = false` mit Dev-Repo-Embargo-Begruendung dokumentiert (Public-Mirror flippt auf `true`).
4. **README.md** in RC1-Format (Spec-Mapping-Tabelle + Public-API + Quickstart + Feature-Flags + Stabilitaet).
5. **CHANGELOG.md** `[1.0.0-rc.1]` Initial-Materialisierung mit vollstaendiger Public-API-Auflistung.
6. **rustdoc-Links**: 2 unresolved-link-Warnings repariert (`current_request_identity`, `ReplyHeader::related_request_id` als prosa-Verweise statt rustdoc-Links — die Items leben in Schwester-Modulen ohne crate-rooted-Pfade).

## 8 Spec-Doc-Updates

`docs/spec-coverage/dds-rpc-1.0.md` Coverage-Status bleibt unveraendert (94 done / 0 partial / 0 open / 10 n/a).

## 9 Doc-Artefacts

- [x] Cargo.toml-Metadata
- [x] lib.rs-Crate-Header (Safety + Spec + Layer + Public-API + Beispiel)
- [x] README.md
- [x] CHANGELOG.md
- [x] doc-tested Code-Examples (`rust,ignore` im README)

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-rpc                           # ✅ 171 + 5 + 4 = 180 passed
cargo clippy -p zerodds-rpc --tests -- -D warnings  # ✅
cargo fmt -p zerodds-rpc -- --check                 # ✅
cargo doc -p zerodds-rpc --no-deps                  # ✅ (post-Fix: 0 Warnings)
cargo run --bin zerodds-lint -- check               # ✅ 105 crates / 1028 files
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README.md
- [x] §1.4 CHANGELOG.md
- [x] §1.5 Public-API-Audit
- [x] §1.5b Coherence-Audit (alle CONNECTED, 1 OPTIONAL-HOOK)
- [x] §1.6 Spec-Coverage-Update (kein Update noetig)
- [x] §1.7 Forbidden-Token-Sweep (0)
- [x] §1.8 License-Header (17 src-Files)
- [x] §1.9 Tests + Lints + Doc-Build gruen
- [x] §1.10 Review-Doc
- [x] §1.11 Tracker auf ✅
- [x] §1.12 Public-Mirror-Artifacts
- [x] §1.13 Spec-Conformance-Audit (F-RPC-1 ✅ resolved)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer-Sign-off:** Claude
- **Tracker-Eintrag aktualisiert:** ✅
