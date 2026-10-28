// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C binding of a map: DDS-XTypes 1.3 §7.5.1.3.1 and §7.5.1.3.2.
//!
//! The header the C column writes for an IDL with maps goes through the
//! C compiler, with a program that calls the operations of Table 47 on it
//! and checks what the clause says each of them returns.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;

const IDL: &str = "\
module m {
  struct Foo { long a; };
  enum Color { RED, GREEN };
  struct Holder {
    map<unsigned long, Foo> by_id;
    map<string, long> counts;
    map<long, string, 3> small;
    map<unsigned long long, Color> colors;
  };
};
";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

fn header(idl: &str) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "maps.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast: Ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("resolve");
    assert!(report.is_clean(), "{:?}", report.broken);
    let body = zerodds_lang_rules::generate::source(
        &ast,
        Language::C,
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
    )
    .expect("the C column renders the maps");
    format!(
        "#ifndef ZD_GEN_H\n#define ZD_GEN_H\n#include \"zerodds_wire.h\"\n\
         #pragma GCC diagnostic ignored \"-Wunused-function\"\n\
         #define ZD_ENDIANNESS ZDW_LITTLE\n{body}\n#endif\n"
    )
}

fn compiler() -> Option<String> {
    if let Ok(chosen) = std::env::var("CC") {
        return Some(chosen);
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
}

/// The program, compiled with the flags of the C corpus gate and run.
fn run(program: &str, tag: &str) -> String {
    let cc = compiler().expect("a C compiler");
    let root = repository_root();
    let runtime = root.join("runtime/c");
    let dir = std::env::temp_dir().join(format!("zd-c-map-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a work directory");
    std::fs::write(dir.join("gen.h"), header(IDL)).expect("write the header");
    std::fs::write(dir.join("main.c"), program).expect("write the program");
    let binary = dir.join("main");
    let mut command = Command::new(&cc);
    command
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Wconversion",
            "-Wsign-conversion",
            "-Werror",
        ])
        .arg(format!("-I{}", dir.display()))
        .arg(format!("-I{}", runtime.join("include").display()))
        .arg(dir.join("main.c"));
    for source in [
        "writer.c",
        "reader.c",
        "md5.c",
        "float128.c",
        "float128_decimal.c",
    ] {
        command.arg(runtime.join("src").join(source));
    }
    command.arg("-lm").arg("-o").arg(&binary);
    let compiled = command.output().expect("run the compiler");
    let diagnostics = String::from_utf8_lossy(&compiled.stderr);
    assert!(
        compiled.status.success(),
        "the compiler refused:\n{}",
        diagnostics.lines().take(60).collect::<Vec<_>>().join("\n")
    );
    let ran = Command::new(&binary).output().expect("run the program");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(ran.status.success(), "the program failed:\n{text}");
    let _ = std::fs::remove_dir_all(&dir);
    text
}

const PRELUDE: &str = "\
#include \"gen.h\"
#include <stdio.h>

#define CHECK(c) do { if (!(c)) { fprintf(stderr, \"line %d: %s\\n\", __LINE__, #c); return 1; } } while (0)
";

/// §7.5.1.3.2.1: the type of a member is the map of the clause, named
/// `[key][element]Map`, and the maps of every type exist.
#[test]
fn the_name_of_a_map_is_the_key_and_the_element() {
    let generated = header(IDL);
    for name in [
        "UInt32m_FooMap",
        "StringInt32Map",
        "Int32StringMap_b3",
        "UInt64m_ColorMap",
    ] {
        assert!(
            generated.contains(&format!("typedef struct {name} {name};")),
            "{name}:\n{generated}"
        );
    }
    // For every type, the three keys of the clause: the ones the model did
    // not use are declared too.
    for name in [
        "UInt32m_FooMap",
        "UInt64m_FooMap",
        "Stringm_FooMap",
        "UInt32m_ColorMap",
        "Stringm_ColorMap",
        "UInt32m_HolderMap",
        "UInt64m_HolderMap",
        "Stringm_HolderMap",
    ] {
        assert!(
            generated.contains(&format!("ZD_MAP_DEFINE({name},")),
            "{name}:\n{generated}"
        );
    }
    // A member has the type of its map.
    let program = format!(
        "{PRELUDE}
int main(void) {{
    m_Holder holder;
    UInt32m_FooMap *by_id = &holder.by_id;
    StringInt32Map *counts = &holder.counts;
    Int32StringMap_b3 *small = &holder.small;
    UInt64m_ColorMap *colors = &holder.colors;
    Stringm_FooMap other;
    UInt64m_FooMap third;
    (void)by_id; (void)counts; (void)small; (void)colors; (void)other; (void)third;
    return 0;
}}
"
    );
    run(&program, "names");
}

