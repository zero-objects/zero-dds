# Packaging Guide

**Stand:** 2026-05-03
**Sprint-Bezug:** Phase-5 E.2.

ZeroDDS wird als nativer Installer auf vier Ziel-Plattformen
ausgeliefert: Debian/Ubuntu, RHEL/Fedora, Windows, macOS. Diese
Doku beschreibt die Build-Wege und welche Tools auf jedem Ziel
notwendig sind.

```
                 ┌──────────────────┐
                 │  cargo workspace │
                 │  build --release │
                 └─────────┬────────┘
        ┌──────────┬───────┴────────┬──────────┐
        ▼          ▼                ▼          ▼
   pkg/debian   pkg/rpm     pkg/windows   pkg/macos
   (.deb x4)    (.rpm x4)     (.msi)      (.pkg /
                                          Homebrew)
```

Vier Binary-Pakete pro Linux-Distro:

| Paket              | Inhalt                                     |
|--------------------|--------------------------------------------|
| `zerodds-tools`    | CLI: dds-{admin,perf,idlc,xmlc,chaos,…}    |
| `libzerodds-dev`   | C/C++ Headers + import-libs                |
| `libzerodds0` (*)  | runtime SO/DyLib                           |
| `librmw-zerodds`   | ROS-2 RMW Plugin (`librmw_zerodds.so`)     |

(*) RHEL nennt es `zerodds-libs`, macOS legt nur `libzerodds.dylib`
ohne Soname-Versionierung an.

---

## 1. Debian / Ubuntu (.deb)

**Voraussetzungen** auf dem Build-Host:

```bash
sudo apt install build-essential debhelper dh-cargo cargo rustc \
                 libssl-dev pkg-config
```

**Build:**

```bash
cd zerodds-source/
cp -r pkg/debian debian       # `debian/` muss im Source-Root liegen
dpkg-buildpackage -us -uc -b
ls ../zerodds-tools_*.deb ../libzerodds*_*.deb ../librmw-zerodds_*.deb
```

**Repo-Hosting (apt.zerodds.org):** Der Setup-Schritt liegt
ausserhalb dieser Crate. Empfehlung: `reprepro` mit GPG-Sign,
gehostet auf einem statischen S3-Bucket / GitLab-Pages.

**Sub-Paket-Splits** sind in `pkg/debian/control` definiert; die
Install-Regeln in `pkg/debian/rules` (manuelles `dh_install` weil
`dh-cargo` mit Multi-Binary-Workspaces fragil ist).

---

## 2. RHEL / Fedora / openSUSE (.rpm)

**Voraussetzungen:**

```bash
sudo dnf install rpm-build cargo rust openssl-devel pkgconfig \
                 rpmdevtools
```

**Build:**

```bash
rpmdev-setuptree
cp pkg/rpm/zerodds.spec ~/rpmbuild/SPECS/
git archive --format=tar.gz --prefix=zerodds-0.0.0/ HEAD \
  > ~/rpmbuild/SOURCES/zerodds-0.0.0.tar.gz
rpmbuild -ba ~/rpmbuild/SPECS/zerodds.spec
ls ~/rpmbuild/RPMS/x86_64/zerodds-*.rpm
```

**copr / openSUSE Build Service:** der Spec-File ist OBS-kompatibel;
einfach hochladen, und sowohl Fedora-Versionen (35..) als auch
SUSE-Distros bauen automatisch.

**Repo-Hosting (yum.zerodds.org):** `createrepo_c` + GPG-Sign,
analog zu Debian.

---

## 3. Windows (.msi)

**Voraussetzungen** auf der Build-Maschine:

```powershell
# Rust toolchain (msvc-target)
rustup target add x86_64-pc-windows-msvc

# .NET 8 SDK + WiX 5
winget install Microsoft.DotNet.SDK.8
dotnet tool install -g wix --version 5.0.*
```

**Build:**

```powershell
pwsh -File pkg/windows/build.ps1 -Configuration Release
# Output: dist/windows/zerodds-0.0.0-x64.msi
```

**Code-Signing** mit signtool (PowerShell, mit Cert im Cert-Store):

```powershell
pwsh -File pkg/windows/build.ps1 -Sign $true -CertSubject "ZeroDDS Maintainers"
```

**Was der Installer macht:**

