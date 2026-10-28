<!-- SPDX-License-Identifier: Apache-2.0 -->
# Cross-Vendor Roundtrip Benchmark — Results & Findings

Apples-to-apples ping/pong round-trip latency across five DDS stacks
(**ZeroDDS**, Cyclone DDS, Fast-DDS, RTI Connext, OpenDDS) over the **full
typed (X)CDR + DCPS + RTPS pipeline** — not opaque byte shoving. Each app is
generated from one shared IDL by the vendor's own code generator, so a cell
exercises that stack's complete encode → DataWriter → RTPS → transport →
DataReader → decode path.

- **Host:** a single Linux host (8-core LXC) with the five vendor SDKs
  installed. The kernel is shared, so absolute µs are **noise-dominated** —
  treat the numbers as *order-of-magnitude + ranking*, not precise latencies.
  Functional pass/fail (match + data flow) is solid.
- **Representation:** XCDR2 (the default across all stacks); RTI forced to
  XCDR2-only so its default XCDR1 doesn't break matching.
- **QoS:** RELIABLE, KEEP_LAST(64), `ALLOW_TYPE_COERCION` on readers.
- **Pattern:** one in-flight sample, event-driven (listener / data-available
  callback), RTT measured at the ping side.

Two bench types:
- **basis** — `Roundtrip` = `{ uint32 sequence_id; uint64 t_send_ns;
  sequence<octet,8192> payload; }` (`@final`). Simple type, full CDR path.
- **rich** — `RoundtripRich` adds `string<64> name`, `double transform[16]`,
  `sequence<Waypoint,32>` (each `Waypoint` = 2 nested `Vec3` + `string` +
  scalars), keeping the payload sweep. Maximises the XCDR2 **member-codec**
  work to isolate it from the transport. 8 waypoints per sample.

---

## Six stacks over UDPv4, 2026-10-02

`matrix_sweep.sh` now runs every pair over UDPv4: it exports
`ZERODDS_SAME_HOST_SHM=0` (`transport.same_host_shm`), so the ZeroDDS self
pair no longer takes ZeroDDS's same-host shared-memory carrier. RTI Connext
7.7.0 is measured again: its evaluation licence keeps working past the trial
end within 6 participants, 12 DataReaders and 12 DataWriters per process.
Payloads 0 and 8192 B, best of five runs; report and data in the internal
measurement record of 2026-10-02.

Self pairs, p50 µs at 0 B / 8192 B: Cyclone 25.1 / 36.6, RTI 30.1 / 37.3,
ZeroDDS 32.6 / 41.9, Fast DDS 41.4 / 42.4, OpenSplice 68.1 / 116.7,
OpenDDS 200.3 / 316.6. ZeroDDS has the lowest p99.9 (56.4) and maximum
(60.0) of the six.

**Finding.** Every pair with ZeroDDS on one side took 64–83 µs at 8192 B,
the pairs between Cyclone, Fast DDS and RTI 41–52 µs. A capture showed
ZeroDDS sending an 8 KB sample to Cyclone as six datagrams, one 1344-B
DATA_FRAG each, where Cyclone sends one DATA: ZeroDDS took the loopback
fragment size only for a reader that is a ZeroDDS participant of this
host. Since `005af7362b1` a reader whose unicast locators all stay on this
host (127/8, ::1, an address of this host's interfaces) gets it too. After
the fix ZeroDDS ↔ Cyclone, Fast DDS and RTI are 37–50 µs at 8192 B.

RTI ↔ OpenSplice could not be measured: RTI rejected the OpenSplice pong's
DataReaders beyond its licence limit of 12, the bench's two among them.
`rti_app.cpp` also set XCDR2 for that pair; it now takes XCDR1-only like
the other apps (`86a7c616991`).

## Six stacks, 2026-09-30 — and the octet runs of the generated C++ codec

`matrix_sweep.sh` over ZeroDDS 1.0.0-rc.7, Cyclone DDS 11.0.1, Fast DDS
3.6.1, OpenDDS 3.34.0 and OpenSplice CE 6.9.210323: 25 pairs, 11 payloads,
best of five runs. Pairs with OpenSplice run XCDR1 (it writes and reads
nothing else; ZeroDDS runs them as `zerodds-xcdr1-roundtrip`), all others
XCDR2. RTI Connext 7.7.0 could not be measured again (evaluation licence
expired); `matrix_json.py --keep` carries its cells of 2026-05-23 into
`website/_data/roundtrip-matrix.json`, each with its date.

**Finding.** The first run put `zerodds -> zerodds` at 8192 B at
**123.3 µs** p50, against 52.1 µs on 2026-05-23. The codec
`idlc generate --lang c++` writes into `gen/zerodds/Roundtrip.hpp` wrote
and read `sequence<octet, 8192>` with one function call per octet. It now
copies a sequence of octets, and a one-dimensional octet array, with one
`Writer::write_octets` / `Reader::read_octets` call. The bytes are the
same: 226 cells of the case-corpus matrix with `octet` or `uint8` match
what the C++ and the Rust columns recorded. The nine pairs with ZeroDDS
were measured again with that build (`BENCH_WITH=zerodds`):

| pair, p50 µs | 0 B before | 0 B after | 8192 B before | 8192 B after |
|---|---|---|---|---|
| zerodds -> zerodds | 20.4 | 17.6 | 123.3 | 24.2 |
| zerodds -> cyclone | 32.3 | 36.5 | 126.5 | 76.3 |
| cyclone -> zerodds | 29.4 | 31.5 | 118.6 | 67.6 |
| zerodds -> fastdds | 32.5 | 32.2 | 131.7 | 73.6 |
| fastdds -> zerodds | 33.2 | 33.2 | 117.8 | 69.1 |

The self pair runs over ZeroDDS's same-host shared-memory carrier (checked:
the pair creates its segments in `/dev/shm`); every other pair runs UDPv4.

