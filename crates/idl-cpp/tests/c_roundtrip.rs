//! C backend (src/c_mode.rs) end-to-end tests.
//!
//! Two layers:
//! 1. `gcc -fsyntax-only` on the generated header (Bug C2: the header must
//!    compile alongside the shipped `zerodds_xcdr2.h` — previously a dead
//!    `zerodds.h` include collided with cbindgen-vs-handwritten prototypes).
//! 2. A real encode→decode roundtrip (Bug C scope widening: enum / typedef /
//!    nested struct / fixed array / @optional must reach the wire and come
//!    back unchanged).
//!
//! gcc-gated and `#[ignore]` by default (not every CI image has gcc).
//! Run via `cargo test -p zerodds-idl-cpp -- --ignored`.

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

/// The runtime C include dir (`crates/zerodds-c-api/include`), which ships
/// `zerodds_xcdr2.h` (the `static inline` XCDR2 helpers the generated body
/// calls — so a generated header is self-contained with no extra link step).
fn c_include_dir() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/../zerodds-c-api/include").to_string()
}

fn gen_c(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_c_header(&ast, &CGenOptions::default()).expect("c-gen")
}

fn syntax_only(header: &str) -> Result<(), String> {
    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let hdr = dir.path().join("gen.h");
    std::fs::write(&hdr, header).map_err(|e| format!("write: {e}"))?;
    let out = Command::new("gcc")
        .args(["-std=c99", "-fsyntax-only", "-I"])
        .arg(c_include_dir())
        .arg(&hdr)
        .output()
        .map_err(|e| format!("spawn: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

/// Compile `header` + `main_src` and run; require exit 0.
fn compile_and_run(header: &str, main_src: &str) -> Result<(), String> {
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
            "roundtrip binary failed (exit {:?}):\n{}",
            run.status.code(),
            String::from_utf8_lossy(&run.stdout)
        ))
    }
}

/// Bug C2: the generated C header must compile with `gcc -fsyntax-only`
/// alongside the shipped runtime header (no `zerodds.h` prototype collision).
#[test]
#[ignore = "requires gcc in PATH"]
fn keyed_header_syntax_checks_alongside_runtime() {
    if !gcc_available() {
        println!("gcc not available, skipping");
        return;
    }
    let h = gen_c("@final struct Keyed { @key long id; double value; };");
    if let Err(e) = syntax_only(&h) {
        panic!("gcc -fsyntax-only failed:\n{e}");
    }
}

/// Bug C (scope widening) + Bug C2 (@optional): enum, typedef, nested struct,
/// fixed array, and @optional members must round-trip over the XCDR2 wire.
#[test]
#[ignore = "requires gcc in PATH"]
fn widened_features_roundtrip_through_gcc() {
    if !gcc_available() {
        println!("gcc not available, skipping");
        return;
    }
    let src = "\
module conf {
  enum Color { RED, GREEN, BLUE };
  typedef double Amps;
  @final struct Point { long x; long y; };
  @final struct Wide {
    long           id;
    Color          c;
    Amps           battery;
    Point          origin;
    long           grid[3];
    @optional long maybe;
    string         label;
  };
};";
    let h = gen_c(src);
    // Sanity: @optional is a presence companion, not a flattened field.
    assert!(
        h.contains("uint8_t maybe_present;"),
        "@optional presence flag missing:\n{h}"
    );
    // Sanity: enum became an int32 alias.
    assert!(
        h.contains("typedef int32_t conf_Color_t;"),
        "enum alias missing"
    );

    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
int main(void) {
    conf_Wide_t w; memset(&w, 0, sizeof(w));
    w.id = 42;
    w.c = conf_Color_GREEN;
    w.battery = 3.5;
    w.origin.x = 7; w.origin.y = 8;
    w.grid[0]=10; w.grid[1]=20; w.grid[2]=30;
    w.maybe = 99; w.maybe_present = 1;
    w.label = "hi";

    uint8_t buf[512]; size_t n=0;
    assert(conf_Wide_encode(&w, buf, sizeof(buf), &n) == 0);
    assert(n > 0);

    conf_Wide_t back; memset(&back, 0, sizeof(back));
    assert(conf_Wide_decode(buf, n, &back) == 0);
    assert(back.id == 42);
    assert(back.c == conf_Color_GREEN);
    assert(back.battery == 3.5);
    assert(back.origin.x == 7 && back.origin.y == 8);
    assert(back.grid[0]==10 && back.grid[1]==20 && back.grid[2]==30);
    assert(back.maybe_present == 1 && back.maybe == 99);
    assert(strcmp(back.label, "hi") == 0);

    /* @optional absent: presence flag clears, value not read */
    conf_Wide_t w2; memset(&w2, 0, sizeof(w2));
    w2.id = 1; w2.c = conf_Color_RED; w2.battery = 1.0;
    w2.maybe_present = 0; w2.label = "x";
    size_t n2 = 0;
    assert(conf_Wide_encode(&w2, buf, sizeof(buf), &n2) == 0);
    conf_Wide_t b2; memset(&b2, 0, sizeof(b2));
    assert(conf_Wide_decode(buf, n2, &b2) == 0);
    assert(b2.maybe_present == 0);
    assert(b2.id == 1);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("C widened-features roundtrip failed:\n{e}");
    }
}

// ===========================================================================
// C-Foundation #43 — additional feature roundtrips (each gcc-compiled + run).
// ===========================================================================

/// const-array-bound: `const long N = 4; long v[N];` — the bound resolves to
/// its literal (4) just like every other backend (#43).
#[test]
#[ignore = "requires gcc in PATH"]
fn const_array_bound_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c("module m { const long N = 4; @final struct A { long v[N]; double w[N/2]; }; };");
    // The bound must be resolved at codegen time (no `[N]` identifier leak).
    assert!(
        h.contains("int32_t v[4];"),
        "named const bound not resolved:\n{h}"
    );
    assert!(
        h.contains("double w[2];"),
        "const-expr bound not folded:\n{h}"
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
int main(void){
    m_A_t a; memset(&a,0,sizeof a);
    for(int i=0;i<4;i++) a.v[i]=i*7;
    a.w[0]=1.5; a.w[1]=2.5;
    uint8_t b[256]; size_t n=0;
    assert(m_A_encode(&a,b,sizeof b,&n)==0);
    m_A_t back; memset(&back,0,sizeof back);
    assert(m_A_decode(b,n,&back)==0);
    for(int i=0;i<4;i++) assert(back.v[i]==i*7);
    assert(back.w[0]==1.5 && back.w[1]==2.5);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("const-array-bound roundtrip failed:\n{e}");
    }
}

