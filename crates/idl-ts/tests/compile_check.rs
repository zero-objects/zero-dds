//! TS-3 — codegen compile tests for TypeScript.
//!
//! Generates TypeScript source from IDL and calls `tsc --noEmit` with a
//! stub runtime for `@zerodds/types` and `@zerodds/amqp/codec`.
//!
//! **Prerequisite:** `tsc` on the PATH (e.g. via `npm install -g typescript`).
//! Tests are skipped if it is not available.

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

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_ts::{generate_ts_source, generate_ts_source_with_amqp};

fn tsc_available() -> bool {
    Command::new("tsc")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Stub runtime for `@zerodds/types` and `@zerodds/amqp/codec`,
/// so the emitted TS code can have its `import`s resolved.
fn write_runtime_stubs(root: &std::path::Path) -> Result<(), String> {
    use std::io::Write;
    let pkg_types = root.join("node_modules/@zerodds/types");
    let pkg_amqp = root.join("node_modules/@zerodds/amqp/codec");
    let pkg_cdr = root.join("node_modules/@zerodds/cdr");
    std::fs::create_dir_all(&pkg_types).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&pkg_amqp).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&pkg_cdr).map_err(|e| e.to_string())?;

    // @zerodds/cdr — stub for XCDR2 TypeSupport.
    let cdr_d_ts = "
export type ExtensibilityKind = 'final' | 'appendable' | 'mutable';
export type EndianMode = 'le' | 'be';
export interface DdsTopicType<T> {
    readonly typeName: string;
    readonly isKeyed: boolean;
    readonly extensibility: ExtensibilityKind;
    encode(sample: T, endian?: EndianMode): Uint8Array;
    decode(bytes: Uint8Array, offset?: number, length?: number): T;
    encodeInto(w: Xcdr2Writer, sample: T): void;
    decodeFrom(r: Xcdr2Reader): T;
    keyHash(sample: T): Uint8Array;
}
export class XcdrError extends Error {}
export class Xcdr2Writer {
    constructor(endian?: EndianMode, maxAlign?: number);
    readonly pos: number;
    readonly endian: EndianMode;
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
}
export class Xcdr2Reader {
    constructor(bytes: Uint8Array, offset?: number, length?: number, endian?: EndianMode, maxAlign?: number);
    readonly pos: number;
    readonly remaining: number;
    readonly endian: EndianMode;
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
    readString(): string;
    readWString(): string;
    readBytes(n: number): Uint8Array;
    readFixedBcd(p: number, s: number): string;
    beginAppendable(): { bodyEnd: number };
    endAppendable(token: { bodyEnd: number }): void;
    beginMutable(): { bodyEnd: number };
    endMutable(token: { bodyEnd: number }): void;
    readEmHeader(): { memberId: number; lc: number; mustUnderstand: boolean; nextInt: number | null };
    static lcInlineSize(lc: number): number;
}
export function md5(input: Uint8Array): Uint8Array;
";
    let mut f = std::fs::File::create(pkg_cdr.join("index.d.ts")).map_err(|e| e.to_string())?;
    write!(f, "{cdr_d_ts}").map_err(|e| e.to_string())?;
    let mut pkg = std::fs::File::create(pkg_cdr.join("package.json")).map_err(|e| e.to_string())?;
    write!(
        pkg,
        r#"{{"name":"@zerodds/cdr","version":"0.0.0","types":"index.d.ts"}}"#
    )
    .map_err(|e| e.to_string())?;

    // @zerodds/types — minimal type-only stubs.
    let types_d_ts = "
