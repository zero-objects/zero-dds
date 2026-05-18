# Track 10-A — OMG-Vendor-ID-Vergabe

**Goal:** offizielle OMG-Vendor-ID für ZeroDDS, sodass der RTPS-
ProtocolHeader-vendor_id im byte-stream identifiziert.

**Status:** ⏸ external-blocked (warten auf OMG)

## Hintergrund

OMG vergibt 16-bit Vendor-IDs für DDSI-RTPS-Implementations. Liste:
[OMG-DDSI-RTPS-Vendor-IDs](https://www.omg.org/spec/DDSI-RTPS/2.5/PDF
Annex). Stand 2026:
- 0x0101 RTI Connext
- 0x0102 ADLINK OpenSplice
- 0x0103 OCI OpenDDS
- 0x0104 Eclipse Cyclone
- 0x010F eProsima Fast DDS
- 0x0118 ADLINK Vortex
- ... weitere

ZeroDDS hat **keine** Vendor-ID. Aktueller Workaround: provisorische
Marker (siehe Vendor-ID-pending-Banner auf der Website).

## Aktionen

### Antrag (sollte bereits eingereicht sein)

- E-Mail an admin@omg.org mit Antrag auf Vendor-ID-Vergabe für ZeroDDS
- Nachweise: zerodds.org-Site, GitHub-Repo, Spec-Coverage gegen
  DDSI-RTPS 2.5

### Während Wartezeit

- Provisorische Vendor-ID `0xFFFF` (laut Spec für "experimental") nutzen
- Vendor-ID-pending-Banner auf Landing + Claims (✅ live)
- README.md-Disclaimer (✅ live)

### Nach Zuweisung

- Vendor-ID-Konstante in `crates/rtps/src/protocol_header.rs`
- Compile-time-feature-Flag entfernen, Default ist die echte ID
- Vendor-ID-pending-Banner von der Website entfernen
- README.md-Disclaimer entfernen
- Cross-vendor-fixtures regenerieren (jeder Capture mit neuer ID)

### Conformance-Audit-Update

`docs/spec-coverage/ddsi-rtps-2.5.md` §8.3.5.1 (vendor_id) bekommt einen
Note "Assigned OMG Vendor-ID: 0xXXXX" mit Audit-Date.

## Acceptance

1. OMG-Mail mit zugewiesener Vendor-ID erhalten
2. Vendor-ID-Konstante im Code
3. Cross-vendor-Tests gegen Cyclone DDS / RTI Connext mit korrekter
   ID-Erkennung
4. Banner + Disclaimer von der Website entfernt
5. Spec-Coverage aktualisiert

## Dependencies

- Externe: OMG-Antragsbearbeitung (Wochen bis Monate)

## Risks

- **Antrag wird abgelehnt** (sehr selten, OMG ist offen für neue
  Vendoren). Mitigation: pre-application-Konsultation per OMG-TC-Mail
- **Lange Wartezeit blockiert 1.0-Tag**. Mitigation: 1.0-final auf einen
  Daten gesetzt, falls OMG länger als 90 Tage braucht: Re-Plan, ggf.
  Provisorische ID weiterführen + 1.0.x-Patch nach Vergabe
