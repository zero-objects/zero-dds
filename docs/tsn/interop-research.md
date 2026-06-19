# Cross-Vendor-TSN-Interop — Research-Spike (Go/No-Go)

Stand 2026-06-10. Frage: Können wir den ZeroDDS-DDS-TSN-Ethernet-PSM
(Annex A, RTPS direkt im Ethernet-Frame, fester EtherType `0x88B5`)
cross-vendor gegen RTI Connext / Fast DDS / Cyclone testen?

## Kurzantwort

**Über den 0x88B5-Ethernet-PSM: NO-GO.** Kein anderer Vendor liefert
eine interoperable Implementierung des OMG-DDS-TSN-Annex-A-Wireformats.

**Über RTPS/UDP auf einem TSN-geschedulten Netz: GO** — aber das ist eine
andere Schicht (Netz-Scheduling, kein Wire-Protokoll-Tausch) und braucht
TSN-Hardware (siehe `hardware-eval.md`).

## Befunde pro Vendor (Versionen auf codepit)

### Cyclone DDS 11.0.1 — hat `raweth`, aber inkompatibel
`General/Transport = raweth` existiert (`DDSI_TRANS_RAWETH` in
`ddsi_config.h`), Adressform `raweth/01:00:5e:7f:00:01.<vlan>.<…>`.
**Aber:** die Log-Meldung im Binary lautet
`"using port number as ethernet type"` — Cyclone mappt die **RTPS-
Portnummer auf den EtherType**, statt den festen DDS-TSN-Wert `0x88B5`
zu verwenden. Das Wireformat ist also ein Cyclone-Eigenbau (Vor-Spec-
Prior-Art), **nicht** der OMG-Annex-A-PSM → keine direkte Interop mit
unserem `ETHERTYPE_RTPS = 0x88B5`.

### RTI Connext 7.7.0 — „DDS + TSN", aber über UDP
RTI vermarktet DDS+TSN (Latenz-Budgets, Deadlines, Reliability über ein
TSN-Netz), aber als **RTPS/UDP über ein TSN-geschedultes Netz** (taprio/
gPTP), nicht als Raw-Ethernet-PSM. Im installierten SDK kein
Ethernet-/0x88B5-Transport-Plugin gefunden (die „TSN"-Header-Treffer
waren `reda_sequenceNumber` = *transmission sequence number*, nicht
Time-Sensitive-Networking).

### Fast DDS 3.x — „DDS-TSN Pro" über TransportPriority/UDP
eProsima dokumentiert ein „DDS-TSN Pro"-Feature, aber transportseitig
nur TCP/UDP/SHM (`SocketTransportDescriptor`, `TCP*`, kein
`EthernetTransportDescriptor`). TSN-Support = `TransportPriority`-QoS
(PCP-Mapping) über UDP auf einem TSN-Netz, kein Raw-Ethernet-PSM.

### NXP/dds-tsn (Referenz-Demo)
Integriert RTI Connext + Fast DDS **über UDP** auf taprio-/gPTP-
konfigurierten i.MX-Boards — bestätigt: der gelebte Cross-Vendor-Weg
ist RTPS/UDP-über-TSN-Netz, nicht der Ethernet-PSM.

## Schlussfolgerung

| Interop-Achse | Machbar? | Voraussetzung |
|---|---|---|
| ZeroDDS ↔ ZeroDDS, 0x88B5-PSM | ✅ bewiesen | `tests/.../veth_loopback.rs` |
| ZeroDDS ↔ Vendor, 0x88B5-PSM | ❌ NO-GO | kein Vendor implementiert es interoperabel |
| ZeroDDS ↔ Vendor, RTPS/UDP über TSN-Netz | ✅ GO | TSN-HW (taprio/gPTP); UDP-Interop haben wir bereits cross-vendor |
| ZeroDDS ↔ Cyclone `raweth` | ⚠️ nur mit Cyclone-Eigenformat | Port-als-EtherType nachbauen; niedrige Priorität |

## Empfehlung

1. **0x88B5-Cross-Vendor jetzt nicht bauen** — es gibt keinen Peer. Wieder
   aufgreifen, wenn ein Vendor den Annex-A-PSM ausliefert.
2. **Echter TSN-Interop-Pfad = RTPS/UDP über ein TSN-Netz.** Das
   bestehende Cross-Vendor-UDP-Interop (`ci/jobs/interop-matrix.yml`) auf
   einem taprio-geschedulten Link wiederverwenden — sobald TSN-HW da ist
   (`hardware-eval.md`). Dann zeigen: ZeroDDS hält unter Cross-Traffic die
   Latenzklasse, die Vendoren ebenso.
3. **Optionaler Cheap-Win (niedrig):** einen `raweth`-kompatiblen Modus
   (Port-als-EtherType) als ZeroDDS-Vendor-Extension, um live gegen
   Cyclone-raweth zu sprechen. Nur falls ein konkreter Cyclone-raweth-
   Anwendungsfall auftaucht.
