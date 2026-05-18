# `zerodds-java-omgdds` v1.0 — Open Items

Stand 2026-05-07.

— **keine offenen Items.**

Total 12 Items: 11 done + 0 partial + 0 open + 1 n/a (Stretch).

## n/a (Stretch) — Decision-Record

### §4.2 gRPC-Bridge fuer Multi-Process

**Decision:** `n/a (Phase-2 Stretch)`.

**Begruendung:** Vendor-Spec `zerodds-java-omgdds-1.0.md` §5 markiert
das explizit als v1.1-Erweiterung. RC1 (v1.0) deckt In-Process-Bus-
Variante voll ab; Multi-Process via gRPC ist ein Add-On wenn der
Use-Case auftaucht.

**Implementierungs-Pfad (wenn aktiviert):**
1. `crates/grpc-bridge/proto/dds-bridge.proto` — DCPS-Service-Schema.
2. `crates/grpc-bridge/src/dds_service.rs` — Server-Side wraps
   `zerodds-dcps`.
3. `crates/java-omgdds/java/src/main/java/org/zerodds/internal/grpc/
   GrpcDdsBridge.java` — Java-Client.
4. System-Property-Switch: `org.zerodds.bridge=inprocess|grpc://host:port`.

**Aufwand:** ~800-1200 LOC + Cross-JVM-Test-Suite.

**Spec-Konformitaet:** OMG DDS-Java-PSM 1.0 macht keine Aussage zur
Wire-Implementation; ZeroDDS' Pure-Java-Pfad (in-process via
`InProcessBus` heute, gRPC-Bridge Phase-2) ist spec-konform.
Decision steht in Vendor-Spec.
