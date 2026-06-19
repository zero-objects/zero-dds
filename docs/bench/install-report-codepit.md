# Install-Report — codepit (frisches System, 2026-05-26)

Traceability-Doku für die bench-Installation. **Genau das**, was auf
codepit ausgeführt wurde, in der Reihenfolge in der es passierte.
Quelle für späteres reproduzierbares Vendor-Setup.

## Plattform-Baseline

```
Host:    codepit (Proxmox LXC, ZFS subvol-111-disk-0)
Distro:  Debian GNU/Linux 13 (trixie)
Kernel:  6.17.2-2-pve
CPU:     AMD Ryzen Threadripper PRO 3955WX, 4 Cores zugewiesen
RAM:     15 GiB
Disk:    250 GB free
```

Vor-Installiert (vom Distro-Image):
- `wget` 1.25.0
- `python3` 3.13.5

Alles andere wird unten dokumentiert installiert.

## Vendor-Policy

| Vendor | Quelle | Version | Status |
|---|---|---|---|
| ZeroDDS | Repo build | rc.3 git (5268d94e) | done |
| Cyclone DDS | source | 11.0.1 stable | done |
| eProsima Fast-DDS | source | v3.6.1 stable | done |
| RTI Connext | apt (packages.rti.com) | 7.7.0 LM | done |
| OpenDDS | source | v3.34.0 stable | done |

## Install-Schritte

### 0. System-Toolchain (Debian apt)

Befehl:
```
apt-get update
apt-get install -y --no-install-recommends \
    build-essential cmake git pkg-config curl ca-certificates \
    openssl libssl-dev libtinyxml2-dev \
    python3-dev python3-pip \
    openjdk-21-jdk-headless \
    ninja-build make
```

Zweck: Build-Toolchain für alle nachfolgenden source-builds. `tinyxml2`
ist Fast-DDS-dependency, `openssl-dev` für DDS-Security in mehreren
vendors, `openjdk-21` für `fastddsgen` (Java-tool von eProsima).

Ergebnis-Versionen (verifiziert nach Install):

| Tool | Version | Pfad |
|---|---|---|
| gcc | 14.2.0 (Debian 14.2.0-19) | /usr/bin/gcc |
| g++ | 14.2.0 (Debian 14.2.0-19) | /usr/bin/g++ |
| cmake | 3.31.6 | /usr/bin/cmake |
| git | 2.47.3 | /usr/bin/git |
| make | GNU Make 4.4.1 | /usr/bin/make |
| ninja | 1.12.1 | /usr/bin/ninja |
| pkg-config | 1.8.1 | /usr/bin/pkg-config |
| Java | OpenJDK 21.0.11 (Debian 21+10-1-deb13u2) | /usr/lib/jvm/java-21-openjdk-amd64 |
| Python | 3.13.5 | /usr/bin/python3 |
| OpenSSL | 3.5.6 (lib + headers) | /usr/lib/x86_64-linux-gnu |

Disk nach Toolchain: 1.8 GB used (von 250 GB).

### 1. Rust (rustup, stable)

Befehl:
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- --default-toolchain stable --profile minimal -y
. $HOME/.cargo/env
```

Profile `minimal` = rustc + cargo + std, kein clippy/rust-docs (für
Bench reicht das).

Verifiziert:
- `rustc 1.95.0 (59807616e 2026-04-14)` in `/root/.cargo/bin/rustc`
- `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` in `/root/.cargo/bin/cargo`

`.cargo/env` muss in jeder neuen Shell gesource'd werden, oder PATH
um `/root/.cargo/bin` erweitert.

### 2. ZeroDDS (Repo, cargo build --release)

Befehl:
```
mkdir -p /root/projects && cd /root/projects
git clone https://gitlab.sandra-kessler.eu/fishermen21/zerodds.git
cd zerodds
. /root/.cargo/env
cargo build --release -p zerodds-c-api
```

Verifiziert:
- Repo: `/root/projects/zerodds`
- Commit: `5268d94e` ("Revert: 5-Vendor ISO-Matrix M1+codepit mit OpenDDS"
  — letzter clean commit nach den Rollback heute)
- Build-Zeit: 44.17 s release-profile (4-Core Ryzen)
- Lib: `/root/projects/zerodds/target/release/libzerodds.so` (2.04 MB)
- Warnings: 1 harmlose `unused_assignments` in `runtime.rs:3206` (pt_t2_out),
  trifft kein behavior.

### 3. Eclipse Cyclone DDS 11.0.1 (C-Core + C++ binding)

Latest stable release. Zwei Repos: `cyclonedds` (C-Core + IDL-Compiler)
und `cyclonedds-cxx` (ISO-C++-PSM binding).

#### 3a. cyclonedds (C-Core)

Befehl:
```
mkdir -p /root/vendors
cd /root/vendors
git clone --depth 1 --branch 11.0.1 \
    https://github.com/eclipse-cyclonedds/cyclonedds.git cyclonedds
