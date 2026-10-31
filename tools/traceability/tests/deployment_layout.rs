//! `zerodds-deployment-1.0` — the packaging tree checked against the
//! clauses that describe it.
//!
//! The deployment spec is a specification about files: which packages
//! exist, which paths they install to, which fields their manifests
//! carry. Those are exactly the statements a test can hold: it opens
//! the file the clause names and asserts the content the clause
//! demands. A test that only checked that the file exists would restate
//! the `**Repo:**` line and prove nothing, so every case below reads the
//! content.
//!
//! Paths are resolved from the repository root, three levels above this
//! crate's manifest (`tools/traceability`).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::{Path, PathBuf};

/// The seven programs of §1.1, in the spelling the packaging files use.
const BRIDGES: [&str; 7] = ["ws", "mqtt", "coap", "amqp", "grpc", "corba", "ros2-shim"];

/// The six of them that run as a service. `zerodds-ros2-shim` is the RMW's
/// diagnostics CLI: rcl loads the RMW into each node, nothing runs beside it.
const SERVICES: [&str; 6] = ["ws", "mqtt", "coap", "amqp", "grpc", "corba"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repository root above tools/traceability")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn contains_all(haystack: &str, needles: &[&str], what: &str) {
    for n in needles {
        assert!(haystack.contains(n), "{what}: missing {n:?}");
    }
}

// ---------------------------------------------------------------------------
// §2 Linux
// ---------------------------------------------------------------------------

/// §2.1 FHS file layout: `/usr/bin`, `/usr/lib/libzerodds.so`,
/// `/usr/lib/pkgconfig/zerodds.pc`, `/usr/include/zerodds.h`,
/// `/etc/zerodds`, `/var/log/zerodds`, `/var/lib/zerodds`,
/// `/usr/share/man`. The runtime directories are created by the
/// tmpfiles fragment, the library and header paths are what the
/// pkg-config descriptor resolves to.
#[test]
fn fhs_layout_paths_are_the_ones_the_clause_names() {
    let tmpfiles = read("packaging/linux/systemd/zerodds-tmpfiles.conf");
    contains_all(
        &tmpfiles,
        &[
            "d /var/log/zerodds",
            "d /var/lib/zerodds",
            "d /etc/zerodds",
            "/etc/zerodds/certs",
        ],
        "tmpfiles fragment",
    );
    // The runtime directories belong to the service user, not to root.
    assert!(
        tmpfiles.lines().filter(|l| l.starts_with("d ")).count() >= 4,
        "one tmpfiles line per runtime directory"
    );
    for line in tmpfiles.lines().filter(|l| l.starts_with("d ")) {
        assert!(
            line.contains("zerodds zerodds"),
            "directory not owned by the service user: {line}"
        );
    }

    let sysusers = read("packaging/linux/systemd/zerodds-sysusers.conf");
    assert!(sysusers.contains("u zerodds"), "system user declared");
    assert!(
        sysusers.contains("/usr/sbin/nologin"),
        "the service user must have no login shell"
    );
    assert!(
        sysusers.contains("/var/lib/zerodds"),
        "home is the state dir"
    );

    let pc = read("packaging/linux/rpm/zerodds.pc");
    contains_all(
        &pc,
        &[
            "prefix=/usr",
            "libdir=${exec_prefix}/lib",
            "includedir=${prefix}/include",
            "-lzerodds",
        ],
        "zerodds.pc",
    );

    // §2.1 also names /etc/zerodds as the config home: one example
    // config per daemon lives beside it.
    for b in BRIDGES {
        let name = if b == "ros2-shim" {
            "ros2.yaml.example".to_string()
        } else {
            format!("{b}-bridged.yaml.example")
        };
        assert!(
            repo_root()
                .join("packaging/linux/configs")
                .join(&name)
                .is_file(),
            "missing example config {name}"
        );
    }
}