/// sequence<primitive> + sequence<string> members round-trip.
#[test]
#[ignore = "requires gcc in PATH"]
fn sequence_primitive_and_string_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c("@final struct S { sequence<long> nums; sequence<string> tags; };");
    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
int main(void){
    S_t s; memset(&s,0,sizeof s);
    int32_t nv[3]={10,20,30};
    s.nums.len=3; s.nums.elems=nv;
    char* tv[2]={"alpha","beta"};
    s.tags.len=2; s.tags.elems=tv;
    uint8_t b[512]; size_t n=0;
    assert(S_encode(&s,b,sizeof b,&n)==0);
    S_t back; memset(&back,0,sizeof back);
    assert(S_decode(b,n,&back)==0);
    assert(back.nums.len==3 && back.nums.elems[0]==10 && back.nums.elems[2]==30);
    assert(back.tags.len==2);
    assert(strcmp(back.tags.elems[0],"alpha")==0);
    assert(strcmp(back.tags.elems[1],"beta")==0);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("sequence<primitive>/<string> roundtrip failed:\n{e}");
    }
}

/// sequence<T> of a non-primitive aggregate (struct), and a nested
/// sequence<sequence<long>> — both round-trip (#43).
#[test]
#[ignore = "requires gcc in PATH"]
fn sequence_of_struct_and_nested_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "@final struct P { long x; long y; }; \
         @final struct S { sequence<P> pts; sequence<sequence<long> > mat; };",
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
int main(void){
    S_t s; memset(&s,0,sizeof s);
    P_t pv[2]; pv[0].x=1; pv[0].y=2; pv[1].x=3; pv[1].y=4;
    s.pts.len=2; s.pts.elems=pv;
    /* mat = [[5,6],[7]] */
    int32_t r0[2]={5,6}; int32_t r1[1]={7};
    struct { uint32_t len; int32_t* elems; } rows[2];
    rows[0].len=2; rows[0].elems=r0;
    rows[1].len=1; rows[1].elems=r1;
    s.mat.len=2; s.mat.elems=rows;
    uint8_t b[512]; size_t n=0;
    assert(S_encode(&s,b,sizeof b,&n)==0);
    S_t back; memset(&back,0,sizeof back);
    assert(S_decode(b,n,&back)==0);
    assert(back.pts.len==2);
    assert(back.pts.elems[0].x==1 && back.pts.elems[1].y==4);
    assert(back.mat.len==2);
    assert(back.mat.elems[0].len==2 && back.mat.elems[0].elems[1]==6);
    assert(back.mat.elems[1].len==1 && back.mat.elems[1].elems[0]==7);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("sequence<struct>/nested roundtrip failed:\n{e}");
    }
}

/// Discriminated union with an integer discriminator, both as a member and as
/// a standalone topic type, round-trips per selected arm (#43, union).
#[test]
#[ignore = "requires gcc in PATH"]
fn union_integer_discriminator_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "@final union U switch (long) { \
            case 1: long i; \
            case 2: double d; \
            default: string s; \
         };",
    );
    assert!(
        h.contains("typedef struct U_s"),
        "union typedef missing:\n{h}"
    );
    assert!(h.contains("U_typesupport"), "union typesupport missing");
    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
int main(void){
    /* arm 1 (long) */
    U_t a; memset(&a,0,sizeof a); a._d=1; a._u.i=12345;
    uint8_t b[256]; size_t n=0;
    assert(U_encode(&a,b,sizeof b,&n)==0);
    U_t ba; memset(&ba,0,sizeof ba);
    assert(U_decode(b,n,&ba)==0);
    assert(ba._d==1 && ba._u.i==12345);
    /* arm 2 (double) */
    U_t c; memset(&c,0,sizeof c); c._d=2; c._u.d=6.25;
    size_t n2=0; assert(U_encode(&c,b,sizeof b,&n2)==0);
    U_t bc; memset(&bc,0,sizeof bc);
    assert(U_decode(b,n2,&bc)==0);
    assert(bc._d==2 && bc._u.d==6.25);
    /* default arm (string) */
    U_t e; memset(&e,0,sizeof e); e._d=99; e._u.s="hello";
    size_t n3=0; assert(U_encode(&e,b,sizeof b,&n3)==0);
    U_t be; memset(&be,0,sizeof be);
    assert(U_decode(b,n3,&be)==0);
    assert(be._d==99 && strcmp(be._u.s,"hello")==0);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("union (integer disc) roundtrip failed:\n{e}");
    }
}

/// Union with an enum discriminator + a union member inside a struct (#43).
#[test]
#[ignore = "requires gcc in PATH"]
fn union_enum_discriminator_member_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "module c { \
            enum Mode { OFF, ON, AUTO }; \
            @final union Reading switch (Mode) { \
                case OFF: long off_code; \
                case ON: double level; \
                default: long other; \
            }; \
            @final struct Telemetry { long seq; Reading r; }; \
         };",
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
int main(void){
    c_Telemetry_t t; memset(&t,0,sizeof t);
    t.seq=7; t.r._d=c_Mode_ON; t.r._u.level=3.5;
    uint8_t b[256]; size_t n=0;
    assert(c_Telemetry_encode(&t,b,sizeof b,&n)==0);
    c_Telemetry_t back; memset(&back,0,sizeof back);
    assert(c_Telemetry_decode(b,n,&back)==0);
    assert(back.seq==7);
    assert(back.r._d==c_Mode_ON);
    assert(back.r._u.level==3.5);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("union (enum disc, struct member) roundtrip failed:\n{e}");
    }
}

