# Install-Report — m1-new (frisches System, 2026-05-27)

Traceability-Doku für die bench-Installation auf dem zweiten M1.
**Genau das**, was auf `dev@192.168.178.192` ausgeführt wurde, in der
Reihenfolge in der es passierte. Schwester-Doku zu
[install-report-codepit.md](install-report-codepit.md).

## Plattform-Baseline

```
Host:    m1-new (dev@192.168.178.192, frisch aufgesetzt)
OS:      macOS 26.5 (build 25F71, "Tahoe")
Kernel:  Darwin
CPU:     Apple M1, 8 Cores (arm64)
RAM:     8 GiB
Disk:    228 GB, 195 GB free
Shell:   zsh
User:    dev (uid=501, admin via sudo)
```

Vor-Installiert (vom macOS-Image):
- `/usr/bin/clang`, `/usr/bin/gcc`, `/usr/bin/g++` (Stubs, brauchen CLT)
- `/usr/bin/git` 2.50.1 (Apple Git-155)
- `/usr/bin/python3` 3.9.6 (System)
- `/usr/bin/java` (Stub, ohne JDK)
- `/usr/bin/make` GNU Make 3.81 (Apple uralt)

Alles andere wird unten dokumentiert installiert.

## Vendor-Policy

| Vendor | Quelle | Version | Status |
|---|---|---|---|
| ZeroDDS | Repo build | rc.3 git | done |
| Cyclone DDS | source | 11.0.1 stable | done |
| eProsima Fast-DDS | source | v3.6.1 stable | done |
| RTI Connext | DMG (LM/Eval) | 7.7.0 | done |
| OpenDDS | source | v3.34.0 stable | done |

## Install-Schritte

### 0. Xcode Command Line Tools (CLT)

Brauchen wir vor allem anderen — System-clang/git sind Stubs ohne CLT.
Non-interactive Install über `softwareupdate` (statt GUI-popup von
`xcode-select --install`):

```
sudo touch /tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress
softwareupdate -i 'Command Line Tools for Xcode 26.5-26.5' --no-scan
```

Verifiziert:
- `xcode-select -p` → `/Library/Developer/CommandLineTools`
- `clang --version` → `Apple clang version 21.0.0 (clang-2100.1.1.101)`

### 1. Homebrew (arm64-native, /opt/homebrew)

Standard-Install ist GUI-interactive (`sudo` askpass). Workaround via
askpass-Wrapper für non-interactive ssh:

```
cat > /tmp/askpass.sh <<'EOF'
#!/bin/sh
echo devdev
EOF
chmod 700 /tmp/askpass.sh
SUDO_ASKPASS=/tmp/askpass.sh NONINTERACTIVE=1 /bin/bash -c \
  "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

`~/.zprofile` extended um:
```
eval "$(/opt/homebrew/bin/brew shellenv zsh)"
```

Verifiziert: `brew --version` → `Homebrew 5.1.14`.

### 2. brew-Toolchain (cmake/ninja/openjdk/openssl/asio/tinyxml2)

Für vendor-source-builds:

```
brew install cmake ninja pkg-config openssl@3 asio tinyxml2 openjdk openjdk@21
```

| Tool | Version | Pfad |
|---|---|---|
| cmake | 4.3.2 | /opt/homebrew/bin/cmake |
| ninja | 1.13.2 | /opt/homebrew/bin/ninja |
| pkg-config | (latest) | /opt/homebrew/bin/pkg-config |
| openssl@3 | 3.6 | /opt/homebrew/opt/openssl@3 |
| asio | (latest) | /opt/homebrew/include |
| tinyxml2 | (latest) | /opt/homebrew |
| openjdk | 26.0.1 | /opt/homebrew/opt/openjdk |
| openjdk@21 | 21.0.11 | /opt/homebrew/opt/openjdk@21 |

**Warum 2 JDKs:** fastddsgen 4.3.0 verwendet Gradle 9.2.1 das nur bis
Java 21 supportet — `BUG! exception in phase 'semantic analysis' in
source unit '_BuildScript_' Unsupported class file major version 70`
mit Java 26. JDK 21 als parallel-keg, im PATH vorgereiht.

PATH-Setup für non-interactive ssh-shells (in `~/.zshrc` + `~/.zprofile`):
```
export PATH="/opt/homebrew/opt/openjdk@21/bin:/opt/homebrew/bin:$HOME/.cargo/bin:$PATH"
```

### 3. Rust (rustup, stable)

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- --default-toolchain stable --profile minimal -y
```