/// §2.2.1 Debian packages: the `zerodds-core` / `zerodds-<bridge>` /
/// `zerodds-cli` / `zerodds-dev` split, with every bridge package
/// depending on core at the same binary version.
#[test]
fn debian_control_declares_the_package_split_of_the_clause() {
    let control = read("packaging/linux/deb/control.tmpl");
    assert!(control.starts_with("Source: zerodds"));
    contains_all(
        &control,
        &[
            "Package: zerodds-core",
            "Package: zerodds-cli",
            "Package: zerodds-dev",
        ],
        "deb control",
    );
    for b in BRIDGES {
        let pkg = if b == "ros2-shim" {
            "Package: zerodds-ros2".to_string()
        } else {
            format!("Package: zerodds-{b}-bridge")
        };
        assert!(control.contains(&pkg), "deb control: missing {pkg}");
    }
    // Every non-core package pins core at the identical version — a
    // bridge built against another core ABI is exactly what this stops.
    let dependents = control
        .matches("zerodds-core (= ${binary:Version})")
        .count();
    assert!(
        dependents >= 9,
        "expected every non-core package to pin core, found {dependents}"
    );
    for f in [
        "postinst.tmpl",
        "postrm.tmpl",
        "prerm.tmpl",
        "assemble-debs.sh",
        "publish-deb.yml",
    ] {
        assert!(
            repo_root().join("packaging/linux/deb").join(f).is_file(),
            "missing packaging/linux/deb/{f}"
        );
    }
}

/// §2.2.2 RPM packages: the same split expressed as sub-packages, with
/// the systemd scriptlet requirements the clause implies.
#[test]
fn rpm_spec_declares_the_same_split_and_systemd_scriptlets() {
    let spec = read("packaging/linux/rpm/zerodds.spec");
    assert!(spec.contains("Name:           zerodds"));
    assert!(spec.contains("License:        Apache-2.0"));
    contains_all(
        &spec,
        &["%package core", "%package cli", "%package devel"],
        "rpm spec",
    );
    for b in BRIDGES {
        let pkg = if b == "ros2-shim" {
            "%package ros2".to_string()
        } else {
            format!("%package {b}-bridge")
        };
        assert!(spec.contains(&pkg), "rpm spec: missing {pkg}");
    }
    contains_all(
        &spec,
        &[
            "Requires(post): systemd",
            "Requires(preun): systemd",
            "Requires(postun): systemd",
            "Requires(pre):  shadow-utils",
        ],
        "rpm scriptlet requirements",
    );
    // The workflow that builds the RPM, in the one place GitHub runs it from.
    // This used to point at `packaging/linux/rpm/publish-rpm.yml`, a copy that
    // sat outside `.github/workflows/` and therefore never ran — and the
    // assertion kept it alive. A version sweep then updated the copy to rc.7
    // and left the real workflow on rc.1, which is exactly the failure a
    // traceability test is supposed to prevent rather than cause.
    let workflow = read(".github/workflows/publish-rpm.yml");
    contains_all(
        &workflow,
        &["zerodds.spec", "rpmbuild", "zd_version", "zd_release"],
        "rpm publish workflow",
    );
    assert!(
        !repo_root()
            .join("packaging/linux/rpm/publish-rpm.yml")
            .exists(),
        "the dead copy is back; `.github/workflows/publish-rpm.yml` is the one that runs"
    );
}

/// §2.2.3 Arch PKGBUILD: the AUR fields plus the daemon and CLI lists
/// the package installs.
#[test]
fn arch_pkgbuild_carries_the_aur_fields_and_the_daemon_list() {
    let pkgbuild = read("packaging/linux/arch/PKGBUILD");
    contains_all(
        &pkgbuild,
        &[
            "pkgname=zerodds",
            "pkgrel=",
            "arch=('x86_64' 'aarch64')",
            "license=('Apache')",
            "makedepends=('rust' 'cargo')",
            "sha512sums=",
        ],
        "PKGBUILD",
    );
    for b in BRIDGES {
        let bin = if b == "ros2-shim" {
            "zerodds-ros2-shim".to_string()
        } else {
            format!("zerodds-{b}-bridged")
        };
        assert!(pkgbuild.contains(&bin), "PKGBUILD: missing {bin}");
    }
    assert!(
        pkgbuild.contains("package()"),
        "PKGBUILD needs a package() function"
    );
}

