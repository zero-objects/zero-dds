//! Adversarial corpus — generates TypeScript from IDL and type-checks it
//! with the real `tsc` toolchain.
//!
//! Three axes, per the cross-backend adversarial-corpus test plan:
//!   1. **reserved-keyword corpus** — every ECMAScript reserved word that
//!      is also a legal IDL identifier, placed at each binding position
//!      (member / struct / enum / module / const / union branch), must
//!      generate `tsc --strict` clean output.
//!   2. **construct corpus** — each IDL construct minimally (enum `@value`,
//!      const of every type, struct inheritance, union with every
//!      discriminator kind, bitset/bitmask, `@optional` + extensibility,
//!      seq, multidim array, map, nested + reopened module, wchar, wstring,
//!      long double, fixed, typedef, and — the F38 target —
//!      interface-nested types) must compile.
//!   3. **compose (multi-file)** — two IDLs generated separately then
//!      combined into one project must compile together.
//!
//! **Prerequisite:** `tsc` on the PATH (e.g. `npm i -g typescript`). All
//! tests no-op with a warning when it is absent (mirrors `compile_check.rs`),
//! so the suite is green on toolchain-free hosts and meaningful on CI.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_ts::generate_ts_source;

fn tsc_available() -> bool {
    Command::new("tsc")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn gen_ts(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default())
        .unwrap_or_else(|e| panic!("parse {src:?}: {e:?}"));
    generate_ts_source(&ast).unwrap_or_else(|e| panic!("gen {src:?}: {e:?}"))
}

/// Writes the `@zerodds/types` and `@zerodds/cdr` runtime stubs. The
/// descriptor interfaces are **permissive** (index-signature'd, generic)
/// so that `interface`-construct output — which emits
/// `ServiceDescriptor<Client, Handler>` — type-checks; the non-generic
/// stub in `compile_check.rs` deliberately cannot (see its trailing note).
fn write_runtime_stubs(root: &std::path::Path) -> Result<(), String> {
    use std::io::Write;
    let pkg_types = root.join("node_modules/@zerodds/types");
    let pkg_cdr = root.join("node_modules/@zerodds/cdr");
    std::fs::create_dir_all(&pkg_types).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&pkg_cdr).map_err(|e| e.to_string())?;

    let cdr_d_ts = "
