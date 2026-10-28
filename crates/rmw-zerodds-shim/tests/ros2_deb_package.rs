// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// `ros-<distro>-rmw-zerodds-cpp` — the RMW implementation rmw_zerodds_cpp as
// a Debian package (spec `zerodds-ros2-bridge-1.0.md` §11: "One .deb package
// per ROS distro: `ros-humble-rmw-zerodds-cpp`, …", plus the images with the
// RMW preinstalled).
//
// The package can only be BUILT on a host with a ROS 2 install — the RMW
// links the distro's rmw/rcutils and matches its glibc — so nothing here
// builds one. What can be checked without ROS 2 is checked here: the package
// name the clause names, the ROS install layout its files sit in, the fact
// that the file list comes from the ament install instead of a second,
// hand-kept list, the CI job that builds it and the image that installs it.
// A drift between those four files is a broken package on the next build; a
// drift in the library itself is caught by rmw_c/run_ament_package_test.sh on
// the ROS host, not here. The CI job is checked in `ros2_deb_ci_job.rs`; the
// public mirror carries neither `ci/full.yml` nor that file.

#![allow(clippy::expect_used, clippy::panic, missing_docs)]

use std::path::PathBuf;

const SCRIPT: &str = "packaging/linux/deb/build-rmw-deb.sh";
const DOCKERFILE: &str = "packaging/docker/ros2-shim/Dockerfile";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repository root above crates/rmw-zerodds-shim")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn contains_all(haystack: &str, needles: &[&str], what: &str) {
    for n in needles {
        assert!(haystack.contains(n), "{what}: missing {n:?}");
    }
}

/// §11 "One .deb package per ROS distro", ros-<distro>-rmw-zerodds-cpp: the
/// package name is built from the distro, and the distro is a parameter with
/// the §10 set as its domain. A name outside it is refused rather than turned
/// into a package whose install prefix nothing sources.
#[test]
fn the_package_is_named_after_the_distro_and_the_distro_is_checked() {
    let script = read(SCRIPT);
    contains_all(
        &script,
        &[
            r#"PKG="ros-${DISTRO}-rmw-zerodds-cpp""#,
            r#"DISTRO="jazzy""#,
            r#"[ "$known" = yes ] || die "--distro '$DISTRO' is not a ROS distro this tree builds"#,
            r#"DEB="$OUT/${PKG}_${VERSION}_${DEB_ARCH}.deb""#,
        ],
        SCRIPT,
    );
    // zerodds-ros2-bridge-1.0.md §10: "Pro Distro eigenes Build-Target". The
    // supported set drops the EOL Iron and adds Kilted, the LTS between Jazzy
    // and Rolling.
    let distros_line = script
        .lines()
        .find(|l| l.starts_with("DISTROS="))
        .expect("the script's distro list");
    let distros: Vec<&str> = distros_line
        .trim_start_matches("DISTROS=")
        .trim_matches('"')
        .split_whitespace()
        .collect();
    assert_eq!(
        distros,
        ["humble", "jazzy", "kilted", "rolling"],
        "the distro set must be the supported one"
    );
    // The distro reaches the install prefix and the package name from the same
    // variable; a hard-coded prefix would build a jazzy package for humble.
    contains_all(
        &script,
        &[
            r#"INSTALL_PREFIX_ROOT="/opt/ros""#,
            r#"INSTALL_PREFIX="${INSTALL_PREFIX_ROOT}/${DISTRO}""#,
        ],
        SCRIPT,
    );
}

