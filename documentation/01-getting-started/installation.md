# Installation

ZeroDDS ships as native packages on Debian/Ubuntu, RHEL/Fedora,
macOS (Homebrew + .pkg), and Windows (.msi). For Rust development
the workspace also builds from source via Cargo.

For a complete maintainer-side packaging guide see
`docs/PACKAGING.md` (internal repo only).

## Debian / Ubuntu

```bash
# When the apt repo lands at apt.zerodds.org,
# the install will look like this:
curl -fsSL https://apt.zerodds.org/key.gpg | sudo apt-key add -
echo "deb https://apt.zerodds.org/ stable main" | sudo tee /etc/apt/sources.list.d/zerodds.list
sudo apt update
sudo apt install zerodds-tools libzerodds-dev
```

Until the public apt repo is hosted, build the .deb locally:

```bash
git clone https://github.com/zero-objects/zero-dds.git
cd zerodds
cp -r pkg/debian debian
dpkg-buildpackage -us -uc -b
sudo dpkg -i ../zerodds-tools_*.deb
```

## RHEL / Fedora / openSUSE

```bash
# Local build:
git clone https://github.com/zero-objects/zero-dds.git
cd zerodds
rpmdev-setuptree
cp pkg/rpm/zerodds.spec ~/rpmbuild/SPECS/
git archive --format=tar.gz --prefix=zerodds-0.0.0/ HEAD \
  > ~/rpmbuild/SOURCES/zerodds-0.0.0.tar.gz
rpmbuild -ba ~/rpmbuild/SPECS/zerodds.spec
sudo dnf install ~/rpmbuild/RPMS/x86_64/zerodds-tools-*.rpm
```

## macOS

### Homebrew (recommended)

```bash
brew tap zerodds/zerodds https://github.com/zero-objects/homebrew-zerodds
brew install zerodds
```

### .pkg (Universal — Apple Silicon + Intel)

```bash
git clone https://github.com/zero-objects/zero-dds.git
cd zerodds
VERSION=0.0.0 ./pkg/macos/build_pkg.sh
sudo installer -pkg dist/macos/zerodds-0.0.0.pkg -target /
```

The .pkg installs to `/usr/local/{bin,lib,include}`.

## Windows

```powershell
# Build the MSI from source:
git clone https://github.com/zero-objects/zero-dds.git
cd zerodds
pwsh -File pkg/windows/build.ps1
msiexec /i dist/windows/zerodds-0.0.0-x64.msi /qn

# Verify:
& "$env:ProgramFiles\ZeroDDS\bin\zerodds-admin.exe" --version
```

The installer adds `%ProgramFiles%\ZeroDDS\bin` to the user PATH —
new shells pick it up.

## From source (Rust)

```bash
git clone https://github.com/zero-objects/zero-dds.git
cd zerodds
cargo build --workspace --release
```

Binaries land in `target/release/`. Reach the rust API via the
`zerodds` crate (`crates/rs/`) once it is wired up; for now use
`zerodds-dcps` directly.

## Verify

After install, on any platform:

```bash
zerodds-admin --version
zerodds-perf hw-info
```

`zerodds-perf hw-info` prints CPU-feature detection (AES-NI / ARMv8-AES
/ PCLMULQDQ / SHA / AVX2 / NEON) — useful as a deployment-audit
breadcrumb.

## Next

→ [DDS in 5 minutes](concepts.md)
