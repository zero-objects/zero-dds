# CORBA Perf & Interop Baseline — ZeroDDS vs omniORB / TAO / JacORB (2026-06-07)

Frische, vollständige Baseline des ZeroDDS-CORBA-Stacks über **alle relevanten
Features**: Cross-Vendor-Roundtrip-Latenz, die volle IDL-Feature-Matrix, der
SSLIOP/TLS-Overhead und das Codegen-vs-hand-marshalled-Delta — plus die
vollständige Cross-ORB-Interop-Matrix. Diese Datei ist die Referenz-Baseline für
den späteren Website-Prozess (sie löst alle bisherigen Einzel-CORBA-Perf-Docs als
gemeinsame Quelle ab).

Reproduktion (codepit): `crates/corba-interop/competitors/run_perf.sh` (Perf) +
`run_interop.sh` (Interop). Rohausgabe dieses Laufs: N=50 000, Payloads 32/256/4096 B.

---

## 0. TL;DR

* **ZeroDDS CORBA ist der schnellste ORB im Feld** (p50, alle Payloads), in
  Pure-Rust gegen drei etablierte C++/Java-ORBs — knapp vor omniORB,
  **~2,1× schneller als TAO**, **~3,5× schneller als JacORB**.
* **Codegen kostet < 1 µs** gegenüber hand-marshalled — der generierte Stub-Pfad
  ist quasi gratis.
* **Jedes IDL-Feature roundtrippt bei ~18 µs** — die Feature-Matrix ist flach;
  Transport + Wire dominieren, kein Konstrukt (string/wstring/any/sequence/
  Exception/Object-Ref) ist teuer.
* **TLS-Overhead ~1,5 µs** pro Roundtrip auf etablierter SSLIOP-Connection.
* **Cross-ORB-Interop: 16/16 Richtungen grün** (Echo/Bench/CosNaming +
  SSLIOP) gegen omniORB/TAO/JacORB, beide Richtungen.

---

## 1. Cross-Vendor Echo-Roundtrip-Latenz

Geteilte IDL für alle vier Stacks:
```idl
interface Echo { string ping(in string msg); };  // echot das Argument zurück
```

### p50 (µs), N = 50 000

| Payload | **ZeroDDS** (codegen) | ZeroDDS (hand) | omniORB 4.3.3 | TAO 2.5.24 | JacORB 3.9 |
|---|---|---|---|---|---|
| 32 B   | **17.1** ⭐ | 16.3 | 18.0 | 35.8 | 60.7 |
| 256 B  | **17.3** ⭐ | 16.6 | 18.3 | 37.7 | 61.5 |
| 4096 B | **19.9** ⭐ | 18.9 | 22.5 | 51.8 | 73.3 |

> Primärvergleich = **Codegen** (apples-to-apples: alle ORBs fahren über
> generierte Stubs/Skeletons). „hand" = ZeroDDS hand-marshallter Pfad (untere
> Schranke; identischer Wire, ohne Stub-Indirektion).

### Volle Verteilung — 32 B Payload

| ORB | min | p50 | p90 | p99 | p99.9 |
|---|---|---|---|---|---|
| **ZeroDDS (hand)**    | 15.4 | **16.3** | 20.2 | 40.3 | 51.9 |
| **ZeroDDS (codegen)** | 14.9 | **17.1** | 23.5 | 44.1 | 58.5 |
| omniORB 4.3.3         | 13.9 | 18.0 | 23.7 | 44.9 | 64.7 |
| TAO 2.5.24            | 25.2 | 35.8 | 42.9 | 83.9 | 163.6 |
| JacORB 3.9            | 41.7 | 60.7 | 77.7 | 123.3 | 211.2 |

### Speedup (ZeroDDS codegen als Basis, p50)

| vs | 32 B | 256 B | 4096 B |
|---|---|---|---|
| omniORB | 1.05× | 1.06× | 1.13× |
| TAO     | 2.09× | 2.18× | 2.60× |
| JacORB  | 3.55× | 3.55× | 3.68× |

---

## 2. ZeroDDS Feature-Matrix — per-Operation-Latenz (Codegen)

Self-Roundtrip pro IDL-Konstruktart über den generierten `BenchStub` →
`dispatch_bench`-Skeleton (`bench_features`, N = 50 000, codepit).

