# `zerodds-idlc` Handbook

Operator reference for the CLI and Rust build integration in the current
source tree. This revision replaces the pre-rebuild compiler instructions.
It describes source-backed interfaces, not the availability of an RC7
release or a certification of every language, annotation and transport.

## 1 What the compiler does

`zerodds-idlc` checks, lints and formats IDL, displays its model, and generates
language declarations plus codecs. The implementation is in
[tools/idlc](../../tools/idlc/README.md). Its composition pipeline uses
[zerodds-idl-compose](../../crates/idl-compose/src/lib.rs), parsing/lowering
and resolution, then the language rules and renderers. It is not the old
Earley/`zerodds-idl-<language>` emitter architecture.

Keep four different outcomes separate:

1. The frontend accepts a schema.
2. The selected backend generates source and that source compiles with its
   matching wire runtime.
3. A codec preserves the intended values under the selected encoding.
4. A typed DDS endpoint interoperates with a particular peer.

A successful earlier step does not establish a later one. In particular,
the language list is not a promise of full typed DDS integration for each
target. C# runtime/consumer integration requires separate verification.

## 2 Installation

Use an existing, trusted ZeroDDS checkout. Run from its root:

```sh
cargo build --locked -j 1 -p zerodds-idlc
./target/debug/zerodds-idlc --version
./target/debug/zerodds-idlc --help
```

A custom Cargo target directory changes the executable path. Add
`--offline` only when the dependencies are already cached. Building with one
job bounds compiler concurrency, not the total memory of all other processes.

Optional installation from this same checkout:

```sh
cargo install --locked --path tools/idlc --jobs 1
zerodds-idlc --help
```

This handbook does not assert that this revision is already available on
crates.io, Homebrew, APT or Scoop. Distribution availability must be verified
for the eventual release separately. The remaining CLI examples assume the
matching executable is on `PATH`, or that its full path replaces
`zerodds-idlc`.

## 3 CLI reference

The [argument parser](../../tools/idlc/src/main.rs) is the authority.
The [manpage](../../man/man1/zerodds-idlc.1) is a second local reference.

```text
zerodds-idlc check      [OPTIONS] FILE...
zerodds-idlc lint       [OPTIONS] FILE...
zerodds-idlc format     [OPTIONS] FILE...
zerodds-idlc dump-ast   [OPTIONS] FILE...
zerodds-idlc print-deps [OPTIONS] FILE...
zerodds-idlc generate --lang LANGUAGE [OPTIONS] FILE...
```

The subcommand comes first. One or more input files are accepted, but each
is processed as a separate unit. There is no flat generation form.

| Option | Scope and meaning |
|---|---|
| `--strict` | All commands: reject vendor extensions, using the OMG IDL 4.2 dialect |
| `--json` | All commands: request machine-readable frontend/lint diagnostics |
| `--deny-warnings` | Make reported warnings fail the command |
| `--default-extensibility final\|appendable\|mutable` | Default for unannotated aggregates; normally appendable, explicit annotations win |
| `--check` | `format` only: do not rewrite, fail if input is not canonical |
| `--write` | `format` only: rewrite non-canonical input in place |
| `--lang NAME` | `generate` only, required: one target per invocation |
| `--out DIR` | `generate` only: output directory, default current directory |
| `--encoding ENC` | `generate` only: encoding version, described below |
| `--endian le\|be` | `generate` only: planned byte order, default little-endian |
| `--xcdr-endianness BE\|LE` | Alternate spelling of `--endian` |
| `-h`, `--help` | Help |
| `-V`, `--version` | Version |

Write option values as separate arguments. `format --check` and
`format --write` are mutually exclusive. `--json` is not a promise that
every output stream contains only JSON: model views, formatted text,
generation errors and I/O failures have their own output paths.

Without `--strict`, the frontend enables its vendor-extension dialect set.
Strict acceptance is a syntax/model result, not proof that another vendor
accepts the input or that generated endpoints interoperate.

### Check, lint and inspect

From the ZeroDDS root, using the checked-in companion schema:

```sh
zerodds-idlc check zerodds-examples/idlc-buildrs/idl/Robot.idl
zerodds-idlc lint zerodds-examples/idlc-buildrs/idl/Robot.idl
zerodds-idlc dump-ast zerodds-examples/idlc-buildrs/idl/Robot.idl
zerodds-idlc print-deps zerodds-examples/idlc-buildrs/idl/Robot.idl
```

`check` runs composition and reports frontend diagnostics. `lint` first
reports those diagnostics and, if there are no errors, applies the
[model lint rules](../../tools/idlc/src/lint.rs). These include wide mutable
structures, target-language keyword collisions, a topic with no key,
unbounded collections/keys and unused typedefs. They are not a two-schema
evolution comparison or a live QoS compatibility test.

