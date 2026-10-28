// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Regression: `payload+encapsulation` never carried a padding count.
//!
//! `crates/wire-probe/tests/three_column_gate.rs`'s Python column found
//! it on `annotation-autoid-hash` under `ros-fastrtps/pl-cdr2/le`, the
//! first time that gate compared the `+enc` slot against the Rust
//! column over the whole corpus rather than the bare payload alone:
//! `crates/wire-codec/src/encode.rs`'s own `Emit::PayloadWithEncapsulation`
//! arm zero-pads the body to the next four-byte boundary and puts the
//! count of those bytes in the encapsulation header's options field, low
//! two bits (§7.6.3.1.2) — and this column's harness wrote a literal
//! `0` there, every time, whatever the body's length was. For a
//! thirty-byte body the header read `0x0000` where the clause and the
//! Rust and C columns all read `0x0002`, and the two padding bytes
//! themselves were missing from the stream entirely.
//!
//! Fixed in the harness `source` builds in
//! `tools/wireprobe/wireprobe-zerodds-python-gen/src/lib.rs`: `zd_pad`
//! is computed from `len(zd_body)` once the body exists, OR'd into the
//! header's fourth byte, and written after the body.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_wireprobe_python_gen::{Args, source};

fn repository_root() -> PathBuf {
    // `tools/wireprobe/wireprobe-zerodds-python-gen` is three below the
    // root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("the repository root")
        .to_path_buf()
}

fn python_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// The four encapsulation header octets and whatever follows, for one
/// case whose thirty-byte `pl-cdr2` body is not a multiple of four —
/// `annotation-autoid-hash` under `--dialect ros-fastrtps`, the exact
/// case and configuration the gate found this under.
#[test]
fn the_options_field_carries_the_padding_count_and_the_padding_follows_the_body() {
    if !python_available() {
        eprintln!("SKIPPED: `python3` is not on the path; this is not measured here");
        return;
    }
    let root = repository_root();
    let case = "annotation-autoid-hash";
    let directory = root.join("tests").join("interop-corpus").join(case);

    let args = Args {
        idl: directory.join("type.idl"),
        type_name: "Sample".to_string(),
        values: Some(directory.join("values.json")),
        encoding: Encoding::PlCdr2,
        endianness: Endianness::Little,
        encapsulation: true,
        dialect: Dialect::RosFastRtps,
        ..Args::default()
    };
    let generated = source(&args)
        .unwrap_or_else(|refusal| panic!("the generator refused: {}", refusal.message()));

    let script = std::env::temp_dir().join(format!(
        "zd-python-gen-encapsulation-padding-{}.py",
        std::process::id()
    ));
    std::fs::write(&script, &generated).expect("the generated module is written");
    let runtime = root.join("runtime").join("python");
    let output = Command::new("python3")
        .arg(&script)
        .env("PYTHONPATH", &runtime)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .expect("python3 starts");
    let _ = std::fs::remove_file(&script);
    assert!(
        output.status.success(),
        "the generated module did not run:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bytes = output.stdout;
    assert!(bytes.len() >= 4, "fewer than four bytes: {bytes:02x?}");
    // Thirty bytes of body — `2` is the case's own known shape, so the
    // test fails loudly if the case or its encoding ever changes under
    // it rather than silently measuring a padding count of zero, which
    // is the one value this defect could not be told apart from by
    // accident.
    let body_len = bytes.len() - 4 - usize::from(bytes[3] & 0x03);
    assert_eq!(
        body_len % 4,
        2,
        "this case's body is no longer 2 bytes past a four-byte boundary; pick a case \
         that still is, or the padding count this test pins is no longer exercised"
    );
    let declared = bytes[3] & 0x03;
    assert_eq!(
        usize::from(declared),
        (4 - body_len % 4) % 4,
        "the options field's low two bits do not carry the body's own padding count"
    );
    assert_eq!(
        bytes.len(),
        4 + body_len + usize::from(declared),
        "the padding bytes themselves are not appended after the body"
    );
    assert!(
        bytes[4 + body_len..].iter().all(|&byte| byte == 0),
        "the trailing padding is not all zero: {:02x?}",
        &bytes[4 + body_len..]
    );
}