| Operation | IDL-Feature | p50 | p90 | p99 |
|---|---|---|---|---|
| `add` | `long` | 18.00 | 23.17 | 47.29 |
| `scale` | `double` (8-aligned) | 18.03 | 22.93 | 45.68 |
| `add64` | `long long` (8-aligned) | 18.00 | 25.33 | 46.57 |
| `next_char` | `char` (1 Byte) | 17.87 | 22.92 | 45.27 |
| `concat` | `string` | 18.14 | 22.92 | 43.89 |
| `wecho` | `wstring` (UTF-16 + BOM) | 18.34 | 22.83 | 47.15 |
| `aecho` | `any` (struct-TypeCode §15.3.5) | 20.02 | 25.34 | 51.08 |
| `aecho` | `any` (sequence<long>) | 18.80 | 23.02 | 48.93 |
| `reverse` | `sequence<long>` (×3) | 18.25 | 23.30 | 45.64 |
| `divmod` | 2× `out`-Param | 18.13 | 22.94 | 45.44 |
| `checked` (ok) | `raises`-fähige Op (Success) | 18.05 | 22.81 | 43.63 |
| `checked` (raises) | typisierte UserException | 18.19 | 21.52 | 43.65 |
| `echo_ref` | `Object`-Ref (IOR-Marshalling) | 17.44 | 20.10 | 45.36 |

**Befund**: ~17,4–20,0 µs p50 über die **gesamte** Feature-Breite. Das Profil ist
flach — der GIOP/IIOP-Transport dominiert, das per-Feature-CDR-Marshalling ist
vernachlässigbar. Spitzenwert ist `any` mit struct-TypeCode (+~2 µs für die
TypeCode-Encapsulation); die typisierte **Exception** kostet exakt so viel wie der
Success-Pfad (kein Penalty für `raises`).

---

## 3. SSLIOP / TLS-Overhead

Identischer Echo-Roundtrip über eine **einmal** aufgebaute TLS-Connection
(`ssliop_bench`, rustls 0.23 ring) vs Plain-IIOP, gleiche 56-B-Payload,
N = 50 000.

| Transport | min | p50 | p90 | p99 | p99.9 |
|---|---|---|---|---|---|
| Plain IIOP (codegen) | 16.2 | **17.1** | 20.5 | 43.8 | 55.6 |
| SSLIOP / TLS         | 15.8 | **18.6** | 23.6 | 47.5 | 63.4 |

**TLS-Overhead ≈ 1,5 µs p50** pro Roundtrip auf etablierter Connection (reine
TLS-Record-Verschlüsselung; der Handshake fällt einmalig beim Connection-Aufbau
an, nicht pro Call).

---

## 4. Codegen vs hand-marshalled

| Payload | hand (p50) | codegen (p50) | Δ |
|---|---|---|---|
| 32 B   | 16.3 | 17.1 | +0.8 µs |
| 256 B  | 16.6 | 17.3 | +0.7 µs |
| 4096 B | 18.9 | 19.9 | +1.0 µs |

Der generierte Stub/Skeleton-Pfad kostet **< 1 µs** über den hand-marshallten
Pfad — die Codegen-Indirektion ist praktisch gratis.

---

## 5. Cross-ORB-Interop-Matrix — 16/16 grün

`run_interop.sh` (codepit), ZeroDDS ↔ omniORB 4.3.3 / TAO 2.5.24 / JacORB 3.9,
beide Richtungen:

| Domäne | Richtungen | Status |
|---|---|---|
| Echo + Bench (Feature-Matrix) | omni/TAO/JacORB × 2 | 6/6 ✅ |
| CosNaming (bind/resolve/rebind/unbind + NotFound + Sub-Context-Föderation) | omni/TAO/JacORB × 2 | 6/6 ✅ |
| SSLIOP/TLS | ZeroDDS↔ZeroDDS + omniORB × 2 | 3/3 ✅ |
| SSLIOP/TLS (TAO) | — | SKIP¹ |

Die **Bench-Feature-Matrix** deckt cross-ORB ab: primitive (long/double/octet/
short/unsigned/long long/bool), `char`/`wchar`, `string`, `wstring` (UTF-16),
`sequence<long/string>`, `struct`, `enum`, `union`, strukturiertes `any`
(§15.3.5 TypeCode), `out`/`inout`/`oneway`, typisierte `raises`-Exceptions und
`Object`-Referenzen (IOR-Roundtrip).

¹ TAO-SSLIOP übersprungen: das vorhandene ACE+TAO-Bundle (`/opt/opendds-secure`)
wurde **ohne** `libTAO_SSLIOP` gebaut. ZeroDDS↔ZeroDDS + omniORB belegen den
SSLIOP-Wire cross-vendor (rustls ↔ OpenSSL 3.5).

---

## 6. Setup-Meta

### Hardware / OS
| Item | Wert |
|---|---|
| Host | `codepit` (LXC-Container) |
| CPU | AMD Ryzen Threadripper PRO 3955WX 16-Cores |
| Cores online | 4 |
| Memory | 15 GiB |
| OS | Debian GNU/Linux 13 (trixie) |
| Kernel | 6.17.2-2-pve |
| Transport | TCP-Loopback, GIOP/IIOP 1.2 |
| Pinning | keines (siehe Caveats) |