Verifiziert:
- `rustc 1.95.0 (59807616e 2026-04-14)` in `~/.cargo/bin/rustc`
- `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` in `~/.cargo/bin/cargo`

### 4. ZeroDDS (Repo, cargo build --release)

Clone via GitLab-PAT (Memory: `reference_gitlab_token.md`):

```
mkdir -p ~/projects && cd ~/projects
git clone 'https://claude_all:<PAT>@gitlab.sandra-kessler.eu/fishermen21/zerodds.git'
cd zerodds
cargo build --release -p zerodds-c-api
```

Verifiziert:
- Repo: `~/projects/zerodds`
- Commit: `5268d94e` ("Revert: 5-Vendor ISO-Matrix M1+codepit mit OpenDDS"
  — derselbe wie codepit, gleicher Stand)
- Build-Zeit: 52s (M1, 8-Core)
- Lib: `~/projects/zerodds/target/release/libzerodds.dylib` (1.7 MB)
- 1 harmlose `unused_assignments`-Warning (runtime.rs:3206)

### 5. Eclipse Cyclone DDS 11.0.1 (C-Core + C++ binding)

Latest stable. Wie auf codepit. Zwei Repos.

#### 5a. cyclonedds (C-Core)
```
mkdir -p ~/vendors && cd ~/vendors
git clone --depth 1 --branch 11.0.1 \
    https://github.com/eclipse-cyclonedds/cyclonedds.git cyclonedds
cd cyclonedds && mkdir build && cd build
cmake -G Ninja \
      -DCMAKE_INSTALL_PREFIX=$HOME/opt/cyclone \
      -DCMAKE_BUILD_TYPE=Release \
      -DBUILD_IDLC=ON -DBUILD_EXAMPLES=OFF -DBUILD_TESTING=OFF ..
ninja -j 8 && ninja install
```

Verifiziert (Build: 8s):
- Source: `~/vendors/cyclonedds` (commit `e54e991f`)
- Install: `~/opt/cyclone/{lib,include,bin}`
- `libddsc.11.0.1.dylib` (1.4 MB)
- `bin/idlc`, `bin/ddsperf`

#### 5b. cyclonedds-cxx (ISO-C++-PSM)
```
cd ~/vendors
git clone --depth 1 --branch 11.0.1 \
    https://github.com/eclipse-cyclonedds/cyclonedds-cxx.git cyclonedds-cxx
cd cyclonedds-cxx && mkdir build && cd build
cmake -G Ninja \
      -DCMAKE_INSTALL_PREFIX=$HOME/opt/cyclone \
      -DCMAKE_PREFIX_PATH=$HOME/opt/cyclone \
      -DCMAKE_BUILD_TYPE=Release \
      -DBUILD_EXAMPLES=OFF -DBUILD_TESTING=OFF ..
ninja -j 8 && ninja install
```

Verifiziert (Build: 6s):
- Source: `~/vendors/cyclonedds-cxx` (commit `20ccaa51`)
- Install: `~/opt/cyclone/lib/libddscxx.11.0.1.dylib` (879 KB)

### 6. eProsima Fast-DDS v3.6.1 (+ deps)

Wie codepit: foonathan-memory → fast-cdr → fast-dds → fastddsgen.

#### 6a. foonathan-memory v0.7-4
```
cd ~/vendors
git clone --depth 1 --branch v0.7-4 \
    https://github.com/foonathan/memory.git foonathan-memory
cd foonathan-memory && mkdir build && cd build
cmake -G Ninja -DCMAKE_INSTALL_PREFIX=$HOME/opt/fastdds \
      -DCMAKE_BUILD_TYPE=Release \
      -DFOONATHAN_MEMORY_BUILD_EXAMPLES=OFF \
      -DFOONATHAN_MEMORY_BUILD_TESTS=OFF \
      -DFOONATHAN_MEMORY_BUILD_TOOLS=OFF ..
ninja -j 8 && ninja install
```
Ergebnis: `~/opt/fastdds/lib/libfoonathan_memory-0.7.4.a` (static).