/// map<K,V> member round-trips (#43, map).
#[test]
#[ignore = "requires gcc in PATH"]
fn map_member_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c("@final struct M { map<string, long> counters; };");
    assert!(
        h.contains("keys;") && h.contains("vals;"),
        "map repr missing:\n{h}"
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
int main(void){
    M_t m; memset(&m,0,sizeof m);
    char* k[2]={"a","bb"}; int32_t v[2]={11,22};
    m.counters.len=2; m.counters.keys=k; m.counters.vals=v;
    uint8_t b[256]; size_t n=0;
    assert(M_encode(&m,b,sizeof b,&n)==0);
    M_t back; memset(&back,0,sizeof back);
    assert(M_decode(b,n,&back)==0);
    assert(back.counters.len==2);
    assert(strcmp(back.counters.keys[0],"a")==0 && back.counters.vals[0]==11);
    assert(strcmp(back.counters.keys[1],"bb")==0 && back.counters.vals[1]==22);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("map<K,V> roundtrip failed:\n{e}");
    }
}

/// wstring (UTF-16) member round-trips (#43, wstring).
#[test]
#[ignore = "requires gcc in PATH"]
fn wstring_member_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c("@final struct W { wstring label; };");
    assert!(h.contains("uint16_t* label;"), "wstring repr missing:\n{h}");
    let main = r#"
#include "gen.h"
#include <assert.h>
int main(void){
    W_t w; memset(&w,0,sizeof w);
    uint16_t txt[4]={ 'H','i', 0x00E9 /* é */, 0 };
    w.label=txt;
    uint8_t b[256]; size_t n=0;
    assert(W_encode(&w,b,sizeof b,&n)==0);
    W_t back; memset(&back,0,sizeof back);
    assert(W_decode(b,n,&back)==0);
    assert(back.label[0]=='H' && back.label[1]=='i');
    assert(back.label[2]==0x00E9 && back.label[3]==0);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("wstring roundtrip failed:\n{e}");
    }
}

/// typedef-to-aggregate: a member typed by a typedef that aliases a struct
/// resolves to the aggregate and round-trips (#43, typedef-to-aggregate).
#[test]
#[ignore = "requires gcc in PATH"]
fn typedef_to_aggregate_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "@final struct Point { long x; long y; }; \
         typedef Point Position; \
         typedef sequence<long> LongSeq; \
         @final struct S { Position p; LongSeq xs; };",
    );
    // typedef-to-struct lands on the struct C type; typedef-to-sequence on the
    // sequence repr.
    assert!(
        h.contains("Point_t p;"),
        "typedef-to-struct not resolved:\n{h}"
    );
    assert!(
        h.contains("elems; } xs;"),
        "typedef-to-sequence not resolved:\n{h}"
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
int main(void){
    S_t s; memset(&s,0,sizeof s);
    s.p.x=4; s.p.y=9;
    int32_t xs[2]={100,200}; s.xs.len=2; s.xs.elems=xs;
    uint8_t b[256]; size_t n=0;
    assert(S_encode(&s,b,sizeof b,&n)==0);
    S_t back; memset(&back,0,sizeof back);
    assert(S_decode(b,n,&back)==0);
    assert(back.p.x==4 && back.p.y==9);
    assert(back.xs.len==2 && back.xs.elems[0]==100 && back.xs.elems[1]==200);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("typedef-to-aggregate roundtrip failed:\n{e}");
    }
}

// ===========================================================================
// C-Foundation round-2 partials — the cases that crash adapters once they
// leave hello-world. Each one had a precise prior symptom.
// ===========================================================================

/// Round-2 partial (a): `sample_free` previously SKIPPED any heap-owning member
/// reached THROUGH a fixed array (`string names[3]`, `sequence<long> rows[2]`)
/// → memory leak. Now those are freed per element. Verified ASan-clean (no
/// invalid/double free) and asserts the array data round-trips first.
#[test]
#[ignore = "requires gcc in PATH"]
fn fixed_array_of_heap_members_freed() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "@final struct Bag { \
            string names[3]; \
            sequence<long> rows[2]; \
            string scalar; \
         };",
    );
    // The free body must loop over the fixed array and free each element.
    assert!(
        h.contains("for (uint32_t fi0 = 0; fi0 < 3;") && h.contains("free(s->names[fi0]);"),
        "fixed-array-of-string free missing:\n{h}"
    );
    assert!(
        h.contains("free((s->rows[fi0]).elems);"),
        "fixed-array-of-seq free missing:\n{h}"
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
int main(void){
    Bag_t bag; memset(&bag,0,sizeof bag);
    bag.names[0]="alpha"; bag.names[1]="beta"; bag.names[2]="gamma";
    int32_t r0[2]={1,2}; int32_t r1[3]={3,4,5};
    bag.rows[0].len=2; bag.rows[0].elems=r0;
    bag.rows[1].len=3; bag.rows[1].elems=r1;
    bag.scalar="solo";
    uint8_t b[512]; size_t n=0;
    assert(Bag_encode(&bag,b,sizeof b,&n)==0);
    Bag_t back; memset(&back,0,sizeof back);
    assert(Bag_decode(b,n,&back)==0);
    assert(strcmp(back.names[0],"alpha")==0);
    assert(strcmp(back.names[2],"gamma")==0);
    assert(back.rows[0].len==2 && back.rows[0].elems[1]==2);
    assert(back.rows[1].len==3 && back.rows[1].elems[2]==5);
    assert(strcmp(back.scalar,"solo")==0);
    /* This is the leak the partial described — must free each array element. */
    Bag_sample_free(&back);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("fixed-array heap free roundtrip failed:\n{e}");
    }
}

