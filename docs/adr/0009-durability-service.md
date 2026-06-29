# 0009 — Durability-Service (TRANSIENT + PERSISTENT) als standalone, adapter-getriebener Daemon

- **Status:** accepted
- **Datum:** 2026-06-10
- **Autoren:** @sandra
- **Kontext:** `crates/durability-store`, `crates/durability-service`, `crates/durability-client`, DDS 1.4 §2.2.3.4/§2.2.3.5

## Kontext

Die DDS-1.4-Durability-Leiter hat vier Stufen: `VOLATILE`,
`TRANSIENT_LOCAL`, `TRANSIENT`, `PERSISTENT`. Die ersten beiden sind
writer-lokal und in ZeroDDS fertig. `TRANSIENT` und `PERSISTENT` sind die
optionalen „Service"-Stufen: Samples müssen den **Tod des erzeugenden
Writer-Prozesses** (TRANSIENT) bzw. einen **System-Neustart** (PERSISTENT)
überleben und an Late-Joiner geliefert werden, ohne dass der Original-Writer
noch lebt.

Heute existiert nur ein **writer-eingebettetes** Backend
(`crates/dcps/src/durability_service.rs`, In-Memory + File-per-Sample). Das
deckt Late-Joiner ab, **solange der Writer-Prozess lebt**, ist aber kein
echter Service: stirbt der Writer-Prozess, sind die TRANSIENT-Samples weg;
nach Reboot re-announced niemand die PERSISTENT-Files. Die
Vendor-Feature-Matrix weist „Transient/Persistent (Service)" entsprechend als
Defizit aus (Cross-Vendor-Wire-Replay fehlt ganz).

Zwei vorhandene Planungs-Docs widersprachen sich:
`internal/plans/milestone-v1.2.md` WP 2.2c wollte einen frischen, sqlite-WAL-
gestützten Daemon; `internal/roadmap/track-dds-persistence-service.md` (RC3-C)
wollte einen nicht existierenden „RC2-A-Datalake-Engine"-Stack spec-konform
wrappen. Diese ADR löst den Widerspruch.

Aus der Design-Session ergaben sich klare Anforderungen: volle Storage-
Schnittstelle; Warehouse/Lake-Adapter bringen ihre **eigenen** Analytics-
Schnittstellen mit (kein generischer Query-Layer in unserem Code);
Retention ist **ausschließlich** der QoS-Contract; das einzige Service-seitige
Stellrad ist die **Hot-Cache-Größe im Memory**, Disk ist die verlustfreie
Umkehrung.

## Entscheidung

Ein **standalone Durability-Daemon pro Domain** (`bin/zerodds-durability-svc`)
über einer **adapter-getriebenen Storage-Abstraktion**. Drei-Schichten-Modell:

```
① Daemon (durability-service): DCPSDurability-Discovery, RTPS-Ingest/Replay, Startup-Sync
②a Hot-Tier  (TieredStore): Memory-Working-Set, LRU, write-/read-through  ← SERVICE-CONFIG: memory_budget
②b Storage-Abstraktion (DurabilityStore-Trait, VOLL): store/query/replay/unregister/cleanup/stats
   │                                                                       ← BASIS: Topic-QoS (Contract)
③ Cold-Adapter: sqlite-WAL · file · lakehouse(Parquet+DuckDB)
       └─ jeder zusätzlich mit EIGENER nativer Read-/Query-Schnittstelle (Analytics, out-of-band)
```

**Invarianten (normativ):**

1. **QoS ist der Contract.** `DurabilityServiceQosPolicy` + `Lifespan`
   (history_depth, max_samples, max_instances, max_samples_per_instance,
   service_cleanup_delay) bestimmen, **wie viel total vorgehalten und an jeden
   Late-Joiner geliefert werden muss**. Nur dieser Contract begrenzt Retention.
   Der Cold-Store erzwingt ihn (drop-oldest bei Cap-Bruch + `RESOURCE_LIMITS`-
   Event, nie still).

2. **Das Memory-Budget ist KEIN Retention-Knopf.** Es bestimmt nur, wie viel
   des contracted History hot im RAM liegt. Der Rest liegt auf Disk und wird
   beim Late-Join **immer** verlustfrei nachgeströmt — Service-Config darf das
   nicht kappen, sonst bräche es den Contract.