- `%ProgramFiles%\ZeroDDS\bin\` → CLI-Binaries
- `%ProgramFiles%\ZeroDDS\lib\` → `zerodds.dll` + import-lib
- `%ProgramFiles%\ZeroDDS\include\` → `zerodds.h` (optional Feature)
- HKCU-PATH-Eintrag fuer `bin\` (User-Scope, kein UAC-Prompt)

**Microsoft Store (MSIX):** das WiX-MSI ist nicht direkt
MSIX-konvertierbar; die MSIX-Variante kommt mit
`MSIX Packaging Tool`. Schritte siehe Microsoft-Doku — out of
scope dieser Crate.

---

## 4. macOS (.pkg / Homebrew)

### Homebrew (empfohlener Pfad fuer Endnutzer)

**Tap:**

```bash
brew tap zerodds/zerodds https://gitlab.sandra-kessler.eu/fishermen21/homebrew-zerodds
brew install zerodds
```

**Formel:** `pkg/macos/Formula/zerodds.rb` (wird in das Tap-Repo
gespiegelt nach jedem Release).

**Maintainer-Workflow:**

1. `git tag v0.0.X && git push origin v0.0.X`
2. SHA256 vom Tag-Tarball berechnen.
3. `url` und `sha256` in der Formula aktualisieren.
4. Formula in `homebrew-zerodds`-Repo pushen.
5. `brew test zerodds` lokal — wenn gruen, Submission an
   homebrew-core (optional, fuer den brew-Default-Tap).

### .pkg-Installer (alternative, fuer Enterprise-Distribution)

**Voraussetzungen:**

- Xcode-CLI-Tools (`xcode-select --install`)
- Optional Apple-Developer-ID-Cert + App-spezifisches PWD im
  Keychain unter Profile-Name `notarytool-zerodds`

**Build:**

```bash
VERSION=0.0.0 ./pkg/macos/build_pkg.sh
# Output: dist/macos/zerodds-0.0.0.pkg
```

**Mit Code-Signing + Notarisierung:**

```bash
DEVELOPER_ID="Developer ID Installer: ZeroDDS Maintainers (ABCDE12345)" \
NOTARYTOOL_KEYCHAIN_PROFILE="notarytool-zerodds" \
VERSION=0.0.0 ./pkg/macos/build_pkg.sh
```

Das Skript baut `aarch64-apple-darwin` + `x86_64-apple-darwin` und
fasst beide via `lipo` zu Universal-Binaries zusammen.

**Was der .pkg installiert:**

- `/usr/local/bin/dds-{admin,perf,…}`
- `/usr/local/lib/libzerodds.dylib`
- `/usr/local/include/zerodds/zerodds.h`

---

## 5. Cross-Platform-Versionierung

Alle vier Pakete tragen denselben `0.0.0`-String aus
`Cargo.toml` workspace-package-Version. Bumps via
`cargo workspaces version`:

```bash
cargo install cargo-workspaces
cargo workspaces version --all minor
```

**Folge-Steps pro Release:**

1. `git tag vX.Y.Z`
2. CI baut alle vier Pakete (linux/.deb, linux/.rpm, win/.msi, mac/.pkg)
3. Hochladen in:
   - GitLab Releases (alle Pakete)
   - apt.zerodds.org (.deb)
   - yum.zerodds.org (.rpm)
   - homebrew-zerodds (Formula-Update)
   - GitHub Releases (.msi + .pkg, fuer Endnutzer-Discoverability)

---

## 6. Smoke-Tests pro Plattform

**Linux:**

```bash
sudo dpkg -i zerodds-tools_*.deb
zerodds-admin --version
sudo dpkg -r zerodds-tools
```

**Windows (PowerShell als Admin):**

```powershell
msiexec /i zerodds-0.0.0-x64.msi /qn
& "$env:ProgramFiles\ZeroDDS\bin\zerodds-admin.exe" --version
msiexec /x zerodds-0.0.0-x64.msi /qn
```

**macOS:**

```bash
sudo installer -pkg zerodds-0.0.0.pkg -target /
zerodds-admin --version
# Uninstall:
sudo rm -rf /usr/local/bin/dds-* /usr/local/lib/libzerodds.dylib \
            /usr/local/include/zerodds
