# Cyclone-DDS Reference-Frames

This file documents the hand-curated RTPS frames in
this directory.

## Important note

The frames are **not** obtained from real tshark captures of a
running Cyclone DDS, but hand-constructed per the
DDSI-RTPS 2.5 spec with Cyclone-typical parameters (VendorId,
GuidPrefix convention). They serve as a wire-format compliance test:

> If our reader parses these bytes as valid RTPS datagrams
> and our writer produces bytes with the same structure, then
> wire-format conformance with Cyclone DDS is plausible.

Real live interop comes with WP 0.7+ (discovery) and WP phase 1
(reliable + endpoint matching).

## Cyclone-DDS VendorId

Eclipse Cyclone DDS uses VendorId `0x01_10` (registered with the
OMG vendor-ID repository as "ADLINK / Eclipse Cyclone DDS").
Some older builds use `0x01_0F` (Vortex Cafe / Lite). Both
are acceptable as test fixtures — we accept arbitrary
VendorIds in the wire decoder.

## Frame capture guide (phase 1)

For real captures:

```bash
# Start the Cyclone container (see ../docker-compose.yml)
docker compose -f tests/interop/docker-compose.yml up -d

# Frame capture with tshark
sudo tshark -i any -f "udp port 7400 or udp port 7410" \
            -w cyclone_dump.pcap -c 50

# Hex export of individual DATA frames with the Wireshark GUI:
# File → Export Packet Bytes...
```

## Format of the hex files

Each `.hex` file contains **one** line per datagram, hex-encoded
without spaces or `0x` prefix. Multi-line is allowed — whitespace
is ignored.