/// Table 47 on a map with a structure as the element and `uint32_t` keys:
/// every operation and the code each returns.
#[test]
fn the_operations_of_table_47_answer_as_the_clause_says() {
    let program = format!(
        "{PRELUDE}
int main(void) {{
    m_Foo foo1; m_Foo foo2; m_Foo *found; uint32_t key; MapEntry entry;
    UInt32m_FooMap *map; UInt32m_FooMap *other;
    foo1.a = 1; foo2.a = 2;

    /* new, initialize: the initial size and capacity are 0 */
    map = UInt32m_FooMap_new();
    CHECK(map != NULL);
    CHECK(UInt32m_FooMap_get_size(map) == 0u && UInt32m_FooMap_get_max_size(map) == 0u);
    /* 'past the current maximum size': PRECONDITION_NOT_MET */
    CHECK(UInt32m_FooMap_insert(map, 7u, &foo1) == DDS_RETCODE_PRECONDITION_NOT_MET);

    /* set_max_size */
    CHECK(UInt32m_FooMap_set_max_size(map, 2u) == DDS_RETCODE_OK);
    CHECK(UInt32m_FooMap_get_max_size(map) == 2u);

    /* insert: size + 1; an existing key and a NULL element are BAD_PARAMETER */
    CHECK(UInt32m_FooMap_insert(map, 7u, &foo1) == DDS_RETCODE_OK);
    CHECK(UInt32m_FooMap_get_size(map) == 1u);
    CHECK(UInt32m_FooMap_insert(map, 7u, &foo2) == DDS_RETCODE_BAD_PARAMETER);
    CHECK(UInt32m_FooMap_insert(map, 8u, NULL) == DDS_RETCODE_BAD_PARAMETER);
    CHECK(UInt32m_FooMap_insert(NULL, 8u, &foo2) == DDS_RETCODE_BAD_PARAMETER);
    CHECK(UInt32m_FooMap_insert(map, 8u, &foo2) == DDS_RETCODE_OK);
    CHECK(UInt32m_FooMap_insert(map, 9u, &foo2) == DDS_RETCODE_PRECONDITION_NOT_MET);
    CHECK(UInt32m_FooMap_get_size(map) == 2u);

    /* the maximum cannot go below the size */
    CHECK(UInt32m_FooMap_set_max_size(map, 1u) == DDS_RETCODE_BAD_PARAMETER);
    CHECK(UInt32m_FooMap_get_max_size(map) == 2u);

    /* insert_or_assign: replaces, or inserts as insert does */
    CHECK(UInt32m_FooMap_insert_or_assign(map, 7u, &foo2) == DDS_RETCODE_OK);
    found = UInt32m_FooMap_find_element(map, 7u);
    CHECK(found != NULL && found->a == 2);
    CHECK(UInt32m_FooMap_get_size(map) == 2u);
    CHECK(UInt32m_FooMap_insert_or_assign(map, 9u, &foo1) == DDS_RETCODE_PRECONDITION_NOT_MET);

    /* find_element: NULL for an absent key */
    CHECK(UInt32m_FooMap_find_element(map, 99u) == NULL);

    /* get_first, get_next, get_pair, find_entry: insertion order */
    entry = UInt32m_FooMap_get_first(map);
    CHECK(entry.valid);
    CHECK(UInt32m_FooMap_get_pair(map, &entry, &key, &found) && key == 7u && found->a == 2);
    CHECK(UInt32m_FooMap_get_next(map, &entry));
    CHECK(UInt32m_FooMap_get_pair(map, &entry, &key, &found) && key == 8u);
    CHECK(!UInt32m_FooMap_get_next(map, &entry));
    CHECK(!entry.valid);
    CHECK(!UInt32m_FooMap_get_pair(map, &entry, &key, &found));
    CHECK(UInt32m_FooMap_find_entry(map, 8u).valid);
    CHECK(!UInt32m_FooMap_find_entry(map, 1u).valid);
    entry = UInt32m_FooMap_find_entry(map, 8u);
    CHECK(UInt32m_FooMap_get_pair(map, &entry, &key, &found) && key == 8u);
    CHECK(!UInt32m_FooMap_get_pair(map, NULL, &key, &found));
    CHECK(!UInt32m_FooMap_get_pair(map, &entry, NULL, &found));
    CHECK(!UInt32m_FooMap_get_pair(map, &entry, &key, NULL));

    /* erase: size - 1, the order of the others is kept */
    CHECK(UInt32m_FooMap_erase(map, 7u) == DDS_RETCODE_OK);
    CHECK(UInt32m_FooMap_get_size(map) == 1u);
    CHECK(UInt32m_FooMap_erase(map, 7u) == DDS_RETCODE_BAD_PARAMETER);
    entry = UInt32m_FooMap_get_first(map);
    CHECK(UInt32m_FooMap_get_pair(map, &entry, &key, &found) && key == 8u);

    /* copy: NULL is BAD_PARAMETER; too small without autogrow leaves the
     * destination alone; with it the maximum grows */
    other = UInt32m_FooMap_new();
    CHECK(other != NULL);
    CHECK(UInt32m_FooMap_copy(other, NULL, true) == DDS_RETCODE_BAD_PARAMETER);
    CHECK(UInt32m_FooMap_copy(other, map, false) == DDS_RETCODE_PRECONDITION_NOT_MET);
    CHECK(UInt32m_FooMap_get_size(other) == 0u && UInt32m_FooMap_get_max_size(other) == 0u);
    CHECK(UInt32m_FooMap_copy(other, map, true) == DDS_RETCODE_OK);
    CHECK(UInt32m_FooMap_get_size(other) == 1u && UInt32m_FooMap_get_max_size(other) >= 1u);
    found = UInt32m_FooMap_find_element(other, 8u);
    CHECK(found != NULL && found->a == 2);
    /* the source is not changed, and the copy is not the same storage */
    CHECK(UInt32m_FooMap_get_size(map) == 1u);
    CHECK(UInt32m_FooMap_find_element(map, 8u) != found);
    /* entries of the destination that are not in the source are erased */
    CHECK(UInt32m_FooMap_set_max_size(other, 3u) == DDS_RETCODE_OK);
    CHECK(UInt32m_FooMap_insert(other, 40u, &foo1) == DDS_RETCODE_OK);
    CHECK(UInt32m_FooMap_get_size(other) == 2u);
    CHECK(UInt32m_FooMap_copy(other, map, false) == DDS_RETCODE_OK);
    CHECK(UInt32m_FooMap_get_size(other) == 1u);
    CHECK(UInt32m_FooMap_find_element(other, 40u) == NULL);
    CHECK(UInt32m_FooMap_find_element(other, 8u) != NULL);

    /* clear: the size is 0 and the maximum does not change */
    UInt32m_FooMap_clear(map);
    CHECK(UInt32m_FooMap_get_size(map) == 0u && UInt32m_FooMap_get_max_size(map) == 2u);
    CHECK(!UInt32m_FooMap_get_first(map).valid);

    /* finalize: clear, then set_max_size(0) */
    UInt32m_FooMap_finalize(other);
    CHECK(UInt32m_FooMap_get_size(other) == 0u && UInt32m_FooMap_get_max_size(other) == 0u);

    /* delete, and delete of NULL */
    UInt32m_FooMap_delete(map);
    UInt32m_FooMap_delete(other);
    UInt32m_FooMap_delete(NULL);
    return 0;
}}
"
    );
    run(&program, "table47");
}

