# zerodds-iceoryx-cyclone

ZeroDDS ↔ Cyclone DDS **same-host zero-copy** bridge over the iceoryx **C++
(POSH)** shared-memory transport — the SHM stack Cyclone uses through its
`iox_psmx` plugin (with the `iox-roudi` daemon), as opposed to the iceoryx2-Rust
stack [`zerodds-flatdata`](../flatdata) bridges to natively.

The crate FFI-binds the stable `libiceoryx_binding_c` C ABI (no C++ name
mangling) and speaks Cyclone's PSMX chunk convention directly, so a ZeroDDS
process reads Cyclone-published samples and writes ones a Cyclone reader
consumes — proven bidirectionally on a live Cyclone + iceoryx + RouDi stack.

## API

```rust
use zerodds_iceoryx_cyclone::{CycloneIoxReader, CycloneIoxWriter, DdsPsmxMetadata, iox_event_name};

// READ a Cyclone publication:
let (svc, inst, ev) = CycloneIoxReader::discover_by_instance("app", "IoxOracle::Sample", 50).unwrap();
let r = CycloneIoxReader::new("app", &svc, &inst, &ev);
if let Some((meta, payload)) = r.take() { /* meta.is_raw(), payload bytes */ }

// WRITE a sample a Cyclone reader consumes:
let w = CycloneIoxWriter::new("app", "DDS_CYCLONE", "IoxOracle::Sample", &iox_event_name("", "IoxOracleTopic"));
w.write_raw(&DdsPsmxMetadata::raw(payload.len() as u32, guid), &payload);
```

## Wire facts

Reverse-engineered + live-verified (see
`docs/interop/cyclone-iceoryx-shm-ground-truth.md`):

- iceoryx ServiceDescription `{ <INSTANCE_NAME, e.g. "DDS_CYCLONE">, <type-name>, ".<topic>" }`;
- each chunk's iceoryx user-header is a [`DdsPsmxMetadata`] (`instance_id` is
  `u32`), the user payload the sample (raw native struct for a self-contained
  type, `RAW_DATA`);
- for Cyclone to accept a **non-discovered** writer (the ZeroDDS writer is a raw
  iceoryx publisher), its PSMX needs `ALLOW_NONDISCOVERED_WRITERS=true` — which
  also makes Cyclone's own writers iox-ineligible, so the read/write proofs use
  separate Cyclone configs.

## Building / proving

`posh` is off by default: the FFI links `libiceoryx_binding_c` and needs a Linux
host with the iceoryx POSH stack. Without it the crate still builds anywhere
(exposing only [`DdsPsmxMetadata`]).

```sh
cargo build -p zerodds-iceoryx-cyclone --features posh   # Linux + iceoryx POSH
./crates/iceoryx-cyclone/proofs/run.sh                   # bidirectional live proof
```

`run.sh` builds the crate + a Cyclone publisher/subscriber, starts `iox-roudi`,
and asserts both `READER OK` (ZeroDDS reads Cyclone) and `CYCLONE OK` (Cyclone
reads ZeroDDS). Verified on the codepit bench host (Cyclone 0.11 / iceoryx POSH
2.0.6).