cd cyclonedds
mkdir build && cd build
cmake -DCMAKE_INSTALL_PREFIX=/opt/cyclone \
      -DCMAKE_BUILD_TYPE=Release \
      -DBUILD_IDLC=ON \
      -DBUILD_EXAMPLES=OFF \
      -DBUILD_TESTING=OFF \
      ..
cmake --build . --parallel 4
cmake --install .
```

Verifiziert:
- Source: `/root/vendors/cyclonedds` (commit `e54e991f`)
- Install: `/opt/cyclone/{lib,include,bin}`
- `libddsc.so.11.0.1` (C-Core lib)
- `libcycloneddsidl.so.11.0.1` (IDL-Compiler-lib)
- `bin/idlc` (IDL-Compiler-CLI)
- `bin/ddsperf` (Cyclone's eigener latency-bench tool)

#### 3b. cyclonedds-cxx (ISO-C++-PSM)

Befehl:
```
cd /root/vendors
git clone --depth 1 --branch 11.0.1 \
    https://github.com/eclipse-cyclonedds/cyclonedds-cxx.git cyclonedds-cxx
cd cyclonedds-cxx
mkdir build && cd build
cmake -DCMAKE_INSTALL_PREFIX=/opt/cyclone \
      -DCMAKE_PREFIX_PATH=/opt/cyclone \
      -DCMAKE_BUILD_TYPE=Release \
      -DBUILD_EXAMPLES=OFF \
      -DBUILD_TESTING=OFF \
      ..
cmake --build . --parallel 4
cmake --install .
```

`CMAKE_PREFIX_PATH=/opt/cyclone` damit der C++-Build die zuvor
installierte C-Core findet.

Verifiziert:
- Source: `/root/vendors/cyclonedds-cxx` (commit `20ccaa51`)
- Install: `/opt/cyclone/lib/libddscxx.so.11.0.1` (5 MB)
- Headers: `/opt/cyclone/include/ddscxx/...`

Disk nach Cyclone: 4.8 GB used.

### 4. eProsima Fast-DDS v3.6.1 (+ deps)

Latest stable. Drei Library-Komponenten + ein Java-Tool:

| Komponent | Version | Source |
|---|---|---|
| foonathan-memory | v0.7-4 | https://github.com/foonathan/memory |
| Fast-CDR | v2.3.5 | https://github.com/eProsima/Fast-CDR |
| Fast-DDS | v3.6.1 | https://github.com/eProsima/Fast-DDS |
| Fast-DDS-Gen | v4.3.0 | https://github.com/eProsima/Fast-DDS-Gen |

Asio (apt) als zusaetzliche dep:
```
apt-get install -y libasio-dev      # 1.30.2-1
```

#### 4a. foonathan-memory
```
cd /root/vendors
git clone --depth 1 --branch v0.7-4 \
    https://github.com/foonathan/memory.git foonathan-memory
cd foonathan-memory && mkdir build && cd build
cmake -DCMAKE_INSTALL_PREFIX=/opt/fastdds \
      -DCMAKE_BUILD_TYPE=Release \
      -DFOONATHAN_MEMORY_BUILD_EXAMPLES=OFF \
      -DFOONATHAN_MEMORY_BUILD_TESTS=OFF \
      -DFOONATHAN_MEMORY_BUILD_TOOLS=OFF ..
cmake --build . --parallel 4 && cmake --install .
```
Ergebnis: `/opt/fastdds/lib/libfoonathan_memory-0.7.4.a` (static).

#### 4b. Fast-CDR
```
cd /root/vendors
git clone --depth 1 --branch v2.3.5 \
    https://github.com/eProsima/Fast-CDR.git fast-cdr
cd fast-cdr && mkdir build && cd build
cmake -DCMAKE_INSTALL_PREFIX=/opt/fastdds \
      -DCMAKE_BUILD_TYPE=Release \
      -DBUILD_TESTING=OFF ..
