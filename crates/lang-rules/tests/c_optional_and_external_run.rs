// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C binding of optional and external members, compiled and run.
//!
//! DDS-XTypes 1.3 §7.5.1.2.4.1 ("Optional members shall be represented
//! using pointers [...] A NULL pointer shall indicate an omitted value"),
//! §7.5.1.2.4.4 (an optional array is a pointer to the array) and
//! §7.5.1.2.3.1.1 (an external member that is not optional "is not allowed
//! [...] to be NULL at the time of a write"). The expected octets are the
//! ones §7.4.3.5.2 rule (20) gives an `OPT_FMEMBER`: a BOOLEAN
//! `<is_present>` and then the value.
//!
//! The generated header goes through `cc` with the flags the corpus gate
//! uses and is linked with the sources of `runtime/c`; the program prints
//! what it saw and the test compares the text.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::PathBuf;
use std::process::Command;

use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compiler() -> String {
    if let Ok(chosen) = std::env::var("CC") {
        return chosen;
    }
    ["cc", "gcc", "clang"]
        .iter()
        .find(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        })
        .map(|found| (*found).to_string())
        .expect("a C compiler (cc, gcc or clang) on the path; this test does not skip")
}

/// The generated C for `idl` as XCDR2 little endian, behind the preamble
/// `zerodds-idlc` writes.
fn generated(idl: &str) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    let body = zerodds_lang_rules::generate::source(
        &ast,
        Language::C,
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
    )
    .unwrap_or_else(|why| panic!("generate: {why:?}"));
    format!(
        "#ifndef ZD_GEN_H\n#define ZD_GEN_H\n#include \"zerodds_wire.h\"\n\
         #define ZD_ENDIANNESS ZDW_LITTLE\n{body}\n#endif\n"
    )
}

/// Compile `idl`'s header, a `main` made of `body`, and `runtime/c`, run
/// it and answer its standard output.
fn run(tag: &str, idl: &str, body: &str) -> String {
    let dir = std::env::temp_dir().join(format!("zd-c-run-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a work directory");
    std::fs::write(dir.join("gen.h"), generated(idl)).expect("the header");
    std::fs::write(
        dir.join("main.c"),
        format!(
            "#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n\
             #include \"gen.h\"\n\
             void hex(const zdw_writer *w) {{\n\
             \x20   size_t i;\n\
             \x20   for (i = 0; i < zdw_writer_length(w); i++) {{ printf(\"%02x\", w->out[i]); }}\n\
             \x20   printf(\"\\n\");\n}}\n\
             int main(void) {{\n{body}\n    return 0;\n}}\n"
        ),
    )
    .expect("the program");
    let runtime = root().join("runtime/c");
    let binary = dir.join("program");
    let mut command = Command::new(compiler());
    command
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Wconversion",
            "-Wsign-conversion",
            "-Wno-unused-function",
            "-Werror",
        ])
        .arg(format!("-I{}", dir.display()))
        .arg(format!("-I{}", runtime.join("include").display()))
        .arg("-o")
        .arg(&binary)
        .arg(dir.join("main.c"));
    for source in [
        "writer.c",
        "reader.c",
        "float128.c",
        "float128_decimal.c",
        "md5.c",
    ] {
        command.arg(runtime.join("src").join(source));
    }
    let compiled = command.output().expect("the compiler starts");
    assert!(
        compiled.status.success(),
        "the generated C does not compile:\n{}\n{}",
        String::from_utf8_lossy(&compiled.stderr),
        std::fs::read_to_string(dir.join("gen.h")).unwrap_or_default()
    );
    let ran = Command::new(&binary).output().expect("the program starts");
    let stdout = String::from_utf8_lossy(&ran.stdout).into_owned();
    assert!(
        ran.status.success(),
        "the program failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
    stdout
}

const OPTIONALS: &str =
    "@final struct S { long a; @optional long b; @optional string c; long d; };";

