# Tools, man pages and packaging

German original: [tools-and-man.md](tools-and-man.md)

This page describes (a) every entry in the directory `tools/` with purpose, invocation and classification as user tool, CI gate or test/measurement environment, (b) the man pages under `man/` with the program each page documents, the programs without a man page, viewing and installation, and the discrepancies found against the program, (c) the directory `packaging/`. The options of the programs themselves are in [cli-options.en.md](cli-options.en.md). The classification "CI gate" is based on references in `ci/*.yml`, `.github/workflows/ci.yml`, `scripts/pre-commit.sh` and `ci/*.py`; an entry without a reference is marked as manual.

Related pages: [README.en.md](README.en.md) · [architecture.en.md](architecture.en.md) · [crates-core.en.md](crates-core.en.md) · [crates-bridges-services.en.md](crates-bridges-services.en.md) · [crates-languages-codegen.en.md](crates-languages-codegen.en.md) · [config.en.md](config.en.md) · [cli-options.en.md](cli-options.en.md) · [tools-and-man.en.md](tools-and-man.en.md) · [system-setup.en.md](system-setup.en.md) · [security.en.md](security.en.md) · [migration.en.md](migration.en.md) · [interop-and-dialects.en.md](interop-and-dialects.en.md) · [ros2.en.md](ros2.en.md)

## Summary

- `tools/` has 61 entries: 31 Rust crates, 11 single scripts or files, 19 directories with scripts, cases and templates.
- CI gates with a reference: `silent-drop-guard.sh`, `staged-tree-guard.sh`, `man-gate`, `endpoints-gate`, `byte-gate/verify.sh`, `interop-results` (`--verify`, `verify-record.sh`), `wireprobe/three-column-gate`, `pairwise-run`, `bench-suite` (`regression`), `zerodds-lint`.
- `man/man1` contains 20 pages, `man/man5` 8 pages. Seven installed programs have no man page (`zerodds-lint`, `-traceability`, `-secure-permissions`, `-chaos`, `-perf`, `-dashboard`, `-recorder-bridge`).
- `tools/man-gate --list` reports 0 discrepancies (option names and exit codes of all pages against the programs).

## (a) The directory `tools/`

Classification: **A** = user tool (packaged or intended for users), **G** = CI gate, **T** = test, interop or measurement environment (local or on the measurement host), **B** = build/release aid, **L** = library.

### Rust crates

