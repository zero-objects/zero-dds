# ZeroDDS ↔ Cyclone DDS interop compliance report

**Status: WP 0.6 completed, 2026-04-18.**

This report documents the state of wire format compliance
between ZeroDDS (phase 0) and Eclipse Cyclone DDS.

---

## What is checked (phase 0)

### ✅ Wire format compliance via reference frames

`crates/rtps/tests/cyclone_compliance.rs` (11 tests):

| Test area | Status | Evidence |
|---|---|---|
| ZeroDDS reader parses Cyclone DATA with empty payload | ✅ | `cyclone_data_empty_payload_decodes` |
| ZeroDDS reader parses Cyclone DATA with CDR2-LE payload | ✅ | `cyclone_data_cdr2_payload_decodes_and_carries_payload` |
| ZeroDDS reader parses Cyclone HEARTBEAT | ✅ | `cyclone_heartbeat_decodes` |
| All frames have "RTPS" magic bytes | ✅ | `cyclone_frames_have_rtps_magic` |
| All frames are RTPS 2.x | ✅ | `cyclone_frames_use_rtps_2_5_or_compatible` |
| ZeroDDS writer output is byte-identical to the Cyclone layout (DATA empty) | ✅ | `zerodds_writer_produces_compatible_data_layout` |
| ZeroDDS writer output is byte-identical to the Cyclone layout (DATA with CDR2) | ✅ | `zerodds_writer_produces_compatible_data_with_payload` |
| Cyclone → decode → encode → bit-identical | ✅ | `cyclone_data_*_roundtrip_bit_identical` |

**Important limitation**: the reference frames are **not**
extracted from a real Cyclone DDS capture, but maintained by hand
according to the DDSI-RTPS 2.5 spec with Cyclone-typical parameters (VendorId
0x0110, version 2.5). Phase 1 will supplement this with real tshark captures
— see `capture.sh` and `docker-compose.yml`.

### ✅ Docker Compose test harness

`tests/interop/docker-compose.yml` starts two Cyclone DDS containers
(publisher + subscriber) on the loopback interface. `capture.sh`
uses tshark to extract frames.

This harness does **not** run in CI (containers + sudo tshark), but
enables developers to capture new reference frames and insert them into
`crates/rtps/tests/fixtures/cyclone/`.

---

## What is not checked (phase 1+)

### ❌ Live interop

ZeroDDS and Cyclone DDS cannot yet "see" each other
— the crux:

1. **SPDP** (Simple Participant Discovery Protocol):
   - Cyclone periodically sends multicast beacons with
     ParticipantBuiltinTopic data
   - ZeroDDS does not implement multicast reception yet
   - **Phase 1: WP-Phase-1-XXX (Discovery)**

2. **SEDP** (Simple Endpoint Discovery Protocol):
   - Reliable path for the subscription/publication builtin topics
   - ZeroDDS has no reliable writer with an AckNack loop yet
   - **Phase 1**

3. **Endpoint matching**:
   - Topic name + type name + QoS must be compatible
   - Requires dds-types (TypeObject, TypeIdentifier)
   - **Phase 1: WP-Phase-1-YYY (Type-System)**

4. **Reliable reliability**:
   - AckNack loop, heartbeat timer, history cache with resend
   - **Phase 1: WP-Phase-1-ZZZ (Reliable-Writer)**

### ❌ Multi-vendor tests

- RTI Connext: VendorId 0x0103, own quirks (e.g. KeyHash mandatory
  in the DATA submessage)
- Fast-DDS: VendorId 0x010F (eProsima)
- OpenDDS: VendorId 0x0103 (legacy)
- **Phase 1+: WP-Phase-2 Multi-Vendor**

---

## Phase 1 hand-off

**Next steps for real interop**:

1. **WP 0.7+** Discovery crate (`dds-discovery`):
   - SPDP beacon sender + receiver
   - SEDP reliable writer + reader
   - ParticipantBuiltinTopic + Sub/PubBuiltinTopic schemas
2. **WP 0.7+** Reliable writer in `dds-rtps`:
   - History cache with sequence tracking
   - AckNack loop + heartbeat timer
   - GAP encoding for resend leaks
3. **WP-Phase-1** dds-types:
   - TypeObject / TypeIdentifier
   - Type compatibility matrix
4. **WP-Phase-1** Live smoke test:
   - ZeroDDS container in `tests/interop/docker-compose.yml`
   - Topic "ZeroDDS_Compat_Test"
   - Cyclone DDS subscriber → receives ZeroDDS writer data
   - Both, order reversed

---

## Capture procedure (for manual tests)

```bash
# 1. Start the Cyclone DDS containers
cd tests/interop
docker compose up -d

# 2. Capture RTPS frames (sudo because of libpcap)
./capture.sh 100 cyclone_dump.pcap
# → cyclone_dump.pcap + cyclone_dump.hex

# 3. Hex inspection of a frame (e.g. the first DATA)
head -1 cyclone_dump.hex | xxd -r -p | xxd

# 4. Insert the frame as a fixture:
#    crates/rtps/tests/fixtures/cyclone/<topic_name>.hex
#    → comment the header bytes with '#'
#    → add a test in cyclone_compliance.rs

# 5. Stop
docker compose down
```

---

## Sales point

> **ZeroDDS produces byte-identical RTPS DATA submessages like
> Eclipse Cyclone DDS** (verified via reference frame roundtrip).
> Wire format incompatibilities are ruled out — the only
> step to live interop is discovery + reliable, not
> wire format bugs.

This is the most critical property for migration promises.

## Empty ACKNACK of the vendors

Fast DDS (and, as recorded, OpenDDS and RTI Connext) send the ACKNACK of a reader that
holds nothing with `bitmapBase` 0 and `numBits` 0, which DDSI-RTPS 2.5 §8.3.5.5 makes
invalid. ZeroDDS by default drops the rest of such a datagram (§8.3.4.1 rule 6), and with
it the SEDP data that follows in it (measured on the Linux test host, sec-diag16/17: every red
zerodds->fastdds cell had rejections with `left_out` > 0, every green cell none; with the
setting all 24 cells green and no rejection). The cross-vendor runners in this directory (`ci/run_vendor.sh`, `run_bridge_cross_vendor.sh`, `shapes_cyclone_interop.sh`, `xv_pub_sub_roundtrip.sh`) and `tools/dcps-shapes-interop.sh` sets
`ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO=1` (setting
`wire.accept_empty_acknack_base_zero`) for the ZeroDDS processes of the vendor pairs, in
one place (one commented block per runner). The default of ZeroDDS stays the reading of the spec.

## PL_CDR1 parameters with the rounded length

Cyclone DDS (extended header) and RTI Connext (short header) write the length of a PL_CDR1
parameter rounded up to four, the form of DDSI-RTPS 2.5 §9.4.2.11 (recorded rows;
`ast-layout` `Dialect::Cyclone` and `Dialect::Rti`, `pads_declared_length`). XTypes 1.3
§7.4.1.2.2 has the exact length, and the default reader of ZeroDDS refuses the other form.
These runners sets `ZERODDS_WIRE_PL_CDR1_READING=tolerant` (setting `wire.pl_cdr1_reading`) for the ZeroDDS readers of the pairs with a Cyclone DDS or RTI writer (a runner that meets every vendor sets it for all pairs),
in the same place as the empty-ACKNACK setting. Fast DDS and OpenDDS write the exact
length. The default of ZeroDDS stays the reading of the spec.