export type Char = string & { readonly __brand: unique symbol };
export type WChar = string & { readonly __brand: unique symbol };
export type LongDouble = number & { readonly __brand: unique symbol };
export interface DdsAny { readonly typeId: string; readonly value: unknown; }
export class DdsException extends Error {}
export interface DdsTypeDescriptor<T = unknown> {
    readonly kind: string;
    readonly name: string;
    readonly extensibility?: string;
    readonly nested?: boolean;
    readonly fields?: ReadonlyArray<DdsMemberDescriptor>;
    readonly typeGuard?: (v: unknown) => v is T;
    readonly hasDefault?: boolean;
    readonly hasImplicitDefault?: boolean;
    readonly explicitLabels?: ReadonlyArray<unknown>;
    readonly bitBound?: number;
    readonly base?: string;
    readonly aliasOf?: unknown;
}
export interface DdsMemberDescriptor {
    readonly name: string;
    readonly id?: number;
    readonly type?: unknown;
    readonly key?: boolean;
    readonly optional?: boolean;
    readonly mustUnderstand?: boolean;
    readonly labels?: ReadonlyArray<unknown>;
    readonly bound?: number;
    readonly default?: unknown;
}
export interface DdsTypeRef { readonly target: string; }
export interface ServiceDescriptor { readonly name: string; }
export interface OperationDescriptor { readonly name: string; }
export interface OperationParameterDescriptor { readonly name: string; }
export interface AttributeDescriptor { readonly name: string; }
export type ParameterMode = 'IN' | 'OUT' | 'INOUT';
export function registerType<T>(_d: DdsTypeDescriptor<T>): void {}
export function makeChar(_s: string): Char { return '' as Char; }
export function makeWChar(_s: string): WChar { return '' as WChar; }
export function makeLongDouble(_n: number): LongDouble { return 0 as LongDouble; }
";
    let mut f = std::fs::File::create(pkg_types.join("index.d.ts")).map_err(|e| e.to_string())?;
    write!(f, "{types_d_ts}").map_err(|e| e.to_string())?;
    let mut pkg =
        std::fs::File::create(pkg_types.join("package.json")).map_err(|e| e.to_string())?;
    write!(
        pkg,
        r#"{{"name":"@zerodds/types","version":"0.0.0","types":"index.d.ts"}}"#
    )
    .map_err(|e| e.to_string())?;

    // @zerodds/amqp/codec — minimal stub for AMQP-helpers.
    let amqp_d_ts = "
export interface Value { readonly __brand: unique symbol; }
export class StructBuilder {
    constructor(typeName: string);
    fieldBool(name: string, v: boolean): this;
    fieldInt8(name: string, v: number): this;
    fieldInt16(name: string, v: number): this;
    fieldInt32(name: string, v: number): this;
    fieldInt64(name: string, v: bigint): this;
    fieldUint8(name: string, v: number): this;
    fieldUint16(name: string, v: number): this;
    fieldUint32(name: string, v: number): this;
    fieldUint64(name: string, v: bigint): this;
    fieldFloat(name: string, v: number): this;
    fieldDouble(name: string, v: number): this;
    fieldString(name: string, v: string): this;
    fieldValue(name: string, v: Value): this;
    build(): Value;
}
export function makeUnionBody(disc: Value, branch?: Value): Value;
export function boolValue(v: boolean): Value;
export function int8Value(v: number): Value;
export function int16Value(v: number): Value;
export function int32Value(v: number): Value;
export function int64Value(v: bigint): Value;
export function uint8Value(v: number): Value;
export function uint16Value(v: number): Value;
export function uint32Value(v: number): Value;
export function uint64Value(v: bigint): Value;
export function floatValue(v: number): Value;
export function doubleValue(v: number): Value;
export function stringValue(v: string): Value;
export function toJson(v: Value): string;
";
    let mut f = std::fs::File::create(pkg_amqp.join("index.d.ts")).map_err(|e| e.to_string())?;
    write!(f, "{amqp_d_ts}").map_err(|e| e.to_string())?;
    let mut pkg =
        std::fs::File::create(pkg_amqp.join("package.json")).map_err(|e| e.to_string())?;
    write!(
        pkg,
        r#"{{"name":"@zerodds/amqp/codec","version":"0.0.0","types":"index.d.ts"}}"#
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Runs `tsc --noEmit --strict --noUnusedLocals` over a ready-made TypeScript
/// source string with the `@zerodds/*` runtime stubs in place. Skips (returns
/// `Ok`) when no `tsc` is on PATH.
fn tsc_check_source(ts_source: &str) -> Result<(), String> {
    if !tsc_available() {
        eprintln!("WARNING: skipping TypeScript compile-check, no tsc in PATH");
        return Ok(());
    }
    use std::io::Write;
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    write_runtime_stubs(tmp.path())?;

    let mut f =
        std::fs::File::create(tmp.path().join("generated.ts")).map_err(|e| e.to_string())?;
    write!(f, "{ts_source}").map_err(|e| e.to_string())?;

    let tsconfig = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "esnext",
    "moduleResolution": "bundler",
    "strict": true,
    "noUnusedLocals": true,
    "noEmit": true,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "isolatedModules": false
  },
  "include": ["generated.ts"]
}
"#;
    std::fs::write(tmp.path().join("tsconfig.json"), tsconfig).map_err(|e| e.to_string())?;

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
        Err(format!(
            "tsc FAILED:\n--- source ---\n{ts_source}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
        ))
    }
}