/// §2.2.4 AppImage per daemon, statically linked against musl. The
/// build script targets musl and produces one AppImage per daemon; the
/// AppRun template execs the binary the build substitutes.
#[test]
fn appimage_build_targets_musl_and_covers_every_daemon() {
    let build = read("packaging/linux/appimage/build.sh");
    assert!(
        build.contains("x86_64-unknown-linux-musl"),
        "AppImage build must default to a musl target"
    );
    assert!(build.contains("appimagetool"), "AppImage tool invoked");
    for b in BRIDGES {
        let bin = if b == "ros2-shim" {
            "zerodds-ros2-shim".to_string()
        } else {
            format!("zerodds-{b}-bridged")
        };
        assert!(build.contains(&bin), "AppImage build: missing {bin}");
    }

    let apprun = read("packaging/linux/appimage/AppRun.template");
    assert!(
        apprun.contains("exec \"$HERE/usr/bin/${BIN}\" \"$@\""),
        "AppRun must exec the bundled binary with the caller's arguments"
    );
    // No code reads a configuration-directory variable: a program takes
    // `--config PATH` or the system layout. An export here would look like a
    // setting and change nothing.
    assert!(
        !apprun.contains("export ZERODDS_CONFIG"),
        "AppRun must not export a configuration selector no program reads"
    );
}

/// §2.3 systemd unit per bridge: `Type=notify`, `ExecStart`,
/// `ExecReload` on SIGHUP, `KillMode=mixed`, and the hardening set
/// `NoNewPrivileges` / `ProtectSystem` / `PrivateTmp` / `LimitNOFILE`.
/// Checked on every one of the six units, not on a sample.
#[test]
fn every_systemd_unit_carries_the_hardening_set_of_the_clause() {
    for b in SERVICES {
        let name = format!("zerodds-{b}-bridged.service");
        let unit = read(&format!("packaging/linux/systemd/{name}"));
        contains_all(
            &unit,
            &[
                "Type=notify",
                "ExecStart=/usr/bin/zerodds-",
                "ExecReload=/bin/kill -HUP $MAINPID",
                "KillMode=mixed",
                "NoNewPrivileges=true",
                "ProtectSystem=strict",
                "PrivateTmp=true",
                "LimitNOFILE=",
                "User=zerodds",
                "[Install]",
            ],
            &name,
        );
        assert!(
            unit.contains("ReadWritePaths=/var/log/zerodds /var/lib/zerodds"),
            "{name}: ProtectSystem=strict needs the two writable paths back"
        );
    }
}

/// The ROS-2 diagnostics CLI has no unit, no plist and no service
/// scriptlet. Its old unit ran `zerodds-ros2-shim --watch`, an option the
/// binary never had, so the service failed at every start.
#[test]
fn the_ros2_diagnostics_cli_is_not_installed_as_a_service() {
    let root = repo_root();
    assert!(
        !root
            .join("packaging/linux/systemd/zerodds-ros2-shim.service")
            .exists()
    );
    assert!(
        !root
            .join("packaging/macos/launchd/org.zerodds.ros2-shim.plist")
            .exists()
    );
    let spec = read("packaging/linux/rpm/zerodds.spec");
    assert!(!spec.contains("zerodds-ros2-shim.service"), "rpm spec");
    let docker = read("packaging/docker/ros2-shim/Dockerfile");
    assert!(!docker.contains("--watch"), "ros2 image");
    // One package owns the binary; two would clash in dpkg and rpm.
    let debs = read("packaging/linux/deb/assemble-debs.sh");
    let cli_tools = debs
        .split_once("for tool in")
        .and_then(|(_, rest)| rest.split_once("; do"))
        .map(|(list, _)| list)
        .expect("the zerodds-cli tool list");
    assert!(
        !cli_tools.contains("ros2-shim"),
        "deb: zerodds-ros2 owns it"
    );
    assert_eq!(
        debs.matches("$stage/usr/bin/zerodds-ros2-shim").count(),
        1,
        "deb: one owner"
    );
    assert_eq!(
        spec.matches("%{_bindir}/zerodds-ros2-shim").count(),
        1,
        "rpm: one owner"
    );
}