`print-deps` reports the types each declaration names, not a Make depfile or
an include-file dependency list. `dump-ast` prints the resolved model view.
Neither is a `dump-typeobject` command.

The similarly named `zerodds-lint` is a separate
[internal Rust workspace checker](../../crates/lint/README.md), not an IDL
analyzer.

### Format

```sh
zerodds-idlc format --check zerodds-examples/idlc-buildrs/idl/Robot.idl
```

Exit 1 can mean that this valid input needs formatting. Without `--check`
or `--write`, canonical text goes to stdout. `--write` changes the named
files, so review the diff. The formatter checks frontend validity and
preserves token spellings, comments and directives. It refuses output it
cannot preserve. Do not redirect output over the input file itself.

### Exit status

| Code | Meaning in this implementation |
|---|---|
| 0 | No errors, and no warning made fatal by `--deny-warnings` |
| 1 | Diagnostics failed, formatting check differed, generation/output writing failed, or warnings were denied |
| 2 | Invalid command line |
| 3 | At least one top-level input could not be read |

A missing included file is reported by composition and is not the same
top-level read failure. Generation is not transactional across files: a
failure can leave earlier outputs in place. Check the exit status and use
a fresh output directory before integrating generated files.

## 4 Generation and wire runtimes

The current accepted names come directly from
[`Language::ALL` and `as_str`](../../crates/lang-cst/src/common.rs):

```text
rust, c++, c, java, c#, python, typescript, go, ocaml, ada, d,
elixir, julia, lua, nim, swift, zig, node, ada83, f#, kotlin
```

These are 21 target names. Spellings are exact: use `c++`, `c#`, `f#`
and `typescript`, not old backend flags or assumed aliases. Quote names
containing punctuation in shell examples.

```sh
idl_output="$(mktemp -d)"
zerodds-idlc generate --lang rust --out "$idl_output/rust" \
  zerodds-examples/idlc-buildrs/idl/Robot.idl
zerodds-idlc generate --lang c --out "$idl_output/c" \
  zerodds-examples/idlc-buildrs/idl/Robot.idl
zerodds-idlc generate --lang 'c#' --out "$idl_output/csharp" \
  zerodds-examples/idlc-buildrs/idl/Robot.idl
```

The output is source, not a built application. Use the matching runtime
from the same candidate revision. Rust calls `zerodds-wire`; the C header
includes `zerodds_wire.h` from [runtime/c](../../runtime/c).
Other targets have their own [runtime directories](../../runtime).
Do not substitute a legacy binding runtime merely because the generated
filename looks familiar.

[Output naming](../../tools/idlc/src/generate.rs) normally uses the input
stem and target extension, such as `Robot.rs`, `Robot.h`, `Robot.cs`
or `Robot.py`. Important exceptions:

- Java codec output is split into `ZdTypes.java` and `ZdCodec.java`.
  Eligible topic aggregates additionally produce `ZdTypeSupports.java`, which
  requires the Java DDS binding. Wire-only consumers compile the first two.
- C# additionally produces `<stem>.TypeSupport.cs` for eligible topic aggregates.
  The main file uses `ZeroDds.Wire`; the companion also requires `ZeroDDS.Cdr`.
  Its explicit representation contract needs the matching native `typed_wire`
  entry points when creating DDS entities. See the [CLI binding contract](../../tools/idlc/README.md).
- Ada uses `zd_types.ads` and, where required, `zd_types.adb`.
- Ada 83 uses `zd83_types.ads` and, where required, `zd83_types.adb`.

No project scaffold is emitted. Use separate output directories when
generating independent schemas, especially for fixed-name compilation
units or inputs with identical stems. The generator creates parent
directories and overwrites matching output files, but does not remove
obsolete output left by older schemas. Preserve hand-written sources.

### Encoding is a code-generation choice

The accepted `--encoding` values are `xcdr1`, `plain-cdr1`, `pl-cdr1`,
`xcdr2`, `plain-cdr2`, `pl-cdr2` and `delimited-cdr`.
The default is XCDR2. The flag selects the version used when planning the
codec. The type's own extensibility selects framing within that version.
The XCDR2 spellings therefore do not force every aggregate to use one
uniform encapsulation kind.

```sh
zerodds-idlc generate --lang rust --encoding xcdr1 --out "$idl_output/v1" \
  zerodds-examples/idlc-buildrs/idl/Robot.idl
zerodds-idlc generate --lang rust --encoding xcdr2 --endian be \
  --out "$idl_output/v2-be" zerodds-examples/idlc-buildrs/idl/Robot.idl
```