#[test]
fn an_optional_member_is_written_with_its_marker_and_read_back_through_a_pointer() {
    let out = run(
        "optional",
        OPTIONALS,
        r#"
    uint8_t buffer[64];
    uint8_t heap[256];
    zdw_writer w;
    zdw_reader r;
    S sample;
    S back;
    int32_t b = 2;
    memset(&sample, 0, sizeof sample);
    sample.a = 1;
    sample.b = &b;
    sample.d = 4;
    zdw_writer_init(&w, buffer, sizeof buffer, ZDW_LITTLE, 4);
    zd_enc_S(&w, &sample);
    printf("status %d\n", (int)zdw_writer_status(&w));
    hex(&w);

    memset(&back, 0xAA, sizeof back);
    zdw_reader_init(&r, buffer, zdw_writer_length(&w), ZDW_LITTLE, 4);
    zdw_reader_set_heap(&r, heap, sizeof heap);
    zd_dec_S(&r, &back, ZDW_TC_DISCARD);
    printf("read %d a=%d b=%d c=%d d=%d\n", (int)zdw_reader_status(&r), (int)back.a,
           back.b != NULL ? (int)*back.b : -1, back.c != NULL, (int)back.d);

    sample.b = NULL;
    zdw_writer_init(&w, buffer, sizeof buffer, ZDW_LITTLE, 4);
    zd_enc_S(&w, &sample);
    hex(&w);
    memset(&back, 0xAA, sizeof back);
    zdw_reader_init(&r, buffer, zdw_writer_length(&w), ZDW_LITTLE, 4);
    zdw_reader_set_heap(&r, heap, sizeof heap);
    zd_dec_S(&r, &back, ZDW_TC_DISCARD);
    printf("read %d b=%d c=%d\n", (int)zdw_reader_status(&r), back.b != NULL, back.c != NULL);
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "status 0", "{out}");
    // a = 1 | present, pad | b = 2 | absent c, pad | d = 4
    assert_eq!(
        lines[1],
        "01000000".to_string() + "01000000" + "02000000" + "00000000" + "04000000",
        "{out}"
    );
    assert_eq!(lines[2], "read 0 a=1 b=2 c=0 d=4", "{out}");
    // b and c absent: a | 0 | 0, pad | d
    assert_eq!(
        lines[3],
        "01000000".to_string() + "00000000" + "04000000",
        "{out}"
    );
    assert_eq!(lines[4], "read 0 b=0 c=0", "{out}");
}

#[test]
fn an_optional_string_is_the_pointer_it_already_is() {
    let out = run(
        "optional-string",
        OPTIONALS,
        r#"
    uint8_t buffer[64];
    uint8_t heap[256];
    zdw_writer w;
    zdw_reader r;
    S sample;
    S back;
    memset(&sample, 0, sizeof sample);
    sample.a = 1;
    sample.c = "hi";
    sample.d = 4;
    zdw_writer_init(&w, buffer, sizeof buffer, ZDW_LITTLE, 4);
    zd_enc_S(&w, &sample);
    printf("status %d\n", (int)zdw_writer_status(&w));
    hex(&w);
    memset(&back, 0, sizeof back);
    zdw_reader_init(&r, buffer, zdw_writer_length(&w), ZDW_LITTLE, 4);
    zdw_reader_set_heap(&r, heap, sizeof heap);
    zd_dec_S(&r, &back, ZDW_TC_DISCARD);
    printf("read %d b=%d c=%s\n", (int)zdw_reader_status(&r), back.b != NULL,
           back.c != NULL ? back.c : "(null)");
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "status 0", "{out}");
    // a | b absent, c present, pad to 8 | length 3, "hi\0", pad | d
    assert_eq!(
        lines[1],
        "01000000".to_string() + "0001" + "0000" + "03000000" + "686900" + "00" + "04000000",
        "{out}"
    );
    assert_eq!(lines[2], "read 0 b=0 c=hi", "{out}");
}

