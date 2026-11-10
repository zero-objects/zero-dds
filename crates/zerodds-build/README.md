# `zerodds-build`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-build/badge.svg)](https://docs.rs/zerodds-build)

A [`prost-build`](https://docs.rs/prost-build)-style `build.rs` helper for
[ZeroDDS](https://zerodds.org) IDL: `Config::new().include_dir(...).compile(&["x.idl"])`.

## Layer position

Layer 3 (schema), build-time-only dependency (put it under
`[build-dependencies]`, not `[dependencies]`). Depends on
`zerodds-idl-compose` for the composition pipeline and on
`zerodds-lang-rules` + `zerodds-lang-render` for the emission.

## Why this exists

`zerodds-idlc generate --lang rust` emits data types and codecs. This
crate integrates generation into Cargo and, on request, adds the `DdsType`
bridge. The composition in front of both is more than a parse: it preprocesses
`#include`/`#define`/`#ifdef`/`#pragma`, applies vendor key-pragmas
(`#pragma keylist` / `DCPS_DATA_KEY` / `cats`), patches default
extensibility/nestedness, and lowers XTypes TypeObjects. Before this crate
existed, five in-repo `build.rs` scripts (`crates/spatial-dds`,
`crates/spatial-ros2`, `crates/corba-interop`, two `zerodds-examples/*`)
called the low-level parse and emit directly and silently skipped all four
of those steps — no `#include` dependency tracking (editing an included
file did not trigger a rebuild), no vendor key-pragma support, no
default-extensibility, no TypeObject block. `zerodds-build` runs the same
composition library the CLI uses (`zerodds-idl-compose`), with a typed
configuration surface for build-time consumers. This is shared machinery,
not a claim that every CLI option has an identical build-script method.

## Usage

```toml
[build-dependencies]
zerodds-build = "1.0.0-rc.7"
```

```rust,no_run
// build.rs
fn main() -> Result<(), zerodds_build::Error> {
    zerodds_build::Config::new()
        .include_dir("idl")
        .compile(&["idl/Robot.idl"])
}
```

```rust,ignore
// src/lib.rs
#[allow(non_snake_case, unused_imports)]
mod robot {
    include!(concat!(env!("OUT_DIR"), "/Robot.rs"));
}
```

`include!` splices the generated tokens into the *including* file, so
any lint allow the generated code needs goes on the wrapping module as an
outer attribute (as above).

## Topic types

The generated type carries a codec — a pair of inherent functions over
`zerodds_wire::Writer`/`Reader` — but not the `zerodds_dcps::DdsType`
trait that `DomainParticipant::create_topic` takes. `dds_type_impls(true)`
writes that impl too:

```rust,no_run
// build.rs
fn main() -> Result<(), zerodds_build::Error> {
    zerodds_build::Config::new()
        .dds_type_impls(true)
        .compile(&["idl/Robot.idl"])
}
```

```rust,ignore
participant.create_topic::<Robot>("entity.robot")?;
```

The impl carries `TYPE_NAME` from the IDL scoped name, `EXTENSIBILITY`
from the annotation, `HAS_KEY` and the PLAIN_CDR2-BE key holder from the
`@key` members, the complete `TypeIdentifier` and `TypeObject` from the
same computation discovery uses, `field_value` for the SQL filter, and
all four codec pairs — XCDR2 and XCDR1, little-endian and big-endian —
from the same emitter under four layout keys.

It is **off by default**: it puts `zerodds-dcps`, `zerodds-types` and
`zerodds-wire` into the consumer's `[dependencies]`, which is the whole
DCPS runtime, and this crate is also used by consumers that want the
codec and no DDS in the graph at all.

An IDL file with a `long double` needs `zerodds-float128` in the
consumer's `[dependencies]` as well: the generated field is a
`zerodds_float128::Float128`, which has no Rust primitive to stand for it.

## Names

The generated Rust takes the IDL's names. Where the IDL name cannot stand
in Rust as written, it is spelled apart; the IDL name stays in the
TypeObject, on the wire and in a content filter:

- a Rust keyword is a raw identifier: `type` is `r#type`, `gen` is `r#gen`;
- the four keywords with no raw form take a trailing `_`: `crate_`,
  `self_`, `super_`, `Self_`;
- a type, module or constant named like an item the generated code uses
  without a path takes a trailing `_`: `Option_`, `Vec_`, `Result_`,
  `Reader_`, `Writer_`, and the others in `zerodds_lang_cst::rust::escape_type_name`.

`strip_enum_prefix(true)` cuts each enumeration's name off its
enumerators' Rust names: `enum MyEnum { MY_ENUM_A, MY_ENUM_B };` has the
variants `A` and `B` instead of `MY_ENUM_A` and `MY_ENUM_B`. The name
matches without regard to case and underscores and ends at an `_` or a
change from lower to upper case; it is cut only where a valid identifier is
left (`MY_ENUM_1` stays), and two enumerators that come out as one name stop
the build. It is off by default.

Where XCDR1 cannot frame a type, the four XCDR1 methods are **left out**
and the trait's own refusing defaults stand, with a comment in the
generated file naming the type. They are not delegated to the XCDR2
codec: that would put version-2 octets under a `CDR_LE`/`CDR_BE`
encapsulation header, which no peer can tell from a sample it is meant
to read.

Every `.idl` path passed to `compile()`, plus every file it transitively
`#include`s, is reported via `cargo:rerun-if-changed` — editing an included
`.idl` triggers a rebuild, not just editing the top-level file.

## A Rust type for an IDL type

An IDL `typedef string UtcTimestamp;` is a `String` in the generated
Rust, and so is a text that is no timestamp. `rust_type` names a Rust
type that stands for a typedef, a struct member or a union branch, and the
generated code holds that type instead (GitHub #33):

```rust,no_run
// build.rs
fn main() -> Result<(), zerodds_build::Error> {
    zerodds_build::Config::new()
        .rust_type("app::UtcTimestamp", "crate::time::UtcTimestamp")
        .rust_type("app::Track.id", "core::num::NonZeroU32")
        .rust_type_with("app::Track.source", "std::net::Ipv4Addr", "zerodds_wire::DisplayFromStr")
        .dds_type_impls(true)
        .compile(&["idl/app.idl"])
}
```

- The path is the IDL scoped name: a typedef (`app::UtcTimestamp`), a
  struct member (`app::Track.id`) or a union branch (`app::Choice.stamp`).
  A member mapping wins over a typedef mapping of the member's type.
- The Rust type implements `zerodds_wire::WireRepr`, whose `Idl` is the
  IDL type's Rust spelling: `str` for a `string`, `u32` for an `unsigned
  long`. `zerodds-wire` carries the impls for `core::num::NonZero*`,
  `String` and `Cow<'_, str>`; a type from another crate goes through an
  adapter (`rust_type_with`), `zerodds_wire::DisplayFromStr` for a
  `Display + FromStr` type on a `string`. The IDL type is a `string` or a
  primitive other than `wchar` and `long double`.
- The generated type holds the Rust type wherever the declaration is used:
  a member, a sequence, array or map element, a union branch, `Option` for
  `@optional`, `Box` for `@external`. A typedef is an alias of it.
- Encode writes the IDL value through `to_wire`; decode reads it — a
  `string` lent out of the sample, not copied — and converts through
  `from_wire`. A value the Rust type refuses rejects the sample with that
  reason.
- The octets, the `TypeObject`, the `TypeIdentifier`, assignability and
  the key hash are the IDL's. A C or C++ peer built from the same IDL
  reads the same samples.
- A path that names no typedef, struct member or union branch of the
  compiled units, a declaration of another IDL type and a Rust type that
  is no type path fail the build with `Error::RustType`. A Rust type whose
  `Idl` is not the IDL type fails to compile: the generated file states
  every mapping as `zerodds_wire::repr::idl_type_is::<T, Idl>()`.
- The generated types derive `Debug`, `Clone` and `PartialEq`, so the
  Rust type implements them. A type with a member whose Rust type refuses
  the IDL default (a `NonZeroU32` for Table 9's zero) gets no `impl
  Default`, and a sample that leaves such a member out — an older
  `@appendable` writer, a key-only sample — is rejected.

`examples/rust-type/` builds a date-time group and a `NonZeroU32` this way.

## `Config`

| `Config` method | what it sets |
|---|---|
| `include_dir(dir)` | a directory the preprocessor searches for `#include` |
| `define(name, value)` | a preprocessor macro, as `-D NAME[=VALUE]` would |
| `out_dir(dir)` | where the generated files go (default: `$OUT_DIR`) |
| `default_extensibility(ext)` | what an unannotated aggregate is: `Final`, `Appendable` or `Mutable` (default: `Appendable`, XTypes 1.3 §7.3.1.2.1.8) |
| `typeobject(bool)` | the `type_objects_*` module of raw `TypeObject` bytes (default: on) |
| `header_comment(text)` | the header comment in the generated file |
| `type_identifier_constants(bool)` | a `zerodds_types::TypeIdentifier` constant per type (default: off) |
| `emit_cargo_directives(bool)` | whether `compile()` prints `cargo:rerun-if-changed` (default: on) |
| `dds_type_impls(bool)` | `impl zerodds_dcps::DdsType` per type (default: off) |
| `dialect(Dialect)` | reproduce one peer's answers to every wire axis at once (default: `Dialect::default()`, the specification) |
| `axis(AxisFlag, value)` | override one answer of the dialect; several calls apply in the order given |
| `rust_type(idl_path, rust_type)` | hold a typedef, struct member or union branch as a Rust type that implements `zerodds_wire::WireRepr` |
| `rust_type_with(idl_path, rust_type, adapter)` | the same through an adapter value that implements `zerodds_wire::WireReprWith` |
| `strip_enum_prefix(bool)` | cut each enumeration's name off its enumerators' Rust names, as `zerodds-idlc --strip-enum-prefix` (default: off) |

The CLI exposes `--out` and `--default-extensibility`, as well as its own
`--lang`, `--encoding` and `--endian` options. Include directories, macro
definitions, Cargo rebuild directives and optional `DdsType` bridging are
configured through this crate's API, not corresponding CLI flags. See the
[CLI reference](../../tools/idlc/README.md).

`compile_bundle(idls, name)` composes several input files into one generated
module when they share declarations. `compile(idls)` emits per-input files.

## Dialects and axis flags

`Config::dialect` and `Config::axis` are `zerodds-idlc generate
--dialect`/`--<axis>` and `wireprobe-zerodds --dialect`/`--<axis>`, read
from the same table (`crates/ast-layout/src/axes.rs`), applied in the
same order: the dialect first, then each axis override. `docs/wire-axis-flags.md`
has every dialect's answer to every axis and the values `axis` takes.

```rust,no_run
// build.rs
fn main() -> Result<(), zerodds_build::Error> {
    zerodds_build::Config::new()
        .dialect(zerodds_ast_layout::Dialect::Cyclone)
        .axis(zerodds_ast_layout::AxisFlag::LengthCodes, "compact")
        .compile(&["idl/Robot.idl"])
}
```

Neither reaches the codec's field-by-field logic alone: `dialect` and
`axis` decide the `LayoutKey` the codec is planned under (offsets,
delimiters, length codes, PID framing, ...) and, for a source whose
`@hashid` or unannotated-scope reading a dialect settles, the same
three lowering options `zerodds-idlc` reads off `DialectLowering`. The
default — no `dialect` call, no `axis` call — is `Dialect::default()`,
the specification's clause-by-clause reading, which is what every
existing `build.rs` using this crate before this option existed already
got: adding neither leaves the generated module unchanged.

An unrecognised axis value is not a build-time panic: `compile()` and
`compile_bundle()` return `Err(Error::Axis { flag, value, detail })`,
naming which `.axis(...)` call was wrong — there is no command line
to refuse it at here, so the `Result` is where a typo in a value
string surfaces.

## External storage

Generated Rust stores `@external T` members in `Box<T>`, and
`@optional @external T` members in `Option<Box<T>>`. This makes an optional
self-reference finite without changing its wire representation. Defaults,
codecs and the supported `DdsType::field_value` paths use the same storage
mapping. Consumers of older inline-generated fields must regenerate and
adjust their constructors. The compiled storage regression is in
[the DdsType tests](tests/dds_type.rs).

## Not yet covered

Only the Rust backend is wired. The composition pipeline
(`zerodds-idl-compose`) is language-agnostic and `zerodds-lang-rules`
carries the rules for every target the CLI knows, so a `zerodds-build`-style
crate for another language would call the same `compose()` and the matching
rule set.

Rust maps and `fixed<P,S>` now have codecs. This does not imply that every
IDL construct has a Rust writer: generation returns an error for unsupported
members rather than leaving them out. The regression coverage lives in
[the codec tests](../lang-rules/tests/codec.rs) and
[compiled DdsType tests](tests/dds_type.rs).
