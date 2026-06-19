# DCPS Interop

This crate provides the public API for DDS applications and is tested
against other DDS stacks (Cyclone DDS, eProsima Fast-DDS) at the wire
level.

## Interop tiers

| Tier | Scope | Status |
|-------|-------|--------|
| **Wire compliance** | Byte-identical DATA/HEARTBEAT/ACKNACK submessages compared against a Cyclone capture. | ✅ (Cyclone replay tests) |
| **SPDP discovery**  | ZeroDDS discovers Cyclone/Fast-DDS participants over multicast. | ✅ (`tests/interop/matrix.sh`) |
| **SEDP discovery**  | ZeroDDS sees Cyclone publications in the SEDP cache. | ✅ (`cyclone_live_sedp.rs`) |
| **Cross-process DCPS** | Two ZeroDDS processes exchange user samples through the public API. | ✅ (`hello_dds_e2e.sh`) |
| **Cross-vendor DCPS** | ZeroDDS publisher ↔ Cyclone subscriber (and vice versa) on an application topic. | ✅ (`tests/interop/xv_pub_sub_roundtrip.sh`) |
| **Cross-arch DCPS** | macOS-aarch64 ↔ Linux-x86_64, self + Cyclone + Fast-DDS. | ✅ (`tests/interop/matrix.sh` + wire compliance) |

## Cross-process smoke

```bash
# Linux-only (multicast loopback)
./tests/interop/hello_dds_e2e.sh
```

Starts `hello_dds_subscriber` + `hello_dds_publisher` in parallel on
domain 0, lets them talk for 10 s, and checks that ≥ 3 `hello #N`
samples appear in the subscriber log. Covers SPDP + SEDP + reliable
data + topic-based matching through the public API.

Plus: DCPS shape tests through the public API:

```bash
cargo test --test e2e_dcps_api -p zerodds-dcps
```

These tests verify topic registry + entity hierarchy via
`create_participant_offline`, without a UDP bind. The **in-process
E2E data-flow test** through the public API will return once
`DataWriter::wait_for_acknowledgments()` +
`DataReader::wait_for_historical_data()` are introduced; without these
synchronization primitives, SPDP+SEDP discovery in the same process is
timing-flaky on shared CI runners.

In the meantime, the E2E data flow is covered by the runtime unit test
`two_runtimes_e2e_user_data_match_and_transfer` and the cross-process
script.

## Cross-vendor (Cyclone)

The cross-vendor live roundtrip is automated as a CI test
(`tests/interop/xv_pub_sub_roundtrip.sh`). Manual setup for ad-hoc
Cyclone interop tests:

### ZeroDDS publisher ↔ Cyclone subscriber

The default topic of `ddsperf` is not `Chatter`. You need:

1. ZeroDDS side: `hello_dds_publisher` starts topic `"Chatter"` with
   type name `"zerodds::RawBytes"`.
2. Cyclone side: a C subscriber that registers the same topic name +
   type name. Skeleton:

   ```c
   #include "dds/dds.h"
   // The topic type must have the same name; the content is an opaque
   // byte blob (no IDL, we are XCDR2 plain bytes here).
   dds_entity_t participant = dds_create_participant(0, NULL, NULL);
   dds_entity_t topic = dds_create_topic(
       participant, /*type-desc=*/&ByteArray_desc,
       "Chatter", NULL, NULL);
   ...
   ```

   As `type-desc`, an `IDL`-generated `sequence<octet>` type is
   sufficient — Cyclone's `IDLgen` provides the descriptor struct.

3. Both on the same multicast group (default 239.255.0.1:7400) and
   domain id.

**Why this already works today**: wire compliance has validated the
complete wire compliance (`cyclone_sedp_replay.rs`,
`cyclone_compliance.rs`, `cyclone_live_typelookup.rs`). All that is
missing is a test client on the Cyclone side with a matching topic/type
name.

### Cyclone publisher ↔ ZeroDDS subscriber

Analogous, reverse direction. `hello_dds_subscriber` registers on
`Chatter`, and a Cyclone program with the same topic + type writes
into it.

## References

- `tests/interop/matrix.sh` — SPDP discovery matrix (Cyclone + FastDDS).
- `tests/interop/hello_dds_e2e.sh` — cross-process E2E.
- `tests/interop/xv_pub_sub_roundtrip.sh` — cross-vendor pub/sub roundtrip (Cyclone ⇆ ZeroDDS).
- `crates/dcps/examples/hello_dds_publisher.rs` + `hello_dds_subscriber.rs`.
- `crates/dcps/tests/e2e_dcps_api.rs` — in-process integration test.
- `crates/discovery/tests/cyclone_live_sedp.rs` — SEDP cross-vendor live test.
