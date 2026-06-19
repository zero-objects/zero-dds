# meta-zerodds — Yocto-Layer fuer ZeroDDS

WP 5.E.4. Liefert das `meta-zerodds`-OE-Layer das ZeroDDS in Yocto-
Builds einklinkt.

## Layer-Aktivierung

```bash
cd ~/poky/build
bitbake-layers add-layer ../../meta-zerodds
```

Oder per Hand in `conf/bblayers.conf`:

```
BBLAYERS += "${TOPDIR}/../meta-zerodds"
```

## Layer-Kompatibilitaet

Yocto-Releases:

* **scarthgap** (5.0 LTS, April 2024) — primaeres Target
* **nanbield** (4.3, October 2023)
* **kirkstone** (4.0 LTS, April 2022)

## Dependencies

* `meta-rust` (https://github.com/meta-rust/meta-rust) — fuer
  `cargo_bin.bbclass` und Cross-Compile-Toolchain.
* `core` — Yocto-Standard.

## Recipe

`recipes-zerodds/zerodds/zerodds_0.0.0.bb` — baut den
ZeroDDS-Workspace mit `cargo build -p dds-c-api --release` und
installiert `libzerodds.so` + `zerodds.h` ins Yocto-Image.

## PACKAGECONFIG-Knobs

| Knob | Beschreibung |
| ---- | ------------ |
| `rtps-only` | Slim-Build ohne Security/Bridges/CCM/CORBA |
| `security` | + DDS-Security 1.2 (default an) |
| `bridges` | + CoAP/MQTT/WebSocket/gRPC/AMQP-Bridges |
| `tools` | + Tool-Binaries (dds-replay, dds-chaos, dds-dashboard) |

Default: `security`. Beispiel-Override in `local.conf`:

```
PACKAGECONFIG:pn-zerodds = "security bridges tools"
```

## Image-Targets

ZeroDDS in ein Image bringen:

```
IMAGE_INSTALL:append = " zerodds"
```

Plus das Tools-Bundle:

```
IMAGE_INSTALL:append = " zerodds zerodds-tools zerodds-dev"
```

## Cross-Compile-Targets

* `aarch64-poky-linux` (Raspberry Pi 4/5, NXP iMX8, Apex.OS)
* `aarch64-poky-linux-musl` (embedded ohne glibc)
* `armv7vehf-poky-linux-gnueabi` (Raspberry Pi 2/3, Cortex-A7/A9)
* `x86_64-poky-linux` (qemu-x86-64 fuer CI)

## QEMU-Smoketest

```bash
bitbake core-image-minimal
runqemu qemuarm64 nographic
# innerhalb des emulierten Targets:
ldd /usr/lib/libzerodds.so
ls /usr/include/zerodds/zerodds.h
```

## Phase-A vs Phase-B

* **Phase-A** (jetzt): Recipe-Skelett + Layer-Conf + README. Recipe
  laeuft auf einem Yocto-Build-Host der `meta-rust` integriert hat;
  Live-Build noch nicht in CI.
* **Phase-B**: GitLab-CI-Job `ci/jobs/yocto-image.yml` baut nightly
  ein qemu-aarch64-Image mit ZeroDDS, runqemu-Smoketest in CI-Runner.