// ---------------------------------------------------------------------------
// §3 macOS
// ---------------------------------------------------------------------------

/// §3.1 macOS layout: `/usr/local/{bin,lib,include,etc,var}/zerodds`
/// and `/Library/LaunchDaemons/org.zerodds.*.plist`. The plists carry
/// the `/usr/local` prefix; the pkg script installs under it.
#[test]
fn macos_layout_uses_the_usr_local_prefix() {
    let plist = read("packaging/macos/launchd/org.zerodds.ws-bridged.plist");
    contains_all(
        &plist,
        &[
            "/usr/local/bin/zerodds-ws-bridged",
            "/usr/local/etc/zerodds/",
            "/usr/local/var/log/zerodds/",
            "/usr/local/var/lib/zerodds",
        ],
        "launchd plist",
    );
    let formula = read("packaging/macos/homebrew/zerodds.rb");
    assert!(
        formula.contains("class Zerodds < Formula"),
        "main formula class name"
    );
    assert!(
        repo_root()
            .join("packaging/macos/pkg/build-pkg.sh")
            .is_file(),
        "pkg build script"
    );
}

/// §3.2.1 Homebrew tap: one formula per daemon, each with a
/// `service do` block so `brew services` can run it.
#[test]
fn every_homebrew_formula_declares_a_brew_service() {
    let formulae = [
        "zerodds-ws-bridge.rb",
        "zerodds-mqtt-bridge.rb",
        "zerodds-coap-bridge.rb",
        "zerodds-amqp-bridge.rb",
        "zerodds-grpc-bridge.rb",
        "zerodds-corba-bridge.rb",
        "zerodds-ros2.rb",
    ];
    // Homebrew rejects a formula without a URL ("formula requires at least
    // a URL"), so every sub-formula names the meta formula's bundle: the
    // same url lines, one download for all of them.
    let meta = read("packaging/macos/homebrew/zerodds.rb");
    let meta_urls: Vec<&str> = meta
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("url \""))
        .collect();
    assert_eq!(meta_urls.len(), 2, "zerodds.rb: one bundle URL per CPU");
    for f in formulae {
        let rb = read(&format!("packaging/macos/homebrew/{f}"));
        assert!(rb.contains("class "), "{f}: formula class");
        assert!(rb.contains("license \"Apache-2.0\""), "{f}: license");
        assert!(
            rb.contains("depends_on \"zerodds\""),
            "{f}: sub-formula depends on the core formula"
        );
        // The binary ships with `zerodds`; this formula installs the
        // example config from the same bundle and must not overwrite a
        // configuration the user already has.
        assert!(rb.contains("def install"), "{f}: install method");
        let urls: Vec<&str> = rb
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with("url \""))
            .collect();
        assert_eq!(urls, meta_urls, "{f}: must name the meta formula's bundle");
        assert_eq!(
            rb.matches("sha256 \"").count(),
            2,
            "{f}: one sha256 per bundle URL"
        );
        assert!(
            rb.contains("unless (etc/\"zerodds\"/config).exist?"),
            "{f}: an existing configuration must survive an upgrade"
        );
        if f != "zerodds-ros2.rb" {
            assert!(
                rb.contains("service do"),
                "{f}: brew-services integration missing"
            );
            assert!(rb.contains("keep_alive true"), "{f}: keep_alive");
            assert!(
                rb.contains("--config"),
                "{f}: the service must pass the configuration path"
            );
        }
    }
    assert!(
        repo_root()
            .join("packaging/github-actions/render-homebrew.sh")
            .is_file(),
        "the tap renderer named by the clause"
    );
}

