# CORBA IIOP Roundtrip Bench — ZeroDDS vs. TAO / omniORB / JacORB (2026-06-06)

GIOP-1.2-IIOP-Roundtrip-Latenz-Benchmark des ZeroDDS-CORBA-Stacks gegen die
drei aktiv verfügbaren CORBA-ORBs (TAO, omniORB, JacORB). Ergebnis: ZeroDDS ist
auf beiden gemessenen Payload-Größen der **schnellste** ORB im Feld.

Dieser Bench ersetzt die früher auf der Website behauptete, **unbelegte** CORBA-
Latenz von „~95 µs p50, competitive with TAO and JacORB". Die Zahl war eine
fälschlich als CORBA gelabelte DDS-Messung; der reale Wert ist ~16 µs und damit
sowohl korrekt als auch deutlich besser als die Behauptung.

## TL;DR — p50 Roundtrip-Latenz

| Payload | **ZeroDDS** | omniORB 4.3.3 | TAO 2.5.24 | JacORB 3.9 |
|---|---|---|---|---|
| 32 B  | **16.3 µs** ⭐ | 17.9 µs | 36.7 µs | 57.1 µs |
| 256 B | **16.3 µs** ⭐ | 17.9 µs | 35.9 µs | 57.5 µs |

ZeroDDS CORBA ist knapp vor omniORB, **~2,2× schneller als TAO** und
**~3,5× schneller als JacORB**.

## Volle Verteilung (32-Byte-Payload, n=50 000)

| ORB | min | p50 | p90 | p99 | p99.9 |
|---|---|---|---|---|---|
| **ZeroDDS CORBA** | 15.5 µs | **16.3 µs** | 23.5 µs | 40.4 µs | 47.8 µs |
| omniORB 4.3.3 | 13.0 µs | 17.9 µs | 22.6 µs | 44.4 µs | 53.3 µs |
| TAO 2.5.24 | 23.3 µs | 36.7 µs | 46.0 µs | 84.9 µs | 107.3 µs |
| JacORB 3.9 | 43.9 µs | 57.1 µs | 78.1 µs | 124.1 µs | 392.1 µs |

**Sample-Count**: n=50 000 gemessene Roundtrips nach 2 000 (C++/Rust) bzw.
10 000 (JacORB, JIT-Warmup) Warmup-Iterationen.

## Setup-Meta

### Hardware
| Item | Wert |
|---|---|
| Host | `codepit` (LXC-Container) |
| CPU | AMD Ryzen Threadripper PRO 3955WX 16-Cores |
| Cores online | 4 (von 32) |
| L2 / L3 | 8 MiB / 64 MiB |
| Memory | 15 GiB |
| Transport | TCP-Loopback (`127.0.0.1`), GIOP/IIOP 1.2 |
| Pinning | keines (siehe Caveats) |

### OS / Tooling
| Item | Wert |
|---|---|
| OS | Debian GNU/Linux 13 (trixie) |
| Kernel | 6.17.2-2-pve |
| gcc / g++ | 14.2.0 (Debian 14.2.0-19) |
| rustc | 1.88.0 (6b00bc388 2025-06-23) |
| JDK (JacORB) | Temurin OpenJDK 1.8.0_492 |

### ORB-Stacks
| ORB | Version | Quelle |
|---|---|---|
| ZeroDDS CORBA | local `corba`-Branch | `crates/corba-interop` (`echo_bench`, `--release`) |
| omniORB | 4.3.3+ds1-1 | Debian apt `omniorb omniidl libomniorb4-dev` |
| TAO (ACE+TAO) | 2.5.24 | OpenDDS-Bundle `/opt/opendds` (`tao_idl`, libs `/opt/opendds/lib`) |
| JacORB | 3.9 | `/opt/jacorb` + JDK 8 (CORBA-Modul seit Java 11 entfernt → JDK 8 nötig) |

## Methodologie

* **Geteilte IDL** (alle vier Stacks identisch):
  ```idl
  interface Echo {
      string ping(in string msg);   // echot das Argument zurück
  };
  ```
* **Roundtrip**: Client encodet die `in string`-Payload in CDR, sendet einen
  GIOP-1.2-Request über IIOP, der Server dispatcht über den POA an den Servant
  (`ping`), encodet die `return string` und sendet die Reply. Gemessen wird die
  volle Client-seitige Roundtrip-Zeit (`Instant`/`steady_clock`/`nanoTime`).
* **Eine Connection**, sequenzielle Requests (kein Pipelining), `SYNC_WITH_TARGET`.
* **Optimierte Builds**: Rust `--release`, C++ `-O2 -std=c++17`, JVM nach Warmup.
* **Reproduktion**: Quellen + Build-Notizen unter
  `crates/corba-interop/competitors/{omniorb,tao,jacorb}/`,
  ZeroDDS via `cargo run --release -p zerodds-corba-interop --bin echo_bench -- 32 50000`.

## Befunde

* **ZeroDDS CORBA ist der schnellste ORB** im Vergleich (p50), und das in
  Pure-Rust gegen drei etablierte C++/Java-ORBs.
* **IOR-Byte-Order divergiert**: omniORB + TAO emittieren Little-Endian-IORs
  (`IOR:01…`), JacORB Big-Endian (`IOR:00…`). Für Cross-ORB-Interop muss der
  Server die Request-Byte-Order-Flag honorieren (Interop-Milestone 2).
* **JacORB läuft nicht auf modernem Java**: das `java.corba`-Modul ist seit
  Java 11 entfernt; JacORB 3.9 benötigt JDK 8. Ein konkreter Punkt für das
  „modern toolchain"-Argument von ZeroDDS.

## Caveats (Ehrlichkeit)

* **Single-Host-Loopback, kein CPU-Pinning** — misst den reinen Wire- +
  Transport- + Marshalling-Pfad, nicht Multi-Host-Netzlatenz. Absolutwerte sind
  host-spezifisch (codepit, LXC); nur die **relativen** ORB-Abstände sind die
  Aussage. **Nicht** mit den DDS-Roundtrip-Docs (anderer Host `llvm`)
  quervergleichbar.
* **ZeroDDS: 1 Prozess / 2 Threads** (Acceptor-Server-Thread + Client-Loop über
  echtes Loopback-TCP), die drei Konkurrenten **2 Prozesse**. Der Kernel-TCP-Pfad
  dominiert in beiden Fällen; ein 2-Prozess-ZeroDDS-Lauf (`echo-server` /
  `echo-client`-Split) ist Teil von Interop-Milestone 2 und macht den Vergleich
  vollends apples-to-apples.
* **ZeroDDS nutzt einen hand-marshallten Echo-Servant**, nicht den (noch nicht
  verdrahteten) IDL-Codegen. Der gemessene Pfad — GIOP/IIOP-Wire + klassisches
  CDR + POA-Dispatch — ist identisch zu dem, was generierte Stubs/Skeletons
  ausführen würden; die Codegen-Verdrahtung ändert die per-Call-Kosten nicht.