Changing XCDR1 to XCDR2 can change emitted codec bodies. Byte order is held
by the wire reader/writer; `--endian` supplies the planned value in the
generated preamble, rather than creating a second set of byte-swapped
method bodies. Construct readers/writers consistently with the sample's
actual representation. The CLI does not create a participant, negotiate
DataRepresentation QoS, or make a generated codec automatically support
every representation.

## 5 Includes, annotations and unsupported constructs

The CLI uses the composition loader without an extra include search list.
Quoted includes resolve beside the including file, with a path from the
working directory as a fallback. The Rust build API additionally provides
explicit include directories and macro definitions.

Use include guards or `#pragma once` where appropriate. Do not treat
separate top-level CLI inputs as one shared model. For Rust inputs with
shared declarations, consider `Config::compile_bundle`.

There is no blanket annotation-support table here: syntax acceptance,
model semantics, target storage, codec behavior and discovery metadata
must each be checked for the construct and language in question.

Examples of distinctions that matter:

- Explicit final/appendable/mutable annotations override the configured
  default. Omitting an annotation is not equivalent to selecting final.
- A member cannot simultaneously be a key and optional. `check` reports
  such frontend errors rather than relying on a backend workaround.
- Rust external members use `Box<T>`, and optional external members use
  `Option<Box<T>>`. Regenerate and adjust constructors when migrating old
  inline-generated storage. See the
  [compiled storage regression](../../crates/zerodds-build/tests/dds_type.rs).
- Optional framing and enum width depend on representation and layout
  rules. Neither is universally a one-byte wire encoding.
- Generation can refuse a valid construct that the target cannot encode.
  A successful parse is not permission to omit an unsupported field.

For coverage, consult the actual
[codec tests](../../crates/lang-rules/tests/codec.rs),
[generated-consumer tests](../../crates/zerodds-build/tests/dds_type.rs)
and relevant measured examples. Do not infer interoperability, annotation
preservation in every TypeObject, or plugin support from a declaration
being accepted.

## 6 build.rs integration (Rust)

The [build helper](../../crates/zerodds-build/README.md) is a library
dependency, not a subprocess invocation of the CLI. The executable
[idlc-buildrs companion](../../zerodds-examples/idlc-buildrs/README.md)
contains this workflow. Its [Cargo.toml](../../zerodds-examples/idlc-buildrs/Cargo.toml)
uses local path dependencies for the current checkout, so it does not
assume that RC7 packages have already been published.

For that existing companion layout, the dependencies are:

```toml
[build-dependencies]
zerodds-build = { path = "../../crates/zerodds-build" }

[dependencies]
zerodds-wire = { path = "../../crates/wire" }
zerodds-dcps = { path = "../../crates/dcps" }
zerodds-types = { path = "../../crates/types" }
zerodds-sql-filter = { path = "../../crates/sql-filter" }
```

Adjust paths for another project. They are relative to its Cargo manifest,
not this handbook. The complete companion manifest also isolates its
workspace and declares the demo binary.

[build.rs](../../zerodds-examples/idlc-buildrs/build.rs):

```rust
fn main() -> Result<(), zerodds_build::Error> {
    zerodds_build::Config::new()
        .include_dir("idl")
        .dds_type_impls(true)
        .compile(&["idl/Robot.idl"])
}
```

[src/lib.rs](../../zerodds-examples/idlc-buildrs/src/lib.rs):

```rust
#[allow(
    clippy::too_many_lines,
    clippy::useless_conversion,
    unused_imports,
    non_snake_case
)]
mod robot {
    include!(concat!(env!("OUT_DIR"), "/Robot.rs"));
}
pub use robot::Robot;
```

Both the CLI and this helper generate declarations and codecs. The Rust
CLI path does not by itself add the `zerodds_dcps::DdsType` bridge.
`dds_type_impls(true)` asks the build helper for that additional interface,
including type metadata, key handling, filtering and representation
methods. It is off by default for codec-only consumers.

This is not a promise that every type can be encoded as XCDR1. Where XCDR1
generation is unsupported, the bridge retains refusing trait defaults
rather than relabeling XCDR2 bytes as XCDR1. A TypeObject returned by a
local method is also not proof of an end-to-end TypeLookup exchange.

Generated inherent `encode`/`decode` methods operate on wire
writers/readers. When calling the trait's byte-buffer interface, disambiguate:

```rust
let mut bytes = Vec::new();
<Robot::Pose as zerodds_dcps::DdsType>::encode(&pose, &mut bytes)?;
let decoded = <Robot::Pose as zerodds_dcps::DdsType>::decode(&bytes)?;
```

This last fragment belongs inside a fallible function with a `pose`
value already constructed. The companion's complete source contains the
actual value and assertions.

### Configuration and rebuilds

