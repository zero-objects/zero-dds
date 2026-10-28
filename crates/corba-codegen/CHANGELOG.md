# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.7]

Rebuilt onto the common AST. The crate was 596 lines in five files with
no dependencies; its consumers were `zerodds-idl-cpp`, `-csharp` and
`-java`, which this rebuild replaced with `zerodds-lang-rules` and
`zerodds-lang-render`. The tables held up. The attachment did not.

### Spec references

- **OMG CORBA 3.3** part 1 Annex A.1 (special types), §7.4 (IDL types),
  §10.7 (repository identifiers), §8.3 (`ServerRequest`).
- **IDL to C++** `formal/2008-01-09`: §1.9 (fixed and variable length),
  §1.14 (arrays), §1.22 (argument passing).
- **IDL to Java** `formal/2008-01-04`: §1.3 (type mapping), §1.4 (holder
  classes), §1.5 (portable `_invoke`).
- **IDL to C#** `formal/2003-09-04` Annex A.
- **OMG IDL 4.2** §7.4.6 (object references), §7.4.11 (`native`).

### Added

- `special_type_of(&Ast, Id<TypeNode>)` — which of the thirteen a model
  type is. The attachment to the rebuilt front end; a generator asks this
  rather than matching on `TypeKind` itself.
- `special_type_of_scoped(&[&str])` — the two rows that are declared IDL
  rather than a production: `TimeBase::TimeT` and `CORBA::TypeCode`.
- `cpp_type`, `java_type`, `rust_type` — the row as a CST node, built
  from the same table the strings come from.
- `mapping_table` and `mapping_table_markdown` — the table as data, so
  `README.md` and the crate cannot drift apart.
- `repository_id_of(&Ast, AnyId)` — the model's identifier, which carries
  `#pragma prefix`, `#pragma version` and `#pragma ID`.
- `split_repository_id`, `scoped_name_from_repository_id`,
  `build_repository_id_with_prefix`.
- `TypeShape` and `shape_of` — the fixed/variable/collection property the
  C++ and Java argument-passing rules turn on, computed from the model's
  members.
- `stub_op_of`, `stub_type_of` — a `StubOp` from an operation in the
  model, with `raises` and `oneway`.
- `cpp_stub_function`, `java_stub_method`, `rust_stub_function` and the
  per-position `cpp_param_type` / `cpp_return_type` / `java_param_type` /
  `java_holder_type` / `rust_param_type`.
- `skeleton_ops_of` — an interface's dispatch table, with attributes in
  the `_get_x` / `_set_x` form §7.4.4 puts on the wire.
- `cpp_dispatch`, `java_dispatch`, `rust_dispatch`.
- `tests/orb-corpus/` — fifteen IDL files, one per row, and `record.sh`,
  which runs `omniidl`, `tao_idl` and JacORB's `idl` over them on the
  measurement host and records what they generated.

### Changed

- **The stub and skeleton helpers build CST nodes.** They used to
  concatenate strings and produce a comment; nothing in the workspace
  called them.
- **The Rust column.** `long double` is `zerodds_float128::Float128` and
  not `f64`, which put eight bytes of a value nobody wrote on the wire.
  `any`, `TypeCode` and `wstring` are `zerodds-corba-rt`'s, following
  their move out of the wire crate. `fixed<P,S>` has no Rust holder in
  this tree and the row says so rather than naming `zerodds-cdr`, which
  was demolished.
- **A `typedef` is written by its own name.** `typedef fixed<10,2> Money`
  is `probe::Money` in a C++ signature and not `CORBA::Fixed`; `native
  Handle` is `probe::Handle` and no ORB declares it. Measured against all
  three.
- **An `abstract interface` is not the `AbstractBase` row.** All three
  ORBs write the interface's own name.
- **A Java `wstring` holder is `StringHolder`.** Both string kinds map to
  `java.lang.String`.
- **A Java `sequence<any>` is `org.omg.CORBA.Any[]`.** The earlier table
  said `AnySeqHelper`, which is a marshalling class and never a member
  type.
- `TargetLanguage` gained `cst_language` and `from_cst_language`, which
  report the two gaps rather than defaulting: C has no CORBA PSM and C#
  has no CST here.

### Removed

- `render_stub_op`'s and `render_skeleton_dispatch`'s infallible
  signatures. Both now return `Result<String, NoRenderer>` and refuse the
  C# column by name.

### Known divergence

- JacORB 3.9 writes `java.io.SerializableHolder` for an `out ValueBase`.
  The JDK has no such class, so a stub naming it does not compile. This
  crate emits `org.omg.CORBA.ValueBaseHolder`, which the mapping defines
  and which exists. `tests/orb_agreement.rs` asserts both halves.
- omniidl and tao_idl write two different typedefs of one type for an
  `out TypeCode` — `CORBA::TypeCode_OUT_arg` and `CORBA::TypeCode_out`.
  This crate emits the second, which is the specification's own name.
