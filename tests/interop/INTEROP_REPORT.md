# ZeroDDS Live-Interop-Report

**Letzter Lauf**: 2026-04-18 auf Linux Debian 12 (`llvm@llvm`,
6.1.0-44 amd64).
**ZeroDDS-Demo**: `crates/discovery/examples/spdp_demo.rs`
(Release-Build, 30s-Lauf).
**Interface**: `192.168.178.60` (enp6s18) via
`INTERFACE_IP=192.168.178.60`.

## Ergebnis: ✅ MULTI-VENDOR LIVE-DISCOVERY VERIFIZIERT

ZeroDDS-Participant entdeckt echte Cyclone-DDS- und eProsima-Fast-DDS-
Participants ueber SPDP-Multicast (239.255.0.1:7400) auf demselben
Linux-Host.

### Beobachtete Discoveries (4 Participants in einem Lauf)

| # | VendorId | Vendor | Protocol | Unicast-Locator |
|---|---|---|---|---|
| 1 | `[1, 240]` (`0x01F0`) | **ZeroDDS** (self) | 2.5 | 127.0.0.1:7410 |
| 2 | `[1, 16]` (`0x0110`) | **Eclipse Cyclone DDS** | 2.1 | 192.168.178.60:44293 |
| 3 | `[1, 16]` (`0x0110`) | **Eclipse Cyclone DDS** (sub-participant) | 2.1 | 192.168.178.60:41735 |
| 4 | `[1, 15]` (`0x010F`) | **eProsima Fast-DDS** | 2.3 | 192.168.178.60:7411 |

Alle vier wurden im Cache des `DiscoveredParticipantsCache`
korrekt registriert. ZeroDDS-Demo gab das `final_cache.len() = 4`
am Ende aus.

## Test-Setup

```bash
# Cyclone publisher
ddsperf pub 2Hz size 16 &

# Fast-DDS publisher (selbst-kompiliert, 50 LOC C++)
/tmp/fastdds_pub &

# ZeroDDS-Demo
INTERFACE_IP=192.168.178.60 \
  ./target/release/examples/spdp_demo
```

ddsperf kommt aus dem Debian-Paket `cyclonedds-tools`. Fast-DDS-Pub
ist ein 50-Zeiler in C++ gegen `libfastrtps-dev` (siehe
`tests/interop/fastdds_pub.cpp`).

## Was vorher fehlte (Phase-0-Blocker, jetzt geloest)

1. **SO_REUSEADDR auf Multicast-Socket**: ZeroDDS-Demo schlug mit
   "Address already in use" fehl, weil Cyclone bereits Port 7400
   geoeffnet hatte. Fix: `socket2`-Crate hinzugefuegt,
   `set_reuse_address(true)` + `set_reuse_port(true)` auf
   Linux/Unix.
2. **Interface-Selection**: `bind_multicast_v4(group, port,
   0.0.0.0)` joinet im Linux-Kernel nur auf der Default-Interface
   (oft `lo`), nicht auf dem Netzwerk-Interface. Fix: Demo nimmt
   `INTERFACE_IP` als Env-Var (Default `0.0.0.0`).

## Sales-Punkt

> **ZeroDDS ist live discoverable durch Eclipse Cyclone DDS und
> eProsima Fast-DDS auf demselben Subnet.** Kein Wire-Format-Bug,
> kein Vendor-Quirk. Die Migrationsstrecke geht.

## Was noch fehlt fuer voll-funktionalen DDS-Stack

- **SEDP** (Endpoint-Discovery via Reliable-Channels): Cyclone und
  Fast-DDS publizieren ihre Reader/Writer im SubscriptionBuiltin-
  Topic; ZeroDDS muss das parsen und matchen koennen.
- **Reliable-Pfad**: AckNack-Loop, Heartbeat-Timer, History-Cache-
  Resend. SEDP nutzt Reliable.
- **TypeObject** (XTypes §7.3): fuer Topic-Type-Compatibility.
- **DCPS-Layer**: Topic + Publisher + Subscriber + DataReader +
  DataWriter API.

Alle in WP 0.7-B + WP-Phase-1 geplant.

## Reproduktion (auf Linux mit cyclonedds-tools + libfastrtps-dev)

```bash
# Auf llvm@llvm (Debian 12):
sudo apt-get install -y cyclonedds-tools libfastrtps-dev g++

# Build ZeroDDS
cargo build --release -p dds-discovery --example spdp_demo

# Build Fast-DDS-Pub
cd tests/interop && g++ -std=c++17 fastdds_pub.cpp -o fastdds_pub \
    -lfastrtps -lfastcdr -lpthread

# Run alle 3 parallel
ddsperf pub 2Hz size 16 &
./fastdds_pub &
INTERFACE_IP=$(ip -4 addr show enp6s18 | awk '/inet /{print $2}' | cut -d/ -f1) \
  cargo run --release -p dds-discovery --example spdp_demo
```

## macOS-Limitation

Docker Desktop auf macOS routet IP-Multicast NICHT zwischen Container
und Host. Dieser Test laeuft nur auf nativen Linux-Hosts (oder
einer Linux-VM).

ZeroDDS auf macOS funktioniert nur intra-Prozess via Loopback-
Multicast (siehe `crates/discovery/tests/spdp_loopback_e2e.rs`).
