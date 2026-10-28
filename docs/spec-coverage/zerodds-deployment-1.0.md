# `zerodds-deployment` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-deployment-1.0.md`

Implementation:

- `crates/zerodds-c-api/` — cdylib + die 7 Bridge-Daemons (ws/mqtt/coap/amqp/grpc/corba/ros2-shim).

## §1 Targets

### §1.1 Daemons (7 Bridges)

**Spec:** §1.1 — sieben Daemon-Binaries: ws/mqtt/coap/amqp/grpc/corba/
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

**Spec:** §1.2 — `libzerodds.{so,dylib,dll}` + `zerodds.h`; siehe
`zerodds-ffi-loader-1.0` §6.

**Repo:** `crates/zerodds-c-api/Cargo.toml` (cdylib),
`crates/zerodds-c-api/include/zerodds.h`.

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_pub_sub_roundtrip`,
`::ffi_handles_null_safely`, `::ffi_pub_sub_roundtrip_via_loan`;
der Header selbst
`crates/zerodds-c-api/tests/abi_compat.rs::the_header_macro_and_the_exported_revision_agree`
und `::abi_snapshot_matches`.

**Status:** done

### §1.3 CLI-Tools (17)

**Spec:** §1.3 — admin/bench/bench-suite/idlc/spy/record/replay/snitch/
monitor/mq/pcap/perf/ros2-shim/secure-keygen/secure-permissions/
typeobject/xml-config.

**Repo:** Per-Tool `crates/<tool>/src/bin/` und `tools/<tool>/`
(admin/bench-suite/idlc/recorder-bridge/replay/perf/qos-matrix/
xmlc/dashboard/interop-matrix/isolation-smoke/
chaos/traceability/cargo-dag); man-Pages
`man/man1/zerodds-{admin,idlc,ros2-shim,ws-bridged,mqtt-bridged,
coap-bridged,amqp-bridged,grpc-bridged,corba-bridged}.1`
(Cluster-C CLI-Coverage-Audit).

**Tests:** Je ein Smoke-Test pro Werkzeug, darunter
`tools/idlc/src/format.rs::tests::the_corpus_formats_idempotently_and_round_trips`,
`tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/lint.rs::tests::a_clean_file_reports_nothing`,
`tools/idlc/src/dump.rs::tests::the_tree_names_every_type_and_its_members`,
`crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs::version_emits_one_line`,
`::validate_with_minimal_yaml_succeeds`, `::info_includes_rmw_compat_marker`,
`::doctor_clean_ros_env_passes`.

**Status:** done

## §2 Linux

### §2.1 FHS-Datei-Layout

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

### §2.2.1 Debian/Ubuntu .deb-Pakete

**Spec:** §2.2.1 — Pakete `zerodds-core`/`zerodds-<bridge>`/
`zerodds-cli`/`zerodds-dev`; Repo `packages.zerodds.io/apt`; Signed
GPG.

**Repo:** `packaging/linux/deb/{control.tmpl,postinst.tmpl,postrm.tmpl,prerm.tmpl,assemble-debs.sh,publish-deb.yml}`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::debian_control_declares_the_package_split_of_the_clause`.

**Status:** done

### §2.2.2 RHEL/Fedora .rpm-Pakete

**Spec:** §2.2.2 — analoges Paket-Set; Repo `packages.zerodds.io/yum/`;
GPG-Key.

**Repo:** `packaging/linux/rpm/{zerodds.spec,publish-rpm.yml,zerodds.pc}`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::rpm_spec_declares_the_same_split_and_systemd_scriptlets`.

**Status:** done

### §2.2.3 Arch Linux PKGBUILD

**Spec:** §2.2.3 — AUR-Pakete; Pacman-Hook für Service-Reload.

**Repo:** `packaging/linux/arch/PKGBUILD` (Cluster-C AUR-Paket-Layout).

**Tests:** `tools/traceability/tests/deployment_layout.rs::arch_pkgbuild_carries_the_aur_fields_and_the_daemon_list`.

**Status:** done

### §2.2.4 AppImage Static-Build pro Daemon

**Spec:** §2.2.4 — `zerodds-<bridge>-1.0.0-x86_64.AppImage`; statisch
gegen musl; GitHub-Releases + S3-Mirror.

**Repo:** `packaging/linux/appimage/{AppRun.template,build.sh}`
(Cluster-C linuxdeploy-Build-Skript).

**Tests:** `tools/traceability/tests/deployment_layout.rs::appimage_build_targets_musl_and_covers_every_daemon`.

**Status:** done

### §2.3 systemd-Unit pro Bridge

**Spec:** §2.3 — `Type=notify`, ExecStart, ExecReload SIGHUP,
KillMode=mixed, NoNewPrivileges/ProtectSystem/PrivateTmp/LimitNOFILE.

**Repo:** `packaging/linux/systemd/zerodds-{ws,mqtt,coap,amqp,grpc,corba}-bridged.service`,
`packaging/linux/systemd/zerodds-sysusers.conf`,
`packaging/linux/systemd/zerodds-tmpfiles.conf`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::every_systemd_unit_carries_the_hardening_set_of_the_clause`.