cmake --build . --parallel 4 && cmake --install .
```
Ergebnis: `/opt/fastdds/lib/libfastcdr.so.2.3.5`.

#### 4c. Fast-DDS
```
cd /root/vendors
git clone --depth 1 --branch v3.6.1 \
    https://github.com/eProsima/Fast-DDS.git fast-dds
cd fast-dds && mkdir build && cd build
cmake -DCMAKE_INSTALL_PREFIX=/opt/fastdds \
      -DCMAKE_PREFIX_PATH=/opt/fastdds \
      -DCMAKE_BUILD_TYPE=Release \
      -DCOMPILE_EXAMPLES=OFF \
      -DSECURITY=OFF \
      -DBUILD_TESTING=OFF ..
cmake --build . --parallel 4 && cmake --install .
```
Ergebnis: `/opt/fastdds/lib/libfastdds.so.3.6.1.0`.

#### 4d. Fast-DDS-Gen (Java-Tool)
```
cd /root/vendors
git clone --depth 1 --branch v4.3.0 \
    https://github.com/eProsima/Fast-DDS-Gen.git fastddsgen
cd fastddsgen
git submodule update --init --recursive
./gradlew assemble
```
Ergebnis: `/root/vendors/fastddsgen/scripts/fastddsgen` (wrapper) +
`/root/vendors/fastddsgen/build/libs/fastddsgen.jar`.
Verifiziert: `fastddsgen -version` → "fastddsgen version 4.3.0".

Disk nach Fast-DDS-Stack: 5.0 GB used.

### 5. RTI Connext DDS 7.7.0 LM (apt-Repo)

Offizieller RTI apt-Repo (Doku: `community.rti.com/.../get-started/apt-install.html`).
Verfügbare Codenames im Repo: `bookworm`, `bullseye`, `jammy`, `focal`,
`noble`. **Kein** `trixie` für Debian 13 — wir nehmen `bookworm` als
Fallback (glibc 2.36 → trixie 2.41 ist ABI-rückwärts-kompatibel).

#### 5a. Repo-Setup
```
curl -sSL -o /usr/share/keyrings/rti-official-archive.gpg \
    https://packages.rti.com/deb/official/repo.key

echo "deb [arch=amd64 signed-by=/usr/share/keyrings/rti-official-archive.gpg] \
    https://packages.rti.com/deb/official bookworm main" \
    > /etc/apt/sources.list.d/rti-official.list

apt-get update
```

#### 5b. EULA-Preseed + Install

RTI-preinst `rti-connext-dds-7.7.0-common` failt non-interactive ohne
debconf-preseed (template-key `rti-connext-dds-7.7.0/license/accepted`):
```
echo "rti-connext-dds-7.7.0-common rti-connext-dds-7.7.0/license/accepted boolean true" \
    | debconf-set-selections
DEBIAN_FRONTEND=noninteractive \
    apt-get install -y --no-install-recommends rti-connext-dds-7.7.0