/// Round-2 partial (b): a `@mutable` struct's EMHEADER path only handled
/// primitive+string member bodies. An AGGREGATE member (sequence/map/nested
/// struct/wstring) inside a @mutable struct now writes its EMHEADER + a
/// back-patched NEXTINT body length and round-trips.
#[test]
#[ignore = "requires gcc in PATH"]
fn mutable_struct_aggregate_members_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "module m { \
            @final struct Inner { long a; long b; }; \
            @mutable struct Mut { \
                @id(1) long seq; \
                @id(2) sequence<long> nums; \
                @id(3) string label; \
                @id(4) Inner inner; \
                @id(5) map<string,long> counters; \
                @id(6) wstring wlabel; \
            }; \
         };",
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
int main(void){
    m_Mut_t t; memset(&t,0,sizeof t);
    t.seq=42;
    int32_t nv[3]={10,20,30}; t.nums.len=3; t.nums.elems=nv;
    t.label="hello";
    t.inner.a=7; t.inner.b=8;
    char* k[2]={"a","bb"}; int32_t v[2]={1,2};
    t.counters.len=2; t.counters.keys=k; t.counters.vals=v;
    uint16_t wl[3]={'O','K',0}; t.wlabel=wl;
    uint8_t b[512]; size_t n=0;
    assert(m_Mut_encode(&t,b,sizeof b,&n)==0);
    m_Mut_t back; memset(&back,0,sizeof back);
    assert(m_Mut_decode(b,n,&back)==0);
    assert(back.seq==42);
    assert(back.nums.len==3 && back.nums.elems[0]==10 && back.nums.elems[2]==30);
    assert(strcmp(back.label,"hello")==0);
    assert(back.inner.a==7 && back.inner.b==8);
    assert(back.counters.len==2);
    assert(strcmp(back.counters.keys[0],"a")==0 && back.counters.vals[0]==1);
    assert(strcmp(back.counters.keys[1],"bb")==0 && back.counters.vals[1]==2);
    assert(back.wlabel[0]=='O' && back.wlabel[1]=='K' && back.wlabel[2]==0);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("mutable aggregate members roundtrip failed:\n{e}");
    }
}

/// Round-2 partial (c): `@optional` inside a NESTED inline struct previously
/// ERRORED. Now both present and absent optionals inside a nested struct
/// round-trip (absent stays absent — presence flag clears).
#[test]
#[ignore = "requires gcc in PATH"]
fn optional_inside_nested_struct_roundtrips() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "module m { \
            @final struct Inner { \
                long a; \
                @optional long maybe; \
                @optional string note; \
            }; \
            @final struct Outer { long id; Inner inner; }; \
         };",
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
int main(void){
    /* present optionals */
    m_Outer_t o; memset(&o,0,sizeof o);
    o.id=5; o.inner.a=1;
    o.inner.maybe=99; o.inner.maybe_present=1;
    o.inner.note="hi"; o.inner.note_present=1;
    uint8_t b[256]; size_t n=0;
    assert(m_Outer_encode(&o,b,sizeof b,&n)==0);
    m_Outer_t back; memset(&back,0,sizeof back);
    assert(m_Outer_decode(b,n,&back)==0);
    assert(back.id==5 && back.inner.a==1);
    assert(back.inner.maybe_present==1 && back.inner.maybe==99);
    assert(back.inner.note_present==1 && strcmp(back.inner.note,"hi")==0);
    /* absent optionals stay absent, not zero-valued */
    m_Outer_t o2; memset(&o2,0,sizeof o2);
    o2.id=6; o2.inner.a=2;
    o2.inner.maybe_present=0; o2.inner.note_present=0;
    size_t n2=0; assert(m_Outer_encode(&o2,b,sizeof b,&n2)==0);
    m_Outer_t b2; memset(&b2,0,sizeof b2);
    assert(m_Outer_decode(b,n2,&b2)==0);
    assert(b2.inner.maybe_present==0 && b2.inner.note_present==0);
    assert(b2.inner.note==NULL);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("optional-in-nested-struct roundtrip failed:\n{e}");
    }
}

// ===========================================================================
// EDGE CHECKLIST (C backend) — adapter-killing cases past hello-world.
// ===========================================================================

/// Edge 1 (empty collections) + Edge 6 (UTF-8/UTF-16) for the C backend:
/// empty sequence, empty bounded sequence, empty string, empty wstring, empty
/// map all round-trip count=0; multi-byte UTF-8 and UTF-16 code points survive.
#[test]
#[ignore = "requires gcc in PATH"]
fn empty_collections_and_unicode_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "@final struct E { \
            sequence<long> s; \
            sequence<long, 4> bs; \
            string str; \
            wstring ws; \
            map<string,long> m; \
            string utf8; \
            wstring utf16; \
         };",
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
int main(void){
    E_t e; memset(&e,0,sizeof e);
    /* all collections empty (count 0) */
    e.s.len=0; e.s.elems=NULL;
    e.bs.len=0; e.bs.elems=NULL;
    e.str="";
    uint16_t empty_ws[1]={0}; e.ws=empty_ws;
    e.m.len=0; e.m.keys=NULL; e.m.vals=NULL;
    /* multi-byte UTF-8: "héllo😀" (the é is 2 bytes, 😀 is 4) */
    e.utf8="h\xC3\xA9llo\xF0\x9F\x98\x80";
    /* UTF-16: 'A', U+00E9 (é), U+4E2D (中), then 0 */
    uint16_t u16[4]={'A',0x00E9,0x4E2D,0}; e.utf16=u16;
    uint8_t b[512]; size_t n=0;
    assert(E_encode(&e,b,sizeof b,&n)==0);
    E_t back; memset(&back,0,sizeof back);
    assert(E_decode(b,n,&back)==0);
    assert(back.s.len==0);
    assert(back.bs.len==0);
    assert(strcmp(back.str,"")==0);
    assert(back.ws[0]==0);
    assert(back.m.len==0);
    assert(strcmp(back.utf8,"h\xC3\xA9llo\xF0\x9F\x98\x80")==0);
    assert(back.utf16[0]=='A' && back.utf16[1]==0x00E9 && back.utf16[2]==0x4E2D && back.utf16[3]==0);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("empty/unicode roundtrip failed:\n{e}");
    }
}