export type ExtensibilityKind = 'final' | 'appendable' | 'mutable';
export type EndianMode = 'le' | 'be';
export interface DdsTopicType<T> {
    readonly typeName: string;
    readonly isKeyed: boolean;
    readonly extensibility: ExtensibilityKind;
    encode(sample: T, endian?: EndianMode, representation?: number): Uint8Array;
    decode(bytes: Uint8Array, offset?: number, length?: number, endian?: EndianMode, representation?: number): T;
    encodeInto(w: Xcdr2Writer, sample: T): void;
    decodeFrom(r: Xcdr2Reader): T;
    keyHash(sample: T): Uint8Array;
}
export class XcdrError extends Error {}
export class Xcdr2Writer {
    constructor(endian?: EndianMode, maxAlign?: number);
    readonly pos: number;
    readonly endian: EndianMode;
    readonly isXcdr1: boolean;
    toBytes(): Uint8Array;
    align(alignment: number): void;
    pushAlignmentOrigin(): void;
    popAlignmentOrigin(): void;
    writeBool(v: boolean): void;
    writeOctet(v: number): void;
    writeChar(c: string): void;
    writeWChar(c: string): void;
    writeInt8(v: number): void;
    writeUint8(v: number): void;
    writeInt16(v: number): void;
    writeUint16(v: number): void;
    writeInt32(v: number): void;
    writeUint32(v: number): void;
    writeInt64(v: bigint): void;
    writeUint64(v: bigint): void;
    writeFloat32(v: number): void;
    writeFloat64(v: number): void;
    writeLongDouble(v: number): void;
    writeString(s: string): void;
    writeWString(s: string): void;
    writeBytes(bytes: Uint8Array): void;
    writeFixedBcd(decimal: string, p: number, s: number): void;
    patchUint32(pos: number, value: number): void;
    beginAppendable(): number;
    endAppendable(token: number): void;
    beginMutable(): number;
    endMutable(token: number): void;
    writeEmHeader(memberId: number, lc: number, mustUnderstand?: boolean, nextInt?: number): void;
    writePlCdr1Member(memberId: number, body: Uint8Array): void;
    writePlCdr1Sentinel(): void;
}
export class Xcdr2Reader {
    constructor(bytes: Uint8Array, offset?: number, length?: number, endian?: EndianMode, maxAlign?: number);
    readonly pos: number;
    readonly remaining: number;
    readonly endian: EndianMode;
    readonly isXcdr1: boolean;
    align(alignment: number): void;
    pushAlignmentOrigin(): void;
    popAlignmentOrigin(): void;
    readBool(): boolean;
    readOctet(): number;
    readChar(): string;
    readWChar(): string;
    readInt8(): number;
    readUint8(): number;
    readInt16(): number;
    readUint16(): number;
    readInt32(): number;
    readUint32(): number;
    readInt64(): bigint;
    readUint64(): bigint;
    readFloat32(): number;
    readFloat64(): number;
    readLongDouble(): number;
    readString(): string;
    readWString(): string;
    readBytes(n: number): Uint8Array;
    readFixedBcd(p: number, s: number): string;
    beginAppendable(): { bodyEnd: number };
    endAppendable(token: { bodyEnd: number }): void;
    beginMutable(): { bodyEnd: number };
    endMutable(token: { bodyEnd: number }): void;
    readEmHeader(): { memberId: number; lc: number; mustUnderstand: boolean; nextInt: number | null };
    beginPlCdr1Member(): { memberId: number; bodyEnd: number } | null;
    endPlCdr1Member(member: { memberId: number; bodyEnd: number }): void;
    static lcInlineSize(lc: number): number;
}
export function md5(input: Uint8Array): Uint8Array;
";
    let mut f = std::fs::File::create(pkg_cdr.join("index.d.ts")).map_err(|e| e.to_string())?;
    write!(f, "{cdr_d_ts}").map_err(|e| e.to_string())?;
    write!(
        std::fs::File::create(pkg_cdr.join("package.json")).map_err(|e| e.to_string())?,
        r#"{{"name":"@zerodds/cdr","version":"0.0.0","types":"index.d.ts"}}"#
    )
    .map_err(|e| e.to_string())?;

    let types_d_ts = "
// Plain aliases (not branded) so codegen that types a raw `r.readChar()`
// result as `Char` type-checks under this permissive test stub.
export type Char = string;
export type WChar = string;
// `long double` is carried as a 16-byte payload object (binary128), not a
// JS number — codegen emits `(x).bytes` on encode and `makeLongDouble(
// readBytes(16))` on decode.
export interface LongDouble { readonly bytes: Uint8Array; }
export interface DdsAny { readonly typeId: string; readonly value: unknown; }
export class DdsException extends Error {}
export interface DdsTypeDescriptor<T = unknown> {
    readonly kind: string;
    readonly name: string;
    readonly [k: string]: unknown;
    readonly typeGuard?: (v: unknown) => v is T;
}
export interface DdsMemberDescriptor { readonly name: string; readonly [k: string]: unknown; }
export interface DdsTypeRef { readonly target: string; }
export interface ServiceDescriptor<C = unknown, H = unknown> {
    readonly name: string;
    readonly [k: string]: unknown;
}
export interface OperationDescriptor { readonly name: string; readonly [k: string]: unknown; }
export interface OperationParameterDescriptor { readonly name: string; readonly [k: string]: unknown; }
export interface AttributeDescriptor { readonly name: string; readonly [k: string]: unknown; }
export type ParameterMode = 'IN' | 'OUT' | 'INOUT';
export function registerType<T>(_d: DdsTypeDescriptor<T>): void {}
export function makeChar(_s: string): Char { return '' as Char; }
export function makeWChar(_s: string): WChar { return '' as WChar; }
export function makeLongDouble(_b: Uint8Array): LongDouble { return { bytes: _b }; }
";
    let mut f = std::fs::File::create(pkg_types.join("index.d.ts")).map_err(|e| e.to_string())?;
    write!(f, "{types_d_ts}").map_err(|e| e.to_string())?;
    write!(
        std::fs::File::create(pkg_types.join("package.json")).map_err(|e| e.to_string())?,
        r#"{{"name":"@zerodds/types","version":"0.0.0","types":"index.d.ts"}}"#
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

const TSCONFIG: &str = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "esnext",
    "moduleResolution": "bundler",
    "strict": true,
    "noEmit": true,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "isolatedModules": false
  },
  "include": ["*.ts"]
}
"#;