**Status:** done

## §3 macOS

### §3.1 Datei-Layout

**Spec:** §3.1 — `/usr/local/{bin,lib,include,etc,var}/zerodds/`,
`/Library/LaunchDaemons/org.zerodds.*.plist`.

**Repo:** `packaging/macos/launchd/`,
`packaging/macos/homebrew/zerodds.rb`,
`packaging/macos/pkg/build-pkg.sh`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::macos_layout_uses_the_usr_local_prefix`,
`tools/traceability/tests/deployment_layout.rs::every_launchd_plist_carries_the_keys_of_the_clause`.

**Status:** done

### §3.2.1 Homebrew-Tap + Formulae

**Spec:** §3.2.1 — Tap `zero-objects/zerodds`; Formulae mit
brew-services-Integration.

**Repo:** `packaging/macos/homebrew/{zerodds.rb,zerodds-ws-bridge.rb,zerodds-mqtt-bridge.rb,zerodds-coap-bridge.rb,zerodds-amqp-bridge.rb,zerodds-grpc-bridge.rb,zerodds-corba-bridge.rb,zerodds-ros2.rb}`,
`packaging/github-actions/render-homebrew.sh`,
`packaging/macos/bundle/build-bundle.sh`, `ci/full.yml` (`macos-bundle`).

**Tests:** `tools/traceability/tests/deployment_layout.rs::every_homebrew_formula_declares_a_brew_service`,
`tools/traceability/tests/deployment_layout.rs::the_bundle_packer_lays_out_what_the_formula_installs`,
`tools/traceability/tests/macos_bundle_ci_job.rs::macos_bundle_job_packs_the_layout_the_formula_installs`.

**Status:** done

### §3.2.2 PKG-Installer signed/notarized

**Spec:** §3.2.2 — Signed `.pkg` mit Apple-Developer-ID.

**Repo:** `packaging/macos/pkg/build-pkg.sh`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::macos_pkg_build_signs_and_notarizes`.

**Status:** done

### §3.3 launchd-Plist pro Bridge

**Spec:** §3.3 — `Label=org.zerodds.<bridge>`, ProgramArguments,
RunAtLoad, KeepAlive, Stdout/Err-Pfade, UserName.

**Repo:** `packaging/macos/launchd/org.zerodds.{ws,mqtt,coap,amqp,grpc,corba}-bridged.plist`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::every_launchd_plist_carries_the_keys_of_the_clause`.

**Status:** done

## §4 Windows

### §4.1 Datei-Layout %PROGRAMFILES%\ZeroDDS

**Spec:** §4.1 — `%PROGRAMFILES%\ZeroDDS\{bin,include,lib,doc}\` +
`%PROGRAMDATA%\ZeroDDS\{*.yaml,certs,logs}\`.

**Repo:** `packaging/windows/msi/zerodds.wxs` (Layout-Definition),
`packaging/windows/services/`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::windows_layout_splits_program_files_from_program_data`.

**Status:** done

### §4.2.1 MSI-Installer (WiX) signed

**Spec:** §4.2.1 — `zerodds-1.0.0-x64.msi` mit EV-Code-Signing;
Optional-Features pro Bridge.

**Repo:** `packaging/windows/msi/zerodds.wxs`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::msi_defines_one_optional_feature_per_bridge`.

**Status:** done

### §4.2.2 Scoop-Manifest

**Spec:** §4.2.2 — `scoop bucket add zerodds`-Layout, Manifest pro
Komponente.

**Repo:** `packaging/windows/scoop/zerodds.json`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::scoop_manifest_has_both_architectures_and_a_bin_list`.

**Status:** done

### §4.2.3 Chocolatey-Package

**Spec:** §4.2.3 — `choco install zerodds zerodds-ws-bridge`-Pakete.

**Repo:** `packaging/windows/chocolatey/{zerodds.nuspec,tools/}`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::chocolatey_package_has_its_nuspec_metadata_and_both_scripts`.

**Status:** done

### §4.3 Win-Service pro Bridge

**Spec:** §4.3 — `sc.exe create ZeroDDS<Bridge>` mit binPath/start=auto/
obj/DisplayName/failure-recovery; pro Bridge ein Service.

**Repo:** `packaging/windows/services/{Install-Services.ps1,Uninstall-Services.ps1}`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::windows_service_scripts_register_and_remove_the_same_bridge_set`.

