# OpenDDS Non-Secure Cross-Vendor Interop — Closeout (2026-06-11)

ZeroDDS ↔ OpenDDS (OCI, vendorId `0x0103`) **plain DDSI-RTPS** roundtrip interop
over `rtps_udp`, both directions, all sweep payloads, measured on codepit vs.
OpenDDS `3.34.0` (`/opt/opendds`, `libOpenDDS_Dcps.so.3.34.0`).

This closes the **last open OpenDDS interop axis**: the secure matrix was already
closed at 9/13 = the ZeroDDS maximum (see `docs/security/opendds-secure-matrix-closeout.md`),
but the *non-secure* functional roundtrip — which exists for Cyclone/Fast-DDS/RTI
— had never been run or recorded with OpenDDS (OpenDDS was excluded from the
default `quick_matrix.sh`/`iso_matrix.sh` vendor list).

## Result: **16 / 16 green** — full roundtrip, both directions, full payload sweep

Reliable (`RELIABLE` + `KEEP_LAST(64)`) typed roundtrip through the complete DCPS
→ XCDR2 → DDSI-RTPS 2.5 → `rtps_udp` pipeline, both roles, payloads `0 / 1638 /
4096 / 8192` bytes, 2 runs each. **No timeouts, no ghosts, no NO_MATCH.** The
8192-byte cell is **fragmented** (DATA_FRAG, >1500-byte MTU) and green in both
directions — the classic cross-vendor fragmentation-reassembly case.

| ping → pong | p=0 | p=1638 | p=4096 | p=8192 (frag) |
|---|---|---|---|---|
| **ZeroDDS → OpenDDS** | 64.9 / 67.7 µs | 105.8 / 85.0 µs | 143.1 / 152.2 µs | 147.2 / 149.3 µs |
| **OpenDDS → ZeroDDS** | 79.3 / 82.5 µs | 113.2 / 126.7 µs | 115.8 / 251.3 µs | 140.9 / 178.1 µs |

(p50 of each of the 2 runs; raw CSV: `tests/perf/dds-roundtrip-bench` →
`opendds_matrix.sh` regenerates it.)

### What made it work (already in-tree, no new fixes needed)
1. **`-DCPSConfigFile opendds_rtps.ini`** with `DCPSDefaultDiscovery=rtps_disc`
   + `transport_type=rtps_udp` — OpenDDS defaults to InfoRepo discovery + a
   non-RTPS transport; interop *requires* RTPS on both.
2. **`use_xtypes=no`** in the discovery config — ZeroDDS's C-FFI byte path sends
   no TypeObject (`type_identifier=None`); OpenDDS then matches purely on
   topic-name + type-name, exactly as Cyclone/Fast-DDS/RTI do with ZeroDDS.
3. **`opendds_idl … -Gxtypes-complete`** (CMake `opendds_target_sources`) — emits
   the *complete* TypeObject so strict XTypes consumers don't reject minimal-only.
4. The `@final @autoid(SEQUENTIAL)` + pinned `@id` IDL (`roundtrip.idl`) gives a
   byte-identical wire layout + TypeObject across all five codegens.

The non-secure path is **strictly easier than the secure one that was already
closed**, so — consistent with the security closeout — it works out of the box.

## The payload bound is the IDL contract, not an interop limit

A stress probe beyond the sweep range (16384 / 32768 / 64000 bytes) is RED in
**both** directions — *and OpenDDS↔OpenDDS-self is RED there too*:

```
terminate called after throwing an instance of 'CORBA::BAD_PARAM'
```

Root cause: `roundtrip.idl` bounds the payload — `@id(2) sequence<octet, 8192> payload`.
Any sample `> 8192` bytes exceeds the **type's own bound**, so OpenDDS's generated
TypeSupport spec-correctly throws `CORBA::BAD_PARAM` on the write (proven: it
crashes OpenDDS talking to itself, independent of ZeroDDS). The legitimate
roundtrip range is therefore `0 … 8192` — and it is fully green.

**One honest ZeroDDS observation (out of interop scope):** ZeroDDS's `zerodds_app`
uses the byte-oriented C-FFI write path (`zerodds_writer_write` with raw bytes),
which does **not** enforce the IDL `sequence<octet, 8192>` bound — so
ZeroDDS↔ZeroDDS-self "succeeds" at 16384 (110 µs) where OpenDDS correctly refuses.
This is the byte-FFI being type-agnostic by design (the same property that makes
`use_xtypes=no` matching work), not an interop defect. Enforcing IDL sequence
bounds in the typed codegen path is a separate conformance item, tracked apart
from interop in
[`bounded-sequence-enforcement-followup.md`](bounded-sequence-enforcement-followup.md)
(open question, to be addressed after Durability-Service P5).

## Verdict

ZeroDDS ↔ OpenDDS interoperates over plain DDSI-RTPS for the full DCPS pipeline,
both directions, including fragmentation, with zero ZeroDDS-side changes. Every
cell in the type's design range is green; every cell beyond it is OpenDDS's own
spec-correct `BAD_PARAM` on an out-of-bound sequence. **Complete.**

## Reproduction (codepit / any Linux with OpenDDS 3.34 + cyclonedds-tools)

```bash
# build libzerodds + the cross-vendor roundtrip apps
cargo build --release -p zerodds-c-api --features security
cd tests/perf/dds-roundtrip-bench && rm -rf build && mkdir build && cd build
export PATH=/opt/opendds/bin:$PATH
export LD_LIBRARY_PATH=/opt/opendds/lib:/opt/cyclone/lib:$PWD/../../../../target/release:$LD_LIBRARY_PATH
cmake .. -DOPENDDS_ROOT=/opt/opendds && make zerodds-roundtrip opendds-roundtrip -j4
# run the ZeroDDS<->OpenDDS matrix
../opendds_matrix.sh
```