fn check_compiles_with(src: &str, with_amqp: bool) -> Result<(), String> {
    if !tsc_available() {
        eprintln!("WARNING: skipping TypeScript compile-check, no tsc in PATH");
        return Ok(());
    }
    use std::io::Write;
    let ast =
        zerodds_idl::parse(src, &ParserConfig::default()).map_err(|e| format!("parse: {e:?}"))?;
    let ts_source = if with_amqp {
        generate_ts_source_with_amqp(&ast).map_err(|e| format!("gen: {e:?}"))?
    } else {
        generate_ts_source(&ast).map_err(|e| format!("gen: {e:?}"))?
    };

    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    write_runtime_stubs(tmp.path())?;

    let mut f =
        std::fs::File::create(tmp.path().join("generated.ts")).map_err(|e| e.to_string())?;
    write!(f, "{ts_source}").map_err(|e| e.to_string())?;

    // tsconfig.json — minimal, target ES2022, strict, noEmit.
    let tsconfig = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "esnext",
    "moduleResolution": "bundler",
    "strict": true,
    "noUnusedLocals": true,
    "noEmit": true,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "isolatedModules": false
  },
  "include": ["generated.ts"]
}
"#;
    std::fs::write(tmp.path().join("tsconfig.json"), tsconfig).map_err(|e| e.to_string())?;

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
        Err(format!(
            "tsc FAILED:\n--- source ---\n{ts_source}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
        ))
    }
}

fn check_compiles(src: &str) -> Result<(), String> {
    check_compiles_with(src, false)
}

#[test]
fn compiles_simple_struct() {
    check_compiles("struct Point { long x; long y; };").expect("simple struct must compile");
}

#[test]
fn compiles_struct_with_string_sequence() {
    check_compiles("struct Bag { string name; sequence<long> ids; };")
        .expect("string+seq must compile");
}

#[test]
fn compiles_module_nesting() {
    check_compiles("module Outer { module Inner { struct S { long x; }; }; };")
        .expect("nested modules must compile");
}

#[test]
fn compiles_enum() {
    check_compiles("enum Color { RED, GREEN, BLUE };").expect("enum must compile");
}

/// #23-shape regression: an enum-only spec has no `TypeSupport` (only
/// struct/union get one), so the generated file must not carry an unused
/// `@zerodds/cdr` import (`Xcdr2Writer`/`Xcdr2Reader`/`DdsTopicType`/
/// `EndianMode`) or unused `@zerodds/types` extras
/// (`Char`/`WChar`/`LongDouble`/`DdsAny`/`DdsException`/`make*`). Before the
/// `runtime_import_block` usage-gate this failed `tsc --noUnusedLocals`
/// (TS6133/TS6196) even though `tsc --strict` alone passed it clean — the
/// gap `noUnusedLocals: true` on this file's tsconfig now closes.
#[test]
fn compiles_enum_only_spec_under_no_unused_locals() {
    check_compiles("enum Mode { MODE_IDLE, MODE_ACTIVE, MODE_FAULT };")
        .expect("enum-only spec must compile clean under --noUnusedLocals");
}

#[test]
fn compiles_union() {
    check_compiles(
        "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };",
    )
    .expect("union must compile");
}

#[test]
fn compiles_inheritance() {
    check_compiles("struct Base { long base_field; }; struct Child : Base { long child_field; };")
        .expect("inheritance must compile");
}

#[test]
fn compiles_amqp_helpers_struct() {
    check_compiles_with("struct Sensor { long id; double temp; };", true)
        .expect("AMQP-struct must compile");
}

#[test]
fn compiles_amqp_helpers_union() {
    check_compiles_with(
        "union U switch (long) { case 1: long a; case 2: double b; };",
        true,
    )
    .expect("AMQP-union must compile");
}

#[test]
fn compiles_struct_with_union_member_gated() {
    // A struct with a union member is gated (no TypeSupport) — must still emit
    // a compiling data type, not a reference to a non-existent union codec.
    check_compiles(
        "union Choice switch (long) { case 1: long a; case 2: double b; }; \
         struct Holder { Choice c; long n; };",
    )
    .expect("struct with union member must compile (gated)");
}

#[test]
fn compiles_struct_with_fixed_member() {
    // `fixed<P,S>` now HAS an XCDR2 codec (CORBA-BCD via the runtime
    // `writeFixedBcd`/`readFixedBcd` helpers); the struct emits a full
    // TypeSupport, not a gated data-type-only stub. The `string` field round-
    // trips the decimal. (Requires an in-sync `@zerodds/cdr` install.)
    check_compiles("struct Money { fixed<10,2> amount; long n; };")
        .expect("struct with fixed member must compile");
}

#[test]
fn compiles_struct_with_any_member_gated() {
    check_compiles("struct Bag { any value; long n; };")
        .expect("struct with any member must compile (gated)");
}

/// Welle C.2 #14 — reserved-keyword escaping. IDL identifiers that collide
/// with TS/ECMAScript reserved words (struct/interface/union/enum/bitset/
/// bitmask/typedef names, module names, const names, RPC operation
/// parameters) previously emitted syntactically invalid TypeScript
/// (`export interface class { ... }` etc.). This must now `tsc --strict`
/// clean.
#[test]
fn compiles_keyword_colliding_struct_and_field_names() {
    check_compiles("struct class { long type; long function; };")
        .expect("keyword-named struct/fields must compile");
}