/// §11 "installing librmw_zerodds_cpp.so, the ament index entries and the
/// package.xml the way ROS 2 RMW packages are laid out": the files live in the
/// distro's own ROS prefix, and the script refuses to build a package whose
/// staging tree lacks one of them.
#[test]
fn the_package_carries_the_library_the_index_entries_and_package_xml() {
    let script = read(SCRIPT);
    contains_all(
        &script,
        &[
            r#"IDX="$stage$INSTALL_PREFIX/share/ament_index/resource_index""#,
            r#"for res in packages rmw_typesupport; do"#,
            r#"[ -f "$IDX/$res/rmw_zerodds_cpp" ]"#,
            r#"PKG_XML="$stage$INSTALL_PREFIX/share/rmw_zerodds_cpp/package.xml""#,
            r#"LIB="$stage$INSTALL_PREFIX/lib/librmw_zerodds_cpp.so""#,
            // The two libraries the RMW resolves through RPATH $ORIGIN.
            r#"for so in librmw_zerodds.so libzerodds.so; do"#,
        ],
        SCRIPT,
    );
    // Every one of those checks is a `die`, not a warning: a .deb that ships
    // an RMW rcl cannot find is worse than no package.
    for guard in [
        r#"|| die "$LIB not installed"#,
        r#"|| die "$IDX/$res/rmw_zerodds_cpp missing"#,
        r#"|| die "$stage$INSTALL_PREFIX/lib/$so missing"#,
        r#"|| die "the ament install has no share/rmw_zerodds_cpp/package.xml"#,
    ] {
        assert!(script.contains(guard), "{SCRIPT}: missing guard {guard:?}");
    }
    // The ament install is the only producer of the file list — the rule
    // assemble-debs.sh follows for zerodds-dev — and not one per-file copy
    // list that can drift from what the ament package installs.
    contains_all(
        &script,
        &[
            r#"DESTDIR="$stage" cmake --install "$ROOT/build""#,
            r#"dpkg-deb --build --root-owner-group "$stage" "$DEB""#,
            // The cmake build tree stays out of the package file root, and the
            // script proves it: a .deb with CMakeCache.txt in it is not a
            // package anybody can reproduce.
            r#"staged_top="$(cd "$stage" && ls -A)""#,
            r#"[ "$staged_top" = "opt" ]"#,
            "Package: $PKG",
        ],
        SCRIPT,
    );
    assert!(
        !script.contains("install -Dm"),
        "{SCRIPT}: a per-file copy list next to the ament install tree"
    );
    // The Rust libraries come out of the workspace's own target dir, which the
    // ament project reads by path; a caller's CARGO_TARGET_DIR must not move
    // them out from under it.
    contains_all(
        &script,
        &[
            r#"CARGO_TARGET_DIR="$REPO_ROOT/target""#,
            "cargo build --release -p rmw-zerodds-shim -p zerodds-c-api",
            r#"ZERODDS_HOME="$REPO_ROOT" cmake -S "$REPO_ROOT/crates/rmw-zerodds-shim/ament""#,
        ],
        SCRIPT,
    );
    // Nothing to start, nothing to reload: a library-only package with no
    // maintainer scripts, like every other ros-<distro>-* package.
    assert!(
        !script.contains("DEBIAN/postinst") && !script.contains("DEBIAN/prerm"),
        "{SCRIPT}: no maintainer scripts"
    );
}

/// The round trip the CI job adds after the load check. Nothing here runs
/// rclpy; the test pins the contract the two processes keep, so the job's
/// `pub` and `sub` invocations cannot drift from the script.
#[test]
fn the_pubsub_check_is_two_processes_on_one_reliable_keep_all_topic() {
    let script = read("packaging/linux/deb/rmw-pubsub-check.py");
    contains_all(
        &script,
        &[
            "from std_msgs.msg import String",
            "ReliabilityPolicy.RELIABLE",
            "HistoryPolicy.KEEP_ALL",
            "COUNT = 20",
            "PERIOD_S = 0.1",
            "TIMEOUT_S = 30.0",
            "def run_pub",
            "def run_sub",
            // The subscriber is the process that decides: 0 only with all
            // twenty distinct payloads, 1 with the missing numbers otherwise.
            "missing = [i for i in range(COUNT) if i not in seen]",
            "return 1",
            "return run_pub() if argv[1] == \"pub\" else run_sub()",
        ],
        "packaging/linux/deb/rmw-pubsub-check.py",
    );
}