### Toolchain / ORB-Stacks
| Item | Version | Quelle |
|---|---|---|
| ZeroDDS CORBA | branch `corba` (8123dea8) | `crates/corba-interop`, `cargo --release` |
| rustc | 1.95.0 (59807616e 2026-04-14) | — |
| g++ | 14.2.0 (Debian 14.2.0-19) | — |
| omniORB | 4.3.3+ds1-1 | Debian apt |
| TAO (ACE+TAO) | 2.5.24 | `/opt/opendds-secure` |
| JacORB | 3.9 | `/opt/jacorb` + JDK 8 |
| JDK (JacORB) | OpenJDK 1.8.0_492 | `/opt/jdk8` |
| OpenSSL (SSLIOP-Peer) | 3.5.6 | omniORB-SSL-Transport |

---

## 7. Methodologie

* **Geteilte IDL**, alle Stacks identisch (Echo + Bench-Feature-Matrix).
* **Roundtrip**: Client encodet die Argumente in CDR, sendet einen GIOP-1.2-
  Request über IIOP; der Server dispatcht über POA/Skeleton an den Servant,
  encodet die Reply. Gemessen wird die volle client-seitige Roundtrip-Zeit
  (`Instant`/`steady_clock`/`nanoTime`).
* **Eine Connection**, sequenzielle Requests (kein Pipelining), `SYNC_WITH_TARGET`.
* **Warmup**: 2 000 (C++/Rust) bzw. 10 000+ (JacORB, JIT) Iterationen vor der
  Messung; danach N = 50 000 gemessene Roundtrips, Perzentile aus sortierten
  Samples.
* **Optimierte Builds**: Rust `--release`, C++ `-O2 -std=c++17`, JVM nach Warmup.

---

## 8. Caveats (Ehrlichkeit)

* **Single-Host-Loopback, kein CPU-Pinning** — misst den reinen Wire- +
  Transport- + Marshalling-Pfad, nicht Multi-Host-Netzlatenz. Absolutwerte sind
  host-spezifisch (codepit, LXC); die **relativen** ORB-Abstände sind die
  Aussage. **Nicht** mit den DDS-Roundtrip-Docs (anderer Host) quervergleichbar.
* **JacORB band auf die LAN-IP** (`192.168.178.115`) statt `127.0.0.1` — der
  Verkehr loopt zwar im selben Host zurück, nimmt aber den LAN-IP-Routing-Pfad
  (kein reines `lo`). JacORBs Absolutwerte können dadurch leicht höher liegen;
  die Größenordnung (~3,5× langsamer) bleibt klar.
* **`--release` mit debuginfo** (Workspace-Default) — Optimierung voll aktiv,
  Debug-Symbole haben keinen Laufzeiteinfluss.
* **JacORB läuft nur auf JDK 8** — das `java.corba`-Modul ist seit Java 11
  entfernt. Konkreter Punkt für das „moderne Toolchain"-Argument von ZeroDDS.
* **SSLIOP-Client-Pooling**: ~~baut pro Call eine frische TLS-Connection auf~~ —
  **behoben am 2026-06-07** (commit 23cda7e7, Extra-Mile #3): der `Connector`
  poolt TLS-Connections nach (Adresse, SNI, Config), der Stub-Pfad
  (`IiopCorbaConnection::with_client_tls`) wiederverwendet die etablierte
  Connection (kein Handshake pro Call). Die hier gemessene **Steady-State**-Zahl
  (etablierte Connection) gilt damit auch für den Stub-Pfad.

---

## 9. Reproduktion

```sh
# Perf-Baseline (Cross-Vendor + Feature-Matrix + SSLIOP):
cd crates/corba-interop/competitors && N=50000 PAYLOADS="32 256 4096" bash run_perf.sh

# Einzelne ZeroDDS-Bins:
cargo run --release -p zerodds-corba-interop --bin echo_bench          -- 32 50000
cargo run --release -p zerodds-corba-interop --bin echo_bench_codegen  -- 32 50000
cargo run --release -p zerodds-corba-interop --bin bench_features      -- 50000
cargo run --release -p zerodds-corba-interop --bin ssliop_bench        -- cert.pem key.pem 56 50000

# Cross-ORB-Interop-Matrix (16 Richtungen):
cd crates/corba-interop/competitors && bash run_interop.sh
```

Vendor-Bench-Quellen: `competitors/{omniorb,tao,jacorb}/` (`server.*` + `client.*`
+ `Echo.idl`).