/// §3.2.1 delivered: the packer (release.yml macOS jobs, and the
/// `macos-bundle` job of ci/full.yml) produces the archive layout the meta
/// formula installs. The GitLab job is checked in `macos_bundle_ci_job.rs`;
/// the public mirror carries neither `ci/full.yml` nor that file.
#[test]
fn the_bundle_packer_lays_out_what_the_formula_installs() {
    // The packer lays out exactly what zerodds.rb reads: bin/, lib/libzerodds.*,
    // include/zerodds.h + include/zerodds/*, lib/pkgconfig/zerodds.pc,
    // share/zerodds/configs/*, the man pages and the launchd plists.
    let packer = read("packaging/macos/bundle/build-bundle.sh");
    contains_all(
        &packer,
        &[
            "\"$STAGE/bin/$bin\"",
            "libzerodds.dylib",
            "include/zerodds.h",
            "include/zerodds/dds.hpp",
            "include/dds",
            "lib/pkgconfig/zerodds.pc",
            "share/zerodds/configs/",
            "share/man/man1/",
            "share/man/man5/",
            "share/launchd/",
            "tar -czf",
            "shasum -a 256",
        ],
        "build-bundle.sh",
    );
    // One archive per target, named as the formula's URL expects.
    assert!(
        packer.contains("zerodds-${VERSION}-${TARGET}.tar.gz"),
        "build-bundle.sh must name the archive zerodds-<v>-<target>.tar.gz"
    );
}

/// §3.2.2 signed and notarized `.pkg`: the build script signs with a
/// Developer ID Installer identity and submits to notarytool.
#[test]
fn macos_pkg_build_signs_and_notarizes() {
    let sh = read("packaging/macos/pkg/build-pkg.sh");
    contains_all(
        &sh,
        &[
            "pkgbuild",
            "productsign --sign \"Developer ID Installer",
            "xcrun notarytool submit",
        ],
        "build-pkg.sh",
    );
}

/// §3.3 launchd plist per bridge: `Label=org.zerodds.<bridge>`,
/// `ProgramArguments`, `RunAtLoad`, `KeepAlive`, stdout/stderr paths,
/// `UserName`. All six, not a sample.
#[test]
fn every_launchd_plist_carries_the_keys_of_the_clause() {
    for b in SERVICES {
        let label = format!("org.zerodds.{b}-bridged");
        let plist = read(&format!("packaging/macos/launchd/{label}.plist"));
        contains_all(
            &plist,
            &[
                "<key>Label</key>",
                &format!("<string>{label}</string>"),
                "<key>ProgramArguments</key>",
                "<key>RunAtLoad</key>",
                "<key>KeepAlive</key>",
                "<key>StandardOutPath</key>",
                "<key>StandardErrorPath</key>",
                "<key>UserName</key>",
            ],
            &label,
        );
        assert!(
            plist.contains("<string>_zerodds</string>"),
            "{label}: must run as the _zerodds service user"
        );
    }
}

// ---------------------------------------------------------------------------
// §4 Windows
// ---------------------------------------------------------------------------

