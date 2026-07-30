//! Regression: the C backend's XTypes TypeObject block must sit **inside** the
//! header's include guard.
//!
//! The C emitter (`crates/idl-cpp/src/c_mode.rs`) wraps the header body in an
//! `#ifndef …_H` / closing `#endif` include guard. The idlc post-pass appends
//! the TypeObject constant block (`static const unsigned char …_type_object[]`)
//! to that header. If the block is appended *after* the guard's closing
//! `#endif` it lives outside the guard, so a second `#include` of the same
//! header re-emits the `static const` arrays → clang/gcc redefinition error.
//! `splice_c_typeobject_block` (in `main.rs`) inserts the block before the
//! closing `#endif` instead; this test drives the real CLI + a real C compiler
//! to prove a double `#include` compiles.
//!
//! `#[ignore]`d by default (needs a C toolchain); run with:
//!
//! ```text
//! cargo test -p zerodds-idlc --test c_typeobject_guard -- --include-ignored
//! ```

#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout, missing_docs)]

use std::path::Path;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_zerodds-idlc");

/// First available C compiler (`clang`, else `gcc`/`cc`), or `None`.
fn c_compiler() -> Option<&'static str> {
    for cc in ["clang", "gcc", "cc"] {
        let ok = Command::new(cc)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return Some(cc);
        }
    }
    None
}

fn c_include_dir() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/zerodds-c-api/include"
    )
    .to_string()
}

/// `<cc> -std=c99 -fsyntax-only -I<runtime> <file>` → Ok on success.
fn syntax_only(cc: &str, file: &Path) -> Result<(), String> {
    let out = Command::new(cc)
        .args(["-std=c99", "-fsyntax-only", "-I"])
        .arg(c_include_dir())
        .arg(file)
        .output()
        .map_err(|e| format!("spawn {cc}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

#[test]
#[ignore = "requires a C toolchain (clang/gcc/cc) in PATH"]
fn c_header_with_typeobject_included_twice_compiles() {
    let Some(cc) = c_compiler() else {
        println!("no C compiler in PATH, skipping");
        return;
    };

    let dir = tempfile::tempdir().expect("tempdir");
    // A @key type is a natural trigger for a TypeObject block in the post-pass.
    let idl = dir.path().join("keyed.idl");
    std::fs::write(&idl, "@final struct Keyed { @key long id; long v; };").expect("write idl");
    let out_dir = dir.path().join("out");

    let status = Command::new(BIN)
        .arg("generate")
        .arg(&idl)
        .arg("--c")
        .arg("-o")
        .arg(&out_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn idlc");
    assert!(status.success(), "idlc --c generation failed");

    let header = out_dir.join("keyed.h");
    let generated = std::fs::read_to_string(&header).expect("read generated header");
    // Guard against a silently-empty repro: the TypeObject block must exist,
    // and it must precede the closing include-guard `#endif`.
    let block_pos = generated.find("_type_object[]").expect(
        "TypeObject block must be emitted for a @key type (repro is meaningless without it)",
    );
    let last_endif = generated
        .rfind("#endif")
        .expect("C header must have an include-guard #endif");
    assert!(
        block_pos < last_endif,
        "TypeObject block is outside the include guard:\n{generated}"
    );

    // The double `#include` — the actual redefinition repro. Place main.c
    // beside keyed.h so the quoted include resolves.
    let main_c = out_dir.join("main.c");
    std::fs::write(
        &main_c,
        "#include \"keyed.h\"\n#include \"keyed.h\"\nint main(void) { return 0; }\n",
    )
    .expect("write main.c");

    if let Err(e) = syntax_only(cc, &main_c) {
        panic!(
            "double #include of C header with TypeObject block did not compile ({cc}):\n{e}\n--- header ---\n{generated}"
        );
    }
}
