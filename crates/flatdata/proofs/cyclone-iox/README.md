# Cross-vendor iceoryx (C++/POSH) SHM proof — Cyclone → external consumer

Proves a ZeroDDS-style external consumer can read a **Cyclone DDS** sample over
the **real iceoryx C++ (POSH) shared-memory transport** — genuine cross-vendor
same-host zero-copy. This is the iceoryx that Cyclone uses (POSH + the
`iox-roudi` daemon, via Cyclone's `iox_psmx` plugin), distinct from the
iceoryx2-Rust stack ZeroDDS bridges to natively (see
`../../examples/iceoryx2_oracle_publisher.rs` + `tools/iceoryx2-oracle-consumer`).

## What it shows

`pub.c` is a Cyclone publisher of a fixed (self-contained) IDL type
`IoxOracle::Sample { uint32 id; uint32 value; octet data[8]; }`, with the iceoryx
PSMX enabled (`cyclone_iox.xml`). `consumer.c` links **only**
`libiceoryx_binding_c` (no Cyclone/DDS library) and:

1. registers with `iox-roudi` (`iox_runtime_init`);
2. discovers Cyclone's iceoryx service (`iox_service_discovery_find_service_apply_callable`);
3. subscribes and takes a chunk;
4. parses the Cyclone PSMX user-header (`dds_psmx_metadata_t`) and the raw
   payload, then verifies the bytes.

## Empirically captured ground truth (codepit, Cyclone 0.11 / iceoryx POSH 2.0.6)

Corrects/confirms `docs/interop/cyclone-iceoryx-shm-ground-truth.md`:

- iceoryx **ServiceDescription** = `{ "DDS_CYCLONE", "<type-name>", ".<topic>" }`
  — the service string is the fixed `DDS_CYCLONE` (not the PSMX `SERVICE_NAME`,
  which is a DDS-level instance name); instance = the IDL type name
  (`IoxOracle::Sample`); event = `"."` + topic for the empty partition
  (`.IoxOracleTopic`).
- user-header `dds_psmx_metadata_t` field sizes: `sample_state` (uint32 enum,
  observed `2` = `RAW_DATA`), `data_type` (uint32), **`instance_id` (uint32)**,
  `sample_size` (uint32), `guid[16]`, `timestamp` (int64), `statusinfo` (uint32),
  `cdr_identifier`/`cdr_options` (uint16). `instance_id` is uint32, not uint64 —
  the offset that matters for reading `sample_size`.
- a fixed/self-contained type travels **raw** (state `RAW_DATA`): the payload is
  the native little-endian C-struct, no CDR.

Observed run:
```
==> subscribing to Cyclone service {DDS_CYCLONE | IoxOracle::Sample | .IoxOracleTopic}
METADATA: sample_size=16 state=2 cdr_id=0x00c0 statusinfo=0 guid=01107395..
PAYLOAD[16]: 01 00 00 00 78 56 34 12 01 02 03 04 05 06 07 08
ORACLE OK: external iceoryx(C++) consumer decoded the Cyclone sample over SHM
```
`01 00 00 00 | 78 56 34 12 | 01..08` = `Sample{ id=1, value=0x12345678, data=1..8 }`.

## Run

Linux host with the iceoryx POSH + Cyclone-iox stack (e.g. codepit):
```sh
./run.sh        # builds pub + consumer, starts iox-roudi, publishes, asserts ORACLE OK
```
