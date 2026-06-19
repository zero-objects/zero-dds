//! Integration-Test: Preprocessor-Hardening (C4.6 §1.13).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_idl::preprocessor::{MemoryResolver, PreprocessError, Preprocessor};

fn run(src: &str) -> String {
    Preprocessor::new(MemoryResolver::new())
        .process("main.idl", src)
        .expect("ok")
        .expanded
}

#[test]
fn multi_line_define_with_backslash_continuation() {
    // `\` directly before `\n` splices the line.
    let src = "#define MAX 1000 + \\\n 234\nconst long L = MAX;\n";
    let out = run(src);
    // Whitespace collapses with one space because Token-Splice removed the
    // backslash + newline; remaining leading space on continuation line stays.
    assert!(out.contains("1000 +  234"), "got {out:?}");
}

#[test]
fn multi_line_with_carriage_return_newline() {
    // CR LF + backslash continuation must work too.
    let src = "#define X 100 \\\r\n + 1\nconst long L = X;\n";
    let out = run(src);
    assert!(out.contains("100  + 1"), "got {out:?}");
}

#[test]
fn include_cycle_detection() {
    let mut r = MemoryResolver::new();
    r.add("a.idl", "#include \"main.idl\"\n");
    let res = Preprocessor::new(r).process("main.idl", "#include \"a.idl\"\n");
    assert!(matches!(res, Err(PreprocessError::IncludeCycle { .. })));
}

#[test]
fn error_directive_emits_diagnostic() {
    let res =
        Preprocessor::new(MemoryResolver::new()).process("main.idl", "#error This is wrong\n");
    match res {
        Err(PreprocessError::ErrorDirective { message, .. }) => {
            assert_eq!(message, "This is wrong");
        }
        other => panic!("expected ErrorDirective, got {other:?}"),
    }
}

#[test]
fn pragma_keylist_collected_into_ast() {
    let result = Preprocessor::new(MemoryResolver::new())
        .process(
            "main.idl",
            "#pragma keylist Sensor sensor_id room_id\nstruct Sensor { long sensor_id; };\n",
        )
        .expect("ok");
    assert_eq!(result.pragma_keylists.len(), 1);
    let kl = &result.pragma_keylists[0];
    assert_eq!(kl.type_name, "Sensor");
    assert_eq!(
        kl.keys,
        vec!["sensor_id".to_string(), "room_id".to_string()]
    );
}

#[test]
fn pragma_other_than_keylist_is_silently_dropped() {
    let result = Preprocessor::new(MemoryResolver::new())
        .process("main.idl", "#pragma once\nstruct X {};\n")
        .expect("ok");
    assert!(result.pragma_keylists.is_empty());
    assert!(!result.expanded.contains("#pragma"));
}

#[test]
fn pragma_keylist_without_fields_is_still_valid() {
    let result = Preprocessor::new(MemoryResolver::new())
        .process("main.idl", "#pragma keylist OnlyKey\n")
        .expect("ok");
    assert_eq!(result.pragma_keylists.len(), 1);
    assert_eq!(result.pragma_keylists[0].type_name, "OnlyKey");
    assert!(result.pragma_keylists[0].keys.is_empty());
}

#[test]
fn deep_include_nesting_within_max_depth() {
    let mut r = MemoryResolver::new();
    r.add("level1.idl", "#include \"level2.idl\"\n");
    r.add("level2.idl", "struct Final { long x; };\n");
    let out = Preprocessor::new(r)
        .process("main.idl", "#include \"level1.idl\"\n")
        .expect("ok");
    assert!(out.expanded.contains("struct Final"));
}
