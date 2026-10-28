// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A second language, end to end, under one non-default dialect:
//! `zerodds-idlc generate --lang c --dialect opendds` compiles and
//! writes the same bytes `tests/dialect_byte_equality.rs` proved the
//! Rust column does.
//!
//! One case and not seven: `tests/dialect_byte_equality.rs` already
//! makes the exhaustive claim (every axis, every dialect, both byte
//! orders) for Rust, through the same generator, planned under the same
//! `LayoutKey`. What a second language adds is that the *runtime* also
//! agrees — `runtime/c`'s `zdw_writer` reads the same six framing
//! numbers off a caller-built value that `zerodds_wire::PlCdr1Framing`
//! does, and a defect in one runtime reading them differently from the
//! plan a shared `crates/ast-layout` computed would not show up in a
//! Rust-only gate. `Ext` under `--dialect opendds` is the case
//! `dialect_byte_equality.rs`'s own module doc uses to explain the
//! technique, so the expected bytes here were cross-checked against the
//! `wireprobe-zerodds` binary directly while this file was written, not
//! only against the reflective encoder both files call.
//!
//! The second test is RTI's PL_CDR1 step: a nested `@mutable` type that
//! is small on its own is framed short inside a small sample and with
//! `PID_EXTENDED` inside one past the step, and the generated C has to
//! make that choice per sample. `--dialect rti` plans
//! `PlCdr1HeaderPolicy::ExtendedForConstructedInLargeSamples`, which the
//! generated codec resolves by handing the sample's step decision down
//! to the encoders of nested types at run time; the reflective encoder
//! (`zerodds_wire_codec` under `Dialect::Rti`) is the comparison target,
//! the same way the OpenDDS case compares against it.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::path::PathBuf;
use std::process::Command;

use serde_json::json;
use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_wire_probe::encode::{hex, model_from_idl_in_dialect};
use zerodds_wire_probe::value::{MapLengthPolicy, ProbeRequest};

