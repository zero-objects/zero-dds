# RTPS-Submessage Golden-Vectors (WP 1.10 T2)

Hex-Dumps kompletter RTPS-Submessage-Bodies (ohne Submessage-Header).

## Vorhanden

- `heartbeat_minimal_le.hex` — 28-byte HEARTBEAT-Body, reader_id=
  BuiltinSubscriptionsReader, writer_id=BuiltinSubscriptionsWriter,
  first_sn=1, last_sn=10, count=0x42, flags: E=1, F=L=0.

## Geplant (WP 1.11 + Wireshark-Captures)

- DATA (no inlineQos), DATA_FRAG, ACKNACK, GAP, NACK_FRAG —
  Cross-Impl-Vectors aus Cyclone/Fast-DDS-Captures.