#[test]
fn compiles_keyword_colliding_type_reference() {
    check_compiles("struct class { long a; }; struct Holder { class m; };")
        .expect("reference to a keyword-named type must compile");
}

#[test]
fn compiles_keyword_colliding_enum() {
    check_compiles("enum class { class, type };").expect("keyword-named enum must compile");
}

#[test]
fn compiles_keyword_colliding_union_and_module() {
    check_compiles(
        "module function { union class switch (long) { case 1: long a; case 2: double b; }; };",
    )
    .expect("keyword-named union/module must compile");
}

#[test]
fn compiles_keyword_colliding_typedef_and_const() {
    check_compiles("typedef long class; const long function = 5;")
        .expect("keyword-named typedef/const must compile");
}

// NOTE: no `interface`-construct tsc compile-check here (and none existed
// pre-existing in this file for *any* IDL interface, keyword-colliding or
// not) — the `@zerodds/types` stub's `ServiceDescriptor` above is declared
// non-generic, while real codegen emits `ServiceDescriptor<Client,
// Handler>`. That stub/codegen mismatch is a pre-existing gap in this test
// harness unrelated to keyword escaping (interface param-name escaping
// itself IS covered — see
// `keyword_interface_param_name_escapes_but_method_name_does_not` in
// `src/lib.rs`, a string-content check that doesn't depend on the stub).

#[test]
fn compiles_keyword_colliding_bitset_and_bitmask() {
    check_compiles("bitset class { bitfield<8> lo; }; bitmask function { A, B };")
        .expect("keyword-named bitset/bitmask must compile");
}

/// TS-cluster + Bug N — the committed conformance fixtures that previously
/// failed (union member, fixed arrays, enum member, the mixed combo with all of
/// them inside a module) must now generate TypeScript that type-checks.
#[test]
fn compiles_conformance_fixtures() {
    for f in ["08_arrays", "03_enums", "09_unions", "20_mixed_combo"] {
        let path = format!(
            "{}/../../tools/idlc/tests/conformance/fixtures/{f}.idl",
            env!("CARGO_MANIFEST_DIR")
        );
        let src =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read fixture {f}: {e}"));
        check_compiles(&src).unwrap_or_else(|e| panic!("fixture {f} must compile: {e}"));
    }
}

/// Assembles the exact source the CLI writes for `--ts`: the emitter output
/// followed by the shared TypeObject post-pass (`zerodds-idl-compose`'s
/// `render_ts`). The emitter alone never carried the post-pass, so the older
/// `check_compiles` tests could not see the P0-8 collision.
fn combined_ts_source(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let mut out = generate_ts_source(&ast).expect("gen");
    let blobs =
        zerodds_idl_compose::typeobject::type_object_blobs(&ast).expect("lower TypeObjects");
    out.push_str(&zerodds_idl_compose::typeobject::render_ts(&blobs));
    out
}

/// P0-8 regression: an enum and a bitmask both emit `export const <Name> = {…}`
/// in the value namespace. Before the `_TYPE_OBJECT` suffix the TypeObject
/// post-pass re-declared those exact names (`tsc` TS2451 "Cannot redeclare
/// block-scoped variable"), so the CLI's `--ts` output did not compile. The
/// combined source must now type-check, and the TypeObject constants carry the
/// suffix.
#[test]
fn compiles_enum_and_bitmask_with_typeobject_postpass() {
    let src =
        "enum NarrowEnum { A, B }; bitmask Flags { F0, F1 }; struct Point { long x; long y; };";
    let combined = combined_ts_source(src);
    // The value-namespace names the emitter owns appear exactly once each …
    assert_eq!(
        combined.matches("export const NarrowEnum = ").count(),
        1,
        "enum value export must not be re-declared by the post-pass:\n{combined}"
    );
    assert_eq!(
        combined.matches("export const Flags = ").count(),
        1,
        "bitmask value export must not be re-declared by the post-pass:\n{combined}"
    );
    // … and the TypeObject bytes live under the suffixed identifier.
    assert!(combined.contains("export const NarrowEnum_TYPE_OBJECT = new Uint8Array(["));
    assert!(combined.contains("export const Flags_TYPE_OBJECT = new Uint8Array(["));
    assert!(combined.contains("export const Point_TYPE_OBJECT = new Uint8Array(["));
    // And the combination the CLI emits must actually type-check.
    tsc_check_source(&combined).expect("combined emitter + TypeObject post-pass must compile");
}