/// §4.1 Windows layout: `%PROGRAMFILES%\ZeroDDS\{bin,include,lib,doc}`
/// and `%PROGRAMDATA%\ZeroDDS\{*.yaml,certs,logs}`. The WiX source
/// defines the install layout, the service scripts default to the same
/// two roots.
#[test]
fn windows_layout_splits_program_files_from_program_data() {
    let wxs = read("packaging/windows/msi/zerodds.wxs");
    contains_all(
        &wxs,
        &["ProgramFiles64Folder", "CommonAppDataFolder", "ZeroDDS"],
        "zerodds.wxs",
    );
    let install = read("packaging/windows/services/Install-Services.ps1");
    assert!(
        install.contains("$env:PROGRAMFILES\\ZeroDDS"),
        "install root"
    );
    assert!(
        install.contains("$env:PROGRAMDATA\\ZeroDDS"),
        "config/state root"
    );
}

/// §4.2.1 MSI installer via WiX, with one optional feature per bridge
/// and a major-upgrade rule.
#[test]
fn msi_defines_one_optional_feature_per_bridge() {
    let wxs = read("packaging/windows/msi/zerodds.wxs");
    assert!(wxs.contains("<MajorUpgrade"), "upgrade rule");
    assert!(wxs.contains("UpgradeCode="), "stable upgrade code");
    for f in [
        "WsBridge",
        "MqttBridge",
        "CoapBridge",
        "AmqpBridge",
        "GrpcBridge",
        "CorbaBridge",
        "Ros2Shim",
    ] {
        assert!(
            wxs.contains(&format!("Feature Id=\"{f}\"")),
            "MSI: missing feature {f}"
        );
    }
    assert!(
        wxs.contains("Feature Id=\"Core\""),
        "MSI: core feature missing"
    );
}

/// §4.2.2 Scoop manifest: a per-architecture download with a checksum
/// and the shipped binaries in `bin`.
#[test]
fn scoop_manifest_has_both_architectures_and_a_bin_list() {
    let raw = read("packaging/windows/scoop/zerodds.json");
    // Parsing is the point: a manifest Scoop cannot read is not a
    // manifest. No JSON dependency here, so the shape is checked on the
    // text plus a brace balance.
    assert_eq!(
        raw.matches('{').count(),
        raw.matches('}').count(),
        "unbalanced JSON braces"
    );
    contains_all(
        &raw,
        &[
            "\"$schema\"",
            "\"version\"",
            "\"license\": \"Apache-2.0\"",
            "\"architecture\"",
            "\"64bit\"",
            "\"arm64\"",
            "\"extract_dir\"",
            "\"bin\"",
            "x86_64-pc-windows-msvc",
            "aarch64-pc-windows-msvc",
        ],
        "scoop manifest",
    );
    let arch_block = raw
        .split_once("\"architecture\"")
        .expect("architecture block")
        .1;
    let arch_block = &arch_block[..arch_block.find("\n  \"").unwrap_or(arch_block.len())];
    assert_eq!(
        arch_block.matches("\"hash\"").count(),
        2,
        "one checksum per architecture"
    );
    for b in ["ws", "mqtt", "coap", "amqp", "grpc", "corba"] {
        assert!(
            raw.contains(&format!("zerodds-{b}-bridged.exe")),
            "scoop bin list: missing {b}"
        );
    }
}

/// §4.2.3 Chocolatey package: nuspec metadata plus the two tool scripts
/// Chocolatey runs on install and uninstall.
#[test]
fn chocolatey_package_has_its_nuspec_metadata_and_both_scripts() {
    let nuspec = read("packaging/windows/chocolatey/zerodds.nuspec");
    contains_all(
        &nuspec,
        &[
            "<id>zerodds</id>",
            "<version>",
            "<licenseUrl>",
            "<projectUrl>https://zerodds.org</projectUrl>",
            "<requireLicenseAcceptance>false</requireLicenseAcceptance>",
        ],
        "nuspec",
    );
    for f in ["chocolateyinstall.ps1", "chocolateyuninstall.ps1"] {
        assert!(
            repo_root()
                .join("packaging/windows/chocolatey/tools")
                .join(f)
                .is_file(),
            "missing chocolatey tools/{f}"
        );
    }
}

