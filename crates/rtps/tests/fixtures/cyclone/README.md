# Cyclone-DDS Reference-Frames

Diese Datei dokumentiert die hand-kuratierten RTPS-Frames in
diesem Verzeichnis.

## Wichtige Notiz

Die Frames sind **nicht** durch echte tshark-Captures aus einem
laufenden Cyclone-DDS gewonnen, sondern hand-konstruiert nach der
DDSI-RTPS-2.5-Spec mit Cyclone-typischen Parametern (VendorId,
GuidPrefix-Konvention). Sie dienen als Wire-Format-Compliance-Test:

> Wenn unser Reader diese Bytes als gueltige RTPS-Datagrams parst
> und unser Writer Bytes mit der gleichen Struktur produziert, ist
> die Wire-Format-Konformitaet zu Cyclone-DDS plausibel.

Echte Live-Interop kommt mit WP 0.7+ (Discovery) und WP-Phase-1
(Reliable + Endpoint-Matching).

## Cyclone-DDS VendorId

Eclipse Cyclone DDS verwendet VendorId `0x01_10` (registriert beim
OMG Vendor-ID-Repository als "ADLINK / Eclipse Cyclone DDS").
Manche aeltere Builds nutzen `0x01_0F` (Vortex Cafe / Lite). Beide
sind als Test-Fixtures akzeptabel — wir akzeptieren beliebige
VendorIds im Wire-Decoder.

## Frame-Capture-Anleitung (Phase 1)

Fuer echte Captures:

```bash
# Cyclone-Container starten (siehe ../docker-compose.yml)
docker compose -f tests/interop/docker-compose.yml up -d

# Frame-Capture mit tshark
sudo tshark -i any -f "udp port 7400 or udp port 7410" \
            -w cyclone_dump.pcap -c 50

# Hex-Export einzelner DATA-Frames mit Wireshark-GUI:
# File → Export Packet Bytes...
```

## Format der Hex-Files

Jede `.hex`-Datei enthaelt **eine** Zeile pro Datagram, hex-encoded
ohne Leerzeichen oder `0x`-Prefix. Multi-Line ist erlaubt — Whitespace
wird ignoriert.