**Status:** done

## §5 Docker

### §5.1 Image-Liste pro Daemon + ROS-Distros

**Spec:** §5.1 — 10 Images (ws/mqtt/coap/amqp/grpc/corba/ros2-{humble,
iron,jazzy}/cli); multi-arch amd64+arm64; docker.io+ghcr.io.

**Repo:** `packaging/docker/{ws-bridged,mqtt-bridged,coap-bridged,amqp-bridged,grpc-bridged,corba-bridged,ros2-shim,cli}/`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::one_dockerfile_per_daemon_plus_cli_and_the_three_ros_distros`.

**Status:** done

### §5.2 Multi-Stage-Dockerfile mit cargo-chef

**Spec:** §5.2 — Stage 1 build mit cargo-chef, Stage 2 debian-slim
runtime + non-root-User.

**Repo:** `packaging/docker/<bridge>/Dockerfile` (per-Bridge).

**Tests:** `tools/traceability/tests/deployment_layout.rs::every_daemon_image_is_cargo_chef_multi_stage_and_runs_non_root`.

**Status:** done

### §5.3 Docker-Compose-Beispiel

**Spec:** §5.3 — `compose.yaml` mit ws-bridge + mqtt-bridge + mosquitto;
`network_mode: host`.

**Repo:** `packaging/docker/docker-compose.yml`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::docker_compose_runs_the_bridges_on_the_host_network`.

**Status:** done

## §6 Cargo-Dist-Setup

### §6 [workspace.metadata.dist] mit cargo-dist 0.20

**Spec:** §6 — installers shell/powershell/homebrew/msi; targets
linux-gnu/musl + apple-darwin + pc-windows-msvc; minisign+cosign.

**Repo:** `Cargo.toml` (`[workspace.metadata.dist]`-Block),
`.github/workflows/release.yml`,
`packaging/github-actions/{render-homebrew.sh,sign-artifacts.sh}`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::cargo_dist_block_declares_the_installers_targets_and_signing`.

**Status:** done

## §7 Version-Sync

### §7 Workspace-Version + Pre-Release-Tags

**Spec:** §7 — `[workspace.package].version` + `version.workspace=true`
pro Crate; rc.1/rc.2/finaler 1.0.0.

**Repo:** `Cargo.toml` (workspace.package),
Per-Crate `Cargo.toml` mit `version.workspace = true`.

**Tests:** `tools/traceability/tests/deployment_layout.rs::every_workspace_member_inherits_the_single_workspace_version`.

**Status:** done

## §8 Cross-References zu Bridge-Specs

### §8 §11-Cross-Refs der 7 Bridge-Specs + ffi-loader §6

**Spec:** §8 — Cross-Refs in den Bridge-Spec-§11-Sektionen + FFI-Loader-
§6; erweiterte Cross-Refs zu monitor/observability-otlp/zddsrec/xcdr2.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §9 Versioning

### §9 SemVer-Bump-Regeln

**Spec:** §9 — Patch=Bugfixes Installer/Service-Files, Minor=additive
Distros (NixOS/Alpine/SUSE-OBS), Major=Layout-Änderung
(`/usr/lib/`→`/opt/zerodds/`).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

23 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test-Lauf:

* `cargo test -p zerodds-traceability --test deployment_layout` — 22 Tests
  grün. Ein Test je Klausel aus §2 bis §7; jeder liest die Datei, die die
  Klausel nennt, und prüft ihren Inhalt (FHS-Pfade, Paket-Split,
  systemd-Härtung über alle sieben Units, launchd-Keys über alle sieben
  Plists, MSI-Features, Scoop-Architekturen, cargo-chef-Stufen,
  cargo-dist-Ziele, Workspace-Version über jedes Member-Manifest).
* `cargo test -p zerodds-traceability --test macos_bundle_ci_job` — 1 Test
  grün: der Job `macos-bundle` aus `ci/full.yml`. Die GitLab-CI-Konfiguration
  ist nicht Teil des öffentlichen Spiegels, dieser Test auch nicht.
* `cargo test -p zerodds-websocket-bridge -p zerodds-mqtt-bridge -p zerodds-coap-bridge -p zerodds-amqp-endpoint -p zerodds-grpc-bridge -p zerodds-corba-dds-bridge -p rmw-zerodds-shim -p zerodds-c-api` — Tests grün, 0 failed.

Keine offenen Punkte oder Decision-Records — alle Items `done` / `n/a (informative)`.