| Entry | Binary | Kind | Purpose | Invocation |
|---|---|---|---|---|
| `admin` | `zerodds-admin` | A | operator CLI: inspect a domain live, check DDS-XML offline | `zerodds-admin domain inspect 0` |
| `amqp-dds-endpoint` | `amqp-dds-endpoint` | T | AMQP 1.0 server per spec §9.2, XML configuration | `amqp-dds-endpoint [--config F] [--listen A]` |
| `bench` | `zerodds-bench` | A | latency/throughput in-process | `zerodds-bench latency` |
| `bench-suite` | `regression`, `roundtrip-1us`, `roundtrip-typed` | G/T | `regression`: cross-process roundtrip gate without external hosts (CI job `bench-regression`, `ci/full.yml`); the other two: latency comparison with vendor apps | `cargo run -p zerodds-bench-suite --bin regression` |
| `cargo-dag` | `cargo-dag` | B | publish order of the workspace crates | `cargo-dag . --only-publishable` |
| `chaos` | `zerodds-chaos` | A | UDP fault proxy, netem, partitions, link flaps | `zerodds-chaos proxy --listen … --forward …` |
| `cli-common` | — | L | shared CLI helpers (`parse_duration`, signal handler, GUID prefix) for record/bench/monitor/spy/snitch/pcap/mq | |
| `dashboard` | `zerodds-dashboard` | A | HTTP dashboard (SPA + JSON), `--demo` with synthetic data | `zerodds-dashboard --bind 127.0.0.1:8089` |
| `dashboard-tauri` | `zerodds-dashboard-tauri` | A | Tauri 2.0 shell around `dashboard` (`.dmg`, `.AppImage`, `.msi`) | see `tools/dashboard-tauri/README.md` |
| `iceoryx2-oracle-consumer` | `iceoryx2-oracle-consumer` | T | iceoryx2 subscriber without a ZeroDDS dependency; checks that ZeroDDS writes real iceoryx2 | start next to a ZeroDDS publisher |
| `idlc` | `zerodds-idlc` | A | IDL 4.2 checker, formatter, code generator (21 languages); `tests/` contains CI scripts (`csharp-support`, `java-typelookup`) | `zerodds-idlc check a.idl` |
| `interop-matrix` | `interop-matrix` | T | JSON → HTML matrix, `--fail-on-red` | `interop-matrix in.json --out m.html` |
| `interop-results` | `interop-results` + scripts | G | result tree `tests/interop-results/`, ledger and tables; `--verify` is the CI command (GitHub `ci.yml`, `ci/full.yml` jobs `interop-results`, `interop-results-record`); scripts `record.sh`, `verify-record.sh`, `regenerate.sh`, `*-sweep.sh` | `cargo run -p zerodds-interop-results --bin interop-results -- --verify` |
| `isolation-smoke` | `isolation-smoke` | T | sender/receiver for `testing/isolation/` (UDP, UDS) | `isolation-smoke --transport=udp --role=receiver …` |
| `monitor` | `zerodds-monitor` | A | metrics snapshot, `/metrics` server | `zerodds-monitor serve` |
| `mq` | `zerodds-mq` | A | raw samples between two domains | `zerodds-mq -t T --src-domain 0 --dst-domain 1` |
| `omg-security-main` | `shapetype_main` | T | reference client for the OMG suite `dds-security`; `run_matrix.py`, `compatibility_matrix.tsv` | see README |
| `omg-shape-main` | `shape_main` | T | reference client for the OMG suite `dds-rtps`; `run_vendors.sh` | see README |
| `omg-xtypes-main` | `xtypes_main` | T | reference client for the OMG suite `dds-xtypes`; `run_matrix.py` | see README |
| `omg-xtypes-test-main` | `zerodds_test_main` | T | reimplementation of `test_main.cxx` of the XTypes suite; `run_configs.sh` | short options `-P/-S -t -y …` |
| `pcap` | `zerodds-pcap` | A | RTPS decoder for pcap files | `zerodds-pcap stats cap.pcap` |
| `perf` | `zerodds-perf` | A | AES-GCM throughput, CPU features, registry as Prometheus text | `zerodds-perf aes-gcm` |
| `qos-matrix` | `qos-matrix` | T | QoS compatibility table from `check_compatibility` | `qos-matrix --format csv` |
| `record` | `zerodds-record` | A | recording to `.zddsrec`, optionally decoded | `zerodds-record record -t T` |
| `recorder-bridge` | `zerodds-recorder-bridge` | A | recording with typed readers `NAME:TYPE` | `zerodds-recorder-bridge --out f --topic T:Ty` |
| `replay` | `zerodds-replay` | A | playback of `.zddsrec`/NDJSON/SQLite | `zerodds-replay replay f.zddsrec --inject` |
| `secure-permissions` | `zerodds-secure-permissions` | A | CMS signature of governance/permissions | `… sign --signer-cert … --in … --out …` |
| `snitch` | `zerodds-snitch` | A | SPDP/SEDP probe | `zerodds-snitch -f json` |
| `spy` | `zerodds-spy` | A | listen to a topic | `zerodds-spy -t T` |
| `traceability` | `zerodds-traceability` | A | requirement→code matrix from `docs/spec-coverage` | `zerodds-traceability --format md` |
| `xmlc` | `zerodds-xmlc` | A | check and render DDS-XML | `zerodds-xmlc validate d.xml` |

The subdirectories `tools/wireprobe/*-gen` (22 crates, generators per language) are described under `wireprobe`.

### Single scripts and files