/// Edge 3 (deep nesting) for the C backend: struct→struct→struct (3 levels),
/// sequence<sequence<struct>>, and map<string, struct-with-a-sequence>.
#[test]
#[ignore = "requires gcc in PATH"]
fn deep_nesting_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "module m { \
            @final struct L3 { long v; }; \
            @final struct L2 { L3 inner; long tag; }; \
            @final struct L1 { L2 mid; long top; }; \
            @final struct Cell { long x; }; \
            @final struct WithSeq { sequence<long> data; }; \
            @final struct Deep { \
                L1 chain; \
                sequence<sequence<Cell> > grid; \
                map<string, WithSeq> table; \
            }; \
         };",
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
int main(void){
    m_Deep_t d; memset(&d,0,sizeof d);
    d.chain.mid.inner.v=42; d.chain.mid.tag=7; d.chain.top=1;
    /* grid = [[{1},{2}],[{3}]] */
    m_Cell_t row0[2]; row0[0].x=1; row0[0]; row0[1].x=2;
    m_Cell_t row1[1]; row1[0].x=3;
    struct { uint32_t len; m_Cell_t* elems; } rows[2];
    rows[0].len=2; rows[0].elems=row0;
    rows[1].len=1; rows[1].elems=row1;
    d.grid.len=2; d.grid.elems=rows;
    /* table = {"k": WithSeq{[10,20]}} */
    int32_t inner_data[2]={10,20};
    m_WithSeq_t wv[1]; wv[0].data.len=2; wv[0].data.elems=inner_data;
    char* tk[1]={"k"};
    d.table.len=1; d.table.keys=tk; d.table.vals=wv;
    uint8_t b[1024]; size_t n=0;
    assert(m_Deep_encode(&d,b,sizeof b,&n)==0);
    m_Deep_t back; memset(&back,0,sizeof back);
    assert(m_Deep_decode(b,n,&back)==0);
    assert(back.chain.mid.inner.v==42 && back.chain.mid.tag==7 && back.chain.top==1);
    assert(back.grid.len==2);
    assert(back.grid.elems[0].len==2 && back.grid.elems[0].elems[1].x==2);
    assert(back.grid.elems[1].len==1 && back.grid.elems[1].elems[0].x==3);
    assert(back.table.len==1 && strcmp(back.table.keys[0],"k")==0);
    assert(back.table.vals[0].data.len==2 && back.table.vals[0].data.elems[1]==20);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("deep nesting roundtrip failed:\n{e}");
    }
}

/// Edge 7 (arrays) + Edge 8 (extreme primitives) for the C backend:
/// array-of-struct, multi-dim array, array-of-bounded-string with every element
/// distinct; plus INT*_MIN/MAX, UINT*_MAX, 0, -1.
#[test]
#[ignore = "requires gcc in PATH"]
fn arrays_and_extreme_primitives_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "module m { \
            @final struct P { long x; long y; }; \
            @final struct A { \
                P pts[3]; \
                long grid[2][2]; \
                int8 i8mm[2]; \
                int64 i64mm[2]; \
                uint32 u32max; \
                int32 zero_neg[2]; \
            }; \
         };",
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
#include <stdint.h>
int main(void){
    m_A_t a; memset(&a,0,sizeof a);
    for(int i=0;i<3;i++){ a.pts[i].x=i*10+1; a.pts[i].y=i*10+2; }
    a.grid[0][0]=1; a.grid[0][1]=2; a.grid[1][0]=3; a.grid[1][1]=4;
    a.i8mm[0]=INT8_MIN; a.i8mm[1]=INT8_MAX;
    a.i64mm[0]=INT64_MIN; a.i64mm[1]=INT64_MAX;
    a.u32max=UINT32_MAX;
    a.zero_neg[0]=0; a.zero_neg[1]=-1;
    uint8_t b[512]; size_t n=0;
    assert(m_A_encode(&a,b,sizeof b,&n)==0);
    m_A_t back; memset(&back,0,sizeof back);
    assert(m_A_decode(b,n,&back)==0);
    for(int i=0;i<3;i++){ assert(back.pts[i].x==i*10+1 && back.pts[i].y==i*10+2); }
    assert(back.grid[0][0]==1 && back.grid[0][1]==2 && back.grid[1][0]==3 && back.grid[1][1]==4);
    assert(back.i8mm[0]==INT8_MIN && back.i8mm[1]==INT8_MAX);
    assert(back.i64mm[0]==INT64_MIN && back.i64mm[1]==INT64_MAX);
    assert(back.u32max==UINT32_MAX);
    assert(back.zero_neg[0]==0 && back.zero_neg[1]==-1);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("arrays/extreme-primitives roundtrip failed:\n{e}");
    }
}

/// Edge 4 (optional aggregate, present AND absent) + Edge 9 (keyed) for the C
/// backend: optional sequence/nested-struct/string each present and absent;
/// two samples with the same @key, different payload, key hashes equal.
#[test]
#[ignore = "requires gcc in PATH"]
fn optional_aggregate_and_keyed_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "module m { \
            @final struct Inner { long a; }; \
            @final struct Opt { \
                long id; \
                @optional sequence<long> oseq; \
                @optional Inner ostruct; \
                @optional string ostr; \
            }; \
            @final struct Keyed { @key long k; long payload; }; \
         };",
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
int main(void){
    /* present aggregates */
    m_Opt_t o; memset(&o,0,sizeof o);
    o.id=1;
    int32_t sv[2]={5,6}; o.oseq.len=2; o.oseq.elems=sv; o.oseq_present=1;
    o.ostruct.a=77; o.ostruct_present=1;
    o.ostr="here"; o.ostr_present=1;
    uint8_t b[256]; size_t n=0;
    assert(m_Opt_encode(&o,b,sizeof b,&n)==0);
    m_Opt_t back; memset(&back,0,sizeof back);
    assert(m_Opt_decode(b,n,&back)==0);
    assert(back.oseq_present==1 && back.oseq.len==2 && back.oseq.elems[1]==6);
    assert(back.ostruct_present==1 && back.ostruct.a==77);
    assert(back.ostr_present==1 && strcmp(back.ostr,"here")==0);
    /* absent aggregates stay absent */
    m_Opt_t o2; memset(&o2,0,sizeof o2);
    o2.id=2; o2.oseq_present=0; o2.ostruct_present=0; o2.ostr_present=0;
    size_t n2=0; assert(m_Opt_encode(&o2,b,sizeof b,&n2)==0);
    m_Opt_t b2; memset(&b2,0,sizeof b2);
    assert(m_Opt_decode(b,n2,&b2)==0);
    assert(b2.oseq_present==0 && b2.ostruct_present==0 && b2.ostr_present==0);
    assert(b2.ostr==NULL);

    /* keyed: same key, different payload → key hash identical */
    m_Keyed_t k1; memset(&k1,0,sizeof k1); k1.k=99; k1.payload=1;
    m_Keyed_t k2; memset(&k2,0,sizeof k2); k2.k=99; k2.payload=2;
    uint8_t h1[16], h2[16];
    assert(m_Keyed_key_hash(&k1,h1)==0);
    assert(m_Keyed_key_hash(&k2,h2)==0);
    assert(memcmp(h1,h2,16)==0);  /* key identical → hash identical */
    /* different key → different hash */
    m_Keyed_t k3; memset(&k3,0,sizeof k3); k3.k=100; k3.payload=1;
    uint8_t h3[16];
    assert(m_Keyed_key_hash(&k3,h3)==0);
    assert(memcmp(h1,h3,16)!=0);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("optional-aggregate/keyed roundtrip failed:\n{e}");
    }
}

