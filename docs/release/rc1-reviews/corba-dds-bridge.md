# RC1 Review — `zerodds-corba-dds-bridge`

> **Layer:** 8 (CORBA-Stack, Tier-C) | **Reviewer:** claude | **Public-Strategy:** 🌐 public
> **Status:** ✅ rc1-ready

## 1 Purpose

Bidirektionale CORBA-Object ↔ DDS-Topic-Bridge: GIOP-Request → DDS-Sample (Servant-Modus) und DDS-Sample → GIOP-Request (Forwarder-Modus). Many-to-Many `BridgeMapping` mit `BridgeServant` + `LifecycleSync` und Wire-Helpers zu `corba-giop` + `corba-ior`.

## 2-3 Inhalt

- 5 src-Files (lib + mapping, servant, sync, wire).
- **17 Unit-Tests + 1 Doc-Test grün** (15 vorher + 2 neue wire-Tests).

## 3.4 Coherence-Audit (Cross-Crate × Spec)

**Verifizierung:** `rg 'zerodds_corba_dds_bridge' --type rust crates/ -g '!crates/corba-dds-bridge/**'` → 0 externe Konsumenten heute (Hosting-Anwendungen instanziieren BridgeServant direkt).

| Item-Familie | Spec-Anker | External Production-Refs | Klassifikation |
|---|---|---|---|
| `BridgeMapping` / `BridgeRoute` / `Direction` / `OperationMapping` / `TopicQosRef` | DDS 1.4 §2.2 + CORBA P2 §15 | 0 (Caller-Layer) | OPTIONAL-HOOK |
| `BridgeServant` | CORBA P1 §11.3.3 (Servant-Trait via corba-poa) | corba-poa::Servant | CONNECTED (intern) |
| `LifecycleSync` / `LifecycleEvent` | DDS 1.4 §2.2.2.2.1 register/unregister_instance | 0 | OPTIONAL-HOOK |
| `wire::decode_giop_request_bytes` / `RequestSummary` | CORBA P2 §15.4.2 GIOP Request | corba-giop::decode_message + Message::Request | CONNECTED |
| `wire::object_key_from_ior` | CORBA P2 §13.6 + §15.7.2 IIOP-ProfileBody.object_key | corba-ior::TaggedProfile::as_iiop | CONNECTED |

**Klassifikation:** Wire-Helpers CONNECTED via produktive `use`-Statements zu corba-giop + corba-ior. Bridge-Mapping-Surface OPTIONAL-HOOK fuer Hosting-Anwendungen.

### F-WORKSPACE-DEAD-DEPS-AUDIT — Resolution

Im Pre-Audit wurden zwei DEAD-DEPs gefunden: `corba-giop` und
`corba-ior` ohne `use`-Statement im src/. Beide sind in dieser
RC1-Cleanup gewired:

- **`corba-giop`**: produktiv genutzt in
  `wire::decode_giop_request_bytes` via `decode_message` +
  `Message::Request`. Test
  `wire::tests::decode_giop_request_bytes_rejects_non_request_frame`.
- **`corba-ior`**: produktiv genutzt in `wire::object_key_from_ior`
  via `Ior::profiles` + `ProfileId::InternetIop` +
  `TaggedProfile::as_iiop`. Test
  `wire::tests::object_key_from_empty_ior_is_none`.

Damit sind die Items 1 und 2 von F-WORKSPACE-DEAD-DEPS-AUDIT
**resolved**. Das Finding bleibt fuer Items 3 (rmw-zerodds-shim) und
4 (java-omgdds) offen — beide sind in nicht-RC1-Crates.

## 6 Cleanup

- **Forbidden:** 0.
- **Sprint-Marker:** 0 (`Sprint-1 Drop-in-Migration`-Marker im Header
  bereinigt).
- **TODO/FIXME/Stub:** 0.

## 7 Cleanup-Actions

1. Cargo.toml: publish=true + Metadata komplett.
2. lib.rs: SPDX bereits da + Sprint-Marker entfernt + Doc-Test
   (`Direction`-Variants).
3. SPDX auf alle 5 src-Files (4 vorher + neuer wire.rs).
4. README + CHANGELOG.
5. Mirror unter `github/crates/corba-dds-bridge/`.
6. `website/docs/corba-dds-bridge.md`.
7. `github/Cargo.toml` + CHANGELOG.md ergaenzt.
8. **Wire-up der DEAD-DEPs zu corba-giop + corba-ior** via neuem
   `wire`-Modul; F-WORKSPACE-DEAD-DEPS-AUDIT Items 1+2 resolved.

## §1.13 Drei-Punkte-Kohärenz

- **(a) Wire + Semantik kohärent:** Bridge mappt GIOP-Request-Form (request_id/operation/body) auf DDS-Sample-Form gemaess Many-to-Many BridgeRoute-Schema; LifecycleSync respektiert DDS §2.2.2.2.1 register/unregister_instance.
- **(b) Wire-up:** CONNECTED via wire-Helpers zu corba-giop + corba-ior; BridgeServant intern CONNECTED zu corba-poa.
- **(c) Getestet:** 17 Unit-Tests (Bridge-Mapping-Roundtrips + Servant-Lifecycle + LifecycleSync-Drain + wire-Helpers fuer GIOP + IOR) + 1 Doc-Test.

## 10-12 Gates

- `cargo test -p zerodds-corba-dds-bridge`: ✅ 17 unit + 1 doc.
- `cargo clippy -p zerodds-corba-dds-bridge --tests -- -D warnings`: ✅.

## RC1-DoD-Status

- §1.1 Cargo.toml ✅
- §1.2 lib.rs Crate-Header mit Doc-Test ✅
- §1.3 README ✅
- §1.4 CHANGELOG ✅
- §1.5b Coherence-Audit: ✅ (3 CONNECTED + 2 OPTIONAL-HOOK)
- §1.6 Spec-Coverage: ✅ (CORBA 3.3 P1 §11 + P2 §15 + §13.6 + DDS 1.4 §2.2)
- §1.7 Forbidden-Sweep ✅
- §1.8 License-Header ✅ (alle 5 Files SPDX)
- §1.9 Tests/Lints/Doc ✅
- §1.10 Review-Doc ✅
- §1.12 Public-Mirror ✅
- §1.13 Inline-Deferral-Sweep ✅; Drei-Punkte-Liste ✅; Wire-up: CONNECTED via wire-Modul (DEAD-DEPS-Audit-Items 1+2 resolved).

## 13 Daemon-Wireup-Append

Folgende Items sind nach dem ersten Sign-off in den `daemon`-Feature-
Pfad eingebracht worden (kein Major-Bump, alles innerhalb 1.0.0-rc.1):

- `daemon_runtime.rs` + `qos_translation.rs` Module.
- SSLIOP TaggedComponent 0x06 via rustls 0.23 + GIOP-Service-Context-Auth
  (CSIv2 SAS-Token) + Topic-ACL via `zerodds-bridge-security` voll wired
  (Bridge-Spec §7.1/§7.2/§7.3 + CORBA §24).
- `notify.rs` (CosNotification-Fanout) + `locate.rs` (CORBA-Locate-Cache)
  + `csiv2_wire.rs` (CSIv2-Wire-Hooks) + `cross_vendor.rs` Module.
- Tests gruen: 17 unit + 1 doc.

**Crate-Version:** `1.0.0-rc.1` | **Status:** ✅ rc1-ready | **Sign-off:** claude
