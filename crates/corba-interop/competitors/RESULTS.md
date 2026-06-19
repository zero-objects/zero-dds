# CORBA IIOP Roundtrip — ZeroDDS vs. TAO / omniORB / JacORB

Reproducible latency comparison data against the actively available CORBA ORBs.
Replaces the previously claimed (unsubstantiated, mislabeled) "~95 us" on the website.

## Methodology
- Host: the Linux test host (Debian 13, kernel 6.17), loopback (127.0.0.1), IIOP/GIOP 1.2.
- Operation: `interface Echo { string ping(in string msg); }` — echo of the argument.
- **Separate server + client processes** (forces a real wire; omniORB/TAO would
  otherwise short-circuit colocated calls). IOR exchange via file.
- N = 50,000 measured roundtrips after warmup, one connection each, p50/p90/p99.
- Optimized builds (Rust `--release`, C++ `-O2`, JVM steady-state after warmup).

## Results (32-byte payload, codegen path — apples-to-apples)

All comparison ORBs use their generated stubs; ZeroDDS therefore also uses the
codegen path.

| ORB | Version | min | p50 | p90 | p99 |
|-----|---------|-----|-----|-----|-----|
| **ZeroDDS CORBA** | 1.0.0-rc.2 | 14.6 | **17.8** | 25.6 | 45.2 |
| omniORB | 4.3.3 | 14.1 | 17.8 | 22.3 | 42.3 |
| TAO (ACE+TAO) | 2.5.24 | 24.3 | 36.2 | 42.8 | 85.9 |
| JacORB | 3.9 (JDK 8) | 43.5 | 59.6 | 83.6 | 140.2 |

(Hand-marshalled ZeroDDS: p50 17.0 µs — codegen costs ~0.8 µs.)
(256 B: p50 ZeroDDS 17.2 / omni 18.1 / TAO 37.9 / JacORB 59.2. 4096 B: 20.2 / 22.5 / 50.6 / 78.4.)

**Raw output:** `results.txt` (this run, all payloads + feature matrix + SSLIOP).

## Findings
- ZeroDDS CORBA is **on par with omniORB** (the fastest established C++ ORB):
  equal at 32 B, slightly ahead at 256 B/4096 B — and ~2× faster than TAO,
  ~3.5× faster than JacORB. All in pure Rust, without a C++ toolchain.
- IOR byte order: omniORB + TAO emit **little-endian** (`01...`), JacORB
  **big-endian** (`00...`). Relevant for cross-ORB interop (Milestone 2).
- **JacORB does not run on modern Java** (CORBA module removed since Java 11) —
  requires JDK 8 (`/opt/jdk8`, Temurin 1.8.0_492).

## Setup (the Linux test host)
- omniORB 4.3.3: `apt install omniorb omniidl libomniorb4-dev`
- TAO 2.5.24: via OpenDDS bundle `/opt/opendds` (`tao_idl`, libs `/opt/opendds/lib`)
- JacORB 3.9: `/opt/jacorb` + JDK 8 `/opt/jdk8`
- ZeroDDS: `cargo run --release -p zerodds-corba-interop --bin echo_bench -- 32 50000`

Build sources per ORB: `competitors/{omniorb,tao,jacorb}/` (server + client + Echo.idl).