/// §4.3 one Windows service per bridge, with binPath, automatic start,
/// a display name and failure recovery — and an uninstall script that
/// removes the same set.
#[test]
fn windows_service_scripts_register_and_remove_the_same_bridge_set() {
    let install = read("packaging/windows/services/Install-Services.ps1");
    let uninstall = read("packaging/windows/services/Uninstall-Services.ps1");
    let services = [
        "ZeroDDSWSBridge",
        "ZeroDDSMQTTBridge",
        "ZeroDDSCoAPBridge",
        "ZeroDDSAMQPBridge",
        "ZeroDDSGrpcBridge",
        "ZeroDDSCORBABridge",
    ];
    for s in services {
        assert!(install.contains(s), "install script: missing {s}");
        assert!(uninstall.contains(s), "uninstall script: missing {s}");
    }
    contains_all(
        &install,
        &[
            "sc.exe create",
            "binPath=",
            "start=",
            "auto",
            "sc.exe description",
            "sc.exe failure",
        ],
        "Install-Services.ps1",
    );
    assert!(
        install.contains("actions= restart/"),
        "the clause asks for failure recovery, not just registration"
    );
    assert!(
        install.contains("Administrator"),
        "service registration must demand elevation"
    );
}

// ---------------------------------------------------------------------------
// §5 Container
// ---------------------------------------------------------------------------

/// §5.1 one image per daemon plus the CLI image, and the ROS shim image
/// parameterised over the three ROS distros.
#[test]
fn one_dockerfile_per_daemon_plus_cli_and_the_three_ros_distros() {
    for dir in [
        "ws-bridged",
        "mqtt-bridged",
        "coap-bridged",
        "amqp-bridged",
        "grpc-bridged",
        "corba-bridged",
        "ros2-shim",
        "cli",
    ] {
        assert!(
            repo_root()
                .join("packaging/docker")
                .join(dir)
                .join("Dockerfile")
                .is_file(),
            "missing packaging/docker/{dir}/Dockerfile"
        );
    }
    let ros = read("packaging/docker/ros2-shim/Dockerfile");
    for distro in ["humble", "iron", "jazzy"] {
        assert!(ros.contains(distro), "ros2-shim image: missing {distro}");
    }
    assert!(
        ros.contains("ARG ROS_DISTRO="),
        "the distro must be a build argument, not a fork of the file"
    );
}

/// §5.2 multi-stage Dockerfile with cargo-chef: a chef/planner/builder
/// chain and a slim runtime stage that drops to a non-root user.
/// Checked on every daemon image.
#[test]
fn every_daemon_image_is_cargo_chef_multi_stage_and_runs_non_root() {
    for dir in [
        "ws-bridged",
        "mqtt-bridged",
        "coap-bridged",
        "amqp-bridged",
        "grpc-bridged",
        "corba-bridged",
    ] {
        let df = read(&format!("packaging/docker/{dir}/Dockerfile"));
        contains_all(
            &df,
            &[
                "AS chef",
                "cargo install cargo-chef",
                "AS planner",
                "cargo chef prepare",
                "AS builder",
                "cargo chef cook --release",
                "FROM debian:bookworm-slim",
                "USER zerodds",
            ],
            dir,
        );
        assert!(
            df.contains("useradd --system"),
            "{dir}: the runtime user must be a system account"
        );
        // The last stage is the slim one — a build image left as the
        // final stage would ship the whole toolchain.
        let last_from = df
            .lines()
            .filter(|l| l.starts_with("FROM "))
            .next_back()
            .unwrap_or_default();
        assert!(
            last_from.contains("debian:bookworm-slim"),
            "{dir}: final stage is {last_from:?}, not the slim runtime"
        );
    }
}

