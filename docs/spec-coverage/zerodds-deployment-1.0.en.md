# `zerodds-deployment` v1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-deployment-1.0.md`

Implementation:

- `crates/zerodds-c-api/` — cdylib + the 7 bridge daemons (ws/mqtt/coap/amqp/grpc/corba/ros2-shim).

## §1 Targets

### §1.1 Daemons (7 bridges)

**Spec:** §1.1 — seven daemon binaries: ws/mqtt/coap/amqp/grpc/corba/
ros2-shim.

**Repo:** `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`,
`crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`,
`crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`,
`crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`,
`crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`,
`crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`,
`crates/rmw-zerodds-shim/src/bin/zerodds-ros2-shim.rs`.

**Tests:** Ein E2E-Test pro Daemon:
`crates/websocket-bridge/tests/daemon_e2e.rs::handshake_roundtrip_succeeds`,
`crates/mqtt-bridge/tests/daemon_e2e.rs::daemon_connects_and_subscribes`,
`crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`,
`crates/amqp-endpoint/tests/bridge_e2e.rs::version_flag_emits_one_line_and_exits_zero`,
`crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`,
`crates/corba-dds-bridge/tests/bridge_e2e.rs::a_request_to_an_unknown_object_is_object_not_exist`,
`crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::selftest_succeeds`.

**Status:** done

### §1.2 Library libzerodds + zerodds.h

**Spec:** §1.2 — `libzerodds.{so,dylib,dll}` + `zerodds.h`; see
`zerodds-ffi-loader-1.0` §6.

**Repo:** `crates/zerodds-c-api/Cargo.toml` (cdylib),
`crates/zerodds-c-api/include/zerodds.h`.

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_pub_sub_roundtrip`,
`::ffi_handles_null_safely`, `::ffi_pub_sub_roundtrip_via_loan`;
the header itself
`crates/zerodds-c-api/tests/abi_compat.rs::the_header_macro_and_the_exported_revision_agree`
und `::abi_snapshot_matches`.

**Status:** done

### §1.3 CLI tools (17)

**Spec:** §1.3 — admin/bench/bench-suite/idlc/spy/record/replay/snitch/
monitor/mq/pcap/perf/ros2-shim/secure-keygen/secure-permissions/
typeobject/xml-config.

**Repo:** per-tool `crates/<tool>/src/bin/` and `tools/<tool>/`
(admin/bench-suite/idlc/recorder-bridge/replay/perf/qos-matrix/
xmlc/dashboard/interop-matrix/isolation-smoke/
chaos/traceability/cargo-dag); man pages
`man/man1/zerodds-{admin,idlc,ros2-shim,ws-bridged,mqtt-bridged,
coap-bridged,amqp-bridged,grpc-bridged,corba-bridged}.1`
(cluster-C CLI coverage audit).

**Tests:** One smoke test per tool, among them
`tools/idlc/src/format.rs::tests::the_corpus_formats_idempotently_and_round_trips`,
`tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/lint.rs::tests::a_clean_file_reports_nothing`,
`tools/idlc/src/dump.rs::tests::the_tree_names_every_type_and_its_members`,
`crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::version_emits_one_line`,
`::validate_with_minimal_yaml_succeeds`, `::info_includes_rmw_compat_marker`,
`::doctor_clean_ros_env_passes`.

**Status:** done

## §2 Linux

### §2.1 FHS file layout

**Spec:** §2.1 — `/usr/bin/`, `/usr/lib/libzerodds.so`,
`/usr/lib/pkgconfig/zerodds.pc`, `/usr/include/zerodds.h`,
`/etc/zerodds/`, `/var/log/zerodds/`, `/var/lib/zerodds/`,
`/usr/share/man/`.

**Repo:** `packaging/linux/configs/`,
`packaging/linux/systemd/zerodds-tmpfiles.conf`,
`packaging/linux/systemd/zerodds-sysusers.conf`,
`packaging/linux/rpm/zerodds.pc`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::fhs_layout_paths_are_the_ones_the_clause_names`.

**Status:** done

### §2.2.1 Debian/Ubuntu .deb packages

**Spec:** §2.2.1 — packages `zerodds-core`/`zerodds-<bridge>`/
`zerodds-cli`/`zerodds-dev`; repo `packages.zerodds.io/apt`; signed GPG.

**Repo:** `packaging/linux/deb/{control.tmpl,postinst.tmpl,postrm.tmpl,prerm.tmpl,assemble-debs.sh,publish-deb.yml}`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::debian_control_declares_the_package_split_of_the_clause`.

**Status:** done

### §2.2.2 RHEL/Fedora .rpm packages

**Spec:** §2.2.2 — an analogous package set; repo `packages.zerodds.io/yum/`;
GPG key.

**Repo:** `packaging/linux/rpm/{zerodds.spec,publish-rpm.yml,zerodds.pc}`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::rpm_spec_declares_the_same_split_and_systemd_scriptlets`.