| Entry | Kind | Purpose | Invocation / integration |
|---|---|---|---|
| `annotate-snippets.js` | B | assigns matching demo directories to the code blocks of the website | manual (listed in `scripts/mirror-exclude.txt`) |
| `dcps-shapes-interop.sh` | T | ZeroDDS ↔ third-party DDS, one shape topic, both directions, four stacks; a case passes only with a receiver count > 0 | `./tools/dcps-shapes-interop.sh [B D …]`; beforehand `cargo build -p zerodds-dcps --example omg_shapes --example classic_shapes` |
| `endpoints-gate` | G | builds and tests the committed suites of all bindings under `endpoints/` | `tools/endpoints-gate [--langs go,zig] [--list] [--require …]`; CI: job `endpoints-gate` in `ci/full.yml`, `ci/run_merge_scope.py` |
| `interop-env.sh` | T | environment of the vendor toolchains for the probes (replaces the lost `/root/tc-env.sh`); missing toolchain → variable unset, one line of message | `. tools/interop-env.sh` |
| `interop-gate` | T | all interop tests in one command, one result per test; intended for times without a CI runner | `tools/interop-gate [--tests a,b] [--require …] [--list]`; no CI reference |
| `interop-vendors.sh` | T | locations of the third-party toolchains as environment variables with measurement-host defaults; a missing toolchain is reported with the row count, never skipped silently | sourced by the interop harnesses in `tools/` |
| `man-gate` | G | compares long options and exit codes from `--help` of the built binaries (`target/debug`) with `man/man1/*.1` | `tools/man-gate` (exit 1 on a difference, 2 if a binary is missing), `--list`; CI: job `man-pages` in `ci/full.yml` (weekly rules) |
| `ros-extensibility-scan.sh` | T | checks whether a ROS 2 type declares an extensibility (basis for FINAL in the RMW); exit ≠ 0 on a match | manual |
| `silent-drop-allowlist.tsv` | G | allowlist with a justification per match for `silent-drop-guard.sh` (key: pattern, file, line text) | data |
| `silent-drop-guard.sh` | G | searches for eleven patterns of silent data loss, fails on every match outside the allowlist | `bash tools/silent-drop-guard.sh`; `scripts/pre-commit.sh`, `.github/workflows/ci.yml`, `ci/full.yml` |
| `staged-tree-guard.sh` | G | builds the index state instead of the working directory | `bash tools/staged-tree-guard.sh [--head]`; `scripts/pre-commit.sh`, `.github/workflows/ci.yml` (`--head`), `ci/full.yml` |

### Directories with scripts, cases and templates