```

#### 5c. License platzieren
```
scp ~/Downloads/rti_license.dat root@codepit:/opt/rti.com/rti_connext_dds-7.7.0/
```

Verifiziert:
- Install: `/opt/rti.com/rti_connext_dds-7.7.0/`
- Disk-Footprint: 4.2 GB (apt-Pkts: `rti-connext-dds-7.7.0-{common,
  lib-amd64,env-amd64,doc,jre,tools-*}`)
- Lib-Arch: `lib/x64Linux4gcc8.5.0/`
- Lizenz: `rti_license.dat`
- Smoke: `NDDSHOME=$RTI LD_LIBRARY_PATH=$RTI/lib/x64Linux4gcc8.5.0
  RTI_LICENSE_FILE=$RTI/rti_license.dat $RTI/bin/rtiddsgen -version`
  → `rtiddsgen version 4.7.0`, templates `8CD7-D2FE-82AF-6A65-CF1B-BA5A-0768-5343`

### 6. OpenDDS v3.34.0

Wie auf M1: source-build aus GitHub, configure + GNU make:
```
cd /root/vendors
git clone --depth 1 --branch v3.34.0 https://github.com/OpenDDS/OpenDDS.git opendds
cd opendds
./configure --prefix=/opt/opendds --no-tests --no-debug --optimize
source setenv.sh
make -j 4
make install
```

Verifiziert (Build: **15min 16s** auf 4-Core codepit, exit 0):
- Source: `/root/vendors/opendds` (commit `1a9f3cc`, OpenDDS Release 3.34.0)
- Install: `/opt/opendds/{bin,lib,include,share}`
- Lib: `/opt/opendds/lib/libOpenDDS_Dcps.so.3.34.0` (20 MB) +
  alle weiteren `libOpenDDS_*.so.3.34.0` + `libACE.so` + `libTAO*.so`
- Bin: `opendds_idl`, `DCPSInfoRepo`, `tao_idl`, `ace_gperf`, `inspect`
- DDS_ROOT-Pfad: `/opt/opendds/share/dds` (Standard-Layout der Templates)

## Final Disk-Usage (codepit, alle 5 Vendors installed)

```
9.2M    /opt/cyclone     (Cyclone DDS install)
22M     /opt/fastdds     (Fast-DDS install incl. fastcdr+foonathan)
97M     /opt/opendds     (OpenDDS install)
4.2G    /opt/rti.com     (RTI Connext LM via apt)
---
856M    /root/vendors    (Sources cyclonedds + fast-dds + opendds + fastddsgen)
349M    /root/projects   (ZeroDDS source + target/release)
---
Total /: 12 GB used of 250 GB (5%)
```

## Env-Setup Cheat-Sheet pro Vendor (codepit, Linux)

| Vendor | NDDSHOME/Root | LD_LIBRARY_PATH | Note |
|---|---|---|---|
| ZeroDDS | — | `/root/projects/zerodds/target/release` | — |
| Cyclone | — | `/opt/cyclone/lib` | — |
| Fast-DDS | — | `/opt/fastdds/lib` | — |
| RTI | `NDDSHOME=/opt/rti.com/rti_connext_dds-7.7.0` + `RTI_LICENSE_FILE=$NDDSHOME/rti_license.dat` | `$NDDSHOME/lib/x64Linux4gcc8.5.0` | apt-installed, kein tarball |
| OpenDDS | `DDS_ROOT=ACE_ROOT=TAO_ROOT=/opt/opendds/share/dds` | `/opt/opendds/lib` | non-Standard prefix-layout |

## Plattform-Patches (KEINE vendor-source-Mods)

1. **RTI apt: codename trixie nicht supported** → bookworm-Pkts (Debian 12)
   auf trixie installiert; glibc-ABI-Kompatibilität gegeben.

2. **RTI debconf-preseed** nötig für non-interactive EULA-Acceptance
   (template-key `rti-connext-dds-7.7.0/license/accepted`).

### 7. Iceoryx 2.0.6 + Cyclone-PSMX-iox (für SHM-Transport)

Cyclone DDS 11.x hat SharedMemory-deprecated, neues System ist
**PSMX (Pluggable Shared Memory eXchange)** mit iceoryx als Default-
Plugin. Daher iceoryx-Daemon + Cyclone-Rebuild mit `ENABLE_ICEORYX`.

```
apt-get install -y iceoryx libiceoryx-binding-c-dev libiceoryx-binding-c2 \
                   libiceoryx-hoofs-dev libiceoryx-posh-dev
# → /usr/bin/iox-roudi
```

Cyclone-Rebuild mit Iceoryx-Support:
```
cd /root/vendors/cyclonedds
rm -rf build && mkdir build && cd build
cmake -G Ninja -DCMAKE_INSTALL_PREFIX=/opt/cyclone -DCMAKE_BUILD_TYPE=Release \
      -DBUILD_IDLC=ON -DBUILD_EXAMPLES=OFF -DBUILD_TESTING=OFF \
      -DENABLE_ICEORYX=ON ..
ninja -j 4 && ninja install
# → /opt/cyclone/lib/libpsmx_iox.so.11.0.1
```

Cyclone-SHM-Bench-Config (XML, via `CYCLONEDDS_URI`):
```xml
<CycloneDDS xmlns="https://cdds.io/config">
  <Domain>
    <General>
      <Interfaces>
        <PubSubMessageExchange type="iox" library="psmx_iox" priority="1000000"/>
      </Interfaces>
    </General>
  </Domain>
</CycloneDDS>
```

Vor jedem SHM-Bench `iox-roudi` als Daemon starten:
```
iox-roudi > /tmp/iox.log 2>&1 &
```

Verifiziert: Cyclone-iceoryx self-roundtrip p50=19µs (vs UDPv4 36µs = -47%).