/// Compiles a set of generated `.ts` modules together in one tsc project.
/// `files` is `(basename, source)`. Returns `Ok(())` when `tsc` is absent
/// (skip) or the project type-checks; `Err` with diagnostics otherwise.
fn compile_project(files: &[(&str, String)]) -> Result<(), String> {
    if !tsc_available() {
        eprintln!("WARNING: skipping adversarial-corpus compile — no tsc in PATH");
        return Ok(());
    }
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    write_runtime_stubs(tmp.path())?;
    for (name, src) in files {
        std::fs::write(tmp.path().join(name), src).map_err(|e| e.to_string())?;
    }
    std::fs::write(tmp.path().join("tsconfig.json"), TSCONFIG).map_err(|e| e.to_string())?;

    let output = Command::new("tsc")
        .arg("--project")
        .arg(tmp.path().join("tsconfig.json"))
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let joined: String = files
            .iter()
            .map(|(n, s)| format!("--- {n} ---\n{s}\n"))
            .collect();
        Err(format!(
            "tsc FAILED:\n{joined}--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
        ))
    }
}

/// Convenience: compile a single generated module.
fn compile_one(src: &str) -> Result<(), String> {
    compile_project(&[("generated.ts", gen_ts(src))])
}

/// ECMAScript reserved words that are also legal OMG-IDL identifiers.
const RESERVED_IDL_LEGAL: &[&str] = &[
    "class",
    "extends",
    "function",
    "return",
    "throw",
    "catch",
    "delete",
    "instanceof",
    "typeof",
    "new",
    "super",
    "this",
    "debugger",
    "continue",
    "break",
    "do",
    "while",
    "with",
    "var",
    "let",
    "implements",
    "package",
    "protected",
    "static",
    "yield",
    "await",
    "null",
];

#[test]
fn reserved_keyword_corpus_compiles() {
    if !tsc_available() {
        eprintln!("WARNING: skipping reserved_keyword_corpus — no tsc in PATH");
        return;
    }
    for kw in RESERVED_IDL_LEGAL {
        // Every binding position at once: module, struct, member, enum,
        // enum-literal, const, union branch member.
        let src = format!(
            "module {kw} {{ \
               struct {kw} {{ long {kw}; }}; \
               enum E_{kw} {{ {kw}, other }}; \
               const long C_{kw} = 1; \
               union U_{kw} switch (long) {{ case 1: long {kw}; default: long other_; }}; \
             }};"
        );
        compile_one(&src).unwrap_or_else(|e| panic!("reserved keyword `{kw}` must compile:\n{e}"));
    }
}