| Entry | Kind | Purpose | Invocation |
|---|---|---|---|
| `byte-gate/` | G/T | bytes of the vendor serializers for one type and one sample, XCDR1/XCDR2 × LE/BE; result `crates/ast-layout/tests/fixtures/vendor_bytes.md` and `.rows.tsv` | `tools/byte-gate/gate.sh`; CI: `bash tools/byte-gate/verify.sh` (job `interop-vendor-bytes`, `ci/full.yml`) |
| `corba-oracle/` | T | ORBs read GIOP bytes written by ZeroDDS (`corba-read.sh`), sweep (`corba-sweep.sh`), `targeted/` | `tools/corba-oracle/corba-read.sh read.tsv` |
| `dds-roundtrip-codegen/` | — | empty directory | |
| `evolution-gate/` | T | type evolution: old type writes, new type reads, and vice versa (`evo.sh`, `cases/`, `smoke/`) | `tools/evolution-gate/evo.sh …`; no CI reference |
| `hashid-gate/` | T | which name `@hashid` hashes, across four implementations | `tools/hashid-gate/gate.sh [--cases …]`; no CI reference |
| `idl-4.2-gap-scan/` | T | measures the IDL 4.2 gap against `expected.tsv`; result `docs/spec-coverage/idl-4.2.gaps.md` | `scan.sh`, `--record`, `--report`, `IDLC=…` |
| `inventory-probes/` | T | single probes for `docs/wire-feature-inventory.md` (keyholder, bound-overrun, encapsulation-be, parameter-edges …) | `tools/inventory-probes/run.sh [name]` |
| `keyhash-probe/` | T | KeyHash per vendor for about 60 types (`keyhash.idl`, RTPS §9.6.4.8) | `run-all.sh [vendor]`, `run-wire.sh` |
| `measurements/` | T | transcripts of measurement runs that are cited in documents; not maintained according to the README | `m1.sh`, `m1b.sh`, `m1c.sh`, `final-record.sh` |
| `optional-bits/` | T | third byte-gate case set: padding bytes and `@optional` in key types | `gate.sh --cases pad2` (options as in `byte-gate/gate.sh`) |
| `pairwise-run/` | G/T | full read matrix over four combinations, one TSV per combination | `run.sh`, `verify.sh`, `combos.py`, `summarise.py`; CI: `ci/pairwise_shards.py` |
| `struct-compare/` | T | generated structure of a corpus case in all languages side by side, for review | `collect.sh <case> > review.md` |
| `try-construct-gate/` | T | narrower reader type reads wider bytes, XTypes 1.3 §7.2.2.7 | `gate.sh`; no CI reference (only in a comment in `scripts/cpd-gate.sh`) |
| `typeobject-bytes/` | T | serialized `TypeObject` bytes of three vendors against ZeroDDS | `record.sh` |
| `unspecified-reach/` | T | four harnesses for constructs that the standard does not specify; result `crates/ast-layout/tests/fixtures/unspecified_constructs.md` | `bitset-bytes.sh`, `fixed-bytes.sh`, `front-ends.sh`, `map-length.sh` |
| `wire-capture/` | T | second measurement level: what implementations put on the socket (fill run, corpus run with 107 cases, 9 implementations according to the README) | `capture-rtps-session.sh`, `check-run.py`, `decode-rtps.py`, `decode-giop.py` |
| `wire-oracle/` | T | second byte-gate case set and single ORB cases (JacORB, omniORB) | `gate.sh --cases …`, `fixed-orbs.sh` |
| `wireprobe/` | G/T | probe collection for the interop corpus: vendor probes (`wireprobe-cyclone`, `-fastdds`, `-rti`, `-opendds`, `-hdds`, `-ros`, `-tao`, `-omniorb`, `-jacorb`), ZeroDDS language probes (`wireprobe-zerodds-<lang>` + `-gen` crates), endpoint probes (`wireprobe-endpoint-*`), templates (`*-template.*`), drivers (`wireprobe-batch`, `wireprobe-pairwise`, `wireprobe-bytes`) and gates (`three-column-gate`, `endpoint-mutation-gate`); no README | CI: `bash tools/wireprobe/three-column-gate --full` (job `three-column-gate-full`, `ci/full.yml`), `ci/zd-test.sh` |
| `xtypes-1.3-gap-scan/` | T | measures the XTypes 1.3 gap; `probe/` is a Rust program (stdin → measured values) | `roster.py [--check]`, `scan.sh [--record\|--report]` |

## (b) Man pages

### Inventory