#### 6b. Fast-CDR v2.3.5
```
cd ~/vendors
git clone --depth 1 --branch v2.3.5 \
    https://github.com/eProsima/Fast-CDR.git fast-cdr
cd fast-cdr && mkdir build && cd build
cmake -G Ninja -DCMAKE_INSTALL_PREFIX=$HOME/opt/fastdds \
      -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=OFF ..
ninja -j 8 && ninja install
```
Ergebnis: `~/opt/fastdds/lib/libfastcdr.2.3.5.dylib` (124 KB).

#### 6c. Fast-DDS v3.6.1

Apple clang 21 ist strenger als gcc: `-Werror -Wnonnull` failt im
upstream-Code (`TypeObjectRegistry.cpp:2620` — `!value.empty() ? value : 0`
passt 0 als nonnull-arg). Workaround per **Build-Flag** (kein
vendor-source-patch — verboten lt. `feedback_never_modify_vendor_binaries`):

```
cd ~/vendors
git clone --depth 1 --branch v3.6.1 \
    https://github.com/eProsima/Fast-DDS.git fast-dds
cd fast-dds && mkdir build && cd build
cmake -G Ninja \
      -DCMAKE_INSTALL_PREFIX=$HOME/opt/fastdds \
      -DCMAKE_PREFIX_PATH="$HOME/opt/fastdds;/opt/homebrew" \
      -DCMAKE_BUILD_TYPE=Release \
      -DCOMPILE_EXAMPLES=OFF -DSECURITY=OFF -DBUILD_TESTING=OFF \
      -DOPENSSL_ROOT_DIR=/opt/homebrew/opt/openssl@3 \
      -DCMAKE_CXX_FLAGS='-Wno-error=nonnull -Wno-error=deprecated-declarations' ..
ninja -j 8 && ninja install
```

Verifiziert (Build: 1m11s):
- Source: `~/vendors/fast-dds` (commit `4e81e8b`)
- Install: `~/opt/fastdds/lib/libfastdds.3.6.1.0.dylib` (11 MB)

#### 6d. Fast-DDS-Gen v4.3.0 (Java-Tool)

Braucht **JDK 21** (siehe Schritt 2):
```
cd ~/vendors
git clone --depth 1 --branch v4.3.0 \
    https://github.com/eProsima/Fast-DDS-Gen.git fastddsgen
cd fastddsgen
git submodule update --init --recursive
PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew assemble --no-daemon
```

Verifiziert (Build: 12s):
- `~/vendors/fastddsgen/scripts/fastddsgen -version` → `fastddsgen version 4.3.0`

### 7. RTI Connext DDS 7.7.0 (LM/Eval)

DMG-Installer von RTI ist **Launchpad/LM-Bundle** — kein `unattended`
mode. Workaround: `--mode text` mit `yes y` für EULA-defaults.

```
# DMG + Lizenz von lokal:
scp ~/Downloads/rti_connext_dds-7.7.0-lm-arm64Darwin23clang16.0.dmg \
    dev@192.168.178.192:~/Downloads/
scp ~/Downloads/rti_license.dat dev@192.168.178.192:~/Downloads/

# Auf m1-new:
hdiutil attach -nobrowse ~/Downloads/rti_connext_dds-7.7.0-lm-arm64Darwin23clang16.0.dmg
ln -sf "/Volumes/RTI Connext DDS LM 7.7.0" /tmp/rti_dmg
INSTALLER=/tmp/rti_dmg/rti_connext_dds-7.7.0-lm-arm64Darwin23clang16.0.app/Contents/MacOS/installbuilder.sh
mkdir -p ~/y
yes y | "$INSTALLER" --mode text --prefix "$HOME/y" --disable_copy_examples true
cp ~/Downloads/rti_license.dat ~/y/rti_connext_dds-7.7.0/

# macOS quarantine clearing (DMG-extracted Files sind quarantined by default):
sudo xattr -dr com.apple.quarantine ~/y/rti_connext_dds-7.7.0
```

**Wichtig:** `sudo xattr -dr com.apple.quarantine` ist KEIN
vendor-binary-patch — wir setzen nur das macOS-Gatekeeper-XATTR, das
darauf signalisiert "Internet-downloaded". Die binaries selbst werden
nicht angefasst.

Verifiziert:
- Install: `~/y/rti_connext_dds-7.7.0/`
- Lizenz: `~/y/rti_connext_dds-7.7.0/rti_license.dat`
- Lib: `~/y/rti_connext_dds-7.7.0/lib/arm64Darwin23clang16.0/`
- `bin/rtiddsgen -version` → `rtiddsgen version 4.7.0`,
  Templates `8CD7-D2FE-82AF-6A65-CF1B-BA5A-0768-5343`