#[test]
fn an_optional_array_is_a_pointer_to_the_array_and_not_an_array_of_pointers() {
    // §7.5.1.2.4.4: `@optional long array_member[10]` is
    // `int32_t (*array_member)[10]`; here with three elements.
    let out = run(
        "optional-array",
        "@final struct M { @optional long array_member[3]; };",
        r#"
    uint8_t buffer[64];
    uint8_t heap[256];
    zdw_writer w;
    zdw_reader r;
    M sample;
    M back;
    int32_t store[3] = {1, 2, 3};
    int32_t (*spelled_in_the_clause)[3];
    memset(&sample, 0, sizeof sample);
    sample.array_member = &store;
    spelled_in_the_clause = sample.array_member;
    printf("same %d\n", spelled_in_the_clause == &store);
    zdw_writer_init(&w, buffer, sizeof buffer, ZDW_LITTLE, 4);
    zd_enc_M(&w, &sample);
    printf("status %d\n", (int)zdw_writer_status(&w));
    hex(&w);
    memset(&back, 0, sizeof back);
    zdw_reader_init(&r, buffer, zdw_writer_length(&w), ZDW_LITTLE, 4);
    zdw_reader_set_heap(&r, heap, sizeof heap);
    zd_dec_M(&r, &back, ZDW_TC_DISCARD);
    printf("read %d %d %d %d\n", (int)zdw_reader_status(&r), (int)(*back.array_member)[0],
           (int)(*back.array_member)[1], (int)(*back.array_member)[2]);
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "same 1", "{out}");
    assert_eq!(lines[1], "status 0", "{out}");
    assert_eq!(
        lines[2],
        "01000000".to_string() + "01000000" + "02000000" + "03000000",
        "{out}"
    );
    assert_eq!(lines[3], "read 0 1 2 3", "{out}");
}

#[test]
fn an_external_member_that_is_not_optional_may_not_be_null_at_a_write() {
    // §7.5.1.2.3.1.1.
    let out = run(
        "external-null",
        "@final struct E { @external long a; @optional @external long b; };",
        r#"
    uint8_t buffer[64];
    zdw_writer w;
    E sample;
    int32_t a = 5;
    memset(&sample, 0, sizeof sample);
    zdw_writer_init(&w, buffer, sizeof buffer, ZDW_LITTLE, 4);
    zd_enc_E(&w, &sample);
    printf("unset: %s\n", zdw_status_name(zdw_writer_status(&w)));
    sample.a = &a;
    zdw_writer_init(&w, buffer, sizeof buffer, ZDW_LITTLE, 4);
    zd_enc_E(&w, &sample);
    printf("set: %s\n", zdw_status_name(zdw_writer_status(&w)));
    hex(&w);
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines[0], "unset: an external member that is not optional has no value",
        "{out}"
    );
    // The optional external member may be NULL: marker 0 after the value.
    assert_eq!(lines[1], "set: ok", "{out}");
    assert_eq!(lines[2], "05000000".to_string() + "00", "{out}");
}

#[test]
fn the_functions_of_an_aggregate_with_an_optional_member_initialise_copy_and_release_it() {
    let out = run(
        "lifecycle",
        "@final struct L { long a; @optional long b; };",
        r#"
    L x;
    L y;
    zd_init_L(&x);
    zd_init_L(&y);
    printf("init %d %d\n", x.b == NULL, y.b == NULL);
    x.a = 7;
    x.b = (int32_t *)malloc(sizeof(int32_t));
    *x.b = 9;
    printf("copy %d\n", zd_copy_L(&y, &x) ? 1 : 0);
    printf("deep %d %d %d\n", y.a, y.b != NULL && y.b != x.b, y.b != NULL ? *y.b : -1);
    zd_fini_L(&x);
    zd_fini_L(&y);
    printf("fini %d %d\n", x.b == NULL, y.b == NULL);
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "init 1 1", "{out}");
    assert_eq!(lines[1], "copy 1", "{out}");
    assert_eq!(lines[2], "deep 7 1 9", "{out}");
    assert_eq!(lines[3], "fini 1 1", "{out}");
}