/// §5.3 compose example: the bridge services plus mosquitto, on
/// `network_mode: host` because RTPS multicast does not survive a NAT
/// bridge network.
#[test]
fn docker_compose_runs_the_bridges_on_the_host_network() {
    let compose = read("packaging/docker/docker-compose.yml");
    contains_all(
        &compose,
        &[
            "ws-bridge:",
            "mqtt-bridge:",
            "mosquitto",
            "network_mode: host",
        ],
        "docker-compose.yml",
    );
    // Every zerodds service must be on the host network; a single
    // service left on the default bridge silently loses discovery.
    let host_lines = compose.matches("network_mode: host").count();
    assert!(
        host_lines >= 2,
        "expected host networking on every bridge service, found {host_lines}"
    );
    assert!(
        compose.contains("no-new-privileges:true"),
        "the compose defaults must keep the hardening of §2.3"
    );
}

// ---------------------------------------------------------------------------
// §6 / §7 Release
// ---------------------------------------------------------------------------

/// §6 `[workspace.metadata.dist]`: the installer set, the target list
/// and the checksum/signing arrangement. The MSI installer is not in
/// cargo-dist's `installers` list — it is built by the `package-msi`
/// job from our own WiX source — and that is asserted rather than
/// glossed over.
#[test]
fn cargo_dist_block_declares_the_installers_targets_and_signing() {
    let root = read("Cargo.toml");
    let dist = root
        .split_once("[workspace.metadata.dist]")
        .expect("[workspace.metadata.dist] block")
        .1;
    contains_all(
        &dist[..dist.find("\n[").unwrap_or(dist.len())],
        &[
            "cargo-dist-version",
            "ci = [\"github\"]",
            "installers = [\"shell\", \"powershell\", \"homebrew\"]",
            "checksum = \"sha512\"",
            "homebrew-tap = \"zero-objects/homebrew-zerodds\"",
        ],
        "[workspace.metadata.dist]",
    );
    for t in [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-unknown-linux-musl",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "aarch64-pc-windows-msvc",
    ] {
        assert!(dist.contains(t), "dist targets: missing {t}");
    }

    // MSI and the signatures live in the release workflow, not in the
    // cargo-dist installer list.
    let release = read(".github/workflows/release.yml");
    contains_all(
        &release,
        &["package-msi:", "minisign", "cosign"],
        "release.yml",
    );
    for f in ["render-homebrew.sh", "sign-artifacts.sh"] {
        assert!(
            repo_root()
                .join("packaging/github-actions")
                .join(f)
                .is_file(),
            "missing packaging/github-actions/{f}"
        );
    }
}

/// §7 one workspace version, inherited by every member: a crate that
/// pins its own version would ship out of step with the rest.
#[test]
fn every_workspace_member_inherits_the_single_workspace_version() {
    let root_toml = read("Cargo.toml");
    let pkg = root_toml
        .split_once("[workspace.package]")
        .expect("[workspace.package] block")
        .1;
    let version_line = pkg
        .lines()
        .find(|l| l.trim_start().starts_with("version = "))
        .expect("workspace version");
    let version = version_line
        .split('"')
        .nth(1)
        .expect("quoted version")
        .to_string();
    assert!(
        version.starts_with("1.0.0"),
        "unexpected workspace version {version}"
    );

    // Every member listed in the workspace must inherit it.
    let members: Vec<String> = root_toml
        .split_once("members = [")
        .expect("members list")
        .1
        .split(']')
        .next()
        .expect("members list end")
        .lines()
        .filter_map(|l| l.trim().strip_prefix('"'))
        .filter_map(|l| l.split('"').next())
        .map(str::to_string)
        .collect();
    assert!(members.len() > 50, "found only {} members", members.len());

    let mut checked = 0usize;
    for m in &members {
        let manifest = repo_root().join(m).join("Cargo.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        assert!(
            text.contains("version.workspace = true"),
            "{m}/Cargo.toml pins its own version instead of inheriting"
        );
        checked += 1;
    }
    assert_eq!(checked, members.len(), "every member manifest must be read");
}