3. **Der Cold-Adapter ist autoritativ für den vollen contracted History**
   (write-through). Der Hot-Tier ist ein bounded read-/write-through Cache.
   `replay`/`query` **streamen** lazy/paginiert aus dem Cold-Store (`SampleStream`,
   nie „alles in RAM").

4. **TRANSIENT_LOCAL bleibt writer-lokal** (kein Service nötig);
   **TRANSIENT/PERSISTENT** wandern in den Service.

5. **Cross-Vendor fällt aus dem Protokoll heraus**: der Service ist aus
   RTPS-Sicht ein normaler Participant — er ingestet als
   `RELIABLE/TRANSIENT_LOCAL/KEEP_ALL`-Reader und replay't als
   `TRANSIENT_LOCAL(KEEP_ALL)`-Writer. Ein Cyclone/FastDDS-`TRANSIENT`-Writer
   wird so automatisch mitgeschrieben und an beliebige Reader repliziert.

**Adapter-Grenze:** Das Trait deckt nur den DDS-Durability-Pfad ab
(store/replay/historical-query). Analytics/Read-Zugriff macht man über die
**native Schnittstelle des Adapters** (SQL auf sqlite, DuckDB/Parquet beim
Lakehouse) — out-of-band, nicht über das Trait.

**Crate-Schnitt:**

- `crates/durability-store` — Trait + Modell (`DurabilitySample`, `Selector`,
  `SampleStream`) + `TieredStore` + Retention-Engine (adapter-agnostisch).
- `crates/durability-store-sqlite` — sqlite-WAL-Adapter (Default, „einfache DB").
- `crates/durability-store-file` — File-per-Sample-Adapter (aus `dcps` extrahiert, dep-frei).
- `crates/durability-store-lakehouse` — Parquet + DuckDB; House-Modus
  (strukturierte SQL-Tabellen/Views) **und** Lake-Modus (roh-Parquet,
  schema-on-read) in einem.
- `crates/durability-client` — Participant-seitiger Client, meldet lokale
  TRANSIENT/PERSISTENT-Topics am Service an.
- `crates/durability-service` — Daemon-Logik.
- `bin/zerodds-durability-svc` — Binary (systemd/Docker), **ein Daemon pro Domain**.

## Alternativen

1. **Writer-embedded Backend behalten/ausbauen** (Status quo) — verworfen: kein
   echter Service, stirbt mit dem Writer-Prozess, kein Cross-Vendor-Replay,
   keine PERSISTENT-Reboot-Garantie.
2. **„RC2-A-Datalake-Engine" wrappen** (roadmap RC3-C) — verworfen: die Engine
   existiert nicht (keine Crate), der Plan hing in der Luft. Diese ADR ersetzt
   ihn: das Lakehouse ist *ein Adapter*, kein vorzubauender Engine-Stack.
3. **Nur sqlite, kein Lakehouse** (milestone-v1.2 WP 2.2c eng) — verworfen: der
   Adapter-getriebene Schnitt kostet wenig mehr und liefert Warehouse/Lake in
   einem Aufwasch; „house vs lake" ist via DuckDB-über-Parquet derselbe Adapter.
4. **Konfigurierbares Late-Join-Disk-Budget** (Zwischenstand der Design-Session)
   — verworfen: hätte den QoS-Contract still beschnitten. Disk ist die
   Umkehrung des Memory-Budgets, nicht unabhängig kappbar (Invariante 2).
5. **Multi-Domain-Daemon** — zurückgestellt: ein Daemon pro Domain ist der
   einfachere, isolierte Default; Multi-Domain als spätere Config-Erweiterung.

## Konsequenzen

**Positiv:** echte TRANSIENT/PERSISTENT-Semantik (überlebt Writer-Prozess bzw.
Reboot); Cross-Vendor-Replay quasi gratis über RTPS; ein Trait, beliebige
Adapter (sqlite-simpel bis Lakehouse-Analytics); Vendor-Matrix
„Transient/Persistent (Service)" wird ✓; saubere Trennung Contract (QoS) vs.
Performance (Memory-Budget).

**Negativ/Risiken:** neue Deploy-Komponente (Daemon) — Betriebsaufwand
(systemd/Docker, Backup); DuckDB/Parquet ziehen C-Dependencies (Lakehouse-
Adapter feature-gated halten); Cold-Store-Streaming muss paginiert sein, sonst
RAM-Blowup bei großen Contracts.

**Folge-Aufgaben:** P1 store-Kern + sqlite/file; P2 Daemon + Client + Discovery
+ RTPS; P3 Lakehouse; P4 Crash-Recovery (5 Szenarien) + Cross-Vendor; P5
Packaging + Deployment-Doc + Matrix-Update.

## Referenzen

- DDS 1.4 §2.2.3.4 (Durability), §2.2.3.5 (DurabilityService)
- `internal/plans/milestone-v1.2.md` §WP 2.2c (durch diese ADR konkretisiert)
- `internal/roadmap/track-dds-persistence-service.md` (durch diese ADR ersetzt)
- `crates/dcps/src/durability_service.rs` (writer-embedded Vorläufer)
- `crates/qos/src/policies/durability_service.rs` (Contract-QoS)
