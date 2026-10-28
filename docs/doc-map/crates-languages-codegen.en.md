# IDL toolchain, code generation and language bindings

German original: [crates-languages-codegen.md](crates-languages-codegen.md)

This page describes the path from IDL text to source code in 21 target languages, the tools `zerodds-idlc` and `zerodds-xmlc`, the DDS language bindings under `crates/` (C ABI, C++, Python, Java, C#, TypeScript, Rust) and the native endpoint SDKs under `endpoints/`. The basis is the code on `main`. Statements that come only from comments or READMEs are marked "(per doc comment)". The flags come from `tools/idlc/src/main.rs` and from `zerodds-idlc --help`; what was run is marked as measured.

## Related pages

- [README.en.md](README.en.md)
- [architecture.en.md](architecture.en.md)
- [crates-core.en.md](crates-core.en.md)
- [crates-bridges-services.en.md](crates-bridges-services.en.md)
- [crates-languages-codegen.en.md](crates-languages-codegen.en.md)
- [config.en.md](config.en.md)
- [cli-options.en.md](cli-options.en.md)
- [tools-and-man.en.md](tools-and-man.en.md)
- [system-setup.en.md](system-setup.en.md)
- [security.en.md](security.en.md)
- [migration.en.md](migration.en.md)
- [interop-and-dialects.en.md](interop-and-dialects.en.md)
- [ros2.en.md](ros2.en.md)

---

## 1. Summary

- One codegen path for all languages: `idl-parse` → `idl-cst` → `idl-lower` → `ast` → `ast-layout` → `lang-rules` → `lang-cst` → `lang-render`. The entry point for users is `zerodds-idlc generate --lang <NAME>`, for Rust build scripts `zerodds-build`.
- `--lang` accepts exactly 21 names: `rust`, `c++`, `c`, `java`, `c#`, `python`, `typescript`, `go`, `ocaml`, `ada`, `d`, `elixir`, `julia`, `lua`, `nim`, `swift`, `zig`, `node`, `ada83`, `f#`, `kotlin` (`Language::ALL`, `crates/lang-cst/src/common.rs:76`).
- Generated code needs the thin wire runtime of its language: `runtime/<lang>/`, for Rust `crates/wire`. A DDS binding (participant, writer, reader) exists for C, C++, Python, Java, C#, TypeScript and Rust. The other languages reach DDS through the endpoint SDKs in `endpoints/<lang>/` (XRCE framing to a Rust hub, ADR 0013).
- The crates `crates/idl`, `crates/idl-java` and `crates/idl-csharp` are not Cargo packages. `crates/idl` contains only empty directories, the other two only runtime source files (`runtime/`).
- The documents use the current CLI syntax `generate --lang … --out …`. The CMake function `zerodds_idlc_generate` calls `generate --lang <backend> --out … -I … --stamp … --depfile … --output-manifest …` (`cmake/tests/run-idlc-generate.sh`: rebuild when an `#include` changes).

---

## 2. Pipeline: IDL text → source code

```text
.idl
  │  zerodds-idl-parse   preprocess (#include/#define) + recursive descent parser, Dialect
  ▼
CST (zerodds-idl-cst)    lossless, trivia kept, reproducible byte for byte
  │  zerodds-idl-lower   scopes, name binding, resolution, completion (@autoid/@id), validation
  ▼
AST (zerodds-ast over zerodds-ast-core)   source-neutral nodes + facets (XTypes, CORBA, ROS 2, XML …)
  │  zerodds-idl-compose bundles the three steps + resolve + cycle check + TypeObject registration
  │
  ├─► zerodds-types::from_ast        Minimal/Complete TypeObject, TypeIdentifier, TypeInformation
  ├─► zerodds-ast-layout             XCDR1/XCDR2 layout plan (member plans, length codes, key holder)
  ▼
zerodds-lang-rules       Ast → ModelGraph → apply(rule::catalogue(lang)) → Materialiser (adapter/)
  │                      + codec walk (emit/encode.rs, emit/decode.rs) over the layout plan
  ▼
zerodds-lang-cst         typed CSTs of the target languages (one module per language)
  │  zerodds-lang-render renders CST to text, decides nothing itself
  ▼
source text  +  preamble/companion from tools/idlc (runtime import, endianness constant, TypeSupport)
```

Evidence:

- `tools/idlc/src/main.rs:744-809` (`analyse`): calls `zerodds_idl_compose::compose_source_with` with `PathLoader::new` over the `-I` directories and `ComposeOptions { dialect, default_extensibility, … }`; without `--default-extensibility`, `default_extensibility` comes from `--dialect`, the lowering options from `DialectLowering::for_dialect`.
- `crates/idl-compose/src/lib.rs:290-346`: `preprocess_with` → `parse_preprocessed_with` → `lower_with` → `resolve` → `inadmissible_cycles` → `from_ast::register_all`.
- `crates/lang-rules/src/generate.rs:7-12` (doc comment) and `generate::source` (line 679): `Ast → ModelGraph → apply(model, rule::catalogue(language)) → Materialiser → Renderer → Text`.
- `tools/idlc/src/main.rs:1027-1041`: `run_generate` builds the `LayoutKey` from `--dialect`, `--encoding`, `--endian` and the axis flags (`dialect_settings_from_overrides`) and hands it to `generate::files_with_mapping` (`main.rs:1078`). `tools/idlc/src/generate.rs:353`: `source_with_preamble_mapping` calls `zerodds_lang_rules::generate::source_with_mapping`, for C# `csharp_source_with_runtime_and_options`, for C++ `cpp_source_with_mapping`; `source` (`generate.rs:324`) is only a wrapper for tests.
- The rule engine is `seesaw-tgg` 2.0 as a version dependency (per doc comment, `crates/lang-rules/src/lib.rs:15-19`).

### Crates of the toolchain

| Crate | Package name | Purpose (from `Cargo.toml` `description`, checked against `src/`) |
|---|---|---|
| `crates/idl-parse` | `zerodds-idl-parse` | Hand-written parser for OMG IDL 4.2, error recovery, preprocessor (`src/preprocess/`), `Dialect::STRICT` and `Dialect::ALL_VENDORS` (`src/dialect.rs:31,40`) |
| `crates/idl-cst` | `zerodds-idl-cst` | Lossless CST (green/red tree) |
| `crates/idl-lower` | `zerodds-idl-lower` | CST → AST: scopes, resolution, completion (`complete.rs`), annotations (`annotation.rs`), validation (`validate.rs`) |
| `crates/idl-compose` | `zerodds-idl-compose` | One call for all build-time users: include search paths, preprocessor, parse, lower, TypeObjects |
| `crates/ast-core` | `zerodds-ast-core` | Arenas, stable IDs, transactions, snapshots; no DDS knowledge |
| `crates/ast` | `zerodds-ast` | Shared AST with facets (`src/facet/`: `xtypes`, `corba`, `ros2`, `xml`, `flatdata`, `vendor`) |
| `crates/ast-layout` | `zerodds-ast-layout` | Layout plan XCDR1/XCDR2, `Encoding`, `Endianness`, `LayoutKey`, KeyHash |
| `crates/types` | `zerodds-types` | XTypes 1.3: TypeIdentifier, TypeObject, Assignability, DynamicType, TypeLookup; `from_ast.rs` creates TypeObjects from the AST |
| `crates/lang-cst` | `zerodds-lang-cst` | CSTs of the 21 target languages (`src/<lang>/`). The package description names only "Rust, C++, C and Java" and is out of date |
| `crates/lang-render` | `zerodds-lang-render` | CST → text, one `src/<lang>.rs` per language |
| `crates/lang-rules` | `zerodds-lang-rules` | Rules as data (`rule/`), materialiser (`adapter/`), codec emitter (`emit/`), Table 9 defaults (`default_<lang>.rs`), `generate.rs` |
| `crates/zerodds-build` | `zerodds-build` | `build.rs` helper: one `.rs` per `.idl` in `OUT_DIR`, `rerun-if-changed` also for `#include` files, optional `DdsType` impls |
| `crates/proto` | `zerodds-proto` | Protobuf `FileDescriptorSet` → XTypes `DynamicType`. Library only, no CLI, no codegen path through `lang-rules` |
| `crates/xml-compose` | `zerodds-xml-compose` | DDS-XML type definitions → shared AST (counterpart to `idl-compose`) |
| `crates/corba-codegen` | `zerodds-corba-codegen` | CORBA 3.3 Annex A.1 special types, stub/skeleton mappings over the AST |
| `crates/corba-rust` | `zerodds-corba-rust` | IDL → Rust for CORBA constructs (interfaces, valuetypes, components) |
| `crates/dlrl-codegen` | `zerodds-dlrl-codegen` | DLRL object/home classes (DDS 1.4 Annex B.4) for C++, C#, Java, TypeScript |
| `crates/flatdata-derive` | `zerodds-flatdata-derive` | `#[derive(FlatStruct)]` for zero-copy SHM, no IDL path |
| `crates/wire` | `zerodds-wire` | Runtime that generated Rust code calls (`Writer`/`Reader`, XCDR1/XCDR2) |
| `crates/wire-codec` | `zerodds-wire-codec` | Executes an `ast-layout` plan against a value, reflectively |
| `crates/idl` | – | Not a package. Only empty directories `src/grammar/` and `.DS_Store`, not under version control (`git ls-files crates/idl` empty) |
| `crates/idl-java` | – | Not a package. `runtime/`: Java annotations (`Key`, `Id`, `Optional`, `External`, `Nested`, `Extensibility`, `MustUnderstand`, `Shared`, `TopicType`) and `rpc/`. Included as a source directory by `crates/java-omgdds/java/pom.xml` |
| `crates/idl-csharp` | – | Not a package. `crates/idl-csharp/runtime/Omg.Types.cs` (sequence contracts, annotation attributes, `ITopicType<T>`). Included by `crates/cs/csharp/ZeroDDS.Cdr/ZeroDDS.Cdr.csproj` |

The former crates `zerodds-idl`, `idl-cpp`, `idl-rust`, `idl-python` and the remaining `crates/idl-<lang>` have been removed. The replacement is `idl-parse` → `idl-lower` → `lang-rules` (comment in `Cargo.toml` lines 332-335 and `crates/zerodds-c-api/Cargo.toml` `[dev-dependencies]`).

---

## 3. `zerodds-idlc`

Package `tools/idlc` (`zerodds-idlc`), binary `zerodds-idlc`, `publish = true`. Man page `man/man1/zerodds-idlc.1`.

Build:

```sh
cargo build --release -p zerodds-idlc
# Binary: target/release/zerodds-idlc
```

### Subcommands (`tools/idlc/src/main.rs:117-128`)

| Subcommand | Effect |
|---|---|
| `check` | Parse and lower, report all diagnostics |
| `lint` | Six rules for valid files (`tools/idlc/src/lint.rs`). Runs only when `check` reports no errors |
| `format` | Canonical form to stdout; `--check` reports, `--write` writes back |
| `dump-ast` | Model as a tree |
| `print-deps` | The types that each declaration names |
| `generate` | Source code for one language |

### Options (`parse_args`, `main.rs:381-735`)

| Option | Applies to | Values / default |
|---|---|---|
| `--strict` | all | OMG IDL 4.2 only (`Dialect::STRICT`); without the flag `Dialect::ALL_VENDORS` |
| `--json` | all | machine-readable diagnostics |
| `--deny-warnings` | all | exit 1 on warnings |
| `--default-extensibility <final\|appendable\|mutable>` | all | without the flag the spec default `appendable` applies; an explicit annotation in the source takes precedence |
| `--check`, `--write` | `format` only | mutually exclusive |
| `--lang <NAME>` | `generate` only, required | one of the 21 names from section 4 |
| `--out <DIR>` | `generate` only | default `.` |
| `--encoding <ENC>` | `generate` only | `xcdr1` = `plain-cdr1`, `xcdr2` = `plain-cdr2` (default), `pl-cdr1`, `pl-cdr2`, `delimited-cdr` |
| `--endian <le\|be>` | `generate` only | default `le`; case-insensitive |
| `--xcdr-endianness <BE\|LE>` | `generate` only | alternative spelling of `--endian` (DDS-TS 1.0 §C.3.1) |
| `--dialect <NAME>` | `generate` only | one of the eleven CDR dialects; default `specification` ([interop-and-dialects.en.md](interop-and-dialects.en.md), 6.3) |
| `--<axis> <VALUE>` | `generate` only | overrides one answer of the dialect; names and values in `docs/wire-axis-flags.md` |
| `-I`, `--include <DIR>` | all | search directory for a quoted `#include` after the including file's directory; repeatable |
| `--stamp <FILE>` | `generate` only | written last, only when the run succeeded |
| `--depfile <FILE>` | `generate` only | Make dependency file (target: the stamp, else the manifest) listing every IDL file read |
| `--output-manifest <FILE>` | `generate` only | JSON of the files read and written |
| `-h`, `--help` / `-V`, `--version` | – | – |

Any other option with a leading `-` is rejected with exit 2. Quoted includes are searched next to the including file, then in the `-I` directories and last relative to the working directory (`crates/idl-compose/src/lib.rs:94-105`; `-I`, `--stamp`, `--depfile`, `--output-manifest` in `main.rs:450-490`).

Exit codes: `0` nothing to report, `1` a diagnostic, or `format --check` found a file, `2` invalid command line, `3` file not readable.

Effect of `--encoding` and `--endian` (per doc comment `main.rs:50-65`, supported by test `the_encoding_flag_changes_the_emitted_codec`):

- `--encoding` changes the generated codec. It selects the XCDR version. `xcdr2`, `delimited-cdr` and `pl-cdr2` produce the same text, as do `xcdr1` and `pl-cdr1`. The encoding of each type follows from its extensibility (XTypes 1.3 §7.4.2).
- `--endian` does not change the codec. It only sets the endianness constant in the preamble. The runtimes carry the byte order in `Writer`/`Reader`.

### Output files (`tools/idlc/src/generate.rs:93-221`)

Without a special case: `<out>/<stem>.<ext>`, `<stem>` = file name of the IDL without extension. Extensions from `Language::extension()`.

| Language | Files |
|---|---|
| Java | `ZdTypes.java`, `ZdCodec.java`, plus `ZdTypeSupports.java` when topic-capable aggregates are present |
| C# | `<stem>.cs`, plus `<stem>.TypeSupport.cs` for topic-capable aggregates |
| Ada | `zd_types.ads`, `zd_types.adb` (no body without a codec) |
| Ada 83 | `zd83_types.ads`, `zd83_types.adb` |
| Python | `<stem>.py`, with `ZD_TOPIC_DESCRIPTORS` at the end (`python_support.rs`) |
| C++ | `<stem>.hpp`, at the end `dds::topic::topic_type_support<T>` specialisations for every non-`@nested` aggregate (`topic_traits`, `generate.rs:927`) |
| all others | one file `<stem>.<ext>` |

The Java and Ada file names do not depend on the stem. A second IDL file into the same `--out` is refused there with `error: … was already written for an earlier input in this run`, exit 1 (measured with `--lang java` and `--lang ada`; test `two_inputs_writing_one_output_file_are_refused` in `tools/idlc/tests/build_contract.rs`).

Tests: `tools/idlc/src/main.rs` (`mod tests`, among others `every_language_emits_a_codec_beside_its_declarations`, `a_model_with_a_codec_gets_its_runtime_preamble`), `tools/idlc/tests/{annotation_arguments,corpus,default_extensibility,strict}.rs`, consumer fixtures `tools/idlc/tests/{csharp-support,java-support,java-typelookup,python-support}/`.

---

## 4. Target languages of `generate`

Source: `Language::ALL`, `Language::as_str`, `Language::extension` (`crates/lang-cst/src/common.rs:76-150`); preambles from `tools/idlc/src/generate.rs:605-884`.

| `--lang` | Extension | Runtime import in the generated file | Runtime in the repo |
|---|---|---|---|
| `rust` | `.rs` | `use zerodds_wire::{…};` | `crates/wire` (`zerodds-wire`) |
| `c++` | `.hpp` | `#include "zerodds/wire.hpp"`, for topic types additionally `dds/topic/TopicTraits.hpp`, `dds/topic/xcdr2.hpp` | `runtime/cpp/include/zerodds/wire.hpp`; traits in `crates/cpp/include/dds/topic/` |
| `c` | `.h` | `#include "zerodds_wire.h"` | `runtime/c/include/zerodds_wire.h` |
| `java` | `.java` | `import org.zerodds.wire.*;` | `runtime/java` |
| `c#` | `.cs` | `global::ZeroDds.Wire` (fully qualified) | `runtime/csharp/ZeroDds.Wire.csproj` |
| `python` | `.py` | `from zerodds_wire import …` | `runtime/python` (package `zerodds-wire`) |
| `typescript` | `.ts` | `from "@zerodds/wire"` | `runtime/typescript` (package `@zerodds/wire`) |
| `go` | `.go` | `import "zdwire"`, package `zdgen` | `runtime/go` |
| `ocaml` | `.ml` | module `Zdwire` without import | `runtime/ocaml/zdwire.ml` |
| `ada` | `.ads`/`.adb` | `with Zd_Wire;` | `runtime/ada` (`zd_wire.gpr`) |
| `d` | `.d` | `import zdwire;` | `runtime/d/zdwire.d` |
| `elixir` | `.ex` | `ZeroddsWire` fully qualified | `runtime/elixir` |
| `julia` | `.jl` | `using .ZeroddsWire` | `runtime/julia/zerodds_wire.jl` |
| `lua` | `.lua` | `require("zerodds_wire")`, file ends with `return ZD` | `runtime/lua/zerodds_wire` |
| `nim` | `.nim` | `import zdwire` | `runtime/nim/zdwire.nim` |
| `swift` | `.swift` | `import ZdWire` | `runtime/swift` |
| `zig` | `.zig` | `@import("zdwire.zig")` | `runtime/zig/zdwire.zig` |
| `node` | `.js` | `require("./zdwire.js")` | `runtime/node/zdwire.js` |
| `ada83` | `.ads`/`.adb` | `with Zd_Wire83;` | `runtime/ada-83` (`zd_wire83.gpr`) |
| `f#` | `.fs` | `ZdWire` fully qualified, `module rec ZdGen` | `runtime/fsharp/ZdWire.fs` |
| `kotlin` | `.kt` | `import zerodds.wire.*` | `runtime/kotlin/ZdWire.kt` |

Notes:

- `c++`, `c#` and `f#` contain special characters. Quote them in the shell: `--lang 'c++'`, `--lang 'c#'`, `--lang 'f#'`.
- A model without a codec (for example an empty module) gets no runtime import (test `a_model_without_a_codec_gets_no_runtime_preamble`).
- Measured state per language (octet comparisons, known gaps in C, C++, F#, Kotlin; Ada, Ada 83, D not measured on macOS): `docs/thin-frontends.md`. History of the language columns: `docs/language-columns-status.md`.
- Codegen tests per language: `crates/lang-rules/tests/<lang>_end_to_end.rs`, `<lang>_codec.rs`, `<lang>_corpus_compiles.rs` (for `rust`, `cpp`, `java`, `csharp`, `python`, `typescript`).

---

## 5. Supported IDL constructs and annotations

Checked in the model. Whether each language maps each case is decided by its materialiser. A case that cannot be mapped yields `GenerateError::NoForm` or `GenerateError::Codec` and thus an error, not a silent omission (`crates/lang-rules/src/generate.rs:42-55`; test `crates/lang-rules/tests/kotlin_and_node_refuse_rather_than_substitute.rs`).

Type kinds (`TypeKind`, `crates/ast/src/node/type_.rs:203-360`):

- `Primitive`, `Fixed { digits, scale }` (`fixed<P,S>`), `String` (`string`/`wstring`, optionally bounded), `Sequence` (optionally bounded), `Array` (multidimensional), `Map` (`map<K,V>`/`map<K,V,N>`)
- `Structure` (single inheritance), `Union` (with `default:`), `Enumeration`, `Bitmask`, `Bitset` (with inheritance), `Alias` (`typedef`)
- `Valuetype`, `ValueBox`, interfaces, exceptions, `AnnotationType`
- `long double` is carried in 16 bytes via `crates/float128` (package description).

Annotations in the model (`XTypesFacet`, `crates/ast/src/facet/xtypes.rs:243-431`):

- Extensibility: `@final`, `@appendable`, `@mutable` (field `extensibility`), default controllable with `--default-extensibility`
- `@key` (also on the union discriminator), `@optional`, `@external`, `@must_understand`, `@non_serialized`, `@nested`, `@default_nested`
- `@id`, `@autoid`, `@hashid` (via `MemberIdAssignment`, `AutoId`)
- `@bit_bound`, `@default`, `@default_literal`, `@range`/`@min`/`@max`, `@unit`
- `@try_construct` (on the member, on the collection element, on the map key, on the discriminator)
- `@verbatim`, `@topic`, `@data_representation`, `@ignore_literal_names`
- Vendor key pragmas become `@key` during lowering (per doc comment, `crates/idl-compose/src/lib.rs:23`)

Validation in the front end:

- Recursion is only permitted through `sequence`, `map` or `@optional` (IDL 4.2 §7.4.1.4.4.4.4, `inadmissible_cycles` in `crates/idl-compose/src/lib.rs`).
- `generate` aborts on any parse or lowering error before anything is written (`run_generate`, `main.rs:969-979`).

---

## 6. `zerodds-xmlc`

Package `tools/xmlc` (`zerodds-xmlc`), only dependency `crates/xml`. Man page `man/man1/zerodds-xmlc.1`. The tool generates no code and opens no socket.

| Subcommand | Effect |
|---|---|
| `validate FILE...` | Parses all five DDS-XML libraries (types, QoS, domain, participant, application), reports every error |
| `types FILE...` | Lists the `<types>` definitions per library |
| `render FILE...` | Prints the deployment: domains, participants, endpoints |

Options: `-h`/`--help`, `-V`/`--version` (`tools/xmlc/src/main.rs:60-67`); `--skip-unknown` leaves unknown elements out with a path message, `--vendor-forms` reads vendor forms (`main.rs:109-112`). Any other option yields exit 2 (`tools/xmlc/src/main.rs:101-118`). Exit codes as for `zerodds-idlc`.

For DDS-XML types as a codegen source there is `crates/xml-compose` (DDS-XML → AST). No CLI path from XML to source code was found.

---

## 7. DDS language bindings under `crates/`

### 7.1 `crates/zerodds-c-api` (C ABI)

- Package `zerodds-c-api`, library name `zerodds`, `crate-type = ["cdylib", "staticlib", "rlib"]`. Artefact `target/<profile>/libzerodds.{so,dylib,a}`, on Windows `zerodds.dll` (CMake).
- Header: `crates/zerodds-c-api/include/zerodds.h`, generated by `build.rs` via cbindgen (`cbindgen.toml`). Second header: `include/zerodds_xcdr2.h`.
- 259 exported `extern "C"` functions (counted in `src/`). ABI snapshot: `crates/zerodds-c-api/tests/abi.snapshot.json`, checked by `crates/zerodds-c-api/tests/abi_compat.rs`.
- Samples are byte-based: `write` takes bytes that are already CDR-encoded, `take` returns wire bytes, release with `zerodds_buffer_free` (per doc comment `src/lib.rs:15-45`, checked against the signatures). Type identity via the `*_typed` variants with a serialised COMPLETE `TypeObject`.
- Features (`Cargo.toml`): `std`, `same-host-shm`, `flatdata-loan` (default), `delivery-iceoryx`, `security`, `fips`, `wolfcrypt`.

Entry functions (checked in `src/lib.rs`, `src/security_ffi.rs`):

| Function | Purpose |
|---|---|
| `zerodds_runtime_create(domain_id)` | Runtime with default configuration |
| `zerodds_runtime_create_from_config(domain_id, config_path, sets, n_sets)` | Configuration like `--config PATH` and `--set PATH=VALUE`; `config_path = NULL` = standard system layout; `"+path=value"` appends to lists (`lib.rs:1221`) |
| `zerodds_runtime_create_ros_defaults(domain_id)` | ROS 2 profile, offers XCDR1 and XCDR2 |
| `zerodds_runtime_create_secure(…)`, `zerodds_runtime_create_secure_from_env(…)` | DDS Security, only with feature `security` |
| `zerodds_runtime_destroy` | Release |

Further groups: DomainParticipantFactory (`zerodds_dpf_*`), participant (`zerodds_dp_*`), publisher/subscriber (`zerodds_pub_*`, `zerodds_sub_*`), DataWriter/DataReader (`zerodds_dw_*`, `zerodds_dr_*`, simplified `zerodds_writer_*`, `zerodds_reader_*`), QoS getters/setters, listeners, WaitSet/conditions, ContentFilteredTopic (`zerodds_dp_create_contentfilteredtopic`), builtin topics, loans and SHM (`zerodds_dw_loan_message`, `zerodds_dr_take_shm`), iceoryx, XCDR2 helpers (`zerodds_xcdr2_encode`/`_decode`).

Example: `crates/zerodds-c-api/examples/c_smoke.c`, build and run with `crates/zerodds-c-api/examples/build_c_smoke.sh`:

```sh
cargo build --release -p zerodds-c-api
gcc -O2 -I crates/zerodds-c-api/include -o /tmp/zerodds_c_smoke \
    crates/zerodds-c-api/examples/c_smoke.c \
    -L target/release -lzerodds -lpthread -ldl -lm
# macOS additionally: -framework CoreFoundation -framework Security
```

Tests: `crates/zerodds-c-api/tests/` (`smoke_ffi.rs`, `abi_compat.rs`, `security_ffi_e2e.rs`, `shm_loan_e2e.rs`, `waitset_event_driven.rs`, `xcdr2_c_codegen.rs`, `xcdr2_c_compile.rs`, `xcdr2_wire_vectors.rs` and others).

CMake package: the root `CMakeLists.txt` builds `zerodds-c-api` via cargo and installs `zerodds::zerodds-cpp`, pkg-config `cmake/zerodds.pc.in`, `cmake/zeroddsConfig.cmake.in`. Options `ZERODDS_BUILD_CPP`, `ZERODDS_BUILD_IDLC`, `ZERODDS_BUILD_EXAMPLES`, `ZERODDS_BUILD_TESTS`, `ZERODDS_SANITIZER`, `ZERODDS_CARGO_FEATURES`, `BUILD_SHARED_LIBS`.

`crates/sys` (`zerodds-sys`, "Stable C-ABI, basis for non-Rust bindings") contains 50 lines without an `extern "C"` export. The C ABI lives entirely in `zerodds-c-api`.

### 7.2 `crates/cpp` (C++17)

- Package `zerodds-cpp`, `publish = false`, header-only. CMake target `zerodds::zerodds-cpp` (`crates/cpp/CMakeLists.txt`, INTERFACE library, `CMAKE_CXX_STANDARD 17`).
- Two API levels:
  - `include/zerodds/dds.hpp`: RAII over `zerodds.h`. `zerodds::Runtime(domain_id)`, `zerodds::Writer`, `zerodds::Reader` (bytes), `zerodds::TypedWriter<T>(rt, topic_name, reliable)`, `zerodds::TypedReader<T>(rt, topic_name, reliable)`. `TypedWriter<T>` requires `dds::topic::topic_type_support<T>`, which `zerodds-idlc generate --lang 'c++'` generates.
  - `include/dds/…`: DDS-PSM-Cxx form (`dds/domain/DomainParticipant.hpp`, `dds/pub/Publisher.hpp`, `dds/sub/Subscriber.hpp`, `dds/topic/Topic.hpp`, `ContentFilteredTopic.hpp`, `BuiltinTopic.hpp`, `dds/core/policy/CorePolicy.hpp`, conditions, listeners).
- Links against `libzerodds` from `zerodds-c-api`.
- Example: `crates/cpp/examples/cpp_smoke.cpp`, build with `crates/cpp/examples/build_cpp_smoke.sh` (`g++ -std=c++17 -I crates/zerodds-c-api/include -I crates/cpp/include … -lzerodds`).
- Tests: `crates/cpp/tests/cpp_smoke_via_cargo.rs`, `crates/cpp/tests/smoke_dds_psm.cpp`; codegen side `crates/lang-rules/tests/cpp_corpus_compiles.rs`, `cpp_reader_generates.rs`; CMake consumers `cmake/tests/`.

### 7.3 `crates/py` (Python, PyO3)

- Cargo package `zerodds-py`, PyPI name `zerodds`, build backend maturin (`crates/py/pyproject.toml`). Extension module `zerodds._core`, Python sources in `crates/py/python/zerodds/`. `requires-python >= 3.9`. Runtime dependency `zerodds-wire==1.0.0rc7` (`runtime/python`).
- Without feature `extension-module` the crate is a placeholder library, so that `cargo build --workspace` runs without Python headers (comment in `Cargo.toml`, feature definition checked).
- Build:

  ```sh
  pip install maturin pytest
  pip install ./runtime/python
  cd crates/py && maturin develop --features extension-module
  ```

- API: `DomainParticipantFactory.instance()`, `create_participant`, bytes topics (`create_bytes_topic`, `create_bytes_writer`, `create_bytes_reader`), QoS classes, listeners, conditions, `aio` (asyncio). Typed topics via `IdlTopic(participant, name, descriptor)` with a descriptor from `ZD_TOPIC_DESCRIPTORS` of the file generated with `--lang python` (`python/zerodds/topic.py`, `IdlTopic.__init__` calls `participant.create_descriptor_topic`). Encoding is only the generated code over `runtime/python` (`zerodds_wire`).
- Additionally `python/zerodds/loader.py`: ctypes loader directly against `libzerodds`, without PyO3.
- Examples: `crates/py/examples/01_bytes_pubsub.py`, `02_shape_pubsub.py`, `03_idl_topic.py` (with `03_idl_topic.idl` and the generated module).
- Tests: `pytest crates/py/python/tests/`; `crates/py/tests/generated_descriptor_live.py` (two processes, generated types), `crates/py/tests/installed_wheel_consumer.sh`.

### 7.4 `crates/java-omgdds` (Java, pure Java)

- Not a Cargo package. Maven project `crates/java-omgdds/java/pom.xml`, `groupId org.zerodds`, `artifactId omgdds`, version `1.0.0-rc.7`, `maven.compiler.source 8`.
- Implements the OMG DDS Java PSM (`org.omg.dds.*`, 70 source files under `src/main/java`) with its own Java RTPS stack (`org.zerodds.rtps`), without JNI and without a Rust library (per README; no JNI code found).
- Includes `crates/idl-java/runtime` and `runtime/java/src/main/java` as source directories (`pom.xml`, build-helper-maven-plugin).
- Build: `mvn -f crates/java-omgdds/java/pom.xml package`.
- Generated types: `zerodds-idlc generate --lang java` → `ZdTypes.java`, `ZdCodec.java`, `ZdTypeSupports.java`. `ZdTypeSupports` provides `TopicTypeSupport<T>` instances for the writers/readers of this binding (per `tools/idlc/README.md`).
- Example: `crates/java-omgdds/java/examples/TopicTypedSmoke.java` (hand-written `TopicTypeSupport`, wire level only).
- Tests: `crates/java-omgdds/java/src/test/java` (15 files), `tools/idlc/tests/java-support/`, `tools/idlc/tests/java-typelookup/`, `crates/java-omgdds/run_grpc_bridge_e2e.sh`.

### 7.5 `crates/cs` (C#, .NET 8)

- Not a Cargo package. .NET projects under `crates/cs/csharp/`:
  - `ZeroDDS` (PackageId `ZeroDDS`, `net8.0`): DDS API via P/Invoke against `zerodds` (`src/Native.cs`, `LibName = "zerodds"`), files `Domain.cs`, `Pub.cs`, `Sub.cs`, `Topic.cs`, `Qos.cs`, `Listener.cs`, `Conditions.cs`, `Cft.cs`, `Async.cs`, `Typed.cs`.
  - `ZeroDDS.Cdr` (PackageId `ZeroDDS.Cdr`): `Xcdr2Writer`, `Xcdr2Reader`, `IDdsTopicType<T>`, `IGeneratedWireTypeSupport`; includes `crates/idl-csharp/runtime/Omg.Types.cs`.
  - `ZeroDDS.Cdr.SourceGenerators` (`netstandard2.0`, analyzer).
- Generated code: `<stem>.cs` needs only `ZeroDds.Wire` (`runtime/csharp`); `<stem>.TypeSupport.cs` additionally needs `ZeroDDS.Cdr` and, for DDS entities, `ZeroDDS` (per `tools/idlc/README.md`).
- Build/run: `crates/cs/csharp/build_cs_smoke.sh` (builds `zerodds-c-api`, then `dotnet build ZeroDDS.Tests`, places `libzerodds` next to the DLL).
- Example: `crates/cs/Examples/TopicTypedSmoke.cs` (`dotnet run --project TopicTypedSmoke.csproj`).
- Tests: `crates/cs/csharp/ZeroDDS.Tests/`, `crates/cs/csharp/ZeroDDS.Cdr.Tests/`, `tools/idlc/tests/csharp-support/`, `crates/lang-rules/tests/csharp_corpus_compiles.rs`, `csharp_consumer_assembly.rs`.
- `crates/dotnet-build-lock` (`zerodds-dotnet-build-lock`, `publish = false`): for tests only. Cross-process lock that serialises parallel `dotnet build` runs on `ZeroDDS.Cdr` (error CS2012). Used by `crates/endpoint-e2e` and `crates/conformance`.

### 7.6 `crates/ts-node` (TypeScript, Node.js)

- Not a Cargo package. npm package `@zerodds/node` 1.0.0-rc.7, FFI via `koffi` against `libzerodds`.
- Library search (`src/native.ts:37-76`): `ZERODDS_LIB`, then `target/{release,debug}/`, `target/<triple>/{release,debug}/`, `dist/runtimes/<platform>-<arch>/`, then the OS search path.
- Build/test: `npm run build` (`tsc`), `npm test` (`test/smoke.test.ts`, `xcdr2-wire-vectors.test.ts`, `qos.test.ts`, `extensibility.test.ts`).
- API: `src/dds.ts`, `src/qos.ts`, `src/cdr/` (`Xcdr2Writer`, `Xcdr2Reader`, `DdsTopicType`).
- Example: `crates/ts-node/examples/topic-typed-smoke.ts` (`npx tsx …`, wire level only).

### 7.7 `crates/ts-wasm` (TypeScript, browser)

- Cargo package `zerodds-ts-wasm`, `crate-type = ["cdylib", "rlib"]`; npm package `@zerodds/wasm`.
- Content: XCDR1/XCDR2 codec from `zerodds-wire` for primitives, strings, bytes (`src/lib.rs`), plus a DCPS-over-WebSocket runtime per DDS-TS Annex C (`src/dcps/`). Live DDS in the browser runs via `crates/websocket-bridge` (per doc comment).
- Build: `npm run build:wasm` (`wasm-pack build --target web --out-dir pkg-web` and `--target nodejs --out-dir pkg-node`), `npm run build:ts`.
- Tests: `test/smoke.test.mjs`, `test/dcps.test.ts`.

### 7.8 `crates/rs` (Rust)

- Package `zerodds-rs`, facade with re-exports from `zerodds-dcps` (`dcps`), `zerodds-dcps-async` (`aio`), `zerodds-qos` (`qos`), `zerodds-types` (`types`), `zerodds-monitor`, `zerodds-recorder`.
- Typed topics: `create_topic::<T>` with `T: zerodds_dcps::DdsType`. The impls are generated by `zerodds-build` with `Config::dds_type_impls(true)`. `zerodds-idlc generate --lang rust` generates type and codec, but no `DdsType` impls (per `tools/idlc/README.md`; no `DdsType` found in the preamble code of `generate.rs`).
- No own `examples/` directory; example in the doc comment of `crates/rs/src/lib.rs`.

### 7.9 Other bindings

- ROS 2: `crates/ros2-rmw`, `crates/rmw-zerodds-shim` (C FFI over `zerodds-ros2-rmw` and `zerodds-c-api`). See [ros2.en.md](ros2.en.md).
- CORBA language mappings: `crates/corba-rust`, `crates/corba-codegen`. See [crates-bridges-services.en.md](crates-bridges-services.en.md).
- DDS-WEB (REST PSM): `crates/web`.

---

## 8. Native endpoint SDKs (ADR 0013)

`docs/adr/0013-native-endpoint-sdks.md`, status `accepted`. Two roles: a Rust hub with full DDS (discovery, security, RTPS, XRCE agent `crates/xrce`) and native endpoints without Rust that talk to the hub over XRCE-shaped framing. The transport is a frame hook (`deliver`/`receive`) that the application fills in.

Directories in `endpoints/` (checked): `ada`, `ada-83`, `ada-native`, `c`, `cpp`, `csharp`, `d`, `elixir`, `fsharp`, `go`, `java`, `julia`, `kotlin`, `lua`, `nim`, `node`, `ocaml`, `python`, `rust`, `swift`, `typescript`, `zig`. In addition `codegen`, `golden-gen`, `xrce-agent-demo`, `examples`, `testdata`. The ADR names only C, C++, Python and Java.

- Every language has `QUICKSTART.md`. Most have `example_sync.*`, `example_async.*`, `example_reliable.*` and `xrce_client.*`. Example C: `cd endpoints/c && make examples` (per `endpoints/c/QUICKSTART.md`).
- `endpoints/codegen` (`zerodds-endpoint-codegen`, `publish = false`): own binary, `zerodds-endpoint-codegen <input.idl> <out-dir> [c|python|java]`. Uses `idl-parse` and `idl-lower` directly, not `lang-rules`. Covers primitives, `string`, `sequence<octet>`, nested structs, `sequence<struct>` and `@final`/`@appendable`/`@mutable` (per doc comment `endpoints/codegen/src/main.rs:1-22`). Test: `endpoints/codegen/tests/emitted_codec.rs`, fixtures `endpoints/codegen/idl/`.
- `endpoints/rust` (`zerodds-endpoint-rust`, `publish = false`): XRCE and XCDR2 client over `zerodds-wire`, `zerodds-xrce-client`; examples `endpoints/rust/examples/example_{sync,async,reliable}.rs`.
- `endpoints/xrce-agent-demo`: Rust peer for live UDP.
- Tests: `crates/endpoint-e2e/tests/<lang>.rs` and `<lang>_reliable.rs`. Package description: "native endpoint apps (idlc-generated types + endpoint SDK) exchange typed samples with a Rust XRCE peer". For `fsharp`, `rust` and `ada83` there is only `_reliable.rs`, for `typescript` no file.

The relation of `runtime/<lang>/` (wire runtime for `idlc` output) to the wire files in `endpoints/<lang>/` (for example `endpoints/python/zerodds_wire.py`) was not checked.

---

## 9. Overview per language

"DDS" = binding with participant/writer/reader in the process. "Endpoint" = native SDK to the Rust hub (section 8).

| Language | Crate/path | Codegen flag | Runtime | Example | Tests |
|---|---|---|---|---|---|
| Rust | DDS: `crates/rs`, `crates/dcps`; Build: `crates/zerodds-build`; Endpoint: `endpoints/rust` | `--lang rust` | `crates/wire` | `endpoints/rust/examples/example_sync.rs`; doc example in `crates/rs/src/lib.rs` | `crates/lang-rules/tests/rust_corpus_compiles.rs`, `crates/zerodds-build/tests/`, `crates/endpoint-e2e/tests/rust_reliable.rs` |
| C | DDS: `crates/zerodds-c-api`; Endpoint: `endpoints/c` | `--lang c` | `runtime/c` | `crates/zerodds-c-api/examples/c_smoke.c` | `crates/zerodds-c-api/tests/`, `crates/lang-rules/tests/c_end_to_end.rs`, `crates/endpoint-e2e/tests/c.rs` |
| C++ | DDS: `crates/cpp`; Endpoint: `endpoints/cpp` | `--lang 'c++'` | `runtime/cpp` + `crates/cpp/include` | `crates/cpp/examples/cpp_smoke.cpp`, `endpoints/cpp/example_sync.cpp` | `crates/cpp/tests/`, `crates/lang-rules/tests/cpp_corpus_compiles.rs`, `crates/endpoint-e2e/tests/cpp.rs` |
| Python | DDS: `crates/py`; Endpoint: `endpoints/python` | `--lang python` | `runtime/python` (`zerodds-wire`) | `crates/py/examples/01_bytes_pubsub.py` | `crates/py/python/tests/`, `tools/idlc/tests/python-support/`, `crates/endpoint-e2e/tests/python.rs` |
| Java | DDS: `crates/java-omgdds`; Endpoint: `endpoints/java` | `--lang java` | `runtime/java`, `crates/idl-java/runtime` | `crates/java-omgdds/java/examples/TopicTypedSmoke.java` | `crates/java-omgdds/java/src/test/java`, `tools/idlc/tests/java-support/`, `crates/endpoint-e2e/tests/java.rs` |
| C# | DDS: `crates/cs`; Endpoint: `endpoints/csharp` | `--lang 'c#'` | `runtime/csharp` (`ZeroDds.Wire`), `crates/idl-csharp/runtime` | `crates/cs/Examples/TopicTypedSmoke.cs` | `crates/cs/csharp/ZeroDDS.Tests/`, `ZeroDDS.Cdr.Tests/`, `tools/idlc/tests/csharp-support/`, `crates/endpoint-e2e/tests/csharp.rs` |
| TypeScript (Node) | DDS: `crates/ts-node` (`@zerodds/node`); Endpoint: `endpoints/typescript` | `--lang typescript` | `runtime/typescript` (`@zerodds/wire`) | `crates/ts-node/examples/topic-typed-smoke.ts` | `crates/ts-node/test/`, `crates/lang-rules/tests/typescript_codec.rs` |
| TypeScript (browser) | `crates/ts-wasm` (`@zerodds/wasm`) | `--lang typescript` | `runtime/typescript`; codec in WASM | – | `crates/ts-wasm/test/` |
| JavaScript (Node) | Endpoint: `endpoints/node` | `--lang node` | `runtime/node/zdwire.js` | `endpoints/node/example_sync.js` | `crates/lang-rules/tests/node_end_to_end.rs`, `crates/endpoint-e2e/tests/node.rs` |
| Go | Endpoint: `endpoints/go` | `--lang go` | `runtime/go` | `endpoints/go/example_sync/` | `crates/lang-rules/tests/go_end_to_end.rs`, `crates/endpoint-e2e/tests/go.rs` |
| Ada 2012 | Endpoint: `endpoints/ada`, `endpoints/ada-native` | `--lang ada` | `runtime/ada` | `endpoints/ada/QUICKSTART.md` | `crates/lang-rules/tests/ada_end_to_end.rs`, `crates/endpoint-e2e/tests/ada.rs` |
| Ada 83 | Endpoint: `endpoints/ada-83` | `--lang ada83` | `runtime/ada-83` | – | `crates/lang-rules/tests/ada83_end_to_end.rs`, `crates/endpoint-e2e/tests/ada83_reliable.rs` |
| D | Endpoint: `endpoints/d` | `--lang d` | `runtime/d/zdwire.d` | `endpoints/d/example_sync.d` | `crates/lang-rules/tests/d_end_to_end.rs`, `crates/endpoint-e2e/tests/d.rs` |
| Elixir | Endpoint: `endpoints/elixir` | `--lang elixir` | `runtime/elixir` | `endpoints/elixir/example_sync.exs` | `crates/lang-rules/tests/elixir_end_to_end.rs`, `crates/endpoint-e2e/tests/elixir.rs` |
| F# | Endpoint: `endpoints/fsharp` | `--lang 'f#'` | `runtime/fsharp/ZdWire.fs` | `endpoints/fsharp/example_sync.fsx` | `crates/lang-rules/tests/fsharp_end_to_end.rs`, `crates/endpoint-e2e/tests/fsharp_reliable.rs` |
| Julia | Endpoint: `endpoints/julia` | `--lang julia` | `runtime/julia/zerodds_wire.jl` | `endpoints/julia/example_sync.jl` | `crates/lang-rules/tests/julia_end_to_end.rs`, `crates/endpoint-e2e/tests/julia.rs` |
| Kotlin | Endpoint: `endpoints/kotlin` | `--lang kotlin` | `runtime/kotlin/ZdWire.kt` | `endpoints/kotlin/example_sync/` | `crates/lang-rules/tests/kotlin_end_to_end.rs`, `crates/endpoint-e2e/tests/kotlin.rs` |
| Lua | Endpoint: `endpoints/lua` | `--lang lua` | `runtime/lua/zerodds_wire` | `endpoints/lua/example_sync.lua` | `crates/lang-rules/tests/lua_end_to_end.rs`, `crates/endpoint-e2e/tests/lua.rs` |
| Nim | Endpoint: `endpoints/nim` | `--lang nim` | `runtime/nim/zdwire.nim` | `endpoints/nim/example_sync.nim` | `crates/lang-rules/tests/nim_end_to_end.rs`, `crates/endpoint-e2e/tests/nim.rs` |
| OCaml | Endpoint: `endpoints/ocaml` | `--lang ocaml` | `runtime/ocaml` | `endpoints/ocaml/example_sync.ml` | `crates/lang-rules/tests/ocaml_end_to_end.rs`, `crates/endpoint-e2e/tests/ocaml.rs` |
| Swift | Endpoint: `endpoints/swift` | `--lang swift` | `runtime/swift` | `endpoints/swift/example_reliable.swift` | `crates/lang-rules/tests/swift_end_to_end.rs`, `crates/endpoint-e2e/tests/swift.rs` |
| Zig | Endpoint: `endpoints/zig` | `--lang zig` | `runtime/zig/zdwire.zig` | `endpoints/zig/example_sync.zig` | `crates/lang-rules/tests/zig_end_to_end.rs`, `crates/endpoint-e2e/tests/zig.rs` |

---

## 10. How to generate types for an application

The commands follow from `parse_args`. They were not run.

### 10.1 Build the tool, check the IDL

```sh
cargo build --release -p zerodds-idlc
IDLC=target/release/zerodds-idlc

$IDLC check  idl/Robot.idl
$IDLC lint   --deny-warnings idl/Robot.idl
$IDLC format --check idl/Robot.idl
```

### 10.2 Generate

```sh
# Default: XCDR2, little endian, output to the current directory
$IDLC generate --lang rust   --out gen/rust   idl/Robot.idl   # gen/rust/Robot.rs
$IDLC generate --lang 'c++'  --out gen/cpp    idl/Robot.idl   # gen/cpp/Robot.hpp
$IDLC generate --lang python --out gen/py     idl/Robot.idl   # gen/py/Robot.py
$IDLC generate --lang java   --out gen/java   idl/Robot.idl   # ZdTypes.java, ZdCodec.java, ZdTypeSupports.java

# XCDR1 for a peer that speaks XCDR1; unannotated aggregates as @final
$IDLC generate --lang 'c#' --encoding xcdr1 --default-extensibility final --out gen/cs idl/Robot.idl
```

Each written file is reported as `<idl>: wrote <path>`, at the end `generate: N file(s), E error(s), W warning(s)`.

### 10.3 Use in the application

Rust (Cargo): use the `build.rs` helper instead of `idlc`. It additionally generates `DdsType` impls and reports includes to cargo.

```rust
// build.rs  ([build-dependencies] zerodds-build)
fn main() -> Result<(), zerodds_build::Error> {
    zerodds_build::Config::new()
        .include_dir("idl")
        .dds_type_impls(true)
        .compile(&["idl/Robot.idl"])
}
```

```rust
// crate root
include!(concat!(env!("OUT_DIR"), "/Robot.rs"));
// participant.create_topic::<Robot>("entity.robot")?;
```

Source: doc comment `crates/zerodds-build/src/lib.rs:7-40`, methods checked (`include_dir` line 243, `dds_type_impls` line 470, `compile` line 622).

C++: the generated `Robot.hpp` contains `topic_type_support<T>`; it is compiled with `crates/cpp/include`, `runtime/cpp/include` and `crates/zerodds-c-api/include` and linked against `libzerodds`.

```cpp
#include "zerodds/dds.hpp"
#include "Robot.hpp"
zerodds::Runtime rt(0);
zerodds::TypedWriter<T> w(rt, "Robot", /*reliable=*/true);   // T = generated type
zerodds::TypedReader<T> r(rt, "Robot", true);
```

Python: install `runtime/python` and `crates/py` (section 7.3), then:

```python
import zerodds, Robot as generated
descriptor = generated.ZD_TOPIC_DESCRIPTORS["<Module>::<Type>"]
topic = zerodds.IdlTopic(participant, "Robot", descriptor)
writer = topic.create_writer(participant.create_publisher())
```

Source: `crates/py/README.md` section "Generated Python and native topics", checked against `crates/py/python/zerodds/topic.py:36-87`.

Java and C#: the companion files (`ZdTypeSupports.java`, `<stem>.TypeSupport.cs`) provide the TypeSupport objects for `crates/java-omgdds` and `crates/cs` respectively. Consumer examples: `tools/idlc/tests/java-support/Consumer.java`, `tools/idlc/tests/csharp-support/Program.cs`.

Other languages: the generated file plus `runtime/<lang>/` gives type and codec. Sending goes through an endpoint SDK from `endpoints/<lang>/` to a hub.

### 10.4 Build systems

`build-integration/<ecosystem>/` calls `zerodds-idlc generate --lang … --out …` from the respective build tool; each `sample-consumer/` generates `Robot::Pose`, compiles it against `runtime/<lang>/` and sends a sample there and back. Measured (round trip 40 bytes each):

| Ecosystem | Host, tool |
|---|---|
| Maven, MSBuild, Mix, SwiftPM, Nimble, Julia `Pkg.build`, Lua/LuaRocks | macOS |
| dub (LDC 1.40.0), gprbuild (Ada) | Linux test host |
| Gradle | not built: no Gradle on either host |

None of the integrations tracks `#include` files; the CMake function does so via `--depfile`. Overview with versions: `build-integration/README.md`.

---

## 11. Unclear or contradictory points

| Location | Finding |
|---|---|
| `crates/idl` | Unversioned directory, ignored via `.gitignore`; contains only empty subdirectories and `.DS_Store` |
| `endpoints/<lang>/zerodds_wire.*` vs. `runtime/<lang>/` | Whether both are the same runtime: not checked |
