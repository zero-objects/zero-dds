# 0006 — PID_SHM_LOCATOR als Vendor-PID 0x8001

- **Status:** accepted
- **Datum:** 2026-05-04
- **Autoren:** @sandra
- **Kontext:** crates/rtps, crates/flatdata, docs/specs/zerodds-flatdata-1.0.md

## Kontext

Same-Host-Zero-Copy braucht eine Discovery-Komponente, die einem
Reader signalisiert: "es gibt ein SHM-Segment für dieses Topic, hier
ist der Pfad". Optionen:

1. Eigene SEDP-Submessage erfinden — bricht Cross-Vendor-Compat.
2. Eigenen Builtin-Topic anlegen — Compat-Risk + DCPS-Layer-Eingriff.
3. Vendor-PID im PublicationBuiltinTopicData — minimal-invasiv,
   Cross-Vendor-stillschweigend ignoriert.

Spec DDSI-RTPS 2.5 §9.6.2.3 erlaubt Vendor-spezifische PIDs im Range
[0x8000, 0xFFFF) mit gesetztem `VENDOR_SPECIFIC_BIT`. Cyclone und
Fast-DDS überspringen unbekannte Vendor-PIDs ohne Fehlermeldung.

## Entscheidung

**Wir definieren `PID_SHM_LOCATOR = 0x8001` als ZeroDDS-Vendor-PID
ohne MUST_UNDERSTAND-Bit.**

- **Wert-Layout** (little-endian):
  - `u32 hostname_hash` (FNV-1a über lokalen Hostname-String)
  - `u32 uid` (POSIX `uid_t`)
  - `u32 slot_count`
  - `u32 slot_size` (Slot-Total-Size, Header + Payload + Padding)
  - `CDR-String segment_path` (`/zddspub_<entity_id>` typisch)
- **MUST_UNDERSTAND**: nicht gesetzt — Cyclone/Fast-DDS-Reader
  ignorieren still und matchen weiterhin via UDP-DATA.
- **VENDOR_SPECIFIC_BIT**: gesetzt (PID 0x8001 mit bit 15 high).
- **Same-Host-Match-Bedingung**: lokaler `hostname_hash` ==
  `locator.hostname_hash` UND lokaler `uid` == `locator.uid`.
  Container-Friendly (gleicher Kernel = gleicher hostname).

## Alternativen

1. **Eigene SEDP-Submessage** — Wire-Bruch zu Cyclone/Fast-DDS.
   Verworfen.
2. **Builtin-Topic `ZeroDdsShmLocator`** — DCPS-Layer-Eingriff,
   Cyclone-Reader sieht Topic aber kann nicht parsen. Verworfen.
3. **Vendor-PID in PublicationBuiltinTopicData** (gewählt) —
   minimal-invasiv, transparent für andere Vendoren.

## Konsequenzen

**Positiv**:
- Cross-Vendor-Domains funktionieren weiterhin: ZeroDDS-Writer +
  Cyclone-Reader bekommen UDP-DATA wie immer; ZeroDDS-Reader sieht
  zusätzlich PID_SHM_LOCATOR und attached an SHM.
- Discovery-Pfad ändert sich nicht — gleicher SEDP-Endpoint, nur
  ein zusätzlicher PID im Wire.
- hostname_hash + uid als Match-Tupel ist Container-Friendly.

**Negativ**:
- 0x8001 ist nicht IANA-registriert — wenn ein anderer Vendor zufällig
  dieselbe PID nutzt, gibt's Wire-Collision (still, kein Crash).
  Mitigation: Kollision wird durch Layout-Validation erkannt
  (Header-Length + Hostname-Hash sind effektiv 64-bit Sentinel).
- Caller, der Cross-User-Container nutzt (uid 1000 ↔ uid 0), bekommt
  keine SHM — das ist absichtlich (Sicherheit), aber Doku-Pflicht.

**Folge-Aufgaben**:
- F2b: PosixSlotAllocator publiziert PID_SHM_LOCATOR im SEDP-Sample.
- F-Dual: DataWriter mit Feature `flatdata-integration` setzt PID
  automatisch.
- Doku: `docs/integration/flatdata-cross-vendor.md` erklärt das
  Verhalten gegenüber Cyclone/Fast-DDS.

## Referenzen

- `docs/specs/zerodds-flatdata-1.0.md` §3, D-3, D-4
- DDSI-RTPS 2.5 §9.6.2.3 (Parameter ID Encoding, Vendor-Specific Bit)
- `crates/rtps/src/parameter_list.rs::pid::SHM_LOCATOR`
- `crates/flatdata/src/locator.rs::ShmLocator`