/// A key that is a string is compared by its characters; an element that
/// is a primitive is passed as the type; a bounded map takes no capacity
/// past its bound.
#[test]
fn string_keys_primitive_elements_and_bounds() {
    let program = format!(
        "{PRELUDE}
int main(void) {{
    StringInt32Map counts;
    Int32StringMap_b3 small;
    UInt64m_ColorMap colors;
    char one[4] = {{'o', 'n', 'e', 0}};
    int32_t *found;
    const char **name;
    m_Color *color;

    StringInt32Map_initialize(&counts);
    CHECK(StringInt32Map_set_max_size(&counts, 2u) == DDS_RETCODE_OK);
    CHECK(StringInt32Map_insert(&counts, \"one\", 1) == DDS_RETCODE_OK);
    /* the same characters in another array are the same key */
    CHECK(StringInt32Map_insert(&counts, one, 5) == DDS_RETCODE_BAD_PARAMETER);
    found = StringInt32Map_find_element(&counts, one);
    CHECK(found != NULL && *found == 1);
    CHECK(StringInt32Map_insert(&counts, NULL, 5) == DDS_RETCODE_BAD_PARAMETER);
    CHECK(StringInt32Map_find_element(&counts, NULL) == NULL);
    CHECK(StringInt32Map_erase(&counts, one) == DDS_RETCODE_OK);
    CHECK(StringInt32Map_get_size(&counts) == 0u);
    StringInt32Map_finalize(&counts);

    /* map<long, string, 3>: the bound is the largest capacity */
    Int32StringMap_b3_initialize(&small);
    CHECK(Int32StringMap_b3_set_max_size(&small, 4u) == DDS_RETCODE_BAD_PARAMETER);
    CHECK(Int32StringMap_b3_set_max_size(&small, 3u) == DDS_RETCODE_OK);
    CHECK(Int32StringMap_b3_insert(&small, -1, \"a\") == DDS_RETCODE_OK);
    CHECK(Int32StringMap_b3_insert(&small, -2, \"b\") == DDS_RETCODE_OK);
    CHECK(Int32StringMap_b3_insert(&small, -3, \"c\") == DDS_RETCODE_OK);
    CHECK(Int32StringMap_b3_insert(&small, -4, \"d\") == DDS_RETCODE_PRECONDITION_NOT_MET);
    name = Int32StringMap_b3_find_element(&small, -2);
    CHECK(name != NULL && (*name)[0] == 'b');
    Int32StringMap_b3_finalize(&small);

    /* an enumeration is a scalar element, a 64-bit key a scalar key */
    UInt64m_ColorMap_initialize(&colors);
    CHECK(UInt64m_ColorMap_set_max_size(&colors, 1u) == DDS_RETCODE_OK);
    CHECK(UInt64m_ColorMap_insert(&colors, 5ull, m_GREEN) == DDS_RETCODE_OK);
    color = UInt64m_ColorMap_find_element(&colors, 5ull);
    CHECK(color != NULL && *color == m_GREEN);
    UInt64m_ColorMap_finalize(&colors);
    return 0;
}}
"
    );
    run(&program, "keys");
}

/// A buffer the map does not own (a generated reader's arena, or an array
/// of the application) is never freed: growing the capacity moves the
/// entries to a block of the map's own.
#[test]
fn a_buffer_that_is_not_the_maps_is_left_alone() {
    let program = format!(
        "{PRELUDE}
static StringInt32MapElement arena[1];

int main(void) {{
    StringInt32Map map;
    int32_t *found;
    arena[0].key = \"k\";
    arena[0].value = 41;
    map._maximum = 1u;
    map._length = 1u;
    map._buffer = arena;
    map._release = false;

    CHECK(StringInt32Map_set_max_size(&map, 3u) == DDS_RETCODE_OK);
    CHECK(map._buffer != arena && map._release);
    CHECK(StringInt32Map_insert(&map, \"j\", 42) == DDS_RETCODE_OK);
    found = StringInt32Map_find_element(&map, \"k\");
    CHECK(found != NULL && *found == 41);
    CHECK(arena[0].value == 41);
    StringInt32Map_finalize(&map);
    CHECK(map._buffer == NULL && !map._release && map._maximum == 0u);
    CHECK(arena[0].value == 41);
    return 0;
}}
"
    );
    run(&program, "arena");
}