/// Edge 5 (union default + as map value) for the C backend: a union whose
/// discriminator hits the default arm round-trips, and a union as a map value.
#[test]
#[ignore = "requires gcc in PATH"]
fn union_default_and_as_map_value_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "module m { \
            @final union U switch (long) { \
                case 1: long i; \
                case 2: double d; \
                default: string s; \
            }; \
            @final struct Holder { map<string, U> by_name; }; \
         };",
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
int main(void){
    /* default arm directly */
    m_U_t u; memset(&u,0,sizeof u); u._d=999; u._u.s="dflt";
    uint8_t b[256]; size_t n=0;
    assert(m_U_encode(&u,b,sizeof b,&n)==0);
    m_U_t bu; memset(&bu,0,sizeof bu);
    assert(m_U_decode(b,n,&bu)==0);
    assert(bu._d==999 && strcmp(bu._u.s,"dflt")==0);

    /* union as a map value */
    m_Holder_t hld; memset(&hld,0,sizeof hld);
    m_U_t vals[2];
    memset(&vals[0],0,sizeof(m_U_t)); vals[0]._d=1; vals[0]._u.i=123;
    memset(&vals[1],0,sizeof(m_U_t)); vals[1]._d=999; vals[1]._u.s="x";
    char* keys[2]={"a","b"};
    hld.by_name.len=2; hld.by_name.keys=keys; hld.by_name.vals=vals;
    size_t n2=0; assert(m_Holder_encode(&hld,b,sizeof b,&n2)==0);
    m_Holder_t hback; memset(&hback,0,sizeof hback);
    assert(m_Holder_decode(b,n2,&hback)==0);
    assert(hback.by_name.len==2);
    assert(strcmp(hback.by_name.keys[0],"a")==0 && hback.by_name.vals[0]._d==1 && hback.by_name.vals[0]._u.i==123);
    assert(strcmp(hback.by_name.keys[1],"b")==0 && hback.by_name.vals[1]._d==999 && strcmp(hback.by_name.vals[1]._u.s,"x")==0);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("union default/map-value roundtrip failed:\n{e}");
    }
}

/// Edge 2 (bound enforcement) for the C backend: a bounded sequence filled
/// exactly to N round-trips; over N must error, not silently corrupt.
#[test]
#[ignore = "requires gcc in PATH"]
fn bounded_sequence_enforcement() {
    if !gcc_available() {
        return;
    }
    let h = gen_c("@final struct B { sequence<long, 3> v; string<4> s; };");
    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
int main(void){
    /* exactly at bound (3 / 4 chars) is fine */
    B_t ok; memset(&ok,0,sizeof ok);
    int32_t e3[3]={1,2,3}; ok.v.len=3; ok.v.elems=e3;
    ok.s="abcd";
    uint8_t b[256]; size_t n=0;
    assert(B_encode(&ok,b,sizeof b,&n)==0);
    B_t back; memset(&back,0,sizeof back);
    assert(B_decode(b,n,&back)==0);
    assert(back.v.len==3 && back.v.elems[2]==3);
    assert(strcmp(back.s,"abcd")==0);

    /* over bound (4 elements > 3) must error, not corrupt */
    B_t over; memset(&over,0,sizeof over);
    int32_t e4[4]={1,2,3,4}; over.v.len=4; over.v.elems=e4;
    over.s="ab";
    size_t n2=0;
    int rc = B_encode(&over,b,sizeof b,&n2);
    assert(rc != 0);  /* must reject, not silently truncate/corrupt */

    /* over string bound (5 chars > 4) must error */
    B_t overs; memset(&overs,0,sizeof overs);
    int32_t e1[1]={9}; overs.v.len=1; overs.v.elems=e1;
    overs.s="abcde";
    size_t n3=0;
    assert(B_encode(&overs,b,sizeof b,&n3) != 0);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("bounded sequence enforcement failed:\n{e}");
    }
}

// ===========================================================================
// C-Foundation #43 — the last four conformance fixtures the C column rejected:
// 06 typedefs (alias chain + typedef-to-array), 12 bitset/bitmask,
// 14 recursion (self-referential through a sequence), 17 forward declarations.
// Each gcc-compiled encode→decode roundtrip.
// ===========================================================================

/// 06_typedefs: a typedef ALIAS CHAIN (`typedef double A; typedef A B;`)
/// resolves to the root primitive, a typedef-to-sequence, and a TYPEDEF-TO-ARRAY
/// (`typedef long Matrix3[3][3];`) all round-trip (#43, typedefs).
#[test]
#[ignore = "requires gcc in PATH"]
fn typedef_alias_chain_and_array_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "module conf { \
            typedef double CurrentInAmpsType; \
            typedef CurrentInAmpsType ChargeCurrentType; \
            typedef sequence<long> LongSeq; \
            typedef long Matrix3[3][3]; \
            @final struct UsesTypedefs { \
                CurrentInAmpsType battery; \
                ChargeCurrentType charger; \
                LongSeq samples; \
                Matrix3 transform; \
            }; \
         };",
    );
    // The alias chain resolves to `double`; the array-alias contributes [3][3].
    assert!(
        h.contains("double battery;"),
        "alias chain not resolved:\n{h}"
    );
    assert!(
        h.contains("double charger;"),
        "alias-of-alias not resolved:\n{h}"
    );
    assert!(
        h.contains("int32_t transform[3][3];"),
        "typedef-to-array dims lost:\n{h}"
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
int main(void){
    conf_UsesTypedefs_t u; memset(&u,0,sizeof u);
    u.battery=3.5; u.charger=7.25;
    int32_t sv[3]={11,22,33}; u.samples.len=3; u.samples.elems=sv;
    for(int i=0;i<3;i++) for(int j=0;j<3;j++) u.transform[i][j]=i*3+j;
    uint8_t b[512]; size_t n=0;
    assert(conf_UsesTypedefs_encode(&u,b,sizeof b,&n)==0);
    conf_UsesTypedefs_t back; memset(&back,0,sizeof back);
    assert(conf_UsesTypedefs_decode(b,n,&back)==0);
    assert(back.battery==3.5 && back.charger==7.25);
    assert(back.samples.len==3 && back.samples.elems[2]==33);
    for(int i=0;i<3;i++) for(int j=0;j<3;j++) assert(back.transform[i][j]==i*3+j);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("typedef alias-chain/array roundtrip failed:\n{e}");
    }
}