| Build-helper method | Purpose |
|---|---|
| `include_dir` | Add an include search directory |
| `define` | Define a preprocessing macro |
| `out_dir` | Override the default Cargo `OUT_DIR` |
| `default_extensibility` | Select the default for unannotated aggregates |
| `typeobject` | Control standalone raw TypeObject constants, default on |
| `type_identifier_constants` | Request identifier constants, default off |
| `dds_type_impls` | Request the Rust DDS bridge, default off |
| `header_comment` | Set the generated header comment |
| `emit_cargo_directives` | Control Cargo rebuild directives, default on |

These are a typed build API, not one-for-one CLI flags. In particular,
the current CLI does not expose `-I`, `-D` or the build helper's metadata
switches. The helper reports top-level and included files through
`cargo:rerun-if-changed`. `compile` emits per input;
`compile_bundle(idls, name)` composes shared inputs into one Rust module.

The [zerodds-build-sample companion](../../zerodds-examples/zerodds-build-sample/README.md)
adds a quoted include and vendor key pragma. Both companions use the
library API and need no `zerodds-idlc` executable on `PATH`.

## 7 Executable companions and verification boundaries

Run these commands from the ZeroDDS root with the Examples checkout nested
at `zerodds-examples/`:

```sh
cargo test -j 1 --manifest-path zerodds-examples/idlc-buildrs/Cargo.toml
cargo run -j 1 --manifest-path zerodds-examples/idlc-buildrs/Cargo.toml \
  --bin idlc-buildrs-demo
cargo test -j 1 --manifest-path zerodds-examples/zerodds-build-sample/Cargo.toml
cargo run -j 1 --manifest-path zerodds-examples/zerodds-build-sample/Cargo.toml \
  --bin zerodds-build-sample-demo
```

The source tests assert value round-trips, type names, key presence and
nonempty TypeObjects. They do not run a cross-vendor DDS network exchange.

Other relevant executable material:

- [Java combo README](../../zerodds-examples/idl-conformance/java/README.md)
  and [App.java](../../zerodds-examples/idl-conformance/java/App.java):
  fresh three-file output, explicit generated descriptor, matching Java wire
  runtime and in-process DDS API loopback. See the
  [Java integration guide](../05-integration/java.md) for packaging and boundaries.
- [C# combo README](../../zerodds-examples/idl-conformance/csharp/README.md)
  and [Program.cs](../../zerodds-examples/idl-conformance/csharp/Program.cs):
  fresh codec/companion output connected to the typed native DDS API.
  Key/lifecycle and transport claims require the matching measured candidate,
  not merely a successful generated-source compile.
- [C combo README](../../zerodds-examples/idl-conformance/_interop/c/README.md)
  and [driver](../../zerodds-examples/idl-conformance/_interop/c/interop.c):
  current CLI-generated C plus a matching wire runtime, explicit framing
  defaults and malformed-input checks.
- [Rust feature harness](../../zerodds-examples/idl-conformance/_interop/rust-features/src/main.rs)
  and [build script](../../zerodds-examples/idl-conformance/_interop/rust-features/build.rs):
  feature codec round-trips including fixed-point values. Its ENCODE path
  writes payload files, so use an isolated output area when measuring.

Verification for this handbook revision: CLI and build-API statements were
checked against source, local references were checked, and companion
snippets were compared with their actual files. This documentation pass
did not rebuild or execute these command blocks. Existing measured
C/feature results are scoped evidence, not proof that every command and
language described here was rerun at the same release head.

## 8 Migrating older instructions and troubleshooting

The following old forms are not current interfaces:

- Flat `zerodds-idlc schema.idl --rust`, backend flags such as `--java`,
  `--all`, `-o`, `--output` and `--out-<language>`.
- `dump-typeobject`, `--parse-only`, `--scaffold`, `--corba` and
  `--rust-plugin`.
- Vendor selector flags `--rti`, `--cyclone`, `--opendds`, and CLI
  `--default-nested`, `--no-typeobject` or `--with-typeobject`.
- Old `CdrEncode`/`CdrDecode` assumptions and the statement that CLI
  generation is types-only.

Do not mechanically replace a flag and keep an old runtime shim. Regenerate
into a fresh directory, identify the runtime the source imports, adapt the
consumer, then compile and test its values and bytes.

For a missing include, check the including file's directory and the build
API's search list. For a type-name collision, inspect the frontend diagnostic
and qualify the type where required instead of renaming wire-visible members
without reviewing the effect. Whitespace before a string bound is not by
itself an error. For an output failure, inspect permissions and the actual
I/O diagnostic; parent directories are created recursively.

For different peer bytes, compare the full schema, defaults, extensibility,
encoding version, byte order, encapsulation and sample value. Equal values
and identical bytes are different results. Preserve both and the exact
compiler/runtime identities. Do not conclude that runtime negotiation must
have selected the right codec merely because generation succeeded.