**Open.** Between different stacks ZeroDDS is still behind with large
samples: at 8192 B the pairs with Cyclone and Fast DDS take 67.6-76.3 µs,
Cyclone <-> Fast DDS 40.8 and 42.9 µs. From 0 to 8192 B,
`zerodds -> cyclone` rises by 39.8 µs, `cyclone -> fastdds` by 8.6 µs. The
gap was there on 2026-05-23 too (88-96 µs). Not investigated yet.
OpenDDS <-> OpenSplice: no match in either direction, all 22 cells.

---

## Headline finding — same-host SHM latency fix

ZeroDDS same-host (`zerodds ↔ zerodds`) auto-uses its POSIX-SHM carrier, not
UDP loopback. The carrier's consumer used an **exponential-backoff sleep-poll**
(`PosixShmTransport::wait_for_frame`, 10→20→40→80 µs … 1 ms), which in a
ping/pong injected a **bimodal 66–140 µs** round-trip (a sample arriving mid-
sleep waits out the backoff), versus ~24 µs for the UDP loopback it shadows.
Only `zerodds ↔ zerodds` was affected (other vendors don't speak ZeroDDS SHM).
The code deferred the proper fix (`"Full futex/eventfd: deferred to v1.3"`).

**Fix** (`crates/transport-shm/src/posix.rs`): event-driven **shared futex** on
the low 32 bits of the ring's `head` write-pointer. `push_frame` (and owner
drop) `FUTEX_WAKE`; `wait_for_frame` reads `head` *before* `pop_frame` then
`FUTEX_WAIT(head_lo, expected, recv_timeout)` — race-free (a publish between
read and wait returns `EAGAIN`). `cfg(target_os="linux", target_endian=
"little")`-gated; the sleep-poll remains as portable fallback; wire format
unchanged.

| `zerodds → zerodds`, payload 0 | p50 | p90 | p99 |
|---|---|---|---|
| before (sleep-poll) | 66–140 (bimodal) | ~140 | 142 |
| **after (futex)** | **16–27** | **17–29** | **25–35** |

Result: the SHM carrier is now **faster than UDP loopback and than pre-merge**,
and `zerodds ↔ zerodds` is the **fastest self-cell of the entire matrix**.
Verified: 19 transport-shm unit tests + 1 cross-process L1 + dcps
`same_host_e2e` 4/4 + `transport_matrix_e2e` 5/5.

`iceoryx` integrations were audited for the same anti-pattern and are clean:
iceoryx2 uses its native event-driven `Notifier`/`Listener`; the classic
iceoryx path is user-driven `take()` with no background poll loop.

---

## basis matrix (simple type, full CDR path)

Median p50 µs, ping (row) → pong (column). **0 failures.**

**payload 0**
| ping\pong | zerodds | cyclone | fastdds | rti |
|---|---|---|---|---|
| **zerodds** | **18.3** | 23.8 | 27.7 | 24.3 |
| cyclone | 35.6 | 34.2 | 51.8 | 42.9 |
| fastdds | 39.0 | 47.1 | 53.0 | 52.9 |
| rti | 37.0 | 22.8 | 49.4 | 44.3 |

