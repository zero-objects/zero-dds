// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-collection enforcement (DDS-XTypes §7.4.3) in the generated C
//! backend (`src/c_mode.rs`, `generate_c_header`) — a SEPARATE emitter from
//! the C++ backend (`src/emitter.rs`, `generate_cpp_header`) covered by
//! `bounded_collections.rs`, sharing only this crate.
//!
//! B1 follow-up: the C backend already had an encode-side bound check
//! (`emit_member_write`/`emit_wstring_write`/`emit_sequence_write`/
//! `emit_map_write`, `goto fail;` on over-bound) but NO decode-side
//! counterpart — the same gap the C++/C#/Java backends had. A bounded field
//! decoded a well-formed-but-oversized wire value without complaint
//! (untrusted-input DoS vector; XTypes 1.3 §7.4.3 requires enforcement on
//! BOTH sides). Fixed in `emit_member_read` (inline string arm),
//! `emit_wstring_read`, `emit_sequence_read`, `emit_map_read` — all return a
//! distinct `-14` (vs. the generic `-7` read-failure code) so callers can
//! tell a bound violation apart from a malformed/truncated wire buffer.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    missing_docs
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CGenOptions, generate_c_header};

fn gcc_available() -> bool {
    Command::new("gcc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn c_include_dir() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/../zerodds-c-api/include").to_string()
}

fn gen_c(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_c_header(&ast, &CGenOptions::default()).expect("c-gen")
}

/// Compile `header` + `main_src` and run; require exit 0.
fn compile_and_run(header: &str, main_src: &str) -> Result<(), String> {
    compile_and_run_with_flags(header, main_src, &[])
}

/// Like [`compile_and_run`] but with extra compiler flags (e.g. `-fsanitize=...`).
fn compile_and_run_with_flags(
    header: &str,
    main_src: &str,
    extra_flags: &[&str],
) -> Result<(), String> {
    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let hdr = dir.path().join("gen.h");
    std::fs::write(&hdr, header).map_err(|e| format!("write hdr: {e}"))?;
    let main = dir.path().join("main.c");
    std::fs::write(&main, main_src).map_err(|e| format!("write main: {e}"))?;
    let bin = dir.path().join("a.out");
    let compile = Command::new("gcc")
        .args(["-std=c99", "-I"])
        .arg(c_include_dir())
        .arg("-I")
        .arg(dir.path())
        .args(extra_flags)
        .arg(&main)
        .arg("-o")
        .arg(&bin)
        .output()
        .map_err(|e| format!("spawn gcc: {e}"))?;
    if !compile.status.success() {
        return Err(format!(
            "compile failed:\n{}",
            String::from_utf8_lossy(&compile.stderr)
        ));
    }
    let run = Command::new(&bin)
        .output()
        .map_err(|e| format!("spawn bin: {e}"))?;
    if run.status.success() {
        Ok(())
    } else {
        Err(format!(
            "test binary failed (exit {:?}):\n{}",
            run.status.code(),
            String::from_utf8_lossy(&run.stdout)
        ))
    }
}

// ---------------------------------------------------------------------
// Codegen-string assertions (fast, no compiler required).
// ---------------------------------------------------------------------

#[test]
fn decode_rejects_over_bound_string() {
    let c = gen_c("@final struct Named { string<8> name; };");
    assert!(
        c.contains("strlen(s->name) > 8u) { free(s->name); (s->name) = NULL; return -14; }"),
        "decode of bounded string<8> must reject over-bound with -14 AND free the \
         already-`zerodds_xcdr2_c_read_string`-allocated buffer first (moderate fix, \
         deep review of #22 — the string path has no wire length available before \
         the shared read helper allocates, unlike wstring/seq/map which check \
         before allocating):\n{c}"
    );
}

#[test]
fn decode_rejects_over_bound_wstring() {
    let c = gen_c("@final struct Named { wstring<8> name; };");
    assert!(
        c.contains("if (ws_n > 8u) return -14;"),
        "decode of bounded wstring<8> must reject over-bound with -14:\n{c}"
    );
}

#[test]
fn decode_rejects_over_bound_sequence() {
    let c = gen_c("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        c.contains("if (seq_len > 4u) return -14;"),
        "decode of bounded sequence<octet,4> must reject over-bound with -14:\n{c}"
    );
}

#[test]
fn decode_rejects_over_bound_map() {
    let c = gen_c("@final struct M { map<string, long, 2> vals; };");
    assert!(
        c.contains("if (map_len > 2u) return -14;"),
        "decode of bounded map<string,long,2> must reject over-bound with -14:\n{c}"
    );
}

#[test]
fn unbounded_decode_no_bound_check() {
    let c = gen_c("@final struct Free { string name; sequence<long> vals; };");
    assert!(
        !c.contains("return -14;"),
        "unbounded string/sequence must NOT get a decode-side bound check:\n{c}"
    );
}

// ---------------------------------------------------------------------
// Real gcc compile + run: forge an over-bound wire value via the runtime
// helpers directly (bypassing the generated encoder's OWN bound check, the
// same "adversarial sender" shape the other backends' tests use) and prove
// the generated decoder rejects it at runtime, not just in generated text.
// ---------------------------------------------------------------------

