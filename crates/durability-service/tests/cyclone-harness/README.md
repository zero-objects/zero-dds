# Cross-vendor durability INGEST harness (CycloneDDS → ZeroDDS)

Proves the ZeroDDS durability service ingests a **real foreign-vendor**
(CycloneDDS) `TRANSIENT_LOCAL` writer over the wire — the cross-vendor side of
the [`external_cyclone_ingest`](../external_cyclone.rs) test.

Verified on codepit (CycloneDDS 0.11, domain 140): `matched=1`, sample ingested.

## What it exercises

A CycloneDDS publisher of the **final** type `dur::Sample` on topic `DurVendorX`.
A final type makes Cyclone advertise an **XCDR1-only** writer — the case that
surfaced the interop fix: the ZeroDDS ingest reader must offer both XCDR1 and
XCDR2 (`data_representation_offer = [0, 2]` in `user_reader_cfg`), otherwise the
reader is RxO-incompatible and never matches.

## Build + run (one host, e.g. codepit)

```sh
# 1. Build the Cyclone publisher (needs CycloneDDS: idlc + libddsc)
cd tests/cyclone-harness
/opt/cyclone/bin/idlc dur.idl                       # → dur.c, dur.h
cc -I. -I/opt/cyclone/include pub.c dur.c \
   -L/opt/cyclone/lib -lddsc -o pub

# 2. Start the ZeroDDS durability service test (serves + polls 40s)
cd <repo>
ZD_DOMAIN=140 ZD_TOPIC=DurVendorX ZD_TYPE="dur::Sample" \
  cargo test -p zerodds-durability-service --test external_cyclone \
  -- --ignored --nocapture external_cyclone_ingest &

# 3. After ~4s, start the Cyclone publisher on the same domain
sleep 4
cd tests/cyclone-harness
DOM=140 LD_LIBRARY_PATH=/opt/cyclone/lib ./pub
```

Expected: `matched=1` (Cyclone side) and `ZD_INGESTED=1` + `test result: ok`
(ZeroDDS side). `dur.c` / `dur.h` / `pub` are idlc/compiler output — not checked in.

## Replay representation-fidelity (alignment-sensitive)

`al.idl` + `align_pub.c` + `align_sub.c` prove the **replay** direction is
representation-faithful with an alignment-sensitive type (`al::AlignS` =
`octet tag; long long val`, whose XCDR1 and XCDR2 layouts differ).

```sh
cd tests/cyclone-harness
/opt/cyclone/bin/idlc al.idl
cc -I. -I/opt/cyclone/include align_pub.c al.c -L/opt/cyclone/lib -lddsc -o align_pub
cc -I. -I/opt/cyclone/include align_sub.c al.c -L/opt/cyclone/lib -lddsc -o align_sub

# Service holds open; pub writes then dies; sub joins AFTER and gets the replay:
cd <repo>
ZD_DOMAIN=140 ZD_TOPIC=AlignTopic ZD_TYPE="al::AlignS" ZD_HOLD_SECS=35 \
  cargo test -p zerodds-durability-service --test external_cyclone \
  -- --ignored --nocapture external_cyclone_ingest &
sleep 4
cd tests/cyclone-harness
DOM=140 LD_LIBRARY_PATH=/opt/cyclone/lib ./align_pub          # writes, stays 7s, exits
DOM=140 LD_LIBRARY_PATH=/opt/cyclone/lib ./align_sub          # late-joiner
```

Expected: `SUB GOT tag=7 val=1122334455667788` — the byte-exact value proves the
ZeroDDS replay declared an XCDR1 encap matching the XCDR1 body (a wrong encap
would mis-align `val` or fail RxO matching).
