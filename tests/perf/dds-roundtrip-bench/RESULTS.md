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
# secure matrix: sec_matrix.sh [BUILD_DIR] [SEC_DIR] [SAMPLES]   (needs security/gen.sh)
# rich: run <vendor>-roundtrip-rich pong|ping (Topics RoundtripRichBench_*)
```

RTI needs `NDDSHOME`, `RTI_LICENSE_FILE`, and (for rich cross-vendor) the
compliance mask shown above — already baked into `rti_app_rich.cpp`.