#[test]
fn construct_corpus_compiles() {
    if !tsc_available() {
        eprintln!("WARNING: skipping construct_corpus — no tsc in PATH");
        return;
    }
    // (label, minimal IDL) — each construct on its own so a failure points
    // at exactly one feature.
    let corpus: &[(&str, &str)] = &[
        (
            "enum_value",
            "enum Mode { @value(5) FAST, SLOW, @value(9) TURBO };",
        ),
        (
            "const_all_types",
            "const boolean CB = TRUE; const octet CO = 255; const short CS = -1; \
             const unsigned short CUS = 2; const long CL = 3; const unsigned long CUL = 4; \
             const long long CLL = 5; const unsigned long long CULL = 6; \
             const float CF = 1.5; const double CD = 2.5; const char CC = 'x'; \
             const string CSTR = \"hi\";",
        ),
        (
            "struct_inheritance",
            "struct Base { long base_field; }; \
             struct Mid : Base { long mid_field; }; \
             struct Leaf : Mid { long leaf_field; };",
        ),
        (
            "union_disc_long",
            "union UL switch (long) { case 1: long a; case 2: double b; default: octet c; };",
        ),
        (
            "union_disc_char",
            "union UC switch (char) { case 'a': long x; case 'b': double y; };",
        ),
        (
            "union_disc_bool",
            "union UB switch (boolean) { case TRUE: long t; case FALSE: double f; };",
        ),
        (
            "union_disc_enum",
            "enum Sel { S_A, S_B, S_C }; \
             union UE switch (Sel) { case S_A: long a; case S_B: double b; default: octet c; };",
        ),
        ("bitset", "bitset Bs { bitfield<3> lo; bitfield<5> hi; };"),
        ("bitmask", "bitmask Bm { FLAG_A, FLAG_B, FLAG_C };"),
        (
            "optional_and_extensibility",
            "@appendable struct Ap { long a; }; \
             @final struct Fi { long b; }; \
             @mutable struct Mu { @optional long c; long d; };",
        ),
        (
            "sequence",
            "struct Sq { sequence<long> a; sequence<string> b; sequence<long, 4> c; };",
        ),
        ("array_multidim", "struct Ar { long grid[2][3][4]; };"),
        ("map", "struct Mp { map<string, long> m; };"),
        (
            "module_nested_reopened",
            "module A { module B { struct Inner { long v; }; }; }; \
             module A { struct AfterReopen { long w; }; };",
        ),
        ("wchar_wstring", "struct Wc { wchar c; wstring s; };"),
        ("long_double", "struct Ld { long double big; long n; };"),
        ("fixed", "struct Fx { fixed<10,2> amount; long n; };"),
        (
            "typedef",
            "typedef sequence<long> LongSeq; struct Td { LongSeq xs; };",
        ),
        (
            "interface_nested_types",
            "module M { interface Sensor { \
               struct Reading { long value; double ts; }; \
               enum Health { OK, DEGRADED }; \
               const long MAX = 100; \
               exception Fault { string reason; }; \
               long read(); \
             }; };",
        ),
        (
            "interface_plain_and_inherited",
            "interface Base { long ping(); }; \
             interface Derived : Base { void pong(in long x); };",
        ),
    ];
    for (label, src) in corpus {
        compile_one(src).unwrap_or_else(|e| panic!("construct `{label}` must compile:\n{e}"));
    }
}

#[test]
fn compose_multifile_compiles() {
    if !tsc_available() {
        eprintln!("WARNING: skipping compose_multifile — no tsc in PATH");
        return;
    }
    // Two IDL units generated separately, then combined into one project —
    // each generated file is a self-contained ES module with its own
    // imports. They must coexist and type-check together.
    let a = gen_ts(
        "module geo { struct Point { long x; long y; }; }; \
                 interface Registry { struct Entry { long id; }; long count(); };",
    );
    let b = gen_ts(
        "module telemetry { \
                   enum Level { LOW, HIGH }; \
                   @appendable struct Sample { long ts; Level lvl; }; \
                 };",
    );
    compile_project(&[("unit_a.ts", a), ("unit_b.ts", b)])
        .expect("separately-generated units must compose and compile");
}