| File | Subject (NAME section) |
|---|---|
| `man/man1/zerodds-admin.1` | ZeroDDS operator CLI for live domains and DDS-XML configs |
| `man/man1/zerodds-amqp-bridged.1` | DDS to AMQP 1.0 bridge daemon |
| `man/man1/zerodds-bench.1` | end-to-end latency and throughput benchmarking on the ZeroDDS DCPS pipeline |
| `man/man1/zerodds-coap-bridged.1` | DDS to CoAP bridge daemon |
| `man/man1/zerodds-corba-bridged.1` | DDS to CORBA GIOP/IIOP bridge daemon |
| `man/man1/zerodds-durability-svc.1` | ZeroDDS Durability-Service daemon |
| `man/man1/zerodds-grpc-bridged.1` | DDS to gRPC bridge daemon |
| `man/man1/zerodds-idlc.1` | read OMG IDL 4.2 and say what is in it |
| `man/man1/zerodds-monitor.1` | live snapshot or Prometheus /metrics server for the ZeroDDS runtime |
| `man/man1/zerodds-mq.1` | cross-domain DDS bridge |
| `man/man1/zerodds-mqtt-bridged.1` | DDS to MQTT 5 bridge daemon |
| `man/man1/zerodds-pcap.1` | offline RTPS submessage decoder for pcap files |
| `man/man1/zerodds-record.1` | capture and inspect ZeroDDS .zddsrec files |
| `man/man1/zerodds-replay.1` | replay recorded ZeroDDS captures to stdout or a live domain |
| `man/man1/zerodds-ros2-shim.1` | ROS-2 RMW shim and diagnostics |
| `man/man1/zerodds-router.1` | DDS routing-service daemon |
| `man/man1/zerodds-snitch.1` | DDS discovery probe (SPDP / SEDP snapshot) |
| `man/man1/zerodds-spy.1` | subscribe to a DDS topic and dump samples |
| `man/man1/zerodds-ws-bridged.1` | DDS to WebSocket bridge daemon |
| `man/man1/zerodds-xmlc.1` | DDS-XML validator, deployment renderer, type lister |
| `man/man5/zerodds-amqp-bridged.yaml.5` | configuration file for zerodds-amqp-bridged(1) |
| `man/man5/zerodds-coap-bridged.yaml.5` | configuration file for zerodds-coap-bridged(1) |
| `man/man5/zerodds-corba-bridged.yaml.5` | configuration file for zerodds-corba-bridged(1) |
| `man/man5/zerodds-grpc-bridged.yaml.5` | configuration file for zerodds-grpc-bridged(1) |
| `man/man5/zerodds-mqtt-bridged.yaml.5` | configuration file for zerodds-mqtt-bridged(1) |
| `man/man5/zerodds-ros2.yaml.5` | the ros2 section of the ZeroDDS runtime configuration |
| `man/man5/zerodds-ws-bridged.yaml.5` | configuration file for zerodds-ws-bridged(1) |
| `man/man5/zerodds.yaml.5` | the ZeroDDS runtime configuration |

No page carries a version in its header (`.TH … "" "ZeroDDS"`); `--version` gives the version.

Without a man page:

- installed programs: `zerodds-lint`, `zerodds-traceability`, `zerodds-secure-permissions`, `zerodds-chaos`, `zerodds-perf`, `zerodds-dashboard`, `zerodds-recorder-bridge`;
- file format: no man5 page for the route file of the router.

The pages refer to the specifications as files (`docs/specs/<name>.md`), not as manual pages; the external pages `mosquitto(8)` and `launchd(8)` remain references.

### Viewing

| System | Command |
|---|---|
| Linux (man-db) | `man -l man/man1/zerodds-spy.1` |
| macOS | `mandoc man/man1/zerodds-spy.1 \| less` or `man ./man/man1/zerodds-spy.1` |
| installed | `man zerodds-spy`, `man 5 zerodds-ws-bridged.yaml` |

`mandoc -T lint` reports no warning on any page except the empty date in `.TH` (measured). `website/_tools/render_man_pages.py` generates the HTML pages under `website/topics/man-pages/` from `man/man1/*.1`.

### Installation

| Package | How |
|---|---|
| rpm (`packaging/linux/rpm/zerodds.spec`) | copies all `man/man1/*.1` and `man/man5/*.5` to `%{_mandir}`; `%files` assigns them to the subpackages (`cli`, one per bridge, `ros2`, `router`) |
| deb (`packaging/linux/deb/assemble-debs.sh`) | `zerodds-core`: `zerodds.yaml.5`; `zerodds-cli`: `man/man1/zerodds-$tool.1`, if present; bridges: `.1` and `.yaml.5`; `zerodds-ros2`: `zerodds-ros2-shim.1`, `zerodds-ros2.yaml.5`; `zerodds-router`: `zerodds-router.1`; `zerodds-durability`: `zerodds-durability-svc.1` |
| Arch (`packaging/linux/arch/PKGBUILD`) | all `man1/*.1`, `man5/*.5` to `/usr/share/man` |
| Homebrew (`packaging/macos/homebrew/zerodds.rb`) | `man1.install Dir["share/man/man1/zerodds-*.1"]`, `man5.install Dir["share/man/man5/zerodds*.yaml.5"]` |
| macOS pkg (`packaging/macos/pkg/build-pkg.sh`) | copies to `/usr/local/share/man/man1` and `man5` |

### Discrepancies man page ↔ program

