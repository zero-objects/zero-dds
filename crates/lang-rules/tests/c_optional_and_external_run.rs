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
//! §7.5.1.2.3.1 gives an aggregate with an external member an initialiser,
//! a destructor and a copy function, and Table 45 the five behaviours of
//! the copy for a destination member that is not NULL. The tests of that
//! clause run the generated functions over a counting allocator
//! ([`ALLOCATOR`]): which blocks a copy asked for, which it gave back and
//! in what order is what the rows of the table differ in.
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
    program(tag, idl, "", "", body)
}

/// [`run`] over the counting allocator, with the functions `helpers`
/// defines between the generated header and `main`.
fn run_counting(tag: &str, idl: &str, helpers: &str, body: &str) -> String {
    program(tag, idl, ALLOCATOR, helpers, body)
}

/// The program: the standard headers, `before`, the generated header,
/// `helpers`, and a `main` made of `body`.
fn program(tag: &str, idl: &str, before: &str, helpers: &str, body: &str) -> String {
    let dir = std::env::temp_dir().join(format!("zd-c-run-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a work directory");
    std::fs::write(dir.join("gen.h"), generated(idl)).expect("the header");
    std::fs::write(
        dir.join("main.c"),
        format!(
            "#include <stdbool.h>\n#include <stddef.h>\n#include <stdint.h>\n\
             #include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n\
             {before}\n\
             #include \"gen.h\"\n\
             void hex(const zdw_writer *w) {{\n\
             \x20   size_t i;\n\
             \x20   for (i = 0; i < zdw_writer_length(w); i++) {{ printf(\"%02x\", w->out[i]); }}\n\
             \x20   printf(\"\\n\");\n}}\n\
             {helpers}\n\
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
fn xtypes_7_5_1_2_4_1_an_optional_member_is_written_with_its_marker_and_read_back_through_a_pointer()
 {
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
fn xtypes_7_5_1_2_4_1_an_optional_string_is_the_pointer_it_already_is() {
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
fn xtypes_7_5_1_2_4_4_an_optional_array_is_a_pointer_to_the_array_and_not_an_array_of_pointers() {
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
fn xtypes_7_5_1_2_3_1_1_an_external_member_that_is_not_optional_may_not_be_null_at_a_write() {
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

// -- §7.5.1.2.3.1: the functions of an aggregate with external members ------

/// A counting allocator under the generated functions: every block they
/// ask for and every block they give back, on one clock. A block is kept
/// for the whole run, so an address names one block and a released block
/// can still be read. `zd_t_bad` counts what must not happen: a block
/// released twice, or one this allocator did not make. A new block is
/// filled with the octet 0xA5, so that storage nothing has written does
/// not read as zeros, as NULL or as false.
const ALLOCATOR: &str = r"
#define ZD_T_BLOCKS 256u
static void *zd_t_block[ZD_T_BLOCKS];
static unsigned zd_t_made[ZD_T_BLOCKS];
static unsigned zd_t_gone[ZD_T_BLOCKS];
static unsigned zd_t_blocks;
static unsigned zd_t_clock;
static unsigned zd_t_mallocs;
static unsigned zd_t_frees;
static unsigned zd_t_bad;
/* The n-th malloc from here answers NULL; 0 for none. */
static unsigned zd_t_fail_at;

static void *zd_t_malloc(size_t size) {
    void *made;
    if (zd_t_fail_at != 0u) {
        zd_t_fail_at--;
        if (zd_t_fail_at == 0u) {
            return NULL;
        }
    }
    if (zd_t_blocks >= ZD_T_BLOCKS) {
        zd_t_bad++;
        return NULL;
    }
    made = malloc(size);
    if (made == NULL) {
        return NULL;
    }
    memset(made, 0xA5, size);
    zd_t_clock++;
    zd_t_block[zd_t_blocks] = made;
    zd_t_made[zd_t_blocks] = zd_t_clock;
    zd_t_gone[zd_t_blocks] = 0u;
    zd_t_blocks++;
    zd_t_mallocs++;
    return made;
}

static void zd_t_free(void *block) {
    unsigned i;
    if (block == NULL) {
        return;
    }
    for (i = 0u; i < zd_t_blocks; i++) {
        if (zd_t_block[i] == block) {
            if (zd_t_gone[i] != 0u) {
                zd_t_bad++;
            } else {
                zd_t_clock++;
                zd_t_gone[i] = zd_t_clock;
                zd_t_frees++;
            }
            return;
        }
    }
    zd_t_bad++;
}

/* When `block` was allocated, on the clock; 0 for a block that was not. */
static unsigned zd_t_made_at(const void *block) {
    unsigned i;
    for (i = 0u; i < zd_t_blocks; i++) {
        if (zd_t_block[i] == block) {
            return zd_t_made[i];
        }
    }
    return 0u;
}

/* When `block` was released, on the clock; 0 for one that is live. */
static unsigned zd_t_freed_at(const void *block) {
    unsigned i;
    for (i = 0u; i < zd_t_blocks; i++) {
        if (zd_t_block[i] == block) {
            return zd_t_gone[i];
        }
    }
    return 0u;
}

static int zd_t_freed(const void *block) {
    return zd_t_freed_at(block) != 0u;
}

/* The blocks that were allocated and not released. */
static unsigned zd_t_live(void) {
    unsigned i;
    unsigned live = 0u;
    for (i = 0u; i < zd_t_blocks; i++) {
        if (zd_t_gone[i] == 0u) {
            live++;
        }
    }
    return live;
}

/* Counts the calls from here. */
static void zd_t_count(void) {
    zd_t_mallocs = 0u;
    zd_t_frees = 0u;
}

#define malloc zd_t_malloc
#define free zd_t_free
";

/// A structure whose external members point at a structure, a scalar and
/// a structure that itself holds an external member, and a union with two
/// external branches.
const SCALARS: &str = "\
@final struct Inner { long a; };
@final struct Deep { @external long a; };
@final struct T { @external Inner p; @external long q; @external Deep r; long z; };
union U switch (long) { case 1: @external Inner p; case 2: long plain; case 3: @external long q; };";

/// `fill(v, base)`: a `T` whose three external members point at objects
/// of their own, holding `base + 1`, `base + 2` and `base + 3`, and whose
/// plain member is `base + 4`. Four blocks.
const SCALAR_HELPERS: &str = r"
static void fill(T *v, int32_t base) {
    zd_init_T(v);
    v->p = malloc(sizeof *v->p);
    v->p->a = base + 1;
    v->q = malloc(sizeof *v->q);
    *v->q = base + 2;
    v->r = malloc(sizeof *v->r);
    zd_init_Deep(v->r);
    v->r->a = malloc(sizeof *v->r->a);
    *v->r->a = base + 3;
    v->z = base + 4;
}
";

fn lines(out: &str) -> Vec<&str> {
    out.lines().collect()
}

#[test]
fn xtypes_7_5_1_2_3_1_a_shallow_copy_into_a_null_destination_points_at_the_source() {
    // "In the case that a shallow copy is made and the destination member
    // is NULL then the destination member pointer will be set to the
    // source member pointer." No action changes that, and nothing is
    // allocated or released.
    let out = run_counting(
        "shallow-null",
        SCALARS,
        SCALAR_HELPERS,
        r#"
    T x;
    T y;
    int action;
    bool ok;
    fill(&x, 10);
    for (action = 0; action < 3; action++) {
        zd_init_T(&y);
        y.z = 0;
        zd_t_count();
        ok = zd_copy_with_T(&y, &x, ZD_COPY_SHALLOW, (zd_copy_action)action);
        printf("copy %d same %d %d %d plain %d calls %u %u\n", ok ? 1 : 0, y.p == x.p,
               y.q == x.q, y.r == x.r, (int)y.z, zd_t_mallocs, zd_t_frees);
        /* The objects are x's: y gives them up before it is destroyed. */
        y.p = NULL;
        y.q = NULL;
        y.r = NULL;
        zd_fini_T(&y);
    }
    zd_fini_T(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "copy 1 same 1 1 1 plain 14 calls 0 0",
            "copy 1 same 1 1 1 plain 14 calls 0 0",
            "copy 1 same 1 1 1 plain 14 calls 0 0",
            "end 0 0",
        ],
        "{out}"
    );
}

#[test]
fn xtypes_7_5_1_2_3_1_a_deep_copy_into_a_null_destination_allocates_and_copies() {
    // "In the case that a deep copy is made and the destination member
    // pointer is NULL, memory for the destination member will be
    // allocated and then copied into." With each action, and with the copy
    // function that takes none ("The copy function of the enclosing
    // object shall do a deep copy of the external members. If the
    // destination external member is NULL it shall be allocated."). The
    // member of the member is an object of its own too, and the
    // destructor releases the four.
    let out = run_counting(
        "deep-null",
        SCALARS,
        SCALAR_HELPERS,
        r#"
    T x;
    T y;
    int form;
    bool ok;
    fill(&x, 10);
    for (form = 0; form < 4; form++) {
        zd_init_T(&y);
        zd_t_count();
        ok = form < 3 ? zd_copy_with_T(&y, &x, ZD_COPY_DEEP, (zd_copy_action)form)
                      : zd_copy_T(&y, &x);
        printf("copy %d own %d %d %d %d values %d %d %d %d calls %u %u\n", ok ? 1 : 0,
               y.p != NULL && y.p != x.p, y.q != NULL && y.q != x.q, y.r != NULL && y.r != x.r,
               y.r != NULL && y.r->a != NULL && y.r->a != x.r->a, (int)y.p->a, (int)*y.q,
               (int)*y.r->a, (int)y.z, zd_t_mallocs, zd_t_frees);
        y.p->a = 97;
        *y.q = 98;
        *y.r->a = 99;
        printf("source %d %d %d\n", (int)x.p->a, (int)*x.q, (int)*x.r->a);
        zd_fini_T(&y);
        printf("freed %u\n", zd_t_frees);
    }
    zd_fini_T(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    let mut expected = Vec::new();
    for _ in 0..4 {
        expected.push("copy 1 own 1 1 1 1 values 11 12 13 14 calls 4 0");
        expected.push("source 11 12 13");
        expected.push("freed 4");
    }
    expected.push("end 0 0");
    assert_eq!(lines(&out), expected, "{out}");
}

#[test]
fn xtypes_7_5_1_2_3_1_a_failed_copy_of_an_external_member_fails_the_copy_of_the_aggregate() {
    // "If the (recursive call to the) copy operation of the external
    // member fails, then the copy function of the containing object shall
    // fail as well." The copy asks for four blocks; each of them refused
    // in turn fails it, the fourth inside the copy of the member `r`. What
    // was copied until then is the destination's, and its destructor
    // releases it.
    let out = run_counting(
        "copy-fails",
        SCALARS,
        SCALAR_HELPERS,
        r#"
    T x;
    T y;
    unsigned n;
    unsigned before;
    bool ok;
    fill(&x, 10);
    before = zd_t_live();
    for (n = 1u; n <= 5u; n++) {
        zd_init_T(&y);
        zd_t_fail_at = n;
        ok = zd_copy_T(&y, &x);
        zd_t_fail_at = 0u;
        printf("malloc %u copy %d holds %u", n, ok ? 1 : 0, zd_t_live() - before);
        zd_fini_T(&y);
        printf(" after %u\n", zd_t_live() - before);
    }
    zd_fini_T(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "malloc 1 copy 0 holds 0 after 0",
            "malloc 2 copy 0 holds 1 after 0",
            "malloc 3 copy 0 holds 2 after 0",
            "malloc 4 copy 0 holds 3 after 0",
            "malloc 5 copy 1 holds 4 after 0",
            "end 0 0",
        ],
        "{out}"
    );
}

#[test]
fn xtypes_7_5_1_2_3_1_the_copy_of_a_union_follows_the_discriminator_of_the_source() {
    // The external member of a union is the branch the discriminator
    // selects. A destination that holds another branch than the source
    // gives it up first: released, except by a deep REPLACE, which leaves
    // it to the application as Table 45 says of the member it replaces.
    let out = run_counting(
        "union",
        SCALARS,
        SCALAR_HELPERS,
        r#"
    U x;
    U y;
    Inner *held;
    int32_t *old;
    bool ok;
    zd_init_U(&x);
    zd_init_U(&y);
    x._d = 1;
    x._u.p = malloc(sizeof *x._u.p);
    x._u.p->a = 5;
    y._d = 3;
    y._u.q = malloc(sizeof *y._u.q);
    *y._u.q = 6;
    old = y._u.q;
    ok = zd_copy_U(&y, &x);
    printf("external %d %d %d %d %d\n", ok ? 1 : 0, (int)y._d,
           y._u.p != NULL && y._u.p != x._u.p, (int)y._u.p->a, zd_t_freed(old));

    /* A source whose branch is not external: the destination's goes. */
    held = y._u.p;
    zd_fini_U(&x);
    x._d = 2;
    x._u.plain = 9;
    ok = zd_copy_U(&y, &x);
    printf("plain %d %d %d %d\n", ok ? 1 : 0, (int)y._d, (int)y._u.plain, zd_t_freed(held));

    /* A deep REPLACE leaves the destination's other branch alone. */
    y._d = 3;
    y._u.q = malloc(sizeof *y._u.q);
    *y._u.q = 7;
    old = y._u.q;
    x._d = 1;
    x._u.p = malloc(sizeof *x._u.p);
    x._u.p->a = 8;
    ok = zd_copy_with_U(&y, &x, ZD_COPY_DEEP, ZD_COPY_REPLACE);
    printf("replace %d %d %d %d %d %d\n", ok ? 1 : 0, (int)y._d, y._u.p != x._u.p,
           (int)y._u.p->a, zd_t_freed(old), (int)*old);
    free(old);

    /* A shallow copy releases it, whatever the action. */
    zd_fini_U(&y);
    y._d = 3;
    y._u.q = malloc(sizeof *y._u.q);
    old = y._u.q;
    ok = zd_copy_with_U(&y, &x, ZD_COPY_SHALLOW, ZD_COPY_REPLACE);
    printf("shallow %d %d %d %d\n", ok ? 1 : 0, (int)y._d, y._u.p == x._u.p, zd_t_freed(old));
    y._u.p = NULL;
    zd_fini_U(&y);
    zd_fini_U(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "external 1 1 1 5 1",
            "plain 1 2 9 1",
            "replace 1 1 1 8 0 7",
            "shallow 1 1 1 1",
            "end 0 0",
        ],
        "{out}"
    );
}

#[test]
fn xtypes_7_5_1_2_3_1_a_copy_of_an_object_onto_itself_changes_nothing() {
    // The clause copies a source into a destination. Where both are one
    // object every destination member is the source member, and a row of
    // Table 45 that releases or replaces "the existing memory pointed to
    // by destination" would take the source with it. The copy answers
    // true and does nothing: the pointers and the values are the ones
    // from before, no block was asked for and none given back.
    let out = run_counting(
        "self-copy",
        SCALARS,
        SCALAR_HELPERS,
        r#"
    T x;
    U u;
    const void *p;
    const void *q;
    const void *r;
    const void *a;
    int form;
    bool ok;
    fill(&x, 10);
    p = x.p;
    q = x.q;
    r = x.r;
    a = x.r->a;
    for (form = 0; form < 7; form++) {
        zd_t_count();
        ok = form == 6 ? zd_copy_T(&x, &x)
                       : zd_copy_with_T(&x, &x,
                                        (zd_copy_depth)(form < 3 ? ZD_COPY_DEEP : ZD_COPY_SHALLOW),
                                        (zd_copy_action)(form % 3));
        printf("copy %d kept %d %d %d %d calls %u %u live %u\n", ok ? 1 : 0, x.p == p, x.q == q,
               x.r == r, x.r != NULL && x.r->a == a, zd_t_mallocs, zd_t_frees, zd_t_live());
    }
    printf("values %d %d %d %d\n", (int)x.p->a, (int)*x.q, (int)*x.r->a, (int)x.z);

    zd_init_U(&u);
    u._d = 1;
    u._u.p = malloc(sizeof *u._u.p);
    u._u.p->a = 5;
    p = u._u.p;
    for (form = 0; form < 6; form++) {
        zd_t_count();
        ok = zd_copy_with_U(&u, &u, (zd_copy_depth)(form < 3 ? ZD_COPY_DEEP : ZD_COPY_SHALLOW),
                            (zd_copy_action)(form % 3));
        printf("union %d kept %d %d calls %u %u\n", ok ? 1 : 0, (int)u._d, u._u.p == p,
               zd_t_mallocs, zd_t_frees);
    }
    printf("value %d\n", (int)u._u.p->a);
    zd_fini_U(&u);
    zd_fini_T(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    let mut expected = vec!["copy 1 kept 1 1 1 1 calls 0 0 live 4"; 7];
    expected.push("values 11 12 13 14");
    expected.extend(["union 1 kept 1 1 calls 0 0"; 6]);
    expected.push("value 5");
    expected.push("end 0 0");
    assert_eq!(lines(&out), expected, "{out}");
}

/// Unions with an external branch whose discriminator is a boolean and an
/// enumeration with a `@default_literal`, a structure that holds such a
/// union inline, and a structure whose external members reach them: as the
/// element of a sequence, inside the element of a sequence, and directly.
const UNIONS: &str = "\
enum Colour { RED, @default_literal GREEN, BLUE };
union B switch (boolean) { case TRUE: @external long q; };
union E switch (Colour) { case BLUE: @external long q; case RED: long plain; };
@final struct Holder { B held; long z; };
@final struct M {
    @external sequence<B> bs;
    @external sequence<E> es;
    @external sequence<Holder> hs;
    @external B one;
};";

/// `fill(v)`: an `M` whose three sequences hold two elements each. The
/// first one selects the external branch, which points at 5, 6 and 7; the
/// second one is as its initialiser left it. `one` selects the external
/// branch, which points at 9. Eleven blocks.
const UNION_HELPERS: &str = r"
static int32_t *number(int32_t value) {
    int32_t *made = malloc(sizeof *made);
    *made = value;
    return made;
}

static void fill(M *v) {
    zd_init_M(v);
    v->bs = malloc(sizeof *v->bs);
    v->bs->_maximum = 2u;
    v->bs->_length = 2u;
    v->bs->_release = true;
    v->bs->_buffer = malloc(2u * sizeof *v->bs->_buffer);
    zd_init_B(&v->bs->_buffer[0]);
    zd_init_B(&v->bs->_buffer[1]);
    v->bs->_buffer[0]._d = true;
    v->bs->_buffer[0]._u.q = number(5);
    v->es = malloc(sizeof *v->es);
    v->es->_maximum = 2u;
    v->es->_length = 2u;
    v->es->_release = true;
    v->es->_buffer = malloc(2u * sizeof *v->es->_buffer);
    zd_init_E(&v->es->_buffer[0]);
    zd_init_E(&v->es->_buffer[1]);
    v->es->_buffer[0]._d = BLUE;
    v->es->_buffer[0]._u.q = number(6);
    v->hs = malloc(sizeof *v->hs);
    v->hs->_maximum = 2u;
    v->hs->_length = 2u;
    v->hs->_release = true;
    v->hs->_buffer = malloc(2u * sizeof *v->hs->_buffer);
    zd_init_Holder(&v->hs->_buffer[0]);
    zd_init_Holder(&v->hs->_buffer[1]);
    v->hs->_buffer[0].held._d = true;
    v->hs->_buffer[0].held._u.q = number(7);
    v->hs->_buffer[0].z = 8;
    v->hs->_buffer[1].z = 0;
    v->one = malloc(sizeof *v->one);
    zd_init_B(v->one);
    v->one->_d = true;
    v->one->_u.q = number(9);
}
";

#[test]
fn xtypes_7_5_1_2_3_1_the_initialiser_of_a_union_sets_the_discriminator() {
    // "The constructor/initializer of the enclosing object shall set the
    // external member pointers to NULL", and which pointer of a union is
    // the member is what `_d` says: the initialiser writes it, the default
    // value of the discriminator type (Table 9: false, the
    // `@default_literal` of an enumeration), so that the copy and the
    // destructor, which read `_d`, read a value. The storage here holds
    // 0xA5 before: the stack objects by `memset`, every new block by the
    // allocator. An element the copy of a sequence has initialised and not
    // yet copied into shows the discriminator the initialiser gave it: the
    // third, sixth and ninth allocation of the copy are the external
    // branch of the first element of `bs`, `es` and `hs`, and with that
    // one refused the second element is still as initialised.
    let out = run_counting(
        "union-init",
        UNIONS,
        UNION_HELPERS,
        r#"
    B b;
    E e;
    M x;
    M y;
    unsigned before;
    bool ok;
    memset(&b, 0xA5, sizeof b);
    memset(&e, 0xA5, sizeof e);
    zd_init_B(&b);
    zd_init_E(&e);
    printf("init %d %d %d %d\n", (int)b._d, b._u.q == NULL, (int)e._d, e._u.q == NULL);

    fill(&x);
    before = zd_t_live();
    zd_init_M(&y);
    zd_t_count();
    ok = zd_copy_M(&y, &x);
    printf("copy %d calls %u %u\n", ok ? 1 : 0, zd_t_mallocs, zd_t_frees);
    printf("bs %d %d %d %d\n", (int)y.bs->_buffer[0]._d,
           y.bs->_buffer[0]._u.q != x.bs->_buffer[0]._u.q, (int)*y.bs->_buffer[0]._u.q,
           (int)y.bs->_buffer[1]._d);
    printf("es %d %d %d %d\n", (int)y.es->_buffer[0]._d,
           y.es->_buffer[0]._u.q != x.es->_buffer[0]._u.q, (int)*y.es->_buffer[0]._u.q,
           (int)y.es->_buffer[1]._d);
    printf("hs %d %d %d %d %d\n", (int)y.hs->_buffer[0].held._d,
           y.hs->_buffer[0].held._u.q != x.hs->_buffer[0].held._u.q, (int)*y.hs->_buffer[0].held._u.q,
           (int)y.hs->_buffer[0].z, (int)y.hs->_buffer[1].held._d);
    printf("one %d %d %d\n", (int)y.one->_d, y.one->_u.q != x.one->_u.q, (int)*y.one->_u.q);
    zd_t_count();
    zd_fini_M(&y);
    printf("freed %u\n", zd_t_frees);

    zd_init_M(&y);
    zd_t_fail_at = 3u;
    ok = zd_copy_M(&y, &x);
    zd_t_fail_at = 0u;
    printf("bs fails %d length %u second %d %d", ok ? 1 : 0, y.bs->_length,
           (int)y.bs->_buffer[1]._d, y.bs->_buffer[1]._u.q == NULL);
    zd_fini_M(&y);
    printf(" left %u\n", zd_t_live() - before);

    zd_init_M(&y);
    zd_t_fail_at = 6u;
    ok = zd_copy_M(&y, &x);
    zd_t_fail_at = 0u;
    printf("es fails %d length %u second %d %d", ok ? 1 : 0, y.es->_length,
           (int)y.es->_buffer[1]._d, y.es->_buffer[1]._u.q == NULL);
    zd_fini_M(&y);
    printf(" left %u\n", zd_t_live() - before);

    zd_init_M(&y);
    zd_t_fail_at = 9u;
    ok = zd_copy_M(&y, &x);
    zd_t_fail_at = 0u;
    printf("hs fails %d length %u second %d %d", ok ? 1 : 0, y.hs->_length,
           (int)y.hs->_buffer[1].held._d, y.hs->_buffer[1].held._u.q == NULL);
    zd_fini_M(&y);
    printf(" left %u\n", zd_t_live() - before);

    zd_fini_M(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "init 0 1 1 1",
            "copy 1 calls 11 0",
            "bs 1 1 5 0",
            "es 2 1 6 1",
            "hs 1 1 7 8 0",
            "one 1 1 9",
            "freed 11",
            "bs fails 0 length 2 second 0 1 left 0",
            "es fails 0 length 2 second 1 1 left 0",
            "hs fails 0 length 2 second 0 1 left 0",
            "end 0 0",
        ],
        "{out}"
    );
}

/// The program of the two shallow rows of Table 45, which carry one
/// description: "Destination will now point to the same memory address as
/// source. The existing memory pointed to by destination is released
/// before making the assignment."
fn shallow_row(tag: &str, action: &str) -> String {
    run_counting(
        tag,
        SCALARS,
        SCALAR_HELPERS,
        &r#"
    T x;
    T y;
    T none;
    Inner *p;
    int32_t *q;
    Deep *r;
    int32_t *ra;
    bool ok;
    fill(&x, 10);
    fill(&y, 20);
    p = y.p;
    q = y.q;
    r = y.r;
    ra = y.r->a;
    zd_t_count();
    ok = zd_copy_with_T(&y, &x, ZD_COPY_SHALLOW, ACTION);
    printf("copy %d points %d %d %d plain %d\n", ok ? 1 : 0, y.p == x.p, y.q == x.q, y.r == x.r,
           (int)y.z);
    printf("released %d %d %d %d\n", zd_t_freed(p), zd_t_freed(q), zd_t_freed(r), zd_t_freed(ra));
    printf("source %d %d %d %d\n", zd_t_freed(x.p), zd_t_freed(x.q), zd_t_freed(x.r),
           zd_t_freed(x.r->a));
    printf("calls %u %u\n", zd_t_mallocs, zd_t_frees);

    /* A destination that points at the source's objects already. */
    zd_t_count();
    ok = zd_copy_with_T(&y, &x, ZD_COPY_SHALLOW, ACTION);
    printf("again %d points %d %d %d calls %u %u\n", ok ? 1 : 0, y.p == x.p, y.q == x.q,
           y.r == x.r, zd_t_mallocs, zd_t_frees);
    y.p = NULL;
    y.q = NULL;
    y.r = NULL;

    /* A source member that is NULL: the destination's is released and NULL. */
    zd_init_T(&none);
    none.z = 0;
    fill(&y, 20);
    p = y.p;
    q = y.q;
    r = y.r;
    ra = y.r->a;
    zd_t_count();
    ok = zd_copy_with_T(&y, &none, ZD_COPY_SHALLOW, ACTION);
    printf("absent %d null %d %d %d released %d %d %d %d calls %u %u\n", ok ? 1 : 0, y.p == NULL,
           y.q == NULL, y.r == NULL, zd_t_freed(p), zd_t_freed(q), zd_t_freed(r), zd_t_freed(ra),
           zd_t_mallocs, zd_t_frees);
    zd_fini_T(&y);
    zd_fini_T(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#
        .replace("ACTION", action),
    )
}

/// What both shallow rows print: the destination points at the source's
/// three objects, its own four blocks (the member of `r` with it) were
/// released, the source's were not, and nothing was allocated.
const SHALLOW_ROW: [&str; 7] = [
    "copy 1 points 1 1 1 plain 14",
    "released 1 1 1 1",
    "source 0 0 0 0",
    "calls 0 4",
    "again 1 points 1 1 1 calls 0 0",
    "absent 1 null 1 1 1 released 1 1 1 1 calls 0 4",
    "end 0 0",
];

#[test]
fn xtypes_7_5_1_2_3_1_table_45_shallow_replace_releases_the_destination_and_points_at_the_source() {
    let out = shallow_row("shallow-replace", "ZD_COPY_REPLACE");
    assert_eq!(lines(&out), SHALLOW_ROW, "{out}");
}

#[test]
fn xtypes_7_5_1_2_3_1_table_45_shallow_release_releases_the_destination_and_points_at_the_source() {
    let out = shallow_row("shallow-release", "ZD_COPY_RELEASE");
    assert_eq!(lines(&out), SHALLOW_ROW, "{out}");
}

#[test]
fn xtypes_7_5_1_2_3_1_table_45_deep_reuse_copies_into_the_existing_memory() {
    // "(Default) Try to reuse the existing memory to copy into." The
    // pointers of the destination are the ones it had, nothing is
    // allocated or released, and the copy function without the two
    // arguments does the same: it is the default.
    let out = run_counting(
        "deep-reuse",
        SCALARS,
        SCALAR_HELPERS,
        r#"
    T x;
    T y;
    Inner *p;
    int32_t *q;
    Deep *r;
    int32_t *ra;
    bool ok;
    fill(&x, 10);
    fill(&y, 20);
    p = y.p;
    q = y.q;
    r = y.r;
    ra = y.r->a;
    zd_t_count();
    ok = zd_copy_with_T(&y, &x, ZD_COPY_DEEP, ZD_COPY_REUSE);
    printf("reuse %d kept %d %d %d %d values %d %d %d %d calls %u %u\n", ok ? 1 : 0, y.p == p,
           y.q == q, y.r == r, y.r->a == ra, (int)y.p->a, (int)*y.q, (int)*y.r->a, (int)y.z,
           zd_t_mallocs, zd_t_frees);
    x.p->a = 31;
    *x.q = 32;
    *x.r->a = 33;
    x.z = 34;
    zd_t_count();
    ok = zd_copy_T(&y, &x);
    printf("default %d kept %d %d %d %d values %d %d %d %d calls %u %u\n", ok ? 1 : 0, y.p == p,
           y.q == q, y.r == r, y.r->a == ra, (int)y.p->a, (int)*y.q, (int)*y.r->a, (int)y.z,
           zd_t_mallocs, zd_t_frees);
    zd_fini_T(&y);
    zd_fini_T(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "reuse 1 kept 1 1 1 1 values 11 12 13 14 calls 0 0",
            "default 1 kept 1 1 1 1 values 31 32 33 34 calls 0 0",
            "end 0 0",
        ],
        "{out}"
    );
}

#[test]
fn xtypes_7_5_1_2_3_1_table_45_deep_replace_allocates_and_leaves_the_old_memory_to_the_application()
{
    // "Replace the destination member. Allocate new memory to copy into
    // and replace the existing memory without releasing it. It is the
    // application's responsibility to release the replaced memory." The
    // four old blocks are live and hold what they held; the program
    // releases them itself.
    let out = run_counting(
        "deep-replace",
        SCALARS,
        SCALAR_HELPERS,
        r#"
    T x;
    T y;
    T none;
    Inner *p;
    int32_t *q;
    Deep *r;
    int32_t *ra;
    bool ok;
    fill(&x, 10);
    fill(&y, 20);
    p = y.p;
    q = y.q;
    r = y.r;
    ra = y.r->a;
    zd_t_count();
    ok = zd_copy_with_T(&y, &x, ZD_COPY_DEEP, ZD_COPY_REPLACE);
    printf("copy %d new %d %d %d %d values %d %d %d\n", ok ? 1 : 0, y.p != p && y.p != x.p,
           y.q != q && y.q != x.q, y.r != r && y.r != x.r, y.r->a != ra && y.r->a != x.r->a,
           (int)y.p->a, (int)*y.q, (int)*y.r->a);
    printf("old %d %d %d %d holds %d %d %d\n", zd_t_freed(p), zd_t_freed(q), zd_t_freed(r),
           zd_t_freed(ra), (int)p->a, (int)*q, (int)*r->a);
    printf("calls %u %u\n", zd_t_mallocs, zd_t_frees);
    free(p);
    free(q);
    zd_fini_Deep(r);
    free(r);

    /* A source member that is NULL: the destination's is NULL, not released. */
    zd_init_T(&none);
    none.z = 0;
    p = y.p;
    q = y.q;
    r = y.r;
    ra = y.r->a;
    zd_t_count();
    ok = zd_copy_with_T(&y, &none, ZD_COPY_DEEP, ZD_COPY_REPLACE);
    printf("absent %d null %d %d %d old %d %d %d %d calls %u %u\n", ok ? 1 : 0, y.p == NULL,
           y.q == NULL, y.r == NULL, zd_t_freed(p), zd_t_freed(q), zd_t_freed(r), zd_t_freed(ra),
           zd_t_mallocs, zd_t_frees);
    free(p);
    free(q);
    zd_fini_Deep(r);
    free(r);
    zd_fini_T(&y);
    zd_fini_T(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "copy 1 new 1 1 1 1 values 11 12 13",
            "old 0 0 0 0 holds 21 22 23",
            "calls 4 0",
            "absent 1 null 1 1 1 old 0 0 0 0 calls 0 0",
            "end 0 0",
        ],
        "{out}"
    );
}

#[test]
fn xtypes_7_5_1_2_3_1_table_45_deep_release_frees_the_old_memory_before_it_allocates() {
    // "Release the existing memory before allocating new memory to copy
    // into." Each old block was released, the member of `r` with it, and
    // on the allocator's clock before the block that took its place was
    // made.
    let out = run_counting(
        "deep-release",
        SCALARS,
        SCALAR_HELPERS,
        r#"
    T x;
    T y;
    T none;
    Inner *p;
    int32_t *q;
    Deep *r;
    int32_t *ra;
    bool ok;
    fill(&x, 10);
    fill(&y, 20);
    p = y.p;
    q = y.q;
    r = y.r;
    ra = y.r->a;
    zd_t_count();
    ok = zd_copy_with_T(&y, &x, ZD_COPY_DEEP, ZD_COPY_RELEASE);
    printf("copy %d new %d %d %d %d values %d %d %d\n", ok ? 1 : 0, y.p != p && y.p != x.p,
           y.q != q && y.q != x.q, y.r != r && y.r != x.r, y.r->a != ra && y.r->a != x.r->a,
           (int)y.p->a, (int)*y.q, (int)*y.r->a);
    printf("old %d %d %d %d\n", zd_t_freed(p), zd_t_freed(q), zd_t_freed(r), zd_t_freed(ra));
    printf("first %d %d %d %d\n", zd_t_freed_at(p) < zd_t_made_at(y.p),
           zd_t_freed_at(q) < zd_t_made_at(y.q), zd_t_freed_at(r) < zd_t_made_at(y.r),
           zd_t_freed_at(ra) < zd_t_made_at(y.r->a));
    printf("calls %u %u\n", zd_t_mallocs, zd_t_frees);

    /* A source member that is NULL: the destination's is released and NULL. */
    zd_init_T(&none);
    none.z = 0;
    p = y.p;
    q = y.q;
    r = y.r;
    ra = y.r->a;
    zd_t_count();
    ok = zd_copy_with_T(&y, &none, ZD_COPY_DEEP, ZD_COPY_RELEASE);
    printf("absent %d null %d %d %d old %d %d %d %d calls %u %u\n", ok ? 1 : 0, y.p == NULL,
           y.q == NULL, y.r == NULL, zd_t_freed(p), zd_t_freed(q), zd_t_freed(r), zd_t_freed(ra),
           zd_t_mallocs, zd_t_frees);
    zd_fini_T(&y);
    zd_fini_T(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "copy 1 new 1 1 1 1 values 11 12 13",
            "old 1 1 1 1",
            "first 1 1 1 1",
            "calls 4 4",
            "absent 1 null 1 1 1 old 1 1 1 1 calls 0 4",
            "end 0 0",
        ],
        "{out}"
    );
}

#[test]
fn xtypes_7_5_1_2_3_1_a_deep_copy_after_a_shallow_one_does_not_release_the_source() {
    // A shallow copy leaves the destination pointing at the source's
    // objects. They are the source's: a deep RELEASE after it must not
    // free them (the copy would then read what it freed), and a REPLACE
    // and a RELEASE give the destination objects of its own. A REUSE
    // copies an object onto itself, which changes nothing.
    let out = run_counting(
        "deep-after-shallow",
        SCALARS,
        SCALAR_HELPERS,
        r#"
    T x;
    T y;
    int action;
    bool ok;
    fill(&x, 10);
    for (action = 0; action < 3; action++) {
        zd_init_T(&y);
        ok = zd_copy_with_T(&y, &x, ZD_COPY_SHALLOW, ZD_COPY_REUSE);
        zd_t_count();
        ok = zd_copy_with_T(&y, &x, ZD_COPY_DEEP, (zd_copy_action)action) && ok;
        printf("action %d copy %d shared %d %d %d source %d %d %d %d values %d %d %d calls %u %u\n",
               action, ok ? 1 : 0, y.p == x.p, y.q == x.q, y.r == x.r, zd_t_freed(x.p),
               zd_t_freed(x.q), zd_t_freed(x.r), zd_t_freed(x.r->a), (int)y.p->a, (int)*y.q,
               (int)*y.r->a, zd_t_mallocs, zd_t_frees);
        if (y.p == x.p) {
            y.p = NULL;
            y.q = NULL;
            y.r = NULL;
        }
        zd_fini_T(&y);
    }
    zd_fini_T(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "action 0 copy 1 shared 1 1 1 source 0 0 0 0 values 11 12 13 calls 0 0",
            "action 1 copy 1 shared 0 0 0 source 0 0 0 0 values 11 12 13 calls 4 0",
            "action 2 copy 1 shared 0 0 0 source 0 0 0 0 values 11 12 13 calls 4 0",
            "end 0 0",
        ],
        "{out}"
    );
}

#[test]
fn xtypes_7_5_1_2_3_1_an_external_array_is_copied_and_released_element_by_element() {
    // An external array is a pointer to the array (§7.5.1.2.4.4 spells the
    // declarator). Its copy is the array's octets, or, for elements that
    // hold external members or are sequences, the copy of each element.
    let out = run_counting(
        "array",
        "@final struct Deep { @external long a; };\n\
         typedef sequence<long> Row;\n\
         @final struct A { @external long m[2]; @external Deep ds[2]; @external Row rows[2]; };",
        "",
        r#"
    A x;
    A y;
    size_t i;
    bool ok;
    zd_init_A(&x);
    zd_init_A(&y);
    x.m = malloc(sizeof *x.m);
    (*x.m)[0] = 1;
    (*x.m)[1] = 2;
    x.ds = malloc(sizeof *x.ds);
    for (i = 0; i < 2; i++) {
        zd_init_Deep(&(*x.ds)[i]);
        (*x.ds)[i].a = malloc(sizeof(int32_t));
        *(*x.ds)[i].a = 10 + (int32_t)i;
    }
    x.rows = malloc(sizeof *x.rows);
    for (i = 0; i < 2; i++) {
        (*x.rows)[i]._maximum = 2u;
        (*x.rows)[i]._length = 1u + (uint32_t)i;
        (*x.rows)[i]._release = true;
        (*x.rows)[i]._buffer = malloc(2u * sizeof(int32_t));
        (*x.rows)[i]._buffer[0] = 20 + (int32_t)i;
        (*x.rows)[i]._buffer[1] = 30 + (int32_t)i;
    }
    zd_t_count();
    ok = zd_copy_A(&y, &x);
    printf("copy %d own %d %d %d %d values %d %d %d %d calls %u %u\n", ok ? 1 : 0, y.m != x.m,
           y.ds != x.ds, (*y.ds)[0].a != (*x.ds)[0].a, (*y.ds)[1].a != (*x.ds)[1].a,
           (int)(*y.m)[0], (int)(*y.m)[1], (int)*(*y.ds)[0].a, (int)*(*y.ds)[1].a, zd_t_mallocs,
           zd_t_frees);
    /* An array of sequences: each buffer is the copy's, of the source's length. */
    printf("rows %d %d %d lengths %u %u capacities %u %u values %d %d %d\n", y.rows != x.rows,
           (*y.rows)[0]._buffer != (*x.rows)[0]._buffer,
           (*y.rows)[1]._buffer != (*x.rows)[1]._buffer, (*y.rows)[0]._length,
           (*y.rows)[1]._length, (*y.rows)[0]._maximum, (*y.rows)[1]._maximum,
           (int)(*y.rows)[0]._buffer[0], (int)(*y.rows)[1]._buffer[0],
           (int)(*y.rows)[1]._buffer[1]);
    (*x.m)[1] = 3;
    *(*x.ds)[1].a = 12;
    (*x.rows)[1]._buffer[1] = 32;
    zd_t_count();
    ok = zd_copy_A(&y, &x);
    printf("reuse %d values %d %d %d calls %u %u\n", ok ? 1 : 0, (int)(*y.m)[1],
           (int)*(*y.ds)[1].a, (int)(*y.rows)[1]._buffer[1], zd_t_mallocs, zd_t_frees);
    /* The first row of the copy holds one element; two do not fit. */
    (*x.rows)[0]._length = 2u;
    ok = zd_copy_A(&y, &x);
    printf("short %d length %u\n", ok ? 1 : 0, (*y.rows)[0]._length);
    zd_t_count();
    zd_fini_A(&y);
    printf("freed %u\n", zd_t_frees);
    zd_fini_A(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "copy 1 own 1 1 1 1 values 1 2 10 11 calls 7 0",
            "rows 1 1 1 lengths 1 2 capacities 1 2 values 20 21 31",
            "reuse 1 values 3 12 32 calls 0 0",
            "short 0 length 1",
            "freed 7",
            "end 0 0",
        ],
        "{out}"
    );
}

/// A structure whose external members point at a sequence, a map, a
/// sequence of structures that hold an external member, a sequence of
/// sequences, and a string.
const COLLECTIONS: &str = "\
@final struct Item { @external long a; };
typedef sequence<long> Row;
@final struct C {
    @external sequence<long> ids;
    @external map<long, long> table;
    @external sequence<Item> items;
    @external sequence<Row> grid;
    @external string name;
    long z;
};";

/// `text(from)`: the characters of `from` in a block of their own.
/// `fill(v, n, base)`: a `C` whose collections hold `n` elements each and
/// own their buffers, and whose string is "hello": `ids[i]` is
/// `base + i`, `table[i]` is `i + 1 -> base + 50 + i`, `*items[i].a` is
/// `base + 100 + i` and `grid[i]` is `{ base + 200 + i, base + 300 + i }`.
/// With three elements that is fifteen blocks: two for `ids`, two for
/// `table`, five for `items`, five for `grid`, one for `name`.
const COLLECTION_HELPERS: &str = r#"
static char *text(const char *from) {
    size_t size = strlen(from) + 1u;
    char *made = malloc(size);
    memcpy(made, from, size);
    return made;
}

static void fill(C *v, uint32_t n, int32_t base) {
    uint32_t i;
    zd_init_C(v);
    v->ids = malloc(sizeof *v->ids);
    v->ids->_maximum = n;
    v->ids->_length = n;
    v->ids->_release = true;
    v->ids->_buffer = malloc(n * sizeof *v->ids->_buffer);
    v->table = malloc(sizeof *v->table);
    v->table->_maximum = n;
    v->table->_length = n;
    v->table->_release = true;
    v->table->_buffer = malloc(n * sizeof *v->table->_buffer);
    v->items = malloc(sizeof *v->items);
    v->items->_maximum = n;
    v->items->_length = n;
    v->items->_release = true;
    v->items->_buffer = malloc(n * sizeof *v->items->_buffer);
    v->grid = malloc(sizeof *v->grid);
    v->grid->_maximum = n;
    v->grid->_length = n;
    v->grid->_release = true;
    v->grid->_buffer = malloc(n * sizeof *v->grid->_buffer);
    for (i = 0u; i < n; i++) {
        v->ids->_buffer[i] = base + (int32_t)i;
        v->table->_buffer[i].key = (int32_t)i + 1;
        v->table->_buffer[i].value = base + 50 + (int32_t)i;
        zd_init_Item(&v->items->_buffer[i]);
        v->items->_buffer[i].a = malloc(sizeof(int32_t));
        *v->items->_buffer[i].a = base + 100 + (int32_t)i;
        v->grid->_buffer[i]._maximum = 2u;
        v->grid->_buffer[i]._length = 2u;
        v->grid->_buffer[i]._release = true;
        v->grid->_buffer[i]._buffer = malloc(2u * sizeof(int32_t));
        v->grid->_buffer[i]._buffer[0] = base + 200 + (int32_t)i;
        v->grid->_buffer[i]._buffer[1] = base + 300 + (int32_t)i;
    }
    v->name = text("hello");
    v->z = base;
}
"#;

#[test]
fn xtypes_7_5_1_2_3_1_the_deep_copy_of_an_external_sequence_does_not_share_the_buffer() {
    // "The copy function of the enclosing object shall do a deep copy of
    // the external members." What an external pointer to a sequence, a map
    // or a string references is the collection with its elements and the
    // characters: the copy has a header, a buffer and elements of its own
    // (the member of an element and the buffer of a sequence inside a
    // sequence too), writing into it leaves the source as it was, and
    // "The destructor of the enclosing object shall delete the objects
    // referenced by non-NULL external member pointers" releases the
    // fifteen blocks.
    let out = run_counting(
        "collections",
        COLLECTIONS,
        COLLECTION_HELPERS,
        r#"
    C x;
    C y;
    C none;
    C empty;
    bool ok;
    fill(&x, 3u, 10);
    zd_init_C(&y);
    zd_t_count();
    ok = zd_copy_C(&y, &x);
    printf("copy %d calls %u %u\n", ok ? 1 : 0, zd_t_mallocs, zd_t_frees);
    printf("ids %d %d %u %u %d %d %d %d\n", y.ids != x.ids, y.ids->_buffer != x.ids->_buffer,
           y.ids->_length, y.ids->_maximum, y.ids->_release ? 1 : 0, (int)y.ids->_buffer[0],
           (int)y.ids->_buffer[1], (int)y.ids->_buffer[2]);
    printf("table %d %d %u %u %d %d %d\n", y.table != x.table,
           y.table->_buffer != x.table->_buffer, y.table->_length, y.table->_maximum,
           y.table->_release ? 1 : 0, (int)y.table->_buffer[2].key,
           (int)y.table->_buffer[2].value);
    printf("items %d %d %u %u %d %d %d\n", y.items != x.items,
           y.items->_buffer != x.items->_buffer, y.items->_length, y.items->_maximum,
           y.items->_release ? 1 : 0, y.items->_buffer[1].a != x.items->_buffer[1].a,
           (int)*y.items->_buffer[1].a);
    printf("grid %d %d %u %d %u %u %d %d\n", y.grid != x.grid, y.grid->_buffer != x.grid->_buffer,
           y.grid->_length, y.grid->_buffer[1]._buffer != x.grid->_buffer[1]._buffer,
           y.grid->_buffer[1]._length, y.grid->_buffer[1]._maximum,
           y.grid->_buffer[1]._release ? 1 : 0, (int)y.grid->_buffer[1]._buffer[1]);
    printf("name %d %s\n", y.name != x.name, y.name);
    y.ids->_buffer[0] = 70;
    y.table->_buffer[0].value = 71;
    *y.items->_buffer[0].a = 72;
    y.grid->_buffer[0]._buffer[0] = 73;
    printf("source %d %d %d %d\n", (int)x.ids->_buffer[0], (int)x.table->_buffer[0].value,
           (int)*x.items->_buffer[0].a, (int)x.grid->_buffer[0]._buffer[0]);
    zd_t_count();
    zd_fini_C(&y);
    printf("freed %u null %d %d %d %d %d\n", zd_t_frees, y.ids == NULL, y.table == NULL,
           y.items == NULL, y.grid == NULL, y.name == NULL);

    /* A sequence without elements: a header, and no buffer. */
    zd_init_C(&empty);
    empty.z = 0;
    empty.ids = malloc(sizeof *empty.ids);
    empty.ids->_maximum = 0u;
    empty.ids->_length = 0u;
    empty.ids->_buffer = NULL;
    empty.ids->_release = false;
    zd_init_C(&none);
    zd_t_count();
    ok = zd_copy_C(&none, &empty);
    printf("empty %d %d %u %u %d calls %u %u\n", ok ? 1 : 0,
           none.ids != NULL && none.ids != empty.ids, none.ids->_length, none.ids->_maximum,
           none.ids->_buffer == NULL, zd_t_mallocs, zd_t_frees);
    zd_fini_C(&none);
    zd_fini_C(&empty);
    zd_fini_C(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "copy 1 calls 15 0",
            "ids 1 1 3 3 1 10 11 12",
            "table 1 1 3 3 1 3 62",
            "items 1 1 3 3 1 1 111",
            "grid 1 1 3 1 2 2 1 311",
            "name 1 hello",
            "source 10 60 110 210",
            "freed 15 null 1 1 1 1 1",
            "empty 1 1 0 0 1 calls 1 0",
            "end 0 0",
        ],
        "{out}"
    );
}

#[test]
fn xtypes_7_5_1_2_3_1_table_45_deep_reuse_fails_where_the_existing_member_is_not_large_enough() {
    // "If the existing member is not large enough, this operation shall
    // fail", and of the copy function: "This may happen when the
    // destination member is not large enough to hold a copy of the
    // source." A sequence and a map are not large enough where `_maximum`
    // is below the source's `_length`, at every depth; a string where it
    // is shorter than the source. The copy answers false and the member
    // is as it was; with room, the same header and the same buffer hold
    // the copy, and nothing was allocated.
    let out = run_counting(
        "reuse-capacity",
        COLLECTIONS,
        COLLECTION_HELPERS,
        r#"
    C x;
    C y;
    const void *header;
    const void *buffer;
    bool ok;
    zd_init_C(&x);
    zd_init_C(&y);
    x.z = 0;
    y.z = 0;

    x.ids = malloc(sizeof *x.ids);
    x.ids->_maximum = 3u;
    x.ids->_length = 3u;
    x.ids->_release = true;
    x.ids->_buffer = malloc(3u * sizeof *x.ids->_buffer);
    x.ids->_buffer[0] = 1;
    x.ids->_buffer[1] = 2;
    x.ids->_buffer[2] = 3;
    y.ids = malloc(sizeof *y.ids);
    y.ids->_maximum = 2u;
    y.ids->_length = 1u;
    y.ids->_release = true;
    y.ids->_buffer = malloc(2u * sizeof *y.ids->_buffer);
    y.ids->_buffer[0] = 9;
    header = y.ids;
    buffer = y.ids->_buffer;
    zd_t_count();
    ok = zd_copy_C(&y, &x);
    printf("sequence small %d kept %d %d holds %u %u %d calls %u %u\n", ok ? 1 : 0,
           y.ids == header, y.ids->_buffer == buffer, y.ids->_length, y.ids->_maximum,
           (int)y.ids->_buffer[0], zd_t_mallocs, zd_t_frees);
    free(y.ids->_buffer);
    y.ids->_buffer = malloc(4u * sizeof *y.ids->_buffer);
    y.ids->_maximum = 4u;
    y.ids->_length = 0u;
    buffer = y.ids->_buffer;
    zd_t_count();
    ok = zd_copy_C(&y, &x);
    printf("sequence room %d kept %d %d holds %u %u %d %d %d calls %u %u\n", ok ? 1 : 0,
           y.ids == header, y.ids->_buffer == buffer, y.ids->_length, y.ids->_maximum,
           (int)y.ids->_buffer[0], (int)y.ids->_buffer[1], (int)y.ids->_buffer[2], zd_t_mallocs,
           zd_t_frees);
    zd_fini_C(&y);
    zd_fini_C(&x);

    x.table = malloc(sizeof *x.table);
    x.table->_maximum = 2u;
    x.table->_length = 2u;
    x.table->_release = true;
    x.table->_buffer = malloc(2u * sizeof *x.table->_buffer);
    x.table->_buffer[0].key = 1;
    x.table->_buffer[0].value = 10;
    x.table->_buffer[1].key = 2;
    x.table->_buffer[1].value = 20;
    y.table = malloc(sizeof *y.table);
    y.table->_maximum = 1u;
    y.table->_length = 1u;
    y.table->_release = true;
    y.table->_buffer = malloc(sizeof *y.table->_buffer);
    y.table->_buffer[0].key = 7;
    y.table->_buffer[0].value = 70;
    header = y.table;
    buffer = y.table->_buffer;
    zd_t_count();
    ok = zd_copy_C(&y, &x);
    printf("map small %d kept %d %d holds %u %u %d %d calls %u %u\n", ok ? 1 : 0,
           y.table == header, y.table->_buffer == buffer, y.table->_length, y.table->_maximum,
           (int)y.table->_buffer[0].key, (int)y.table->_buffer[0].value, zd_t_mallocs,
           zd_t_frees);
    free(y.table->_buffer);
    y.table->_buffer = malloc(2u * sizeof *y.table->_buffer);
    y.table->_maximum = 2u;
    y.table->_length = 0u;
    buffer = y.table->_buffer;
    zd_t_count();
    ok = zd_copy_C(&y, &x);
    printf("map room %d kept %d %d holds %u %u %d %d %d %d calls %u %u\n", ok ? 1 : 0,
           y.table == header, y.table->_buffer == buffer, y.table->_length, y.table->_maximum,
           (int)y.table->_buffer[0].key, (int)y.table->_buffer[0].value,
           (int)y.table->_buffer[1].key, (int)y.table->_buffer[1].value, zd_t_mallocs,
           zd_t_frees);
    zd_fini_C(&y);
    zd_fini_C(&x);

    /* The sequence inside a sequence is the one that is not large enough. */
    x.grid = malloc(sizeof *x.grid);
    x.grid->_maximum = 1u;
    x.grid->_length = 1u;
    x.grid->_release = true;
    x.grid->_buffer = malloc(sizeof *x.grid->_buffer);
    x.grid->_buffer[0]._maximum = 3u;
    x.grid->_buffer[0]._length = 3u;
    x.grid->_buffer[0]._release = true;
    x.grid->_buffer[0]._buffer = malloc(3u * sizeof(int32_t));
    x.grid->_buffer[0]._buffer[0] = 4;
    x.grid->_buffer[0]._buffer[1] = 5;
    x.grid->_buffer[0]._buffer[2] = 6;
    y.grid = malloc(sizeof *y.grid);
    y.grid->_maximum = 1u;
    y.grid->_length = 1u;
    y.grid->_release = true;
    y.grid->_buffer = malloc(sizeof *y.grid->_buffer);
    y.grid->_buffer[0]._maximum = 2u;
    y.grid->_buffer[0]._length = 0u;
    y.grid->_buffer[0]._release = true;
    y.grid->_buffer[0]._buffer = malloc(2u * sizeof(int32_t));
    buffer = y.grid->_buffer[0]._buffer;
    zd_t_count();
    ok = zd_copy_C(&y, &x);
    printf("inside small %d kept %d holds %u %u calls %u %u\n", ok ? 1 : 0,
           y.grid->_buffer[0]._buffer == buffer, y.grid->_buffer[0]._length,
           y.grid->_buffer[0]._maximum, zd_t_mallocs, zd_t_frees);
    free(y.grid->_buffer[0]._buffer);
    y.grid->_buffer[0]._buffer = malloc(3u * sizeof(int32_t));
    y.grid->_buffer[0]._maximum = 3u;
    buffer = y.grid->_buffer[0]._buffer;
    zd_t_count();
    ok = zd_copy_C(&y, &x);
    printf("inside room %d kept %d holds %u %u %d %d %d calls %u %u\n", ok ? 1 : 0,
           y.grid->_buffer[0]._buffer == buffer, y.grid->_buffer[0]._length,
           y.grid->_buffer[0]._maximum, (int)y.grid->_buffer[0]._buffer[0],
           (int)y.grid->_buffer[0]._buffer[1], (int)y.grid->_buffer[0]._buffer[2], zd_t_mallocs,
           zd_t_frees);
    zd_fini_C(&y);
    zd_fini_C(&x);

    x.name = text("hello");
    y.name = text("hi");
    buffer = y.name;
    zd_t_count();
    ok = zd_copy_C(&y, &x);
    printf("string small %d kept %d holds %s calls %u %u\n", ok ? 1 : 0, y.name == buffer, y.name,
           zd_t_mallocs, zd_t_frees);
    free((char *)y.name);
    y.name = text("world!");
    buffer = y.name;
    zd_t_count();
    ok = zd_copy_C(&y, &x);
    printf("string room %d kept %d holds %s calls %u %u\n", ok ? 1 : 0, y.name == buffer, y.name,
           zd_t_mallocs, zd_t_frees);
    zd_fini_C(&y);
    zd_fini_C(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "sequence small 0 kept 1 1 holds 1 2 9 calls 0 0",
            "sequence room 1 kept 1 1 holds 3 4 1 2 3 calls 0 0",
            "map small 0 kept 1 1 holds 1 1 7 70 calls 0 0",
            "map room 1 kept 1 1 holds 2 2 1 10 2 20 calls 0 0",
            "inside small 0 kept 1 holds 0 2 calls 0 0",
            "inside room 1 kept 1 holds 3 3 4 5 6 calls 0 0",
            "string small 0 kept 1 holds hi calls 0 0",
            "string room 1 kept 1 holds hello calls 0 0",
            "end 0 0",
        ],
        "{out}"
    );
}

#[test]
fn xtypes_7_5_1_2_3_1_a_reuse_of_an_external_sequence_releases_what_it_drops_and_initialises_what_it_adds()
 {
    // The elements of a sequence are objects, the slots behind them are
    // storage. A reuse with a shorter source finalises the elements it
    // drops (their external members are released); with a longer one it
    // initialises the slots it takes, whatever they held, and then copies
    // into them. The elements that stay keep their memory.
    let out = run_counting(
        "reuse-elements",
        COLLECTIONS,
        COLLECTION_HELPERS,
        r#"
    C x;
    C y;
    int32_t *a0;
    int32_t *a1;
    uint32_t i;
    bool ok;
    zd_init_C(&x);
    zd_init_C(&y);
    x.z = 0;
    y.z = 0;
    x.items = malloc(sizeof *x.items);
    x.items->_maximum = 3u;
    x.items->_length = 3u;
    x.items->_release = true;
    x.items->_buffer = malloc(3u * sizeof *x.items->_buffer);
    for (i = 0u; i < 3u; i++) {
        zd_init_Item(&x.items->_buffer[i]);
        x.items->_buffer[i].a = malloc(sizeof(int32_t));
        *x.items->_buffer[i].a = 5 + (int32_t)i;
    }
    y.items = malloc(sizeof *y.items);
    y.items->_maximum = 3u;
    y.items->_length = 2u;
    y.items->_release = true;
    y.items->_buffer = malloc(3u * sizeof *y.items->_buffer);
    for (i = 0u; i < 2u; i++) {
        zd_init_Item(&y.items->_buffer[i]);
        y.items->_buffer[i].a = malloc(sizeof(int32_t));
        *y.items->_buffer[i].a = 1 + (int32_t)i;
    }
    memset(&y.items->_buffer[2], 0x55, sizeof y.items->_buffer[2]);
    a0 = y.items->_buffer[0].a;
    a1 = y.items->_buffer[1].a;

    x.items->_length = 1u;
    zd_t_count();
    ok = zd_copy_C(&y, &x);
    printf("fewer %d length %u kept %d value %d dropped %d calls %u %u\n", ok ? 1 : 0,
           y.items->_length, y.items->_buffer[0].a == a0, (int)*y.items->_buffer[0].a,
           zd_t_freed(a1), zd_t_mallocs, zd_t_frees);

    x.items->_length = 3u;
    memset(&y.items->_buffer[1], 0x55, 2u * sizeof y.items->_buffer[1]);
    zd_t_count();
    ok = zd_copy_C(&y, &x);
    printf("more %d length %u kept %d values %d %d %d calls %u %u\n", ok ? 1 : 0,
           y.items->_length, y.items->_buffer[0].a == a0, (int)*y.items->_buffer[0].a,
           (int)*y.items->_buffer[1].a, (int)*y.items->_buffer[2].a, zd_t_mallocs, zd_t_frees);
    zd_t_count();
    zd_fini_C(&y);
    printf("freed %u\n", zd_t_frees);
    zd_fini_C(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "fewer 1 length 1 kept 1 value 5 dropped 1 calls 0 1",
            "more 1 length 3 kept 1 values 5 6 7 calls 2 0",
            "freed 5",
            "end 0 0",
        ],
        "{out}"
    );
}

#[test]
fn xtypes_7_5_1_2_3_1_table_45_replace_and_release_of_an_external_sequence_allocate_a_header_and_a_buffer()
 {
    // The two deep rows that do not reuse, for a member whose object is
    // more than one block: REPLACE leaves the old header and the old
    // buffer live and as they were, RELEASE frees both before the new
    // header is made.
    let out = run_counting(
        "sequence-rows",
        COLLECTIONS,
        COLLECTION_HELPERS,
        r#"
    C x;
    C y;
    zd_seq_long *header;
    int32_t *buffer;
    bool ok;
    zd_init_C(&x);
    zd_init_C(&y);
    x.z = 0;
    y.z = 0;
    x.ids = malloc(sizeof *x.ids);
    x.ids->_maximum = 3u;
    x.ids->_length = 3u;
    x.ids->_release = true;
    x.ids->_buffer = malloc(3u * sizeof *x.ids->_buffer);
    x.ids->_buffer[0] = 1;
    x.ids->_buffer[1] = 2;
    x.ids->_buffer[2] = 3;
    y.ids = malloc(sizeof *y.ids);
    y.ids->_maximum = 2u;
    y.ids->_length = 1u;
    y.ids->_release = true;
    y.ids->_buffer = malloc(2u * sizeof *y.ids->_buffer);
    y.ids->_buffer[0] = 9;
    header = y.ids;
    buffer = y.ids->_buffer;
    zd_t_count();
    ok = zd_copy_with_C(&y, &x, ZD_COPY_DEEP, ZD_COPY_REPLACE);
    printf("replace %d new %d %d holds %u %u %d old %d %d %u %d calls %u %u\n", ok ? 1 : 0,
           y.ids != header, y.ids->_buffer != buffer, y.ids->_length, y.ids->_maximum,
           (int)y.ids->_buffer[2], zd_t_freed(header), zd_t_freed(buffer), header->_length,
           (int)buffer[0], zd_t_mallocs, zd_t_frees);
    free(buffer);
    free(header);

    header = y.ids;
    buffer = y.ids->_buffer;
    zd_t_count();
    ok = zd_copy_with_C(&y, &x, ZD_COPY_DEEP, ZD_COPY_RELEASE);
    printf("release %d new %d %d holds %u %u %d old %d %d first %d calls %u %u\n", ok ? 1 : 0,
           y.ids != header, y.ids->_buffer != buffer, y.ids->_length, y.ids->_maximum,
           (int)y.ids->_buffer[2], zd_t_freed(header), zd_t_freed(buffer),
           zd_t_freed_at(header) < zd_t_made_at(y.ids), zd_t_mallocs, zd_t_frees);
    zd_fini_C(&y);
    zd_fini_C(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "replace 1 new 1 1 holds 3 3 3 old 0 0 1 9 calls 2 0",
            "release 1 new 1 1 holds 3 3 3 old 1 1 first 1 calls 2 2",
            "end 0 0",
        ],
        "{out}"
    );
}

#[test]
fn xtypes_7_5_1_2_3_1_an_external_string_is_released_and_copied_as_table_45_says() {
    // "String and wide string members are already represented using
    // pointers, so the mappings for these members do not change": the
    // pointer of an external string is an external member pointer, and
    // what it references are the characters. The rows of Table 45 on them.
    let out = run_counting(
        "string-rows",
        COLLECTIONS,
        COLLECTION_HELPERS,
        r#"
    C x;
    C y;
    const char *old;
    bool ok;
    zd_init_C(&x);
    zd_init_C(&y);
    x.z = 0;
    y.z = 0;
    x.name = text("hello");

    y.name = text("old");
    old = y.name;
    zd_t_count();
    ok = zd_copy_with_C(&y, &x, ZD_COPY_SHALLOW, ZD_COPY_RELEASE);
    printf("shallow %d same %d old %d calls %u %u\n", ok ? 1 : 0, y.name == x.name,
           zd_t_freed(old), zd_t_mallocs, zd_t_frees);
    ok = zd_copy_with_C(&y, &x, ZD_COPY_DEEP, ZD_COPY_REUSE);
    printf("shared %d same %d source %d\n", ok ? 1 : 0, y.name == x.name, zd_t_freed(x.name));
    ok = zd_copy_with_C(&y, &x, ZD_COPY_DEEP, ZD_COPY_RELEASE);
    printf("own %d same %d source %d holds %s\n", ok ? 1 : 0, y.name == x.name,
           zd_t_freed(x.name), y.name);
    free((char *)y.name);

    y.name = text("older");
    old = y.name;
    zd_t_count();
    ok = zd_copy_with_C(&y, &x, ZD_COPY_DEEP, ZD_COPY_REPLACE);
    printf("replace %d new %d holds %s old %d %s calls %u %u\n", ok ? 1 : 0,
           y.name != old && y.name != x.name, y.name, zd_t_freed(old), old, zd_t_mallocs,
           zd_t_frees);
    free((char *)old);

    old = y.name;
    zd_t_count();
    ok = zd_copy_with_C(&y, &x, ZD_COPY_DEEP, ZD_COPY_RELEASE);
    printf("release %d new %d holds %s old %d first %d calls %u %u\n", ok ? 1 : 0,
           y.name != old && y.name != x.name, y.name, zd_t_freed(old),
           zd_t_freed_at(old) < zd_t_made_at(y.name), zd_t_mallocs, zd_t_frees);

    old = y.name;
    free((char *)x.name);
    x.name = text("hey");
    zd_t_count();
    ok = zd_copy_with_C(&y, &x, ZD_COPY_DEEP, ZD_COPY_REUSE);
    printf("reuse %d kept %d holds %s calls %u %u\n", ok ? 1 : 0, y.name == old, y.name,
           zd_t_mallocs, zd_t_frees);
    zd_t_count();
    zd_fini_C(&y);
    zd_fini_C(&x);
    printf("freed %u null %d %d\n", zd_t_frees, y.name == NULL, x.name == NULL);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "shallow 1 same 1 old 1 calls 0 1",
            "shared 1 same 1 source 0",
            "own 1 same 0 source 0 holds hello",
            "replace 1 new 1 holds hello old 0 older calls 1 0",
            "release 1 new 1 holds hello old 1 first 1 calls 1 1",
            "reuse 1 kept 1 holds hey calls 0 0",
            "freed 2 null 1 1",
            "end 0 0",
        ],
        "{out}"
    );
}

#[test]
fn xtypes_7_5_1_2_3_1_a_copy_of_collections_that_fails_leaves_an_object_the_destructor_releases() {
    // "If the (recursive call to the) copy operation of the external
    // member fails, then the copy function of the containing object shall
    // fail as well", for members whose objects are several blocks. The
    // copy asks for fifteen blocks; each refused in turn fails it, and
    // what the destination holds then is released by its destructor: no
    // block is left, none is released twice.
    let out = run_counting(
        "collections-fail",
        COLLECTIONS,
        COLLECTION_HELPERS,
        r#"
    C x;
    C y;
    unsigned n;
    unsigned before;
    unsigned failed = 0u;
    unsigned left = 0u;
    bool ok;
    fill(&x, 3u, 10);
    before = zd_t_live();
    for (n = 1u; n <= 16u; n++) {
        zd_init_C(&y);
        zd_t_fail_at = n;
        ok = zd_copy_C(&y, &x);
        zd_t_fail_at = 0u;
        if (!ok) {
            failed++;
        }
        zd_fini_C(&y);
        left += zd_t_live() - before;
    }
    printf("failed %u left %u\n", failed, left);
    zd_fini_C(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(lines(&out), ["failed 15 left 0", "end 0 0"], "{out}");
}

/// A structure whose external members point at a map whose values hold an
/// external member, a map with a `string` key whose values are sequences,
/// a `wstring` and a `fixed`.
const OWNING_MAPS: &str = "\
@final struct Item { @external long a; };
typedef sequence<long> Row;
@final struct W {
    @external map<long, Item> items;
    @external map<string, Row> rows;
    @external wstring wide;
    @external fixed<7,2> amount;
};";

/// `fill(v, n, base, keys, wide, amount)`: a `W` whose maps hold `n`
/// entries each and own their buffers: `items[i]` is
/// `base + i -> { base + 100 + i }` and `rows[i]` is
/// `keys[i] -> { base + 200 + i, base + 300 + i }`, and whose `wstring`
/// and `fixed` are the characters given, in blocks of their own. With
/// three entries that is twelve blocks: five for `items`, five for
/// `rows`, two for the characters.
const OWNING_MAP_HELPERS: &str = r#"
static const char *const KEYS_A[3] = { "a0", "a1", "a2" };
static const char *const KEYS_B[3] = { "b0", "b1", "b2" };

static char *text(const char *from) {
    size_t size = strlen(from) + 1u;
    char *made = malloc(size);
    memcpy(made, from, size);
    return made;
}

static void fill(W *v, uint32_t n, int32_t base, const char *const *keys, const char *wide,
                 const char *amount) {
    uint32_t i;
    zd_init_W(v);
    v->items = malloc(sizeof *v->items);
    v->items->_maximum = n;
    v->items->_length = n;
    v->items->_release = true;
    v->items->_buffer = malloc(n * sizeof *v->items->_buffer);
    v->rows = malloc(sizeof *v->rows);
    v->rows->_maximum = n;
    v->rows->_length = n;
    v->rows->_release = true;
    v->rows->_buffer = malloc(n * sizeof *v->rows->_buffer);
    for (i = 0u; i < n; i++) {
        v->items->_buffer[i].key = base + (int32_t)i;
        zd_init_Item(&v->items->_buffer[i].value);
        v->items->_buffer[i].value.a = malloc(sizeof(int32_t));
        *v->items->_buffer[i].value.a = base + 100 + (int32_t)i;
        v->rows->_buffer[i].key = keys[i];
        v->rows->_buffer[i].value._maximum = 2u;
        v->rows->_buffer[i].value._length = 2u;
        v->rows->_buffer[i].value._release = true;
        v->rows->_buffer[i].value._buffer = malloc(2u * sizeof(int32_t));
        v->rows->_buffer[i].value._buffer[0] = base + 200 + (int32_t)i;
        v->rows->_buffer[i].value._buffer[1] = base + 300 + (int32_t)i;
    }
    v->wide = text(wide);
    v->amount = text(amount);
}
"#;

#[test]
fn xtypes_7_5_1_2_3_1_the_deep_copy_of_an_external_map_copies_what_its_values_own() {
    // "The copy function of the enclosing object shall do a deep copy of
    // the external members", for a map whose values own storage, a
    // `wstring` and a `fixed`: the value of an entry is copied by the
    // functions of its type (the external member of a structure, the
    // buffer of a sequence), its key is assigned, and the characters of
    // the two texts are copied. Twelve blocks, which the destructor
    // releases. A destination that exists is reused: the entries it drops
    // are finalised, the ones it adds are initialised and given storage,
    // and the texts are written into the characters that are there.
    let out = run_counting(
        "owning-maps",
        OWNING_MAPS,
        OWNING_MAP_HELPERS,
        r#"
    W x;
    W y;
    W two;
    const void *header;
    const void *buffer;
    const void *first;
    const void *dropped;
    const void *row;
    const void *dropped_row;
    const void *wide;
    const void *amount;
    bool ok;
    fill(&x, 3u, 10, KEYS_A, "wide", "123.45");
    zd_init_W(&y);
    zd_t_count();
    ok = zd_copy_W(&y, &x);
    printf("copy %d calls %u %u\n", ok ? 1 : 0, zd_t_mallocs, zd_t_frees);
    printf("items %d %d %u %u %d %d %d %d\n", y.items != x.items,
           y.items->_buffer != x.items->_buffer, y.items->_length, y.items->_maximum,
           y.items->_release ? 1 : 0, (int)y.items->_buffer[2].key,
           y.items->_buffer[2].value.a != x.items->_buffer[2].value.a,
           (int)*y.items->_buffer[2].value.a);
    printf("rows %d %d %u %u %d %s %d %d %u %u %d %d %d\n", y.rows != x.rows,
           y.rows->_buffer != x.rows->_buffer, y.rows->_length, y.rows->_maximum,
           y.rows->_release ? 1 : 0, y.rows->_buffer[1].key,
           y.rows->_buffer[1].key == x.rows->_buffer[1].key,
           y.rows->_buffer[1].value._buffer != x.rows->_buffer[1].value._buffer,
           y.rows->_buffer[1].value._length, y.rows->_buffer[1].value._maximum,
           y.rows->_buffer[1].value._release ? 1 : 0, (int)y.rows->_buffer[1].value._buffer[0],
           (int)y.rows->_buffer[1].value._buffer[1]);
    printf("text %d %s %d %s\n", y.wide != x.wide, y.wide, y.amount != x.amount, y.amount);
    *y.items->_buffer[0].value.a = 70;
    y.rows->_buffer[0].value._buffer[0] = 71;
    ((char *)y.wide)[0] = 'W';
    ((char *)y.amount)[0] = '9';
    printf("source %d %d %s %s\n", (int)*x.items->_buffer[0].value.a,
           (int)x.rows->_buffer[0].value._buffer[0], x.wide, x.amount);
    zd_t_count();
    zd_fini_W(&y);
    printf("freed %u null %d %d %d %d\n", zd_t_frees, y.items == NULL, y.rows == NULL,
           y.wide == NULL, y.amount == NULL);

    /* Three entries take a copy of two: the third is finalised. */
    fill(&y, 3u, 50, KEYS_B, "wider!!", "99999.99");
    fill(&two, 2u, 20, KEYS_A, "wider", "1234.56");
    header = y.items;
    buffer = y.items->_buffer;
    first = y.items->_buffer[0].value.a;
    dropped = y.items->_buffer[2].value.a;
    row = y.rows->_buffer[0].value._buffer;
    dropped_row = y.rows->_buffer[2].value._buffer;
    wide = y.wide;
    amount = y.amount;
    zd_t_count();
    ok = zd_copy_W(&y, &two);
    printf("shrink %d kept %d %d %d %d %d %d dropped %d %d calls %u %u\n", ok ? 1 : 0,
           y.items == header, y.items->_buffer == buffer, y.items->_buffer[0].value.a == first,
           y.rows->_buffer[0].value._buffer == row, y.wide == wide, y.amount == amount,
           zd_t_freed(dropped), zd_t_freed(dropped_row), zd_t_mallocs, zd_t_frees);
    printf("holds %u %u %d %d %u %u %s %d %s %s\n", y.items->_length, y.items->_maximum,
           (int)y.items->_buffer[1].key, (int)*y.items->_buffer[1].value.a, y.rows->_length,
           y.rows->_maximum, y.rows->_buffer[1].key, (int)y.rows->_buffer[1].value._buffer[1],
           y.wide, y.amount);

    /* And a copy of three again: the third is initialised and gets storage. */
    zd_t_count();
    ok = zd_copy_W(&y, &x);
    printf("grow %d kept %d %d holds %u %d %d %u %s %d %d calls %u %u\n", ok ? 1 : 0,
           y.items == header, y.items->_buffer == buffer, y.items->_length,
           (int)y.items->_buffer[2].key, (int)*y.items->_buffer[2].value.a, y.rows->_length,
           y.rows->_buffer[2].key, (int)y.rows->_buffer[2].value._buffer[0],
           y.rows->_buffer[2].value._release ? 1 : 0, zd_t_mallocs, zd_t_frees);
    printf("text %d %d %s %s\n", y.wide == wide, y.amount == amount, y.wide, y.amount);
    zd_fini_W(&two);
    zd_fini_W(&y);
    zd_fini_W(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "copy 1 calls 12 0",
            "items 1 1 3 3 1 12 1 112",
            "rows 1 1 3 3 1 a1 1 1 2 2 1 211 311",
            "text 1 wide 1 123.45",
            "source 110 210 wide 123.45",
            "freed 12 null 1 1 1 1",
            "shrink 1 kept 1 1 1 1 1 1 dropped 1 1 calls 0 2",
            "holds 2 3 21 121 2 3 a1 321 wider 1234.56",
            "grow 1 kept 1 1 holds 3 12 112 3 a2 212 1 calls 2 0",
            "text 1 1 wide 123.45",
            "end 0 0",
        ],
        "{out}"
    );
}

#[test]
fn xtypes_7_5_1_2_3_1_table_45_deep_reuse_fails_for_a_map_a_map_value_a_wstring_and_a_fixed_that_are_too_small()
 {
    // "If the existing member is not large enough, this operation shall
    // fail." A map whose values own storage is not large enough where its
    // `_maximum` is below the source's `_length`, a sequence that is the
    // value of an entry likewise, a `wstring` and a `fixed` where they are
    // shorter than the source. The copy answers false, and the collection
    // or the text that is too small is as it was. What the copy reached
    // before it holds the copy: the member in front of it, the entry in
    // front of the one that failed, and the key of the entry whose value
    // is too small. The destructor releases the result.
    let out = run_counting(
        "owning-maps-small",
        OWNING_MAPS,
        OWNING_MAP_HELPERS,
        r#"
    W x;
    W y;
    const void *kept;
    bool ok;
    fill(&x, 2u, 10, KEYS_A, "wide", "123.45");

    fill(&y, 1u, 50, KEYS_B, "wider!", "9999.99");
    kept = y.items->_buffer;
    zd_t_count();
    ok = zd_copy_W(&y, &x);
    printf("items small %d kept %d holds %u %u %d %d calls %u %u\n", ok ? 1 : 0,
           y.items->_buffer == kept, y.items->_length, y.items->_maximum,
           (int)y.items->_buffer[0].key, (int)*y.items->_buffer[0].value.a, zd_t_mallocs,
           zd_t_frees);
    zd_fini_W(&y);

    /* The second map is the one that is too small; the first holds the copy. */
    fill(&y, 2u, 50, KEYS_B, "wider!", "9999.99");
    free(y.rows->_buffer[1].value._buffer);
    y.rows->_length = 1u;
    y.rows->_maximum = 1u;
    kept = y.rows->_buffer;
    zd_t_count();
    ok = zd_copy_W(&y, &x);
    printf("rows small %d kept %d holds %u %u %s %d before %d %d calls %u %u\n", ok ? 1 : 0,
           y.rows->_buffer == kept, y.rows->_length, y.rows->_maximum, y.rows->_buffer[0].key,
           (int)y.rows->_buffer[0].value._buffer[0], (int)y.items->_buffer[1].key,
           (int)*y.items->_buffer[1].value.a, zd_t_mallocs, zd_t_frees);
    zd_fini_W(&y);

    /* The value of the second entry is the sequence that is too small; the
     * first entry holds the copy. */
    fill(&y, 2u, 50, KEYS_B, "wider!", "9999.99");
    y.rows->_buffer[1].value._maximum = 1u;
    y.rows->_buffer[1].value._length = 1u;
    kept = y.rows->_buffer[1].value._buffer;
    zd_t_count();
    ok = zd_copy_W(&y, &x);
    printf("value small %d kept %d holds %u %u %d key %s before %s %d %d calls %u %u\n",
           ok ? 1 : 0, y.rows->_buffer[1].value._buffer == kept,
           y.rows->_buffer[1].value._length, y.rows->_buffer[1].value._maximum,
           (int)y.rows->_buffer[1].value._buffer[0], y.rows->_buffer[1].key,
           y.rows->_buffer[0].key, (int)y.rows->_buffer[0].value._buffer[0],
           (int)y.rows->_buffer[0].value._buffer[1], zd_t_mallocs, zd_t_frees);
    zd_fini_W(&y);

    fill(&y, 2u, 50, KEYS_B, "wid", "9999.99");
    kept = y.wide;
    zd_t_count();
    ok = zd_copy_W(&y, &x);
    printf("wstring small %d kept %d holds %s calls %u %u\n", ok ? 1 : 0, y.wide == kept, y.wide,
           zd_t_mallocs, zd_t_frees);
    zd_fini_W(&y);

    fill(&y, 2u, 50, KEYS_B, "wider!", "9.99");
    kept = y.amount;
    zd_t_count();
    ok = zd_copy_W(&y, &x);
    printf("fixed small %d kept %d holds %s before %s calls %u %u\n", ok ? 1 : 0,
           y.amount == kept, y.amount, y.wide, zd_t_mallocs, zd_t_frees);
    zd_fini_W(&y);
    zd_fini_W(&x);
    printf("end %u %u\n", zd_t_live(), zd_t_bad);
"#,
    );
    assert_eq!(
        lines(&out),
        [
            "items small 0 kept 1 holds 1 1 50 150 calls 0 0",
            "rows small 0 kept 1 holds 1 1 b0 250 before 11 111 calls 0 0",
            "value small 0 kept 1 holds 1 1 251 key a1 before a0 210 310 calls 0 0",
            "wstring small 0 kept 1 holds wid calls 0 0",
            "fixed small 0 kept 1 holds 9.99 before wide calls 0 0",
            "end 0 0",
        ],
        "{out}"
    );
}