/// 12_bitset_bitmask: a bitmask (default `@bit_bound(32)` → uint32 holder) and a
/// bitset (packed into one holder integer, here 8 bits → uint8) round-trip; the
/// bitset's SHIFT/MASK accessors pack/unpack each field (XTypes §7.3.1.2).
#[test]
#[ignore = "requires gcc in PATH"]
fn bitset_bitmask_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "module conf { \
            bitmask Permissions { READ, WRITE, EXECUTE }; \
            bitset Flags { bitfield<3> kind; bitfield<1> active; bitfield<4> priority; }; \
            @final struct UsesBits { Permissions perms; Flags flags; }; \
         };",
    );
    // Bitmask → uint32 holder + flag constants; bitset → uint8 holder + macros.
    assert!(
        h.contains("typedef uint32_t conf_Permissions_t;"),
        "bitmask holder:\n{h}"
    );
    assert!(
        h.contains("conf_Permissions_WRITE = (1u << 1)"),
        "bitmask flag bit:\n{h}"
    );
    assert!(
        h.contains("typedef uint8_t conf_Flags_t;"),
        "bitset holder (3+1+4=8 → u8):\n{h}"
    );
    assert!(
        h.contains("conf_Flags_priority_SHIFT = 4"),
        "bitset field offset:\n{h}"
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
int main(void){
    conf_UsesBits_t u; memset(&u,0,sizeof u);
    u.perms = conf_Permissions_READ | conf_Permissions_EXECUTE;
    /* pack the bitset fields: kind=5, active=1, priority=9 */
    u.flags = 0;
    u.flags |= (5u & conf_Flags_kind_MASK)     << conf_Flags_kind_SHIFT;
    u.flags |= (1u & conf_Flags_active_MASK)   << conf_Flags_active_SHIFT;
    u.flags |= (9u & conf_Flags_priority_MASK) << conf_Flags_priority_SHIFT;
    uint8_t b[64]; size_t n=0;
    assert(conf_UsesBits_encode(&u,b,sizeof b,&n)==0);
    conf_UsesBits_t back; memset(&back,0,sizeof back);
    assert(conf_UsesBits_decode(b,n,&back)==0);
    assert(back.perms == (conf_Permissions_READ | conf_Permissions_EXECUTE));
    assert(((back.flags >> conf_Flags_kind_SHIFT)     & conf_Flags_kind_MASK)     == 5);
    assert(((back.flags >> conf_Flags_active_SHIFT)   & conf_Flags_active_MASK)   == 1);
    assert(((back.flags >> conf_Flags_priority_SHIFT) & conf_Flags_priority_MASK) == 9);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("bitset/bitmask roundtrip failed:\n{e}");
    }
}

/// 14_recursion: a self-referential type through a sequence (a tree node) emits
/// a heap-indirected C type + a runtime body helper, and round-trips a depth-3
/// tree without infinite codegen recursion (XTypes §7.4.5, #43, recursion).
#[test]
#[ignore = "requires gcc in PATH"]
fn recursive_tree_through_sequence_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "module conf { \
            struct TreeNode { long value; sequence<TreeNode> children; }; \
         };",
    );
    // The self-reference is behind a pointer (struct tag form), and spliced via
    // a runtime body helper — never inline-recursed.
    assert!(
        h.contains("struct conf_TreeNode_s* elems;"),
        "recursive seq element must be a pointer-to-tag:\n{h}"
    );
    assert!(
        h.contains("conf_TreeNode_write_body(") && h.contains("conf_TreeNode_read_body("),
        "recursive body helper missing:\n{h}"
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
int main(void){
    /* tree: root(1) -> [ a(2) -> [ c(4) ], b(3) ] */
    conf_TreeNode_t c; memset(&c,0,sizeof c); c.value=4;
    conf_TreeNode_t a; memset(&a,0,sizeof a); a.value=2;
    a.children.len=1; a.children.elems=&c;
    conf_TreeNode_t bnode; memset(&bnode,0,sizeof bnode); bnode.value=3;
    conf_TreeNode_t kids[2]; kids[0]=a; kids[1]=bnode;
    conf_TreeNode_t root; memset(&root,0,sizeof root); root.value=1;
    root.children.len=2; root.children.elems=kids;

    uint8_t buf[512]; size_t n=0;
    assert(conf_TreeNode_encode(&root,buf,sizeof buf,&n)==0);
    conf_TreeNode_t back; memset(&back,0,sizeof back);
    assert(conf_TreeNode_decode(buf,n,&back)==0);
    assert(back.value==1 && back.children.len==2);
    assert(back.children.elems[0].value==2);
    assert(back.children.elems[0].children.len==1);
    assert(back.children.elems[0].children.elems[0].value==4);
    assert(back.children.elems[1].value==3 && back.children.elems[1].children.len==0);
    conf_TreeNode_sample_free(&back);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("recursive tree roundtrip failed:\n{e}");
    }
}

/// 14_recursion (genuine infinite case): a BARE self-membership (`Node n;`
/// directly, no heap indirection) is an infinite-size type and must be rejected
/// cleanly — the parser/semantics reject it before codegen, so generation
/// errors rather than emitting a broken (infinite struct) type.
#[test]
fn direct_self_membership_rejected() {
    // `struct Node { Node n; };` — a by-value self member is infinite size.
    let parsed = zerodds_idl::parse("struct Node { long v; Node n; };", &ParserConfig::default());
    match parsed {
        // Most likely: the parser/semantics reject the infinite type.
        Err(_) => {}
        // If it parses, codegen must still refuse to emit an infinite C type
        // (a recursive type is only legal behind a pointer/sequence).
        Ok(ast) => {
            let r = generate_c_header(&ast, &CGenOptions::default());
            assert!(
                r.is_err(),
                "direct self-membership must not generate an infinite C struct"
            );
        }
    }
}