#[test]
#[ignore = "requires gcc in PATH"]
fn decode_rejects_over_bound_string_at_runtime() {
    if !gcc_available() {
        return;
    }
    let h = gen_c("@final struct Named { string<8> name; };");
    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
int main(void){
    /* Within-bound roundtrip must still work (no false positives). */
    Named_t ok; memset(&ok, 0, sizeof ok);
    ok.name = "short";
    uint8_t buf[512]; size_t n = 0;
    assert(Named_encode(&ok, buf, sizeof buf, &n) == 0);
    Named_t back; memset(&back, 0, sizeof back);
    assert(Named_decode(buf, n, &back) == 0);
    assert(strcmp(back.name, "short") == 0);

    /* Forge a well-formed wire value whose string content (11 bytes)
     * exceeds the IDL bound (8) via the low-level runtime writer directly
     * -- bypassing Named_encode's OWN bound check, which only guards
     * honest senders, not adversarial/foreign ones. */
    uint8_t* w_buf = NULL; size_t w_len = 0; size_t w_cap = 0;
    assert(zerodds_xcdr2_c_write_string(&w_buf, &w_len, &w_cap, "way-too-long") == 0);
    Named_t forged; memset(&forged, 0, sizeof forged);
    int rc = Named_decode(w_buf, w_len, &forged);
    free(w_buf);
    assert(rc == -14 && "decode must reject an over-bound string with -14, not silently accept it");
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("runtime bound-rejection test failed:\n{e}");
    }
}

#[test]
#[ignore = "requires gcc in PATH"]
fn decode_rejects_over_bound_sequence_at_runtime() {
    if !gcc_available() {
        return;
    }
    let h = gen_c("@final struct Cap { sequence<octet, 4> data; };");
    let main = r#"
#include "gen.h"
#include <assert.h>
int main(void){
    /* Within-bound roundtrip. */
    Cap_t ok; memset(&ok, 0, sizeof ok);
    uint8_t d[3] = {1,2,3};
    ok.data.len = 3; ok.data.elems = d;
    uint8_t buf[512]; size_t n = 0;
    assert(Cap_encode(&ok, buf, sizeof buf, &n) == 0);
    Cap_t back; memset(&back, 0, sizeof back);
    assert(Cap_decode(buf, n, &back) == 0);
    assert(back.data.len == 3);

    /* Forge a well-formed wire value with 6 octets, over the IDL bound (4),
     * writing the raw XCDR2 sequence<octet> form directly. */
    uint8_t* w_buf = NULL; size_t w_len = 0; size_t w_cap = 0;
    uint8_t payload[6] = {9,9,9,9,9,9};
    assert(zerodds_xcdr2_c_write_u32(&w_buf, &w_len, &w_cap, 6) == 0);
    for (int i = 0; i < 6; i++) {
        assert(zerodds_xcdr2_c_write_u8(&w_buf, &w_len, &w_cap, payload[i]) == 0);
    }
    Cap_t forged; memset(&forged, 0, sizeof forged);
    int rc = Cap_decode(w_buf, w_len, &forged);
    free(w_buf);
    assert(rc == -14 && "decode must reject an over-bound sequence with -14, not silently accept it");
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("runtime bound-rejection test failed:\n{e}");
    }
}

/// Moderate fix (deep review of #22 decode-bounds-cross-backend):
/// `zerodds_xcdr2_c_read_string` already `malloc`s the string before
/// `emit_member_read`'s bound check runs — the check used to `return -14`
/// straight away, leaking the just-allocated buffer on every rejected
/// decode. Now it `free`s first. Prove it under AddressSanitizer/
/// LeakSanitizer by rejecting an over-bound string many times in a loop —
/// a leak would show up as growing, then LSAN-reported, allocations.
///
/// LeakSanitizer is Linux-only (unsupported under Apple's ASan runtime), so
/// this is a real leak proof only on Linux/codepit — locally on macOS the
/// binary still runs under ASan (use-after-free / double-free would still
/// be caught by the `(s->name) = NULL` defensive-reset this fix also adds),
/// but a real leak would not be flagged. codepit run: PENDING.
#[test]
#[ignore = "requires gcc/clang with -fsanitize=address,leak (Linux for LSAN); PENDING codepit run"]
fn decode_rejects_over_bound_string_does_not_leak_under_asan() {
    if !gcc_available() {
        return;
    }
    let h = gen_c("@final struct Named { string<8> name; };");
    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
int main(void){
    uint8_t* w_buf = NULL; size_t w_len = 0; size_t w_cap = 0;
    assert(zerodds_xcdr2_c_write_string(&w_buf, &w_len, &w_cap, "way-too-long") == 0);
    /* Reject the same over-bound wire value many times. If the bound-check
     * rejection path leaked the read_string allocation (the pre-fix bug),
     * LeakSanitizer flags it at process exit; ASan's use-after-free/
     * double-free checks also cover the `(s->name) = NULL` reset this fix
     * relies on to make repeated decode-into-the-same-struct safe. */
    for (int i = 0; i < 10000; i++) {
        Named_t forged; memset(&forged, 0, sizeof forged);
        int rc = Named_decode(w_buf, w_len, &forged);
        assert(rc == -14 && "decode must reject an over-bound string with -14");
        assert(forged.name == NULL && "rejected decode must not leave a dangling pointer");
    }
    free(w_buf);
    return 0;
}
"#;
    let flags = ["-fsanitize=address,leak", "-g", "-O0"];
    if let Err(e) = compile_and_run_with_flags(&h, main, &flags) {
        // Fall back to plain ASan (no LSAN) if the toolchain rejects the
        // combined flag (e.g. Apple clang, which has no LeakSanitizer) — the
        // UAF/double-free coverage still runs even where the leak check itself
        // is unsupported.
        let fallback = ["-fsanitize=address", "-g", "-O0"];
        if let Err(e2) = compile_and_run_with_flags(&h, main, &fallback) {
            panic!("ASan run failed with both leak and plain sanitizer flags:\n{e}\n---\n{e2}");
        }
    }
}