/// The two endpoint-info scripts the CI job runs after the round trip: the
/// type hash in USER_DATA against the package itself and both ways against
/// the image's rmw_fastrtps_cpp, and Rolling's service endpoint info. Each
/// exits non-zero on its own FAIL line. That the job runs them and fails on
/// any is checked in `ros2_deb_ci_job.rs`.
#[test]
fn the_endpoint_info_scripts_exit_non_zero_on_a_mismatch() {
    let type_hash = read("packaging/linux/deb/rmw-type-hash-check.py");
    contains_all(
        &type_hash,
        &[
            "env=dict(os.environ, RMW_IMPLEMENTATION=sub_rmw)",
            "os.environ[\"RMW_IMPLEMENTATION\"] = pub_rmw",
            "str(subs[0].topic_type_hash).startswith(\"RIHS01_\")",
            "str(subs[0].topic_type_hash) == str(pubs[0].topic_type_hash)",
            "return 0 if ok else 1",
        ],
        "packaging/linux/deb/rmw-type-hash-check.py",
    );
    let service_info = read("packaging/linux/deb/rmw-service-info-check.py");
    contains_all(
        &service_info,
        &[
            "node.get_servers_info_by_service(SERVICE)",
            "node.get_clients_info_by_service(SERVICE)",
            "servers[0].node_name == SERVER_NODE",
            "clients[0].node_name == CLIENT_NODE",
            "SERVICE_TYPE = \"rcl_interfaces/srv/GetParameters\"",
            "return 0 if ok else 1",
        ],
        "packaging/linux/deb/rmw-service-info-check.py",
    );
}

/// §11 "(with the ros2 image as base + ZeroDDS RMW preinstalled)": the image
/// installs the package rather than a copy of the same tree under a prefix of
/// its own, so the image and the package cannot disagree about what an
/// installed rmw_zerodds_cpp is.
#[test]
fn the_ros2_image_installs_the_rmw_from_the_package() {
    let docker = read(DOCKERFILE);
    contains_all(
        &docker,
        &[
            "ARG ROS_DISTRO=jazzy",
            "FROM ros:${ROS_DISTRO}-ros-base AS builder",
            "packaging/linux/deb/build-rmw-deb.sh",
            "dpkg -i /tmp/dist-deb/ros-${ROS_DISTRO}-rmw-zerodds-cpp_*.deb",
            "ENV RMW_IMPLEMENTATION=rmw_zerodds_cpp",
            "ENV AMENT_PREFIX_PATH=/opt/ros/${ROS_DISTRO}",
            "ENV LD_LIBRARY_PATH=/opt/ros/${ROS_DISTRO}/lib",
        ],
        DOCKERFILE,
    );
    assert!(
        !docker.contains("/opt/zerodds"),
        "{DOCKERFILE}: the RMW now lives in the distro's ROS prefix"
    );
}

/// The other half of the pair must not go on claiming that the library is in
/// no package: the description of `zerodds-ros2` in the deb control template
/// names the package that owns it.
#[test]
fn the_ros2_deb_description_points_at_the_rmw_package() {
    let control = read("packaging/linux/deb/control.tmpl");
    let ros2 = control
        .split("Package: zerodds-ros2")
        .nth(1)
        .expect("the zerodds-ros2 stanza in the deb control template")
        .split("Package: ")
        .next()
        .expect("the end of the zerodds-ros2 stanza");
    assert!(
        ros2.contains("ros-<distro>-rmw-zerodds-cpp"),
        "control.tmpl: the zerodds-ros2 description does not name the package that ships the RMW"
    );
    assert!(
        !ros2.contains("is not in this package: the ament package"),
        "control.tmpl: the RMW is in a package now"
    );
}

/// The script finds the repository from its own directory: one `..` per
/// path component between the root and the script. Two of the three made
/// `REPO_ROOT` the `packaging/` directory, and the image build stopped at
/// `sed: can't read /build/packaging/Cargo.toml` — the version it reads
/// from the workspace manifest.
#[test]
fn the_script_walks_up_to_the_repository_root() {
    let depth = std::path::Path::new(SCRIPT)
        .parent()
        .expect("the script sits in a directory")
        .components()
        .count();
    let up = vec![".."; depth].join("/");
    contains_all(
        &read(SCRIPT),
        &[&format!(r#"REPO_ROOT="$(cd "$HERE/{up}" && pwd)""#)],
        SCRIPT,
    );
    assert!(
        repo_root().join("Cargo.toml").is_file(),
        "the walked-up directory holds the workspace manifest"
    );
}