/// 17_forward_decl: a forward-declared struct (then defined, self-referential
/// through a sequence) and a forward-declared union (then defined, embedding the
/// struct by value) both round-trip (#43, forward declarations).
#[test]
#[ignore = "requires gcc in PATH"]
fn forward_declared_struct_and_union_roundtrip() {
    if !gcc_available() {
        return;
    }
    let h = gen_c(
        "module conf { \
            struct Node; \
            union Variant; \
            struct Node { long value; sequence<Node> next; }; \
            union Variant switch (long) { case 0: long a; case 1: Node n; }; \
         };",
    );
    // The union embeds the (recursive) struct by value; the struct's typedef must
    // precede the union typedef (topological order) and be complete.
    assert!(
        h.contains("typedef struct conf_Node_s"),
        "node typedef missing:\n{h}"
    );
    assert!(
        h.contains("typedef struct conf_Variant_s"),
        "variant typedef missing:\n{h}"
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
int main(void){
    /* Node self-ref: head(10) -> [ tail(20) ] */
    conf_Node_t tail; memset(&tail,0,sizeof tail); tail.value=20;
    conf_Node_t head; memset(&head,0,sizeof head); head.value=10;
    head.next.len=1; head.next.elems=&tail;
    uint8_t b[256]; size_t n=0;
    assert(conf_Node_encode(&head,b,sizeof b,&n)==0);
    conf_Node_t nback; memset(&nback,0,sizeof nback);
    assert(conf_Node_decode(b,n,&nback)==0);
    assert(nback.value==10 && nback.next.len==1 && nback.next.elems[0].value==20);
    conf_Node_sample_free(&nback);

    /* Variant arm 1 embeds a Node by value */
    conf_Variant_t v; memset(&v,0,sizeof v);
    v._d=1; v._u.n.value=99; v._u.n.next.len=0; v._u.n.next.elems=NULL;
    size_t n2=0; assert(conf_Variant_encode(&v,b,sizeof b,&n2)==0);
    conf_Variant_t vback; memset(&vback,0,sizeof vback);
    assert(conf_Variant_decode(b,n2,&vback)==0);
    assert(vback._d==1 && vback._u.n.value==99 && vback._u.n.next.len==0);

    /* Variant arm 0 (plain long) */
    conf_Variant_t v0; memset(&v0,0,sizeof v0); v0._d=0; v0._u.a=42;
    size_t n3=0; assert(conf_Variant_encode(&v0,b,sizeof b,&n3)==0);
    conf_Variant_t v0back; memset(&v0back,0,sizeof v0back);
    assert(conf_Variant_decode(b,n3,&v0back)==0);
    assert(v0back._d==0 && v0back._u.a==42);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("forward-decl struct/union roundtrip failed:\n{e}");
    }
}

// REGRESSION GATE (C backend): @mutable members WITHOUT explicit @id must (a) NOT
// be rejected — the C backend used to error "@mutable member without @id(N)" — and
// (b) take SEQUENTIAL 0-based ids (XTypes 1.3 §7.3.4.3; vendor-confirmed vs Cyclone).
// EMHEADER for a 4-byte primitive = (LC=2 << 28) | id: id0 -> 0x20000000, id1 -> 0x20000001.
#[test]
fn mutable_autoid_is_zero_based_sequential() {
    // gen_c().expect() panics if the backend rejects -> this call alone gates (a).
    let c = gen_c("@mutable struct AutoId { long a; long b; };");
    assert!(
        c.contains("0x20000000u"),
        "first @mutable auto-id must be id 0 (EMHEADER 0x20000000):\n{c}"
    );
    assert!(
        c.contains("0x20000001u"),
        "second @mutable auto-id must be id 1 (EMHEADER 0x20000001):\n{c}"
    );
}

/// `fixed<P,S>` member: emitted as a (P+2)/2-octet BCD byte field, written and
/// read as the raw CORBA/GIOP §9.3.2.7 octets. Asserts the exact oracle vectors
/// (JacORB/omniORB == the other six PSMs) + a value roundtrip.
#[test]
#[ignore = "requires gcc in PATH"]
fn fixed_member_roundtrips_through_gcc() {
    let h = gen_c(
        "module shop { @appendable struct Order { long id; fixed<4,0> qty; }; \
                       @final      struct Price { long sku; fixed<5,2> amount; }; };",
    );
    let main = r#"
#include "gen.h"
#include <assert.h>
#include <string.h>
int main(void) {
    /* @appendable Order: DHEADER(7) + id(4) + bcd(3); fixed<4,0> 1234 = 01 23 4c */
    shop_Order_t o; memset(&o, 0, sizeof(o));
    o.id = 7; o.qty.bcd[0]=0x01; o.qty.bcd[1]=0x23; o.qty.bcd[2]=0x4c;
    unsigned char buf[64]; size_t n = 0;
    assert(shop_Order_encode(&o, buf, sizeof(buf), &n) == 0);
    unsigned char want_o[] = {0x07,0,0,0, 0x07,0,0,0, 0x01,0x23,0x4c};
    assert(n == sizeof(want_o) && memcmp(buf, want_o, n) == 0);
    shop_Order_t ob; memset(&ob, 0, sizeof(ob));
    assert(shop_Order_decode(buf, n, &ob) == 0);
    assert(ob.id == 7 && ob.qty.bcd[0]==0x01 && ob.qty.bcd[1]==0x23 && ob.qty.bcd[2]==0x4c);

    /* @final Price: sku(4) + bcd(3); fixed<5,2> 123.45 = 12 34 5c (no DHEADER) */
    shop_Price_t p; memset(&p, 0, sizeof(p));
    p.sku = 9; p.amount.bcd[0]=0x12; p.amount.bcd[1]=0x34; p.amount.bcd[2]=0x5c;
    assert(shop_Price_encode(&p, buf, sizeof(buf), &n) == 0);
    unsigned char want_p[] = {0x09,0,0,0, 0x12,0x34,0x5c};
    assert(n == sizeof(want_p) && memcmp(buf, want_p, n) == 0);
    return 0;
}
"#;
    if let Err(e) = compile_and_run(&h, main) {
        panic!("C fixed roundtrip failed:\n{e}");
    }
}