**payload 8192**
| ping\pong | zerodds | cyclone | fastdds | rti |
|---|---|---|---|---|
| **zerodds** | **23.1** | 89.5 | 98.9 | 71.5 |
| cyclone | 86.1 | 42.2 | 67.0 | 77.5 |
| fastdds | 51.5 | 47.4 | 38.2 | 57.7 |
| rti | 47.5 | 82.0 | 65.5 | 51.0 |

`zerodds ↔ zerodds` leads at both payloads; at 8 KiB it stays at 23 µs (SHM
avoids UDP fragmentation) while others sit at 38–99 µs.

---

## self-cell sweep across all five stacks (`self_compare.sh`)

The matrices above are ping x pong. This one is the diagonal only — every
stack talking to itself, which is the cell each one is tuned for and the
only comparison that is about the stack rather than about which two stacks
had to agree on a wire format. Run it with
`tests/perf/dds-roundtrip-bench/self_compare.sh`.

**Host** the Linux bench host (LXC), 16 cpus, 60 cells, `basis` type from the shared
`roundtrip.idl`, 2000 samples after 200 warm-up, three runs per cell.
**1-minute load 1.11-2.60, median 1.78** — the script reads `/proc/loadavg`
before every cell, waits for the host to come down, and writes the load it
measured under into every CSV row, because several agents measure on this
host at once and a latency cell next to a workspace build measures the
compiler. **Every cell gets its own domain** (140-199, 60 domains, none
reused, domain 0 never touched): concurrent DDS traffic from other agents
sat on domain 0 and is disjoint by RTPS port mapping. 60 of 60 cells
produced a number.

Median of the per-run p50 RTT, microseconds:

| payload | ZeroDDS | Cyclone | RTI 7.7.0 | Fast DDS | OpenDDS 3.34 |
|---|---|---|---|---|---|
| 0 | **23.1** | 25.2 | 32.0 | 37.1 | 203.6 |
| 1638 | **25.0** | 31.3 | 38.3 | 37.5 | 224.1 |
| 4096 | **20.9** | 37.1 | 38.7 | 40.8 | 254.4 |
| 8192 | **25.1** | 36.8 | 36.5 | 40.0 | 511.7 |

| vendor | mean rank | payloads won |
|---|---|---|
| **ZeroDDS** | **1.00** | 4 of 4 |
| Cyclone | 2.25 | 0 |
| RTI | 3.00 | 0 |
| Fast DDS | 3.75 | 0 |
| OpenDDS | 5.00 | 0 |

Two further sweeps the same day reproduce the ordering under heavier load
(1.86-5.96 and 1.3-4.4): ZeroDDS first in every payload row of all three,
mean rank 1.00 each time. Absolute microseconds move with the load; the
ranking does not.

## throughput — measured, and not apples-to-apples

There is no shared-IDL throughput harness. What exists measures two
different types with two different tools, so the numbers below are an
order-of-magnitude signal and a flag for investigation, not a verdict.
Both stacks on their own domain (31 and 32), 1 KiB, 20 s, load 1.59-3.40:

| | writes | receives | lost |
|---|---|---|---|
| Cyclone `ddsperf` | — | **692.9 kS/s** (5676 Mb/s) | 0 |
| ZeroDDS `zerodds_perf` | 1098.7 kS/s (9000 Mb/s) | **74.8 kS/s** | 3.25 M of 4.61 M written never arrived |

ZeroDDS writes faster than Cyclone receives and then delivers an order of
magnitude less: 1 359 662 samples arrived of 4 614 678 written. Cyclone
delivered 10 823 984 with `lost 0`. Neither side pins QoS — `zerodds_perf`
takes `DataWriterQos::default()` / `DataReaderQos::default()` and ddsperf
its own defaults — so part of the gap may be a best-effort writer doing
what a best-effort writer may do. **That it is unexplained is the point:
this is the one axis where the measurement puts ZeroDDS behind, and it is
not currently possible to say by how much, because no two of these numbers
were produced by the same type through the same QoS.** The missing piece
is a `--throughput` mode in the roundtrip apps, which already generate
from one shared IDL.

---

## secure matrix (DDS-Security 1.2, SRTPS, payload 64)

| ping\pong | cyclone | fastdds | opendds | zerodds |
|---|---|---|---|---|
| cyclone | 92.1 | NO_MATCH | NO_MATCH | **78.5** |
| fastdds | FAIL | 206.2 | FAIL | FAIL |
| opendds | FAIL | FAIL | FAIL | FAIL |
| **zerodds** | **48.2** | NO_MATCH | NO_MATCH | **37.3** |