None open. `tools/man-gate --list` compares option names and exit codes of all pages with the programs and reports 0 discrepancies; details in [cli-options.en.md](cli-options.en.md#discrepancies-between-help-text-code-and-man-page).

## (c) `packaging/`

| Path | Content |
|---|---|
| `linux/deb/` | `assemble-debs.sh` builds the subpackages (`zerodds-core`, `-cli`, `-dev`, one per bridge, `-ros2`, `-router`) from `target/<triple>/release`; `build-rmw-deb.sh` builds `ros-<distro>-rmw-zerodds-cpp` from the ament project (`--distro`, jazzy as the default); `control.tmpl`, `postinst.tmpl`, `prerm.tmpl`, `postrm.tmpl`; `publish-deb.yml` is the cargo-dist publish job |
| `linux/rpm/` | `zerodds.spec` (subpackages `cli`, `devel`, one per bridge, `ros2`, `router`; builds without `zerodds-durability-service-bin`), `zerodds.pc` (pkg-config) |
| `linux/arch/` | `PKGBUILD` |
| `linux/appimage/` | `build.sh` (musl-static AppImage per daemon/CLI, needs `appimagetool`), `AppRun.template` |
| `linux/systemd/` | units for the six bridges, `zerodds-router.service`, `zerodds-durability-svc.service`; `zerodds-sysusers.conf`, `zerodds-tmpfiles.conf` |
| `linux/configs/` | example configurations `*-bridged.yaml.example` (six bridges), `ros2.yaml.example`, `router.json.example` |
| `macos/homebrew/` | `zerodds.rb` (all binaries, man pages, service `zerodds-ws-bridged`), `zerodds-ros2.rb`, one pure service formula per bridge (no own archive; the binary comes from `zerodds`) |
| `macos/bundle/` | `build-bundle.sh` packs `zerodds-<v>-<target>.tar.gz` (the layout `zerodds.rb` installs) plus its `.sha256`; called by the `macos-bundle` job (`ci/full.yml`) |
| `macos/launchd/` | `org.zerodds.<bridge>-bridged.plist` for the six bridges, each with `--config /usr/local/etc/zerodds/<stem>.yaml` |
| `macos/pkg/` | `build-pkg.sh` builds a `.pkg` with binaries and man pages under `/usr/local` |
| `windows/msi/` | `zerodds.wxs` (WiX; `bin`, `lib`, `include`, PATH entry) |
| `windows/chocolatey/` | `zerodds.nuspec`, `packaging/windows/chocolatey/tools/chocolateyinstall.ps1`, `chocolateyuninstall.ps1` |
| `windows/scoop/` | `zerodds.json` (release ZIPs x86_64 and aarch64) |
| `windows/services/` | `Install-Services.ps1`, `Uninstall-Services.ps1` for the services `ZeroDDSWSBridge`, `ZeroDDSMQTTBridge`, `ZeroDDSCoAPBridge`, `ZeroDDSAMQPBridge`, `ZeroDDSGrpcBridge`, `ZeroDDSCORBABridge` |
| `docker/` | Dockerfiles per bridge (`*-bridged/`), `durability-svc/`, `ros2-shim/`, `cli/` (18 CLI binaries), runtime images `py-runtime/`, `cpp-runtime/`, `ts-node-runtime/` (each with a README); `docker-compose.yml` with the bridges, `cli`, `mosquitto`, `rabbitmq` |
| `python/` | `sync-version.py` aligns the versions of the Python distributions with `[workspace.package].version` (`--check`) |
| `github-actions/` | `sign-artifacts.sh` (minisign, cosign), `sign-macos.sh` (codesign, notarization, `.pkg`), `render-homebrew.sh` (SHA-256 into the formulas, copy into the tap), `aur-publish.sh` (PKGBUILD/.SRCINFO for `zerodds-bin` and `zerodds`) |
| `keys/` | `minisign.pub` |

Notes from the comparison:

- deb `zerodds-cli` does not install `zerodds-secure-permissions`; rpm `zerodds-cli` does.
- The systemd unit of the durability service is skipped in the rpm (`zerodds.spec`, comment on libduckdb); there is no package for the binary.