### 8. OpenDDS v3.34.0

```
cd ~/vendors
git clone --depth 1 --branch v3.34.0 https://github.com/OpenDDS/OpenDDS.git opendds
cd opendds
./configure --prefix=$HOME/opt/opendds --no-tests --no-debug --optimize
source setenv.sh
make -j 8
make install
```

Configure dauert ~18s, generiert ACE_wrappers + TAO + OpenDDS GNUmakefiles
in-tree. `make` baut alles sequenziell: erst ACE, dann TAO, dann
OpenDDS-DCPS — kein parallel-targets über die Layer hinaus.

Verifiziert (Build: **38min** auf 8-Core M1, exit 0):
- Source: `~/vendors/opendds` (commit `1a9f3cc` — OpenDDS Release 3.34.0)
- Install-prefix: `~/opt/opendds/`
- Lib: `~/opt/opendds/lib/libOpenDDS_Dcps.dylib` (27 MB) + 30+ weitere
  `libOpenDDS_*.dylib` + `libACE.dylib` + `libTAO*.dylib`
- Bin: `bin/opendds_idl`, `bin/DCPSInfoRepo`, `bin/tao_idl`, `bin/ace_gperf`
- Templates: `share/dds/dds/idl/IDLTemplate.txt` (DDS_ROOT must be set
  to `~/opt/opendds/share/dds` für `opendds_idl`)

Smoke-Test:
```
DDS_ROOT=~/opt/opendds/share/dds ACE_ROOT=~/opt/opendds/share/dds \
TAO_ROOT=~/opt/opendds/share/dds DYLD_LIBRARY_PATH=~/opt/opendds/lib \
~/opt/opendds/bin/opendds_idl -o /tmp /tmp/test.idl
# → testTypeSupport.idl + testTypeSupportImpl.{cpp,h} generated
```

## Final Disk-Usage

```
7.3M    ~/opt/cyclone           (Cyclone DDS install)
 16M    ~/opt/fastdds           (Fast-DDS install incl. fastcdr+foonathan)
110M    ~/opt/opendds           (OpenDDS install)
2.8G    ~/y/rti_connext_dds-7.7.0 (RTI Connext LM)
---
Sources (vendors/, can be deleted after benchmarking):
 43M    ~/vendors/cyclonedds
8.8M    ~/vendors/cyclonedds-cxx
 11M    ~/vendors/fast-cdr
122M    ~/vendors/fast-dds
 63M    ~/vendors/fastddsgen
536M    ~/vendors/opendds       (~/vendors/opendds = ACE+TAO source+build)
---
Total ~/ disk: ~24 GB used of 228 GB (12%)
```

## Env-Setup Cheat-Sheet pro Vendor

| Vendor | NDDSHOME/Root | DYLD_LIBRARY_PATH | Note |
|---|---|---|---|
| ZeroDDS | — | `~/projects/zerodds/target/release` | — |
| Cyclone | — | `~/opt/cyclone/lib` | — |
| Fast-DDS | — | `~/opt/fastdds/lib` | — |
| RTI | `NDDSHOME=~/y/rti_connext_dds-7.7.0` + `RTI_LICENSE_FILE=$NDDSHOME/rti_license.dat` | `$NDDSHOME/lib/arm64Darwin23clang16.0` | — |
| OpenDDS | `DDS_ROOT=ACE_ROOT=TAO_ROOT=~/opt/opendds/share/dds` | `~/opt/opendds/lib` | non-Standard prefix-layout |

## Bekannte Plattform-Patches (KEINE vendor-source-Mods)

1. **Apple clang 21 + `-Werror -Wnonnull`** trifft Fast-DDS upstream
   `TypeObjectRegistry.cpp:2620` → Workaround: `-DCMAKE_CXX_FLAGS=
   '-Wno-error=nonnull -Wno-error=deprecated-declarations'` beim
   Fast-DDS-cmake-configure.

2. **Gradle 9.2.1 max Java 21** vs brew openjdk@26 → zweite JDK-Keg
   `openjdk@21` parallel installiert, im PATH vor `openjdk` gesetzt.

3. **macOS Quarantine** auf DMG-extracted RTI-Files →
   `sudo xattr -dr com.apple.quarantine ~/y/rti_connext_dds-7.7.0`
   (clear XATTR, **NICHT** vendor-binary-patch).