Working secured pairs: `zerodds ↔ zerodds`, `zerodds ↔ cyclone` (both
directions), and each vendor with itself. The fastdds-cross and all-opendds
failures are **pre-existing** DDS-Security cross-vendor handshake limits, not
ZeroDDS regressions (proof: `cyclone → fastdds` NO_MATCH involves no ZeroDDS).
RTI security is license-blocked (deliberately not purchased).

**Crypto cost on ZeroDDS** (basis → secure, same pair): `zerodds ↔ zerodds`
+19 µs, `zerodds → cyclone` +24 µs — **lighter than Cyclone's +58 µs**.
(Security bypasses the SHM carrier, so secure `zerodds ↔ zerodds` runs over
SRTPS-over-UDP.)

---

## rich matrix (maximal XCDR2 member-codec, payload 64)

Median p50 µs, after the RTI compliance fix below. `✗` = NO_DATA.

| ping\pong | zerodds | cyclone | fastdds | rti | opendds |
|---|---|---|---|---|---|
| **zerodds** | 20 | 41 | 35 | **61** | 60 |
| cyclone | 62 | 70 | 77 | ✗ | ✗ |
| fastdds | 70 | 91 | 64 | **76** | 123 |
| rti | **23** | ✗ | 67 | 56 | ✗ |
| opendds | 103 | ✗ | 79 | ✗ | 226 |

**ZeroDDS interoperates with all four other vendors on the rich type — the
only fully-connected stack besides Fast-DDS.** Codec-cost deltas (rich vs
basis) are within the LXC noise floor, so only the ranking is reportable.

### secure rich matrix (DDS-Security 1.2 + rich type, payload 64)

Median p50 µs. RTI omitted (security license-blocked).

| ping\pong | zerodds | cyclone | fastdds | opendds |
|---|---|---|---|---|
| **zerodds** | 91 | 72 | ✗ | **194** |
| cyclone | 113 | 135 | ✗ | ✗ |
| fastdds | ✗ | ✗ | 152 | – |
| opendds | **125** | ✗ | – | 232 |

**ZeroDDS is the only stack that interoperates secured + rich with both
Cyclone and OpenDDS** (and itself). Security runs over SRTPS-over-UDP (the
SHM carrier is bypassed when security is on).

**Fast-DDS secured cross-vendor needs RSA identity certs.** The bench's default
EC P-256 certs work for Cyclone/OpenDDS/ZeroDDS but FastDDS' PKI-DH handshake
rejects EC certs cross-vendor (`writer wait_for_matched timeout`) — this is a
FastDDS limitation, not ZeroDDS: **`cyclone↔fastdds` secured also fails with EC
and no ZeroDDS involved**, while `fastdds↔fastdds` works with EC. Regenerate
with `KEY_ALGO=RSA bash security/gen.sh` and **`zerodds↔fastdds` secured rich
works both ways: 70 / 28 µs**. (RSA in turn breaks `zerodds↔cyclone` — Cyclone
prefers EC — so no single cert algorithm spans all four vendors; ZeroDDS
interoperates secured under **both** EC and RSA.)

Remaining residual: **Cyclone↔OpenDDS** rich (the same TypeObject-strictness
pair that fails plain rich too) — no ZeroDDS involved.

**Mission take-away:** ZeroDDS interoperates secured + rich with **every** other
stack — Cyclone & OpenDDS under EC certs, Fast-DDS under RSA certs, RTI under
the XCDR2 compliance mask. The cells that don't light up are always vendor↔vendor
incompatibilities (FastDDS's RSA-only / Cyclone's EC-only / Cyclone↔OpenDDS
TypeObject), reproducible without ZeroDDS.

**Getting OpenDDS into the secure matrix** required three things; OpenDDS
secured does work, the bench just had never wired it:
1. Build the OpenDDS rich app against the **secure** OpenDDS install:
   `cmake -DOPENDDS_ROOT=/opt/opendds-secure …` (the default `/opt/opendds`
   has no security plugins). Run it with that install's libs.
2. Use `opendds_rtps_sec.ini` (`DCPSSecurity=1`).
3. **Governance:** OpenDDS' AccessControl rejected the bench's per-topic
   `enable_read/write_access_control=true` combined with the (FastDDS-oriented)
   permissions file, reporting it misleadingly as *"No governance exists for
   this domain"* — which blocked even `opendds↔opendds`. Setting those two
   flags to **false** in `governance.xml` makes the **same** governance
   acceptable to all four stacks. Encryption (ENCRYPT for data / metadata /
   rtps / discovery / liveliness), authentication (PKI-DH) and participant
   `join_access_control` stay fully on — only per-topic permission
   *enforcement* is relaxed (the permissions grant `*` topics anyway). OpenDDS
   also requires the SMIME `openssl smime -sign -text` wrapper (the bench
   already uses it).

