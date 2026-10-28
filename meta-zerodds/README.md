# meta-zerodds — Yocto layer for ZeroDDS

WP 5.E.4. Provides the `meta-zerodds` OE layer that hooks ZeroDDS into Yocto
builds.

## Layer activation

```bash
cd ~/poky/build
bitbake-layers add-layer ../../meta-zerodds
```

Or by hand in `conf/bblayers.conf`:

```
BBLAYERS += "${TOPDIR}/../meta-zerodds"
```

## Layer compatibility

Yocto releases:

* **scarthgap** (5.0 LTS, April 2024) — primary target
* **nanbield** (4.3, October 2023)
* **kirkstone** (4.0 LTS, April 2022)

## Dependencies

* `meta-rust` (https://github.com/meta-rust/meta-rust) — for
  `cargo_bin.bbclass` and the cross-compile toolchain.
* `core` — Yocto standard.

## Recipe

`recipes-zerodds/zerodds/zerodds_0.0.0.bb` — builds the
ZeroDDS workspace with `cargo build -p dds-c-api --release` and
installs `libzerodds.so` + `zerodds.h` into the Yocto image.

## PACKAGECONFIG knobs

| Knob | Description |
| ---- | ------------ |
| `rtps-only` | Slim build without security/bridges/CCM/CORBA |
| `security` | + DDS-Security 1.2 (on by default) |
| `bridges` | + CoAP/MQTT/WebSocket/gRPC/AMQP bridges |
| `tools` | + tool binaries (dds-replay, dds-chaos, dds-dashboard) |

Default: `security`. Example override in `local.conf`:

```
PACKAGECONFIG:pn-zerodds = "security bridges tools"
```

## Image targets

Bringing ZeroDDS into an image:

```
IMAGE_INSTALL:append = " zerodds"
```

Plus the tools bundle:

```
IMAGE_INSTALL:append = " zerodds zerodds-tools zerodds-dev"
```

## Cross-compile targets

* `aarch64-poky-linux` (Raspberry Pi 4/5, NXP iMX8, Apex.OS)
* `aarch64-poky-linux-musl` (embedded without glibc)
* `armv7vehf-poky-linux-gnueabi` (Raspberry Pi 2/3, Cortex-A7/A9)
* `x86_64-poky-linux` (qemu-x86-64 for CI)

## QEMU smoke test

```bash
bitbake core-image-minimal
runqemu qemuarm64 nographic
# inside the emulated target:
ldd /usr/lib/libzerodds.so
ls /usr/include/zerodds/zerodds.h
```

## Phase A vs phase B

* **Phase A** (now): recipe skeleton + layer conf + README. The recipe
  runs on a Yocto build host that has integrated `meta-rust`;
  live build not yet in CI.
* **Phase B**: the GitLab CI job `ci/jobs/yocto-image.yml` builds a
  qemu-aarch64 image with ZeroDDS nightly, runqemu smoke test in the CI runner.