const IDL: &str = "@mutable struct Ext { long a; double b; short c; };";

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn work() -> PathBuf {
    std::env::temp_dir().join(format!(
        "zd-idlc-dialect-c-bytes-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ))
}

fn cc_available() -> bool {
    Command::new("cc")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// The bytes the wire-probe reflective encoder gives for `Ext` under
/// OpenDDS — the answer `wireprobe-zerodds --idl … --type Ext --dialect
/// opendds --encoding pl-cdr1` gives.
fn expected_bytes() -> Vec<u8> {
    let ast = model_from_idl_in_dialect(IDL, "ext.idl", Dialect::OpenDds).expect("the model");
    let key = zerodds_ast_layout::LayoutKey::in_dialect(
        Dialect::OpenDds,
        Encoding::PlCdr1,
        Endianness::Little,
        zerodds_ast_layout::key::Purpose::Data,
    );
    let request = ProbeRequest::from_key("Ext", key, MapLengthPolicy::SequenceEquivalent);
    let sample = json!({ "a": 7, "b": 1.5, "c": -3 });
    zerodds_wire_probe::encode::encode(&ast, &request, &sample).expect("the reflective encode")
}

/// `zerodds-idlc generate --lang c --dialect opendds`, for [`IDL`].
fn generate_c(dir: &std::path::Path) -> PathBuf {
    let idl = dir.join("ext.idl");
    std::fs::write(&idl, IDL).expect("the source is written");
    let out = dir.join("out");
    std::fs::create_dir_all(&out).expect("the output directory");
    let output = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"))
        .args([
            "generate",
            "--lang",
            "c",
            "--dialect",
            "opendds",
            "--encoding",
            "pl-cdr1",
            "--endian",
            "le",
            "--out",
        ])
        .arg(&out)
        .arg(&idl)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "generate --lang c --dialect opendds: exited {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    out.join("ext.h")
}

/// A C harness that builds `Ext { 7, 1.5, -3 }`, encodes it with a
/// `zdw_writer` framed the way OpenDDS's `LayoutKey` says a `Writer`
/// has to be, and prints the bytes as hex.
///
/// The six numbers embedded below are read at `cargo test` time off
/// `zerodds_ast_layout::LayoutKey::in_dialect(Dialect::OpenDds, …)` and
/// `ProbeRequest::pl_cdr1_framing`, not retyped from a doc comment —
/// the same discipline `dialect_byte_equality.rs`'s `writer_prelude`
/// keeps for the Rust harness, so this file cannot drift from what
/// OpenDDS's preset actually is.
fn harness_source(generated_header: &str) -> String {
    let key = zerodds_ast_layout::LayoutKey::in_dialect(
        Dialect::OpenDds,
        Encoding::PlCdr1,
        Endianness::Little,
        zerodds_ast_layout::key::Purpose::Data,
    );
    let request = ProbeRequest::from_key("Ext", key, MapLengthPolicy::SequenceEquivalent);
    let framing = request.pl_cdr1_framing();
    let pid_flags = match framing.pid_flags {
        zerodds_wire::PidFlagPolicy::SentinelMustUnderstand => {
            "ZDW_PID_FLAGS_SENTINEL_MUST_UNDERSTAND"
        }
        zerodds_wire::PidFlagPolicy::Bare => "ZDW_PID_FLAGS_BARE",
        zerodds_wire::PidFlagPolicy::ImplExtensionOnMembers => {
            "ZDW_PID_FLAGS_IMPL_EXTENSION_ON_MEMBERS"
        }
    };
    format!(
        "#include \"{generated_header}\"\n\
         #include <stdio.h>\n\
         int main(void) {{\n\
         \x20\x20\x20\x20uint8_t buf[256];\n\
         \x20\x20\x20\x20zdw_writer w;\n\
         \x20\x20\x20\x20zdw_writer_init(&w, buf, sizeof(buf), ZDW_LITTLE, {max_align});\n\
         \x20\x20\x20\x20zdw_pl_cdr1_framing framing;\n\
         \x20\x20\x20\x20framing.pid_flags = {pid_flags};\n\
         \x20\x20\x20\x20framing.highest_short_id = {highest_short_id};\n\
         \x20\x20\x20\x20framing.pad_declared_length = {pad_declared_length};\n\
         \x20\x20\x20\x20framing.pad_declared_optional_length = {pad_declared_optional_length};\n\
         \x20\x20\x20\x20framing.member_must_understand = {member_must_understand};\n\
         \x20\x20\x20\x20framing.pad_after_value = {pad_after_value};\n\
         \x20\x20\x20\x20zdw_writer_set_pl_cdr1(&w, framing);\n\
         \x20\x20\x20\x20w.short_length_codes = {short_length_codes};\n\
         \x20\x20\x20\x20Ext v;\n\
         \x20\x20\x20\x20v.a = 7;\n\
         \x20\x20\x20\x20v.b = 1.5;\n\
         \x20\x20\x20\x20v.c = -3;\n\
         \x20\x20\x20\x20zd_enc_Ext(&w, &v);\n\
         \x20\x20\x20\x20for (size_t i = 0; i < w.len; i++) {{ printf(\"%02x\", w.out[i]); }}\n\
         \x20\x20\x20\x20printf(\"\\n\");\n\
         \x20\x20\x20\x20return 0;\n\
         }}\n",
        max_align = Encoding::PlCdr1.max_alignment(),
        highest_short_id = framing.highest_short_id,
        pad_declared_length = c_bool(framing.pad_declared_length),
        pad_declared_optional_length = c_bool(framing.pad_declared_optional_length),
        member_must_understand = c_bool(framing.member_must_understand),
        pad_after_value = c_bool(framing.pad_after_value),
        short_length_codes = c_bool(request.length_codes.short_code_where_it_fits()),
    )
}

fn c_bool(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

#[test]
fn generated_c_under_opendds_writes_what_the_rust_probe_does() {
    if !cc_available() {
        eprintln!("SKIPPED: `cc` is not usable.");
        return;
    }
    let dir = work();
    std::fs::create_dir_all(&dir).expect("the scratch directory");

    let header = generate_c(&dir);
    let harness = harness_source(&header.display().to_string());
    let main_c = dir.join("main.c");
    std::fs::write(&main_c, &harness).expect("the harness is written");

    let runtime = repository_root().join("runtime/c");
    let binary = dir.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    let output = Command::new("cc")
        .args(["-std=c99", "-O0"])
        .arg("-I")
        .arg(runtime.join("include"))
        .arg("-I")
        .arg(header.parent().expect("the generated directory"))
        .arg(&main_c)
        // The whole runtime, reader included: the generated header
        // carries the decoders too, and GCC at -O0 emits their static
        // bodies whether the harness calls them or not, so a link
        // without reader.c fails on Linux where clang on macOS
        // discarded them.
        .arg(runtime.join("src/writer.c"))
        .arg(runtime.join("src/reader.c"))
        .arg(runtime.join("src/md5.c"))
        .arg(runtime.join("src/float128.c"))
        .arg(runtime.join("src/float128_decimal.c"))
        .args(["-lm", "-o"])
        .arg(&binary)
        .output()
        .expect("cc runs");
    assert!(
        output.status.success(),
        "the harness did not compile:\n{}\n---\n{harness}",
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&binary).output().expect("the binary runs");
    assert!(
        run.status.success(),
        "the harness did not run: {:?}\n{}",
        run.status.code(),
        String::from_utf8_lossy(&run.stderr)
    );
    let got = String::from_utf8_lossy(&run.stdout).trim().to_string();
    let expected = hex(&expected_bytes());
    assert_eq!(
        got, expected,
        "generated C under --dialect opendds disagrees with the reflective encoder"
    );
    // Cross-checked against wireprobe-zerodds directly while this test
    // was written; pinned here so a change to either side is visible as
    // a diff and not only as an assertion against a moving target.
    assert_eq!(
        expected,
        "00c004000700000001c00800000000000000f83f02c00200fdff0000023f0000"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A nested `@mutable` type that is small on its own, in two samples:
/// `Small`'s type stays within RTI's short-header step and `Large`'s
/// passes it. The nested `Inner.s` is framed short in one and with
/// `PID_EXTENDED` in the other, and `id` (a `long`) keeps the short
/// header in both.
const RTI_STEP_IDL: &str = "\
module m {
  @mutable struct Inner { long a; sequence<long, 100> s; };
  @mutable struct Small { long id; Inner part; sequence<octet, 100> big; };
  @mutable struct Large { long id; Inner part; sequence<octet, 65532> big; };
};";

/// The bytes the reflective encoder (`zerodds_wire_codec` under
/// `Dialect::Rti`) writes for one of the two samples.
fn rti_expected(type_name: &str) -> Vec<u8> {
    let ast = model_from_idl_in_dialect(RTI_STEP_IDL, "rti.idl", Dialect::Rti).expect("the model");
    let key = zerodds_ast_layout::LayoutKey::in_dialect(
        Dialect::Rti,
        Encoding::PlCdr1,
        Endianness::Little,
        zerodds_ast_layout::key::Purpose::Data,
    );
    let request = ProbeRequest::from_key(type_name, key, MapLengthPolicy::SequenceEquivalent);
    let sample = json!({ "id": 1, "part": { "a": 2, "s": [3] }, "big": [4] });
    zerodds_wire_probe::encode::encode(&ast, &request, &sample).expect("the reflective encode")
}

/// `zerodds-idlc generate --lang c --dialect rti`, for [`RTI_STEP_IDL`].
fn generate_rti_c(dir: &std::path::Path) -> PathBuf {
    let idl = dir.join("rti.idl");
    std::fs::write(&idl, RTI_STEP_IDL).expect("the source is written");
    let out = dir.join("out-rti");
    std::fs::create_dir_all(&out).expect("the output directory");
    let output = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"))
        .args([
            "generate",
            "--lang",
            "c",
            "--dialect",
            "rti",
            "--encoding",
            "pl-cdr1",
            "--endian",
            "le",
            "--out",
        ])
        .arg(&out)
        .arg(&idl)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "generate --lang c --dialect rti: exited {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    out.join("rti.h")
}

/// A C harness that encodes the same `{ id: 1, part: { a: 2, s: [3] },
/// big: [4] }` as `Small` and as `Large`, with a `zdw_writer` framed the
/// way RTI's `LayoutKey` says a writer has to be, and prints each as
/// hex. The six framing numbers are read off `ProbeRequest` at test
/// time, as in [`harness_source`].
fn rti_harness_source(header: &str) -> String {
    let key = zerodds_ast_layout::LayoutKey::in_dialect(
        Dialect::Rti,
        Encoding::PlCdr1,
        Endianness::Little,
        zerodds_ast_layout::key::Purpose::Data,
    );
    let request = ProbeRequest::from_key("m::Large", key, MapLengthPolicy::SequenceEquivalent);
    let framing = request.pl_cdr1_framing();
    let pid_flags = match framing.pid_flags {
        zerodds_wire::PidFlagPolicy::SentinelMustUnderstand => {
            "ZDW_PID_FLAGS_SENTINEL_MUST_UNDERSTAND"
        }
        zerodds_wire::PidFlagPolicy::Bare => "ZDW_PID_FLAGS_BARE",
        zerodds_wire::PidFlagPolicy::ImplExtensionOnMembers => {
            "ZDW_PID_FLAGS_IMPL_EXTENSION_ON_MEMBERS"
        }
    };
    format!(
        "#include \"{header}\"\n\
         #include <stdio.h>\n\
         #include <string.h>\n\
         static int32_t zd_s[1] = {{3}};\n\
         static uint8_t zd_big[1] = {{4}};\n\
         static void zd_show(const uint8_t *p, size_t n) {{\n\
         \x20\x20\x20\x20size_t i;\n\
         \x20\x20\x20\x20for (i = 0; i < n; i++) {{ printf(\"%02x\", p[i]); }}\n\
         \x20\x20\x20\x20printf(\"\\n\");\n\
         }}\n\
         static void zd_frame(zdw_writer *w) {{\n\
         \x20\x20\x20\x20zdw_pl_cdr1_framing framing;\n\
         \x20\x20\x20\x20framing.pid_flags = {pid_flags};\n\
         \x20\x20\x20\x20framing.highest_short_id = {highest_short_id};\n\
         \x20\x20\x20\x20framing.pad_declared_length = {pad_declared_length};\n\
         \x20\x20\x20\x20framing.pad_declared_optional_length = {pad_declared_optional_length};\n\
         \x20\x20\x20\x20framing.member_must_understand = {member_must_understand};\n\
         \x20\x20\x20\x20framing.pad_after_value = {pad_after_value};\n\
         \x20\x20\x20\x20zdw_writer_set_pl_cdr1(w, framing);\n\
         \x20\x20\x20\x20w->short_length_codes = {short_length_codes};\n\
         }}\n\
         int main(void) {{\n\
         \x20\x20\x20\x20static uint8_t zd_buf[200000];\n\
         \x20\x20\x20\x20zdw_writer w;\n\
         \x20\x20\x20\x20m_Small small;\n\
         \x20\x20\x20\x20m_Large large;\n\
         \x20\x20\x20\x20zdw_writer_init(&w, zd_buf, sizeof zd_buf, ZDW_LITTLE, {max_align});\n\
         \x20\x20\x20\x20zd_frame(&w);\n\
         \x20\x20\x20\x20memset(&small, 0, sizeof small);\n\
         \x20\x20\x20\x20small.id = 1; small.part.a = 2; small.part.s._buffer = zd_s;\n\
         \x20\x20\x20\x20small.part.s._length = 1; small.big._buffer = zd_big; small.big._length = 1;\n\
         \x20\x20\x20\x20zd_enc_m_Small(&w, &small);\n\
         \x20\x20\x20\x20zd_show(zd_buf, w.len);\n\
         \x20\x20\x20\x20zdw_writer_init(&w, zd_buf, sizeof zd_buf, ZDW_LITTLE, {max_align});\n\
         \x20\x20\x20\x20zd_frame(&w);\n\
         \x20\x20\x20\x20memset(&large, 0, sizeof large);\n\
         \x20\x20\x20\x20large.id = 1; large.part.a = 2; large.part.s._buffer = zd_s;\n\
         \x20\x20\x20\x20large.part.s._length = 1; large.big._buffer = zd_big; large.big._length = 1;\n\
         \x20\x20\x20\x20zd_enc_m_Large(&w, &large);\n\
         \x20\x20\x20\x20zd_show(zd_buf, w.len);\n\
         \x20\x20\x20\x20return 0;\n\
         }}\n",
        max_align = Encoding::PlCdr1.max_alignment(),
        highest_short_id = framing.highest_short_id,
        pad_declared_length = c_bool(framing.pad_declared_length),
        pad_declared_optional_length = c_bool(framing.pad_declared_optional_length),
        member_must_understand = c_bool(framing.member_must_understand),
        pad_after_value = c_bool(framing.pad_after_value),
        short_length_codes = c_bool(request.length_codes.short_code_where_it_fits()),
    )
}

/// RTI decides the PL_CDR1 header form for the **sample's** type: a
/// nested `@mutable` type that is small on its own is framed short in a
/// small sample and with `PID_EXTENDED` in a sample past the step. The
/// generated C resolves that per sample and hands it to the nested
/// type's encoder at run time, so both samples write the octets the
/// reflective encoder writes.
#[test]
fn generated_c_under_rti_frames_a_nested_type_by_its_sample() {
    if !cc_available() {
        eprintln!("SKIPPED: `cc` is not usable.");
        return;
    }
    let dir = work();
    std::fs::create_dir_all(&dir).expect("the scratch directory");

    let header = generate_rti_c(&dir);
    let harness = rti_harness_source(&header.display().to_string());
    let main_c = dir.join("rti_main.c");
    std::fs::write(&main_c, &harness).expect("the harness is written");

    let runtime = repository_root().join("runtime/c");
    let binary = dir.join(if cfg!(windows) {
        "rti-probe.exe"
    } else {
        "rti-probe"
    });
    let output = Command::new("cc")
        .args(["-std=c99", "-O0"])
        .arg("-I")
        .arg(runtime.join("include"))
        .arg("-I")
        .arg(header.parent().expect("the generated directory"))
        .arg(&main_c)
        .arg(runtime.join("src/writer.c"))
        .arg(runtime.join("src/reader.c"))
        .arg(runtime.join("src/md5.c"))
        .arg(runtime.join("src/float128.c"))
        .arg(runtime.join("src/float128_decimal.c"))
        .args(["-lm", "-o"])
        .arg(&binary)
        .output()
        .expect("cc runs");
    assert!(
        output.status.success(),
        "the RTI harness did not compile:\n{}\n---\n{harness}",
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&binary).output().expect("the binary runs");
    assert!(
        run.status.success(),
        "the RTI harness did not run: {:?}\n{}",
        run.status.code(),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    let mut lines = stdout.lines();
    let small = lines.next().expect("the Small line").to_string();
    let large = lines.next().expect("the Large line").to_string();
    let expected_small = hex(&rti_expected("m::Small"));
    let expected_large = hex(&rti_expected("m::Large"));
    assert_eq!(
        small, expected_small,
        "Small: generated C under --dialect rti disagrees with the reflective encoder"
    );
    assert_eq!(
        large, expected_large,
        "Large: generated C under --dialect rti disagrees with the reflective encoder"
    );
    assert_ne!(
        small, large,
        "the two samples framed the nested type the same way; the step did not reach the \
         nested encoder"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