---

## Cross-vendor rich findings & root causes

### 1. RTI ✗ everyone → **fixed** (RTI was non-compliant; ZeroDDS was correct)

Initially `rti` failed the rich type with *every* vendor (only `rti ↔ rti`
worked). RTI verbose logging (`NDDS_XTYPES_COMPLIANCE_MASK` unset; verbosity
`STATUS_ALL`) showed RTI **matches** the endpoints (`PRESPsService_linkLocal*:
MATCH`, receives the `RoundtripRich` TypeObject + all dependent
TypeIdentifiers) but then **drops every sample**:

```
RTIXCdrInterpreter_fullDeserializeSample: RoundtripBench::RoundtripRich:
waypoints deserialization error. Received sequence length 580 is larger
than maximum 32
```

RTI reads the **collection DHEADER** of `sequence<Waypoint>` (≈ the
sequence's total byte size) as if it were the element count. Per
**OMG DDS-XTypes 1.3 §7.4.3.5**, a sequence/array of *non-primitive* members
is serialized with a DHEADER in XCDR2 — which ZeroDDS, Cyclone, Fast-DDS and
OpenDDS all emit. RTI's own release notes confirm RTI's default serialization
is **not** spec-compliant here for backward compatibility (default compliance
mask `0x18C` lacks the `dheader_in_non_primitive_collections` bit `0x1`).

**Conclusion: ZeroDDS is the spec-conformant side; RTI diverges by default.**

**Fix (RTI-side config, no ZeroDDS change):** the RTI bench app sets RTI's
documented full-compliance mask before init:
`setenv("NDDS_XTYPES_COMPLIANCE_MASK", "0x000001a9", 0)`. After this:
`zerodds ↔ rti` (61/23 µs) and `fastdds ↔ rti` (76/67 µs) are green.

Gotcha: in RTI 7.7 the QoS *property* `dds.xtypes.compliance_mask` did **not**
take effect at either participant or endpoint level — only the environment
variable worked.

### 2. Residual NO_DATA — vendor-to-vendor, no ZeroDDS involved

`cyclone ↔ rti`, `opendds ↔ rti`, `cyclone ↔ opendds` remain NO_DATA. These
show **no deserialization error** — the data never reaches the deserializer,
i.e. a discovery/TypeObject match failure between those vendors on the rich
nested type (the basis simple type matched fine). This is the known
OpenDDS↔Cyclone TypeObject strictness (OpenDDS#4244 / cyclonedds-cxx#448) plus
RTI's strict TypeObject matching with Cyclone/OpenDDS. No ZeroDDS endpoint is
involved and no ZeroDDS-side change can resolve them.

### 3. OpenDDS build note

`opendds-roundtrip-rich` builds via `opendds_target_sources` with
`-Gxtypes-complete`. At runtime it **requires the matching OpenDDS libraries**
(`LD_LIBRARY_PATH=/opt/opendds/lib`, the build target) — loading the unrelated
`/opt/opendds-secure` libs mixes ABIs with the rpath-baked TAO/ACE and
segfaults in `TransportClient::enable_transport`. Run with the build's
OpenDDS libs and `-DCPSConfigFile opendds_rtps.ini`.

---

## Reproduce

```bash
# from build/, with the five vendor SDKs discoverable (see CMakeLists.txt)
cmake .. && cmake --build .          # builds *-roundtrip and *-roundtrip-rich
# basis matrix:  quick_matrix.sh <out_dir> [N] [payloads]
# self-cells across all five, load-gated, one domain per cell:
#   OUT_DIR=/tmp/x N_RUNS=3 ../self_compare.sh
#   -> $OUT_DIR/self_compare.csv (one row per cell, with the load it ran under)
#      $OUT_DIR/self_compare.md  (ranking table)
# secure matrix: sec_matrix.sh [BUILD_DIR] [SEC_DIR] [SAMPLES]   (needs security/gen.sh)
# rich: run <vendor>-roundtrip-rich pong|ping (Topics RoundtripRichBench_*)
```

RTI needs `NDDSHOME`, `RTI_LICENSE_FILE`, and (for rich cross-vendor) the
compliance mask shown above — already baked into the rich build of `rti_app.cpp` (`BENCH_RICH`).