```

---

## 7. Was nicht in dieser Crate ist

* **Repo-Hosting + Signing-Keys** — apt.zerodds.org, yum.zerodds.org,
  Homebrew-Tap-Repo, Apple-Developer-ID, Microsoft Authenticode-Cert
  liegen im Operations-Bereich.
* **CI-Pipelines** — `.gitlab-ci.yml` Stanzas fuer die vier
  Plattformen sind ein Folge-Sprint (E.2-Phase-B), inklusive
  Multi-Stage-Cache + Artifacts-Upload.
* **Yocto/Buildroot-Integration** — siehe E.4 in
  `docs/PHASE5_PLAN.md`.
* **Snap / Flatpak** — nicht aktiv geplant; sollen als
  Community-Beitraege via Tap/PPA-Ecosystem entstehen, nicht
  Maintainer-Pflicht.

---

## 7a. Gepackte CLIs + Daemons (Spec §1.1, §1.3)

ZeroDDS-Distributions packen **24 Binaries**: 7 Daemons + 17 CLI-Tools.
Die `[workspace.metadata.dist.bin-aliases]`-Tabelle in `Cargo.toml`
hinterlegt fuer jedes Binary einen Kurz-Alias, der parallel installiert
wird (`/usr/bin/<alias>` zusaetzlich zu `/usr/bin/<full-name>`).

**Daemons (Spec §1.1):**

| Binary                  | Alias        | Bridge / Service                          |
|-------------------------|--------------|-------------------------------------------|
| `zerodds-ws-bridged`    | `zddsws`     | WebSocket-Bridge (Browser + Node)         |
| `zerodds-mqtt-bridged`  | `zddsmqtt`   | MQTT-5-Bridge (gegen Mosquitto/HiveMQ)    |
| `zerodds-coap-bridged`  | `zddscoap`   | CoAP-Bridge (RFC 7252/7641/7959)          |
| `zerodds-amqp-bridged`  | `zddsamqp`   | AMQP-1.0-Bridge (RabbitMQ + qpid-proton)  |
| `zerodds-grpc-bridged`  | `zddsgrpc`   | gRPC-Bridge (HTTP/2 + Reflection)         |
| `zerodds-corba-bridged` | `zddscorba`  | CORBA-Bridge (IIOP + GIOP 1.0/1.1/1.2)    |
| `zerodds-ros2-shim`     | `zddsros2`   | ROS-2 RMW-Shim (REP-2003/2008)            |

**CLI-Tools (Spec §1.3):**

| Binary              | Alias         | Funktion                                |
|---------------------|---------------|-----------------------------------------|
| `zerodds-admin`     | `zdds`        | Top-Level-Admin (status, list, etc.)    |
| `zerodds-idlc`      | `zddsidl`     | IDL-Compiler (Code-Gen 8 Sprachen)      |
| `zerodds-spy`       | `zddsspy`     | Topic-Spy (Live-Sample-Dump)            |
| `zerodds-record`    | `zddsrec`     | PCAP-/Topic-Recorder                    |
| `zerodds-replay`    | `zddsplay`    | Replay aus PCAP                         |
| `zerodds-bench`     | `zddsbench`   | DDS-Latenz-/Throughput-Bench            |
| `zerodds-snitch`    | `zddssnitch`  | Reality-Inspector-CLI (PDE-WP-E)        |
| `zerodds-monitor`   | `zddsmon`     | Live-DDS-Monitor (TUI)                  |
| `zerodds-mq`        | `zddsmq`      | Message-Queue-Inspect (AMQP/MQTT)       |
| `zerodds-pcap`      | `zddspcap`    | PCAP-Wire-Decoder                       |
| `zerodds-perf`      | `zddsperf`    | Cross-Vendor-Perf-Harness (Cyclone)     |
| `zerodds-shape`     | (— )          | Shapes-Demo (RTI-Compatible)            |
| `zerodds-keys`      | (— )          | DDS-Security Key-Generator              |
| `zerodds-perm`      | (— )          | Permissions-Document-Editor             |
| `zerodds-cert`      | (— )          | X.509-Cert-Generator (CA-Workflow)      |
| `zerodds-doctor`    | (— )          | System-Health-Checker                   |
| `zerodds-license`   | (— )          | License-Inspector + Telemetry-Off       |

**Per-Distribution-Mapping:**
* **Debian/Ubuntu** (`packaging/linux/deb/`): alle 24 Binaries unter
  `/usr/bin/`, Aliase als Symlinks.
* **Fedora/RHEL** (`packaging/linux/rpm/`): identisch.
* **Arch Linux** (`packaging/linux/arch/PKGBUILD`): identisch, +
  Manpages unter `/usr/share/man/man1/`.
* **AppImage** (`packaging/linux/appimage/build.sh`): pro Daemon ein
  selbsttragendes AppImage (musl-static), CLIs in einem
  `zerodds-tools.AppImage`-Bundle.
* **Homebrew** (`packaging/macos/homebrew/`): nur Daemons + Top-Level-
  CLIs (`zerodds-admin`, `zerodds-idlc`, `zerodds-spy`).
* **Windows MSI** (`packaging/windows/`): alle 24 Binaries unter
  `%ProgramFiles%\ZeroDDS\bin\`.

## 8. Referenzen

* Debian Policy 4.6 — https://www.debian.org/doc/debian-policy/
* Fedora Packaging Guidelines — https://docs.fedoraproject.org/en-US/packaging-guidelines/
* WiX Toolset 5 — https://wixtoolset.org/docs/intro/
* Apple `productbuild(1)` / `notarytool(1)` — https://developer.apple.com/documentation/xcode/customizing-the-notarization-workflow
* Homebrew Formula Cookbook — https://docs.brew.sh/Formula-Cookbook