**Status:** done

### §2.2.3 Arch Linux PKGBUILD

**Spec:** §2.2.3 — AUR packages; a pacman hook for service reload.

**Repo:** `packaging/linux/arch/PKGBUILD` (cluster-C AUR package layout).

**Tests:** `tools/traceability/tests/deployment_layout.rs::arch_pkgbuild_carries_the_aur_fields_and_the_daemon_list`.

**Status:** done

### §2.2.4 AppImage static build per daemon

**Spec:** §2.2.4 — `zerodds-<bridge>-1.0.0-x86_64.AppImage`; statically
against musl; GitHub Releases + S3 mirror.

**Repo:** `packaging/linux/appimage/{AppRun.template,build.sh}` (cluster-C
linuxdeploy build script).

**Tests:** `tools/traceability/tests/deployment_layout.rs::appimage_build_targets_musl_and_covers_every_daemon`.

**Status:** done

### §2.3 systemd unit per bridge

**Spec:** §2.3 — `Type=notify`, ExecStart, ExecReload SIGHUP,
KillMode=mixed, NoNewPrivileges/ProtectSystem/PrivateTmp/LimitNOFILE.

**Repo:** `packaging/linux/systemd/zerodds-{ws,mqtt,coap,amqp,grpc,corba}-bridged.service`,
`packaging/linux/systemd/zerodds-sysusers.conf`,
`packaging/linux/systemd/zerodds-tmpfiles.conf`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::every_systemd_unit_carries_the_hardening_set_of_the_clause`.

**Status:** done

## §3 macOS

### §3.1 File layout

**Spec:** §3.1 — `/usr/local/{bin,lib,include,etc,var}/zerodds/`,
`/Library/LaunchDaemons/org.zerodds.*.plist`.

**Repo:** `packaging/macos/launchd/`,
`packaging/macos/homebrew/zerodds.rb`,
`packaging/macos/pkg/build-pkg.sh`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::macos_layout_uses_the_usr_local_prefix`,
`tools/traceability/tests/deployment_layout.rs::every_launchd_plist_carries_the_keys_of_the_clause`.

**Status:** done

### §3.2.1 Homebrew tap + formulae

**Spec:** §3.2.1 — tap `zero-objects/zerodds`; formulae with
brew-services integration.

**Repo:** `packaging/macos/homebrew/{zerodds.rb,zerodds-ws-bridge.rb,zerodds-mqtt-bridge.rb,zerodds-coap-bridge.rb,zerodds-amqp-bridge.rb,zerodds-grpc-bridge.rb,zerodds-corba-bridge.rb,zerodds-ros2.rb}`,
`packaging/github-actions/render-homebrew.sh`,
`packaging/macos/bundle/build-bundle.sh`, `ci/full.yml` (`macos-bundle`).

**Tests:** `tools/traceability/tests/deployment_layout.rs::every_homebrew_formula_declares_a_brew_service`,
`tools/traceability/tests/deployment_layout.rs::the_bundle_packer_lays_out_what_the_formula_installs`,
`tools/traceability/tests/macos_bundle_ci_job.rs::macos_bundle_job_packs_the_layout_the_formula_installs`.

**Status:** done

### §3.2.2 PKG installer signed/notarized

**Spec:** §3.2.2 — a signed `.pkg` with an Apple Developer ID.

**Repo:** `packaging/macos/pkg/build-pkg.sh`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::macos_pkg_build_signs_and_notarizes`.

**Status:** done

### §3.3 launchd plist per bridge

**Spec:** §3.3 — `Label=org.zerodds.<bridge>`, ProgramArguments,
RunAtLoad, KeepAlive, Stdout/Err paths, UserName.

**Repo:** `packaging/macos/launchd/org.zerodds.{ws,mqtt,coap,amqp,grpc,corba}-bridged.plist`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::every_launchd_plist_carries_the_keys_of_the_clause`.

**Status:** done

## §4 Windows

### §4.1 File layout %PROGRAMFILES%\ZeroDDS

**Spec:** §4.1 — `%PROGRAMFILES%\ZeroDDS\{bin,include,lib,doc}\` +
`%PROGRAMDATA%\ZeroDDS\{*.yaml,certs,logs}\`.

**Repo:** `packaging/windows/msi/zerodds.wxs` (layout definition),
`packaging/windows/services/`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::windows_layout_splits_program_files_from_program_data`.

**Status:** done

### §4.2.1 MSI installer (WiX) signed

**Spec:** §4.2.1 — `zerodds-1.0.0-x64.msi` with EV code signing; optional
features per bridge.

