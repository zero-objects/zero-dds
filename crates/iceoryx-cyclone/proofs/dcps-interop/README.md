# DCPS-level ZeroDDS ↔ Cyclone iceoryx interop proof

Proves the bridge is **wired into the DCPS API**, not just a standalone helper:
a normal typed `DataWriter<KMsg>` / `DataReader<KMsg>` — once
`enable_cyclone_iox()`-ed — interoperates with Cyclone DDS over the iceoryx C++
(POSH) transport, for a **variable-length, keyed** type
(`IoxOracle::KMsg { @key uint32 id; string name; uint32 value; }`) carried as
Cyclone's serialized PSMX form (classic CDR / XCDR1).

The proof uses only the public ZeroDDS API:

```rust
let dr = sub.create_datareader::<KMsg>(&topic, DataReaderQos::default())?;
dr.enable_cyclone_iox()?;
for m in dr.take()? { /* Cyclone-published KMsg */ }

let dw = pubr.create_datawriter::<KMsg>(&topic, DataWriterQos::default())?;
dw.enable_cyclone_iox()?;
dw.write(&KMsg { id: 7, name: "...".into(), value: 0xABCD })?;
```

## What `run.sh` verifies — both directions, live

- **READ** (`READER OK`) — a Cyclone publisher → `DataReader<KMsg>::take()`
  returns the Cyclone sample, classic-CDR-decoded (`decode_xcdr1`). End-to-end
  through the real DCPS take path; exercises serialized + keyed + variable-length.
- **WRITE** (`CYCLONE OK`) — `DataWriter<KMsg>::write()` on an **online** RTPS
  participant (domain 0) → a Cyclone subscriber that **discovers the writer over
  SEDP** and accepts its serialized iceoryx sample by **GUID match**. This is the
  production path: the iceoryx chunk's metadata GUID is the writer's real RTPS
  GUID (`DataWriter::rtps_guid()`), so Cyclone associates the shared-memory
  sample with the discovered writer — no `ALLOW_NONDISCOVERED_WRITERS` shortcut.

### Why the writer must be online for the WRITE leg

Cyclone's `ddsi_serdata_from_psmx` looks the writer up by the chunk's GUID. A
**RAW self-contained** sample is accepted from a *non-discovered* writer
(`../run.sh` shows `CYCLONE OK` for the fixed `Sample` type via
`ALLOW_NONDISCOVERED_WRITERS`), but a **SERIALIZED** sample needs the writer
discovered over RTPS SEDP — which the online ZeroDDS participant provides (its
RTPS 2.5 wire is byte-compatible with Cyclone). The proof's `writeonline` mode
brings up that online participant; `cyclone_online.xml` gives Cyclone the iox
PSMX **and** SPDP-multicast discovery on domain 0.

## Run

Linux host with iceoryx POSH + iox-roudi + Cyclone-iox (e.g. codepit):

```sh
./run.sh   # builds the proof + a Cyclone publisher + an iceoryx consumer,
           # starts iox-roudi, asserts READ PASS + WRITE-bytes PASS
```
