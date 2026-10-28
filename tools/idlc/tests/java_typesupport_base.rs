// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-Java-PSM 1.0 §7.5.1: "each type to be published or subscribed with DDS is
//! represented by a class extending org.omg.dds.type.TypeSupport". The
//! `Support_<tag>` classes of the generated `ZdTypeSupports.java` extend that
//! class and keep implementing `org.zerodds.cdr.TopicTypeSupport`.
//!
//! The test generates the Java for `java-support/support.idl`, compiles it with
//! `-Xlint:all -Werror` against the sources of the DDS Java binding and of the
//! wire runtime, and runs a small program that uses the classes through the
//! base class. It needs a JDK (`javac`, `java`); without one it says so and
//! does nothing, so a host without a JDK does not show it as run.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::path::Path;
use std::process::Command;

fn idlc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"))
}

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn usable(command: &str) -> bool {
    Command::new(command)
        .arg("-version")
        .output()
        .is_ok_and(|out| out.status.success())
}

const CHECK: &str = r#"
public final class Check {
    public static void main(String[] args) {
        org.omg.dds.type.TypeSupport<ZdTypes.bridge.Point> base =
                ZdTypeSupports.Support_bridge_Point.INSTANCE;
        if (base.getType() != ZdTypes.bridge.Point.class) throw new AssertionError("getType");
        if (base.newData() == null) throw new AssertionError("newData");
        if (!"bridge::Point".equals(base.getTypeName())) throw new AssertionError("getTypeName " + base.getTypeName());
        org.zerodds.cdr.TopicTypeSupport<ZdTypes.bridge.Point> codec =
                ZdTypeSupports.Support_bridge_Point.INSTANCE;
        ZdTypes.bridge.Point point = base.newData();
        point.id = 7;
        point.reading = 9;
        ZdTypes.bridge.Point back = codec.decode(codec.encode(point));
        if (back.id != 7 || back.reading != 9) throw new AssertionError("round trip");
        System.out.println("TYPESUPPORT_BASE_OK");
    }
}
"#;

#[test]
fn generated_type_supports_extend_the_psm_type_support() {
    if !usable("javac") || !usable("java") {
        eprintln!("NOT RUN: no JDK (javac, java) on the path.");
        return;
    }
    for encoding in ["xcdr1", "xcdr2"] {
        let dir =
            Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("java-typesupport-{encoding}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("classes")).unwrap();
        let generated = idlc()
            .args([
                "generate",
                "--lang",
                "java",
                "--java-mapping",
                "omg",
                "--encoding",
                encoding,
                "--out",
            ])
            .arg(&dir)
            .arg(root().join("tools/idlc/tests/java-support/support.idl"))
            .output()
            .unwrap();
        assert!(
            generated.status.success(),
            "idlc generate ({encoding}): {}",
            String::from_utf8_lossy(&generated.stderr)
        );
        std::fs::write(dir.join("Check.java"), CHECK).unwrap();
        let sourcepath = format!(
            "{}:{}",
            root()
                .join("crates/java-omgdds/java/src/main/java")
                .display(),
            root().join("runtime/java/src/main/java").display()
        );
        let compiled = Command::new("javac")
            .args([
                "-encoding",
                "UTF-8",
                "-Xlint:all,-unchecked",
                "-Werror",
                "-sourcepath",
                &sourcepath,
                "-d",
            ])
            .arg(dir.join("classes"))
            .arg(dir.join("ZdTypes.java"))
            .arg(dir.join("ZdCodec.java"))
            .arg(dir.join("ZdTypeSupports.java"))
            .arg(dir.join("Check.java"))
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "javac ({encoding}): {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let run = Command::new("java")
            .arg("-cp")
            .arg(dir.join("classes"))
            .arg("Check")
            .output()
            .unwrap();
        assert!(
            run.status.success()
                && String::from_utf8_lossy(&run.stdout).contains("TYPESUPPORT_BASE_OK"),
            "java Check ({encoding}): {}{}",
            String::from_utf8_lossy(&run.stdout),
            String::from_utf8_lossy(&run.stderr)
        );
    }
}