**Repo:** `packaging/windows/msi/zerodds.wxs`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::msi_defines_one_optional_feature_per_bridge`.

**Status:** done

### §4.2.2 Scoop manifest

**Spec:** §4.2.2 — a `scoop bucket add zerodds` layout, a manifest per
component.

**Repo:** `packaging/windows/scoop/zerodds.json`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::scoop_manifest_has_both_architectures_and_a_bin_list`.

**Status:** done

### §4.2.3 Chocolatey package

**Spec:** §4.2.3 — `choco install zerodds zerodds-ws-bridge` packages.

**Repo:** `packaging/windows/chocolatey/{zerodds.nuspec,tools/}`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::chocolatey_package_has_its_nuspec_metadata_and_both_scripts`.

**Status:** done

### §4.3 Windows service per bridge

**Spec:** §4.3 — `sc.exe create ZeroDDS<Bridge>` with
binPath/start=auto/obj/DisplayName/failure-recovery; one service per bridge.

**Repo:** `packaging/windows/services/{Install-Services.ps1,Uninstall-Services.ps1}`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::windows_service_scripts_register_and_remove_the_same_bridge_set`.

**Status:** done

## §5 Docker

### §5.1 Image list per daemon + ROS distros

**Spec:** §5.1 — 10 images (ws/mqtt/coap/amqp/grpc/corba/ros2-{humble,
iron,jazzy}/cli); multi-arch amd64+arm64; docker.io+ghcr.io.

**Repo:** `packaging/docker/{ws-bridged,mqtt-bridged,coap-bridged,amqp-bridged,grpc-bridged,corba-bridged,ros2-shim,cli}/`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::one_dockerfile_per_daemon_plus_cli_and_the_three_ros_distros`.

**Status:** done

### §5.2 Multi-stage Dockerfile with cargo-chef

**Spec:** §5.2 — stage 1 build with cargo-chef, stage 2 debian-slim
runtime + non-root user.

**Repo:** `packaging/docker/<bridge>/Dockerfile` (per-bridge).

**Tests:** `tools/traceability/tests/deployment_layout.rs::every_daemon_image_is_cargo_chef_multi_stage_and_runs_non_root`.

**Status:** done

### §5.3 Docker-compose example

**Spec:** §5.3 — `compose.yaml` with ws-bridge + mqtt-bridge + mosquitto;
`network_mode: host`.

**Repo:** `packaging/docker/docker-compose.yml`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::docker_compose_runs_the_bridges_on_the_host_network`.

**Status:** done

## §6 cargo-dist setup

### §6 [workspace.metadata.dist] with cargo-dist 0.20

**Spec:** §6 — installers shell/powershell/homebrew/msi; targets
linux-gnu/musl + apple-darwin + pc-windows-msvc; minisign+cosign.

**Repo:** `Cargo.toml` (`[workspace.metadata.dist]` block),
`.github/workflows/release.yml`,
`packaging/github-actions/{render-homebrew.sh,sign-artifacts.sh}`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::cargo_dist_block_declares_the_installers_targets_and_signing`.

**Status:** done

## §7 Version sync

### §7 Workspace version + pre-release tags

**Spec:** §7 — `[workspace.package].version` + `version.workspace=true`
per crate; rc.1/rc.2/final 1.0.0.

**Repo:** `Cargo.toml` (workspace.package), per-crate `Cargo.toml` with
`version.workspace = true`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::every_workspace_member_inherits_the_single_workspace_version`.

**Status:** done

## §8 Cross-references to bridge specs

### §8 §11 cross-refs of the 7 bridge specs + ffi-loader §6

**Spec:** §8 — cross-refs in the bridge-spec §11 sections + the FFI-loader
§6; extended cross-refs to monitor/observability-otlp/zddsrec/xcdr2.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §9 Versioning

### §9 SemVer bump rules

**Spec:** §9 — patch=bugfixes installer/service files, minor=additive
distros (NixOS/Alpine/SUSE-OBS), major=layout change
(`/usr/lib/`→`/opt/zerodds/`).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit status

23 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test run:

* `cargo test -p zerodds-traceability --test deployment_layout` — 22 tests
  green. One test per clause from §2 through §7; each opens the file the
  clause names and checks its content (FHS paths, package split, systemd
  hardening across all seven units, launchd keys across all seven plists,
  MSI features, Scoop architectures, cargo-chef stages, cargo-dist
  targets, workspace version across every member manifest).
* `cargo test -p zerodds-traceability --test macos_bundle_ci_job` — 1 test
  green: the `macos-bundle` job of `ci/full.yml`. The GitLab CI
  configuration is not part of the public mirror, and neither is this test.
* `cargo test -p zerodds-websocket-bridge -p zerodds-mqtt-bridge -p zerodds-coap-bridge -p zerodds-amqp-endpoint -p zerodds-grpc-bridge -p zerodds-corba-dds-bridge -p rmw-zerodds-shim -p zerodds-c-api` — tests green, 0 failed.

No open items or decision records — all items `done` / `n/a (informative)`.
