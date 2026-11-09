# Changelog

All notable changes to ZeroDDS at the workspace level are documented
here. Per-crate CHANGELOGs in each `crates/<name>/CHANGELOG.md`.

Format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning 2.0](https://semver.org/).

## [Unreleased]

### Added

- **The forms in which the implementations write XTypes TypeObjects, as
  options** (`zerodds-types` `from_ast::TypeObjectForm`, `zerodds-dcps`
  `DomainParticipant::set_minimal_type_flags_kept`). RTI Connext 7.7.0,
  CoreDX 6.15.3, Cyclone DDS 11.0.1 and HDDS 1.4.8 keep `IS_AUTOID_HASH` in
  the MINIMAL object and leave `AppliedBuiltinMemberAnnotations::hash_id` out
  of the COMPLETE one; the defaults stay Annex B's (`docs/omg-xtypes-issues.md`
  B11, B12). The OMG dds-xtypes client selects the vendors' form, the suite's
  answer, and its typeid cases `struct_hashid_1` and `_2` print the vendors'
  hashes (`--minimal-type-flags`, `--hash-id` give Annex B's). The client
  also sets the settings the suite needs where the runtime keeps the clause
  (an endpoint without PID 0x0074 as ALLOW_TYPE_COERCION, PL_CDR1 lengths of
  either convention, the empty ACKNACK), as `omg-shape-main` does, and
  prints the TypeObject V1 id of DDS-XTypes 1.1. Measured on the Linux test
  host: the seven cases ZeroDDS -> Cyclone and `primitives_struct_mutable`
  Cyclone -> ZeroDDS that were declared deviations pass; their rows left
  `tests/interop/omg/vendor-deviations.tsv`.

- **`PID_WRITER_GROUP_INFO` in 16 octets for Dust DDS participants**
  (`zerodds-rtps` `dialect::WriterGroupInfoWidth`, `VendorId::DUST`). A Dust
  0.17.0 reader reads the parameter as 16 octets (`GroupDigest = [Octet;
  16]`, dust-dds `dds/src/rtps_messages/types.rs`, read in
  `cache_change.rs`) and drops a DATA that carries the 4 octets of
  DDSI-RTPS 2.5 §9.3.2.5. `WriterGroupInfoWidth::Sixteen` writes the digest
  followed by 12 zero octets; the default stays the specification's form, and
  a ZeroDDS reader reads the first 4 octets of either. `shape_main` sets it
  for `VendorId::DUST` (0x0114). Measured on the Linux test host: OMG
  dds-rtps ZeroDDS -> Dust `Test_CoherentSets_6`, `_7`, `_8`, `_12`, `_16`,
  `_17`, `_18`, `_21` and `Test_OrderedAccess_6`, `_7`, `_8`, `_14`, `_15`,
  `_16` red before, 14 of 14 green after; the whole suite ZeroDDS -> Dust
  110 of 110, ZeroDDS -> Connext 110 of 110. Tests:
  `a_participant_set_to_sixteen_octets_gets_the_writer_set_widened`,
  `a_sixteen_octet_writer_group_info_is_read_by_its_first_four`.

- **`derive(DdsType)`: a `@final` or `@appendable` enumeration, and a
  discriminator value without a member as a unit variant** (GitHub #33;
  `zerodds-derive`, `zerodds-codec`, `zerodds-wire` `TypeDesc::
  enum_extensibility`; DDS-XTypes 1.3 §7.2.3 Table 12, IDL 4.2
  §7.4.1.4.4.4). A field-less enum takes `extensibility = "final"` or
  `"appendable"` and exports it; the TypeObject carries it, and Table 18
  assigns enumerations of one kind only. In a union, `#[dds(case = label)]
  V` is that discriminator value without a member: it goes out alone,
  reads back as `V` and declares no branch; the `no_member` variant beside
  it takes the values no variant names and refuses one a variant names.
  Tests: `issue_33_a_union_whose_discriminator_values_select_no_member`
  (in `xtypes_7_3_4_7_4_issue_33_a_derived_type_and_its_idl_are_one_type`:
  the reporter's `@final enum` and union, as unit variants and as
  `no_member`, give the TypeIdentifier, TypeObject, dependencies and the
  octets of all four encodings of the IDL route), and two tests in
  `crates/codec/tests/derive_union_generic_borrowed.rs`.
- **A loaned take, and a type whose fields borrow through a DataReader and
  a DataWriter** (GitHub #33; `zerodds-dcps` `DataReader::take_loan`,
  `return_loan_of`, `loan::Loan`, `WriteAs`, `DataWriter::write_as`,
  `DdsType::BORROWS`, `DdsType::key_material_from_body`; `zerodds-codec`
  `LoanExt::read_as`, `write`, `Borrowed`; `zerodds-derive`; DDS 1.4
  §2.2.2.5.3.5, §2.2.2.5.3.20). `take_loan` takes the samples out of the
  cache as the octets they arrived in, with their `SampleInfo`; from the
  first call on the reader admits by key members, so what it lends was not
  decoded. `return_loan_of(&mut loan)` refuses a loan of another reader and
  leaves a returned loan empty; a reader with a loan that was not returned
  or dropped is not deleted, alone or with its subscriber or participant
  (§2.2.2.5.2.6, §2.2.2.5.2.14). `read_as::<Lent<'_>>(i)` reads a derived
  type with a lifetime lent out of the received datagram; `zerodds_codec::write(&writer, &value)`
  writes one with borrows of any lifetime through the writer of its
  `'static` form, whose `decode` refuses. Test
  `issue_33_a_borrowing_type_is_written_and_read_lent_through_dcps`: the
  text and the octets read are inside the received body. OPEN-ITEMS 56 is
  closed.
- **`derive(DdsType)`: an enum with data as an IDL union, generic types,
  borrowed fields** (GitHub #33, `rc8-rust-newtype.md`, "Derive route",
  maintainer's decision of 2026-11-05). An enum whose variants carry data
  is a `union`: `#[dds(discriminator = "…")]` (an integer, `bool`, `char` or
  a derived field-less enum, default `i32`), `case` labels written or
  counted, `default`, `no_member` for a discriminator no case takes, branch
  `id`/`hashid`/`name`/`bound`, every extensibility; a case with several
  labels and the `default:` case are `V(D, T)` and go out under the
  discriminator they were read with. The codec API types a union too:
  `serialize_union::<T>()` and `branch::<B1>(&d, &value)` with a type-level
  index, so a value of another branch type does not compile. A generic type
  is one IDL type per instantiation, its name a template (`Pair_{T}`,
  `Pair<u32>` → `Pair_unsigned_long`), each with its own `TypeObject` and
  `TypeIdentifier`; `const` parameters too. A field may be `&'a str`,
  `&'a [u8]`, `Cow<'a, str>` or `Cow<'a, [u8]>`, which `zerodds_codec::
  decode` lends out of the received bytes; such a type has no
  `zerodds_dcps::DdsType` impl, because the loaned take of
  `rc8-zero-copy.md` is not built (OPEN-ITEMS 56). `DdsMember` no longer
  reads: `DdsMemberRead<'de>` does, and the derive builds a member's default
  from its fields. Tests: `crates/zerodds-build/tests/dds_type.rs::
  xtypes_7_3_4_7_4_issue_33_a_derived_type_and_its_idl_are_one_type` (now
  also unions as roots, members, in a sequence and as a key, `@mutable`
  framing, an enumeration discriminator, no member, `Pair<i32>`,
  `Pair<Inner>`, `Grid<3>`, a borrowed struct; all four encodings against
  the generated codec), `xtypes_7_4_issue_33_a_c_peer_reads_a_derived_union_
  and_generic_type`, `issue_33_a_hand_written_union_codec_of_another_branch_
  type_does_not_compile` (integration-codegen-build);
  `crates/codec/tests/derive_union_generic_borrowed.rs`. Example:
  `examples/rust-derive/` with a union, a generic pair and a borrowed view.

- **A union keyed by its discriminator** (DDS-XTypes 1.3 §7.2.2.4.4.4.8,
  `switch (@key D)`): `#[dds(key)]` on a derived union, and `zerodds-build`
  carries the annotation (`KeyOnUnionDiscriminator::Carried`, before
  `Refused`). The union is a keyed topic type whose key holder is the
  discriminator alone (§7.6.8 step 1); as a key member of a struct only
  its discriminator is part of that key, and a union without a key
  discriminator is whole and `@final` there. Tests:
  `crates/zerodds-build/tests/dds_type.rs::xtypes_7_6_8_a_nested_key_member_is_its_own_final_key_holder`
  (octets worked by hand from §7.6.8), `…::xtypes_7_3_4_7_4_issue_33_a_derived_type_and_its_idl_are_one_type`
  (`a_union_keyed_by_its_discriminator`),
  `crates/codec/tests/derive_union_generic_borrowed.rs::xtypes_7_6_8_the_key_holder_of_a_union_keyed_by_its_discriminator`.
- **Content filters reach union members** (DDS-XTypes 1.4 Beta 1
  §7.6.7.1, DDSXTY14-1): a branch by its member's name, the discriminator
  as `#discriminator`; an expression that names a branch the
  discriminator does not select is `false` as a whole
  (`RowAccess::unselected`, `DdsType::field_unselected`). For
  `derive(DdsType)`, the types `zerodds-build` generates and samples with
  a run-time type. An enumerated member compares with its enumerator's
  name in the types `zerodds-build` generates and in `derive(DdsType)`
  (DDS 1.4 Annex B.2 ENUMERATEDVALUE); before, it had no filter value.
  Tests: `crates/sql-filter/src/evaluator.rs::tests::xtypes_1_4_7_6_7_1_a_member_the_discriminator_does_not_select_makes_the_filter_false`,
  `crates/dcps/src/serialized_type.rs::tests::xtypes_1_4_7_6_7_1_a_dynamic_union_branch_the_discriminator_does_not_select`,
  `crates/codec/tests/derive_union_generic_borrowed.rs::xtypes_1_4_7_6_7_1_a_filter_reads_the_selected_branch_and_the_discriminator`,
  `crates/zerodds-build/tests/dds_type.rs::xtypes_7_3_4_7_4_issue_33_a_derived_type_and_its_idl_are_one_type`
  (`a_filter_reads_union_branches_and_the_discriminator`).

- **`long double` constant expressions are evaluated in IEEE binary128**
  (`zerodds-float128`: the `core::ops` operators `+`, `-`, `*` and `/`,
  each rounded once to nearest, ties to even; `zerodds-ast`
  evaluates a `long double` constant, and a `double` or `float` one with
  a `long double` operand, in them, IDL 4.2 §7.4.1.4.2). Before, such an
  expression was refused with `EvalError::LongDouble`. A sub-expression
  beyond the range of binary128, and a floating literal that does not
  fit the type of the constant it is the value of, is
  `EvalError::Overflow`. The four operations give the bits GCC's
  `__float128` gives for 6072 operand pairs (oracle
  `crates/float128/tests/oracle/quad_ops.c`, test
  `crates/float128/tests/gcc_binary128_ops.rs`). OPEN-ITEMS 35 is closed.
- **`#include <orb.idl>` and `#include <TypeCode.idl>` bring the CORBA
  module** (OMG IDL 4.2 §7.4.6.4.7): the IDL of the CORBA module as the
  OMG publishes it with CORBA 3.3 is built into the preprocessor
  (`crates/idl-parse/src/preprocess/corba/`, two transcription changes
  named in its README) and is read when the caller's loader has no file
  of that name. Test:
  `crates/idl-compose/tests/corba_module.rs`.
- **`zerodds-corba-rust`: a `custom valuetype` is marshaled by the
  application** (OMG IDL 4.2 §7.4.7.4.4, CORBA 3.3 part 1 §9.5.1). The
  generated code declares `<Name>CustomMarshal` with `marshal` and
  `unmarshal`; the generated `ValueMarshal` and the registered factory
  call it. Test:
  `crates/corba-rust/tests/compile_check.rs::idl_7_4_7_4_4_a_custom_valuetype_is_marshaled_by_the_application`.

- **The IDL front end checks the scoping, inheritance, component-model
  and profile rules of OMG IDL 4.2** (`crates/idl-lower`,
  `crates/idl-parse/src/building_block.rs`). Names resolve as §7.5
  gives it: a name used in a scope is introduced there and may not be
  declared again in it (§7.5.2), an inherited name is found through the
  bases and an ambiguous one is reported with every candidate (§7.5.1).
  Refused now, with the clause in the message: a base interface named
  twice, an abstract interface with a concrete base, an inherited
  operation or attribute redefined (§7.4.3.4.3.2.1); the value
  inheritance combinations of §7.4.7.4.3 and Table 7-19, `truncatable`
  on a custom value type (§7.4.7.4.5); a constant outside the range of
  its type (Table 7-26); a bit position taken twice (§7.4.13.4.3.3);
  `@range` with max below min and a `@default` or `@range` value that
  does not fit the element (§8.3.3); a `oneway` operation with a result,
  an `out` parameter or a `raises` clause (§7.4.6.4.5, §8.3.6.2); a
  native parameter in a remote interface (§7.4.6.4.4; the setting
  `native_in_remote_interfaces: Admitted` keeps the TAO and JacORB
  reading); a malformed `context` string (§7.4.6.4.6); `#pragma ID` or
  `#pragma prefix` disagreeing with `typeid` or `typeprefix`
  (§7.4.6.4.1.3); a `\u` escape in a narrow literal (§7.2.6.2.2); an
  operation named like its interface (§7.5.2); facets, receptacles,
  homes, port types and connectors that do not name what §7.4.8 to
  §7.4.11 require. `@oneway(FALSE)` and `@ami(FALSE)` are honoured.
  IDL that the front end accepted before can be refused by these rules.
  `zerodds-idlc --idl-profile <P>` reports every construct outside one
  of the seven profiles of clause 9 (`IDL0023`). Tests:
  `crates/idl-lower/tests/idl_4_2_rules.rs`,
  `crates/idl-parse/tests/building_blocks.rs`,
  `tools/idlc/tests/idl_4_2_gaps.rs` (every probe of the gap scan
  against the specification's answer).

- **`zerodds-codec` and `zerodds-derive`: a DDS topic type defined in Rust**
  (GitHub #33, `rc8-rust-newtype.md`, "Derive route" and "Codec API").
  `#[derive(DdsType)]` on a struct with named fields writes the type's
  description (`zerodds_wire::codec::DdsTypeInfo`: the members' Rust types
  as a type list, the declaration as a constant), its codec
  (`DdsSerialize`, `DdsDeserialize<'de>`), its `DdsMember` impl and the
  `zerodds_dcps::DdsType` impl, and `T::idl()` exports the IDL for C, C++
  and other vendors. Attributes after the IDL annotations: `name`,
  `extensibility`, `autoid`, `nested`, `codec = "manual"` on the struct;
  `key`, `id`, `hashid`, `bound`, `name`, `optional`, `with` on a field;
  `name`, `bit_bound` on a field-less enum and `name`, `value` on a
  variant. `#[derive(WireRepr)]` with `#[wire(idl = "string")]` reads
  through `TryFrom<&str>` and writes through `Display`, with an IDL
  primitive through `TryFrom`/`Into`; such a type works as a member and as
  the target of `rust_type`. The description becomes IDL and goes through
  `zerodds-idl-compose`, `zerodds-types` and `zerodds-ast-layout` as a
  `.idl` file does; the plans drive the serializer of `zerodds-wire`, one
  per encapsulation. A hand-written codec is checked at compile time:
  `serialize_struct::<T>()` is typed by `T::Members`, `member` takes the
  head's type and `end` exists only on the empty list; `@mutable` decoding
  fills one typed slot per member id. `core::time::Duration` is
  `DDS::Duration_t`, `std::time::SystemTime` (feature `std` of
  `zerodds-wire`) `DDS::Time_t`. Tests:
  `crates/zerodds-build/tests/dds_type.rs::xtypes_7_3_4_7_4_issue_33_a_derived_type_and_its_idl_are_one_type`
  (integration-codegen-build: per construct the same `TypeIdentifier`,
  `TypeObject`, dependencies, octets in all four encodings and key holder
  as the type generated from the equivalent IDL),
  `xtypes_7_4_issue_33_a_c_peer_reads_what_a_derived_type_wrote`, the two
  `issue_33_a_hand_written_codec_*_does_not_compile`;
  `crates/codec/tests/derive_behaviour.rs` and the contract targets of
  `zerodds-codec`. Example: `examples/rust-derive/`, both routes and a C
  peer generated by `zerodds-idlc` from the exported IDL. Not in this
  version: data-carrying enums, generic types, borrowed fields.
- **The annotations of the IDL4 language mappings reach the model**:
  `@java_mapping` (IDL4 to Java 1.0 §8.1), `@csharp_mapping` (IDL4 to
  C# 1.0 §8.1) and `@cpp_mapping` (IDL4 to C++ 1.0 §8.1) are built-in
  annotations, checked against the declaration their clause gives —
  an unknown parameter, an enumerator the enumeration does not have, a
  value of the wrong kind and the positional form are `IDL0007` — and
  held as `Facets::mappings` (`zerodds_ast::MappingSetting`). The
  snapshot carries them (`SNAPSHOT_VERSION` 5; a version-4 snapshot is
  refused as before for an unknown version). Test:
  `crates/idl-lower/tests/mapping_annotations.rs`.
- **An attribute in a value type body is an attribute of the value type**
  (OMG IDL 4.2 §7.4.5.3, rule (105)). It was refused with
  `Unsupported`; `AttributeNode::parent` is now an `AnyId`, and
  `zerodds-corba-rust` declares its getter and, unless it is `readonly`,
  its setter on the value type's trait. Tests:
  `crates/idl-lower/tests/dialects.rs::idl_7_4_5_3_an_attribute_in_a_value_type_body_is_an_attribute_of_it`,
  `crates/corba-rust/tests/compile_check.rs::idl_7_4_5_3_a_value_types_attributes_are_methods_of_its_trait`.
- **An annotation prefixing the type of a typedef annotates the typedef**
  (OMG IDL 4.2 §7.4.15.4.2): `typedef @max(100) long Length;`, the
  form IDL4 to Java 1.0 §7.2.4.6 uses, was a parse error; the annotation
  now annotates each declared alias. Test:
  `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_15_4_2_an_annotation_prefixing_a_typedefs_type_annotates_the_typedef`.
- **`zerodds-ccm` maps extended ports and connectors** (OMG IDL 4.2
  §7.4.11, DDS4CCM 1.1 §7.2): `expand_extended_ports` turns a `port` into
  the basic ports of its port type, named `<port>_<port feature>`, with
  `mirrorport` swapping provides and uses, nested ports expanded and the
  port type's attributes declared on the component;
  `transform_connector` maps a connector through its ports as a
  component. A multiple receptacle declares `<name>Connection` and
  `<name>Connections` in the component's scope (CCM 4.0 §6.5.1), and
  the `<home>Explicit`, `<home>Implicit` and `<event type>Consumer`
  references name the scope of the declaration they derive from; they
  named the root scope, which only reached them when the component was
  declared there. A port's `<event type>Consumer` is the object reference
  to the consumer interface the event type's transform declared, and that
  interface is declared once. The operations and attributes of a home
  body move to `<home>Explicit` (CCM 4.0 §6.7.2): a `factory` returns the
  managed component and raises `Components::CreateFailure` besides its own
  exceptions, a `finder` raises `Components::FinderFailure`; they stayed
  on the equivalent interface, a `factory` returning nothing. The front end
  marks the two (`CorbaFacet::home_operation`, in the snapshot), and
  `Ast::set_return_type`, `Ast::move_operation` and
  `Ast::move_attribute` make the move. Tests:
  `crates/ccm/tests/equivalent_idl.rs`.
- **A template module's `const` formal parameter has its type** (OMG IDL
  4.2 §7.4.12.4.2): the binding of `const long Capacity` in an instance
  was a constant without a type, which no resolve pass could resolve; it
  has the formal's `<const_type>` now, and the instance records its actual
  arguments (`CorbaFacet::template_args`), so a mapping can tell the
  binding from a declaration of the instance. Test:
  `crates/idl-lower/tests/dialects.rs::idl_7_4_12_4_2_a_const_formal_has_its_type_and_is_an_actual_argument`.
- **`zerodds-idlc generate --lang java --java-mapping idl4`: the IDL4 to
  Java Language Mapping 1.0** (`crates/lang-rules/src/emit/idl4_java/`).
  A package per module and a file per class; structures with private
  fields, accessors, modifiers and the default and all-values
  constructors; unions with `__discriminator`; Java enums with
  `valueOf(int)`; bit sets, bit masks (`<Name>Flags` and
  `java.util.BitSet`), maps (`java.util.Map`), the sequence classes of
  Table 7.4 and `org.omg.type.Holder<E>` (`runtime/java/.../org/omg/type`);
  interfaces with `throws`, value types as `<Name>Abstract` and `<Name>`,
  user annotations as annotation types; the IDL and Java naming schemes
  (`--java-naming`) and `@java_mapping`; components, homes, ports,
  connectors and template module instances through their intermediate IDL
  (`crates/lang-rules/src/emit/intermediate_idl.rs`); the CORBA platform of
  Annex A (`--java-platform corba`) with `org.omg.CORBA.UserException`,
  `TypeCode`, `Object` and `Any` in `runtime/java/corba/org/omg/CORBA` and
  `--java-corba-module` for the Java of the CORBA module, whose pseudo IDL
  is transcribed in `runtime/corba/CORBA_PIDL.idl`. The default of `--lang
  java` stays the DDS-Java-PSM binding. Tests:
  `crates/lang-rules/tests/java_idl4_mapping.rs`,
  `java_idl4_corba.rs`, `java_idl4_components.rs` (each compiled with
  `javac --release 8 -Xlint:all -Werror` and run).
- **`zerodds-idlc generate --lang csharp --csharp-mapping idl4`: the IDL4
  to C# Language Mapping 1.0** (`crates/lang-rules/src/emit/idl4_csharp/`).
  A namespace per module; constants as static classes or a `Constants`
  container; structures as classes (or, with `@csharp_mapping(struct_type =
  "struct")`, structs) with properties, the default, copy and all-values
  constructors and `IEquatable<T>`; unions with `Discriminator`; enums
  with the underlying type of `@bit_bound`; `I<Name>` interfaces with
  `ref` and `out`, `I<Name>Operations` and a class for embedded
  declarations (§7.5); exceptions on `System.Exception`; value types as
  abstract classes with `[Omg.Types.Factory]`; bit sets as structs; bit
  masks as `[Flags]` enums and `BitArray`; maps as `IDictionary`;
  `@optional`, `@key`, `@unit`, `@range` and `@external` as Tables 7.5 to
  7.8 say; user annotations as attribute classes; the IDL and .NET naming
  schemes (`--csharp-naming`); components through their intermediate IDL;
  the CORBA platform (`--csharp-platform corba`, `--csharp-corba-module`)
  with `Corba.UserException`, `TypeCode`, `IObject`, `Object` and `Any`
  (`crates/idl-csharp/runtime/Corba.cs`). `Omg.Types`
  (`crates/idl-csharp/runtime/Omg.Types.cs`) carries `ISequence<T>` with
  the methods of §7.2.4.2.1, its bounded and unbounded implementations, a
  bounded dictionary, `Any` and the attributes. The default of `--lang
  csharp` stays the XCDR2 binding. Tests:
  `crates/lang-rules/tests/csharp_idl4_mapping.rs`, `csharp_idl4_corba.rs`
  (each compiled with Roslyn, `-warnaserror+`, and run).
- **A snapshot's modules are children of their parents again after
  loading**: `Ast::from_snapshot` left `ModuleNode::children` empty, so a
  walk down from the root found no nested module. Test:
  `crates/ast/tests/snapshot_sections.rs::a_loaded_module_is_a_child_of_its_parent`.

- **`zerodds_build::Config::rust_type` and `rust_type_with`: an IDL
  typedef, struct member or union branch held as a Rust type of one's
  own** (GitHub #33, `rc8-rust-newtype.md`). `rust_type("app::Dtg",
  "crate::Dtg")` makes every use of the typedef — a member, a sequence,
  array or map element, a union branch, `@optional`, `@external` — hold
  the Rust type; `rust_type("app::Track.id", "core::num::NonZeroU32")`
  does it for one member, ahead of a typedef mapping. The generated codec
  writes the IDL value through `zerodds_wire::WireRepr::to_wire` and reads
  it — a `string` lent out of the sample — through `from_wire`; a value
  the Rust type refuses rejects the sample with that reason. The
  `TypeObject`, the `TypeIdentifier`, assignability and the key hash stay
  the IDL's. `rust_type_with` goes through an adapter
  (`zerodds_wire::DisplayFromStr` for `Display + FromStr` types). A path
  that names no typedef, member or branch of the compiled units, an IDL
  type no Rust type can stand for (a collection, `wstring`, `wchar`,
  `long double`) and a Rust type that is no type path fail the build
  (`Error::RustType`); a Rust type of another IDL type fails to compile
  at `zerodds_wire::repr::idl_type_is`, which the generated file states
  per mapping. A type whose member refuses its IDL default (`NonZeroU32`
  and the default zero) has no `impl Default`, and a sample without that
  member — an older `@appendable` writer, a key-only sample — is rejected.
  `field_value` compares the IDL value. Tests:
  `crates/zerodds-build/tests/rust_type.rs` (the refused paths; the
  `TypeObject`s and `TypeIdentifier`s of a mapped unit equal the IDL's),
  `crates/zerodds-build/tests/dds_type.rs::xtypes_7_4_issue_33_*` and
  `issue_33_a_rust_type_of_another_idl_type_fails_the_build`
  (integration-codegen-build: the octets equal the unmapped types' in all
  four encodings, rejected samples, the key hash over the IDL value,
  Table 19 defaults, and a C peer generated from the same IDL reading the
  samples). Example: `examples/rust-type/`.

- **`zerodds_wire::WireRepr`: a Rust type that stands for an IDL primitive
  or `string`** (GitHub #33, `rc8-rust-newtype.md`). `type Idl` names the
  IDL type (`str`, `u32`, …); `to_wire` writes through an `IdlOut` for
  exactly that type and returns the `Written` only an `IdlOut` makes;
  `from_wire` reads `&Self::Idl`, a `string` lent out of the received
  sample. `IdlOut::display` formats a `Display` value straight into the
  sample's buffer and patches the length in afterwards, with the bound
  and NUL checks on the written text; `Reader::read_str` and
  `Reader::fit_str` read and narrow a `string` without allocating
  (`read_string` is `read_str` copied). Impls for `core::num::NonZero*`,
  `String`, `Cow<'_, str>` and the primitives; `WireReprWith` with the
  `DisplayFromStr` adapter for `Display + FromStr` types;
  `WireError::invalid_value` for a refusal. Tests:
  `crates/wire/tests/repr.rs` (the octets of XTypes 1.3 §7.4.1.1.2 for a
  displayed string, the refusals that leave the buffer as it was, the
  lent text, a zero refused for `NonZeroU32`, `DisplayFromStr`).

- **The vendor dialects bring `xtypes.key_must_understand: implied`.**
  `wire.rtps_dialect` or `wire.codec_dialect` set to `cyclone`, `fastdds`
  or `opendds` selects `implied` when `xtypes.key_must_understand` is not
  set; the setting, where given, wins. Measured besides Cyclone `master`:
  the Fast DDS 3.6.1 and OpenDDS 3.35 readers of `tools/dispose-reader`
  without `@must_understand` announce the key member without
  IS_MUST_UNDERSTAND and no PID 0x0074, and a ZeroDDS writer refused both
  with the default and matched both with `implied`. The readers of Connext
  7.7.0, CoreDX 6.15.3, Dust 0.16.1, HDDS 1.4.9 and InterCOM 4.3.1 announce
  PID 0x0074 and match with the default.

- **`xtypes.key_must_understand`** (`flagged`, the default, or `implied`;
  environment `ZERODDS_KEY_MUST_UNDERSTAND`). A Cyclone DDS reader built
  with its type library announces a `TypeObject` whose key member lacks
  `IS_MUST_UNDERSTAND` (the `idlc` of 11.0.1 and of `master`) and no
  TypeConsistencyEnforcement; DDS-XTypes 1.3 §7.6.3.4.1 holds it to
  DISALLOW_TYPE_COERCION, the minimal identifiers differ in that bit
  (1.4 Beta 1 §7.3.3.4), and a ZeroDDS writer does not match it. The
  default keeps the clause; `implied` compares with `IS_KEY` implying the
  bit. The dds-rtps `shape_main` of Cyclone `master` in the ci-omg image
  is built with Cyclone's type library again, and the weekly group
  `rtps-cyclone-master` runs with `implied` (`rtps-vendor-1`: 15 of 15
  cases with ZeroDDS publishing red without the setting, passed with it).

- **The IDL 4.2 gap scan is a gate.** `tools/idl-4.2-gap-scan/scan.sh` ran
  in no CI job, and its record had drifted from `main` by 58 cells. It runs
  now in the push lane as `merge-idl-gap-scan` for a change to
  `zerodds-idlc`, a package it depends on directly or the scan's own files
  (`ci/merge_lane.py`, `gap_scan`), and weekly as `idl-gap-scan`
  (`ci/weekly-checks.yml`). The direction of a changed cell is judged
  against the probe's `expect` column (OMG IDL 4.2), not against the
  record; each changed cell prints the subcommand's first lines; the
  probes run in parallel (7 s on the CI runner, job 230868).

- **`integration-dds-chat`**: every port of the dds-chat tutorial reads
  the ten fixtures of `shared/fixtures/` to their values and writes them
  back to their octets (C++ three, Python two, TypeScript three, Java two,
  C# two, Rust: the tutorial crate and the `no_std` codec; the five bridges
  carry a fixture; `tests/integration/dds_chat.sh`, weekly).
  `vendor-live-dcps` serializes the same samples with Cyclone DDS's Python
  binding (`fixtures.py --cyclone`).

- **The Java PSM, the WASM build and the Flutter binding exchange samples
  with a Rust process** (zerodds-ffi-loader-1.0 §5, OPEN-ITEMS 30): a
  DataReader of the Java PSM on its RTPS path (job `integration-java-psm`),
  the DCPS client of `zerodds-ts-wasm` through the daemon
  `zerodds-ws-bridged` with the crate's WebAssembly codec decoding every
  body, in Node without a browser (job `integration-ts-wasm`), and a Dart
  subscriber over the dart:ffi loader of the dds-chat tutorial (job
  `integration-flutter`) each take the 16 samples of a Rust publisher in
  another process, held to the bodies DDS-XTypes 1.3 §7.4.3.5 gives.

- **HDDS in the speed pillar**: the row `hdds` of `tests/perf/speed/speed.py`,
  a Rust app over the HDDS API (`tests/perf/dds-roundtrip-bench/hdds`, HDDS
  v1.1.5 and hddsgen 1.2.0 from git.hdds.io, the type generated from
  `roundtrip.idl`), built by `build.sh` and measured by `speed-vendors-4`
  in the setup of every other vendor (UDPv4, the same sizes and QoS,
  rate at the reader, loss, latency quantiles). Its values are in
  `tests/perf/baseline/speed.json` under the label `vendors`.

- **Dust DDS in the speed pillar**: the row `dust` of
  `tests/perf/speed/speed.py`, a Rust app over the dust_dds API
  (`tests/perf/dds-roundtrip-bench/dust`, dust_dds and dust_dds_gen 0.16.0
  from crates.io, the type generated from `roundtrip.idl`), built by
  `build.sh` and measured by `speed-vendors-5` in the same run as HDDS. Dust
  DDS 0.16.0 has RTPS over UDPv4 only; its app refuses
  `ZERODDS_BENCH_TRANSPORT=SHM`. Under KEEP_ALL its writer keeps every
  sample, so a write after `max_samples` writes returns `OutOfResources`
  (measured on the Linux test host, `max_samples` 1000: 1000 written, every
  later write refused, RELIABLE and BEST_EFFORT); the app's header says so.

- **Cyclone DDS `master` beside its release in the interop pillar** (GitHub
  #60). The ci-omg image builds Cyclone at `CYCLONEDDS_MASTER_COMMIT`
  (`ci/Dockerfile.omg`, stage `cyclone-master`) and the dds-rtps and
  dds-xtypes clients of the pinned harnesses against it,
  statically (`ci/build-cyclone-suite-clients.sh`); the weekly groups
  `rtps-cyclone-master` (`rtps-vendor-1`) and `xtypes-cyclone-master`
  (`xtypes-master-1`, 99 cases of the union, try-construct, extensibility,
  struct, widening and enumeration families) run against them. Against
  the release column, measured on the Linux test host: every case run
  against both gave the same verdict (dds-xtypes 328 per direction,
  dds-rtps 68 and 70), master fails the same cases against itself, and the
  seven deviations of OPEN-ITEMS 55 stay. The released
  `eclipse_cyclone-11.0.1` executables are master builds of mid-2026, not
  the tag (`rc8-test-suite.md`, "Interop").

- **The weekly run without the duplicate dds-xtypes group**: `xtypes-cyclone-master`
  runs on a release candidate and when the Cyclone `master` pin moves
  (release playbook, "Vendor pins", step 3), not every week: in pipelines
  9097, 9170 and 9186 its 99 cases gave the results of the same cases
  against 11.0.1 (4.0 minutes of the CI runner host in 9186). `rtps-cyclone-master`
  stays weekly; it is the one group that runs
  `xtypes.key_must_understand: implied` against a live peer.

- **JacORB and its Java 8 JDK in the integration hosts' tool cache**
  (`tests/integration/provision.sh`): `integration-bridges-vendor-corba`
  downloaded both in every job, and the download of JacORB 3.9 stalled
  until the job's 5 minutes ran out (pipeline 9186, job 231150, 296.6 s
  provisioning; OPEN-ITEMS 41). A download that stays below 1 KiB/s for
  30 s is retried, and each download prints its duration.

- **The speed pillar measures ZeroDDS against HDDS of the same run** (`tests/perf/speed/speed.py`): the row `zerodds-hdds-timing` runs ZeroDDS with the reliability timing HDDS 1.1.5 uses (its reader answers a HEARTBEAT at once and at most every 10 ms, its writer a NACK at once), the job `speed-zerodds-hdds-timing` measures it next to HDDS, and the report lists every ZeroDDS row against HDDS cell by cell (the targets of `rc8-plan.md` D3: equal or better). ZeroDDS' defaults stay the specification's.

- **One weekly run of the core crates' tests**: the coverage jobs of
  `zerodds-wire`, `-types`, `-rtps`, `-discovery`, `-transport-udp` and
  `-dcps` decide the week's result for these packages (a failing test fails
  the run, 60 s per test under instrumentation), and the weekly crate tier
  leaves out their default-feature jobs, which ran the same tests a second
  time (294 s in pipelines 9186 and 9187; maintainer's decision,
  2026-11-04). Feature runs stay in the crate tier; the merge lane is
  unchanged.

- **ci-interop on the pinned toolchain**: the image of the vendor jobs is
  rebuilt on ci-rust with rustc 1.97.1 (pipeline 9198); built on 1.88, every
  `vendor-live` job downloaded the toolchain of `rust-toolchain.toml` first,
  6.0 minutes of the CI runner host in the weekly run 9186. `.vendor-live` and the byte
  gate take the new digest.

- **Vendor pins and when they move** (`internal/release/release-playbook.md`,
  "Vendor pins"): every vendor version is pinned in one place per artifact
  and moves before a release candidate run (Cyclone `master`), on a new
  release of the vendor, or by hand for a fix, with the result of every
  case recorded against the pin before (`tests/interop/omg/compare_runs.py`).
  The weekly job `omg-vendor-pins` (`ci/vendor_pins.py`) prints how far
  each pin lies behind its vendor and turns nothing red.

- **Tests without a CI job have one.** The Flutter app of the dds-chat
  tutorial: the weekly job `integration-flutter`
  (`tests/integration/flutter_chat.sh`, the Flutter SDK from
  `tests/integration/provision.sh flutter`, JUnit from
  `ci/flutter_junit.py`) runs its dart:ffi loader between two Dart processes
  on a leased domain and its codec against octets spelled out from DDS-XTypes
  1.3. The TypeLookup service and the non-assignable type pairs against
  Cyclone DDS: `crates/dcps/tests/cyclone_live_type_lookup.rs` in
  `vendor-live-dcps` (a type with a member appended fetched and assignable in
  both directions, `label` as a string and a `@final` writer under XCDR1 and
  XCDR2 refused with INCONSISTENT_TOPIC). The cross-ORB cells of the former
  `competitors/run_interop.sh` of `zerodds-corba-interop` as tests against
  omniORB and JacORB (`integration-bridges-vendor-corba`) and TAO
  (`vendor-live-corba-tao`), built by `competitors/build_peers.sh`, with the
  AMI client against omniORB, JacORB's OTS context and the Features and
  Shapes operations against omniORB's stubs and skeletons.
  `website/_tools/gen_navmap.py --check` runs in `merge-docs` for a change
  under `website/` or `man/man1/`; without the zerodds-examples checkout it
  reads `website/_data/navmap-examples.json`, which it holds to the checkout
  where one exists.

- **The bindings over `libzerodds` exchange samples with a Rust process**
  (zerodds-ffi-loader-1.0 §5 and §8.3, OPEN-ITEMS 46). A subscriber in C,
  C++, Python (the ctypes loader), TypeScript (the koffi binding) and C#
  (`DllImport`), each in a process of its own, takes the 16 samples of a Rust
  publisher in another process on a leased domain and prints their bodies;
  `tests/integration/tests/bindings_ffi.rs` compares them with the XCDR2
  bodies DDS-XTypes 1.3 §7.4.3.5 gives, stated before the run: count,
  content, order. New weekly job `integration-bindings-ffi`
  (`tests/integration/ffi_peers.sh` builds the subscribers).

- **reliable-endpoint-1.0 §5 point 4 for Ada, Ada 83, native Ada, Kotlin,
  Node and Rust** (OPEN-ITEMS 44): `reliable_5_4_<lang>_the_producer_enqueue_costs_less_than_the_inline_send`
  runs each SDK's bench mode and checks that the decoupled enqueue is below
  the inline send. Ada 83 has a decoupled writer now
  (`endpoints/ada-83/src/zerodds_ada83_async_writer`, an Ada 83 task
  draining the SPSC ring `zerodds_ada83_ring`).

- **The five runtime operations no corpus case reaches are checked in all 20
  runtimes** (OPEN-ITEMS 30): `write_pid_extended` against Cyclone DDS's
  recording of `annotation-autoid-sequential`, `write_union_overlay` against
  Cyclone DDS 11.0.1's union storage, `reader_clear_discard`, and
  `reader_save`/`reader_restore` over the counting pass of a delimited map.
  `crates/lang-rules/tests/codec_backend_prerequisites.rs` generates four
  declarations that reach the five in every column and holds every runtime
  to publishing them.

- **`ci/coverage_results.py`: every test the coverage documents cite,
  against its result in CI.** It reads the job logs and JUnit reports of the
  pipelines of `main` from the GitLab API (nextest's and libtest's status
  lines, CTest, the steps of `ci/run_integration.py`, the summary of
  `tools/endpoints-gate`, node's test runner) and reports per document the
  items whose cited tests are green, red or skipped, or ran in no job of the
  new suite; `--test-run` prints a document's Test-Lauf block. The 15
  coverage documents without a Test-Lauf line have it now, and `merge-docs`
  declares no finding (OPEN-ITEMS 34). A pipeline started by hand with
  `MERGE_PACKAGES="<package> ..."` runs the crate tier of the named packages
  (`ci/merge_lane.py`): 45 packages whose crate tier had run in no pipeline
  since the merge lane began ran in pipeline 8996, all 45 crate-tier jobs
  green. `ts-node-test` runs `runtime/typescript/run-tests.sh`, which no job
  ran and `dds-ts-1.0.md` cites.

- **The C#, Java and Kotlin column tests of `zerodds-wire-probe` read and
  write the octets of `tests/interop-corpus/expected-bytes.tsv`** (OPEN-ITEMS
  83). They held the columns against the Rust column's recordings, and the
  Java round trip read back only what the Java column wrote. Now
  `csharp_column_gate` and `java_column_gate` run the probe under the
  dialect of the table's row and compare with its octets, `java_reads_the_table`
  (the round trip before) and `kotlin_bounded_collections` read a vendor's
  octets back to the case's sample; one test per case. They need `dotnet`, a
  JDK or `kotlinc` and run in the codegen jobs of their language
  (`ci/integration_scenarios.py`, `CODEGEN`), not in the crate tier, where
  they said SKIPPED. On the Linux test host: Java 1.4 to 7.5 s, Kotlin 17.8 s
  per test. `primitive-longdouble` has no row in the table and left both
  write checks.

- **`integration-ts-wasm`: the JavaScript tests of `zerodds-ts-wasm` in
  CI.** `tests/integration/ts_wasm.sh` builds the crate for
  `wasm32-unknown-unknown`, generates the glue for Node and the web with the
  wasm-bindgen CLI (the version of `Cargo.lock`; target and CLI from
  `tests/integration/provision.sh wasm`, the archive pinned by its sha256)
  and runs `npm test`: the codec through the binding and the
  DCPS-over-WebSocket facade of DDS-TS Annex C, 7 tests in 14.4 s
  (pipeline 8879). Weekly and on a change to the crate. The glue for Node
  is CommonJS and gets a `package.json` that says so, as wasm-pack writes
  it; under the crate's `"type": "module"` Node read it as an ES module
  without exports.

- **The C and the Python column hold their octets against
  `tests/interop-corpus/expected-bytes.tsv`** (decision 3 of
  `internal/release/rc8-test-suite.md`; OPEN-ITEMS 82).
  `tools/wireprobe/wireprobe-zerodds-{c,python}-gen/tests/expected_bytes.rs`
  run each column's matrix driver per configuration (new option
  `--encoding`) and hold the payloads to the vendors' octets and the
  column's read of them to the case's sample, through
  `zerodds_wire_probe::expected`; one test per configuration, 2 to 8 s each.
  `crates/wire-probe/tests/three_column_gate.rs`, which compared the three
  columns with each other in no tier, goes with its record;
  `tools/wireprobe/three-column-gate` only writes the two columns for
  `tools/interop-results`. Found and fixed: the Python generator lowered
  the IDL without the dialect's answer on a declaration named like its
  enclosing scope, and the C driver read the C column's octets back
  against a model lowered the same way, so `resolve-module-named-as-type`
  was refused under Fast DDS, RTI and the ROS middlewares, which write it.

- **The tiers of the Java PSM, the C# binding and the Gradle plugin
  (`rc8-test-suite.md`, "Crate tier" point 3).** A change to
  `crates/java-omgdds` (or to the Java sources its POM compiles in) runs
  `merge-java-omgdds`, the JUnit tests under Maven; a change to `crates/cs`
  or `zerodds-c-api` runs `merge-cs`, the C# binding's xUnit tests against
  `libzerodds`; a change to `build-integration/java-gradle` runs
  `merge-java-gradle`, which builds the Gradle plugin and runs its sample.
  `ci/foreign_tests.py` lists the tests a Maven or `dotnet` step runs by
  name, `ci/run_integration.py` holds the Surefire reports and the TRX log
  to that list, and `ci/cutover_gate.py` counts them as run. Weekly:
  `integration-java-psm` (the round trip of `GrpcBridgeClientTest` against
  the gRPC bridge daemon, a declared skip before),
  `integration-java-typelookup` and `integration-java-gradle`.
  The cutover list of the three is empty; `endpoints-java-async` and
  `cs-smoke` left `ci/full.yml`.

- **The platform jobs (`rc8-test-suite.md`, decision 6).** The core crates
  (`zerodds-wire`, `-types`, `-rtps`, `-discovery`, `-transport-udp`,
  `-dcps`) run as the crate tier runs them on Linux arm64 (the macOS runner,
  in colima's VM) and on Windows, weekly, on a release candidate and
  alone with `PLATFORMS=1`; `zerodds-transport-shm` runs on macOS on merge
  and weekly, so its two flink-sweep tests of `zerodds-shm-transport-1.0`
  §5.5 run in a tier and the crate left the `test` job.
  `ci/platform_jobs.py` holds the table, `ci/cutover_gate.py` counts a
  platform job's tests under that platform's `cfg`. The first runs found a
  late LIVELINESS_LOST (below) and two tests of `zerodds-dcps` that did not
  hold on Windows (a path built with a slash, a scan that read each file
  once per function and ran past its 25 s).

- **The integration pillar (`rc8-test-suite.md`, "Pillars → Integration").**
  `tests/integration` runs ZeroDDS end to end in processes of its own,
  each scenario on a leased DDS domain, waiting on events instead of
  sleeping, against an oracle outside the code under test: DCPS over UDPv4,
  TCPv4, Unix domain sockets, the same-host shared memory channel and TSN
  on a veth pair, with the kernel's view of the sockets and frames; writer
  restarts (killed, deleted, best effort, four generations, two live
  writers); DDS-Security 1.2 with key material made by `openssl` and the
  sent bytes recorded by `strace`; the durability and routing services as
  daemons; the MQTT, CoAP, WebSocket, gRPC, AMQP, OPC UA and CORBA bridges
  against Mosquitto, libcoap, `websockets`, grpcio, RabbitMQ with pika,
  open62541 and omniORB; the endpoint SDKs of all 21 languages at runtime;
  the C and C++ SDK through CMake, CTest and an installed-package consumer;
  the safe-profile gates of `docs/architecture/04_safety_by_architecture.md`
  §3. `ci/integration_scenarios.py` places each job (weekly, release
  candidate; on merge where the merge pipeline stays within its budget),
  `ci/run_integration.py` runs one.

- **The speed setup and its baseline (`rc8-plan.md` D1, D2).**
  `tests/perf/speed/speed.py` measures ZeroDDS, Cyclone DDS, Fast DDS, RTI
  Connext, OpenDDS and OpenSplice the same way on one host: each stack's
  self pair over UDPv4, 64 B, 1 KiB and 64 KiB, BEST_EFFORT and RELIABLE
  with KEEP_ALL, throughput, share received, CPU per sample on both ends,
  latency p50/p99/p99.9. The apps of `tests/perf/dds-roundtrip-bench` got
  the throughput modes `tput-pub`/`tput-sub` and take their QoS from the
  environment; the sequence bound of `roundtrip.idl` is 65 536.
  `tests/perf/baseline/speed.json` holds ten runs per value of rc.7 and
  `main` and five per vendor; `ci/jobs/speed-pillar.yml` measures weekly and
  on a release candidate on the exclusive host and reports against it.

- **Expected wire bytes from the matrix (`rc8-test-suite.md` decision 3).**
  `tests/interop-corpus/expected-bytes.tsv`, written by
  `tools/expected-bytes/generate.py` from one `zerodds-matrix` commit, holds
  the octets other implementations wrote for 417 corpus cases per ZeroDDS
  dialect; the oracles are the vendors' columns and, for `specification`,
  the octets the DDS implementations agree on. `cargo test -p
  zerodds-wire-probe --test expected_bytes` checks the reflective codec
  against it in both directions.

- **The OMG interoperability suites in CI.** The merge lane runs the
  dds-rtps suite against Cyclone DDS in both directions when `wire`, `types`,
  `rtps`, `discovery` or `security-*` change (`ci/interop_scenarios.py`);
  `ci/interop-omg-weekly.yml` runs dds-rtps, dds-xtypes and dds-security
  against the vendors in jobs of at most 120 s of cases, in groups picked
  with `OMG_GROUPS`. The image `ci-omg` carries the suites and the vendors'
  released executables; `tests/interop/omg/omg_suite.py` leases the DDS
  domains by the protocol of `crates/test-domains`.

### Fixed

- **Discovery asks the routing table once per destination, not once per pair
  of endpoints** (`zerodds-dcps`, OPEN-ITEMS 114). Choosing among the UDPv4
  locators a peer announced cost a socket, `bind`, `connect` and
  `getsockname` per locator for every pair of a local and a remote endpoint
  matched; in a ring of 20 processes with 40 writers and 40 readers each,
  that was 43 % of all CPU samples. The answer is kept 5 s per destination.
  Measured on the Linux test host in that ring, CPU seconds of all
  processes until every endpoint matched: 19.7–26.8 before, 8.8–11.3 after;
  time until matched 2.2–2.7 s before, 1.28–1.39 s after (4 runs each).

- **`zerodds-record`, `zerodds-spy` and `zerodds-recorder-bridge` take the
  history a TRANSIENT_LOCAL writer keeps** (`zerodds-cli-common`
  `monitor_reader_configs`, `MonitorReader`). Their readers were VOLATILE,
  and a VOLATILE reader is sent nothing written before it matched (DDS 1.4
  §2.2.3.4); since `9351ab2d37c` the runtime holds to that, and a recording
  started after its writers lacked the samples written before (OPEN-ITEMS
  113; a measured topic went from 14 samples per recording to 0). A third
  reader, RELIABLE and TRANSIENT_LOCAL, matches the writers that offer
  TRANSIENT_LOCAL or more; a sample that reaches several readers is kept
  once, by writer GUID and sequence number. Test:
  `dcps_2_2_3_4_the_recorder_records_the_history_of_a_transient_local_writer`.

- **`@key ::m::T id;` parses: an annotation name ends at a `::` with white
  space before it** (`zerodds-idl-parse`; zero-objects/zero-dds#62). IDL 4.2
  rules (4) and (218) make the line ambiguous; it read as the annotation
  `key::m::T` and failed with "expected a declarator name". White space
  decides it, as in Cyclone DDS's `idlc` (`scan_scope`); `@ann::mark(7)`
  stays one name. The preprocessor keeps the source's white space between
  tokens in its expanded text, so the composed path decides the same way.

- **Generated Rust compiles for a keyed type with a non-key `char` array**
  (`zerodds-lang-rules`; zero-objects/zero-dds#63). The key-only read
  dropped the array as `let _ = r.read_array_n(...)?`, and with no field to
  give `N` rustc stopped at `E0284`. The dropped read now writes each
  dimension out (`read_array`).

- **An integer `@default` on a `float` or `double` member is a
  floating-point literal in generated Rust** (`zerodds-lang-rules`;
  zero-objects/zero-dds#64). `@default(1)` gave `f32_value: 1`, which does
  not compile (`E0308`); it gives `1.0f32`.

- **The skeleton `zerodds-corba-rust` generates for an operation that raises
  every user exception of its interface has no unreachable `_` arm.** The
  shared `{Interface}Error` match took a wildcard for the variants other
  operations raise even when there were none, and rustc's
  `unreachable_patterns` stopped the generated crate under `-D warnings`.

- **`DataWriterImpl.retainForDurability` of the Java binding has one doc
  comment.** A second `/**` above it was attached to nothing, and javac 23
  and later refuse that under `-Xlint` with `-Werror` (`dangling-doc-comments`).

- **In the RTI/CoreDX minimal camp, the announced and served minimal hashes
  agree with the registry** (`zerodds-types` `TypeInformation::from_registry`,
  `from_component`; `zerodds-dcps` TypeLookup replies). They were taken in
  the default camp.

- **The end of a group coherent set is checked against its members'
  first sequence number** (DDSI-RTPS 2.5 §8.7.6; `zerodds-dcps`, and through
  it `zerodds-c-api`). The end-of-coherent-set DATA of a group coherent set
  carries `PID_GROUP_SEQ_NUM`, `PID_GROUP_COHERENT_SET` and
  `PID_WRITER_GROUP_INFO`; `PID_COHERENT_SET`, where the writer's part
  began, is on each member. ZeroDDS read an end without it as "this writer
  wrote nothing into the set" and discarded every set of Dust DDS 0.17.0 as
  incomplete. Measured in the official dds-rtps workflow of the ZeroDDS fork
  and on the Linux test host: Dust 0.17.0 -> ZeroDDS `Test_CoherentSets_12`
  and `_21` `DATA_NOT_RECEIVED`, the subscriber reporting
  `subscriber.group_order.incomplete_set_discarded` 63 times in one run.

- **`begin_access` presents one cut of the group** (DDS 1.4 §2.2.2.5.2.8;
  `zerodds-dcps` `Subscriber`, `zerodds-c-api` `zerodds_sub_begin_access`).
  Under GROUP access scope with ordered or coherent access, a coherent set
  that became complete while the application read its readers one after the
  other inside one `begin_access`/`end_access` bracket reached the readers
  read after that moment and not those read before. Measured in the official
  dds-rtps workflow of the ZeroDDS fork, `Test_CoherentSets_12` and `_21`
  ZeroDDS -> ZeroDDS: `DATA_NOT_CORRECT`, topic `Square` with the first set,
  `Square1` and `Square2` with the first two, in one read. `begin_access` now
  releases what is complete and opens the access; nothing more is released
  until the last `end_access`. On the Linux test host the 40
  `Test_CoherentSets_*` and `Test_OrderedAccess_*` cases ZeroDDS -> ZeroDDS
  passed five times in a row, 200 of 200.

- **A writer removes its expired samples before every send** (DDS 1.4
  §2.2.3.16). The removal ran only on the housekeeping tick, which comes late
  under load, and a repair sent on receipt of an ACKNACK did not look at the
  lifespan at all; a repair of an expired sample delivered it after its
  expiry. Measured on the Linux test host, OMG dds-rtps `Test_Lifespan_0`
  ZeroDDS -> Connext 7.7.0: SN 13 and 14 sent again 300 ms after their source
  timestamp with a lifespan of 250 ms. Removed, a repair of them is a GAP.
  `Test_Lifespan_0`/`_1` ZeroDDS -> Connext under load: 19 of 20 green (1 of
  9 red before); the one left is OPEN-ITEMS 109.

- **An OpenDDS reader's `TypeConsistencyKind` 2 reads as
  `ALLOW_TYPE_COERCION`.** OpenDDS numbers the kind one more than XTypes 1.3
  Annex D (`dds/DdsDcpsCore.idl`: `DISALLOW_TYPE_COERCION = 1`,
  `ALLOW_TYPE_COERCION = 2`), and its default reader announces `02 00 01 01
  01`. ZeroDDS read the unknown 2 as DISALLOW and dropped the three
  `ignore_*` flags after it, so a ZeroDDS writer called an OpenDDS reader of an
  assignable type inconsistent. The lenient decoder of the SEDP path reads 2
  as ALLOW, any other unknown value as DISALLOW, and keeps the flags; the strict
  `try_from_bytes` still refuses both. Measured on the Linux test host,
  dds-xtypes ZeroDDS -> OpenDDS 3.35.0-dev: 294 of 335 (234 before; CoreDX
  6.15.3 -> OpenDDS 291 in the official base), no case red that is green for
  CoreDX. OpenDDS -> ZeroDDS stays 231: OpenDDS reads the spec's 1 of a ZeroDDS
  reader as its DISALLOW, as it does for Connext, CoreDX and Cyclone readers
  in the base.

- **A peer's type that answers only in its COMPLETE form is resolved and
  decided.** A type missing under the identifier a peer announced is asked for
  under the COMPLETE identifier of the same TypeInformation as well, and the
  pair is decided by the complete identifier once that object and its closure
  are here; the assignability walk projects the pair down itself. XTypes 1.3
  §7.6.3.3.4 has a peer answer for any identifier it announced; Dust DDS
  0.16.0 files only COMPLETE TypeObjects, leaves a request for its MINIMAL
  identifier unanswered, and its minimal hash agrees with neither computation
  of `docs/omg-xtypes-issues.md` C2. Measured on the Linux test host, every
  dds-xtypes case with the client of the OMG pull request, no flags: Dust
  0.16.0 335 of 335 in each direction (305 before); HDDS 1.3.0 335/335 both
  ways; CoreDX 6.15.3 332 and 335; Cyclone 11.0.1 328 and 321 with 7 declared
  deviations; Connext 7.7.0 307 both ways; OpenDDS 3.35.0-dev 231 and 234 —
  every red case Connext's, Cyclone's or CoreDX's in the official base, the
  OpenDDS pairs as in the GitHub run of 2026-11-07.

- **A TypeLookup request that names its own sender as the service is
  answered.** XTypes 1.3 §7.6.3.3.4 gives the `instanceName` as
  "dds.builtin.TOS." and "the DomainParticipant GUID" without saying whose.
  ZeroDDS names the server and answered only a request naming this
  participant or none (all zeros); Dust DDS 0.16.0 names itself, and its
  requests were refused, so Dust matched no ZeroDDS endpoint of a type it had
  to look up. A request naming its own sender is for no other service and is
  answered now. Measured: OMG dds-xtypes Dust -> ZeroDDS 258 cases red in the
  fork run of 2026-11-07; on the Linux test host with the change 305 of 335
  green in each direction, the 30 left a type ZeroDDS cannot obtain from
  Dust 0.16.0 (it files only COMPLETE TypeObjects and leaves the request for
  the MINIMAL identifier it announces unanswered). Filed as
  `docs/omg-xtypes-issues.md` B10.

- **A participant pinned to an interface names it as its SPDP multicast
  device** (`IP_MULTICAST_IF`). It bound the sender to the pinned address
  only; Linux picks the multicast device by the source address, macOS by
  the route, so on macOS a participant pinned to 127.0.0.1
  (`ZERODDS_INTERFACE`) sent its SPDP and the wake-up datagram of its
  shutdown through the route's interface, and a receiver joined on
  127.0.0.1 got neither. Closing a socket does not wake its receive thread
  on macOS, so every shutdown waited out the SPDP multicast receiver's 1 s
  read timeout: 0.74–0.84 s per process, measured on a MacBook Pro in a
  ten-process chain. On macOS loopback a sender bound to 127.0.0.1 reached
  a receiver joined there with 0 of 200 datagrams without the option and
  200 of 200 with it. Where a platform refuses the option the sender binds
  as before and the event `discovery.spdp_multicast.sender_refused` says
  so.

- **The default PL_CDR1 reader takes either setting of the must-understand
  bit on `PID_EXTENDED` and `PID_LIST_END`**, as DDS-XTypes 1.3 §7.4.1.2.1
  has every reader do ("shall be robust to any setting of the
  FLAG_MUST_UNDERSTAND bit and accept the parameter nevertheless"). CoreDX
  6.15.3 closes a `@mutable` sample under XCDR1 with a bare `3f02`; the
  default reader refused it, and six OMG dds-xtypes cases with CoreDX as
  publisher ended in `DATA_NOT_CORRECT`. They pass now (Linux test host).
  Another `FLAG_IMPL_EXTENSION`, and a declared length rounded up to four,
  still need `wire.pl_cdr1_reading = tolerant`.
- **The OMG dds-xtypes client reads received samples under the run's
  `wire.pl_cdr1_reading`** and reports why a sample does not decode. Its
  codec took the specification's framing whatever the configuration said,
  so the setting `run_configs.sh` gives never reached the samples. Cyclone
  11.0.1's rounded PL_CDR1 length is a declared deviation under `-x 1` in
  `tests/interop/omg/vendor-deviations.tsv`, which gains an optional column
  for the harness arguments a row holds for.

- **Samples of an RTI Connext writer whose type ends in a one- or two-octet
  member were refused**: the reader held the payload against the writer's
  `dataMaxSizeSerialized` (DDSI-RTPS 2.5 Table 8.72) with the padding octets
  the encapsulation options count. DDS-XTypes 1.3 §7.6.3.1.2 puts them after
  the end of the serialized payload, and ZeroDDS' own writer announces the
  header and the data without them. Connext 7.7.0 announces 5 for
  `@final struct { uint8 x1; }` and sends 8 octets, three declared padding;
  OMG dds-xtypes, Connext as publisher: `struct_uint8_uint8` and seven other
  cases ended in `DATA_NOT_RECEIVED` in all four configurations and pass in
  32 of 32 runs (Linux test host). The refusal of a payload beyond the bound
  is no longer silent: it counts as SAMPLE_LOST (DDS 1.4 §2.2.4.1; it was
  marked irrelevant, which RTPS leaves out of the lost changes), and the
  event `reader.sample.oversize_refused`, once per reader and writer, names
  the bound.

- **A `shape_main` reader without `-b` is RELIABLE**, as in the reference
  client: OMG dds-rtps `srcCxx/shape_main.cxx` sets `reliability_kind =
  RELIABLE_RELIABILITY_QOS` in `ShapeOptions()` and applies it to the writer
  and the reader; only `-b` gives BEST_EFFORT. `shape_main` left the DDS
  default when neither `-r` nor `-b` was given, and the default of a
  DataReader is BEST_EFFORT: in the 39 suite cases whose subscriber names
  neither flag the ZeroDDS reader was the one best-effort endpoint.
  Measured: `Test_FinalInstanceState_1` CoreDX -> ZeroDDS red in both runs
  of the whole suite (the zero-objects fork of dds-rtps on GitHub, the Linux
  test host), green in 16 runs outside it (11 alone, 5 after
  `Test_FinalInstanceState_0`); in the red run no HEARTBEAT or ACKNACK of
  the user writer was on the wire and CoreDX never sent its four dispose
  DATA. After:
  each of the ten products (ZeroDDS included) -> ZeroDDS 39 of 39 for the
  39 cases, the whole suite CoreDX -> ZeroDDS 110 of 110, and HEARTBEAT and
  ACKNACK of the user writer on the wire. `-b` with `-r` is refused. Test:
  `reliability_is_reliable_unless_b_for_writer_and_reader`.

- **`zerodds-lint check` reports no error**
  (`docs/architecture/04_safety_by_architecture.md` §3.4): of the 40, 24 were a method named `expect` in `zerodds-wire`'s
  codec that returns a `Result` (now `require`); a `dyn` became a generic
  in `zerodds-wire`, `zerodds-sql-filter`, `zerodds-lang-rules` and
  `zerodds-derive`; six recursive functions state their depth. Two files
  carry an exception marker with the reason: the key holder's held
  members in `zerodds-wire` (`dyn`, XTypes 1.3 §7.6.8), and
  `encode_key_holder_be` of `zerodds-codec` (`panic!`, OPEN-ITEMS 35).
- **A BEST_EFFORT DataReader with finite RESOURCE_LIMITS holds no more
  than them while the application does not take** (`zerodds-dcps`,
  `ReaderRoom`, DDS 1.4 §2.2.3.18, §2.2.3.19): past the limit, the thread
  that queued the sample takes the reader's channel into its cache, whose
  admission discards the oldest. Before, the channel kept every sample with
  its datagram until the application took. Measured on the Linux test host
  (`zerodds_perf`, BEST_EFFORT KEEP_ALL, `max_samples` 100 000, 1 KiB,
  15 s, the subscriber taking every 3 s): peak 310 MB instead of 6.88 GB;
  the receive thread admits each sample past the limit itself and no longer
  keeps up, 791 374 datagrams dropped by the kernel instead of 5 523
  (OPEN-ITEMS 50). A RELIABLE reader is unchanged.
- **A DataReader with a full cache counted it again for every sample it
  admitted** (`zerodds-dcps`, `DataReader` admission, DDS 1.4 §2.2.3.19
  RESOURCE_LIMITS `max_samples`): the count of samples with data is now
  taken once per run under the cache lock and kept up to date. Measured on
  the Linux test host (`zerodds_perf`, BEST_EFFORT KEEP_ALL, `max_samples`
  100 000, 1 KiB, 4 s at 600 kS/s): before, 1 199 631 of 1 500 473 samples
  taken, a 22 s stall and 80.7 % of the subscriber's CPU in the admission;
  after, 2 395 136 of 2 395 136, the admission 3.4 %. Over 15 s the
  subscriber had run into an 8 GB limit; now 9 148 976 of 9 149 184 samples
  arrive. OPEN-ITEMS 50 (a `take` of the full cache took 47 s) is closed.
- **A RELIABLE writer sent changes again that no reader had asked for.**
  An ACKNACK whose SequenceNumberSet was empty requested every change past
  its base (`ReliableWriter::handle_acknack_window`), also the changes still
  on their way to the reader; DDSI-RTPS 2.5 Table 8.66 T8 requests what the
  set names and nothing else. Each ACKNACK that answered a HEARTBEAT while
  samples were in flight sent them all again, with a HEARTBEAT that drew the
  next ACKNACK. Measured on the Linux test host with `zerodds-roundtrip`
  (`tput-pub`/`tput-sub`, RELIABLE, KEEP_ALL, 64 KiB, 5 s,
  `reader.heartbeat_response_delay_ms` and `writer.nack_response_delay_ms`
  0), loopback capture: before, 816 samples (0.158 kS/s), 239 998
  datagrams, 6.98 GB of UDP payload, each DATA_FRAG sent 130 times on
  average and at most 234 times, 4.67 ms of CPU per sample at the reader;
  after, 76 240 samples (15.2 kS/s), 287 590 datagrams, 5.72 GB, each
  DATA_FRAG 1.23 times on average and at most 3 times, the repeats asked for
  by NACK or NACK_FRAG, 74 µs per sample. Test:
  `crates/rtps/src/reliable_writer.rs::tests::rtps_8_4_9_2_an_acknack_with_an_empty_set_requests_no_change`.

- **A nested key member's KeyHash in the Rust code of `zerodds-build` and
  in `derive(DdsType)`** (DDS-XTypes 1.3 §7.6.8 step 3). A struct or union
  inside a key holder was written by its data codec, with its DHEADER, its
  EMHEADERs and its non-key members: `@key Inner who` over an
  `@appendable Inner { long x; string<8> tag; }` gave the holder
  `00 00 00 0a 00 00 00 03 …` where the specification and
  `zerodds-wire-codec` give `00 00 00 03 …`; over a `@mutable` nested
  type with its own key the holder carried every member. Each aggregate
  inside a key holder is now its own holder: `@final`, its key members
  alone when it declares any (`encode_key_holder`/`decode_key_holder` in
  the generated code, `HolderPlan` in `zerodds-codec`). The KeyHash of
  such a type changes; a type whose nested key members are `@final`
  structs without own keys keeps it. Test:
  `crates/zerodds-build/tests/dds_type.rs::xtypes_7_6_8_a_nested_key_member_is_its_own_final_key_holder`.

- **The generated key-holder reader of a type with a struct-typed key
  member refused every holder.** `key_members_with` names the key members
  of the nested struct too; the reader looked each of them up as a member
  of the outer type, found none and generated a reader that refuses
  (`a key holder with a member this emitter cannot read`). It reads only
  the type's own members now, the nested ones with their struct
  (`crates/lang-rules/src/encode.rs`). Found by the equivalence tests of
  `derive(DdsType)`, which read a struct-typed key back on both routes.
  Test: `crates/zerodds-build/tests/dds_type.rs::xtypes_7_6_8_a_struct_typed_key_member_is_read_back_from_the_key_holder`
  (integration-codegen-build).

- **One simple type name in two IDL modules compiles through `zerodds-build`**
  (OPEN-ITEMS 56). The constants of `type_objects_<unit>` and
  `type_identifiers_<unit>` were named after the type's simple name in one
  flat module, so `module v1 { struct Part … }; module v2 { struct Part … };`
  gave `pub const Part` twice and `error[E0428]`. Each constant now sits in
  one `pub mod` per IDL module, as the type does
  (`type_objects_<unit>::v1::Part`); a constant of a type at file scope
  keeps its path. Tests:
  `crates/zerodds-build/tests/reported_issues.rs::idl_4_2_7_5_2_one_simple_name_in_two_modules_is_two_constants`,
  and `crates/zerodds-build/tests/dds_type.rs::xtypes_7_2_3_a_nested_appendable_type_reads_a_newer_and_an_older_writer`,
  which now builds a crate around `v1::Part` and `v2::Part`.

- **The generated Rust decoder of an `@appendable` struct steps over the members a newer writer appended** (`crates/lang-rules/src/encode.rs`, `decode_in_plan_order`). Under XCDR2 it stopped after its own last member; a type nested in another then read the enclosing type's next member out of the unknown ones, with no error. Measured with `EvoOld { PartOld piece; long tail; }` reading the bytes of `EvoNew`, whose `PartNew` has a third member `short c = -3`: `tail` came back as 65533 instead of 42. The reader now moves to the DHEADER's end, which also refuses a reader that read past it, as the other columns do (`emit/decode.rs`, `close_delimiter`). Tests: `crates/zerodds-build/tests/dds_type.rs::xtypes_7_2_3_a_nested_appendable_type_reads_a_newer_and_an_older_writer` (both directions), `crates/lang-rules/tests/codec.rs::xtypes_7_2_3_an_appendable_reader_steps_over_the_members_a_newer_writer_appended`.

- **`speed-async-write` measured a tree no pipeline had built for it**:
  after a failed `speed-build` it ran `perf_targets` in the speed store's
  checkout of an earlier pipeline and passed (weekly run 9186, job 231135);
  it refuses now without this pipeline's build, as the measuring steps do.

- **`runtime_drop_threads` (zerodds-dcps) counted the lease heartbeat
  thread as one of the runtime's**: the thread names itself once it runs,
  and under load it still carried the process's name in the test's
  snapshot (job 231306). Threads are told apart by task id.

- **`zerodds-lint` and `rustdoc` on `zerodds-types`**: a `&dyn Fn` in a
  SAFE crate and an intra-doc link read from "[its]" in a quotation, both
  red in weekly run 9186 (jobs 231200, 231202).

- **A TypeLookup reply reaches the Cyclone DDS reader that asked for it.**
  The TypeLookup service of `zerodds-dcps` sent a written request or reply
  and the HEARTBEATs of its tick after it had released the transport, so a
  HEARTBEAT could announce a reply before its DATA left. Cyclone DDS 11.0.1
  starts a VOLATILE reader at a writer's first HEARTBEAT and dropped the
  reply; the match it had deferred on the writer's type never came
  (`vendor-live-dispose-bridges` red in pipeline 9149; the Cyclone test of
  `zerodds-zenoh-bridge`'s `dispose_reaches_a_foreign_reader` failed 8 of
  300 times with Cyclone's trace, pipeline 9171). Both now send while the
  transport is held: 0 of 490 runs with the trace and 0 of 469 without
  (pipelines 9176 and 9177), and the five TypeLookup tests against Cyclone
  of `cyclone_live_type_lookup` pass.

- **The Java PSM counts the endpoints of other processes in
  SUBSCRIPTION_MATCHED and PUBLICATION_MATCHED** (DDS 1.4 §2.2.4.1,
  OPEN-ITEMS 30). Both statuses counted the endpoints of their own process
  only: a writer or reader matched over SEDP was in
  `getMatchedPublications`/`getMatchedSubscriptions` and not in
  `current_count`, `total_count` or the change counts, and no listener was
  told of it. The RTPS participant reports every match and its end
  (`RtpsParticipant.MatchListener`: SEDP match, lease expiry, ignore, no
  common representation) in order on a thread of its own, and
  `DataReaderImpl`/`DataWriterImpl` count and report them as they do the
  endpoints of their own process. `tests/integration/tests/bindings_ffi.rs`
  checks both sides against a Rust writer and a Rust reader, the match and
  its end by `ignore_publication`/`ignore_subscription` (job
  `integration-java-psm`); the Java subscriber of zerodds-ffi-loader-1.0 §5
  waits on SUBSCRIPTION_MATCHED again.

- **The dds-chat tutorial writes XCDR2 as DDS-XTypes 1.3 gives it.** Its
  three `@appendable` types were written with `unsigned long long` aligned
  to eight under the header CDR2_LE, beside a non-standard "XCDR1-Compact"
  form, and the fixtures came from the Rust port's own codec. Every port
  (Rust, C++, C#, Java, Python, TypeScript, Dart) now writes D_CDR2_LE with
  the padding count in the options, the DHEADER, members aligned to at most
  four and the `<is_present>` octet of an optional member;
  `shared/fixtures/fixtures.py` derives the fixtures from the rules. Over
  DDS the writers are declared appendable and offer XCDR2, and hand the
  runtime the stream after the header (the C-API ports put the tutorial's
  own header into the payload before, so the wire carried two). The Flutter
  loader declares the fourth parameter of `zerodds_reader_take`. Chapter 10
  implements the current `AccessControlPlugin`, which the tutorial crate
  had not compiled against; its lock files are current.

- **idl-lower: the two readings of an omitted annotation member, and an
  `any` member.** An annotation applied without a value for a member that
  declares no default is refused under `--strict`, as OMG IDL 4.2
  §7.4.15.4.2 requires, and read with a warning otherwise, as DDS-XTypes 1.3
  §7.3.1.2.5 gives it (it was accepted silently in every mode). An
  annotation member of type `any` (IDL 4.2 rules 223, 224) is read with a
  warning, as DDS-XTypes 1.3 §7.3.1.1.1 asks for a construct outside its
  profile; it was refused.

- **`zerodds-ts-wasm` speaks the binary frames of zerodds-ws-bridge-1.0**
  (OPEN-ITEMS 40). Its WebSocket transport sent JSON messages
  (`{"op":"subscribe","topic"…}`, `publish` with base64 data) the daemon
  `zerodds-ws-bridged` does not have, and waited for a `notify` it never
  sends, so the browser DCPS client exchanged no sample with a bridge. It
  opens a connection per topic on the topic's path (§4.2, the slug of
  §5.1), sends and reads the frames of §4.3 (`ZDB1`, flags, encapsulation
  header, payload), takes the byte order from the header and the instance
  state from the flags, and keeps the daemon's `error` messages (§4.4). The
  base64 helpers are gone from the package's exports.

- **Every test that binds a socket leases its DDS domain** (OPEN-ITEMS 63).
  `simulation_clock_e2e` ran on domain 93 with fixed GUID prefixes,
  `participant_config_user_data` on 173, and about 230 call sites in
  `zerodds-dcps`, `zerodds-c-api` and `zerodds-dcps-async` on literal
  domains, which the same test on another host of the LAN shares at the same
  time. `domain_hygiene` follows a literal through a `let`, a `const` or
  `static` and a helper that forwards it, for the factory,
  `DcpsRuntime::start` and the six C entry points, and takes 0.75 s instead
  of 2.6 s on the CI runner host (pipelines 9094 and 9051).

- **The tests of `zerodds-dcps` that start a `DcpsRuntime` shut it down on
  their own thread** (OPEN-ITEMS 39). 27 of its 1583 test processes ended
  with threads of a runtime still running; none does now (pipelines 9094
  and 9113), and the five weekly coverage jobs had no negative line count in
  three runs (pipelines 9114, 9117, 9124).

- **The destroy test of `zerodds-c-api` checks what destroy promises since
  the runtime's threads hold it weakly**: the caller's clone is the only
  strong reference left and the threads end. It asserted more than two
  strong references and failed since `6c018d1662a` (job 230640).

- **The Node and TypeScript runtimes wrote PID_EXTENDED without the
  must-understand bit by default.** Their `PL_CDR1_DEFAULT` used no PID flags;
  `zerodds_wire::PlCdr1Framing::DEFAULT` and the other 18 runtimes set the
  bit on the extended marker and on the sentinel, which is what Cyclone DDS
  recorded (`017f0800`). Found by the new self-test vector of
  `write_pid_extended`. A generated codec passes its framing and was not
  affected.

- **F#: `ReaderSave`/`ReaderRestore` did not carry the reader's status**, so
  a refusal inside a counting pass outlived the restore. `ReaderMark` carries
  it now, and `ReaderClearDiscard` returns whether it cleared.

- **The reliable end-to-end tests of the endpoint SDKs use generated code**
  (OPEN-ITEMS 31). All 21 `crates/endpoint-e2e/tests/*_reliable.rs` build
  their app from the `Ping` the language's generator writes from the IDL of
  `crates/endpoint-e2e/src/codegen.rs` and the SDK's reliable writer, and
  `assert_reliable_pings` reads every delivered body by the IDL's XCDR2
  layout. The Rust app's generated codec is committed
  (`crates/endpoint-e2e/tests/generated/pingpong_rust.rs`) and held to the
  generator's output.
  `endpoints/c/test/reliable_udp_app.c` and `endpoints/ocaml/reliable_app.ml`,
  the former senders that nothing builds any more, are removed.

- **The test `transient_backend_restore_keys_and_times_history_and_serves_every_late_joiner_once`
  of `zerodds-dcps` raced the runtime's own tick** (OPEN-ITEMS 60). It
  ticks the writer itself and looks at a reader proxy between a match and
  its own tick; a tick of the runtime's scheduler in between pushed the
  history to the new reader, and the proxy's `highest_sent_sn` read 4
  instead of 0. Red on Windows in the weekly run 9037 (job 230275). On the
  Linux test host 7 of 300 runs failed on `b1999073f5b` and 4 of 300 on
  `3be7f0f9a81`, before `6c018d1662a`; with a pause of 300 ms after the
  second match every run failed on both. The test starts its runtime with
  `external_tick` now, so no tick but its own runs: 300 of 300 runs pass,
  with the pause 5 of 5.

- **The integration pillar's nextest steps reach GitLab's test report.**
  nextest writes its JUnit file below the workspace's `target/nextest`, not
  below `CARGO_TARGET_DIR`, where `ci/run_integration.py` looked; no job of
  the pillar had a JUnit report (pipeline 8967).

- **`zerodds-types`: a plain collection of a plain collection of a hashed
  type said `EK_BOTH` in its header.** XTypes 1.3 §7.3.4.6.1 gives
  `EK_BOTH` to a collection whose element (and key) identifier is fully
  descriptive; `from_ast`, the minimal projection and the TypeObject v1
  conversion gave it to every element that was a plain collection, so
  `sequence<sequence<S>>` had a TypeIdentifier no other implementation
  computes (CycloneDDS writes `EK_COMPLETE`, case `sequence-nested-struct`).
  `TypeIdentifier::is_fully_descriptive` decides it in all three.

- **`zerodds-dcps`: a participant that sends no multicast stops without
  waiting a second** (OPEN-ITEMS 39). `DcpsRuntime::shutdown` woke its
  receive threads with a datagram to their own sockets, and the one to the
  SPDP multicast group went out only where the participant sends multicast:
  with `discovery.no_multicast`, or not yet enabled, the thread of the SPDP
  multicast socket waited out its 1 s read timeout, and so did every
  deletion of such a participant. The shutdown now also closes the receive
  sockets: `UdpTransport` implements `Transport::close`, which shuts the
  read side down and on Linux wakes a thread waiting in it at once (sending
  stays possible, the SPDP departure still goes out). A layered user
  transport ends a waiting receive on `close` as well. On the Linux test
  host the 145 tests of `zerodds-dcps --features security` took 118.0 s, 49
  of them 1.0 to 1.1 s each, and take 57.8 s now, none of them in that
  range.

- **`zerodds-dcps`: `delete_participant` returned while a listener of the
  participant was still running.** The thread that calls the status
  listeners was told to stop and not waited for; it is joined now, so a
  listener call in progress has returned when the deletion returns and none
  follows. A thread of the runtime that ends up deleting its own
  participant (a listener that deletes it) joins every thread but itself.
  `participant_deletion_threads.rs` checks it, and on Linux that every
  thread the participant started has ended.

- **`vendor-live-corba-tao` is back.** The TAO 2.5.24 clients of
  `zerodds-corba-dds-bridge`'s `tao_e2e` ended with `SIGSEGV` in the interop
  image: its open-file limit is 1073741816, TAO sizes its tables by it and
  the client's transport cache was not allocated (gdb: `bind_i` of
  `TAO::Transport_Cache_Manager_T`, pipeline 8912, job 229335). With
  `ulimit -n 1024` both tests pass (pipeline 8916); the test now starts the
  TAO programs that way, the job runs them weekly again (pipeline 8917, job
  229347: 2 tests in 18.3 s), `zerodds-corba-bridge-1.0` §10 and §12.3 cite
  them (OPEN-ITEMS 101).

- **`bench-regression` passed a cell that measured nothing.** A ping
  without an echo ends at its maximum runtime with an empty histogram whose
  quantiles read 0; the SHM cells stood at 0.00 µs as OK (pipeline 8827).
  The gate counts the samples now and fails a cell with fewer than half of
  them (SETUP_FAIL): the SHM cells get 0 of 30 000 (pipeline 8888,
  OPEN-ITEMS 41).

- **The Gradle plugin of `build-integration/java-gradle` did not compile.**
  Its Kotlin used `it` in `Action` lambdas, which `kotlin-dsl` gives their
  argument as the receiver; it had never been built (its README said so).
  Built with Gradle 8.10.2 in `merge-java-gradle`, and the sample holds the
  octets of its generated type to DDS-XTypes 1.3 §7.4 instead of round-tripping
  it; the sample compiles for Java 17 with the JDK Gradle runs on instead of a
  toolchain of a fixed version.

- **`zerodds-dcps`: LIVELINESS_LOST of a new writer comes when its lease
  runs out.** Registering a user writer did not wake the scheduler's
  housekeeping pass; with nothing pending before, a silent MANUAL_BY_TOPIC
  writer was counted up to 250 ms (the pass's idle park) after its lease ran
  out (DDS 1.4 §2.2.4.1). Found by the first run on Linux arm64.

- **`rmw-zerodds-shim` and `zerodds-opcua-bridge` keep the header of a
  sample's type.** The shim handed every taken sample (and every service
  request and reply) on under a header rebuilt from the XCDR version and
  the byte order, PLAIN_CDR2 also for a sample that came as D_CDR2 or
  PL_CDR2; the OPC UA bridge wrote its XCDR2 bodies without an XCDR2
  offer and under the header of a `@final` type whatever the topic type
  was (DDS-XTypes 1.3 §7.6.3.1.2, Table 60). `zerodds-dcps`'s
  `SampleOrigin` and the C API's `ZeroDdsSampleOrigin` carry the
  representation identifier of the header for that; the shim takes it from
  there, the bridge sets the writer's offer and extensibility from the
  topic type.

- **`zerodds-opcua-pubsub`: a DataValue writes each timestamp followed by
  its picoseconds.** OPC 10000-6 Table 27 orders the fields Value, Status,
  SourceTimestamp, SourcePicoseconds, ServerTimestamp, ServerPicoseconds;
  the encoder and the decoder put both timestamps before both picosecond
  fields, so a DataValue with picoseconds and a server timestamp went out
  and was read with the fields swapped. Found by
  `uacs_5_2_2_17_a_data_value_is_its_mask_and_the_fields_in_the_order_of_table_27`.

- **`zerodds-opcua-pubsub`: the AES-CTR counter block starts at 1.** The
  BlockCounter of the counter block KeyNonce ‖ MessageNonce ‖ BlockCounter
  was 0 for the first block; OPC 10000-14 §7.2.4.4.3.2, Table 157, and
  RFC 3686 §4 start it at 1, so every encrypted payload of PubSub-Aes128-CTR
  and PubSub-Aes256-CTR used the keystream of another implementation shifted
  by one block. Found by the RFC 3686 test vectors #1 and #7.

- **`zerodds-opcua-pubsub`: UADP discovery messages carry the SecurityHeader
  and the ProbeType.** OPC 10000-14 §7.2.4.6.3 and §7.2.4.6.12.1 set the
  SecurityHeader bit of ExtendedFlags1 for discovery announcements and
  probes; the header was missing, and a probe went out without its
  ProbeType (Table 178) before the InformationType. Both now follow Tables
  168 and 178 to 180; a signed or encrypted discovery message is refused on
  decode.

- **`zerodds-opcua-bridge`: a PubSub publisher sends the connection's
  `publisher_id`.** The WriterGroup took the default
  NetworkMessageContentMask, which does not name the PublisherId, and the
  configured `opcua.pubsub.connections[].publisher_id` never went out
  (OPC 10000-14 §7.2.4.4.2 Table 154); a Subscriber that filters by
  PublisherId (§6.2.9.1) did not match the bridge. Found by the contract
  test with `zerodds-opcua-pubsub`.

- **`zerodds-webdds-bridge`: a type a peer answers at once is no longer
  missed.** `TypeRegistry::resolve_named` read the generation of the
  TypeLookup registry after it sent `getTypes`; a reply that landed in
  between was counted already, the generation did not move again, and
  `create_topic` of an unconfigured type ended in "never resolved via
  TypeLookup within the bound" with the type in the registry (merge
  pipeline 8660, `create_topic_resolves_an_unconfigured_type_via_type_lookup`).
  The generation is read before the first look into the registry, and the
  request, one datagram, is sent again every 500 ms until the reply is in.

- `zerodds-record` records a dispose with unregister of one instance
  (DDSI-RTPS 2.5 §9.6.4.9, the D and U bits of a status info) as two
  entries, `NotAliveDisposed` and then `NotAliveUnregistered`; it recorded
  the unregister alone and the dispose was lost (OPEN-ITEMS 46).

- **What the integration pillar found in the services and bridges.**
  `zerodds-durability-service`'s replay writer offered TRANSIENT_LOCAL, so
  a late TRANSIENT or PERSISTENT reader never matched it (DDS 1.4
  §2.2.3.4); it offers PERSISTENT. `zerodds-mqtt-bridge` published the CDR
  bytes without the encapsulation header and wrote a received header into
  the sample (zerodds-mqtt-bridge-1.0 §4.2). `zerodds-websocket-bridge`
  sent JSON text frames to every client and read binary frames as text; it
  sends binary `ZDB1` frames unless the client asks for
  `zerodds-ws-bridge-1.0+json` (§4.1, §4.3). `zerodds-grpc-bridged` refused
  the §3 keys `dds_type`, `grpc_package` and `qos`, answered `Subscribe`
  with one sample instead of a server stream, let two streams of one topic
  split its samples, and carried `cdr_payload` without its header (§3,
  §4.3, §4.4); every `Subscribe` stream now has a DataReader of its own.
  `zeroddsConfig.cmake` refused every consumer of an SDK built without
  idlc. The Nim reliable endpoint dropped ACKNACKs and ended at its 20 s
  safety valve.

- **`zerodds-routing-service` reads and forwards a sample in the format and
  byte order of its encapsulation header (DDSI-RTPS 2.5 §10.5).** The filter
  and the transform read every body little-endian, so a filter over a
  `CDR_BE` sample compared swapped values, and the output writer framed a
  forwarded body by its XCDR version alone: a `D_CDR2_BE` or `PL_CDR2_BE`
  body went out under `CDR2_LE`. A route, a relay route included, now gives
  its output the input's representation identifier (XCDR version, plain,
  delimited or parameter list, byte order); a transformed body is written
  in the input's byte order. A body a flat shape does not describe (a
  parameter list, or a DHEADER the shape does not expect, DDS-XTypes 1.3
  §7.4.3.4) is refused and counted as an error instead of read at wrong
  offsets. `SampleProcessor::process` takes the identifier and returns a
  `Processed`.

- **What the contract tests with `zerodds-dcps` found in its consumers.**
  `zerodds-durability-service` told its own replay samples apart by the
  writer's entity handle, which no `SampleInfo` carries; it takes
  `publication_handle()` (DDS 1.4 §2.2.2.5.5). `zerodds-mq` and
  `zerodds-monitor` end their participants with `shutdown()`, so the peers
  get the SPDP departure instead of waiting for the lease. The Python
  binding documented a listener mask of 0 as every status; it is none, as
  in `zerodds-dcps`.

- **What the contract tests of the core crates found in their consumers
  (`rc8-test-suite.md`, "Crate tier").** `zerodds-opcua-bridge` gave an
  unbounded sequence or map the bound 4294967295 (`LENGTH_UNLIMITED`,
  DDS-XTypes 1.3 §7.5.2.4.2), so the gateway announced ArrayDimensions
  `[4294967295]` instead of `[0]`, and it now enables the `dynamic` feature
  of `zerodds-types` it uses (as do `zerodds-record`, `zerodds-spy` and
  `zerodds-webdds-bridge`); `zerodds-corba-cos-transactions` wrote a
  non-nil IOR after another one without the alignment of CORBA 3.3 part 2
  §9.3.1.1; `zerodds-corba-interop`'s `decode_string_body` returns the
  error of a body that is no string instead of an empty one. Dependencies
  nothing in a crate's shipped code used are gone or moved to the
  dev-dependencies (`zerodds-wire` of `zerodds-wire-probe`,
  `zerodds-discovery` of `zerodds-admin`, `zerodds-types` of the CoAP, MQTT,
  WebSocket and CORBA bridges and of `zerodds-cli-common`,
  `zerodds-wire-codec` of `zerodds-recorder-decode`).

- **`chaos-clock-skew` drifts from elapsed time.** `CHAOS_CLOCK_DRIFT_PPM`
  used the time since the epoch scaled by 1e-9: 100 ppm was a fixed jump of
  about 176 s. The drift now accumulates from the first intercepted call
  (`elapsed_ns * ppm / 1e6`) in both `clock_gettime` and `gettimeofday`
  (`302e6aad1d0`).

- **The arm64 `zerodds-dev` package carried x86-64 static libraries.**
  `assemble-debs.sh` built the SDK for the host; it now cross-builds for
  `--target` and checks every `.a` against the package architecture
  (`de738d7e0c1`).

- **docs.rs could not build `zerodds-c-api` and `zerodds-bench-suite`.**
  The build script of `zerodds-c-api` rewrote `include/zerodds.h` on every
  build; docs.rs builds in a read-only source directory and the write
  failed (os error 30). The build scripts of `zerodds-c-api` and
  `rmw-zerodds-shim` skip the header under `DOCS_RS` and write it only when
  it changed. The binary-only tools (`zerodds-idlc`, `-replay`, `-admin`,
  `-recorder-bridge`, `-xmlc`) link their documentation to zerodds.org
  (`8dec7db570c`).

- **No `.rpm` for aarch64.** `publish-rpm.yml` built x86_64 only; aarch64
  is built natively on an arm64 runner (`01cee6e5cfe`).

- **No `zerodds-core` RPM could be installed.** `libzerodds.so` has no
  SONAME, so the package required `libzerodds.so()(64bit)` (from the
  `libzerodds.so` link) and provided only `libzerodds.so.1.0.0()(64bit)`.
  The spec sets the SONAME `libzerodds.so.1` (`4e52e463515`).

### Removed

- **`zerodds-corba-interop`: `tests/codegen_roundtrip.rs` and
  `tests/valuetype_wire.rs`**, which checked the generated stub against the
  generated skeleton and the value codec against itself (the cross-ORB tests
  and the octets of `value_wire` check the same against omniORB, TAO and
  JacORB), and `competitors/run_interop.sh`, `run_suite.sh` and
  `build_tao_naming_service.sh`.

- **`tests/cross_lang_live` and the step `cross-lang-live` of
  `tools/interop-gate`** (OPEN-ITEMS 46). The six scripts printed `PASS`
  without starting a publisher or reading a sample. A subscriber of a
  binding over `libzerodds` against a Rust publisher in another process is
  still missing; the binding jobs of `crates/endpoint-e2e` check the
  endpoint SDKs over DDS-XRCE, not these bindings.

- **`zerodds-corba-dds-bridge`: the six library modules no code used**
  (`locate`, `notify`, `qos_translation`, `servant`, `sync`, `wire`;
  OPEN-ITEMS 98). The daemon implements LocateRequest, the request, reply
  and notify mappings and the QoS of `zerodds-corba-bridge-1.0` §4.3, §4.4,
  §4.7 and §6 in `bridge_daemon`; the modules described designs the spec
  does not have. Their 24 tests and the 6 of `mapping` checked the code
  against itself; `mapping` stays, the dds-chat tutorial and the
  dds-warehouse demo describe their mappings with it.

### Changed

- **`tools/omg-xtypes-test-main`: the suite's answers are the client's
  defaults.** On the four axes where the OMG dds-xtypes suite expects another
  answer than the clause (README "Where the suite expects another answer than
  the clause"), the client now takes the suite's without flags:
  `--key-member-bounds ignored`, `--defaulted-discriminator-member
  stream-value`, `--strong-assignability names_ignored` (unless
  `ZERODDS_STRONG_ASSIGNABILITY` names one), `--key-on-union-discriminator
  carried` as before. The official matrix starts the client with a case's
  arguments only; the runtime's defaults stay the clause's, and the flags
  give the clause's reading. Measured on the Linux test host, every case Dust
  0.16.0 <-> ZeroDDS: without flags 305 of 335 in each direction, the same as
  with the four flags.

- **A generated type reads its key members alone** (`zerodds-lang-rules`
  `decode_keys…`, `zerodds-build`, `zerodds-dcps`
  `DdsType::decode_key_members`, `zerodds-wire` `Reader::skip_*`, XTypes 1.3
  §7.6.8): a `@final` or `@appendable` structure with a `@key` member gets
  a read of the key members that steps over every other member — a
  primitive, a `string`, a sequence or array of primitives, anything a
  DHEADER delimits — or reads and drops it, and leaves it at its default.
  Test `xtypes_7_6_8_decode_key_members_gives_the_key_of_the_whole_sample`
  (a consumer build): the KeyHash is the whole sample's in XCDR2 and XCDR1,
  both byte orders. `interop::ShapeType` reads its `color` alone.
- **A BEST_EFFORT DataReader past its RESOURCE_LIMITS takes its channel
  in by the eighth of its limit, by key alone** (`zerodds-dcps`,
  `ReaderRoom`, `DataReader::ingest_for_room`): a take-in run walks the
  whole cache, and one run per sample past the limit kept the receive
  thread busy; the run now admits by the key members where no filter and
  no EXCLUSIVE ownership needs the value, which is decoded when it is
  taken. Linux test host, BEST_EFFORT KEEP_ALL, `max_samples` 100 000,
  1 KiB, the subscriber taking every 3 s: 65 040 instead of 785 293
  datagrams dropped by the kernel in 15 s, peak 330 MB. Admitting by key
  alone made no difference measured on its own (785 293 either way).
- **The runtime computes a KeyHash from the key members alone**
  (`zerodds-wire-codec` `key_of_sample`, `decode_with`, `DecodePlans`;
  `zerodds-dcps` `SerializedTypeSupport::key_material_from_body`, XTypes 1.3
  §7.6.8): for a sample whose writer sent no `PID_KEY_HASH`, the runtime
  read the whole body dynamically to hash its key; it now reads the key
  members and steps over the others (a primitive, a `string`, a sequence or
  array of primitives, anything behind a DHEADER or an EMHEADER). Linux
  test host, release build, a kilobyte of octets before the key: 3.3 µs
  instead of 92 µs per sample. Keeping the decoder's layout plans per type
  (`DecodePlans`) made the whole read 1 % faster (92.8 → 91.5 µs). Test:
  `xtypes_7_6_8_the_key_from_the_body_is_the_key_of_the_whole_sample`
  (seven types, XCDR1 and XCDR2, both byte orders).
- **A derived type that owns its fields reads its key from a body by the
  plan** (`zerodds-wire` `codec::key_holder_from_body`; `zerodds-codec`
  `key_material_by_plan`; `zerodds-derive`; XTypes 1.3 §7.6.8): its
  `DdsType::key_material_from_body` reads the key members into the key
  holder and steps over every other member by the plan, without building
  a value, so a reader that admits by key — after `take_loan`, or past a
  BEST_EFFORT reader's limit — decodes no sample for its instance. A shape
  the read does not take (a final union on the way, an `@optional` key
  member, a key member a writer left out, a PL_CDR1 parameter list) answers
  `None`, and the reader reads the whole sample as before. Test
  `xtypes_7_6_8_the_key_read_by_the_plan_is_the_key_of_the_whole_sample`:
  final, appendable and mutable types with a nested key struct, strings,
  sequences, an array and an `@optional` member, under XCDR1 and XCDR2 in
  both byte orders, give the KeyHash and holder of the whole sample.
  Measured on the Linux test host (release build, 2 408 octets): 1.13 µs
  per sample against 3.47 µs to decode the sample and key the value.
- **A DATA's in-line QoS of only PID_KEY_HASH and PID_STATUS_INFO is read
  in place** (`zerodds-rtps`, `DataSubmessage::inline_qos` is an
  `inline_qos::InlineQos`; DDSI-RTPS 2.5 §9.4.5.3.2): such a list, the
  in-line QoS of almost every sample, is held without an allocation, as
  Cyclone DDS's `ddsi_plist_quickscan` reads it; any other list is parsed
  into a `ParameterList` as before. `InlineQos::key_hash` and
  `status_info` answer for both forms, `parsed` gives the list for the
  other parameters, `to_list`/`into_list` the whole list, and a list built
  for a DATA goes in with `.into()`. Test
  `ddsi_9_4_5_3_2_inline_qos_read_in_place_is_what_the_parser_reads`: the
  same parameters, octets taken and refusals as the parser, for both
  orders, both byte orders, both list ends, PAD, IGNORE, a duplicate, the
  must-understand bit, a wrong length and a cut-off list. Measured on the
  Linux test host (1 kB samples, BEST_EFFORT, six pairs in alternating
  order): 0.26 instead of 1.26 allocations per sample on the receive
  thread, 6.8–7.0 % fewer user-space instructions per sample in the
  subscriber; the writer is unchanged.
- **An in-line QoS value of up to 24 octets no longer has an allocation of
  its own** (`zerodds-rtps`, `Parameter::value` is a `ParameterValue`,
  `INLINE_VALUE`; DDSI-RTPS 2.5 §9.4.2.11): PID_KEY_HASH, PID_STATUS_INFO
  and the other short values lie in the `Parameter`, a longer one in a
  `Vec<u8>`. `ParameterValue` dereferences to `[u8]`, compares with
  `[u8]`, `Vec<u8>` and arrays, and is built with `.into()` from them.
  Measured on the Linux test host (1 kB samples, BEST_EFFORT, three pairs
  in alternating order): 1.25 instead of 2.26–2.30 allocations per sample
  on the receive thread, 1.7 % fewer user-space instructions per sample in
  the subscriber; the writer is unchanged.
- **A received datagram is a `Payload`** (`zerodds-transport`,
  `ReceivedDatagram::data`; `zerodds-rtps` `Payload`; `zerodds-dcps`
  `SampleBytes`): a payload is a range of an `Arc<[u8]>` or of a shared
  `Vec<u8>`, which it takes over without a copy. A transport builds `data`
  with `.into()` from a `Vec<u8>`, an `Arc<[u8]>` or a `&[u8]`. Measured on
  the Linux test host (1 kB samples, BEST_EFFORT): the receive thread's CPU
  time per sample is unchanged (1.15–1.17 µs).
- **A writer no longer copies its QoS for every sample to learn the
  in-line QoS length** (`zerodds-rtps`, `ReliableWriter`, DDSI-RTPS 2.5
  §8.4.9.2.4, §8.4.14.1): the length the fragment size is made of is
  counted from the parts. Measured on the Linux test host (1 kB samples,
  BEST_EFFORT, two runs each): 707–712 instead of 596–610 kS/s written,
  4.1 instead of 14.1 allocations per sample on the writing thread.
- **The receiver state no longer allocates per submessage** (`zerodds-rtps`,
  `ReceiverState`, DDSI-RTPS 2.5 §8.3.4): its reply-locator lists are a
  `ReplyLocators` that holds one locator in place. Measured on the Linux
  test host (1 kB samples, BEST_EFFORT, 600 kS/s for 15 s): 20.2 instead
  of 59.7 million allocations on the receive thread; the CPU time per
  sample is unchanged within the spread of the measurement.
- **The Windows platform jobs of the weekly run and of a release candidate
  start after every job on the CI runner host** (the stage `windows`;
  `internal/release/rc8-test-suite.md`, decision 12): the Windows runner shares the
  virtualization host's disk with the CI runner host, and its tests ran into
  their 25 s beside the CI runner host's builds. `crate-tier-record` moved to the stage `record`
  after them. Weekly run 9186: the Windows jobs started 3 s after the last
  job of the CI runner host, 11 of 11 green, the slowest of 3868 test results 4.3 s.

- **The speed pillar's clones fetch with the CI job's token** instead of
  the speed host's git credential store, which had dropped its credential
  after a rejection (weekly run 9186, job 231127).

- **The weekly coverage jobs name a negative line count and fail** instead
  of counting the line again from the regions of `cargo llvm-cov report
  --json` (`ci/coverage_jobs.py`): such a count is a test that ends with
  threads of its own still running.

- **`wireprobe-c-matrix` hands the units of a run to the compiler in
  groups**, one compiler call and one program for up to 48 units, where it
  compiled and linked each unit alone (OPEN-ITEMS 61): a configuration with
  an empty cache 8.4 to 9.7 s on the CI runner host (pipelines 9068 and 9075). Its
  `expected_bytes` test runs a configuration per encoding, and the CORBA and
  ROS 2 configurations, whose rows in the table are `xcdr1` only, no longer
  compile `xcdr2`: at most 4.0 s per test with an empty cache on the Linux
  test host (two runs).

- **`ros-rmw-deb` takes rustup's toolchain and the crates.io registry from
  the cache volume of the runner host** (OPEN-ITEMS 97): 1.4 GiB written by
  a job instead of 4.3 to 4.4 GiB (jobs 230660, 229526, 229706).

- **`integration-bindings-ffi` runs on merge** for every change to
  `zerodds-c-api`, `zerodds-cpp`, `zerodds-py`, `crates/cs` or
  `crates/ts-node`, beside the job of the touched area: the merge lane runs
  at most one job per area and, new, the jobs a specification has run on
  every push of what it covers (`merge_beside` in
  `ci/integration_scenarios.py`). zerodds-ffi-loader-1.0 §5 says so now
  in place of a `cross-lang-matrix` job on every push.

- **The C column gives a structure without members one member,
  `uint8_t zd_no_members`, that is not serialized** (C23 §6.7.2.1p8 leaves a
  structure with no named member undefined; IDL 4.2 allows one). It refused
  the structure before; the 32 cells of `empty-struct`,
  `empty-struct-as-base` and `empty-struct-in-sequence` that the C column's
  gaps file declared as a language limit now match
  `tests/interop-corpus/expected-bytes.tsv`. Cyclone DDS's idlc 11.0.1 puts
  such a structure under `#if 0` and a type using it does not compile; RTI's
  rtiddsgen 4.7.0 writes an empty structure, a GNU C extension.
- **The Flutter app of the dds-chat tutorial writes and reads XCDR2**: a
  DHEADER and members aligned to at most four octets, where it wrote neither.
- **The round trip gate compares with the last release in the same
  window** (OPEN-ITEMS 41, the maintainer's decision). `speed-regression-1`
  and `-2` measure `roundtrip-1us` of the release tag (v1.0.0-rc.7, built
  by `speed-build-reference` with the checked tree's `roundtrip_1us.rs`)
  and of the checked tree in turn, five repetitions per side and cell; a
  cell is red when the median p50 of the checked tree lies more than 25 %
  above the release's. The tolerance comes from one binary measured
  against itself on the speed host: 24 cell-runs, differences -10.4 to
  +17.0 %, standard deviation 5.6 %. The stored baselines
  (`tests/perf/internal-bench/baselines.txt`, `--update`,
  `RUN_BENCH_REGRESSION_UPDATE`) are gone. rc.7 cannot measure the SHM
  cells (its SHM user transport, fixed in `9ba7af8cf3a`); the gate declares
  that per tag and measures the checked tree alone there.

- **The round trip gate measures on the exclusive speed host.** The
  release job `bench-regression` ran on a shared CI runner, where its SHM
  and SameHost cells measured above the +20 % tolerance while other jobs
  ran beside it (pipeline 8962). It is now `speed-regression`, a step of
  the speed pillar (`ci/jobs/speed-pillar.yml`, `tests/perf/speed/pillar.sh
  regression`): the same eight cells and the same gate, built by
  `speed-build` and run on the host the pillar reserves exclusively through
  the board. It blocks a release candidate and reports in the weekly run;
  `SPEED_PILLAR=regression` runs it alone, `RUN_BENCH_REGRESSION_UPDATE=true`
  keeps new baselines as an artifact.

- **`zerodds-dcps`: the last handle of a `DcpsRuntime` stops it.** Its
  worker threads, and the tick driver of an external executor, hold the
  runtime weakly and only for one datagram or one tick, not while they
  wait; when the last `Arc<DcpsRuntime>` its owner holds drops, `Drop`
  stops the threads and joins them, as `shutdown` does. Before, each
  thread held its own strong reference, and a dropped handle stopped
  nothing: the five threads of a runtime ran on until the process ended
  (measured on the Linux test host). `runtime_drop_threads.rs` checks it
  without a participant and without `shutdown`.

- **`zerodds-idl-lower`: the caller's `default_extensibility` reaches
  enumerations and bitmasks**, every type that carries an extensibility
  (XTypes 1.3 §7.3.1.2.1.8, Annex B `EnumTypeFlag`, `BitmaskTypeFlag`), as
  CycloneDDS's `idlc` applies its default; it filled structures and unions
  only, and an unannotated enumeration under a `@final` default kept the
  appendable position in its TypeObject flags.

- **A coverage item whose cited test runs in no CI job is `partial`**
  (`internal/spec-coverage/PROCESS.md`: `done` needs a test that is green in
  CI), and the missing run is an `open` item of its own beside it:
  `ros2-rmw.md` 17 items and `zerodds-py-1.0.md` §6.4 (the ROS 2 tests and
  check programs, OPEN-ITEMS 42), `zerodds-ffi-loader-1.0.md` §3.6 (the
  Flutter tests, 59), `cross-vendor-validation.md` CV-18b and CV-21b (62). A
  test of `ci/manual-tests.tsv` with a green last run counts as green
  (decision 11 of `rc8-test-suite.md`), shown apart: `dds-tsn-1.0.md` Annex A
  stays `done`; so does a test only the speed pillar's `speed-async-write`
  runs, by that job's status (three items of `zerodds-async-1.0.md`). CV-3 cites the OMG cases by their
  names in the JUnit report of `omg-rtps-fastdds`. Every coverage document's
  Test-Lauf line names the CI jobs that ran its cited tests green
  (`coverage_results.py --test-run`), not a run on a host.

- **The weekly run runs the crate tier of the packages a change of the week
  reaches** (`ci/weekly-crate-tier.yml`, `ci/weekly_crate_tier.py`): every
  package one of whose transitive dependencies, or which itself, changed
  since the last green weekly run, the first time every package. The merge
  lane runs a package's own tests only when the package changes. Its first
  run (pipeline 9041): 200 jobs, 74.2 minutes of job time, two red, one a
  test of `zerodds-routing-service` that dropped its sink runtime (fixed),
  one a C column case past 25 s under load (OPEN-ITEMS 61). The
  `security-pki` contract with `zerodds-security` holds that a class id
  without versions is not PKI-DH 1.x.

- **The TypeObjects of the interop corpus are held to CycloneDDS's**
  (`crates/types/tests/type_object_bridge.rs`). The test compared this
  project's TypeObjects with its own decoder. Now
  `tests/interop-corpus/cyclone-type-objects.tsv` holds the TypeObjects
  Cyclone's `idlc` wrote for 333 corpus cases (generated from the matrix by
  `tools/expected-bytes/type_objects.py`, MD5 of each object checked
  against its identifier): the model rebuilt from Cyclone's COMPLETE object
  writes the sample as Cyclone does (`expected-bytes.tsv`, XCDR2 and XCDR1
  little endian, 630 cells), and the identifiers computed
  from the IDL, read with Cyclone's default extensibility (`@final`), are
  Cyclone's in 263 COMPLETE and 266 MINIMAL cases; the others part where
  XTypes 1.3 and Cyclone do: the must-understand bit of a union
  discriminator (§7.2.2.4.4.4.6) and of a key member (§7.2.2.4.4.4.8), the
  `hash_id` of a `@hashid` member, and Annex B's minimal type-flag mask.

- **`zerodds-float128` reads a long decimal from its first forty
  significant digits** when the two values they bound the decimal by round
  to the same binary128; only a value that close to a rounding boundary
  reads every digit
  (IEEE 754-2019 §5.12.2 still holds, checked at the tie between one and
  its successor written out to 200 digits). The exact form of the smallest
  normal value, 11 458 digits, which the JSON column writes, took 49.3 ms
  to read in a test build and takes 5.5 ms. The `dynamic_corpus` test of
  XTypes 7.5.2, which reads it 16 times, took 1.69 to 1.73 s on the Linux
  test host and takes 0.98 to 0.99 s.

- **`zerodds_dcps::runtime::setting_path` looks a name up in a table** built
  once out of `SETTINGS`, the first row of a name winning as before; it
  walked the rows per lookup, and every configuration read looks up each of
  its settings.

- **The Elixir corpus check compiles a share's cases in one Erlang VM**
  (`elixir_compiles_together` in `crates/lang-rules/tests/elixir_end_to_end.rs`)
  and unloads a case's modules before the next; a share that does not pass
  cleanly goes through `elixirc` case by case as before. All Elixir codegen
  tests are one job, `integration-codegen-elixir`: 107.0 s on the CI runner host (pipeline
  8976) instead of three jobs of 497.0 s together (pipeline 8967).

- **The weekly run checks each property once** (internal/release/rc8-test-suite.md,
  "Interop" and "The weekly run measured"). The OMG dds-rtps suite runs
  every case against Cyclone and the cases that carry a vendor's dialect
  (one per family) against Fast DDS and Connext, Connext with the
  presentation families only it runs; dds-security the permission
  combinations and protection kinds against Connext (83 of 275 cells) and
  every protection kind against CoreDX: 14 jobs instead of 26. The
  `vendor-live` jobs are four (one build of the vendor programs per job);
  the three against the Fast DDS shapes client went, the dds-rtps cases
  against Fast DDS check the same exchanges. The byte gate
  (`ci/vendor-bytes.yml`) runs when the pinned interop image, its cases or
  the recorded rows change and on a release candidate, not weekly. The
  three SwiftPM examples run in `integration-bindings-swift` as the
  programs `swift build` built: under `swift run` each ended about two
  minutes after its last line. The F# corpus is one job: the cases of a
  share go through one `dotnet fsi` (the header comment of a generated file
  had sent each case through one of its own), 16 shares instead of 256.
  `provision.sh` keeps the SDK archives once per runner host and the
  Debian packages per slot (`integration-codegen-swift` wrote 142 instead of
  3407 MiB, pipelines 8952 and 8956). `cutover-owners` went with the empty
  cutover list (B5). `ci/job_io.sh` (`JOB_IO_SCAN=1`) reports what a job
  wrote.

- **The speed pillar starts with the weekly run.** Its jobs belong to the
  pipeline itself, in its first stage (`ci/jobs/speed-pillar.yml`), and no
  longer to a child pipeline in the last stage: GitLab gives a runner the
  pending job with the lowest id, and a child pipeline's jobs are created
  after every job of its parent (the `cutover-owners` child of pipeline
  8823 was created at 04:24 and started at 05:28). Every job may fail; a
  job without a reservation or without the pipeline's build stops at once
  and says so (`tests/perf/speed/board.py`, `pillar.sh measure`), pipeline
  8863.

- **`integration-sdk` without the C++ programs of DDS-RPC.** They run in
  `integration-rpc-cpp` (`tests/integration/rpc_cpp.sh`, `zerodds-rpc`
  `cpp_*`), on a change to `zerodds-rpc` and weekly. `integration-sdk` took
  292.3 s of its 300 with them (pipeline 8791) and 163.9 s without
  (pipeline 8856); `integration-rpc-cpp` 40.8 s.

- **`bench-regression` leases its DDS domains** through
  `tests/integration/with_domains.py` (`ZERODDS_TEST_DOMAINS`) from the
  runner host's share of the band, instead of domains 140 to 175 it
  computed itself, whose RTPS ports lie in the kernel's ephemeral range.

- **The whole-corpus tests of `zerodds-interop-results` run weekly.** The
  job `interop-results` runs `--verify` and the 12 tests in the cargo
  profile `corpus`, a test build with `opt-level = 2`: 4.0 to 16.4 s each
  instead of 22.5 to 75.3 s, the job 113 s (pipelines 8854, 8877). Four tests went: they compared
  the committed tree, ledger, tables, generated documents and page with a
  fresh run, which `--verify` does.

- **The first job of each coverage chain fetches no artifacts** of the
  jobs before it; it uses none (17.5 and 17.3 s in pipeline 8823).

- **`ci/full.yml` is deleted.** No pipeline included it; its last job,
  `test`, only carried the `--exclude` list of the packages cut over to the
  crate tier. That list is `ci/cutover-packages.txt` now, one package per
  line, and `ci/cutover_gate.py --check` also fails on a name there that is
  no package of the workspace. `ci/select_merge.py` loses the child
  configuration that included the file (no job called it; the merge lane is
  `ci/merge_lane.py`).

- **`macos-bundle` runs by hand in the release packaging check**
  (`RELEASE_PACKAGING_CHECK=true`, `RELEASE_PACKAGING_JOB=macos-bundle`;
  `ci/jobs/macos-bundle.yml`, which `ci/full.yml` includes for release
  tags). Its manual rule on a push to `main` never fired, because a push
  pipeline does not include `ci/full.yml`. The job does not let `brew
  style` install gems into the runner's Homebrew.

- **Rust toolchain 1.97.1 (Ferrocene 26.08).** `rust-toolchain.toml` pins
  1.97.1, the workspace declares `rust-version = "1.97"`; the CI image
  (`ci/Dockerfile.rust`, base `rust:1.97.1-slim-trixie`), the tool pins, the
  GitHub workflows, the `.deb`/`.rpm` builds and the Docker images under
  `packaging/docker/` follow. `rust-toolchain.toml` lists the Ferrocene 26.08
  targets; `aarch64-unknown-linux-gnu` is not one of them. The code follows
  the lints of clippy and rustdoc 1.97: nested `if let` as let chains,
  `is_multiple_of`, derived `Default`, `ptr::slice_from_raw_parts_mut`
  instead of a reference cast, and intra-doc links that name a crate by its
  Rust name.

- **Release files.** `source.tar.gz` comes from `git archive` with one top
  directory `zerodds-<version>/`; one `SHA512SUMS` over all release files is
  signed with minisign (`SHA512SUMS.minisig`) and keyless with cosign
  (`SHA512SUMS.cosign.bundle`); the per-file signatures of the Linux builds,
  never published, are gone (`de738d7e0c1`).

- **Homebrew.** The macOS release jobs build the bundle the formulae
  install; `publish-distros.yml` renders the formulae from the release,
  installs them on macOS and pushes the tap. Formulae point to
  `zero-objects/zero-dds` (`de738d7e0c1`).

- **RPM repository.** The pull deploy of `packages.zerodds.org` is in the
  repository (`packaging/linux/repo-server/sync-zerodds-packages`). It
  verifies `SHA512SUMS.minisig` and every package against `SHA512SUMS`,
  signs every `.rpm` with the repository key and publishes it only after
  the signature verifies against `zerodds.gpg`, and writes one `.repo` file
  per distribution (`zerodds-fedora.repo`, `zerodds-rhel-9.repo`,
  `zerodds-opensuse-leap-15.6.repo`). The release workflow installs every
  `.rpm` from a signed local repository with `gpgcheck=1` before
  publishing (`01cee6e5cfe`, `4e52e463515`).

- **Crate docs follow the code** in `durability-store-file`, `cli-common`,
  `corba-rt`, `corba-rust`, `corba-cos-notify`, `opcua-pubsub`,
  `recorder-decode` and `opcua-server` (`e398c0db084`).

### Security

- **`rusqlite` 0.41 closes RUSTSEC-2026-0459.** The SQLite that
  `libsqlite3-sys` 0.30.1 bundled (before 3.54.0) can return uninitialized
  page-cache memory when a database is opened with a `-wal` file of another
  page size. `zerodds-durability-store-sqlite`, `zerodds-recorder-decode`
  and `zerodds-record` take `rusqlite` 0.41.1 with `libsqlite3-sys` 0.39.1
  (SQLite 3.54.1); the API they use did not change.

## [1.0.0-rc.7.5] — 2026-11-09

Fourteen crates on crates.io only; no other channel.

### Fixed

- `zerodds-idl-parse` and `zerodds-lang-rules`: GitHub #62, #63 and #64 (the
  Unreleased section above). `zerodds-build` 1.0.0-rc.7.3 takes them on a
  `cargo update`.
- `zerodds-dcps`: discovery asks the routing table once per destination
  (OPEN-ITEMS 114).
- `zerodds-record`, `zerodds-spy`, `zerodds-recorder-bridge` and
  `zerodds-cli-common` record the history a TRANSIENT_LOCAL writer keeps
  (OPEN-ITEMS 113). Their last release was 1.0.0-rc.7, so they carry
  everything since, and so do `zerodds-recorder` and
  `zerodds-recorder-decode`, which they build on (capture format 3).
- `zerodds-recorder-decode` links SQLite through rusqlite 0.41
  (RUSTSEC-2026-0459). `zerodds-durability-store-sqlite` 1.0.0-rc.7 links it
  through rusqlite 0.32, and the two cannot share a graph, so the durability
  crates are released with it: `zerodds-durability-store`, `-store-file`,
  `-store-postgres`, `-store-sqlite` and `zerodds-durability-service`.

Requirements on `zerodds-wire`, `-wire-codec` and `-rtps` are
`^1.0.0-rc.7.3`, on `zerodds-types` `^1.0.0-rc.7.4`. Checked on the Linux
test host before the upload, with the fourteen packaged crates patched in:
every published zerodds crate checks as without them (133 of 146, the 13
others stop with "no longer part of ZeroDDS" either way), the binaries of
`zerodds-record`, `-spy` and `-recorder-bridge` build, and the
reproductions of #62 to #64 build and pass with `zerodds-build`
1.0.0-rc.7.3. With the next release the fourteen return to the workspace
version.

## [1.0.0-rc.7.4] — 2026-11-09

Two crates on crates.io only; no other channel.

### Added

- `zerodds-types` and `zerodds-dcps`, the two that changed since rc.7.3: the
  forms in which the implementations write XTypes TypeObjects, as options
  (`from_ast::TypeObjectForm`, `DomainParticipant::set_minimal_type_flags_kept`),
  and the RTI/CoreDX minimal camp in the announced TypeInformation and the
  TypeLookup replies (`eb2a0597371`; the Unreleased section above). The OMG
  dds-xtypes client builds against them. Their requirements on
  `zerodds-wire`, `-wire-codec` and `-rtps` are `^1.0.0-rc.7.3`. Checked on
  the Linux test host before the upload (the clients with the two packaged
  crates patched in) and after it: the three OMG clients built from
  crates.io, every zerodds package from the registry; every published
  zerodds crate checks with them as with rc.7.3 (133 of 146, the 13 others
  stop with "no longer part of ZeroDDS" either way). With the next release
  the two return to the workspace version.

## [1.0.0-rc.7.3] — 2026-11-08

Six crates on crates.io only; no other channel.

### Fixed

- The crates the clients of the OMG interoperability pull requests build
  against (dds-rtps, dds-xtypes, dds-security): `zerodds-wire`,
  `zerodds-wire-codec`, `zerodds-types`, `zerodds-build`, `zerodds-rtps`
  and `zerodds-dcps`, the six that changed since rc.7.2. Their fixes are in
  the Unreleased section above (`804c1b4989c` to `b6b8179437c`), among
  them `PID_WRITER_GROUP_INFO` in 16 octets for Dust
  DDS, `dataMaxSizeSerialized` without declared padding, the must-understand
  bit on `PID_EXTENDED` and `PID_LIST_END` read either way, TypeLookup
  requests naming their sender and peers that answer only for COMPLETE
  identifiers, OpenDDS's `TypeConsistencyKind` numbering, lifespan before
  every send, one group cut per `begin_access`, and the end of a group
  coherent set without `PID_COHERENT_SET`. Their requirements on each other
  are `^1.0.0-rc.7.3`; the other crates stay as published and take them
  when they resolve. Checked after the upload on the Linux test host: every
  published zerodds crate checks against them as against rc.7.2 (133 of
  146; the 13 others are the crates that stop with "no longer part of
  ZeroDDS" either way), and the three clients built from crates.io pass a
  short run (dds-rtps against themselves and both ways with Connext 7.7.0,
  dds-xtypes against themselves). With the next release the six return to
  the workspace version.

## [1.0.0-rc.7.2] — 2026-11-06

44 crates on crates.io only; no other channel.

### Added

- The crates of GitHub #33 on crates.io, ahead of RC8: `zerodds-codec`
  and `zerodds-derive` (new), `zerodds-build` with `rust_type`, and the 41
  other crates of the workspace they depend on that changed since rc.7,
  among them `zerodds-dcps`, `-types`, `-wire` and `-rtps`
  (`7726f6ce3a8`). Their requirements on each other are `^1.0.0-rc.7.2`;
  the published crates that ask for `^1.0.0-rc.7` take them when they
  resolve. Checked before the upload: every published zerodds crate checks
  against them as against rc.7, and the cargo examples of zero-dds-snippets
  build against them. `rust-version` is 1.97 (rc.7: 1.88). With the next
  release the 44 return to the workspace version.

## [1.0.0-rc.7.1] — 2026-10-31

Three crates on crates.io only; no other channel.

### Fixed

- crates.io had the rc.7 crates from `a13c80b07`, before three fixes in
  the tag. Published alone as `1.0.0-rc.7.1`: `zerodds-transport-udp`
  builds for musl, `zerodds-webdds-bridge` for Windows, `zerodds-lang-rules`
  writes Javadoc that doclint accepts. Dependents ask for `^1.0.0-rc.7` and
  take rc.7.1. With the next release the three return to the workspace
  version.

## [1.0.0-rc.7] — 2026-10-28

The workspace was taken apart and rebuilt. `f225e12ef5e` reduced `members`
from 141 entries to three, `fa2ed8b0a9c` deleted the old code — 1321 files,
367,739 lines, 37 `Cargo.toml` — and everything since is a crate re-entering
on the new foundation. That single fact explains most of what follows.

### Removed

- **The `pages` job is gone.** It copied `documentation/specs/index.html`,
  which no longer exists, to `specs.zerodds.io`, which was never set up
  (`zerodds.io` does not resolve); a spec tag would have failed on it. The
  two spec PDFs stay linked from the website.

- **Twelve published crates no longer exist.** `zerodds-cdr`,
  `zerodds-cdr-derive`, `zerodds-idl`, `zerodds-idl-rust`, `zerodds-idl-cpp`,
  `zerodds-idl-java`, `zerodds-idl-python`, `zerodds-idl-ts`,
  `zerodds-idl-csharp`, `zerodds-cs`, `zerodds-java-omgdds` and
  `tools/dds-roundtrip-codegen` were members at rc.6 and are neither members
  nor on disk at rc.7 (`fa2ed8b0a9c`).

  **Migration:** `zerodds-cdr` → `zerodds-wire`. `zerodds-idl` →
  `zerodds-idl-cst` / `-parse` / `-lower` over `zerodds-ast`. The
  `zerodds-idl-<lang>` backends → `zerodds-lang-rules` with `-cst`, `-render`
  and `-python`. For a `build.rs`, the whole chain is one call:
  `zerodds_build::Config::new().compile(&["types.idl"])`. The C# source
  tree came back under `crates/cs/csharp/` on the C FFI, the Java source
  tree under `crates/java-omgdds/java/` as a pure-Java implementation with
  its own RTPS stack and no JNI (`043616494b3`); neither has a Rust crate of
  its own.

- **`zerodds-idlc` keeps its name and loses its whole flag surface.** Gone:
  `--c`, `--rust`, `--cpp`, `--ts`, `--csharp`, `--java`, `--python` (now
  `generate --lang <name>`), `--all`, `-o`/`--output`, `--out-<lang>`, `-I`,
  `-D`, `--scaffold`, `--rti`, `--corba`, `--no-typeobject`,
  `--with-typeobject`, `--dump-typeobject` and its subcommand, `--parse-only`,
  `--dump-deps`, `--default-nested`, `--opendds`, `--cyclone`,
  `-v`/`--verbose`, `-q`/`--quiet`, `--color`. New: the `lint` and `format`
  subcommands, `--json`, `--deny-warnings`, `--strict`,
  `generate --encoding`, `generate --endian`.

  **The loss of `-D` binds every build system** that passed preprocessor
  defines on the command line; `zerodds-build` carries it (`Config::define`)
  and is the supported path. `--default-extensibility` was removed with the
  rest and put back in `926bfa9d184`, this time on all six subcommands.
  `-I`/`--include` came back in `998acfd871c`, with `generate --stamp`,
  `--depfile` and `--output-manifest`: the build contract the CMake function
  `zerodds_idlc_generate` drives.

- **Python: `zerodds.idl` and `zerodds.cdr` are gone** (`8ae310a6e6f`). Their
  callers use the code `zerodds-idlc generate --lang python` writes and the
  separately versioned wire runtime (`runtime/python`, `zerodds_wire`).

- **The `interop.rti_group_coherent` setting is gone** (`af9b246601e`), with
  the RTI group-coherent virtual-GUID emulation it switched (`f95383939ba`):
  RTI reassembles a group-coherent set by a proprietary rule that the
  emulation did not reach.

- **DDS-RPC 1.0: names the specification does not define are gone from
  `zerodds-rpc`** (`1e28b13546c`, `e305669e2c4`, `2fe2b7cdea1`).
  `RpcAnnotation::RpcRequest` and `RpcReply` (`@RPCRequest`, `@RPCReply`);
  `codegen::build_enhanced_pair`, `build_enhanced_all` and `MethodPair`,
  which described one topic pair per operation with a header, where
  §7.4.3 and §7.5.1.2 have one `Request` and one `Reply` type per
  interface; `RemoteExceptionCode::UnknownInterface` (wire value 6;
  §7.5.1.1.1 lists six codes, and `from_u32(6)` is refused); the
  `discovery_ext` types with a service name and a mapping profile, and
  `client_matches_service`/`service_matches_client`.

  **Migration:** `codegen::build_enhanced_service` for the Enhanced
  mapping; `discovery_ext::service_matching_complete` and
  `client_matching_complete` for matching (§7.6.2).

- **DDS Java PSM: `org.omg.dds.topic.TopicType` is gone**
  (`81d3f84b59b`). DDS-Java-PSM 1.0 §7.2.6 forbids an implementation's
  extensions in `org.omg.dds` or any package with that prefix, and nothing
  referenced the type. The annotation bridge of the IDL-Java runtime is in
  `org.zerodds.types` and `org.zerodds.rpc`.

### Security

- **`cryptoki` 0.12 and `duckdb` 1.10506.1 close four advisories.**
  `zerodds-security-runtime`'s `pkcs11` feature took `cryptoki` 0.7, which
  RUSTSEC-2026-0286 covers (an out-of-bounds read when decoding
  `CKA_ALLOWED_MECHANISMS`); it now takes 0.12, with `CInitializeArgs::new`
  and the `secrecy` PIN of that release. The lakehouse store's `duckdb`
  feature reached `zip` 6 through `libduckdb-sys` 1.10505.0
  (RUSTSEC-2026-0367, -0368, -0369, all in archive extraction);
  `libduckdb-sys` 1.10506.1 takes `zip` 9. Both CIs run `cargo deny` with
  `--all-features` now, which is where the four showed: without it the
  optional features were not checked. `serialport`, the XRCE agent's serial
  line behind its `serial` feature, is allowed under MPL-2.0 as a single
  exception.

- **A deeply nested XML text no longer overflows the stack of its reader.**
  `roxmltree` 0.20 recurses once per open element and has no limit of its
  own; 100 000 unclosed start tags overflowed an 8 MiB main thread in a
  release build (measured on the Linux test host). The readers of
  permissions and governance documents (`zerodds-security-permissions`, the
  permissions document reaches it from the handshake), of the AMQP
  endpoint, routing-service, TSN and OPC UA gateway configurations now run
  `zerodds_foundation::check_nesting` first, with the limit
  `XML_NESTING_LIMIT` = 64 that `zerodds-xml` uses. Test per reader:
  `a_deeply_nested_text_is_refused_before_the_parser`.

- **`PermissionsAccessControl::validate_remote_permissions` verifies the
  remote permissions** (DDS-Security 1.2 §9.4.1.2.1, §9.4.2.9.2). The
  exported plugin read the token as plain XML, checked no signature, and
  bound the peer to the document's first grant. It now verifies the S/MIME
  signature against the Permissions CA armed with
  `arm_remote_permissions_verifier`, and takes the grant of the subject of
  the remote identity certificate (DER, `remote_credential`). The runtime's
  own gate was not affected: it verifies `c.perm` against the Permissions
  CA. **Migration:** a caller of the plugin arms the Permissions CA;
  without it every remote token is refused. Tests:
  `crates/security-permissions/tests/remote_permissions.rs`.

- **A topic without a governance rule gets no endpoint** (DDS-Security 1.2
  §10.4.1.2.7). It was taken as unprotected: writers and readers were
  created and remote ones matched. Test: `a_topic_without_a_rule_is_refused`.

- **`check_remote_participant` is called; an unauthorized peer is ignored**
  (DDS-Security 1.2 §9.4.2.9.9, §9.8.7). In a domain with
  `enable_join_access_control` an authenticated peer whose permissions do
  not validate or do not allow joining is ignored as by
  `ignore_participant`. The operation was never called.

- **A forged handshake reply or final no longer takes the handshake down**
  (DDS-Security 1.2 §9.3.2.11.5/.6). The PKI-DH plugin removed the
  handshake state before checking the message; the genuine message that
  followed found none. The state is removed once the checks passed. Test:
  `a_forged_reply_does_not_take_the_handshake_down`.

- **Governance and permissions documents outside the XSD are refused**
  (DDS-Security 1.2 §10.4.1.2.3, §10.4.1.3, §10.4.1.5.3.3/.5). A typo in a
  ProtectionKind was read as NONE, in a boolean as FALSE, in `<default>` as
  ALLOW; an unreadable domain id as every domain, an unreadable validity
  bound as none. Each is an error now; an unknown governance element is one
  unless it has `must_interpret="false"`. The zone of an `xs:dateTime` is
  applied. Tests: `a_governance_outside_the_xsd_is_refused`,
  `an_unreadable_validity_or_default_is_refused`.

- **Permissions are bound to the identity's SubjectName; the remote
  endpoint checks fail closed** (DDS-Security 1.2 §10.4.1.5.3.1, Table 70,
  §9.4.2.9.10/.11). Every grant of a permissions document counted for every
  identity; now only the one whose `subject_name` is the certificate's, and
  a `SecurityProfile` without one is refused. A peer's endpoints on an
  access-controlled topic were permitted while its grant was unknown; they
  are denied until the handshake hands it to the gate, and matched then.
  `SharedSecurityGate::set_remote_permissions` takes the peer's subject and
  returns `RemotePermissions`. Tests: `only_the_grant_of_the_subject_counts`,
  `a_peer_without_a_grant_is_denied_on_an_access_controlled_topic`.

- **PKI-DH checks the participant GUID in `c.pdata`** (DDS-Security 1.2
  §10.3.3 Table 59, §7.4.3). Bit 0 set and bits 1 to 47 from the SHA-256 of
  the certificate's SubjectName, for the request and the reply, and the
  prefix the one the participant was discovered with. A participant with a
  valid certificate could hand in any GUID, and announce another
  participant's over SPDP. A test that builds its own PKI plugin needs a
  GUID bound to its certificate (`adjust_participant_guid_prefix`) and
  `set_local_participant_data`. Tests in `security-pki` `plugin::tests`.

- **The PKI-DH replier hands out the SharedSecret only after a verified
  final** (DDS-Security 1.2 §9.3.2.5.2.3, §10.3.3 Table 59). The replier
  registered its secret when it sent the reply; the discovery stack, which
  asks for the secret after every handshake step, took the peer as
  authenticated before the final arrived or its signature was checked. A
  final that fails its checks now leaves no secret. Test:
  `the_replier_has_no_shared_secret_before_a_verified_final`.

- **PKI-DH: `hash_c1`, `hash_c2`, `dh1` and `dh2` are optional in the
  final** (DDS-Security 1.2 §10.3.2.5.3 Table 58). The final parser wanted
  all four and refused a final without them ("missing binary property:
  hash_c1"); an echo that is sent must still match, and the signature covers
  the replier's own values. Cyclone sends them only with
  `IncludeOptionalFields`. Since the replier hands out its secret only after
  the final, no handshake with Cyclone or OpenDDS as initiator completed.
  Tests: `a_final_without_the_optional_echoes_completes_the_replier`,
  `a_final_with_a_wrong_optional_echo_fails`.

- **A refused handshake step keeps the handshake in the discovery stack**
  (DDS-Security 1.2 §9.3.2.11.5, §9.3.2.11.6). The stack took the peer out
  of its table for the step and did not put it back when the plugin refused
  it; after a forged reply or final the genuine one found no peer and was
  dropped, though the plugin had kept its state. Test:
  `a_forged_final_does_not_take_the_handshake_down`.

- **An undiscovered PKI-DH initiator is bound to the prefix it sent from**
  (DDS-Security 1.2 §7.4.3, §10.3.3 Table 59). A request from a participant
  whose SPDP had not arrived got the local handle as a placeholder identity,
  and `begin_handshake_reply` had no GUID to compare `c.pdata` with: a
  participant with a valid certificate could authenticate under another
  participant's prefix. The stack now validates such an initiator under the
  RTPS prefix of the request; a refused first request keeps no peer and
  returns the identity. New: `AuthenticationPlugin::return_identity_handle`
  (§9.3.2.11.18), for the PKI-DH and PSK plugins. Tests:
  `an_undiscovered_request_under_another_prefix_is_refused`,
  `a_returned_identity_handle_is_disposed_of`.

- **Handshake messages are taken only as DDS-Security 1.2 addresses them**
  (§7.5.3.4, §9.3.2.11.5, §9.3.2.11.6). A stateless message whose
  `destination_participant_guid` names another participant is not taken; a
  handshake message whose `message_identity.source_guid` is not the sender's
  is refused; a reply or final whose `related_message_identity` is not the
  last message sent to the peer is refused and leaves the handshake as it
  was. The reply to an undiscovered initiator is addressed to its
  participant GUID; it took the request's `source_guid`, which Fast DDS sets
  to its stateless writer and then drops the reply. Tests:
  `a_handshake_message_is_taken_only_as_the_spec_addresses_it`,
  `the_reply_to_an_undiscovered_initiator_is_addressed_to_its_participant`.

- **An inbound datagram the security layer refuses is recorded** (DDS-Security
  1.2 §8.4.2.4, §8.5). The refusals of `classify_inbound` went only to a
  configured security log, and a DATA with an encrypted SerializedPayload
  that decodes with no key of the sender was dropped without any trace. Both
  are now the event `security.inbound.refused` with `peer`, `check` and
  `reason`. Tests: `a_refused_inbound_datagram_is_recorded`,
  `an_undecodable_encrypted_payload_is_recorded`.

- **PKI-DH sends and checks the AuthRequestMessageToken** (DDS-Security 1.2
  §9.3.2.11.3, §10.3.2.4 Table 55, §10.3.3 Table 59).
  `AuthenticationPlugin::validate_remote_identity` takes the
  `remote_auth_request_token` that caused the authentication to begin, and
  `local_auth_request_token` gives the one to send. Without a remote token
  PKI-DH makes one; its `future_challenge` is this side's `challenge1` or
  `challenge2`, and a remote token's must be the peer's (Table 59). The
  discovery stack sends its token on discovery, ahead of a request, and
  keeps one from a participant not yet discovered (at most 256) for its
  discovery; one for a peer whose authentication runs is not taken. The
  stack sent none and dropped the ones it got; challenges were fresh NONCEs
  and never checked. Cyclone and OpenDDS send the token, use its challenge
  and check the peer's (`authentication.c`,
  `AuthenticationBuiltInImpl.cpp`); Fast DDS 3.6.1 neither sends nor reads
  it (`PKIDH.cpp`). Tests:
  `the_auth_request_announces_the_challenge1_of_the_request`,
  `a_request_whose_challenge1_is_not_the_announced_one_is_refused`,
  `a_malformed_auth_request_fails_validate_remote_identity`,
  `an_auth_request_goes_out_on_discovery_and_binds_the_challenge`,
  `a_reply_whose_challenge2_is_not_the_announced_one_is_refused`.

- **The SRTPS_POSTFIX of a signed message is found by the submessages that
  lead to it** (DDS-Security 1.2 §9.5.2.4, DDSI-RTPS 2.5 §9.4.5.1.3). A fast
  path took the octet 24 before the end of a GMAC SRTPS message for the
  POSTFIX id when it was 0x34; Fast DDS puts a vendor submessage after the
  POSTFIX or receiver-specific MACs into it, and about once in 256 messages
  that octet is 0x34 by chance. ct and the MAC were cut in the wrong place
  and the message was refused ("srtps gmac verify failed"). The submessage
  walks of SRTPS, SEC_PREFIX/SEC_POSTFIX and the runtime also read
  `octetsToNextHeader == 0` of a PAD or INFO_TS as an empty body, not as the
  rest of the message. Tests:
  `the_srtps_postfix_is_found_by_its_submessages_not_by_its_place`,
  `an_info_ts_with_otn_zero_inside_srtps_is_an_empty_submessage`.

- **A message of an RTPS header without submessages is taken, not refused
  as malformed** (DDSI-RTPS 2.5 §8.3.6.3: a header is invalid when the
  message is shorter than a full header). `classify_inbound` wanted 28
  octets; Fast DDS sends 20-octet messages, and each was refused and, since
  the refusals are recorded, reported. Test:
  `classify_inbound_takes_a_header_without_submessages`.

- **A secured participant matches the builtin endpoints of a peer as its
  authentication allows** (DDS-Security 1.2 §9.8.3). The SEDP endpoints,
  regular and secure, and the volatile secure endpoints were matched by the
  `builtin_endpoint_set` of the SPDP announcement, before any
  authentication. With `allow_unauthenticated_participants` FALSE a peer is
  now matched with the ParticipantStatelessMessage endpoints only until it
  authenticates, then with all its builtin endpoints; with TRUE an
  unauthenticated peer gets the regular ones and the secure ones follow its
  authentication. A participant ignored for its permissions gets none. The
  endpoints of a secured same-process participant are taken once it
  authenticated, and the secure SPDP goes to authenticated participants only.
  Tests: `the_sedp_of_a_peer_is_matched_once_it_authenticated`,
  `an_unauthenticated_peer_gets_the_regular_sedp_only`.

- **The key material of an endpoint follows the protection kinds of its
  topic** (DDS-Security 1.2 §10.5.3.1 Table 78, §10.4.1.2.6.6,
  §10.4.1.2.6.7, §10.4.1.2.5). Every endpoint got the suites of the topic
  rule for the empty topic name; a writer of a topic with
  `data_protection_kind` ENCRYPT under a SIGN rule for that name had an
  AES-256-GMAC key and an only authenticated payload. A user endpoint now
  gets its key material at creation for the Metadata and Data Protection
  Kind of the rule of its topic, a secure builtin endpoint for the discovery
  or the liveliness protection kind of the domain.
  `SharedSecurityGate::endpoint_protection_kinds`,
  `register_local_endpoint_with_kinds`; the crypto plugin reads the kinds
  from the properties `zerodds.crypto.metadata_protection_kind` and
  `zerodds.crypto.data_protection_kind`. Tests:
  `the_key_material_of_an_endpoint_follows_its_protection_kinds`,
  `the_protection_kinds_of_an_endpoint_come_from_its_topic_rule`,
  `a_writer_gets_the_key_material_of_its_topic_rule`.

- **Crypto: kind 0x01 is AES128-GMAC, `master_salt` up to 32 octets,
  distinct key ids** (DDS-Security 1.2 §10.5.2 Table 77, §10.5.2.1.1,
  §10.5.2.2). The slot 0x01 carried HMAC-SHA256: an AES128-GMAC key of
  another vendor was verified with HMAC, and AES128-GMAC did not exist. It is
  now `Suite::Aes128Gmac`; the SIGN hint of the policy is AES-256-GMAC
  (`SuiteHint::Aes256Gmac`, capability string `AES_256_GMAC`), and the
  auth-only path of `seal_with`/`open_with` is AES-GMAC with a 16-octet tag
  instead of a 32-octet HMAC. `master_salt` is a `sequence<octet, 32>`: a
  peer's 16 octets for AES-128 were refused, and a new key now gets one of
  its key length, as Cyclone makes it. The `transformation_key_id` of a new
  key is drawn until no other key of this side has it, and never 0. Tests:
  `aes128_gmac_uses_spec_id_1`,
  `an_aes128_gmac_key_with_a_short_salt_crosses_instances`,
  `the_local_keys_have_distinct_nonzero_key_ids`.

- **SRTPS: the flags A and P and a HeaderExtension are read** (DDS-Security
  1.2 §7.4.6.6 Table 7, §10.5.3.3.1 Table 80). A message whose SRTPS_PREFIX
  sets A ("the RTPS Header and HeaderExtension are also protected as
  Additional Authenticated Data") was checked against no AAD; it is now
  checked with them, the HeaderExtension with `messageLength` and
  `messageChecksum` set to zero. A message with P (pre-shared key) is
  refused as such instead of being checked with the participant key. A
  HeaderExtension between the RTPS Header and the SRTPS_PREFIX is found and
  kept in the decoded message, without the two fields that describe the
  encoded one. ZeroDDS sends with A and P unset; Cyclone, Fast DDS and
  OpenDDS set neither. Tests:
  `a_message_with_the_flag_a_is_checked_with_its_header`,
  `the_header_extension_is_adjusted_for_the_aad_and_the_decoded_message`.

- **The permissions are evaluated as DDS-Security 1.2 §10.4.1.5.3 and
  Table 70 say.** The rules of a grant apply in document order and the
  first match decides; they were folded into two lists and any deny beat
  every allow. The domains and partitions of a deny rule, and the
  partitions and data tags of each rule, now count for that rule: an allow
  rule needs all partitions of the entity in its set (`""` without a
  `<partitions>`) and all its data tags in its set; a deny rule needs one
  partition (`*` without the element) and one data tag (every tag without
  it). `<relay>` is read; a remote reader whose grant allows only relaying
  is `EndpointAccess::RelayOnly` (see the relay entries below).
  `<subject_name_expression>` is matched with
  `fnmatch()` as §10.4.1.5.3.2.1 says. Joining follows Table 70: the
  grant's first rule for the domain decides. The "legacy matching" of the
  partitions (§10.4.1.5.3.4.1.4) is selected with the participant property
  `zerodds.sec.access.partition_matching: legacy`.

  The documents follow the XSD: `<dds>` as the root, `<validity>`, at
  least one rule, `<domains>` in every rule and `<topics>` in every
  action are required, and an element outside the XSD needs
  `must_interpret="false"` (§10.4.1.6). A grant without `<validity>` was
  valid forever; the grant-level `<domains>` and `<data_tags>` and a bare
  `<topic>` were taken. Fast DDS refuses such documents too
  (`PermissionsParser::parse_grant`). `<default>` may be absent and is
  then DENY.

  Topic, partition, domain tag and subject expressions are POSIX
  `fnmatch()`: sets, ranges, negation and the backslash; only `*` and `?`
  were known, `Sq[ua]are` matched nothing.

  Governance: a domain rule matches the participant's DomainTag
  (`<tag>`, `<tag_expression>`, `""` without them, §10.4.1.2.5.1); the
  tags were read and not applied. A missing
  `allow_unauthenticated_participants` or `enable_join_access_control` is
  TRUE (Table 70); it was FALSE. A rule with `rtps_protection_kind` other
  than NONE and `allow_unauthenticated_participants` TRUE is refused
  (§9.4.2.4 Table 33). DDS:Access:PSK returns TRUE from every check
  (§10.8.3 Table 85); it evaluated grants.

  **Migration:** permissions documents outside the XSD are refused; give
  each grant a `<validity>`, each rule `<domains>`, each action `<topics>`.
  A governance that relied on a missing `allow_unauthenticated_participants`
  or `enable_join_access_control` meaning FALSE sets it. `Grant`,
  `Permissions::find_grant`, `Governance::find_domain_rule` and the
  `check_*` functions take the new model (`Rule`, `Criteria`, `Action`,
  `Domain`); `SecurityProfileConfig` has `partition_matching`. Tests:
  `the_first_matching_rule_decides`,
  `a_deny_rule_holds_only_in_its_domains`,
  `partitions_of_allow_and_deny_rules`,
  `data_tags_of_allow_and_deny_rules`, `relay_is_an_action_of_its_own`,
  `joining_follows_the_first_rule_for_the_domain`,
  `rules_match_the_domain_tag`, `a_subject_name_expression_matches_by_fnmatch`,
  `a_document_outside_the_xsd_is_refused`,
  `missing_domain_rule_flags_are_true`,
  `rtps_protection_with_unauthenticated_participants_is_refused`,
  `domain_rules_match_the_domain_tag`,
  `sets_ranges_and_negation_match_one_character`,
  `pathname_keeps_slashes_out_of_wildcards`.

- **Crypto tokens reach their destination only, and never in plaintext**
  (DDS-Security 1.2 §7.5.4, §7.5.4.4). With two or more authenticated
  peers, the repairs, HEARTBEATs and ACKNACKs of the
  ParticipantVolatileMessageSecure channel went out unprotected: the tick
  protected them only while exactly one peer was authenticated, and a write
  protected the datagrams to every reader with the key of the one it was
  addressed to, so the others asked for them again and got them in
  plaintext, the crypto tokens with their keys included (read in the code
  and measured as a relay-only reader taking another peer's keys; since
  `9aa15bca5fc`, in `v1.0.0-rc.6`). Each volatile datagram is now protected
  with the key of the peer its INFO_DST names, or dropped and recorded as
  `security.volatile.unprotectable`. A message with a
  `destination_participant_guid` goes as DATA to that participant's reader
  only, the others get a GAP for its number, and a reader takes only what
  is addressed to its participant. Tests:
  `a_volatile_datagram_goes_out_protected_or_not_at_all`,
  `a_change_for_one_participant_reaches_the_others_as_a_gap`,
  `the_implied_content_filter`,
  `reader_does_not_take_a_message_for_another_participant`.

- **Plaintext from loopback is accepted only when configured** (DDS-Security
  1.2 §9.4.2.4 Table 33). It was accepted from `Loopback` and `LocalHost`
  whatever the governance said. **Migration:** a deployment that relied on
  it sets the participant property `zerodds.sec.plaintext_loopback: true`;
  a participant without a security configuration is unaffected. Tests:
  `classify_inbound_plain_on_loopback_needs_the_setting`,
  `inbound_loopback_plaintext_needs_the_setting`.

- **A relay-only reader is matched and gets the submessage keys only**
  (DDS-Security 1.2 §9.4.2.9.11, §9.5.1.9.5, Table 70). A writer whose
  submessages and payload are both protected has two keys, as Cyclone DDS
  and OpenDDS have; the relay's token carries the first. A peer whose SPDP
  PermissionsToken is missing or names another PluginClassName or
  MajorVersion keeps no grant. Test:
  `a_relay_only_reader_is_matched_and_cannot_read_the_payload`.

- **A sample above the fragment size is protected** (DDS-Security 1.2
  §9.5.3.3.4.4). The payload layer covered DATA only; under
  `data_protection_kind` a fragmented sample went out with its payload in
  plaintext, and a fragmented CryptoContent of another vendor was not
  opened. The whole SerializedPayload is protected as it enters the
  writer's history and carries the NonStandardPayloadFlag; each reader
  opens it before recording the change as received, a reassembled one too.
  The fast path to participants in the same process protects a datagram as
  the wire does for its target; it used the participant key. Tests:
  `matrix_fragmented_samples_all_payload_kinds`,
  `a_protected_sample_enters_the_history_as_crypto_content`.

- **A relay service forwards what it may not read** (DDS-Security 1.2
  §7.1.1.4). A reader created with `UserReaderConfig::relay` on the
  `<relay>` action of the grant does not open a protected payload; it hands
  the samples on as `UserSample::Alive` with `crypto_content`, through its
  channel only. `DcpsRuntime::write_user_sample_relayed` sends such a
  payload unchanged, on behalf of its writer (DDSI-RTPS 2.5 §8.7.9),
  protecting only the submessage with the forwarder's key. A reader takes a
  forwarded sample as the original writer's only from a writer whose grant
  allows `<relay>` on the topic, and opens it with the original writer's
  key. The routing service has a relay mode for that (`Route::relay`,
  JSON and the `relay` attribute of the native XML). Tests:
  `a_relay_forwards_what_it_cannot_read_and_the_reader_takes_it_as_the_originals`,
  `a_relay_route_forwards_on_behalf_of_the_writer`,
  `relaying_needs_the_relay_action_where_access_is_controlled`.

- **The SessionId changes after `max_blocks_per_session` blocks**
  (DDS-Security 1.2 §10.5.3.3.2 Table 81, §10.5.3.3.4). A key's SessionId
  never changed. Before ciphering, the blocks of 16 octets are counted; when
  they would exceed `max_blocks_per_session`, the SessionId is incremented
  and a new session key derived. Receivers follow it from the CryptoHeader,
  as before. The value is the participant property
  `dds.sec.crypto.maxblockspersession` (the name Fast DDS reads; the
  specification names none), default 2^32 blocks; a value that is not a
  positive whole number refuses the participant. Tests:
  `the_session_id_changes_after_max_blocks_per_session`,
  `max_blocks_per_session_reaches_the_profile_config`.

- **PKI-DH takes the algorithm names of DDS-Security 1.2** (§8.2 Table 25,
  §8.3 Table 26, §10.3.2.5.1). `c.dsign_algo` `ECDSA+P256+SHA256` and
  `RSASSA-PSS-MGF1SHA256+2048+SHA256` and `c.kagree_algo` `ECDHE-CEUM+P256`
  were refused; they are taken next to their names of 1.1, which Table 25
  and 26 keep as deprecated and which ZeroDDS sends, as Cyclone, Fast DDS
  and OpenDDS do. A replier answers in the names of the request. `X25519`,
  a vendor extension, is taken only where this side runs it itself; it was
  taken from any initiator. Tests:
  `the_algorithm_names_of_1_2_and_1_1_are_taken`,
  `a_replier_answers_in_the_names_of_the_request`.

- **PKI-DH checks identity certificates against a CRL** (DDS-Security 1.2
  §10.3.3 Table 59: "The operation shall check a CRL and/or an OCSP (RFC
  2560) responder"). The two CRL and OCSP modules had no caller, and a
  revoked certificate was taken. The participant property `dds.sec.auth.crl`
  (the name Cyclone DDS reads; `dds.sec.auth.builtin.PKI-DH.identity_crl`,
  the one Fast DDS reads, too) names the CRL(s) of the identity CA, PEM or
  DER; the local and every remote identity certificate are checked against
  them in the path validation of rustls-webpki, with the signature of the CRL
  against its issuer and its `nextUpdate`. A CRL together with the X.509 v1
  deviation is refused, since that path does not consult it. OCSP is not
  done. Tests: `a_certificate_the_crl_lists_is_refused`,
  `an_expired_crl_is_refused_and_one_of_another_ca_has_no_say`,
  `crls_are_read_from_pem_and_der`.

- **The PreSharedKeyFlag of the SRTPS_PREFIX is 0x04** (DDS-Security 1.2
  §7.4.7.8.1 `|X|X|X|X|X|P|A|E|`, Table 7). `zerodds-security-rtps` set 0x02,
  the AdditionalAuthenticatedDataFlag, for a message protected with a
  pre-shared key. Test: `psk_encode_sets_pre_shared_key_flag`.

- **A failed handshake step is recorded with its reason** (DDS-Security 1.2
  §8.3.2.9). The runtime dropped the error of `begin_handshake_reply` and
  `process_handshake`; the peer stayed unmatched without a trace. It is now
  the event `security.handshake.failed` with `peer` and `reason`, and an
  entry `authentication.handshake.failed` in the security log. Test:
  `a_refused_handshake_request_is_recorded_with_its_reason`.

### Changed

- **BREAKING (generated Rust): an IDL `map` is a `Vec<MapEntry<K, V>>`**
  (`9ea4944e251`, `06ca537fe49`). XTypes 1.3 §7.5.1.3.6 binds a map to the
  sequence of `MapEntry` where a language has no map of its own; rc.6
  generated a `BTreeMap<K, V>`, whose entries come out in ascending key
  order rather than in the order they were put in. The entry is
  `zerodds_wire::MapEntry { key, value }`. **Migration:** build the
  entries (`vec![MapEntry::new(k, v)]`), or keep the `BTreeMap` with
  `zerodds-idlc --rust-map btree-map` or, in a `build.rs`,
  `zerodds_build::Config::rust_map(RustMapForm::BTreeMap)`.

- **BREAKING: `UserWriterConfig` and `UserReaderConfig` grew fields since
  rc.6.** `UserWriterConfig`: `data_max_size_serialized`, `entity_name`,
  `group_entity_id`, `durability_service`, `latency_budget`,
  `destination_order`, `transport_priority`, `presentation`, `property`,
  `data_tags`. `UserReaderConfig`, besides `resource_limits` below:
  `entity_name`, `group_entity_id`, `time_based_filter`, `latency_budget`,
  `destination_order`, `presentation`, `content_filter`, `relay`,
  `property`, `data_tags`. A struct literal that names every field does
  not compile. **Migration:** both have a `Default` (`UserWriterConfig`
  since `f2b52d2cd1e`); `..Default::default()` keeps a literal to the
  fields that differ.

- **BREAKING (`zerodds-rtps`): `ParticipantBuiltinTopicData` grew six
  fields** besides the locator lists below: `domain_tag`,
  `available_builtin_endpoints_ext`, `expects_inline_qos`,
  `manual_liveliness_count`, `entity_name`, `builtin_endpoint_qos`. The
  struct has no `Default`; a struct literal names them.

- **BREAKING (`zerodds-security`): `LoggingPlugin` has the operations of
  DDS-Security 1.2 Table 46** (`889ebc4488b`): `set_log_options`,
  `enable_logging` and `set_listener` (§9.6.2.2.1, .3, .4) are required
  methods. An implementation outside the workspace implements them; `log`
  logs according to the options only after `enable_logging`.

- **The C++ generators prefix an identifier that is a C++ reserved word
  with `cxx_`** (DDS-RPC 1.0 §7.11.1.1.2 rule 8, `cxx_name` in
  `crates/lang-rules/src/adapter/cpp.rs`): a member `class` is `cxx_class`.
  rc.6 appended an underscore (`class_`, GitHub #14). Code that names the
  escaped member changes with it; the IDL name on the wire and in the
  TypeObject is unchanged.

- **`zerodds-build`: several units that share a module are one output.**
  `compile_bundle(idls, name)` composes them into one model and writes
  `name.rs` with each shared module declared once (GitHub #39,
  `tests/reported_issues.rs`). `compile(idls)` still writes one file per
  unit. `flatten_shared_includes` and `cdr_only` of the build API on
  `main` before the rebuild are gone: the first is `compile_bundle`, the
  second selected a codec that no longer exists.

- **The DDS-XML reader takes the DDS-XML 1.0 forms and refuses the rest
  by default** (DDS-XML 1.0, with the normative XSD files of the OMG, which
  `crates/xml/schemas` now ships unchanged). An element the schemas do not
  have in its place ends the load with `XmlError::UnknownElement` naming
  its path; it was skipped without a message. The spellings other products
  write for what the schemas define (`participant_qos` and
  `domainparticipant_qos`, `<topic_filter>` as an element, `qos_profile_ref`,
  IDL type names such as `long`, `extensibility="appendable"`, enumeration
  values such as `RELIABLE`) are refused with `XmlError::VendorForm`, which
  names the DDS-XML form. Octet sequences are read from `<valueB64>`,
  partition names from `<name><element>`, and `initial_samples` and
  `initial_instances` are read. Migration: `ReadOptions::report_and_skip()`
  leaves an unknown element out and lists it with its path,
  `ReadOptions::accept_vendor_forms()` reads the vendor forms and reports
  each; `zerodds-admin` and `zerodds-xmlc` have `--skip-unknown` and
  `--vendor-forms`, the XRCE agent `vendor_forms: accept`.

- **XRCE: the Annex A types without `@extensibility` are APPENDABLE, and the
  agent follows DDS-XRCE 1.0 where eProsima Micro-XRCE-DDS does not**
  (DDS-XTypes 1.3 §7.3.1.2.1.8). `ACKNACK_Payload` and the other unannotated
  Annex A types carry a DHEADER, so an ACKNACK is 17 octets instead of 13;
  readers accept both forms. A CREATE in a representation format §7.7.3.1
  does not allow for its object kind is `STATUS_ERR_INVALID_DATA`, a topic
  of a type the agent does not hold `STATUS_ERR_UNKNOWN_REFERENCE`
  (§7.7.3.7.3), and a WRITE_DATA is answered with a STATUS. The readings
  the specification leaves open are named settings: `representation_timestamps`,
  `sample_info`, `status_agent`, `delete_client_reply`,
  `max_elapsed_time_unit`, `unsupported_qos`, `update_of_missing_object`,
  `same_session_create_client`, `read_without_delivery_control`.
  Migration for eProsima clients: `unannotated_types: final` on the agent
  (and the unannotated-types option of the SDK clients),
  `representation_formats: any`, `unknown_topic_types: forward`,
  `status_agent: with_result_status`.

- **DataRepresentation matches strictly, and a reader without PID 0x0074
  is held to DISALLOW_TYPE_COERCION** (DDS-XTypes 1.3 §7.6.3.1.1,
  §7.6.3.4.1). A writer's offered representation has to be in the reader's
  sequence; any overlap matched before. A remote reader that announces no
  TypeConsistencyEnforcement is matched by equivalence; it was taken for
  ALLOW_TYPE_COERCION. Migration: `wire.data_representation_match:
  tolerant` (`ZERODDS_DATA_REPR_MATCH=tolerant`) restores the overlap rule,
  `xtypes.type_consistency_absent: allow`
  (`ZERODDS_TYPE_CONSISTENCY_ABSENT=allow`) the policy's default for such a
  reader. A reader that announces DISALLOW itself keeps it under `allow`
  (`SubscriptionBuiltinTopicData::type_consistency_announced`). Tests:
  `the_xtypes_defaults_are_the_clauses_and_each_has_a_named_way_back`,
  `runtime_config_data_rep_match_mode_default_is_strict`.

- **A user payload is padded to a multiple of four octets** and the two
  low bits of the encapsulation options give the count (DDS-XTypes 1.3
  §7.6.3.1.2); the options were always 0 and the payload ended where its
  data ended. A reader removes exactly the padding the options name. Test:
  `a_user_payload_names_its_padding_in_the_options`.

- **C API: `set_qos` with NULL takes the factory's current default**
  (DDS 1.4 §2.2.2.1.1.1, `*_QOS_DEFAULT`); it took the specification's
  default. `zerodds_dp_get_current_time` reads the participant's clock
  (`time_source`, §2.2.2.2.1.32) and returns NOT_ENABLED for a disabled
  participant; it read the system clock. Test:
  `set_qos_with_null_takes_the_current_default_of_the_factory`.

- **Default QoS of the factories, `*_QOS_DEFAULT`, `*_QOS_USE_TOPIC_QOS`,
  `THE_PARTICIPANT_FACTORY`** (DDS 1.4 §2.2.2.2.1.20–25, §2.2.2.4.1.15–16,
  §2.2.2.5.2.15–16, §2.2.2.2.2.1, §2.3.3). `DomainParticipant` has
  `set_default_{topic,publisher,subscriber}_qos` and the getters,
  `Publisher` `set_default_datawriter_qos`, `Subscriber`
  `set_default_datareader_qos`. The QoS argument of `create_*`, `set_qos`
  and `set_default_*_qos` is `impl Into<QosArg<Q>>`
  (`impl Into<EndpointQosArg<Q>>` for `create_datawriter`/`create_datareader`):
  a QoS value reads as before, `PUBLISHER_QOS_DEFAULT` and the others take
  the factory's default, `DATAWRITER_QOS_USE_TOPIC_QOS` that default with
  the Topic QoS copied over it. `create_participant` did not read
  `set_default_participant_qos`; `ParticipantBuilder` without `with_qos` now
  takes it. `set_qos(*_QOS_DEFAULT)` applies the factory's current default
  under the immutability rules of a value. `Publisher::get_participant`,
  `Subscriber::get_participant`. **Migration:** a call that passed
  `Default::default()` as the QoS no longer infers its type; name it
  (`PublisherQos::default()`) or pass the placeholder. Tests:
  `crates/dcps/tests/default_qos.rs`.

- **A read or take keeps an instance's samples together and, under GROUP
  scope, wants `begin_access`** (DDS 1.4 §2.2.2.5.3.8, §2.2.2.5.2.8). The
  samples of an instance are consecutive except under TOPIC or GROUP with
  `ordered_access`; the default presentation interleaved them. Under GROUP
  with `ordered_access` a read or take returns at most one sample. Under
  GROUP scope every read and take outside `begin_access`/`end_access` is
  PRECONDITION_NOT_MET, except inside a data listener; only
  `get_datareaders` asked. Tests:
  `the_default_presentation_groups_the_plain_take_by_instance`,
  `a_take_under_group_scope_wants_begin_access`.

- **`DomainParticipant::get_current_time`** (DDS 1.4 §2.2.2.2.1.32): the
  time the runtime stamps writes with (`RuntimeConfig::time_source`), the
  system clock offline. The participant had no such operation. Test:
  `get_current_time_is_the_clock_writes_are_stamped_with`.

- **The state masks have the values of the IDL PSM** (DDS 1.4 §2.3.3):
  `sample_info::sample_state_mask::READ` is `0x0001`, `NOT_READ` `0x0002`,
  and the `ANY` masks of sample, view and instance state are `0xffff`, as
  `psm_constants::state` has them. `READ` and `NOT_READ` were the other way
  round, so a caller passing the PSM constant to `read`/`take` or
  `create_readcondition` got READ samples for NOT_READ. Python:
  `zerodds.sample_state.READ == 1`, `NOT_READ == 2`; the C API had the PSM
  values. Test: `the_state_masks_are_those_of_the_idl`.

- **`delete_participant` refuses a participant that still has entities,
  and deletes the participant** (DDS 1.4 §2.2.2.2.2.2: "Otherwise the error
  PRECONDITION_NOT_MET is returned"). It deleted the contained entities
  itself and left the participant working. Call
  `delete_contained_entities` first; a deleted participant's runtime stops,
  and its operations return ALREADY_DELETED. The C API refused already.
  Test: `delete_participant_wants_its_entities_deleted_first`.

- **On equal strength the writer with the lower GUID owns an EXCLUSIVE
  instance**, as in Cyclone DDS (DDS 1.4 §2.2.3.9.2: "all DataReader
  objects will make the same choice"). The higher one did. Test:
  `exclusive_ownership_tie_break_by_lower_guid`.

- **Group operations of a disabled Publisher or Subscriber return
  NOT_ENABLED** (DDS 1.4 §2.2.2.1.1.7): `suspend_publications`,
  `resume_publications`, `begin_coherent_changes`, `end_coherent_changes`,
  `begin_access`, `end_access`, `get_datareaders`, `notify_datareaders`
  and `ignore_*`. `Subscriber::begin_access` returns a Result. Test:
  `group_operations_of_a_disabled_publisher_or_subscriber_are_not_enabled`.

- **`begin_coherent_changes` nests; `suspend_publications` and
  `resume_publications` return a Result** (DDS 1.4 §2.2.2.4.1.8–10). A
  coherent set ends with the `end_coherent_changes` that matches the first
  `begin`; an inner pair closed it. Each `suspend` wants a `resume`, and a
  `resume` without one is PRECONDITION_NOT_MET, in Rust and in the C API;
  it did nothing and returned OK. Tests:
  `nested_coherent_changes_end_with_the_last_end`,
  `resume_without_suspend_is_precondition_not_met`.

- **The XRCE agent's clock reaches TCP and serial clients** (DDS-XRCE 1.0
  §11.3.2, §8.4.14.1, §8.3.5.9). Only the UDP listener called `tick`, and it
  sent its output — HEARTBEATs of the reliable streams, the samples of a
  running READ_DATA, the FRAGMENTs that follow — for a TCP client over UDP to
  the TCP address; over TCP and serial none of it arrived. One ticker thread
  now drives the engine for all listeners and sends each message over the
  transport its client registered on: a UDP listener's socket, or the queue
  of the writer of a TCP connection or serial line; the routes of a closed
  connection go with it. Test:
  `a_tcp_client_gets_the_messages_of_the_agent_over_its_connection`.

- **The XRCE agent reads after DDS-XRCE 1.0 §7.8.5.1 Table 9 and
  §8.3.5.9.2.** `max_samples` 0 (`MAX_SAMPLES_ZERO`) cancels the active
  read of the DataReader and sends nothing more; it started a read without
  a limit. 0xFFFF (`MAX_SAMPLES_UNLIMITED`) has no limit; it ended the read
  after 65 535 samples. A READ_DATA without `delivery_control` calls the
  read once and returns the samples in the cache; it returned the first of
  several and then the next sample that arrived. `preferred_stream_id`
  STREAMID_NONE lets the agent choose: DATA goes on the built-in
  best-effort stream, where it went on STREAMID_NONE. A read takes from the
  DataReader only the samples it delivers: up to 64 were taken at every
  pump, and what a read ended by `max_samples` had not delivered was lost,
  also for the next read. A sample longer than a DATA submessage carries
  (65 535 octets of body) is reported and answered with STATUS
  `STATUS_ERR_RESOURCES`; it was skipped without a word.

  `read_without_delivery_control: next-sample` reads as Micro-XRCE-DDS
  3.0.1 does (`DataReader::read`: `max_samples(1)` without a time limit);
  the default is `once`. `zerodds-xrce`: `MAX_SAMPLES_ZERO`,
  `MAX_SAMPLES_UNLIMITED`, `ReadStream::once`, `ReadStream::capacity`;
  `DeliveryControl::single_shot` is gone and the default of `max_samples`
  is `MAX_SAMPLES_UNLIMITED`. Tests:
  `max_samples_zero_cancels_the_active_read`,
  `a_read_without_delivery_control_returns_the_cache_once`,
  `next_sample_waits_for_one_sample`,
  `a_read_takes_only_the_samples_it_delivers`,
  `streamid_none_lets_the_agent_choose_the_stream`,
  `a_sample_too_large_for_data_is_reported`,
  `max_samples_zero_ends_the_read_and_unlimited_has_no_limit`,
  `the_capacity_is_what_the_stream_still_delivers`.

- **HEARTBEAT of agent and client is tested on STREAMID_NONE** (DDS-XRCE
  1.0 §8.3.5.12). Tests: `a_heartbeat_of_the_agent_is_on_streamid_none`,
  `a_heartbeat_of_the_client_is_on_streamid_none`.

- **The XRCE serial line is framed after DDS-XRCE 1.0 Annex C.1**, in
  `zerodds-xrce`, the agent and the C, C++, Java, Python and Ada-native
  endpoint SDKs. A frame was `7E | stuffed payload | CRC big endian | 7E`:
  no `sourceAddress`, `remoteAddress` or `payloadLength`, and the CRC was
  CCITT-FALSE (not reflected, not complemented, 0x29B1 for "123456789") over
  the payload. A frame is now `7E | sourceAddress | remoteAddress |
  payloadLength LE | payload | frameCheck LE` (C.1.1); `frameCheck` is the
  FCS-16 of RFC 1662 over addresses, length and payload (C.1.1.6, 0x906E for
  "123456789"). A frame ends after its length, and a non-escaped `0x7E`
  before that drops it and begins the next (C.1.3).

  The agent took every frame of the line as the frames of one client,
  `[0xFE; 4]`, whatever the client's ClientKey, and its replies carried no
  address. It now has an address on the line (`address:`, default 0) and
  takes only the frames to it; a frame belongs to the client its header or
  CREATE_CLIENT names, else to the client registered from its
  `sourceAddress`, and the agent's messages go to that address. A failed
  write or encode on the line is reported; a read error other than the
  timeout ends the listener with the error, where it retried without a word.

  eProsima Micro-XRCE-DDS 3.0.1 frames as Annex C does but computes
  `frameCheck` as CRC-16/ARC over the payload alone
  (`StreamFramingProtocol.cpp`, `stream_framing_protocol.c`). The agent
  takes and makes those frames with `frame_check: crc16-arc-payload`;
  `zerodds-xrce` has `FrameCheck::Crc16ArcPayload`. The SDKs frame after
  Annex C only. Tests:
  `the_frame_check_is_the_fcs16_of_rfc_1662`,
  `a_frame_carries_the_addresses_the_length_and_a_little_endian_check`,
  `a_begin_flag_inside_a_frame_drops_it_and_begins_the_next`,
  `the_frame_check_of_micro_xrce_dds_is_taken_when_chosen`,
  `the_clients_of_a_serial_line_are_told_apart_by_their_address`,
  `a_serial_listener_takes_its_address_and_frame_check`, and the serial
  goldens of the SDKs.

- **A reassembled XRCE `FRAGMENT` run yields every submessage** (DDS-XRCE
  1.0 §8.3.5.14, 4-aligned as after a header, §8.3.3), at the agent and at
  the client. Both took the first and dropped the rest without a word. A
  run that is no sequence of submessages is a STATUS
  `STATUS_ERR_INVALID_DATA` at the agent and the new
  `ClientEvent::FragmentDiscarded` at the client, as is a failed
  reassembly at the client. `zerodds_xrce::submessages::decode_submessages`
  is the decoder `Message::decode` uses. Tests:
  `a_fragmented_run_of_two_submessages_runs_both`,
  `a_submessage_run_decodes_every_submessage_past_the_padding`.

- **`zerodds_sub_get_datareaders` takes the three state masks** of DDS 1.4
  §2.2.2.5.2.10 and lists the readers that hold samples in them; under
  GROUP access scope with `ordered_access` a reader once per sample, by
  source timestamp. It listed every reader, with samples or not.

  **Migration:** append `sample_states, view_states, instance_states`
  (`0, 0, 0` for any state) to the call; a reader without samples is not
  in the list.

- **`zerodds_sub_notify_datareaders` calls `on_data_available`** of every
  DataReader of the subscriber whose DATA_AVAILABLE changed (DDS 1.4
  §2.2.2.5.2.14), on the calling thread; it did nothing. The call covers
  the data: the next `zerodds_poll_listeners` does not report it again, and
  a poll's call is not repeated by `notify_datareaders` (§2.2.4.2.2). Test:
  `notify_datareaders_calls_on_data_available_once_per_change`.

- **A C reader applies HISTORY and RESOURCE_LIMITS** (DDS 1.4 §2.2.3.18,
  §2.2.3.19), as the typed reader does: KEEP_LAST(depth) keeps the newest
  `depth` samples of an instance; over `max_samples_per_instance` or
  `max_samples` a BEST_EFFORT reader drops the oldest, a RELIABLE one
  refuses the sample with SAMPLE_REJECTED and delivers it with a later
  take. It applied `max_instances` only and kept every sample: the
  default KEEP_LAST(1) now holds one sample per instance, and a C, C++ or
  C# reader that wants all of them sets KEEP_ALL. Tests:
  `crates/zerodds-c-api/tests/reader_resource_limits.rs`.

- **TIME_BASED_FILTER measures on a clock the setting
  `reader.time_based_filter_clock` chooses, and a RELIABLE reader gets
  the last filtered sample** (DDS 1.4 §2.2.3.12, which leaves the clock
  open):
  - `sender`, the default: the source timestamp of the change (INFO_TS),
    the reception time for a sample without one. `receiver`: the
    reception time. Env `ZERODDS_READER_TIME_BASED_FILTER_CLOCK`.
  - The typed reader measured with the time of its ingest, at the
    application's read. A writer that filters for the reader itself lost
    every sample that came in less than `minimum_separation` after the one
    before: InterCom DDS 4.3.1 → ZeroDDS delivered `size` 17, 37, 47, 57 …
    of 7, 17, 27 … (OMG `Test_TimeBasedFilter_0/1`).
  - Under RELIABLE the last filtered sample of each instance is delivered
    `minimum_separation` after it arrived when none passed until then
    ("the system should guarantee delivery the last sample"), with
    DATA_AVAILABLE, in the typed reader and in the C API and rmw. It was
    dropped.
  - The runtime path of the C API and rmw filters by the key hash it
    computed, not only by the writer's `PID_KEY_HASH` (without it all
    instances were one), and a reader in the writer's participant is
    filtered too. Under BY_SOURCE_TIMESTAMP the filter compared the
    reception time against the order's source timestamp; the two keep
    their own time.

  `UserSample::Alive` has the field `reception_timestamp`; `DcpsRuntime`
  has `add_tick_hook`. Tests: `crates/dcps/tests/time_based_filter_qos.rs`,
  `crates/zerodds-c-api/tests/time_based_filter.rs`.

- **The C API has coherent sets, the GROUP order and the PRESENTATION
  read order** (DDS 1.4 §2.2.2.4.1.9–10, §2.2.2.5.2.8–9, §2.2.2.5.3.8,
  §2.2.3.6; DDSI-RTPS 2.5 §8.7.5, §8.7.6), on the path of the typed API:
  - `zerodds_pub_begin_coherent_changes` and
    `zerodds_pub_end_coherent_changes` open and close a set on every writer
    of the publisher. The calls nest, a writer created inside an open set
    joins it, and an end without a set is PRECONDITION_NOT_MET. Under
    GROUP access scope the set spans the writers, which draw their group
    sequence numbers from the publisher's group; a writer is neither
    created nor deleted while the set is open (§8.7.6), in Rust
    `delete_contained_entities` neither.
  - A C reader under a subscriber with `coherent_access` takes a set whole
    once it ended and discards an incomplete one with the event
    `reader.coherent_set.incomplete_discarded`. Under GROUP access scope
    the subscriber orders the samples of a GROUP-scope publisher across its
    readers.
  - `zerodds_sub_begin_access` and `zerodds_sub_end_access` count: under
    GROUP access scope a read, a take and `get_datareaders` without an open
    `begin_access` are PRECONDITION_NOT_MET, except inside a data listener,
    and an `end_access` without one too.
  - A read or take returns the samples of an instance consecutively, in
    write order under TOPIC scope with `ordered_access`, one per call under
    GROUP scope with `ordered_access`.
  - C writers announce their publisher as `PID_GROUP_GUID`, C readers their
    subscriber.

  The four calls did nothing and returned OK; the reader handed out the
  members of an open set and the instances interleaved. The hold and the
  group order are one code for both APIs now,
  `zerodds_dcps::presentation`. Tests: `crates/zerodds-c-api/tests/presentation.rs`.

- **DATA_AVAILABLE and DATA_ON_READERS are flags** (DDS 1.4 §2.2.4.2.2):
  an `on_data_available` call or any read or take lowers DATA_AVAILABLE;
  `on_data_on_readers`, `on_data_available` of any reader or a read or take
  on any reader of the Subscriber lowers DATA_ON_READERS. They were up
  while unread samples were held: a `read_instance` of one instance left
  them up for another's, a WaitSet on them returned at once again, and a
  listener call left them up until the application read. Tests:
  `a_read_of_one_instance_lowers_data_available`,
  `an_on_data_available_call_lowers_data_available`,
  `a_take_on_one_reader_lowers_data_on_readers`.

- **Deleting a DataWriter unregisters its instances and, with
  WRITER_DATA_LIFECYCLE `autodispose_unregistered_instances` (default
  TRUE), disposes them** (DDS 1.4 §2.2.2.4.1.6, §2.2.3.21: the flag
  "will determine whether instances are ultimately disposed when the
  DataWriter is deleted"). A deleted writer sent no lifecycle marker;
  its readers took the instances to NOT_ALIVE_NO_WRITERS when the SEDP
  dispose of the publication arrived. `writer.unregister_on_delete:
  false` (env `ZERODDS_WRITER_UNREGISTER_ON_DELETE=0`) keeps that way.
  It is the state a reader ends in with Connext 7.7.0, Fast DDS 3.6.1,
  CoreDX 6.15.3 and HDDS 1.4.6 as the publisher, which the OMG dds-rtps
  case `Test_FinalInstanceState_2` asks for; with the default, ZeroDDS
  as the publisher fails that case against Connext and Fast DDS, as
  Cyclone 11.0.1 and OpenDDS 3.35 do against themselves (measured on the
  Linux test host). Test:
  `delete_datawriter_disposes_and_unregisters_its_instances`.

- **Deleting a reliable DataWriter waits for its readers to acknowledge
  what it sent**, the unregister and dispose markers of §2.2.2.4.1.6
  among it, at most `writer.linger_ms` (env `ZERODDS_WRITER_LINGER_MS`,
  default 1000, the default of Cyclone DDS's
  `Internal/WriterLingerDuration`; `0` removes the writer at once). It
  asks for the acknowledgment with a HEARTBEAT; a writer whose readers
  have everything goes at once. The writer used to remove itself right
  after the markers, its SEDP dispose 77 µs behind the last one, and
  could repair none of them; now Connext 7.7.0 acknowledges every marker
  before the SEDP dispose goes out (both measured with the OMG dds-rtps
  `Test_FinalInstanceState_1`). In 1 of 14 runs of that case before,
  Connext reported NOT_ALIVE_NO_WRITERS for the disposed instances, in 0
  of 10 after; the run was not captured, its cause is not established.
  Test: `a_deleted_writer_goes_once_its_reader_has_the_dispose`.

- **`on_data_on_readers` takes an arrival from `on_data_available`**
  (DDS 1.4 §2.2.4.4: "if on_data_on_readers is called, then the
  middleware will not try to call on_data_available"). Both were called.
  `Subscriber::notify_datareaders` hands the change on to the
  DataReaderListeners. Tests: `listener_integration.rs`, six
  tests changed to the specification's expectations.

- **`DataWriter<T>` and `DataReader<T>` are handles**: `Clone` gives
  another handle to the same endpoint, as `lookup_datawriter` and
  `lookup_datareader` do. The endpoint ends with its deletion or with
  its last handle, as it ended with its value before; an operation on a
  handle of a deleted endpoint returns `AlreadyDeleted` (§2.2.1.1).
  `DataReader::with_filter` sets the filter for every handle.

- **`Topic<T>` and `ContentFilteredTopic<T>` are `Send` and `Sync`
  whatever `T` is**: they hold no `T` (`PhantomData<fn() -> T>`).

- **A DataReader's data listeners run on the thread that handed it the
  samples** — a receive thread, or the writing thread for a reader of the
  same participant — as Cyclone DDS and Fast DDS call them, and one
  thread at a time per reader. They ran on the listener dispatcher's
  thread, which spun for 50 µs after every data pass so the receive
  thread need not wake it per datagram, and so held a core for as long
  as a stream ran: 0.94–1.07 µs of CPU per sample on the Linux test host
  with a 1 kB BEST_EFFORT stream at 800 kS/s; it takes none now, with
  the taking and the receive thread as before. A RELIABLE write from
  inside a data listener does not wait for the flow-control window: the
  thread may be the one that processes the acknowledgements. The other
  statuses' listeners stay on the dispatcher. Test:
  `a_listener_writes_from_the_receive_thread_without_waiting_for_the_window`.

- **`DataReader<T>`'s operations ask `T: Send + 'static`**, as
  `Subscriber::create_datareader` already did: `take` hands out the `T`
  admission decoded, which the reader cache keeps as
  `Box<dyn Any + Send>`. **Migration:** generic code over a
  `DataReader<T>` with `T: DdsType` alone adds `Send + 'static` to its
  bound.

- **The zerodds-async §9.1 gate (`write().await` within 5 % of
  `write()`) measures one live writer on the release build.** It compared
  two offline writers whose writes only fill a test queue, 78 ns each on
  the Linux test host, in the instrumented debug build of the `test` job;
  pipeline 7214 failed it at 1.070 once the sync write had lost its
  per-write allocations. Two sync writers measured that way differed by as
  much as the overhead looked for, and against 78 ns any `.await` is 7 %:
  an `async` block around the sync write cost 5.2–5.9 ns,
  `AsyncDataWriter::write` 5.8–6.4 ns. The test now times the async
  writer's own sync writer against its `write().await` on a live
  participant without a reader, alternating which goes first, and is
  `#[ignore]` under `debug_assertions`; `perf-gate` runs it with
  `--release`: 1.003–1.029 over six runs, about 511 ns per sync write.
  `AsyncDataWriter::write` keeps its retry loop in a future of its own,
  entered only on `OutOfResources`: inline it made every write's future
  larger, 10 ns per offline write against 3.5 ns apart.

- **`OutboundDatagram::targets` and `MessageBuilder::open` take an
  `Arc<Vec<Locator>>` instead of an `Rc<Vec<Locator>>`** (`zerodds-rtps`).
  A writer now keeps one target list per matched reader and hands every
  datagram a reference to it; a writer moves between threads, so the list
  it keeps has to be `Send`. Callers that built an `Rc::new(vec![..])`
  build an `Arc::new(vec![..])`.

- **`zerodds_perf sub` takes when its listener wakes it.** It used to sleep
  1 ms between `take()` calls, so under `KEEP_LAST(1)` its loss column
  measured its own loop: about 500 samples a second of a 440 kS/s stream.
  `ZERODDS_PERF_SUB_POLL_US` still selects the poll loop. `pub` writes one
  sample over and over with its counter moved, as `ddsperf` does, and reads
  the clock once per 256 writes.

- **The `bench-regression` baselines are reseeded on the CI runner**
  (pipeline 7166, `RUN_BENCH_REGRESSION_UPDATE=true`), each transport
  without the same-host channel and the channel as a cell of its own: p50
  roundtrip, median of three runs, UDPv4/64 41.25 µs, TCPv4/64 86.59 µs,
  SHM/64 59.94 µs, SameHost/64 29.87 µs (4096 bytes: 42.40, 99.07, 65.79,
  33.38). The old TCPv4 and SHM rows, 64.51 and 64.67 µs, were taken while
  the same-host path carried every transport's payload.

- **`bench-regression` measures each transport and the same-host channel
  apart.** Since `c996fd7a076` the same-host channel carries datagrams to
  UDP locators only, so the `UDPv4` cell measured the channel; before it,
  the older same-host path carried user data for every transport and the
  `TCPv4` and `SHM` cells measured that path too. The jump the nightly of
  2026-10-06 reported (TCPv4/64 p50 90.1 µs against a baseline of 64.5) is
  that change and not a slower transport: on the Linux test host without the channel,
  TCPv4/64 is 50.8 µs at `8f94dbd7d6e` and 58.6 µs now, SHM/64 31.8 and
  32.7, UDPv4/64 34.7 and 36.8. The transport cells now run with
  `ZERODDS_SAME_HOST_SHM=0` and a `SameHost` cell measures the channel.
- **The Homebrew channel has a build.** A `macos-bundle` job on a runner
  tagged `macos` packs `zerodds-<v>-{aarch64,x86_64}-apple-darwin.tar.gz`
  with their sha256, `render-homebrew.sh` renders the formulae from them,
  and the bridge sub-formulae are service formulae over the meta
  formula's binaries with an `install` method. The runner is not
  registered yet, so the job has not run.
- **`ros-<distro>-rmw-zerodds-cpp` is built for Humble, Jazzy, Kilted and
  Rolling** (`ros-rmw-deb` as a matrix), Iron is out of the supported list,
  and the job has two processes exchange twenty `std_msgs/String`
  messages through the installed package. Only the Jazzy build has run so
  far (nightly 7117), before the matrix and the exchange were added.

- **A ZeroDDS GUID prefix opens with the vendor id** (`06ee6f30fe9`).
  DDSI-RTPS 2.5 §9.3.1.5 puts `vendorId` in octets 0..2 of `guidPrefix`
  ("shall"); ZeroDDS put a host id there. `GuidPrefix::zerodds` writes
  `0x01F0`, the host id in octets 2..6 (`GuidPrefix::host_id` reads it
  there) and six octets made unique from the PID and a clock-seeded
  counter. Every generator uses it. The wire identity of every
  participant changes; same-host SHM and UDS decisions read the new
  host id.

- **BREAKING: a `DdsType` without a big-endian codec refuses big-endian**
  (`0df940ec023`). `encode_be`/`decode_be` defaulted to the
  little-endian codec and wrote little-endian octets under a big-endian
  encapsulation. They refuse now unless the type sets
  `BYTE_ORDER_AGNOSTIC`.

- **BREAKING: `ReconnectExhausted` carries the last error**
  (`06ee6f30fe9`). `amqp-dds-endpoint`'s
  `ClientError::ReconnectExhausted { attempts, last }` keeps the error of
  the last attempt; it was a count alone.

- **BREAKING: `zerodds_idl_parse::Diagnostic` has a `file` field**
  (`70ac19a8b05`), `None` for the unit itself. A struct literal adds
  `file: None`.

- **Model snapshots are version 3** (`c5910638396`). A version 2
  snapshot is refused rather than misread.

- **Status listeners run when a status changes** (`18d4f22089b`,
  `54d289e7d3f`). DDS 1.4 §2.2.4.1: a reader's, writer's and topic's
  communication statuses were dispatched only inside the status getters
  and `drive_listeners()`. A dispatcher thread per runtime runs them on
  the runtime's status hook; a first poll at zero dispatches nothing.

- **21 crates no longer claim `no_std`** (`e57af154bcd`). They did not
  build for `thumbv7em-none-eabihf`; the category, the keyword and the
  documentation line went. The remaining 23 are built by
  `tools/no-std-claims-gate.sh` in the CI job `no-std`.

- **`#![no_std]` in `lib.rs` is a claim the gate measures** (GitHub #44–#59).
  73 crates carried `#![no_std]` or `#![cfg_attr(not(feature = "std"),
  no_std)]`; 34 of them did not build for `thumbv7em-none-eabihf` with
  `--no-default-features` (plus `alloc`). 19 build now: workspace
  dependencies are taken without default features and `std`/`alloc` are
  passed on, socket2 and the monitor are optional under `std` in
  `zerodds-transport-udp`, roxmltree is taken without default features, and
  the parts behind a `std::sync::Mutex` are behind `std` — the POA instance,
  the CosEvent channels, the CCM `HomeFinder` and `PortRegistry`, the
  in-memory `NamingContext`, the D&C `ContainerHost`, the bridge servant.
  `zerodds-dlrl` takes its 64-bit atomics from portable-atomic. 15 crates
  lose the attribute because their function needs `std`: `zerodds-dcps`
  (`alloc` implied `std` already), `-dcps-async`, `-rpc`, `-rt-linux`,
  `-transport-shm`, `-monitor`, `-proto`, `-conformance`, `-ts-wasm` and the
  six security plugin crates `-crypto`, `-keyexchange`, `-pki`,
  `-permissions`, `-runtime`, `-logging`. `tools/no-std-claims-gate.sh`
  reads the attribute as well as the manifest: 58 crates, 0 failures.

- **BREAKING: the default dialect is the specification, not Cyclone**
  (`b1560e5ec68`). Four axes change what goes on the wire when a caller names
  no dialect: `default_extensibility` `Final` → `Appendable` (XTypes 1.3
  §7.3.1.2.1.8), `pl_cdr1_headers` `AlwaysExtended` → the short header wherever
  §7.4.1.2.2 allows it, `must_understand` `Declared` → `DeclaredAndKey`, plus
  a `wstring`'s code-unit byte order and the branch for a negative case label
  on a signed discriminator narrower than 32 bits.

  In practice: an aggregate whose IDL carries no extensibility annotation now
  gets a DHEADER under XCDR2. Cyclone's `idlc` reads the same silence as
  `@final` and will read that DHEADER as the first member. **A caller talking
  to a Cyclone peer asks for its answers by name** — `--dialect cyclone` on
  the probes, `zerodds-idlc --default-extensibility final` for generated code,
  or `zerodds_build::Config::default_extensibility(DefaultExt::Final)` in a
  `build.rs`.

- **BREAKING (generated code): a bounded collection's helper names carry
  the bound in front of the element** (`2e8f74034fe`). `sequence<string, 4>` and
  `sequence<string<4> >` used to get one name, `seq_string_b4`, and so one
  codec body — the member declared first decided whether the length or the
  string was checked. Same for nested sequences and for a map's bound against
  its value's. Now `seq_b4_string` beside `seq_string_b4` in every column;
  unbounded names are unchanged. In C the name is also the struct a header
  declares (`zd_seq_long_b4` is `zd_seq_b4_long`), so code that names one
  has to follow.

- **BREAKING (`zerodds-security-runtime`): `SecurityProfileConfig` carries
  a `Material` per entry and the key's password** (`f44e6061f25`,
  `e29a7520cb7`). The six paths are `Material::File` or `Material::Inline`
  — a `data:` URI is content, not a path — and `identity_key_password`
  is new; so are `max_blocks_per_session` (`7f684a325da`) and
  `identity_crl` (`011b8e183b8`). **Migration:** `.into()` on a `PathBuf`,
  `identity_key_password: None` where the key is not encrypted, and
  `max_blocks_per_session: None, identity_crl: None` for the defaults.

- **BREAKING (`zerodds-lang-rules`, `zerodds-lang-cst`): `@verbatim`
  nodes in every column** (`a7c29ac5be5`). Seventeen target CSTs gained a
  `Verbatim` variant, several structs a `BodyText` field, and
  `adapter::elixir::elixir_modules` returns `elixir::TopLevel` entries.
  A `match` over one of those enums, or a struct literal of one, has to
  name the new parts.

- **BREAKING: annotations in a `CompleteTypeObject` take the form of
  DDS-XTypes 1.3 Annex B** (`45eb03d1048`). A member with `@min`, `@max`,
  `@unit` or `@hashid`, or a type with `@verbatim`, got a `COMPLETE` hash no
  vendor computes: `AppliedBuiltinMemberAnnotations` had no `DHEADER`, every
  `AnnotationParameterValue` sat behind a length of its own instead of being
  the final union, and `@default` went on the wire as a fifth member the IDL
  does not have. Measured on one struct against Cyclone DDS 11.0.1 and RTI
  Connext 7.7.0: ZeroDDS's object is now Cyclone's octet for octet, and RTI's
  decodes. `@default` stays in the in-memory object only; XTypes 1.3 has no
  slot for it. `AppliedAnnotationParameter::value` is an
  `AnnotationParameterValue`, not bytes.

- **BREAKING: `long double` goes on the wire as binary128** (`a3dc668fff5`).
  It used to be a `double` followed by eight zero bytes — right length, wrong
  value, reported as success. Now sixteen octets per CORBA 3.3 part 2
  §9.3.1.3 and XTypes 1.3 table 31, all sixteen reversed on big-endian. An
  rc.6 peer that sends or reads `long double` is not compatible with rc.7.

- **BREAKING: the encapsulation options carry the padding count**
  (`16884c5e117`), the low two bits of the second options byte per XTypes 1.3
  §7.6.3.1.2. ZeroDDS neither wrote nor read the field. Measured with tshark:
  Cyclone and RTI write `options 0x0003` for a one-octet payload — a peer that
  evaluated the field took every ZeroDDS payload of non-multiple-of-four
  length to be up to three bytes shorter than it was.

- **BREAKING (`zerodds-rtps`): `ParticipantBuiltinTopicData` locator fields are
  now `Vec<Locator>`, not `Option<Locator>`.** Every field that is semantically a
  locator list per DDSI-RTPS 2.5 §9.6.1 (`default_unicast_locators`,
  `default_multicast_locators`, `metatraffic_unicast_locators`,
  `metatraffic_multicast_locators`) changed type, and the four fields were
  renamed to the plural. This is required for #27 (a `*_LOCATOR` PID may repeat;
  the wire already allowed it — decode now retains all, in LE and BE).

  **Migration:**
  - Construction: `default_unicast_locator: Some(loc)` → `default_unicast_locators: vec![loc]`; `…: None` → `…: Vec::new()`.
  - Single-value reads: use `primary_default_unicast_locator()` /
    `primary_metatraffic_unicast_locator()` (return `Option<Locator>`, first of
    the list), or `metatraffic_or_default_unicast_locators()` for the full
    fan-out set.
  - Decode no longer filters locators; routing/usability selection moved to
    DCPS/transport (`locator_looks_routable` is a public predicate there).
  - No wire-format change and no on-the-wire compatibility break — repeated
    locator PIDs are standard RTPS and are emitted/parsed by Cyclone/FastDDS.

- **BREAKING: `ParsedSubmessage` is `#[non_exhaustive]`** (`f71a2ec85c7`). An
  exhaustive `match` outside this workspace needs a wildcard arm. The variant
  that forced it is `InfoDestination` (`1cbe25e22cc`).

- **BREAKING: two builtin-topic structs and one config struct grew a field.**
  `PublicationBuiltinTopicData` and `SubscriptionBuiltinTopicData` carry
  `vendor_id` (`52128b12557`); `UserReaderConfig` carries `resource_limits`
  (`8e3688b6fdf`). `UserReaderConfig` has a `Default` since `71b9ff4d8cc`, so
  `..Default::default()` makes the next field a change to one file rather
  than to seventy.

- **The reliable receive cache holds 65,536 samples per writer proxy, not 256**
  (`6f4d4a569b2`). Measured on a quiet host at 114.7 kS/s: the same cell lost
  between 1.78 and 6.69 percent with 256 and 0.00 with 65,536. It is a cap and
  not a reservation — the cache is a `BTreeMap` — so the memory appears only
  while an earlier sequence number is genuinely missing. A reader's own
  RESOURCE_LIMITS now reaches it (`8e3688b6fdf`); where memory is the tighter
  constraint, `ZERODDS_MAX_SAMPLES_PER_PROXY` lowers it per participant.

- **BREAKING (CMake): the option names carry a prefix** (`ae548d63f22`).
  `BUILD_ZERODDS_CPP` → `ZERODDS_BUILD_CPP`, `BUILD_ZERODDS_IDLC` →
  `ZERODDS_BUILD_IDLC`, `BUILD_EXAMPLES` → `ZERODDS_BUILD_EXAMPLES`,
  `BUILD_TESTS` → `ZERODDS_BUILD_TESTS`. `CMAKE_BUILD_TYPE` is no longer
  forced to `Release` unconditionally.

- **BREAKING (Rust API): `CacheChange` carries `inline_qos`, DATA_FRAG has no
  key-hash flag** (`5536e2abcac`). A struct literal of `CacheChange` outside
  this workspace needs the new field (constructor callers are unchanged). The
  non-standard DATA_FRAG `hash_key_flag`, its H-bit constant and its decoder
  argument were removed; a key hash travels as `PID_KEY_HASH` in the inline
  QoS, as for DATA.

- **Same-host traffic between ZeroDDS participants runs over one shared-memory
  channel per participant pair** (`c996fd7a076`, `51274f0cb96`,
  `5ab1427ddd0`), not over a segment per writer/reader pair. The channel
  carries every unicast datagram, user data, SEDP and WLP alike; SPDP stays
  on multicast. `transport.same_host_shm: false` turns it off at run time
  (`f2f9d22cc20`). Its lifetime is the participant's: a lock on each
  segment and doorbell marks it in use, orphans are swept at start, on Linux,
  macOS and Windows (`8a7cde41e7d`, `6b3be68cbcf`, `1b56fde6644`), and a
  mapping keeps no file descriptor (`3e714db8c59`).

- **`zerodds_runtime_destroy` shuts the runtime down** (`7489df77b9e`). It
  stops the threads, announces the participant's departure and closes its
  channels and sockets; freeing the handle alone left the participant on the
  wire until the process ended. `DcpsRuntime::shutdown` wakes its receive
  threads and closes the user transport (`4b96317ab0a`, `f04d083ed2f`); a
  new `Transport::close` (default: nothing) is what a transport ends there.

- **`DataReaderQos` and `DataWriterQos` default `RESOURCE_LIMITS` to
  `LENGTH_UNLIMITED`** (`a2c35049f2e`, `2932c24cd05`), as DDS 1.4 specifies;
  a live writer holds its limits and a write without room waits, then
  returns `TIMEOUT` (`fc3b5ae4ba3`).

- **Without a configured SPDP period a participant beacons at least three
  times a lease** (`f1b40400dfa`): the period is the lesser of 5 s and a
  third of the lease, at least 100 ms.

- **The bridge daemons read their command line through
  `zerodds_config::cli`** (`492a70ad206`). The webdds and websocket daemons
  now say `flag X needs a value` and `bad value for X: V` like the others.

- **BREAKING: the DDS-RPC Basic mapping writes classic CDR samples, announces
  the interface's topic types and places the `Call` union after the header**
  (DDS-RPC 1.0 §7.5.1.1, §7.5.1.1.6/7; `c5285c57fad`, `e305669e2c4`,
  `80df19b642f`, `ec3707125a9`, `3a6ff8b0a50`). `Requester` and `Replier`
  announced `zerodds::RawBytes` as the type name, wrote XCDR2, put the
  payload unaligned behind the header and answered an error with a reply
  that had no `data`. They now announce `${interfaceName}_Request` and
  `_Reply`, write XCDR1 (§7.5.1.1 targets DDS 1.3), start the `Call` union
  on a 4-octet boundary after the `RequestHeader` and let an error reply
  carry the `default` case of the `Return` union
  (`wire_codec::ERROR_REPLY_DATA`). The endpoints offer the XCDR1 data
  representation. A peer that expects the earlier samples neither matches
  on the type name nor decodes the sample. The Enhanced mapping stays
  XCDR2. The bridge requester of `zerodds-webdds-bridge` announces the same
  type names and speaks classic CDR.

  **Migration:** `RpcEndpointOptions::wire_encoding` with
  `WireEncoding::Xcdr2` (Rust), `wire_encoding()` on the params (C++),
  `WireEncoding.XCDR2` (Java) select the earlier samples. Tests:
  `the_call_union_starts_on_a_4_octet_boundary`,
  `the_error_reply_data_is_the_default_case_of_the_union`,
  `the_topics_are_announced_with_the_interface_type_names`,
  `the_two_headers_take_the_same_octets_in_both_columns`.

- **BREAKING: a `Replier` answers requests for its own instance only, a
  `Requester` drops replies to other clients, and the request identity is
  the identity of the sample** (DDS-RPC 1.0 §7.2.1, §7.6.2, §7.8.1,
  §7.8.2, §7.11.1.4.3; `4432cc165c6`, `35aa702e490`, `58634aa2702`,
  `2314c1c5fb5`, `2fe2b7cdea1`). A named replier answered requests that
  carried no instance name; it ignores them now, and a request whose
  `instanceName` differs from the replier's is ignored in Rust, C++ and
  Java, also when one of the names is empty. A requester on a shared reply
  topic received the replies of other clients; the C++ and Java
  requesters drop a reply whose related request id carries another
  request-writer GUID. `Requester` writes the RTPS GUID of its request
  writer and the sequence number the writer is about to assign into the
  `RequestHeader` (offline it keeps a synthesized GUID). The C++ and Java
  bindings correlate over the explicit `RequestHeader::requestId`, so a
  binding whose reader reports no sample identity correlates.
  `SampleIdentity::RELATED_SAMPLE_IDENTITY_INVALID` is
  `{ GUID_UNKNOWN, SEQUENCENUMBER_UNKNOWN }`, sequence number
  `{ high = -1, low = 0 }`, not 0.

  **Migration:** give the requester the instance name of the replier it
  talks to (`Requester::with_instance`); code that compares a sequence
  number against 0 as "no identity" compares against
  `SampleIdentity::UNKNOWN`. Tests:
  `a_named_replier_does_not_answer_a_request_without_an_instance`,
  `tick_filters_requests_for_other_instance_name`.

- **BREAKING: operation opcodes are the HASH of the operation name, an
  attribute implies two operations, and the dispatcher's error codes follow
  §7.5.2** (DDS-RPC 1.0 §7.5.1.1.3, §7.5.1.1.6 rule 5a, §7.5.2, §7.7.1.1;
  `c4a68d07800`, `259cf675efd`, `b527a9f5802`, `2fe2b7cdea1`,
  `b7f5da73ca2`). `function_call` numbered the operations sequentially; the
  opcode is now the HASH that the case labels of the `Call` and `Return`
  unions carry. `ServiceDef::all_methods` implies
  `get_attribute_<name>` and `set_attribute_<name>` for an interface
  attribute, so the unions include them, and a declared operation of that
  name is refused. A generated Rust dispatcher answers an unknown operation
  with `REMOTE_EX_UNSUPPORTED` and an undeclared exception with
  `REMOTE_EX_UNKNOWN_EXCEPTION`; the replier answers a failed reply encoding
  with `REMOTE_EX_UNKNOWN_EXCEPTION`, not `REMOTE_EX_OUT_OF_RESOURCES`.
  `@DDSService(name = "…")` supplies the service segment of the topic name
  (`<interface>_<name>_Request`) in `JavaServiceDef` and no longer replaces
  the interface name. A caller that built case labels or topic names
  itself recomputes them.

- **`evolution_rules::is_compatible` follows DDS-RPC 1.0 §7.7** (`17fe2de3f32`).
  The Basic mapping treats added, removed and reordered operations as
  compatible and a signature change as breaking. The Enhanced mapping treats
  added, removed and reordered parameters and return values as breaking
  (default `@final`) and allows type changes; `AddRemoveException` is a new
  entry. Tests: `basic_mapping_change_signature_is_breaking`,
  `basic_mapping_add_remove_operation_is_compatible`.

- **BREAKING (`zerodds-rpc`): the default QoS keeps all samples, and
  `instance_name` is limited to 255 octets** (DDS-RPC 1.0 §7.10.2,
  §7.5.1.1.1; `ff6353d0ef2`, `e5f4e895ad1`).
  `RpcQos::default_basic` and `default_enhanced` used KEEP_LAST with depth
  10 and 64; they use KEEP_ALL, and `DEFAULT_HISTORY_QOS` replaces
  `DEFAULT_BASIC_HISTORY_DEPTH` and `DEFAULT_ENHANCED_HISTORY_DEPTH`.
  `RequestHeader::to_cdr_le`, `to_cdr_be` and
  `wire_codec::encode_request_frame` return `RpcResult` and refuse an
  `instance_name` longer than `string<255>`; `MAX_STRING_LEN` is 255.

- **BREAKING (C++ and Java runtimes of DDS-RPC): the classes take the form
  the specification gives them** (DDS-RPC 1.0 §7.11.1.3–.5, §7.11.2.2;
  `2ea2abb7d84`, `208e7e0b25c`, `771f1284680`). C++: `RequesterParams`,
  `ReplierParams`, `ServiceParams` and `ClientParams` carry participant,
  publisher, subscriber, service name, instance name, topic name suffixes,
  interface name, binding style and QoS; the service name names the topic
  (§7.4.1) and `instance_name()` the instance, where the service name had
  been used as the instance name. `Requester`, `Replier`, `ServiceEndpoint`,
  `ClientEndpoint`, `ServiceProxy` and `Server` are reference types, endpoints
  are created through a `Publisher` and `Subscriber`, `send_request` takes a
  `WriteSampleRef`, and a local failure is a `LocalError` (Figure 7.6), not
  `RemoteOutOfResourcesError`. `DomainParticipant::create_writer` and
  `create_reader` take the DDS type name beside the topic name, so a
  request topic is announced as `${interfaceName}_Request`. Java:
  `Requester`, `Replier`, `RequesterParams`, `ReplierParams` and `Server`
  are interfaces with `…Impl` classes behind them, `RPCRuntime.over`
  returns one runtime per participant and is the factory for params and
  server, `Sample.Iterator` is a `ListIterator`, and the `Support` topic
  constants of generated code are module-qualified
  (`robot::Calculator_Request`).

  **Migration:** construct through `RPCRuntime.over(participant)` in Java
  and through `ServiceParams`/`ClientParams` in C++. Tests:
  `the_cpp_rpc_runtime_and_the_generated_header_run_together`,
  `the_java_rpc_runtime_and_the_generated_services_run_together`.

- **`zerodds_writer_set_qos` and `zerodds_reader_set_qos` refuse a change of
  HISTORY, RESOURCE_LIMITS and the `max_blocking_time` of RELIABILITY**
  (DDS 1.4 §2.2.3; `19814c9f2e9`). They returned success for these; the
  result is `ImmutablePolicy`, as for RELIABILITY and DURABILITY. Of the
  policies the DDS-RPC QoS carries (§7.10.2), only DEADLINE and LIFESPAN
  (writers) are changeable. **Migration (C):** set these policies at the
  creation. Test: `the_mutable_policies_of_a_live_endpoint_change_and_the_immutable_ones_are_refused`.

- **`RpcQos::from_xml_profile` and `from_qos_url` follow `base_name` and refuse
  what `RpcQos` cannot carry** (DDS-XML 1.0 §7.3.2.4.2, §7.3.2.4.3;
  `9b74a81b0a9`). The named profile was read without its base chain, so
  the policies of a base profile were missing. The chain is resolved through
  `zerodds_xml::resolve_profile`. A policy beyond reliability, durability,
  history, resource limits, lifespan and deadline, the container of an entity
  the RPC layer does not configure, `lifespan` in a reader container, and a
  policy that the writer and reader containers set differently are
  `RpcError::UnmodelledQos`. A circular `base_name` chain and a base defined
  after its derived profile are `QosProfileInvalid { path, reason }`; only
  an unknown library or profile is `QosProfileNotFound`. A profile with
  several `<datawriter_qos>` or `<datareader_qos>` containers uses the one
  without a topic filter, else the first (`zerodds_xml::default_qos`).
  **Migration:** `XmlProfileOptions::ignore_unmodelled`
  (`from_xml_profile_with`, `from_qos_url_with`, `from_service_profiles_with`)
  reads what it can. Tests: `from_xml_profile_follows_base_name_chain`,
  `from_xml_profile_unresolvable_chain_is_invalid_not_missing`,
  `a_policy_the_endpoints_cannot_carry_is_refused_by_name`.

- **`service_mapping::lower_service` refuses the IDL that the DDS-RPC service
  grammar leaves out** (DDS-RPC 1.0 §7.3.1.1; `1627b386b56`). `import`,
  `typeid`/`typeprefix`, CCM and non-plain interface forms, declarations of
  types, constants and exceptions inside an interface body, `oneway` (and
  `@oneway`) and `context` clauses are `RpcError::OutsideServiceGrammar`,
  naming the first construct found. A constant in an interface body used to
  vanish and a `oneway` operation lost its reply structure and `Return`-union
  case without a message. Tests:
  `a_nested_declaration_is_outside_the_service_grammar`,
  `the_oneway_keyword_is_outside_the_service_grammar`.

- **`zerodds-idlc generate --lang java` writes `ZdServices` to a file of its
  own** (`25aa4020218`). The Java text holds `ZdTypes`, `ZdServices` and
  `ZdCodec` as public top-level classes, and JLS §7.6 allows one per file;
  `ZdServices` stayed in `ZdTypes.java`, which does not compile. The output
  is one file per public class (`generate::java_units`). Test:
  `crates/lang-rules/tests/java_units.rs`.

- **BREAKING (C++ API): the `dds::` value, QoS, status and sample classes are
  the ones DDS-PSM-Cxx 1.0 §7.5 and §7.6 define** (`036e7235a9e`,
  `8e2afae80f4`, `b5df4c0dc8e`, `9feb13103aa`, `cd7f594cdff`, `124b33e4dee`).
  `Duration::from_millis` and `to_millis` are `from_millisecs` and
  `to_millisecs`; `Duration::is_infinite()` is gone (compare with
  `Duration::infinite()`, which is `{0x7fffffff, 0x7fffffff}` as DDS 1.2
  IDL §9.3.2 has it and also accepts the C API's spelling); `Time::sec()`
  is `int64_t`. The policies are in `dds::core::policy` with `safe_enum`
  kinds, the QoS classes are `TEntityQos` containers, and `dds/core/qos.hpp`
  and the `dds::core::*Qos` classes are gone; `DataWriterQos` and
  `DataReaderQos` no longer hold PARTITION, PRESENTATION, GROUP_DATA and
  TOPIC_DATA, which belong to the Publisher, Subscriber and Topic;
  `Reliability::Reliable_` is `Reliability::Reliable`. The status classes
  have constructors and accessors instead of public fields.
  `read(max_samples)` and `take(max_samples)` are gone
  (`select().max_samples(n)`). `dispose_instance(handle)` and
  `unregister_instance(handle)` took a sample and take a handle.
  `ContentFilteredTopic<T>(topic, name, Filter)` replaces the constructor
  with participant, expression and parameter list, and `topic()` replaces
  `related_topic()`. `ReadCondition` and `QueryCondition` are classes, not
  templates; `Query::data_reader()` replaces `native_reader_handle()`.
  `BuiltinTopicKey::value()` is `dds::core::array<uint8_t, 16>`, and
  `ParticipantBuiltinTopicData::key()` replaces `guid()`.

  **Migration:** `from_millisecs`/`to_millisecs`, `select().max_samples(n)`,
  `Reliability::Reliable` and the other replacements named above;
  `crates/cpp/tests/smoke_dds_psm.cpp` shows the forms
  (`test_qos_composition`, `test_samples_api`, `test_entity_operations`,
  `test_default_qos_matches_the_specification`).

- **BREAKING (C++ generated types): `zerodds-idlc generate --lang c++`
  writes the DDS-PSM-Cxx 1.0 §7.4 and §8.1 mapping by default**
  (`13777502ceb`). A structure is a class with accessor functions
  (`t.celsius()` reads, `t.celsius(21.5)` writes) instead of public fields,
  a union has `_d()` and one accessor per case, `@optional` is
  `dds::core::optional`, an array `dds::core::array` and a `map`
  `std::map`. The native types the codec reads and writes stand in the
  namespace `zd_native`.

  **Migration:** accessor calls instead of field access, or
  `--cpp-mapping native` for the structures with public fields of rc.6.

- **BREAKING (C++ API): vendor interfaces left the namespaces that
  DDS-PSM-Cxx 1.0 §2.2 reserves, and entity classes carry no vendor
  members** (`4dbadb87eda`, `34dd648fbb7`, `71ff4943db2`).
  `check_status`, `DataRepresentationKind`, `cdr_lite` and `Fixed` are in
  `zerodds::psm` (`zerodds/psm/check_status.hpp`, `type_support.hpp`,
  `Fixed.hpp`); `dds/core/Fixed.hpp` is gone and `dds::topic::core` no longer
  exists. `DomainParticipantFactory`, the `ignore_*` members,
  `discovered_*_count`, `native_handle`, `participant_handle`, `borrow`, the
  per-entity `*_handle` accessors, `wait_for_matched` and
  `native_cft_handle` are removed from the public classes;
  `StatusCondition` takes an entity instead of `void*`. `zerodds-idlc`
  writes `::zerodds::psm::DataRepresentationKind`, so generated headers are
  regenerated. Every header that names `uint32_t`, `int64_t` and the like
  includes `<stdint.h>`, which declares them globally as §7.4.2 requires.

  **Migration:** vendor access goes through `operator->` or `delegate()`;
  `default_participant_qos` stays on `DomainParticipant`.

- **The `dds::` reference and value types are `T…<DELEGATE>` templates**
  (DDS-PSM-Cxx 1.0 §7.5; `429cdde1978`, `4a96017d92a`). `Publisher`,
  `Subscriber`, `DomainParticipant`, `Topic<T>`, `ContentFilteredTopic<T>`,
  `DataReader<T>`, `GuardCondition`, `StatusCondition` and `WaitSet` derive
  from `TPublisher`, `TSubscriber`, … over ZeroDDS's delegates, and the 22
  policies, the 11 status classes, `InstanceHandle`, `SampleInfo` and
  `DomainId` are `typedef T…<detail::…>`. A `Value<D>` takes its delegate by
  move. Where the specification's text has a template template parameter
  for the delegate, ZeroDDS takes the full delegate type
  (`TSample<T, DELEGATE>`, `DataReaderDelegate<T>`).
  `TWaitSet::dispatch`, `conditions` and `detach_condition` return `bool`.

- **BREAKING (Java PSM): errors are exceptions, not `ReturnCode` values**
  (DDS-Java-PSM 1.0 §7.3.2, Table 7.1; `7f18142b5e1`). The binding had its
  own `org.omg.dds.core.ReturnCode` enum. A `write()` on a closed or not yet
  enabled writer returned a code, and the sample was lost if the caller did
  not check it; `read()`/`take()` on a closed reader returned an empty list
  that could not be told from "no data". `org.omg.dds.core` has the
  abstract `DDSException` and its eight PSM subclasses, with ZeroDDS's
  concrete ones in `org.zerodds.core`. `write`, `dispose`,
  `unregisterInstance`, `assertLiveliness` and `enable` return `void` and
  throw `AlreadyClosedException` or `NotEnabledException`; `read`/`take`
  throw instead of returning an empty list; a null argument is an
  `IllegalArgumentException` (RETCODE_BAD_PARAMETER). The `ReturnCode`
  enum is removed. **Migration:** catch the exceptions where the code
  tested the return value. Tests:
  `writeOnAClosedWriterThrowsAlreadyClosed`,
  `readAndTakeOnAClosedReaderThrowAlreadyClosed`,
  `aNotEnabledWriterRefusesToWrite` (`ExceptionMappingTest`).

- **BREAKING (Java PSM): the participant factory takes a `ServiceEnvironment`,
  and `createParticipant` returns a new participant per call**
  (DDS-Java-PSM 1.0 §7.3.1, §7.4.1, §7.2.3; DDS 1.2 §7.1.2.2.2.1;
  `0840f9ed436`, `5041698cee3`, `9e7c893dd93`, `b6d007ac00a`).
  `DomainParticipantFactory.getInstance()` without an argument is gone;
  `getInstance(null)` throws `NullPointerException`; the factory's
  `deleteParticipant` is gone, `close()` deletes (§7.2.3). The factory had
  cached one participant per domain and returned it again, also after
  close; it creates a fresh one on every call, and a closed participant
  refuses `createPublisher`, `createSubscriber` and `createTopic`.
  **Migration:** set the system property
  `org.omg.dds.serviceClassName` to `org.zerodds.ServiceEnvironmentImpl`
  (`ServiceEnvironment.IMPLEMENTATION_CLASS_NAME_PROPERTY`) and call
  `DomainParticipantFactory.getInstance(ServiceEnvironment.createInstance(classLoader))`.
  Tests: `QuickstartApiTest`, `DomainParticipantFactoryTest`.

- **BREAKING (Java PSM): the types of `org.omg.dds` are the interfaces and
  abstract classes of the specification** (DDS-Java-PSM 1.0 §7.2.1, §7.3–§7.7;
  `5041698cee3`, `d23eff980f3`, `9267b77a38a`, `a91e0f632bb`, `5eb3cb8ca5b`,
  `0840f9ed436`). `Time`, `Duration`, `InstanceHandle`, the 21 QoS policies
  (immutable, with a `with*` DSL), the 13 communication statuses,
  `Entity` (generic and `Closeable`), `DomainEntity`, `StatusCondition`, the
  fourteen `org.omg.dds.core.event` types, `ReadCondition`,
  `QueryCondition`, `TopicDescription`, `Topic`, `Sample`, `Publisher`,
  `Subscriber`, `DataWriter`, `DataReader` and the participant types were
  final classes or enums of ZeroDDS with other names and accessors; the
  implementations are in `org.zerodds` (`DomainParticipantImpl`,
  `PublisherImpl`, `DataWriterImpl`, …). A listener receives one event
  object that names the entity and carries the status, and is told only
  about the statuses of its mask. `Sample` has the bean properties of
  `SampleInfo` (`getData`, `getSampleState`, …) in place of `data()`,
  `sampleState()`. `read`/`take` have the `Sample.Iterator` form and the
  copy-into-a-list form; code that used a `List` return walks an iterator or
  calls the list flavour. `createDataWriter(Topic, TopicTypeSupport,
  DataWriterQos)` moved to `PublisherImpl` and `SubscriberImpl`. The
  operations of DDS 1.2 that the binding lacked are there, among them
  `findTopic`, `lookupTopicDescription`, `closeContainedEntities`,
  `waitForAcknowledgments`, `waitForHistoricalData`, `getKeyValue`,
  `beginCoherentChanges`, `beginAccess` and `ignore*`. **Migration:**
  `event.getSource()` and `event.getStatus()` in listener callbacks; `TimeUnit`
  in place of `toMillis`.

- **BREAKING (Java PSM wire): a narrow `String` is written as UTF-8**
  (DDS-XTypes 1.3 §7.2.2.2.1.2.4, §7.4.1.1.2; `8950dc6c224`). Reflection
  type support and `DynamicDataTypeSupport` truncated each Java character to
  its least-significant byte (DDS-Java-PSM 1.0 §8.5.1, which predates
  XTypes 1.3); `été` was `e9 74 e9` and is `c3 a9 74 c3 a9`, the form of
  `tests/interop-corpus` and of generated code. Up to U+007F the forms are
  equal. IDL `char` is one octet and refuses a character above U+00FF in
  both. **Migration:** `-Dzerodds.java.narrow-string=truncate` selects the
  earlier form.

- **Java PSM entity behaviour follows DDS 1.2: defaults, matching, states
  and closing** (DDS 1.2 §7.1.2.4.2.7, §7.1.2.5.1, §7.1.3; DDS-Java-PSM 1.0
  §7.2.3; `04d956d2b1d`, `4cef53c6cc0`, `096be70d055`, `c1612b69777`,
  `dadfce7b8f6`). The default RELIABILITY of a reader and a topic is
  BEST_EFFORT with HISTORY KEEP_LAST and depth 1 (`DataReaderQosImpl.defaults`,
  `TopicQosImpl.defaults`); a single default of RELIABLE with depth 10 had
  served every entity. The writer default is RELIABLE. A writer and a reader match
  on the type name and the request-offered QoS rules, and an incompatible
  peer is recorded in `OFFERED_INCOMPATIBLE_QOS` and
  `REQUESTED_INCOMPATIBLE_QOS`; the bus had matched on domain and topic
  name only, so a reader decoded the bytes of a foreign type. `sample_state`
  becomes READ with a read, `view_state` is NEW until the first access, and
  the instance state is the current one for all samples of the instance.
  NOT_ALIVE_NO_WRITERS is reported when no writer remains; closing one of
  two writers reported it. `close()` of a participant, publisher and
  subscriber closes the contained entities; their writers had stayed on the
  bus. Tests: `reliabilityAndHistoryQosDefaults` (`CoreTypesTest`),
  `QosCompatibilityTest`, `SampleStateTest`, `CloseCascadeTest`,
  `closingOneWriterDoesNotMarkNoWritersWhileAnotherWrites`.

- **BREAKING (`zerodds-xml`): QoS numbers take the types the OMG's XSD
  gives them, and `-1` is written `LENGTH_UNLIMITED`** (DDS-XML 1.0
  §7.2.2.1; `7fbca3f5f05`). `max_samples`, `max_instances` and
  `max_samples_per_instance` of `resource_limits` and
  `durability_service` are `positiveInteger_UNLIMITED`: 1 or more, or the
  name `LENGTH_UNLIMITED`. The history `depth`, `history_depth`,
  `initial_samples` and `initial_instances` are `xs:positiveInteger`, and the
  `value` of `ownership_strength` and `transport_priority` is
  `xs:nonNegativeInteger`. The reader took any `long`, so `0`, `-1` and a
  negative priority passed; it refuses them with `XmlError::ValueOutOfRange`.
  `parse_positive_long_unlimited`, which had no caller, reads the limits.
  **Migration:** a document that wrote `-1` for a limit writes
  `LENGTH_UNLIMITED`; the value still reads as `-1` in `ResourceLimitsQosPolicy`.
  Tests: `numbers_outside_the_schemas_types_are_refused` (`spec_items`),
  `t11_length_unlimited_in_resource_limits` (`qos_profile`).

- **BREAKING (`zerodds-xml`): a `register_type` refers to a type defined before
  its domain library, and an application's participant inherits only from
  libraries defined before the application** (DDS-XML 1.0 §7.3.4.4.1,
  §7.3.6.4.2; `a83476cb034`, `5e03ee5e374`). `resolve_participant` searched
  every `<types>` block of the document, so a block after the domain library
  satisfied the reference; the registrations of the referenced domain,
  including those inherited through `base_name`, now resolve in the
  `<types>` blocks that precede the library, and a later block gives
  `XmlError::UnresolvedReference`. `resolve_application` did not check
  the position of a base; a base in a participant library after the application is
  `XmlError::InvalidXml`. `DomainLibrary::type_libraries_before` and
  `ApplicationLibrary::participant_libraries_before` record the count of
  preceding blocks (`None` for a library read on its own); both structures
  have a new public field, so a struct literal outside this workspace adds
  `None`. **Migration:** move the `<types>` block, or the participant library,
  in front of the element that uses it. Tests:
  `register_type_refers_to_a_type_defined_before_the_domain`,
  `application_inherits_only_from_libraries_defined_before_it`.

### Added

- **Coherent and ordered access with Connext, HDDS, zzdds and InterCOM.**
  Measured against the published OMG dds-rtps run of 2026-09-30: of 430 red
  cases of ZeroDDS against the vendors, 48 were cases another product passes
  in ZeroDDS's position; 14 of them coherent and ordered cases. Two new axes
  of `zerodds_rtps::dialect::RtpsBehaviour`: `GroupWireForm::CoherentSetsOnly`
  sends a GROUP-scope publisher's sets as under TOPIC, without the group
  information of DDSI-RTPS 2.5 §8.7.5/§8.7.6, the form HDDS and zzdds send
  and Connext 7.7.0, HDDS 1.4.8 and zzdds 0.3.2 read (setting
  `writer.group_info_on_wire`, default `true`); `EndOfSetForm::Gap` gives a
  peer the end of a writer's coherent set as a GAP, which InterCOM 4.3.1
  takes where it requests the DATA of Table 9.22 again and again.
  `DcpsRuntime::set_vendor_rtps_behaviour` sets a behaviour for every
  participant of a vendor the application names, `user_writer_behaviour`
  is the configured base. `shape_main` uses both for these vendors, writes
  a GROUP-scope coherent or ordered publisher's first sample once its
  writers have matched readers (Connext delivers nothing of a writer whose
  samples start past SN 1 for it; CoreDX fails the same cases against
  Connext in the published run) and writes the iterations of one coherent
  set without a pause (a zzdds reader takes a HEARTBEAT inside a set as its
  end). ZeroDDS -> Connext `Test_CoherentSets_8`, `_12`, `_18`, `_21`,
  `Test_OrderedAccess_8`, -> HDDS and -> zzdds `Test_CoherentSets_12`/`_21`,
  -> InterCOM `Test_CoherentSets_10`/`_19` green on the Linux test host.

- **XRCE: a `<types>` CREATE may reference types the agent already knows**
  (DDS-XRCE 1.0 §7.7.3.3.2): the types of the client, of `type_files:` and
  of the profile files, through the new
  `zerodds_xml::bridge_library_complete_with_known`; a name that is in
  neither is `STATUS_ERR_UNKNOWN_REFERENCE`. Tests:
  `a_types_string_may_reference_types_the_agent_knows`,
  `a_types_string_references_a_type_created_before`.

- **Deletion and lookup on DomainParticipant, Publisher and
  Subscriber** (DDS 1.4 §2.2.2.2.1.2, .4, §2.2.2.4.1.6, .7, .14,
  §2.2.2.5.2.6, .7, .10, .11, .14): `delete_publisher`,
  `delete_subscriber`, `delete_datawriter`, `lookup_datawriter`,
  `delete_datareader`, `lookup_datareader`, `get_datareaders` (returning
  `AnyDataReader`, a reader of any type with `typed::<T>()`),
  `notify_datareaders`, and `delete_contained_entities` on Publisher and
  Subscriber, with the specification's PRECONDITION_NOT_MET cases (an
  endpoint of another factory; a publisher with writers; a subscriber
  with readers; a reader with ReadConditions or QueryConditions).
  `get_datareaders` under PRESENTATION GROUP scope needs `begin_access`
  and, with `ordered_access`, lists a reader once per sample in source
  timestamp order. Tests: `crates/dcps/tests/entity_deletion.rs`, 18.

- **A DataReader creates and deletes its ReadConditions and
  QueryConditions** (DDS 1.4 §2.2.2.5.3): `create_readcondition`,
  `create_querycondition`, `delete_readcondition` and
  `delete_contained_entities`. A condition's trigger reads the reader's
  cache as `read_w_condition` does, the QueryCondition's with its
  expression and parameters; a sample received and not yet in the cache
  counts as NOT_READ with any view and instance state. A deleted
  condition does not trigger. The Rust API had only `ReadCondition::new`
  with a closure the caller had to write. Tests:
  `crates/dcps/tests/reader_conditions.rs`.

- **A RELIABLE writer holds what its reliable readers have not
  acknowledged to a window, 512 KiB by default**
  (`writer.unacked_window_bytes`, `ZERODDS_WRITER_UNACKED_WINDOW`, `0`
  for none). A write past it waits for acknowledgements up to RELIABILITY
  `max_blocking_time`, then returns `TIMEOUT`; half a window asks for a
  HEARTBEAT at once. Cyclone DDS holds its writers to 500 kB. Measured on
  the Linux test host with `zerodds_perf`, RELIABLE, 1 kB, KEEP_ALL, in
  turn with Cyclone's `ddsperf`: 496–542 kS/s with no datagram dropped by
  the kernel and every sample received, `ddsperf` 543–547; without the
  window the same writer reached 480–520 kS/s for the first seconds, then
  the reader's kernel dropped 46 551–61 638 datagrams per cell. With it, a
  RELIABLE write arms its next HEARTBEAT instead of waking the scheduler
  for a pass (the tick thread took 3.0 µs of CPU per sample), the history
  gives up acknowledged changes from the front instead of `split_off`, and
  a reader with a room takes a run of DATA under one budget.

- **A BEST_EFFORT reader takes 93–100 % of a 1 kB stream of 800 000
  samples a second instead of 45–47 %; a RELIABLE stream runs at 271–295
  kS/s instead of 51–149.** Measured on the Linux test host with
  `zerodds_perf`, 1 kB, KEEP_ALL on both ends, in turn with Cyclone's
  `ddsperf`: the BEST_EFFORT reader received 45–47 % of the samples
  written before; after, 99.95–100.00 % in four cells written at 781–803
  kS/s, and 93.07–99.96 % (median 99.7 %) in twelve cells written at
  767–851 kS/s, the low ones where other guests loaded the host;
  `ddsperf` received 97.0–99.8 % in its series, at 839–898 kS/s. RELIABLE, in turn with the build before: 51–149 kS/s
  before with 6 400–28 507 datagrams dropped by the kernel for a full
  receive buffer (`RcvbufErrors`) per cell, 271–295 kS/s after with none
  and every written sample received; before, one cell ended with 90 330
  written samples not yet received. `ddsperf` 573–613. One cell of an
  earlier series still dropped 11 558 datagrams and fell to 114 kS/s;
  the recovery is under Fixed ("A RELIABLE stream recovers from lost
  datagrams at round-trip speed"), the window above keeps the drops off.
  CPU per sample on the subscriber after: taking thread 0.87–0.95 µs,
  receive thread 1.10–1.24 µs. On the receive thread:
  - a DATA's serializedPayload is a range of the received datagram, not a
    copy of its own (`zerodds_rtps::payload::Payload`);
  - the DATA of one writer to one reader in a datagram go to the reader
    under one lock and in one release. The INFO_TS a writer puts in front
    of each DATA ended the run, so every run was one DATA long;
  - a BEST_EFFORT sample behind a gap is handed out at once, not through
    the receive cache;
  - the writer proxy keeps the run of known sequence numbers as one number,
    not one set entry per sequence number;
  - the lists a datagram is decoded and handed over in, and the
    `recvmmsg` header arrays and buffers, stay with the receiving thread;
  - WaitSets are told once per run and only while one waits; the async
    waker is woken once per run.

  On the thread that takes:
  - the reader cache is a `VecDeque`. Evicting the oldest of 100 000
    samples moved the other 99 999, and a KEEP_ALL BEST_EFFORT reader spent
    55 s in one `take`;
  - `take` hands out the `T` admission decoded, where it decoded the bytes
    again;
  - admission notes a sample's instance under one lock of the instance
    tracker instead of seven, and a `take` notes a run of one instance's
    samples once; the reader QoS is read once per `take` instead of twice
    per sample; SAMPLE_LOST is counted once per `take` instead of once per
    evicted sample, each time under the reader's runtime slot, which the
    receive thread holds while it hands the reader samples; and a `take`
    asks for departed writers without that lock;
  - a keyless reader counts its cache only once the cache is as long as
    a limit, where it counted it for every sample.

- **The receive path decodes a user datagram once and hands an in-order
  sample on without the receive cache.** Every user datagram was decoded
  in full for the TypeLookup service before the reader decoded it again;
  a walk over the submessage headers now answers that. A sample that is
  next in order, with nothing waiting before it, no longer goes into the
  reader's receive cache and straight out again (a cache node, a copy of
  the inline QoS, a removal). A receive batch wakes the scheduler only
  when it held a HEARTBEAT, GAP, DATA_FRAG, ACKNACK or NACK_FRAG or reached
  a reader watching a DEADLINE; the reader-progress signal and the
  listener dispatcher are woken only while a thread waits on them —
  `Condvar::notify_*` is a futex call either way. A reader reads an
  instance's state without copying it and builds a key holder only for a
  new instance. Measured on the Linux test host with `zerodds_perf`
  BEST_EFFORT, 1 kB, KEEP_ALL on both ends: the reader received 31–33 % of
  the samples written before (3.98–4.15 million of 12.6–12.9 million) and
  45–47 % after (5.43–5.61 million of 11.8–12.4 million); the kernel
  dropped 686 458 datagrams for a full receive buffer (`RcvbufErrors`)
  in one cell before and 479 916–519 794 after; the receive thread made
  about 14 allocations per received sample before and 5.5 after.

- **A BEST_EFFORT write makes five heap allocations instead of 34.**
  Measured with an allocation counter on the Linux test host,
  `zerodds_perf` BEST_EFFORT, 1 kB, keyed: 27.3 `malloc` and 6.3 `realloc`
  per sample before, 5.1 and 0 after; Cyclone's `ddsperf` makes one of
  each. Written: 444–453 kS/s before, 800–885 after, `ddsperf` 867–949 in
  the same cells; publisher CPU per sample 2.66 µs before, 1.36 after,
  `ddsperf` 1.20. What went:
  - the two metric series of a write (`dds_dcps_samples_written_total`,
    `dds_dcps_sample_size_bytes`) are looked up once per writer, not per
    write with fresh label strings under the registry's lock — nine
    allocations; a reader holds its `dds_dcps_samples_read_total` the
    same way;
  - an encoder writes into the buffer it is handed
    (`zerodds_wire::Writer::appending`), in `ShapeType` and in the
    `DdsType` bridge `zerodds-build` generates, instead of growing a
    writer of its own and copying it over;
  - the key holder is written into a buffer the writer keeps, and the
    instance tracker gets a copy only for a new instance;
  - the RTPS writer's payload pool is refilled from what a VOLATILE writer
    removes right after the write, its target list per reader is built
    when the reader is added, a keyed DATA goes into the message as a
    stack prefix with `[PID_KEY_HASH, PID_SENTINEL]` and the payload
    (the octets `DataSubmessage::write_body` writes, pinned by
    `the_compact_data_prefix_writes_the_octets_write_body_writes`), INFO_TS
    is written without a heap buffer, and a message reserves at most an
    Ethernet datagram — with readers on the same host it reserved the
    64 000-octet loopback MTU per sample;
  - a batch keeps its buffer for the next one instead of releasing it on
    the scheduler thread, which took glibc's lock on the writing thread's
    arena, shares its target list with the datagram, and is no longer
    copied once per target when there is no security gate.

- **The language runtimes' self-tests run in CI.** `tools/runtime-selftests.sh`
  runs each runtime's own test against recorded octets and reports a
  missing toolchain as a failure. The new `runtime-selftests` job runs
  thirteen in every full pipeline (C, C++, Python, Go, Node, TypeScript, Ada,
  Ada 83, D, Elixir, Lua, OCaml, Java), and the weekly `endpoints-<lang>`
  jobs run the other seven with the toolchain each installs (C#, F#, Kotlin,
  Swift, Zig, Julia, Nim). None ran anywhere before; three were broken
  (Ada, Ada 83, F#) and were fixed when the script first ran them.

- **A BEST_EFFORT writer no longer wakes the scheduler per write or per
  batch.** Every write raised a scheduler pass for the HEARTBEAT a reliable
  writer owes; a BEST_EFFORT writer owes none, and the pass locked every
  writer slot against the next write. Now only a write that moved a
  HEARTBEAT, DEADLINE, LIFESPAN or manual-LIVELINESS window wakes it. Under
  LATENCY_BUDGET every opened batch also armed a scheduler deadline of its
  own, one futex wake per thirteen samples; one armed deadline now covers
  every batch due no earlier, and the tick re-arms for the earliest batch
  still pending. The encode buffer starts at the last body's length, and a
  write reads the clock for the instance tracker only when it registers.
  Measured on the Linux test host, `zerodds_perf` BEST_EFFORT 1 kB: 292–319
  kS/s before, 444–453 after; futex calls per sample 0.20 before, 0.044
  after; Cyclone's `ddsperf` in the same cell 846–895 kS/s.

- **A keyed writer no longer hashes the same key on every write.** Since
  `2e9ce2e2f9a` every write of a keyed sample computed its KeyHash, which
  for a key holder that can exceed sixteen octets is an MD5 over it (XTypes
  1.3 §7.6.8). `zerodds_perf` carries its 1 kB payload in `ShapeType`'s
  `@key` string, and its write rate fell from 396.8 to 173.9 kS/s at that
  commit (bisected on the Linux test host). Writer and reader now keep
  their last key holder and its hash and reuse the hash for the same
  octets; `zerodds_wire::md5` pads on the stack instead of copying its
  input into a `Vec`; the holder is moved instead of copied twice, and the
  instance tracker answers under one lock. Measured: 281–320 kS/s written,
  against 151–156 before; Cyclone's `ddsperf` in the same cell 876–932.
  `zerodds_perf` now carries its size in OMG `shape.idl`'s
  `additional_payload_size` sequence with `BLUE` as the key, as `ddsperf`
  keeps its baggage out of its key: 292–319 kS/s written, `ddsperf`
  858–915.

- **The wire capture covers OpenSplice and HDDS.** `tools/wire-capture`
  runs a real DataWriter for both: OpenSplice 6.9 through SACPP over DDSI2
  on the loopback (`opensplice-wire-template.cpp`), HDDS through the
  published runtime 1.1.2 with hddsgen 1.2.0's type, forwarded by an
  adapter that writes no octet of its own (`hdds-wire-template.rs`).
  `wire-goldens.tsv` holds 1605 rows for eleven implementations, 901 of
  them goldens. OpenSplice: 28 goldens, 69 refusals (the in-memory
  probe's), 10 rows whose alignment fill differs from sample to sample.
  HDDS: 51 captured samples, every one XCDR2 octets under encapsulation
  `0x0001` (PLAIN_CDR_LE), because hddsgen marks every struct `IS_FINAL`
  and the runtime takes `0x0001` for `@final`; 55 refusals where the
  generated code does not compile, as in memory. `extract-rtps.py` now
  attributes a sample to its writer by the whole GUID and not the entity
  id: OpenSplice's subscriber publishes its built-in `DCPSParticipant`
  through a writer whose entity id is `00000202`, the same id as the
  publisher's writer of a keyed user topic, and two cases were recorded as
  "SEDP never named the topic". `recheck.sh` gives the same 839 rows for
  the nine earlier implementations before and after. `--all` counts the
  domain offsets of the RTPS implementations and the three ORBs apart: the
  eleventh at a common count would start at domain 220, and HDDS refuses a
  domain id above 232. 27 earlier rows that stood `wire-only` now compare
  against the in-memory rows recorded since (26 `same`, TAO
  `char-and-wchar` `differs`).

- **The OpenDDS column has `fixed` cells.** `wireprobe-opendds` and
  `wireprobe-batch` build a case whose IDL names `fixed<` through OpenDDS's
  FACE mapping (`opendds_idl -Lface`, linked with `libOpenDDS_FACE`), the
  one mapping OpenDDS 3.34 carries `fixed` in; the classic mapping dropped
  the member (`tao_idl -Sa -St`: "fixed data types are not supported", exit
  0), and every `fixed` case stood as refused for a missing member. Of the
  ten `fixed` cases, `fixed-basic` and `fixed-scales` with their
  unannotated twins write the declared `(P + 2) / 2` octets in both
  encoding versions and both byte orders (16 rows, OpenDDS 1148 → 1164 rows
  with octets) and read omniORB's octets back; `Dialect::OpenDds`
  reproduces and reads back all 1164. `fixed-scale-equals-digits` and its
  twin are refused with `CORBA::DATA_CONVERSION` from `Fixed_T<P,P>`'s
  default constructor; `sequence-fixed` and `array-1d-fixed` with their
  twins are refused with the compiler's errors on the C++ the FACE
  generator writes for an anonymous sequence or array of a `fixed` typedef.
  The `-Wreturn-type` warning that follows those errors is classified as a
  cascade in `WARNINGS.md`.

- **`FixedDigitCount::ValueStringDigits`, the digit count ACE's own CDR
  operator writes a `fixed<P,S>` with** (`--fixed-digits
  value-string-digits`). ACE/TAO 2.5.24's `ACE_CDR::Fixed::from_string`
  counts the digits of the string it was parsed from and adds one leading
  zero to an even count: `0.12345` under `fixed<5,5>` is `0012345c`, `0.00`
  under `fixed<5,2>` is `000c` (`unspecified_constructs.md` §3, seven
  measured rows, each a decimal with `S` fraction digits). The count here is
  the integer digits as written plus `S`: the fraction as written would have
  cut `7` under `fixed<5,2>` to its last nibble, `0c`. The Rust writer and
  all 20 language runtimes write and read it, with a test against those
  rows in each runtime that has self-tests; a count past 31 digits is
  refused rather than wrapped in the `u8` it was counted into. No dialect
  selects it: OpenDDS 3.34, which an earlier note took for ACE's writer,
  writes `OpenDDS::FaceTypes::Fixed_T<P,S>`, the declared count — measured
  through its FACE mapping, `0.00` under `fixed<5,2>` as `00000c` and `7` as
  `00700c` — so `Dialect::OpenDds` keeps `Declared`. In the same run
  `Fixed_T<5,5>` threw `CORBA::DATA_CONVERSION` from its default
  constructor, and an anonymous `fixed` member came out as an undefined
  `Sample::fixed` type, under both the FACE and the C++11 mapping.

- **`Dialect::Hdds`, opt-in (`--dialect hdds`, `wire.codec_dialect: hdds`).**
  HDDS's generated serialiser (hddsgen 1.2.0) departs from XTypes 1.3 in
  four ways, each pinned by a recorded row: collection elements packed
  without alignment (`array-1d-struct-final`: `01000000 0200 03000000 …`),
  every aggregate framed as `@final` with no DHEADER and no EMHEADER, a
  four-octet length in front of each element of a sequence of a
  non-`@final` type, and a `wstring` in UTF-8. Four axes carry them
  (`ElementPadding`, `AggregateDelimiter`, `SequenceElementDelimiter`,
  `WstringCodeUnits`), each with the specification as its default; the
  dialect reproduces all 326 recorded HDDS rows in both directions
  (`dialect_completeness.rs`, `dialect_read_completeness.rs`), where the
  best assignment of the existing axes reached 123 of 163 constructs under
  XCDR1 and 115 under XCDR2. The planner and `zerodds-wire-codec` carry the
  axes, and so do the generated Rust codec and all twenty generated codecs
  (Ada, Ada 83, C, C++, C#, D, Elixir, F#, Go, Java, Julia, Kotlin, Lua,
  Nim, Node, OCaml, Python, Swift, TypeScript, Zig): `zerodds-idlc generate
  --dialect hdds` writes them. The generated Rust writes and reads the
  octets HDDS wrote for five corpus cases under PLAIN_CDR and PLAIN_CDR2
  (`tools/idlc/tests/dialect_byte_equality.rs`), and each of the twenty
  runtimes tests each axis against a recorded HDDS file; all twenty
  self-tests passed on the Linux test host. A caller that names no target
  still gets the refusal naming the three axes
  (`lang-rules/tests/hdds_axes_are_refused.rs`).

- **`rmw_zerodds_cpp` answers Rolling's `rmw_get_servers_info_by_service`
  and `rmw_get_clients_info_by_service`** (`ros2 service info --verbose`).
  A server is a reader on the request topic and a writer on the reply
  topic, a client the other way round; each entry pairs a reader with the
  writer of the same node and service type, reader first, as
  `rmw_dds_common`'s `GraphCache` does for `rmw_fastrtps` and
  `rmw_cyclonedds`. `no_mangle` is refused, as there. On Rolling the
  package loaded with `failed to resolve symbol` for both. A topic's
  endpoint list that could not store a row returned the others with
  `RMW_RET_OK`; it now fails with `RMW_RET_BAD_ALLOC`. The four
  `ros-rmw-deb` jobs (Humble, Jazzy, Kilted, Rolling) build the package,
  load it and exchange 20 of 20 messages through it in pipeline 7166; in
  the nightly before, Kilted and Rolling did not build.

- **`rmw_zerodds_cpp` announces type hashes (REP 2011) and reads the
  peers'.** Every publisher, subscription, client and service puts
  `typehash=RIHS01_<hex>;` into the USER_DATA of its DDS endpoints, from the
  type support's `get_type_hash_func`, as `rmw_dds_common` does for
  `rmw_fastrtps` and `rmw_cyclonedds`; a client's request writer carries it
  beside `clientid=`. The topic and service endpoint info parses it back, so
  `ros2 topic info -v` and `ros2 service info -v` no longer show
  `Type hash: INVALID`. Underneath, `zerodds_writer_create_qos_user_data` and
  `zerodds_reader_create_qos_user_data` in the C API set an endpoint's
  USER_DATA, and `ZeroDdsEndpointInfo` returns the one a peer announced. Humble
  has no type hashes and builds without them. The `ros-rmw-deb` job checks it
  with `packaging/linux/deb/rmw-type-hash-check.py` against the package
  itself and both ways against `rmw_fastrtps_cpp` (Jazzy, Kilted, Rolling),
  and Rolling's service endpoint info with `rmw-service-info-check.py`; both
  passed on the CI runner in the distro images.

- **`wireprobe-cpp-matrix`, the C++ column over the whole matrix.** One
  binary per case, plan and encoding answers both byte orders and all
  four emit forms. Against the Rust column in
  `crates/wire-probe/tests/reference_columns.rs`: 176,400 slots, 0
  disagreements — 101,000 with the same octets, 49,900 read back as the
  sample, 60 read back as the Rust column read them, 25,440 refused where
  the Rust column refused (Linux test host, `eba97aa9ea2`).
- **`wireprobe-java-matrix`, the Java column over the whole matrix.**
  176,400 slots, 0 disagreements against the Rust column: 100,920 with
  the same octets, 49,860 read back as the sample, 60 read back as the
  Rust column read them, 25,140 refused where the Rust column refused,
  and 420 refused in `resolve-module-named-as-type`, which has no Java
  form (JLS 8.1: a nested class may not share the simple name of an
  enclosing one) and is named as such in the test (Linux test host,
  `c08eeb989a6`). On the way the generated PL_CDR1 per-sample `Pid`
  handle became a declaration, since `runtime/java`'s constructor is
  package-private and javac refused 21 cells under `rti`.

- **An RTI reader keeps one instance for a key that RTI and ZeroDDS both
  write under XCDR1.** `tools/keyhash-probe/rti-live/run.sh`: the key
  holder `@key short a; @key long long b;` puts `b` at 4 under PLAIN_CDR2,
  which XTypes 1.3 §7.6.8 prescribes for the KeyHash, and at 8 under
  XCDR1. RTI Connext 7.7.0's reader put 20 samples of its own writer and
  15 of ZeroDDS's in one instance, whose handle is the PLAIN_CDR2 KeyHash
  `00010000 00000000 00000002 00000000`; ZeroDDS's reader put 16 and 18 in
  one instance.
- **Foreign TypeObjects read against the foreign data.**
  `tools/foreign-typeobject-probe` takes the `TypeInformation` a writer
  announced, fetches the complete `TypeObject` over TypeLookup (`getTypes`,
  `getTypeDependencies`), builds a `DynamicType` from it and decodes the
  writer's samples. Over the 102 publisher variants of the OMG `dds-xtypes`
  v2 suite, XCDR2: CoreDX 100, Cyclone 100, RTI 93, OpenDDS 91 of 100 equal
  to the suite's data; every remaining variant is missing type information,
  a PL_CDR1 framing, OpenDDS's `long double`, or a sample that did not
  arrive — none a type object that described its data otherwise. Fast DDS
  3.6.1, which the suite has no `test_main` for, publishes through
  `fastdds_pub.cpp` with its own IDL parser and JSON reader: 57 of 57
  published variants equal under XCDR2 and under XCDR1; its parser or
  reader refused the other 43 before anything reached the wire.
- **`pkcs11:` URIs for the DDS-Security identity** (DDS-Security 1.2
  §9.3.1 Table 44, RFC 7512), behind the `pkcs11` feature of
  `zerodds-security-runtime`. The CA and identity certificates are read
  from the token, and the private key stays on it: the handshake signs
  through `IdentitySigner`, which for a token key calls `C_Sign`.
  Measured against SoftHSM 2.6.1: the token key signs a handshake-shaped
  message that the certificate read from the token verifies
  (`tools/pkcs11/softhsm-fixture.sh`, `tests/pkcs11_softhsm.rs`). A full
  two-participant handshake with a token identity has not been run.
- **Coverage page `idl-4.2` is back**, 189 items, each `done` quoting the
  assertion that holds it; every published coverage page has its source
  again.
- **The website's generated matrix covers the 420-case corpus**, 419
  rendered: `resolve-module-named-as-type` is refused by the front end
  (OMG IDL 4.2 §7.5.2). It listed 376.
- **Coverage pages `dds-java-psm-1.0` and `idl4-csharp-1.0` are back**,
  each `done` item quoting the assertion that holds it.
- **Every tracked `Cargo.toml` is named by the workspace's `members` or
  `exclude`;** `ci/test_dependency_graph.py` holds the list complete.
- **The full three-column tier reads its matrix back** and fails on a
  recorded disagreement, and `interop-results` reads the warning tags of
  rustc (`#[warn(...)]`) and the C# compiler (`CSnnnn`).
- **`ros-<distro>-rmw-zerodds-cpp`, the RMW implementation as a Debian
  package** (`zerodds-ros2-bridge-1.0` §11: "Pro ROS-Distro ein .deb-Package:
  ros-humble-rmw-zerodds-cpp, ros-iron-rmw-zerodds-cpp, etc.").
  `packaging/linux/deb/build-rmw-deb.sh --distro <humble|iron|jazzy|rolling>`
  builds it from the ament project `crates/rmw-zerodds-shim/ament` and
  installs it into the distro's own ROS prefix `/opt/ros/<distro>`:
  `librmw_zerodds_cpp.so`, the two libraries it resolves through
  `RPATH=$ORIGIN`, the REP-2007/REP-2008 headers, the ament index entries
  and `package.xml` — the layout the RMW packages on packages.ros.org have.
  The file list comes from `cmake --install` alone, no second hand-kept copy
  list next to it, and a staging tree without the library or an index entry
  is an error rather than a `.deb` that installs an RMW rcl cannot load.
  There are no maintainer scripts: a library, not a daemon.
  `ci/full.yml::ros-rmw-deb` (weekly) builds the package inside
  `ros:jazzy-ros-base-noble`, checks the .deb's own file list for the library
  and the resource index entry and installs it back into the ROS prefix;
  `packaging/docker/ros2-shim/Dockerfile` dpkg-installs that package instead
  of keeping a second copy of the install tree under `/opt/zerodds`.
  Checked without ROS 2 by
  `crates/rmw-zerodds-shim/tests/ros2_deb_package.rs`; building the package
  and the image needs the Linux host and is not measured here.
- **`DomainParticipant::delete_topic`** (DDS 1.4 §2.2.2.2.1.6): refused
  with `PreconditionNotMet` while a DataWriter, DataReader,
  ContentFilteredTopic or MultiTopic uses the topic or when another
  participant owns it; a deleted topic is disposed on the SEDP topics
  writer. `zerodds_dp_delete_topic` applies the same precondition.
- **The DDS:Logging:DDS_LogTopic builtin logging plugin** (DDS-Security 1.2
  §10.6): security log events are published on `DDS:Security:LogTopicV2`
  (`BuiltinLoggingTypeV2`), optionally also on the legacy
  `DDS:Security:LogTopic`; the permissions plugin applies the clause's
  topic rule to them.
- **The C++ PSM's `get_discovered_topics` and
  `get_discovered_topic_data`.**

- **`DomainParticipant::delete_topic`** (DDS 1.4 §2.2.2.2.1.6). The Rust API
  had no way to delete one topic: a topic was disposed only by
  `delete_contained_entities`. The new operation refuses with
  `PreconditionNotMet` while a DataWriter, DataReader, ContentFilteredTopic
  or MultiTopic uses the topic, and with the same code when it is called on a
  `DomainParticipant` that did not create it — both sentences of the clause.
  It then removes the topic from the participant's registry and disposes it on
  the SEDP topics writer (DDSI-RTPS 2.5 Table 8.84), and only when the
  deletion is legal. The precondition is a count of *live* users, kept per
  topic name: every endpoint and topic description registers itself at
  creation and unregisters on `Drop`, so a deleted writer stops holding its
  topic. `lookup_topicdescription` (§2.2.2.2.1.12) and `contains_entity`
  (§2.2.2.2.1.10) stop seeing a deleted topic, and a second `delete_topic` on
  another handle of the same topic — the handles share one entity, §2.2.2.2.1.10
  — reports `AlreadyDeleted` (DDS 1.4 §2.2.2.1.1) rather than succeeding
  twice. `delete_topic_by_name` is the untyped form the C FFI needs, and
  `topic_users` reads the counts without attempting the delete.

  **Migration:** `zerodds_dp_delete_topic` now applies the same
  precondition and returns `ZERODDS_PRECONDITION_NOT_MET` while a DataWriter,
  DataReader or ContentFilteredTopic of that participant still uses the topic;
  it also reaches the Rust participant so the topic is disposed on the SEDP
  topics writer. A caller that destroys a topic before its endpoints now sees
  the refusal instead of a freed handle. The participant also tracks its
  ContentFilteredTopics, so `zerodds_dp_delete_contentfilteredtopic` takes
  theirs out of that count and `delete_contained_entities` frees them.

- **The SEDP topics writer and reader** (DDSI-RTPS 2.5 §8.5.4.2,
  `ENTITYID_SEDP_BUILTIN_TOPICS_ANNOUNCER`/`DETECTOR`), announced in
  `PID_BUILTIN_ENDPOINT_SET`. A created topic is announced (at its enable),
  announced again on a QoS change and disposed by `delete_topic` or
  `delete_contained_entities`. A peer's announcement fills `DCPSTopic` with
  every member DDS 1.4 §2.2.5 lists — `TopicBuiltinTopicData` in Rust, the C
  API (`zerodds_topic_builtin_topic_data_free` releases it) and C++ carry
  them all; HISTORY, RESOURCE_LIMITS and TRANSPORT_PRIORITY were missing
  before, because no endpoint announcement holds them. The key is
  `PID_ENDPOINT_GUID` (Table 9.19). Measured against Cyclone DDS 11.0.1 with
  `Discovery/EnableTopicDiscoveryEndpoints`: Cyclone's topics reach
  `DCPSTopic` with every member. The other direction does not: Cyclone keys
  topic samples on its own vendor parameter `0x801b` and refuses ours
  ("deserialization failed", `serdata_plist_fix`). A topic sample that does
  not decode is counted (`DcpsRuntime::refused_topic_announcements`) and no
  longer costs the endpoint announcements in the same datagram.

  **Migration:** `TopicBuiltinTopicData::durability` and `::reliability`
  are the full policies, not their kinds (`.durability.kind`). The C struct
  `ZeroDdsTopicBuiltinTopicData` lost `durability_kind`/`reliability_kind`
  and is released with `zerodds_topic_builtin_topic_data_free`.

- **A ROS 2 dialect for the front end** (`zerodds-idlc --ros2`,
  `Dialect::ros2`): it admits ROS 2's own member form, `Pose pose;`, which
  OMG IDL 4.2 §7.2.3.1 refuses because the member name and the type name
  differ only in case. Only that collision within one declaration is
  admitted; every other one stays an error. `crates/spatial-ros2` reads its
  ROS message IDL under it.

- **A C DDS TypeSupport per topic type** (`9575bf2b4d8`).
  `zerodds-idlc generate --lang c` writes `<stem>_typesupport.h` beside
  the codec header: one `zerodds_typesupport_t` with the type name,
  extensibility, keyedness, the EquivalenceHash of the CompleteTypeObject,
  and `encode`, `decode`, `decode_repr`, `key_hash` and `sample_free` for
  `zerodds_writer_write_typed` and `zerodds_reader_take_typed`. It writes
  XCDR2 and reads XCDR1 and XCDR2 in either byte order; a decoded sample
  owns its strings, sequence buffers and indirections, and `sample_free`
  releases them. Include the companion instead of the codec header.

- **The C preprocessor of ISO/IEC 14882:2003 §16**, which IDL 4.2 §7.3
  adopts (`9303a2a33cf`): function-like macros, `#` and `##`, the rescan
  with hide sets, `#if` over a full constant expression, `__LINE__`,
  `__FILE__`, `#line`, backslash continuation. New code `IDL0022`.

- **IDL 4.2 grammar** (`811b31a6685`): adjacent string literals
  (§7.2.6.3), a constructed type defined inside a `typedef` (rule 64),
  and `typeid`/`typeprefix`/`import` in interface and valuetype bodies
  (rule 112).

- **`expectsInlineQos` is read and answered** (`493eb7e3b5e`).
  `PID_EXPECTS_INLINE_QOS` (0x0043) in SEDP and SPDP; a reader that
  expects in-line QoS gets the topic name and the writer's Table 8.85
  policies in every DATA (DDSI-RTPS 2.5 §8.4.9.2.4).

- **Floating-point constant expressions** (`46fa40c9b8c`). IDL 4.2
  §7.4.1.4.2: evaluated in `double`; `long double` arithmetic, mixed
  integer operands and operators other than `+ - * /` are refused by
  name.

- **`nackResponseDelay` and `nackSuppressionDuration` are settable**
  (`35a2dcf9608`). DDSI-RTPS 2.5 §8.4.7.1, per writer
  (`ReliableWriter::set_nack_response_delay`,
  `set_nack_suppression_duration`) and for every user writer through
  `writer.nack_response_delay_ms` / `writer.nack_suppression_duration_ms`.
  Defaults stay 0 and 0.

- **`Subscriber::copy_from_topic_qos`** (`591e3e38505`), the reader half
  of `Publisher::copy_from_topic_qos`.

- **`AmqpClient::send_to_accepted`** (`e04ef1d2646`): returns once the
  broker settled the delivery as `accepted` (AMQP 1.0 §3.4.2).

- **A C++ header reads both XCDR versions** (`eae15a59d7f`): the other
  version's decoders in a nested namespace; `topic_type_support<T>::decode`
  threw for the version `--encoding` did not name.

- **The C++, Java, C# and TypeScript wireprobe columns write a KeyHash**
  (`d9c8eddc559`).

- **Gates**: the C column compiles the whole corpus
  (`d06d88bf0a2`, job `idl-c-corpus`); every coverage citation must
  resolve (`3dcc6cb9540`, job `coverage-evidence`); every `no_std` claim
  is built (`e57af154bcd`); `interop-results --verify` compares every
  file `--regenerate` writes (`12c41058497`).

- **`@verbatim` in every column** (`a7c29ac5be5`). OMG IDL 4.2 §8.3.5.1:
  the C, C++, C# and Java columns wove the text; the other seventeen
  dropped it. All twenty-one place the six placements now, with
  `BEGIN_DECLARATION`/`END_DECLARATION` inside the declaration's body
  where the language has one (Lua and Node, whose structs are literals,
  take them adjacent). `generate::source` moves `END_FILE` past the codec,
  where it used to stand between the declarations and the codec in every
  column. `crates/lang-rules/tests/verbatim_every_column.rs`.

- **`dds.sec.auth.password` opens an encrypted private key**
  (`e29a7520cb7`). It was refused. PKCS#8 `ENCRYPTED PRIVATE KEY` opens
  with it as the passphrase; the OpenSSL form (`Proc-Type: 4,ENCRYPTED`,
  AES-CBC) with it first as the base64 AES-128 key DDS-Security 1.2
  §10.3.1 Table 50 names and then as the OpenSSL passphrase Cyclone DDS
  and Fast DDS read it as.

- **`data:` URIs in the `dds.sec.*` properties** (`f44e6061f25`,
  `679ed2ee75c`). Table 50 allows them by RFC 2397; they were refused.
  `data:,<content>` is read as Cyclone DDS reads it, the `;base64` and
  percent-encoded forms decode. `pkcs11:` is still refused by name.

- **The Nim endpoint's decoupled send path is a lock-free ring**
  (`631dfc08708`). `spsc_ring.nim` replaces the stdlib `Channel`;
  `spsc_ring_test.nim` runs in `endpoints-gate`, and ThreadSanitizer
  reports nothing on it.

- **Configurable minimal-`TypeObject` vendor camp (XTypes 1.3 7.3.4.5).**
  The minimal `EquivalenceHash` splits into two vendor camps: RTI
  Connext/CoreDX append one octet inside each aggregated member's
  `DHEADER`, Cyclone/Fast DDS/HDDS do not. It is now a serialization axis
  (`Writer::with_minimal_member_trailing_octet`, `to_bytes_le_with`,
  `hash::*_with`, `from_ast::*_with`, carried through `to_minimal` on the
  `TypeRegistry`), mapped per vendor by
  `Dialect::minimal_member_trailing_octet` and selected on a participant
  with `DomainParticipant::set_minimal_type_object_camp`. Default is the
  Cyclone camp (unchanged); with the axis on, the tree reproduces RTI's
  minimal hash octet-for-octet, recursively through nested members.
  `docs/omg-xtypes-issues.md` C2.

- **A new IDL and wire stack.** `zerodds-ast-core` and `zerodds-ast` (the
  source-neutral node space), `zerodds-ast-layout` (XCDR1/XCDR2 member plans
  and the `Dialect` type), `zerodds-idl-cst` / `-parse` / `-lower` (a
  lossless CST, a recursive-descent parser with error recovery, and the
  lowering into the AST), `zerodds-wire` and `zerodds-wire-codec` (the runtime
  a generated codec calls, and the plan executor over it), `zerodds-lang-cst`
  / `-render` / `-rules` / `-python` (target-language CSTs, a renderer that
  decides nothing, and the rules that map the AST onto them),
  `zerodds-idl-compose` and `zerodds-xml-compose` (one call from source to
  registered types, for IDL and for DDS-XML).

- **`zerodds-build`** (`fe13dbca516`) — the `build.rs` path, in the shape a
  Rust caller expects: one module per `.idl`, with `#include` dependency
  tracking so Cargo re-runs when a header changes.

- **`zerodds-config`** (`c2238816875`) — layered configuration with a
  traceable origin: a typed base file, drop-in directories read in order and
  sorted by name inside each, nested `include:` with cycle detection, `+key`
  to append where `key` replaces, and `origin_of` to answer where a value came
  from. It replaces three copies of the same YAML-subset parser in the CoAP,
  MQTT and WebSocket bridges. `RuntimeConfig` reads files underneath the
  environment since `4bb89b627b9`, instead of 23 `ZERODDS_*` variables inline
  in `default()`.

- **`zerodds-proto`** (`6b86783e613` and predecessors) — Protobuf type reuse:
  a `FileDescriptorSet` becomes an XTypes `DynamicType`, so a `.proto` data
  model rides the DDS-native XCDR wire. `no_std` + `alloc`, covering
  `map<K,V>`, repeated message and enum, and `oneof` as a union.

- **`zerodds-float128`** (`a3dc668fff5`) — IEEE-754 binary128 in software, the
  value type behind the sixteen octets CDR gives a `long double`.

- **Native endpoint SDKs in twenty languages** and XRCE endpoint clients
  beside them — Go, Zig, Kotlin, Node, Elixir, OCaml, Julia, F#, Nim, D, Lua,
  Swift, Ada and Ada 83 among them — plus `zerodds-endpoint-rust`
  (`2276f8f5a71`) and the live cross-language ping-pong that exercises them
  (`f619eded8f0`).

- **Dialects as a mechanism** (`fa2ed8b0a9c`): one flag picks a whole
  implementation's answers, over ten axes — map entry order, union label
  matching, enum holder width, fixed digit count, RTI's member-id counter
  after `@hashid`, OpenDDS's map and PID behaviour, and more. Each axis was
  derived from recorded octets, not from plausibility.

- **Coherent sets** (`640808cf1ea`, `05ab9f969dd`) — `begin_coherent_changes`
  and `end_coherent_changes` on the Publisher, `PID_COHERENT_SET` on every
  DATA of the set, and a reader that holds an open set back.

- **`PID_TYPE_INFORMATION` is announced** (`4427eeeb777`, #42). Before this a
  ZeroDDS endpoint wrote its type identity only into its own vendor PID,
  which no other vendor reads.

- **`idlc lint` and `idlc format`** — four rules (`topic-missing-key`,
  `unbounded-collection`, `unbounded-key`, `unused-typedef`), each reporting
  the declaration's own span.

- **Four OMG reference clients**, built and validated locally: dds-rtps
  (`743798b03cc`), dds-security (`892536e1b10`) and dds-xtypes in both suite
  versions (`a33f13ef853`, `f2685b2a07b`).

- **A participant that shuts down announces its departure** (`a0f5b69b9ad`),
  the key-only SPDP DATA of DDSI-RTPS 2.5 §8.5.3.3. A peer dropped a
  ZeroDDS participant only when its lease ran out (100 s by default).

- **DataReaders on a `ContentFilteredTopic`, with the filter announced in
  SEDP** (`28cded5af6b`).

- **`zerodds-idlc dlrl`**, DDS 1.2 §8 object and home classes (`eaea9467ab3`);
  **`generate --dialect`** and every wire-axis flag (`3af36dd60ec`).

- **`threads.*` real-time scheduling** through `zerodds-rt-linux` behind the
  `rt` feature (`f46eb132c85`), and a pluggable time source for
  `SOURCE_TIMESTAMP` and `DESTINATION_ORDER` (`c7ed10e56cf`).

- **`DurabilityServiceQosPolicy` travels in SEDP** (`012ec81b230`,
  `d0b3271b581`).

- **DDS-RPC 1.0: the Enhanced Service Mapping, in `zerodds-rpc` and in the
  generated Rust, C++ and Java bindings** (§7.5.1.2, §7.8.2; `0edbbcf1132`,
  `e305669e2c4`, `41b3b02f7a2`, `45ead22cc2b`, `2ea77f7dad8`).
  `codegen::build_enhanced_service` maps an interface to one `@Choice
  @Autoid` `Request` and one `Reply` structure over all operations with a
  `remoteEx` member, an `In`, `Out` and `Result` structure per operation,
  flat inheritance (§7.5.1.2.6) and `@Mutable` (§7.5.1.2.1.4);
  `RpcError::MemberIdCollision` reports an `@Autoid` hash collision. An
  operation without parameters carries the `dummy` member of §7.5.1.1.4
  rule 3c; `EnhancedOptions::empty_operations` selects the `@Empty`
  reading. `Requester` and `Replier` take
  `RpcEndpointOptions { mapping: RpcMapping::Enhanced }`: the sample is the
  `Request`/`Reply` structure alone, the request is identified by the
  writer GUID and sequence number of its DATA, the reply carries
  `PID_RELATED_SAMPLE_IDENTITY` and is correlated by it, and a remote
  exception is the `remoteEx` member. The C++ and Java bindings have a
  PL_CDR2 codec and the function-call classes for it. Tests:
  `an_enhanced_call_is_answered_through_the_related_sample_identity`,
  `an_enhanced_remote_exception_is_the_remote_ex_member`,
  `an_enhanced_request_has_no_header`.

- **`zerodds-idlc generate --rpc-mapping basic|enhanced|enhanced-empty`**
  (DDS-RPC 1.0 §7.2.4, §7.5.1; `c17fbe91ffe`). The option selects the
  service mapping an `interface` is written for. The specification names
  no default; `basic` needs no extension of DDS and is used when the
  option is absent. `enhanced-empty` writes an operation without
  parameters as an `@Empty` structure (§7.5.1.2.1.3). The DDS-RPC
  annotations `@DDSRequestTopic`, `@DDSReplyTopic`, `@RPCRequestType`,
  `@RPCReplyType`, `@Qos`, `@Choice`, `@Autoid`, `@Empty` and `@Mutable`
  are predefined, so a file uses them without declaring them
  (`6c7f585a36e`). Test:
  `the_rpc_mapping_selects_what_an_interface_is_written_as`.

- **DDS-RPC 1.0 discovery extension and topic aliases** (§7.4.3, §7.6.2.1,
  §7.6.2.2; `211d416cdda`, `bdf9ab1e6ca`, `17a3aef7436`, `5a472acf053`,
  `8385434c232`). Endpoints announce `PID_SERVICE_INSTANCE_NAME` (0x0080),
  `PID_RELATED_ENTITY_GUID` (0x0081) and `PID_TOPIC_ALIASES` (0x0082) and
  read them from matched peers: `DataWriter::set_rpc_discovery`,
  `DataReader::set_rpc_discovery`, `matched_rpc_readers`,
  `matched_rpc_writers` in Rust; `zerodds_writer_set_rpc_discovery`,
  `zerodds_reader_set_rpc_discovery`, `zerodds_writer_matched_rpc_readers`,
  `zerodds_reader_matched_rpc_writers` in C;
  `zerodds_rpc_service_matching_complete` (Figure 7.4) and
  `zerodds_rpc_client_matching_complete` (Figure 7.3) run the matching
  algorithms. A reader and a writer on different topics match when the topic
  of one is among the announced aliases of the other
  (`sedp::topics_correspond`). The Enhanced client sends no request before
  it has discovered both service endpoints, and the service holds the
  requests of a client it has not discovered completely;
  `ServiceMatching`, `ClientMatching` and
  `RpcEndpointOptions::accept_unannounced` select the other readings. The
  C++ and Java runtimes do the same where the binding offers the extension.
  Tests: `the_extension_is_announced_read_and_decides_the_matching`,
  `an_enhanced_request_of_an_undiscovered_client_is_held`.

- **Related sample identity on write and take** (DDS-RPC 1.0 §7.8.2;
  `8385434c232`, `5a472acf053`, `771f1284680`). `DataWriter::write_related`
  and `zerodds_writer_write_related` write a sample with
  `PID_RELATED_SAMPLE_IDENTITY` in its inline QoS; `DataReader::take_with_wire`
  and `zerodds_reader_take_wire` return the writer GUID, the RTPS sequence
  number and the related identity of each sample's DATA;
  `DataWriter::next_sequence_number` names the sequence number of the next
  sample; `DomainParticipant::create_topic_with_type_name` announces a topic
  under a type name of the caller's choice; `DdsType::encode_continuing` and
  `decode_continuing` encode a value that continues a sample at an offset.
  On the DDS Java PSM binding, `DataWriterImpl.writeIdentified(data, related)`
  returns the identity a sample was sent under, and
  `SerializedSample.identity()`/`relatedIdentity()` give both to the reader.
  Tests: `crates/zerodds-c-api/tests/related_sample_identity.rs`,
  `the_zerodds_binding_sends_and_reads_the_related_sample_identity`.

- **DDS-RPC 1.0: listeners run when the sample arrives, and the three request
  processing ways** (§7.9.2.2, §7.9.3; `cd07b2221f0`, `c5af5c7c138`,
  `05c799d7e04`). `Replier::activate`, `activate_async` and
  `Requester::activate` call the synchronous listener, the asynchronous
  listener or the reply delivery as the request or reply arrives, and
  `deactivate_listener` returns to pumping; `close` deletes the DataReader
  and DataWriter (Java: `deleteWriter`/`deleteReader` on the participant
  seam). The `Replier` offers the synchronous `ReplierHandler`/`tick`, the
  asynchronous `AsyncReplierHandler`/`dispatch_async` and polling through
  `receive_requests`, `send_reply` and `send_exception`; the last two hand
  the `RequestIdentity` to the application. In C++, a listener ran only
  inside `dispatch_requests()` or `dispatch_replies()` and a future was
  fulfilled only by a later read; both run on arrival now. The C API has
  `zerodds_reader_set_arrival_callback`, which calls a function on the
  receive thread after each run of samples and leaves the samples in the
  reader (unlike `zerodds_reader_set_data_callback`, whose callback may not
  call ZeroDDS), and `zerodds_writer_set_data_representation` (`0` XCDR1,
  `2` XCDR2). The Java runtime has `ListenerDelivery.ON_ARRIVAL`;
  `ON_PUMP` is the named setting for a reader that cannot notify. Tests:
  `a_signal_follows_a_run_of_samples_and_the_samples_stay_in_the_reader`,
  `the_zerodds_reader_calls_its_callback_on_arrival_and_not_after_removal`.

- **DDS-RPC 1.0 `@Qos` profiles and run-time QoS changes** (§7.10.1,
  §7.10.2; `152906e062f`, `5e31aafecaa`, `e0eb1bf9c60`, `7253f822258`,
  `771f1284680`). `RpcQos::from_qos_url` resolves
  `[file://]path[#library]` (`file` is the only protocol);
  `RpcQos::from_service_profiles(request_url, reply_url)` resolves
  `@Qos(RequestProfile, ReplyProfile)`, and the reply endpoints are built
  from the reply profile when one is named. The generated Rust binding
  reads `@Qos` and has `new_with_declared_qos` and `serve_with_declared_qos`;
  the C++ params have `request_profile()`/`reply_profile()` and
  `DomainParticipant::load_qos_profile`; the Java runtime has `RpcQos` and
  `setQos`. A profile that does not resolve is an error, not the default.
  `set_qos` on `Requester`, `Replier`, `ClientEndpoint` and `ServiceEndpoint`
  changes the mutable policies of the live endpoints, and an immutable or
  inconsistent policy is `RpcError::Dcps` (C++: `ImmutablePolicyError`). The
  C API has `zerodds_endpoint_qos_from_profile_url`. Tests:
  `a_profile_url_resolves_the_named_library_over_the_rpc_defaults`,
  `the_zerodds_binding_carries_the_rpc_qos_to_the_c_api`.

- **DDS-RPC 1.0: a service defined by a pair of types** (§7.3.1.4, §7.4.2.2;
  `1e28b13546c`, `d9446301d9b`, `f641beefbe7`, `217bc385a33`, `80894d01a97`,
  `35156b3406a`). `type_pair::lower_type_pairs` reads two struct types marked
  `@RPCRequestType` and `@RPCReplyType`: a `header` member of
  `dds::rpc::RequestHeader` and `dds::rpc::ReplyHeader` in both types
  (Basic) or in neither (Enhanced), topic names from `@DDSRequestTopic` and
  `@DDSReplyTopic`, and the service name from `@DDSService` on either type.
  `header_pair::{RequestWithHeader, ReplyWithHeader}`,
  `Requester::with_pair` and `Replier::with_pair` run it; the generated Rust
  binding (`emit::type_pair_rust`), C++ (`dds_type_traits` and
  `<Request>Support`) and Java (`<Request>Support` in `ZdServices`) write the
  traits, type names and params. The common types of §7.5.1.1.1 are the
  runtime's, so idlc writes no declaration, codec or `topic_type_support`
  for them. Tests: `a_pair_of_types_with_headers_is_called_and_answered`,
  `a_marked_pair_with_headers_is_a_basic_service`.

- **DDS-RPC 1.0: generated clients and services in C++ and Java, and the
  Rust XCDR1 forms** (§7.2.2.3, §7.9.1, §7.11.1.1.2; `ec3707125a9`,
  `c512ee36551`, `cdb7355e374`, `771f1284680`). In C++, `${I}Client` and
  `${I}Service` pack and unpack the `Call`, `Return` and `Result` unions,
  throw a declared exception or the exception of a remote exception code,
  and return a future from `${op}_async`; `${I}Service` derives from
  `dds::rpc::Service`, so a `Server` takes it, and a derived interface holds
  the client and service of each base on the base's topics (§7.5.1.1.8).
  The Java binding runs over the DDS Java PSM through
  `org.omg.dds.rpc.zerodds.ZeroDdsParticipant`. The generated Rust binding
  writes `encode_xcdr1` and `decode_xcdr1` for every topic type, exception
  and declared type they reach, so a nested `@appendable` member has no
  DHEADER and a nested `@mutable` member is a parameter list under classic
  CDR (XTypes 1.3 §7.4.1); a type that reaches one without an XCDR1 form
  keeps the refusing defaults of the trait. Tests:
  `the_generated_function_call_client_calls_the_generated_service`,
  `crates/rpc-rust-e2e/tests/xcdr1_nested.rs`.

- **`zerodds-c-api`: the RPC calls are behind the `rpc` feature, which is in
  the default set** (`a0d1af7efd7`, `50c8e93c2c5`, `5001d179667`). The
  discovery calls and the profile-URL loader need it; `set_qos` does not. A
  build with `--no-default-features --features std` does not link
  `zerodds-rpc`; `zerodds-xml` is linked either way. `zerodds.h` lists the symbols either
  way, because cbindgen does not evaluate features. The two peer-list calls
  report `OutOfResources` past their capacity, and
  `ZeroDdsRpcPeer.name_truncated` is set when an announced instance name did
  not fit its 257-octet buffer; a truncated name is never compared. Tests:
  `writer_matched_rpc_readers_bounds`, `reader_matched_rpc_writers_bounds`.

- **`zerodds-idlc generate --lang c++ --cpp-mapping native|psm|classic`**
  (DDS-PSM-Cxx 1.0 §8.1; `1db807cbf0f`, `13777502ceb`, `f5fde275918`).
  `psm` is the default; `native` is the mapping that existed; `classic` is
  the OMG IDL to C++ 1.3 mapping in C++03, over the native codec (test:
  `tools/idlc/tests/cpp_classic.rs`). `psm` writes a structure as a class with the
  accessors of §7.4.6, an all-members constructor, `swap` and `==`; a union
  as a class with `_d()` and one accessor per case; `wchar` as `wchar_t`,
  `wstring` as `std::wstring`, `map` as `std::map`, an array as
  `dds::core::array` and `@optional` as `dds::core::optional`. The wire
  codec, the TypeObject and the endpoint gates are the native ones: the
  native types move into the namespace `zd_native`, and
  `zerodds::psm::Conv<P>` (`zerodds/psm/convert.hpp`) converts between a PSM
  class and its native type. A PSM sample costs one conversion on `write`
  and one on `take`. `long double` converts through IEEE binary128 and
  rounds to the width of the platform's `long double`. Exceptions, value
  types and constants stay native, and the spellings `@Optional` and
  `@Shared` are not recognised, because IDL 4.2 §8.2 makes annotation names
  case-sensitive. Under `native`, generated structures have a member
  `swap(T&)` and an `inline swap(T&, T&)` in the type's namespace
  (DDS-PSM-Cxx 1.0 §7.12; `fcc81cc5e24`); a structure with a member spelled
  `swap` gets neither. Test:
  `a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`
  (`generated_psm_roundtrip.cpp` from `tests/generated/psm_probe.idl`).

- **`dds::core::QosProvider` and `zerodds_qosprovider_*`** (DDS-PSM-Cxx 1.0
  §7.6.2.1; `ebaa3c4e2e6`, `4ef7021a8ed`, `4ed2678ac5b`, `0e91cffd853`).
  The provider loads the DDS-XML QoS profiles of a URI and answers
  `participant_qos`, `topic_qos`, `publisher_qos`, `subscriber_qos`,
  `datawriter_qos` and `datareader_qos` by profile id (`Library::Profile`,
  `base_name` inheritance per DDS-XML 1.0 §7.3.2.4.2) or for the provider's
  own profile. URIs: `file:` (RFC 8089), `data:` (RFC 2397, percent-encoded
  or `;base64`), `http:` (one HTTP/1.1 GET, status 200 only, no redirect,
  10 s timeout) and `https:` (TLS 1.2/1.3 through rustls with the webpki
  roots; not exercised against a TLS server). Another scheme is
  `ZERODDS_UNSUPPORTED` (C++ `UnsupportedError`), a text that is no URI is
  `BAD_PARAMETER`, a profile the document lacks is `InvalidArgumentError`.
  Of several QoS containers in a profile the provider returns the one
  without a `topic_filter`, else the first; no call selects by topic name,
  because the PSM `QosProvider` has none. New dependencies of
  `zerodds-c-api`: `zerodds-xml`, `base64`, `rustls`, `rustls-pki-types`,
  `webpki-roots`. Tests:
  `a_provider_answers_the_qos_of_every_entity_kind_with_inheritance`,
  `a_data_uri_and_an_http_uri_name_the_document_too`,
  `an_unknown_scheme_is_unsupported_and_a_missing_file_or_a_closed_https_port_is_bad_parameter`,
  `test_qos_provider`.

- **DDS-XTypes dynamic types in C++ and the C API** (DDS-PSM-Cxx 1.0 §7.5;
  `6909f5fe2aa`, `854cd8fabfa`, `ae11c055ad2`, `7d5ea3d45b3`).
  `zerodds_dyntype_from_idl` builds a dynamic type from IDL text, with the
  name, key, extensibility, TypeObject, TypeObject dependencies and default
  value as JSON, and `zerodds_dyntype_encode_json`/`decode_json` between a
  value in the JSON dialect of `zerodds-wire-probe` and the body of a sample
  (XCDR1 or XCDR2, either byte order). `zerodds_types_load` and
  `zerodds_types_describe_idl` describe the types of an IDL or DDS-XML
  document as JSON; `zerodds_dr_set_type_objects` gives a reader the type of
  its samples. C++ has `dds/core/xtypes/`: `DynamicType`, the primitive,
  string, sequence, map, structure and union types, `TDynamicData` with
  `value<T>` by member name or id, `encode` and `decode`, `TypeProvider`
  (`load_type`, `load_types` from `file:`, `data:`, `http:`, `https:`),
  `Topic<DynamicData>`, and `BytesTopicType`, `StringTopicType`,
  `KeyedBytesTopicType`, `KeyedStringTopicType` (XTypes 1.3 Annex E).
  Not included: bitmasks, arrays, unions with a default case or an
  enumeration discriminator, members of a union by name, `fixed`. Tests:
  `a_type_from_idl_has_the_identity_of_a_compiled_one_and_a_value_round_trips`,
  `idl_types_are_described_with_members_keys_enumerators_and_aliases`,
  `a_dds_xml_type_library_is_described_like_the_idl_it_stands_for`,
  `xtypes_dynamic_data.cpp`, `xtypes_dynamic_topic.cpp`.

- **C API: `enable`, `get_status_changes` and `get_instance_handle` for every
  entity, and ENTITY_FACTORY `autoenable_created_entities`** (DDS 1.4
  §2.2.2.1.1.6–8, §2.2.3.20; `789ac482d54`, `5d6bf43ff4b`).
  `zerodds_{dp,topic,pub,sub,dw,dr}_enable`, `_get_status_changes` and
  `_get_instance_handle` are new. An entity created from a disabled factory,
  or from one that does not autoenable, is created disabled; a DataWriter or
  DataReader is announced and matched at its first `enable`, and an
  operation the specification does not allow on a disabled entity returns
  `ZERODDS_NOT_ENABLED`. `zerodds_dpf_set_qos` reaches the factory that
  creates participants. Tests:
  `entities_of_a_participant_that_does_not_autoenable_are_created_disabled`,
  `a_writer_of_a_publisher_that_does_not_autoenable_is_not_announced_until_enabled`.

- **C API and C++: QueryCondition setters, instance reads with a condition,
  key hash, incompatible-QoS counts** (`267fcd18f01`, `d7324a52745`,
  `b2ef9044822`, `b5df4c0dc8e`). `zerodds_querycondition_set_expression` and
  `_set_parameters` change a query condition (DDS 1.4 §2.2.2.5.9), and
  `zerodds_dr_{read,take}_instance_w_condition` and
  `_next_instance_w_condition` read the samples of an instance that a
  condition selects; C++ `QueryCondition::expression(str)`, `parameters(..)`,
  `add_parameter`, `select().next_instance(h).content(query)` and
  `StatusCondition::entity()` use them (a content filter together with
  `instance(h)` is refused, since DDS has no such read).
  `zerodds_dw_instance_key_hash` returns the key hash of a handle's instance,
  so `dispose_instance(handle)` and `unregister_instance(handle)` reach
  `zerodds_dw_dispose`.
  `zerodds_dw_get_offered_incompatible_qos_status_ex` and
  `zerodds_dr_get_requested_incompatible_qos_status_ex` return the count per
  policy (`ZeroDdsIncompatibleQosStatusEx`, up to 32
  `ZeroDdsQosPolicyCount`); the plain status has the last policy id only.
  Tests: `the_key_hash_of_a_registered_instance_comes_back_and_an_unknown_handle_is_refused`,
  `both_sides_report_the_incompatible_policy_with_its_count`.

- **The headers and functions DDS-PSM-Cxx 1.0 §7.2 lists and the library
  lacked** (`f1a07de5ec2`, `4dbadb87eda`, `49df7a275e3`, `9feb13103aa`,
  `cd7f594cdff`). `dds/core/macros.hpp`, `conformance.hpp`
  (`OMG_DDS_*_SUPPORT`), `Optional.hpp`, `array.hpp`, `dds/pub/CoherentSet.hpp`,
  `SuspendedPublication.hpp`, `sub/CoherentAccess.hpp`,
  `topic/BuiltinTopicKey.hpp`, `dds/{pub,sub,topic}/find.hpp`;
  `dds::domain::find`, `dds::{domain,pub,sub,topic}::ignore`;
  `topic::discover<TOPIC>(dp, name, timeout)` and the forms without a
  participant, which read from all participants the process created and
  still holds; the iterator form of `matched_subscriptions` and
  `matched_publications`; the normative listener classes with a `NoOp`
  class each; `AnyTopic`, `AnyTopicDescription`, `AnyDataWriter`,
  `AnyDataReader`, `Filter`, `TopicInstance<T>`. `find` reads a registry of
  weak references to what the process created. Each public header compiles
  on its own; a CMake object library builds one translation unit per
  header. Tests: `test_layout_headers_and_find`,
  `test_topic_any_topic_and_listener`, `test_publisher_subscriber_datawriter_api`.

- **`zerodds-idlc generate --lang java --java-mapping omg|psm`**
  (DDS-Java-PSM 1.0 §9.1.1, §9.1.2; `5e078003c16`). `omg`, the IDL-to-Java
  mapping, stays the default and its output is unchanged. `psm` is Clause 9:
  a structure, union, bit set or exception is a final class with private
  state, a no-argument constructor and Bean accessors (`isX()` too for a
  `boolean`); an unbounded sequence is `java.util.Collection<E>`, a bounded
  one and an array are Java arrays. The codec reads and writes through the
  accessors and calls `org.zerodds.wire.Sequences` for the one thing it
  cannot decide by type (count, index, allocate, fill and trim an array or a
  `Collection`). An `interface` is refused under `psm`, by name. The
  generated output of the 418 renderable cases of `tests/interop-corpus`
  compiled under both mappings with `javac -Xlint:all` and no warning
  (stated in the commit message; not re-run for this entry). Tests:
  `the_default_is_the_idl_to_java_mapping_and_the_option_is_scoped`
  (`crates/lang-rules/tests/java_psm_mapping.rs`), `SequencesTest`.

- **Java Type Representation of DDS-Java-PSM 1.0 §8 in the reflective type
  support and in `createType(Class)`** (`30a9d32dfc7`, `36094b9d871`,
  `cb60331dfa9`). `@SerializeAs` (§8.2) gives the DDS kind of a field in
  place of the default of Table 8.1 (`UINT_16`, `UINT_32`, `UINT_64`,
  `FLOAT_128`, `CHAR_32`, wide and bounded strings, bounded sequences and
  maps, arrays with their dimensions, bit sets); a combination Table 8.2
  does not list is an `XcdrException` at the first introspection. A class
  annotated `@SerializeAs(UNION_TYPE)` with one `@UnionDiscriminator` field
  and `@UnionMember(discriminatorValue = …)` fields is a union (§8.6.2). An
  enumeration annotated `@BitSet` is a bit set type, and an `EnumSet` or
  `java.util.BitSet` member marked `@BitSet` is a bit mask (§8.7).
  `createType(Class)` mapped every member by a recursive `createType`, so a
  primitive became an empty structure; it follows Table 8.1 and reflects
  strings, enumerations, maps, collections, arrays, nested structures,
  unions and the annotations `Extensibility`, `Key`, `Optional` and
  `MustUnderstand`. `@SerializeAs(ALIAS_TYPE)` and `ANNOTATION_TYPE` on a
  member stay refused, with the reason in the message. Tests:
  `SerializeAsTest`, `ReflectionTypeSupportTest`, `DynamicTypeReflectionTest`.

- **`createTopic(String, DynamicType)` works** (DDS-Java-PSM 1.0 §7.5.1;
  `d2ca428c7b8`, `3cfc4060517`). Both forms threw
  `UnsupportedOperationException`. PSM 1.0 types the result
  `Topic<DynamicType>` while its samples are `DynamicData`;
  `DynamicTopics.dataTopic(topic)` gives the `Topic<DynamicData>` that
  `createDataWriter` and `createDataReader` take, and
  `DomainParticipantImpl.createDynamicTopic` does both steps.
  `DynamicDataTypeSupport` is the XCDR2 codec of a `DynamicType` (FINAL,
  EXTENSIBLE, APPENDABLE and MUTABLE structures, unions, sequences, arrays,
  maps, strings, enumerations, bit sets and the primitive kinds) with the
  bytes of `ReflectionTypeSupport` for the corresponding class.
  `createTopic(name, DynamicType, TypeSupport)` keeps the topic's type name;
  a null `TypeSupport` is an `IllegalArgumentException`. Test:
  `aWriterAndAReaderOnTheTopicOfADynamicTypeMoveDynamicData`.

- **Content filters and `MultiTopic`** (DDS 1.4 §2.2.2.3.4, §2.2.2.5.3.8,
  Annex B; DDSI-RTPS 2.5 §9.4.1.1; `926a94382a1`, `2ded60e772b`,
  `fd437c122cf`). `createContentFilteredTopic`, the trigger value of
  `createQueryCondition` and the query expression of a read/take
  `Selector` raised `UnsupportedOperationException`. `org.zerodds.sub.ContentFilter`
  evaluates the comparison operators, `AND`, `OR`, `NOT`, parentheses,
  `LIKE` (with `ESCAPE`), `BETWEEN`, `IN`, `IS [NOT] NULL`, `%n` parameters
  and the operands `FIELDNAME RelOp FIELDNAME` and `Parameter RelOp FIELDNAME`
  over a dotted member path (`getX()`, `isX()`, a method named like the
  member, or a field). A `MultiTopic` takes `SELECT <fields | *> FROM
  <topic> [NATURAL JOIN <topic> …] [WHERE <condition>]`; its reader reads
  the topics of the FROM clause through readers of its own, joins the
  current samples on the members the topics share by name (a cross product
  when none), projects, filters and delivers an ordinary reader.
  `-Dzerodds.multitopic.join=key` restricts the join to members that are
  keys in every topic that has them. The expression is checked when the
  MultiTopic is created (`IllegalArgumentException`); `ORDER BY` is a syntax
  error. Tests: `ContentFilterTest`, `MultiTopicTest`, `MultiTopicJoinTest`.

- **Built-in topics with their QoS** (DDS 1.2 §2.2.1.3.1, DDS 1.4 §2.2.5,
  DDSI-RTPS 2.5 §9.3.1.2, §9.6.2; `4b11df9eaaf`, `f7567dbd513`,
  `62d0de660ef`). A `DomainParticipant` creates `DCPSParticipant`,
  `DCPSPublication`, `DCPSSubscription` and `DCPSTopic` and announces itself,
  its topics, writers and readers on them, so `getBuiltinSubscriber()` sees
  the domain; `createTopic` with a built-in name returns that topic. The data
  carries every member of the structure and travels as a `PL_CDR_LE`
  ParameterList (Table 9.13); it was the identifying members only. SEDP
  announces the QoS of each writer and reader (reliability and durability
  kinds only before), SPDP the USER_DATA of the participant, and a policy
  absent from an announcement is the default of the entity kind.
  `getMatchedSubscriptionData` and `getMatchedPublicationData` return the
  matched entity's data and `BAD_PARAMETER` for a handle that is not
  matched; they threw `PreconditionNotMetException`. The TypeInformation of
  the entity's type is `PID_TYPE_INFORMATION` (0x0075) in the built-in
  samples. `equivalentTypeName` and `baseTypeName` travel on the
  vendor-specific parameters 0x8003 and 0x8004, since the PSM's `@ID(0x0075)`
  and `@ID(0x0076)` collide with XTypes 1.3. Tests: `BuiltinTopicsTest`,
  `thePublicationWireFormCarriesTheQosOnTheParameterIdsOfTable913`
  (`BuiltinTopicQosTest`).

### Fixed

- **C++: a structure with a bitset member compiles under the default
  mapping.** Under `--cpp-mapping psm` a bitset is the native type, and the
  `operator==` of a PSM class compares every member with `==`; the native
  bitset had none, so the header did not compile. A native bitset has
  `operator==` and `operator!=` over its named fields now. A bitset of
  padding alone left its decoder an unused `zd_bits` local, an error under
  `-Wextra -Werror`; the anchor is read and not kept. Test:
  `a_structure_with_a_bitset_member_compiles_under_the_default_mapping`.

- **Java: a topic of a generated TypeSupport carries samples.**
  `createTopic(name, ZdTypeSupports.Support_<tag>.INSTANCE)`, the
  explicit-TypeSupport form of DDS-Java-PSM 1.0 §7.5.1, created the topic,
  and `createDataWriter` and `createDataReader` refused it with "topic
  carries a TypeSupport of another implementation". A TypeSupport that is
  its own codec (`org.zerodds.cdr.TopicTypeSupport`), as every generated one
  is, is taken. Test: `generated_type_supports_extend_the_psm_type_support`,
  which sends a sample through such a topic.

- **Generated Kotlin compiles under Kotlin 2.4.** A typedef of a plain
  value had a function of the alias's own name beside its `typealias`
  (`typealias ValueT = Int`, `fun ValueT(): ValueT`), for the codec's
  `Array(n) { ValueT() }`; kotlinc 2.4.21 reports the two as conflicting
  overloads, 2.0.21 accepted them. The function is gone; the codec writes
  the zero into the slots (`Array(n) { 0 }`), and a defaulter's empty
  sequence of such an alias does the same. Test:
  `the_generated_kotlin_compiles` under kotlinc 2.4.21.

- **DDS-RPC: a type from `zerodds-build` is answered under XCDR1.** A
  reply stands behind its 28-octet `ReplyHeader` (DDS-RPC 1.0 §7.5.1.1).
  The `DdsType` impls of `zerodds-build` had no `encode_continuing` and
  `decode_continuing`, so the trait's default refused that offset under
  XCDR1, and the replier sent `REMOTE_EX_UNKNOWN_EXCEPTION` in place of the
  reply. The generated codec now continues at any offset
  (`Writer::continuing`, `Reader::skip_to`), as the interface emitter's
  types already did. Tests: `the_continuing_pair_is_overridden`,
  `a_generated_type_continues_behind_a_reply_header`.

- **Generated Java has no documentation comment that documents nothing.**
  The Java generator wrote what is left of a `typedef` or a `native` type,
  and a note between statements, as Javadoc; javac 23 and later report
  that as `dangling-doc-comments` and `-Werror` refuses it. They are line
  comments now, as in the C# generator. A file's leading comment is a line
  comment as well: above `package`, or above a class with its own Javadoc,
  it was a second Javadoc attached to nothing, and the Java wire-probe
  column did not compile under javac 25. Eleven hand-written Java comments
  that had been separated from their method by a later insertion are back
  at it.

- **The script locks hold under uutils `mkdir`.** The wire-probe runtime
  cache, the eleven corpus sweeps and `silent-drop-guard.sh --update` took
  their lock with `mkdir`. uutils `mkdir` before 0.11.0, the `mkdir` of
  Ubuntu 26.04 (`rust-coreutils` 0.10.0), reports success to a concurrent
  loser (uutils #13970): on the GitHub runner two cache publishers held the
  lock at once and `runtime_cache_gate` was red. With uutils 0.10.0, 16
  concurrent `mkdir` of one path had more than one winner in 194 of 500
  rounds, GNU `mkdir` in none. The lock is a file the shell creates with
  `O_EXCL` (`set -o noclobber`) now. Test: `runtime_cache_gate`, whose 24
  parallel publishers see a `mkdir` that ignores `EEXIST`.

- **The GitHub interop workflow has its scripts and its Python.** The
  mirror's exclude list named `ci/` without a leading slash, which also
  left out `tests/interop/ci/`: `interop.yml` stopped at
  `install_fastdds.sh: No such file or directory`. The entry is `/ci/` now,
  and `build-github-mirror.sh` reports a workflow step that names a file the
  mirror leaves out (with the old entry: 7 references in `interop.yml`).
  `cyclonedds` 11.0.1 has wheels for CPython 3.10 to 3.13; the cyclone and
  harness jobs set up 3.13, where the runner's 3.14 made pip build the
  sdist. The harness job builds the reader as the GitLab job does; the
  `zerodds-idlc generate` step it still ran is gone with the generator.
  `install_fastdds.sh` hands CMake the generator by its own path: through
  the symlink it put on `PATH`, the Fast-DDS-Gen launcher looked for its
  jar beside the symlink and the `fastdds_robot` configure failed.
  The harness job then passed with `test_domain_arg.sh` failing
  (`cyclonedds not importable`): a step without `shell: bash` runs
  `bash -e` without `pipefail`, and `test.sh | tee log` reported `tee`'s
  status. The Linux workflows run bash with `pipefail` now, the harness job
  installs `cyclonedds`, and `build-github-mirror.sh` reports a workflow step
  that pipes without `pipefail` (on the previous workflows: 9 steps).

- **`generated_type_via_cargo` makes the C++ runtime archive once.** Its
  three tests run in parallel, and each ran `make` for
  `runtime/cpp/build/libzerodds_wire.a` in the same directory. On the GitHub
  runner two of them met in `ar`: "file format not recognized" and "No such
  file or directory". The archive is made once per test process now.

- **Zenoh bridge: an auto-discovered topic waits for its type.** The
  daemon asked once for a discovered endpoint's type and waited at most
  1 s; a reply that came later left the topic bridged without its type for
  good, a keyed topic's Zenoh -> DDS samples then without their key hash.
  On a loaded CI host `auto_discovered_keyed_topic_with_registered_type_bridges_keyed`
  was red that way in one of three pipelines. A direction whose
  `EquivalenceHashComplete` the registry does not have now waits without
  blocking the discovery loop, is asked for again every second and wired
  as soon as its type resolves, or after 10 s with the entity-kind fallback
  as before. Tests: `a_keyed_topic_whose_type_arrives_late_is_bridged_with_it`,
  `a_keyed_topic_whose_type_never_resolves_is_bridged_after_the_deadline`.

- **dust_dds 0.16.1 and ZeroDDS match each other.** ZeroDDS announced
  LIVELINESS, DEADLINE and LIFESPAN also at their default, a Duration_t
  INFINITE. dust_dds 0.16.1 reads that value as a finite duration and
  refused ZeroDDS endpoints as incompatible, LIVELINESS at its reader and
  DEADLINE at its writer: in OMG dds-rtps ZeroDDS -> dust_dds 91 of 105
  cases were red, dust_dds -> ZeroDDS 90. The three policies are left out
  at their default now, as Cyclone 11.0.1, Connext 7.7.0 and OpenDDS 3.34.0
  do (`crates/discovery/tests/fixtures/vendor-discovery/`); a reader takes
  the default for an absent parameter (DDSI-RTPS 2.5 §9.4.2.11). Tests:
  `the_infinite_defaults_are_left_out_and_read_back`,
  `the_readers_infinite_defaults_are_left_out_and_read_back`. dust_dds
  0.16.1 also announces the key `color` of ShapeType unbounded where the
  suite's IDL has `string<128>`, and a ZeroDDS reader refuses such a writer
  under XTypes 1.3 Table 19; `shape_main` sets
  `xtypes.key_member_bounds=ignored` itself. Measured on the Linux test
  host: ZeroDDS -> dust_dds 40 red, dust_dds -> ZeroDDS 41. The 40 of each
  direction are cases every other product fails against dust_dds in the
  published run of 2026-09-30; the 41st, `Test_LargeData_0`, is green 6 of 6
  with the HEARTBEAT suppression of the next entry. Against the other eight
  products `Test_Reliability_0`,
  `Test_Deadline_0`/`_1` and `Test_Lifespan_0`/`_1` in both directions 74 of
  80 green, the 6 red ones red for every other product as well.

- **`shape_main` readers ignore a HEARTBEAT that comes within 62.5 ms of
  the previous one**, as the suite's Connext reader does: Connext 7.7.0
  keeps its default `heartbeat_suppression_duration` of 62.5 ms there, and
  the suite sets only its response delay to 0, which `shape_main` already
  took over. A reader that answered every HEARTBEAT at once asked dust_dds
  0.16.1, which puts one between the fragments of a sample, again for every
  sample in flight: `Test_LargeData_0` dust_dds -> ZeroDDS 1.5 GB on the
  wire for 180 samples, a Cyclone reader 26 MB for 199; red once in four
  runs at 457 of 500 samples. Measured after: dust_dds -> ZeroDDS
  `Test_LargeData_0` 6 of 6 green; each of the nine products -> ZeroDDS
  `Test_Reliability_3`/`_4`, `Test_History_0`/`_1`, `Test_Durability_9` and
  `Test_LargeData_0` 54 of 54 green. The runtime's default stays 0
  (DDSI-RTPS 2.5 §8.4.10.1).

- **A bitmask discriminator with a bit its type does not declare is
  refused** (DDS-XTypes 1.3 §7.2.2.4.4.3). The bit was left out, so the
  discriminator read as another value and selected another case or none.
  A plain bitmask member still leaves such a bit out, as §7.4.1.1.3.2 has
  it. Test: `an_undeclared_bit_in_the_discriminator_is_refused`.

- **XRCE: a reliable stream with a full reorder buffer takes the sample it
  waits for.** `recv_data` refused every sample once 64 were held, the
  expected one too, which is the one that empties the buffer; the stream
  stopped. Test: `recv_data_accepts_the_expected_sample_when_the_buffer_is_full`.

- **XRCE: two agents on one host find each other.** The GUID prefix of
  their runtimes came from the domain and the host alone, so both agents
  had the same one. It carries the process and a counter now. Test:
  `crates/xrce-agent/tests/two_agents.rs`.

- **An endpoint ignored while its match was being made does not stay
  matched** (DDS 1.4 §2.2.2.2.1.14–17). `ignore_participant`,
  `ignore_topic`, `ignore_publication` and `ignore_subscription` took out
  the matches already made, and the filter kept new ones from forming; a
  match a thread was wiring at the moment of the call formed after the
  disconnect and stayed, since later passes only refused to wire it again.
  The match path asks the filter again after wiring and takes such a match
  out. Seen once on the Linux test host with security in
  `ignore_topic_unmatches_the_endpoints_of_the_topic`, where the same-process
  peer's endpoints are taken after its authentication.

- **A data listener runs on the thread that handed the samples over, not
  on the listener dispatcher's.** Every pass of the dispatcher, which a
  change of any status starts, called the data listeners for what had
  arrived; a pass that came before the receiving thread ran them on
  `zerodds-listeners`. The pass now calls them only for what no thread
  handed over: samples that came before a data listener was set, samples
  still untold at two passes in a row, and writers that left. Seen once on
  the Linux test host in `a_listener_writes_from_the_receive_thread_without_waiting_for_the_window`.

- **Forwarded and directed samples are taken as DDSI-RTPS 2.5 §8.7.7 and
  §8.7.9 say.** A change with `PID_ORIGINAL_WRITER_INFO` is handed out as
  the original writer's, with its sequence number; one the reader has
  already, from the original or another forwarder, is a duplicate. A reader
  that no `PID_DIRECTED_WRITE` names acknowledges the sample and absorbs it
  as after a GAP; only the first of several targets was read. Neither
  parameter was evaluated. `DcpsRuntime::write_user_sample_on_behalf` and
  `write_user_lifecycle_on_behalf` write on behalf of another writer; the
  durability service replays its stored samples so. Before, a late joiner
  that received a writer's history from the writer and from the service
  held every sample twice. Tests: nine in `reliable_reader::tests`, among
  them `a_forwarded_change_is_the_originals_and_its_direct_copy_a_duplicate`
  and `a_writer_that_comes_back_starts_a_new_stream`;
  `a_sample_written_on_behalf_of_another_writer_is_that_writers`,
  `a_replayed_sample_goes_out_on_behalf_of_its_writer`.

- **A dispose or unregister with its `serializedKey` reaches the instance of
  that key.** The DataReader keyed a data sample by the hash it computes
  from the sample and a lifecycle marker by the writer's `PID_KEY_HASH`,
  and dropped the key holder the marker came with. zzdds 0.2.0 sends the
  key's octets as `PID_KEY_HASH` where DDSI-RTPS 2.5 §9.6.4.8 takes the
  MD5 of a key that can exceed sixteen octets; its dispose landed on an
  instance of its own without a key holder, and every `take` after it
  returned `Unsupported`. OMG dds-rtps `Test_FinalInstanceState_0`/`_1`/
  `_2` with zzdds as the publisher failed and pass now (measured). Test:
  `a_dispose_with_a_foreign_key_hash_disposes_the_instance_of_its_key`.

- **The QoS policy ids of DataRepresentation and TypeConsistencyEnforcement
  are 23 and 24**, as XTypes 1.3 Annex D has them; they were the other way
  round, and the incompatible-QoS status named a representation that did
  not match by the id of the other policy. Test:
  `the_xtypes_policy_ids_are_those_of_its_idl`.

- **A listener's status is read before the listener runs** (DDS 1.4
  §2.2.4.2.1: "if the application calls the get_<plain communication
  status> from inside the listener it will see the status already
  reset"). It was acknowledged after the listener returned; a
  `get_*_status` inside it read the changes a second time. Test:
  `the_status_is_read_before_the_listener_runs`.

- **Attaching a condition wakes a waiting WaitSet** (DDS 1.4
  §2.2.2.1.6.1). A wait over read conditions alone slept until the next
  sample or its timeout (measured: 5 s, now 0.10 s). Test:
  `attaching_a_triggered_condition_unblocks_a_waiting_waitset`.

- **`create_topic`, `create_datawriter`, `create_datareader` and `set_qos`
  refuse an inconsistent QoS** with INCONSISTENT_POLICY (DDS 1.4 §2.2.3,
  §2.2.3.12, §2.2.3.18, §2.2.3.19): a HISTORY depth above
  max_samples_per_instance, max_samples below max_samples_per_instance, a
  KEEP_LAST depth below 1, a TIME_BASED_FILTER longer than the DEADLINE.
  They were taken. Tests: `qos_consistency.rs`.

- **The instance state and view state in a SampleInfo are those of the
  instance when it is read** (DDS 1.4 §2.2.2.5.1.3, §2.2.2.5.1.8). They
  were set when the sample arrived: a sample read after its instance was
  disposed showed ALIVE, a second read NEW, and the state masks selected
  by the old state. Tests:
  `the_instance_state_of_a_sample_is_that_of_its_instance_when_read`,
  `the_view_state_of_a_sample_is_that_of_its_instance_when_read`.

- **An instance whose writers lost their liveliness is
  NOT_ALIVE_NO_WRITERS** (DDS 1.4 §2.2.2.5.1.3, §2.2.4.2.2). A writer whose
  lease ran out changed only LIVELINESS_CHANGED; the instance stayed
  ALIVE. An unregister of one of several writers of an instance made it
  NOT_ALIVE_NO_WRITERS; it leaves the instance ALIVE now. Under OWNERSHIP
  EXCLUSIVE the owner gives an instance up when it loses its liveliness,
  its match or unregisters it (§2.2.3.9.2, §2.2.3.23.3), and only the
  owner disposes it. Tests: `instance_writers_lifecycle.rs`,
  `an_instance_whose_writer_lost_its_liveliness_has_no_writers`.

- **A writer that leaves raises DATA_AVAILABLE for the
  NOT_ALIVE_NO_WRITERS markers it leaves** (DDS 1.4 §2.2.4.2.2). A writer
  unmatched, gone with its participant or out of liveliness left its
  markers for the next read or take, and until then no
  `on_data_available` was called, and the reader's StatusCondition and a
  ReadCondition for NOT_ALIVE_NO_WRITERS samples stayed down. Tests:
  `a_lost_writer_raises_data_available_before_any_read`,
  `a_writer_that_leaves_raises_data_available_before_any_read`.

- **A writer lost with its participant counts in `not_alive_count`**
  (DDS 1.4 §2.2.4.1: it "becomes not alive for some reason other than the
  normal deletion of that DataWriter"). A participant whose lease ran out
  took its writers out of `alive_count` and out of every count, as an SPDP
  or SEDP dispose does; the writer stays not alive until it matches again
  under its GUID and reasserts. Cyclone DDS 11 and OpenDDS count it as
  deleted. Tests:
  `a_writer_lost_with_its_participant_is_not_alive_until_it_reasserts`,
  `a_writer_lost_with_its_participant_counts_as_not_alive`.

- **An ACKNACK with the FinalFlag clear is answered by a HEARTBEAT**
  (DDSI-RTPS 2.5 §9.4.5.3), for user, SEDP and TypeLookup writers: at
  once when the reader had none yet, else a period after the last one at
  the earliest. The flag stopped at the receive path. The ZeroDDS reader
  cleared F in every ACKNACK to a writer without data, also after that
  writer's HEARTBEAT; it sets F once a HEARTBEAT came and nothing is
  missing. Test:
  `an_acknack_with_the_final_flag_clear_is_answered_by_a_heartbeat`.

- **A change acknowledged before and asked for again is sent again**
  (DDSI-RTPS 2.5 §8.4.2.3), as a GAP when the writer no longer holds it.
  A reader that came back under the same GUID got nothing for what its
  earlier self had acknowledged. Tests:
  `a_request_for_an_acknowledged_change_is_kept`,
  `an_acknowledged_change_asked_for_again_is_sent_again`.

- **A best-effort reader is passive, and a writer serves it as one**
  (DDSI-RTPS 2.5 §8.4.2.3, §8.4.4, §8.4.9.1). A best-effort user reader
  sent ACKNACK and NACK_FRAG like a reliable one; it sends nothing now. A
  writer made the proxy of every matched reader with its own reliability,
  so a RELIABLE writer waited for the acknowledgments of a BEST_EFFORT
  reader, and such a reader got the history of a TRANSIENT_LOCAL writer
  only by asking for it. The proxy is reliable only when writer and reader
  are, and `tick` pushes a lagging best-effort proxy every change it has
  not been sent, a GAP for one the cache no longer holds. Tests:
  `a_best_effort_reader_sends_no_acknack`,
  `a_best_effort_reader_matched_late_is_pushed_what_the_cache_holds`,
  `transient_local_writer_delivers_history_to_late_joiner`.

- **An invalid submessage invalidates the rest of the message, not the
  submessages before it** (DDSI-RTPS 2.5 §8.3.4.1 rules 1, 2, 6). The
  whole datagram was dropped. A DATA or DATA_FRAG with an unrecognized
  must-understand ParameterId is skipped and parsing goes on (Table 9.6);
  in the HeaderExtension it still drops the message.
  `ParsedDatagram::invalid_rest` names the error. Tests:
  `the_submessages_before_an_invalid_one_stand`,
  `a_submessage_length_past_the_end_invalidates_the_rest`,
  `a_data_with_an_unknown_must_understand_pid_is_skipped`.

- **A TypeLookup request padded to four octets without its options word
  is answered.** InterCom DDS 4.3.1 pads a `getTypes` request that ends
  one octet short of four and leaves the options word at `00 00`, where
  XTypes 1.3 §7.6.3.1.2 has the writer count the padding. ZeroDDS read the
  octet as trailing data and dropped the request without an event;
  InterCom could not resolve the reader's type and matched it only in some
  runs (measured, OMG dds-rtps FinalInstanceState cases: 3 of 24 green
  before, `_0` and `_1` 8 of 8 after). Up to three zero octets that end the
  payload on a four-octet offset are padding now; a dropped request
  reports `typelookup.request.rejected`. Tests:
  `an_intercom_get_types_request_padded_without_its_options_is_read`,
  `only_zero_octets_up_to_the_next_four_octet_offset_are_padding`.

- **A dispose with `PID_KEY_HASH` alone no longer blocks the reads of a
  DataReader that does not know the instance.** It went into the cache
  without a key holder, and every `take` and `read` that selected it
  failed with `Unsupported`, whatever else the cache held (read from the
  code, not measured on the wire). The key is now read from the KeyHash
  where DDSI-RTPS 2.5 §9.6.4.8 Step 5.1 makes it the key's octets; where
  the KeyHash is an MD5 the marker is not taken in and
  `reader.lifecycle.unknown_instance` is reported, as Cyclone DDS drops
  it. Tests: `a_hash_only_marker_of_an_unknown_md5_instance_is_not_taken_in`,
  `a_hash_only_marker_of_a_known_instance_keeps_its_holder`,
  `a_hash_only_marker_of_an_unknown_short_key_reads_the_key_from_the_hash`.

- **An unregister of a disposed instance leaves it NOT_ALIVE_DISPOSED.**
  The DataReader brought the instance of a lifecycle marker back to ALIVE,
  with a disposed generation counted, before the marker applied, and an
  unregister after a dispose ended NOT_ALIVE_NO_WRITERS. DDS 1.4
  §2.2.2.5.1.3 makes an instance ALIVE when a writer writes it, §2.2.3.22
  ends it NOT_ALIVE_DISPOSED when the last writer disposed it. Measured
  with InterCom 4.3.1 as the publisher. Test:
  `an_unregister_after_a_dispose_leaves_the_instance_disposed`.

- **`wait_for_acknowledgments` waits for the reliable readers only**
  (DDS 1.4 §2.2.2.4.2.15). A best-effort reader, which sends no ACKNACK,
  was counted. Test:
  `all_samples_acknowledged_asks_the_reliable_readers_only`.

- **`wait_for_acknowledgments` no longer sleeps to its deadline after an
  ACKNACK it missed.** It waited on a bare notifier, and an ACKNACK
  between its check and the wait was lost; it waits on a generation now,
  which an ACKNACK and the loss of a reader move.

- **A RELIABLE reader of Cyclone DDS matched after a ZeroDDS writer had
  emptied its cache received nothing.** A VOLATILE writer removes what
  every reader acknowledged (since `93d61c1adcd`), and a HEARTBEAT took
  lastSN from the cache: 0 once it was empty, first 32 and last 0 after
  SN 31. DDSI-RTPS 2.5 §8.3.7.5 requires lastSN >= firstSN - 1; Cyclone
  11.0.1 drops such a message as malformed with the DATA in it. lastSN
  is the last SN written now. Measured with the OMG dds-rtps suite on
  the Linux test host, ZeroDDS publisher to Cyclone subscriber:
  `Test_DataRepresentation_0`/`_3`, `Test_Reliability_3`,
  `Test_History_0`, `Test_Ownership_0` and `Test_FinalInstanceState_0`/
  `_1` failed with DATA_NOT_RECEIVED on `aae4a255f82` and pass now;
  BEST_EFFORT was not affected. Test:
  `a_heartbeat_after_the_cache_emptied_announces_the_last_sn_written`.

- **`DomainParticipant::delete_contained_entities` deletes what it
  contains.** It emptied its lists of publishers and subscribers and left
  their writers and readers running. `contains_entity`,
  `Publisher::contains_writer` and `Subscriber::contains_reader` no
  longer report a writer or reader that is gone: the lists of handles
  they read were never emptied.

- **`on_data_on_readers` is called with the subscriber's handle**; it
  was called with the reader's.

- **A RELIABLE stream recovers from lost datagrams at round-trip speed.**
  A reader's ACKNACK names what it misses in a window of 256 sequence
  numbers from the first one missing. The writer sent a HEARTBEAT with
  its repair only when the set named 256 changes, so that the reader
  could ask for the next window; gaps spread over a stream leave a few
  changes in each window, and the next one waited for the periodic
  HEARTBEAT, 100 ms later. A set that spans the whole window
  (`numBits` 256) now gets one too. Measured on the Linux test host with
  `zerodds_perf`, RELIABLE, 1 kB, KEEP_ALL, four cells each in turn:
  before, 83–283 kS/s, and the cells in which the kernel dropped
  datagrams spent 2 to 7 of 15 seconds below 20 000 samples a second;
  after, 240–294 kS/s and no such second, with up to 53 661 datagrams
  dropped. Test:
  `a_nack_set_spanning_the_window_gets_a_heartbeat_with_its_repair`.

- **A take of the C API takes only what it selects.**
  `zerodds_dr_take_instance`, `zerodds_dr_take_next_instance` and
  `zerodds_dr_take_w_condition` took every sample and filtered what they
  handed out: the samples of the other instances, or in the other states,
  were gone. The read variants marked every sample READ, and
  `zerodds_dr_read`/`zerodds_dr_take` did not look at their state masks.
  The reader now reads or takes from its cache only what the masks and the
  instance select (DDS 1.4 §2.2.2.5.3.8, §2.2.2.5.3.14–17). Tests:
  `an_instance_read_or_take_leaves_the_other_instances`,
  `take_next_instance_takes_one_instance`.

- **`dispose` and `unregister_instance` return what sending their marker
  returned** (DDS 1.4 §2.2.2.4.2.7, §2.2.2.4.2.13: TIMEOUT,
  OUT_OF_RESOURCES). They returned OK, with the instance changed at the
  writer and no marker sent, when a reader's or the writer's room ran out.
  The marker goes first now; the instance changes once it went out. Test:
  `a_dispose_a_full_reader_holds_back_times_out`.

- **`read_next_instance` and `take_next_instance` return the next
  instance with samples, one instance** (DDS 1.4 §2.2.2.5.3.16): the
  smallest handle above `previous_handle` that has samples, also for a
  handle the reader does not manage. They went by the key hashes, stopped
  at an instance without samples, and under `ordered_access` above INSTANCE
  scope returned every instance's samples. Tests:
  `next_instance_passes_over_an_instance_without_samples`,
  `take_next_instance_returns_one_instance_under_ordered_access`.

- **EXCLUSIVE ownership changes with the owner's strength and its
  deadline** (DDS 1.4 §2.2.3.9.2 (b), (d)). An owner that lowered its
  strength refused its own samples; a strength announced over SEDP did not
  reach the reader's owners; an owner that missed the reader's DEADLINE
  kept its instances. Tests:
  `an_owner_that_lowers_its_strength_keeps_the_instance_until_a_stronger_one`,
  `an_owner_that_missed_its_deadline_gives_the_instance_up`.

- **A topic created twice goes with its second `delete_topic`** (DDS 1.4
  §2.2.2.2.1.11: "it must also be deleted that same number of times").
  The first `delete_topic` deleted it. Test:
  `a_topic_created_twice_goes_with_the_second_delete`.

- **PRECONDITION_NOT_MET where the spec names it** (DDS 1.4
  §2.2.2.2.1.8, .10, .28, .30, §2.2.2.4.2.7): `get_discovered_*_data` with
  an unknown handle, `delete_contentfilteredtopic`/`delete_multitopic` of
  another participant, and `dispose`/`unregister_instance` with the handle
  of another existing instance returned BAD_PARAMETER, in the C API too.

- **A timestamp with nanosec of 10^9 or more is BAD_PARAMETER** at the
  `*_w_timestamp` operations (DDS 1.4 §2.3.2); it went on the wire. The C
  API's `dispose_w_timestamp` and `unregister_instance_w_timestamp` dropped
  their timestamp and send it with the marker now. Tests:
  `a_timestamp_out_of_range_is_bad_parameter`,
  `dw_w_timestamp_calls_refuse_a_nanosec_out_of_range`.

- **A built-in reader's instance handle is that of the entity, and
  `ignore_*` unmatches** (DDS 1.4 §2.2.2.2.1.14–17, §2.2.2.4.2.24). The
  built-in readers numbered their instances 1, 2, …, so the handle from a
  `DCPSParticipant` sample ignored nothing and matched no
  `get_matched_*` handle; it is `InstanceHandle::from_guid` of the entity's
  GUID now. `ignore_participant`, `ignore_topic`, `ignore_publication` and
  `ignore_subscription` unmatch what was matched before the call, and a
  match asks whether the endpoint's Topic is ignored. Tests:
  `ignore_entities.rs`.

- **A Publisher's or Subscriber's `set_qos` that fails changes nothing**
  (DDS 1.4 §2.2.2.1.1.1). ENTITY_FACTORY was set before the PARTITION check
  of the endpoints and stayed set when it refused.

- **A lifecycle marker names its writer and carries its source time**
  (DDS 1.4 §2.2.2.5.5, §2.2.2.4.2.11). Its SampleInfo had HANDLE_NIL and
  the reader's clock. Test:
  `a_dispose_marker_names_its_writer_and_its_source_time`.

- **Two partition names that both hold wildcards never match** (DDS 1.4
  §2.2.3.13). `p*` matched `p*` and `p?`, since the literal string `p*`
  fits the pattern `p*`. Test: `two_wildcard_names_never_match`.

- **A deadline or liveliness lease of 0 is the shortest in the RxO
  comparison** (DDS 1.4 §2.2.3.7, §2.2.3.11). The match compared the
  nanoseconds the runtime keeps for its monitoring, where 0 stands for
  INFINITE: a reader that requested 0 accepted every writer. Test:
  `a_request_of_zero_is_met_by_an_offer_of_zero_only`.

- **INCONSISTENT_TOPIC counts each discovered Topic once** (DDS 1.4
  §2.2.4.1: "Total cumulative count of the Topics discovered"). Every
  refused pair of a local and a remote endpoint counted; a remote
  participant's Topic of the name counts once now, until the participant
  leaves. A local `create_topic` of a name with another type tells the
  listener dispatcher at once. Test:
  `an_inconsistent_topic_counts_once_per_remote_participant`.

- **The receive path runs the data listener before it wakes the
  WaitSets** (DDS 1.4 §2.2.4.6: "the listener mechanism is used first and
  then the WaitSet objects are signalled"). It woke them first. Test:
  `the_listener_runs_before_the_waitset_is_signalled`.

- **The built-in Subscriber presents by TOPIC** (DDS 1.4 §2.2.5:
  "PRESENTATION access_scope = TOPIC"). It had INSTANCE. Test:
  `the_builtin_subscriber_presents_by_topic`.

- **SAMPLE_LOST counts only samples that never arrived** (DDS 1.4
  §2.2.4.1: "A sample has been lost (never received)"). A BEST_EFFORT
  reader counted the received samples it discarded for room under
  `max_samples` or `max_samples_per_instance`; they change no status now,
  as under KEEP_LAST (§2.2.3.18 KEEP_ALL: "the old values will be
  discarded"). The sample it refuses for `max_instances` is SAMPLE_REJECTED
  with REJECTED_BY_INSTANCES_LIMIT, as on a RELIABLE reader, in Rust and in
  the C API. RTI Connext reports the discarded ones as lost. Tests:
  `keep_all_max_samples_per_instance_discards_the_oldest_when_best_effort`,
  `max_instances_rejects_when_best_effort`.

- **SAMPLE_LOST counts the samples a reader never gets** (DDS 1.4
  §2.2.4.1). A BEST_EFFORT reader that stepped over a gap and a RELIABLE
  reader whose writer's HEARTBEAT `firstSN` passed changes it never
  received counted nothing: SAMPLE_LOST held only what a departed writer
  left missing, and a sample lost in transit showed in no status. What
  came before the first sample a reader took from a writer is not
  counted; a reader that joined late was never sent it. Tests:
  `a_best_effort_reader_counts_the_samples_a_gap_lost`,
  `a_reliable_reader_counts_the_changes_its_writer_dropped`,
  `samples_a_best_effort_reader_steps_over_are_sample_lost`.

- **A PAD or INFO_TS with `octetsToNextHeader` 0 does not end the
  message.** DDSI-RTPS 2.5 §9.4.5.1.3 makes a zero length mean "to the
  end of the message" for every submessage "other than PAD or INFO_TS".
  An INFO_TS with the invalidate flag has no body; read as the last
  submessage, everything after it in the datagram was taken for its body
  and lost. Test:
  `an_info_ts_without_a_body_is_not_the_last_submessage`.

- **A DATA whose inline QoS holds a PID_IGNORE is read from the right
  octets.** The length of the inline QoS was taken from the parsed list
  encoded again, which leaves out the skipped PID_IGNORE parameter
  (DDSI-RTPS 2.5 §9.6.2.2.1): the serializedPayload started that many
  octets early and the sample decoded from the wrong octets. The parser's
  own count is used now, as DATA_FRAG already did.
  Test: `a_pid_ignore_in_the_inline_qos_does_not_move_the_payload`.

- **A live DataReader's `on_data_available` is called when samples
  arrive.** It was called only from `DataReader::wait_for_data`, when the
  application itself waited; a reader with a listener and no wait heard
  nothing, and no test covered it — the listener tests run offline and
  push samples by hand. Measured on the Linux test host, `zerodds_perf
  sub` taking in its listener read 957 samples of a 13 million sample
  stream. The receive path now signals each run of samples it hands a
  reader whose chain has a DATA_AVAILABLE or DATA_ON_READERS listener, and
  the listener dispatcher's thread calls the listener in a pass of its own
  that runs no other status poll (DDS 1.4 §2.2.4.1). A listener set after
  samples arrived hears of them. `wait_for_data` no longer calls the
  listener too, and blocked WaitSets are told once per run instead of once
  per sample. Tests: `crates/dcps/tests/data_available_live.rs`.

- **SPDP multicast went out, and was heard, on one interface only.** The
  SPDP group was joined, and SPDP sent, on the interface the kernel picks by
  route; on a host with a LAN and a container bridge that was the LAN.
  Measured on the CI runner, domain 231, one participant in a container on
  the default bridge and one in a container on the host network: 0 samples
  in either direction, where Fast DDS (`rmw_fastrtps_cpp`, Rolling) found
  its peer both ways; with the host side pinned to the bridge
  (`ZERODDS_INTERFACE`), 2 492 and 680. The group is now joined on every
  eligible interface and SPDP goes out through one sender per interface
  (`IP_MULTICAST_IF`); a join or a sender that fails is skipped, the event
  `discovery.spdp_multicast.interfaces` names what was joined, and a pinned
  `transport.interface` keeps its one interface. Same measurement after:
  5 749 and 468. No multicast arrives between the CI runner and the Linux
  test host with either build, so the LAN direction rests on
  `udp_transport::tests::a_group_joined_on_every_interface_hears_a_sender_on_each`
  and not on a run between hosts.

- **A RELIABLE reader lost the samples it held for want of room when their
  writer left.** A sample past the DataReader's room stays in the RTPS
  reader's receive cache, received and acknowledged; removing the writer's
  proxy removed that cache. A writer process that ended after
  `wait_for_acknowledgments` returned therefore took up to a receive cache
  of samples with it: `zerodds_perf`, RELIABLE KEEP_ALL, `max_samples`
  100 000, a reader that takes every 3 s, 10 s of writing on the Linux test host —
  400 001 of 485 249 samples arrived. `ReliableReader::depart_writer_proxy`
  keeps the departed writer's receive state until the consumer has taken
  what it holds; a change missing in front of a held sample can no longer
  arrive and counts as SAMPLE_LOST (DDS 1.4 §2.2.4.1), and the DataReader
  learns of the departure (§2.2.2.5.1, NOT_ALIVE_NO_WRITERS) after the last
  held sample. Same run since: 485 761 of 485 761, 777 966 of 777 966 with
  20 s and 1 029 669 of 1 029 669 with 30 s of writing.
  `reliable_e2e::e2e_a_slow_consumer_gets_every_sample_in_order` takes
  27 000 of 30 000 with the old removal and all with the new one.

- **After a KEEP_ALL reader overflowed, what it had not acknowledged came
  back at about 2 560 samples a second.** The writer repairs only what an
  ACKNACK names, at most one `SequenceNumberSet` of 256 (DDSI-RTPS 2.5
  §9.4.2.6), and the reader asks again only in response to a HEARTBEAT
  (§8.4.2.3), 10 a second: measured 7 680 samples per 3 s take. A repair of
  a full set now carries a HEARTBEAT, so the next set is asked for a round
  trip later; a reader whose receive cache is full of samples the consumer
  has not taken holds its ACKNACK back until it takes (§8.4.2.3, "only if
  the Reader can accommodate these missing samples in its cache") and then
  names no more than it has room for. Before, 30 s of writing delivered
  455 690 of 503 835 samples in 67 s; now each 3 s take is a full 100 000.

- **A full receive cache refused the change that would have let it drain.**
  A KEEP_ALL receive cache full of changes behind a missing one refused the
  missing one when it was repaired, so nothing behind it was delivered
  again. The cache now gives up its highest change, which is MISSING again
  and asked for later
  (`reliable_reader::tests::a_full_cache_takes_the_change_in_front_of_what_it_holds`).

- **A status read while the listener thread looked at it could stay
  flagged.** The twelve `get_*_status` getters cleared the
  StatusChangedFlag and then took the status; the listener thread's look
  could see the status unread before the take and set the flag after the
  clear (DDS 1.4 §2.2.4.1 resets it on the read). Since `7aff265c730`
  `status_reads::a_status_change_raises_the_flag_and_the_read_lowers_it`
  failed in 6 of 80 runs on the Linux test host and failed the nightly of 2026-10-07;
  0 of 80 before. The getters now take and then clear, and a look checks
  the status again after it sets the flag: 0 failing tests in 200 runs of
  the whole binary, against 10 in 200 before.

- **The IDL load of the generated wire probes refused a declaration named
  like its scope in every dialect.** `wireprobe-zerodds-gen`'s copy of the
  lowering call did not pass `name_of_the_enclosing_scope`, so Fast DDS,
  RTI and both ROS dialects, which take `resolve-module-named-as-type`
  (IDL 4.2 §7.5.2), were answered with a refusal; measured as 140 slots
  of the C++ column against the Rust column's octets. The Ada and Ada 83
  copies had the same gap and also threw away the resolve report, which
  a reference that resolves to nothing passes silently; both now refuse
  an unresolved model as the shared copy does.
- **The spatial bridge forwarded a sample to nobody when its writer had
  not been discovered yet.** Both bridge writers were RELIABLE,
  KEEP_LAST(10), VOLATILE, so a pose the pump converted before the writer
  on the far side had a matched reader was gone: a VOLATILE writer keeps
  no sample for a reader that matches later (DDS 1.4 §2.2.3.4). Measured
  against a ROS 2 Jazzy node with `rmw_zerodds_cpp` (Linux test host, the
  `rmw-spatial-bridge` example): `ros_to_dds`, one run of three delivered
  59 of 60 poses with the first missing; `dds_to_ros`, one run of 13
  delivered 57 of 58, again without the first. Both writers offer TRANSIENT_LOCAL now, with the
  KEEP_LAST(10) they had, so a reader joining late is served the last ten.
  The bridge's readers stay VOLATILE: a reader requesting TRANSIENT_LOCAL
  matches no VOLATILE publisher (§2.2.3, table "QoS compatibility",
  DURABILITY RxO), and ROS publishers are VOLATILE unless the application
  says otherwise. Both directions in-process, with the VOLATILE case that
  keeps the TRANSIENT_LOCAL one honest
  (`crates/rmw-zerodds-shim/tests/spatial_bridge_e2e.rs`).
- **A VOLATILE reader was served samples written before it matched.** The
  late-joiner history was suppressed on the writer's own durability alone,
  on both delivery paths: a reader in the writer's own runtime got every
  retained sample of a TRANSIENT_LOCAL writer whatever it had requested,
  and over the wire the same held for a remote reader — a ROS subscription
  on its default profile included. A VOLATILE reader never receives
  samples written before it matched (DDS 1.4 §2.2.3.4). The pair still
  matches (offered TRANSIENT_LOCAL satisfies a requested VOLATILE,
  §2.2.3 table "QoS compatibility") and the samples written from the
  match on still arrive; only the history is declined now, decided on the
  reader's requested durability on both paths
  (`crates/dcps/tests/transient_local_durability.rs`).

- **A full RELIABLE KEEP_ALL reader holds its writer back.** A DataReader
  whose RESOURCE_LIMITS bound it took every sample the runtime received
  onto its channel, and RTPS acknowledged each at once, so a KEEP_ALL
  writer with finite RESOURCE_LIMITS never blocked and the reader held
  whatever the writer sent (DDS 1.4 §2.2.3.14, §2.2.2.4.2.11). The runtime
  now hands such a reader no more than its limit and one sample, which the
  reader refuses and reports in SAMPLE_REJECTED. The rest waits in the
  RTPS receive cache, capped at the reader's limit per writer, and what
  does not fit there is not acknowledged. A writer in the reader's own
  runtime waits for the reader's room as it waits for its own. Measured
  with a reader in a second participant, `max_samples` 4, writer
  `max_samples` 8: the writer's write times out after at most 64 samples
  and every sample it placed arrives, in order
  (`crates/dcps/tests/reader_room_backpressure.rs`). With `zerodds_perf`,
  `max_samples` 100 000 on both sides, a reader that takes every 3 s and
  30 s of writing: before, the writer wrote 2 097 805 samples without
  blocking and the reader's resident set peaked at 1.61 GB; now the writer
  waits and writes 648 629, the reader peaks at 393 MB, and every sample
  arrives in both. Throughput with a reader that keeps up is unchanged:
  68.8 kS/s median on the Linux test host (69–70 before).
- **The Rust codec writes a member id past 16 bits whole.** The plan's
  16-bit parameter id saturates at `u16::MAX` for such a member, and
  `rust_codec` handed it to `write_pid_extended`: a `@hashid` member of a
  PL_CDR1 type went out as member 65535 (XTypes 1.3 §7.4.1.2.2). The
  extended header takes `member_id` now. Six corpus cases change their
  generated Rust under `--encoding pl-cdr1`.
- **Rust's codec under `--dialect rti` frames a nested `@mutable` type by
  the sample it is in**, as the 20 languages on the shared walk do since
  `9243765fe90`: each encoder body takes the sample's decision and the
  public `encode` resolves it from its root (`LayoutKey::for_sample`). Over
  the 421-file corpus the generated Rust is byte-identical under
  `--dialect cyclone` and `fastdds`; under `rti` with `pl-cdr1` 420 files
  gain the per-sample body and no decoder changes. Held by two compiled
  and run tests against the reflective encoder and decoder
  (`tools/idlc/tests/dialect_byte_equality.rs`).
- **A wide gap no longer stops the RELIABLE repair.** An ACKNACK's
  `SequenceNumberSet` took the span of the missing list as `numBits`; with
  received samples between the missing ones it reached 1168 bits, and every
  receiver, this one included, refuses more than 256 (DDSI-RTPS 2.5
  §9.4.2.6). The writer never learned what was missing: with the reader
  under load, 24 603 of 120 393 samples arrived and the following 20 s held
  HEARTBEAT/ACKNACK pairs and no repair. The set is cut at 256 bits now, as
  the fragment set already was; under the same load 366 956 of 366 956 and
  418 806 of 418 806 arrived. This was the loss the throughput gate's CI
  runs reported. `zerodds_perf`'s writer now also waits for its
  acknowledgments before it leaves, and the gate's reader runs 30 s past it.
- **Generated codecs under `--dialect rti` frame a nested `@mutable`
  type by the sample it is in**, the short PL_CDR1 header below RTI's step
  and `PID_EXTENDED` above it, in the 20 languages on the shared walk
  (`crates/lang-rules/src/emit/encode.rs`). Held by a generated-C test
  against the reflective encoder and by the three-column gate, whose two
  recursive-case exemptions are retired. Rust's `rust_codec` still writes
  the short form.
- **The full three-column tier runs in CI** (`three-column-gate-full`,
  444 s, green) and reads its matrix back.
- **A RELIABLE sample RESOURCE_LIMITS refuses is held back, not
  dropped.** The throughput gate's first CI run lost 60 098 of 904 814
  samples under RELIABLE KEEP_ALL. The reader drains its channel on
  `take`; what the cache could not admit was counted in SAMPLE_REJECTED
  and dropped after the RTPS layer had acknowledged it, so no writer
  sent it again. It now waits, with everything after it and in arrival
  order, for the next ingest (DDS 1.4 §2.2.4.1: "a rejected sample may
  be accepted in the future"). Measured with a reader that takes every
  3 s and `max_samples` 100 000: before 300 000 of 680 100 samples
  arrived, now 523 273 of 523 273.
- **Every endpoint SDK writes `double`, DHEADER-framed and `@mutable`
  samples as the Rust column does.** Measured with
  `wireprobe-endpoint-corpus` over `primitive-mixed`, `struct-appendable`
  and `struct-mutable` in all ten configurations: 30 of 30 cells the same
  in each of the 17 SDKs. Four had not built the generated program — the
  generator spelled `Pid`, `emheaderBegin`, the length code and Swift's
  `pidBegin` labels otherwise than kotlin, nim, swift and zig declare
  them.
- **`transport.same_host_shm` follows the build.** It defaulted to `true`
  in every build, and `zerodds-dcps` does not carry `same-host-shm` by
  default: such a build sent every same-host sample over UDP under a
  setting that said shared memory. Measured, two processes, 1 kB RELIABLE
  for 5 s: without the feature 301 677 samples in 301 794 UDP datagrams,
  with it 512 946 samples and 1 033 datagrams (the ring-full fallbacks
  `same_host_shm_udp_fallbacks` counts). The default is now the feature,
  and `true` without it makes `DcpsRuntime::start` refuse.
- **`zerodds-rtps` builds without `std` again.** The DCPSTopic sample's
  module lacked the `alloc` gate its neighbours carry
  (`tools/no-std-claims-gate.sh`, `thumbv7em-none-eabihf`).
- **The rmw package script finds the repository root** (it read
  `packaging/Cargo.toml`), the staged-tree guard no longer reads a quoted
  word in a comment of the member list as a member, and `xrce-agent`'s
  test-only import moved into the tests.

- **A participant whose `PID_DOMAIN_ID` names another domain is not
  discovered.** `spdp_announced` compared the domain tag and never the
  domain id it parsed, so a beacon of another domain that reached a
  socket was taken: the participant answered it and sent SEDP to that
  domain's ports, as in the recorded capture of a domain-122 participant
  writing to 7416. Cyclone DDS 11.0.1 ignores such a participant the
  same way (`ddsi_discovery_spdp.c:573`); an absent `PID_DOMAIN_ID` is
  the receiver's own domain. `DcpsRuntime::ignored_spdp_samples` counts
  what was ignored for another domain id or tag. How the foreign beacon
  reached the socket is not reproduced.
- **A status change reaches its listener once.** The listener thread and
  a `get_*_status` caller both read the StatusChangedFlag before either
  acknowledged it and both called the listener:
  `on_sample_rejected_listener_fires` saw two calls for one rejection in
  4 of 8 runs on the Linux test host. One look at a status runs at a time
  per entity now, and a change that arrives during a callback is handed
  over after it. 100 runs of both listener tests, 0 failed.
- **A `get_*_status` between a listener's look and its acknowledgement
  is not taken off twice.** The read reset the change fields and the
  acknowledgement subtracted the reported changes again, leaving
  `total_count_change` at -1, which is unread and handed the listener a
  second, negative change. `StatusLedger` counts its reads and an
  acknowledgement takes nothing off when a read came between
  (`a_read_between_the_look_and_the_acknowledgement_is_not_taken_off_twice`).
- **The HDDS dump refuses a type handle that does not resolve** instead
  of skipping it, and its tests read the field forms the emitter writes.
- **The ten endpoint renderers hand `put_f32` the corpus decimal**, not a
  value rounded through `f64` first (`wireprobe-endpoint-gen`,
  `surface_and_renderers_agree`).

- **Two gates against classes of defects.** The silent-drop guard
  watches every crate for a list or buffer cut to the caller's capacity
  (`capacity_clip`). The nightly runs the C# binding's smoke, which ran in
  no pipeline and was red on main (its status check read a reader that a
  match could make live), and `tools/perf-gate/run.sh`, a RELIABLE KEEP_ALL
  throughput cell that fails on a lost sample and below a floor.

- **A RELIABLE writer with RESOURCE_LIMITS walked its whole history on
  every write.** `HistoryCache::instance_len`, asked by the RESOURCE_LIMITS
  check since fc3b5ae4ba3, and the KEEP_LAST per-instance eviction counted
  an instance by scanning every change. The cache keeps an index per
  instance now. Bisected and measured on the Linux test host with
  `zerodds_perf`, RELIABLE KEEP_ALL, 1 kB: 72 kS/s before that commit, 42
  after it, 67–69 with the index; no sample lost in any run.
- **DDS-XRCE `CREATE_CLIENT` carries the typed `CLIENT_Representation`**
  in `crates/xrce`, not opaque bytes.

- **The C API reports the total length of every list it copies.** Seven
  functions wrote `min(len, cap)` entries and set `*out_count` to that, so a
  caller with too small a buffer silently got a shorter list. They now write
  `min(len, cap)` entries, set `*out_count` to the total and return
  `ZERODDS_OUT_OF_RESOURCES` when `len > cap`; `cap == 0` with a NULL buffer
  asks for the length (`docs/specs/zerodds-c-api-1.0.md` §2.8). The C++ and
  C# callers retry with the reported total.

- **An SEDP sample that does not decode no longer takes its datagram's
  other events with it.** `handle_sedp_datagram` dropped every event of a
  datagram in which one publication, subscription or topic sample failed
  to decode, without a word, and the reliable reader had already taken
  those samples. Each such sample is now refused on its own
  (`SedpEvents::refused`, by writer GUID) and counted
  (`DcpsRuntime::refused_sedp_samples`); a datagram that is not RTPS at
  all is counted in `refused_sedp_datagrams`.

- **The annotation TypeObjects are the ones DDS-XTypes 1.3 Annex B
  declares.** `MinimalAnnotationType`/`CompleteAnnotationType` carried a
  `member_id` Annex B does not have, the full name where the minimal form
  has a `name_hash`, a `CompleteTypeDetail` where the complete header has
  `annotation_name` alone, the default value as length-prefixed bytes, no
  `DHEADER` on the appendable parameter and header structs, and the
  parameters in declaration order instead of by name hash / name. The
  `EquivalenceHash` of an annotation type changes with it; no other kind's.

- **A user-declared annotation reaches the `TypeObject`
  (DDS-XTypes 1.3 §7.3.4.3, Annex B).** An application of an
  `@annotation` in a file lowered to nothing on the wire: `from_ast`
  wrote `ann_custom` absent wherever one appeared, so a `@Unit("m")` a
  reader was told nothing about, and `to_ast` read none back. Both halves
  are in place now. `from_ast` writes an `AppliedAnnotation` whose
  `annotation_typeid` is the `EK_COMPLETE` hash of that declaration's own
  `CompleteTypeObject` (Table 29's `TK_ANNOTATION` row, which Annex B's
  `TypeIdentifier` union has no case for — `docs/omg-xtypes-issues.md` §B9),
  with one `AppliedAnnotationParameter` per parameter given, each named by
  its `NameHash` and holding the value in its declared kind, both sequences
  in the order §7.3.4.5 requires. `to_ast` looks the hash up in the
  registry, rebuilds the declaration from the `CompleteAnnotationType`
  there — its qualified name, its parameters, their defaults — and puts
  the application back on `Facets::annotations`. Every position that has
  an `ann_custom` is read: a `CompleteTypeDetail`, a
  `CompleteMemberDetail`, a `CompleteDiscriminatorMember`, a
  `CompleteAliasBody`, a `CompleteCollectionElement`, an enumeration
  literal, a bitmask flag and a bitfield.

  **BREAKING:** the `COMPLETE` hash of any type or member that carries an
  applied annotation changes, because `ann_custom` is in the object it is
  hashed over. §7.3.4.4 leaves user-defined annotations out of the
  `MINIMAL` form, so no minimal hash changes and assignability is
  unaffected.

  `register_all` files every `@annotation` declaration's two objects under
  their hashes, and `dependency_type_objects` returns the declarations a
  type's closure applies — an `annotation_typeid` **is** a hash reference,
  and an endpoint that served the annotated type without the annotation
  would announce a dependency it cannot answer for (§7.6.3.3.4). An
  annotation the file does not declare is still ignored, which is IDL 4.2
  §7.4.15.4.2's own instruction: there is no declaration to hash. A
  declaration the model cannot build — a `wchar` parameter has no
  `AnnotationParameterValue` case — is refused by name rather than
  producing a member with the application quietly missing.
- **IDL 4.2: the 26 constructs both code generators emitted and no
  committed `.idl` contained have a fixture each**
  (`tools/idlc/tests/idl-4.2-gaps/`, swept by
  `tools/idlc/tests/idl_4_2_gaps.rs`); section D of the gap page is empty.

- **The c, cpp, java, python, rust and typescript endpoint SDKs refuse a
  sample the XRCE WRITE_DATA frame cannot describe.** Its
  `submessage_length` is sixteen bits; a body (`BaseObjectRequest` plus
  sample) above 0xFFFF was framed with a truncated length (cpp, python),
  saturated to 0xFFFF (rust `reliable`) or panicked (rust). Each now
  refuses — `std::length_error`, `ValueError`, `FrameError::BodyTooLarge` —
  and no caller hands a refused frame to the transport. The other fifteen
  SDKs did so already.

- **`--dialect rti` writes RTI's PL_CDR1 header form in a large sample**
  (`08ae1d7161f`). RTI Connext 7.7 gives `PID_EXTENDED` to every member
  whose type is not a primitive or an enumeration once the sample's type
  can exceed what a short parameter header states — nested types and
  `@optional` members included, a recursive type always. Measured on
  rtiddsgen's type support at six steps; the reflective encoder and
  decoder decide the form for the sample's type
  (`LayoutKey::for_sample`). `--dialect rti` now reproduces all 1090 rows
  RTI wrote in the interop corpus. Generated encoders under
  `--dialect rti` still write the short form there.

- **CORBA: a ZeroDDS server's `wstring` and CosNaming's exceptions reach
  omniORB, TAO and JacORB.** A generated skeleton writes a reply's
  `wstring` with its byte-order mark (omniORB and TAO read the unmarked
  little-endian units wrong). An exception, type or constant declared in
  an interface or value type is named through it, so
  `CosNaming::NamingContext::NotFound` has the repository id the three
  ORBs raise and expect. **Migration:** a declaration in an interface or
  value type is found as `Scope::Interface::Name`, no longer as
  `Scope::Name`; an AST snapshot is format 4.

- **A reliable writer with nothing written answers a reader that keeps
  asking.** An OpenDDS reader stayed unmatched for 25 s in 2 of 30 runs:
  the writer's one handshake HEARTBEAT reached it before it held the
  writer, and its preemptive ACKNACK closed the handshake. An ACKNACK a
  heartbeat period after the last HEARTBEAT reopens it (`9c1d99a0a42`);
  60 of 60 runs matched.

- **The merge pipeline checks what a long push changed.** A push of more
  commits than the clone depth left the diff base outside the clone, and
  the lane planned the push as no change; it fetches the base now.

- **`zerodds_xcdr2.h` and `zerodds.h` compile in one translation unit.**
  The typed-dispatch header declared the typed functions a second time
  over a struct of its own; it names `zerodds.h`'s `zerodds_ZeroDdsTypeSupport`
  as `zerodds_typesupport_t` now and adds the `ZERODDS_TS_*` status values
  and the representation byte's `ZERODDS_TS_REPR_BIG_ENDIAN` bit.
  **Migration (C):** code that defined a table as `struct
  zerodds_typesupport_s` names `zerodds_typesupport_t` instead.

- **The typed C path keeps the byte order, the key and the framing.**
  `zerodds_reader_take_typed` read a big-endian sample as little-endian;
  it hands `decode_repr` the order, and a table with only `decode` answers
  UNSUPPORTED for one. `zerodds_writer_write_typed` wrote a keyed type
  without its KeyHash, and under the writer's default FINAL framing; it
  writes the KeyHash and sets the framing from the table.

- **A status listener hears a change made before it was set.** A match
  that discovery made before `set_listener` was consumed by the listener
  path with no listener attached and reached none; a change stays unread
  until a listener or a `get_*_status` takes it, and `set_listener` runs
  the listeners once (`01565bd5896`, `19f87ff5151`). An acknowledgement no
  longer resets the StatusChangedFlag over a change made after the
  listener's look.

- **An older SEDP announcement no longer overwrites a newer one from a
  peer in the same process** (`4553fd2598c`, `5cc7b7483a6`). A reader's
  changed QoS came back as the old one under load, and its writer matched
  it again.

- **A model node's source range is a range of its file**
  (`3f20f7a41d2`). A node lowered from a preprocessed file held an offset
  into the parser's tree, which omits the expanded text's token
  separators; lint findings and diagnostics raised on the model pointed
  at the wrong declaration. `Parse::source_span` is new.

- **A Rust union's enumeration discriminator crosses the wire through
  the `EnumeratorValue` axis** (`941c83666e3`); it wrote and read the
  declared value under every setting.

- **TRANSPORT_PRIORITY reaches the wire** (DDS 1.4 §2.2.3.15). A
  writer's datagrams go out with the value as the IPv4 TOS octet or the
  IPv6 Traffic Class (1 to 255), a reader that expects in-line QoS gets
  `PID_TRANSPORT_PRIORITY`, and `set_qos` changes it; the value stayed in
  the DCPS QoS before. `Transport` has `send_with_priority`.

- **The C API's QoS reaches the runtime.** `zerodds_dw_set_qos` and
  `zerodds_dr_set_qos` stored the QoS in the handle and stopped; they
  announce a changeable policy and refuse an immutable one with
  `ZERODDS_IMMUTABLE_POLICY`. A C writer's LATENCY_BUDGET and
  DESTINATION_ORDER and a C reader's TIME_BASED_FILTER, LATENCY_BUDGET,
  DESTINATION_ORDER and TYPE_CONSISTENCY_ENFORCEMENT were dropped at the
  creation; a C reader's TIME_BASED_FILTER filters now. **Migration (C):**
  a `set_qos` that changes an immutable policy fails; set it at the
  creation.

- **A duration rounds to the nearest nanosecond.** Every conversion
  between the 2^-32 s fraction of `Duration_t`/`Time_t` and nanoseconds
  cut: a C `set_qos` of 300 ms read back as 299 999 999 ns, a written
  source timestamp came back one nanosecond early, and the AMQP, CORBA,
  gRPC and WebSocket bridges translated a 300 ms deadline as 299 ms.
  `zerodds_qos::Duration` has `from_sec_nanosec`, `to_sec_nanosec`,
  `fraction_of_nanosec` and `to_millis`, all rounding; `from_millis`,
  `from_micros` and `to_nanos` round as well, and every whole nanosecond
  survives the way there and back.

- **DEADLINE is watched per instance** (DDS 1.4 §2.2.3.7, §2.2.4.1). A
  writer of two instances that updated one missed nothing, and a reader
  that never received a sample missed deadlines from its creation; now
  each instance has its period, an instance leaves with its dispose or
  unregister, and OFFERED_/REQUESTED_DEADLINE_MISSED name the instance in
  `last_instance_handle` (Rust and C), which stayed NIL.

- **`wait_for_historical_data` waits for the history** (DDS 1.4
  §2.2.2.5.3). `DataReader::wait_for_historical_data` is new; it and
  `zerodds_dr_wait_for_historical_data` block until every reliable
  matched writer's announced history has arrived, or answer TIMEOUT. The
  C function answered OK at once for every reader.

- **A StatusCondition triggers on what changed** (DDS 1.4 §2.2.2.1.6,
  §2.2.4.1). In Rust DATA_AVAILABLE and DATA_ON_READERS are up while a
  reader holds data not read yet; no flag was set for them. In C the
  StatusCondition answered `enabled_statuses != 0`, so a WaitSet holding
  one returned at once; it now reads the flags of its writer, reader,
  topic or subscriber.

- **The communication statuses can be read, and a read resets them**
  (DDS 1.4 §2.2.2.4.2, §2.2.2.5.3, §2.2.4.1). `DataWriter` and
  `DataReader` have the ten `get_*_status` operations, `Topic` has
  `get_inconsistent_topic_status`; a read or a listener call takes the
  change fields and resets the StatusChangedFlag, which no status set
  before, so a StatusCondition never triggered on one. PUBLICATION_ and
  SUBSCRIPTION_MATCHED count every endpoint that ever matched and name
  the last that joined or left; `total_count` was the larger of the last
  and the current count. The C API wrote the match count into both counts,
  0 into every change field and NIL into every handle, and zeros for
  INCONSISTENT_TOPIC; the Python bindings did the same. Both hand the
  real record through. `DataWriter::get_publisher` and
  `DataReader::get_subscriber` are new. **Migration (Rust):**
  `get_liveliness_changed_status` returns a `Result` (NOT_ENABLED before
  `enable`). **Migration (Python):** the status methods raise on a
  disabled entity.

- **A writer's matched subscriptions and a reader's matched publications
  have their data** (DDS 1.4 §2.2.2.4.2, §2.2.2.5.3). Rust had the count
  alone; `DataWriter::matched_subscriptions`/`matched_subscription_data`
  and `DataReader::matched_publications`/`matched_publication_data` are
  new, and C++ has `dds::pub::matched_subscription_data` and
  `dds::sub::matched_publication_data` with `BuiltinTopic.hpp`'s classes
  holding every policy. The C API looked a handle up in the builtin
  reader, which holds discovered endpoints only: a handle
  `get_matched_subscriptions` listed for a reader of the same participant
  answered NO_DATA, and a discovered reader the writer was not matched
  with answered with data. A handle with no match is BAD_PARAMETER.
  **Migration (C):** test for `ZERODDS_BAD_PARAMETER` where the code
  tested for `ZERODDS_NO_DATA`.

- **A model node names the file it was written in.** Its `SourceFacet`
  held a range of the expanded text under the unit's path and no macro
  chain; a reader of a node's source pointed into the wrong file or line.
  The composer now gives each node its file, the range in that file and
  the macros that wrote it (`MacroExpansion::path` is new), and an
  unresolved name is reported where it is written instead of at offset 0.

- **`zerodds-idlc format` lays out a file with a function-like macro
  call.** It refused one: it checked the source before the preprocessor,
  where `TWICE(3)` is no IDL syntax, while `check` accepted the file after
  expansion. `format` now decides validity as `check` does and lays the
  text out over the source's tokens; the call stays as written.

- **A Rust union keeps its discriminator.** The Rust column's union enum
  held one variant per case and no discriminator: a value read under the
  second label of a case, or under any value the `default:` case takes,
  went out again under the first label or under a value of the column's
  choosing; a discriminator selecting no member had no value and was
  refused on read; and a `boolean` or `char` discriminator was written as
  an integer (`w.write_bool(1)`, `w.write_char(97)`), which `rustc`
  refuses. A variant whose case does not fix the discriminator carries it
  ahead of the member, `_NoMember(<discriminator>)` holds one that selects
  no member, and an encode refuses a pairing the type does not allow.
  **Migration (Rust):** such variants take the discriminator as their
  first field, `U::N(7, value)`; a union without `default:` has the extra
  variant.

- **An entity created disabled is not discovered before its `enable`**
  (`a72a554f2a5`, `312e9a97949`). DDS 1.4 §2.2.2.1.1.7: a DataWriter or
  DataReader is registered with the runtime and announced at its first
  `enable`, with the QoS it has then; a participant its factory created
  disabled sends no SPDP before. `enable` with a disabled factory returns
  PRECONDITION_NOT_MET, a factory's `enable` enables what it created when
  its ENTITY_FACTORY says so, and data operations of a disabled endpoint
  return NOT_ENABLED. A participant's `set_qos` of USER_DATA goes out in
  the SPDP beacon.

- **The DCPSPublication and DCPSSubscription samples carry every policy**
  (`fe3fa5c5a4e`, `c34d466e5db`). DDS 1.4 §2.2.5: each member as its QoS
  policy, in Rust and in the C API's matched publication and subscription
  data; they held four kinds and three durations in whole seconds, and no
  USER_DATA, TOPIC_DATA, GROUP_DATA, LATENCY_BUDGET, DESTINATION_ORDER or
  PRESENTATION. **Migration (C):** the struct members changed; release a
  filled struct with `zerodds_publication_builtin_topic_data_free` or
  `zerodds_subscription_builtin_topic_data_free`, which frees the names
  as well.

- **A subscription announces its TIME_BASED_FILTER** (`e1041b4e842`),
  `PID_TIME_BASED_FILTER` in SEDP (DDS 1.4 §2.2.5).

- **`ros2 topic info -v` shows durations as set** (`2017519c67f`). The
  endpoint info carried whole seconds: a 150 ms deadline read as 0 s and
  a remote INFINITE as 2147483647 s.

- **A `set_qos` once enabled is announced and decides the pairs again**
  (`591e3e38505`). DataWriter, DataReader, Publisher, Subscriber and
  Topic hand the changeable policies (DDS 1.4 §2.2.3) to the runtime;
  the endpoint announces again, and a matched pair is held to the
  latest announcement of either side by the rules of a first match. A
  Topic's TOPIC_DATA reaches the endpoints that set none of their own.
  A matched writer's new OWNERSHIP_STRENGTH and LIFESPAN reach the
  reader. OFFERED_ and REQUESTED_INCOMPATIBLE_QOS count a remote
  endpoint once per policy, not once per matching pass.
  `Publisher::copy_from_topic_qos` copies every shared policy, not
  DURABILITY and RELIABILITY alone. Changing DURABILITY_SERVICE,
  DESTINATION_ORDER or PRESENTATION on an enabled endpoint, PRESENTATION
  on an enabled group and the Changeable=NO topic policies is refused.

- **A diagnostic is placed in the file it points at** (`70ac19a8b05`).
  Spans were offsets into the preprocessor's output and were read as
  line and column of the unit as written; comments above an error moved
  it, and an error in an included file was printed against the unit.
  `zerodds-idlc` prints the file and position the diagnostic names and
  `zerodds-build` prefixes each message with `file:line:column`.

- **A `@mutable` type on an `@optional` cycle over two types is planned**
  (`e3ef9a95346`). The EMHEADER's length code planned the member's type
  outside the `@optional` barrier and reported IDL0014.

- **A snapshot keeps a constant's type and an unresolved reference's
  key** (`c5910638396`). The constant's type was not written at all.

- **A member on a type cycle is held behind an indirection**
  (`7ed0d6c547e`) in the C++, Rust, D, Zig, Nim and Swift columns, as an
  `@external` one is; a C default that needs storage takes the reader.

- **A `ParticipantGenericMessage` under `CDR_BE` is read**
  (`3918f83dbed`); it was refused.

- **`cargo deny` duplicate `md-5`** (`65145a60731`): `security-pki` uses
  0.11, the version the rest of the tree has.

- **Generated Rust passes clippy without allowances** (`b620eed7ac1`).
  `unit_arg`, `derivable_impls`, and on a `@mutable` decode
  `collapsible_else_if`, `manual_unwrap_or` and
  `manual_unwrap_or_default` fired in every crate that included the
  text — 214 in `tools/omg-xtypes-main`. The four consumers dropped their
  allowances.

- **`tools/wireprobe/endpoint-number-surface` measured nothing for seven
  columns and exited 0** (`baaee669ff7`). Writer constructors had grown
  an argument, two columns had no emitter, the Ada `use type` clause was
  never written. A control `put_u16(1)` per binding now fails the run when
  the probe's own program breaks, and `endpoints-gate` runs it. Measured:
  all seventeen bindings refuse `put_u16(70000)`.

- **The Java RPC PSM test ran nowhere with a JDK** (`7736a7947fe`). It
  passed in `test:` without measuring; `endpoints-java-async` runs it with
  `ZERODDS_REQUIRE_JDK=1`.

- **Kotlin: generated code that did not compile, and `long double` values
  written wrong** (`98b68a603ae`, `85f5dff960c`, `51f1611c18d`). Measured on
  the Kotlin column of the matrix, recorded but never reported because the
  matrix's defect reader did not list the column: 1965 cells whose generated
  program did not compile in one direction or both (3237 defect rows), and 176 `long double` cells that wrote other
  values than every other column while reporting the write as done.
  `Float128.parse` halved a `BigDecimal` with Kotlin's `/`, which rounds to
  the dividend's scale (`-2.75` went out as `-2.76`), and wrote subnormals as
  zero; it now rounds the exact rational once, as the Java runtime does, and
  `toString` writes every digit. A decoded array's slots start with a value
  of their own form (`IntArray(0)`, `emptyArray<T>()`, `Float128.zero()`), a
  typedef of a plain value gets a function of its name that answers its
  zero, and the runtime's constants are written with their package where an
  IDL type of the same name shadows them.
- **F#: every bounded string and wstring was a compile error**
  (`35af67f88ae`). The codec writes bounds as `uint32` literals and
  `WriteStringBounded`/`WriteWstringBounded` took an `int`.

- **A durability-service restart never served a `--topic NAME` entry with no
  `type` override, so a late joiner got zero of the samples still on disk**
  (`eaf36de300f`, `a47414f1dba`, `546ed7aa060`, `bda02ce0d42`, `bd9f597b457`,
  `5eccbe704c1`, `2e56c405a9d`). Such an entry always resolved through
  `serve_discovered`, which calls `serve_typed` only once a writer's SEDP
  announcement arrives; on a restart with no application writer running,
  that wait never ends. Every `DurabilityStore` backend (in-memory, file,
  sqlite, lakehouse, postgres) now persists a topic's type name, keyedness
  and retention contract (`TopicMeta`) the moment it is first served, and
  `serve_if_stored` resumes serving it from that alone on the next run —
  before falling back to discovery for a genuinely new topic. Fixing this
  also surfaced a self-deadlock in the sqlite/lakehouse/postgres adapters:
  `cleanup()` held its connection/client lock while iterating rows of
  another statement, and the new contract lookup re-locked the same handle
  from inside that loop; `contract_for_conn`/`topic_meta_row_conn` now take
  an already-locked connection instead of re-locking it.

- **`zerodds-spy` did not build** (`427a342d9b5`). The coherent-set-end
  line formatted `Option<i64>` with `{}`, which `Option` does not
  implement; formatted with `{:?}` like the neighbouring lifecycle arm.

- **A RELIABLE stream lost samples permanently and without a trace**
  (`924342d75c0`). `ReliableReader::handle_data` marked the sequence number
  received and discarded the cache insert's result with `let _ =`. With a full
  proxy cache the number counted as received with no sample behind it, and the
  reader never asked again — one lost datagram could take several hundred
  later samples with it.

- **Every sample taken through the C API created an instance with a full
  payload copy that never went away** (`f5faadc9cbe`). Measured externally:
  258.4 MB in 545,265 live allocations after 613 s, 69 % of the process image,
  24.7 MiB/min. `payload_key_hash` hashes the whole payload, so on a keyless
  topic every sample was its own instance.

- **A reader received nothing after a writer restart** (#32, `fc86c9d2f4c`,
  `31d0cebe210`, `ba56dad5c96`). A restarted writer inherits its dead
  predecessor's unicast port; the reader keeps the dead proxy for the SPDP
  lease and sends both proxies' HEARTBEATs to that one port — 632 for the dead
  one announcing an empty range, 2 for the live one. Without `INFO_DST` the
  successor applies the dead proxy's range and the endpoints never match.
  The fix is scoping, and it had to be repaired twice more: a later
  `set_behaviour` reset the axis the constructor had set, and GAP, lifecycle
  DATA, single DATA and the DATA_FRAG paths were opening their datagram
  without it.

- **Every sample went to the same process twice** (`cb17648c134`) on a host
  with two IPv4 addresses. Measured with `strace -c`: 2.011 `sendto` per
  sample.

- **A reliable ZeroDDS reader rejected a Cyclone writer by RxO**
  (`71c8286ee95`). Both SEDP parsers ended the RELIABILITY parameter with
  `.unwrap_or_default()`, and the policy's default is the reader's
  (BEST_EFFORT) — while DDS 1.4 table 12 gives a DataWriter RELIABLE. Cyclone
  omits the parameter because it is the default.

- **A foreign vendor id was read, discarded and replaced with our own**
  (`52128b12557`) in three of four rewritten captures. **A vendor PID was read
  as ours** (`78b0b317d8b`): `PID_ZERODDS_TYPE_ID` is 0x8002, and DDSI-RTPS
  2.5 §9.6.2.2.1 gives that range to every vendor — RTI's announcement carries
  an 0x8002 with a different meaning, and both `ci-rti` cells matched nothing.

- **Cross-vendor type matching fell back to the topic type name** (#30,
  `a9a790d5473`, `8081dfc6a7e`). The `PID_TYPE_INFORMATION` blob was stored
  and never turned into a comparable `TypeIdentifier`, so the assignability
  guard never ran: a `@final`-against-`@appendable` conflict produced a match
  followed by a decode error, instead of a refusal at match time.

- **LIFESPAN expired only in the writer cache** (`6ef38c4250d`) and **instance
  end states never reached the reader** (`d586f258dc8`) — the OMG dds-rtps
  suite's `Test_Lifespan_0..7` and `Test_FinalInstanceState_0..2` were red.

- **Wildcard partitions never matched** (`1f61814288c`): partition names were
  compared for exact string equality, so a writer in `p1` never matched a
  reader in `p*`.

- **A WaitSet of ReadConditions ticked while idle** (`08caf769173`) — measured
  by a user at 226 trigger evaluations per 300 ms and reader with nothing
  arriving — and **`get_trigger_value` drained the channel on every
  evaluation** (`440416d3325`).

- **Multi-homed discovery (#27).** On a host with more than one interface (VPN,
  Docker, second NIC) ZeroDDS announced a single discovery locator resolved from
  the default-route source-address probe. When that interface differs from the
  one a peer shares, the announced unicast locator is unreachable: the peer
  discovers the participant over multicast (`discovered=1`) but never matches an
  endpoint (`matched=0`) and no data flows. Automatic mode now enumerates every
  eligible unicast interface (`if-addrs`) and announces **all** of them, so a
  peer on any shared segment finds a reachable locator. Metatraffic is fanned out
  to every de-duplicated usable peer locator (UDP cannot detect a dead
  destination). `ZERODDS_INTERFACE` keeps its deterministic single-interface
  behaviour. Multi-interface multicast join/TX is intentionally deferred —
  packet capture proved multicast was never the failing operation.

- **Wire-supplied lengths are capped before allocation** (`ac00e4cb8c8`) — a
  network-reachable denial of service in the OPC UA PubSub and AMQP paths.

- **Secured interoperation with Fast DDS.** A secured ZeroDDS participant
  announced the reliable secure SPDP endpoints and ran them only with
  `ZERODDS_SECURE_SPDP=1`; Fast DDS 3.6.1 authenticates and admits such a
  peer but matches its secure SEDP endpoints only after the participant
  data arrived on that channel, so no secured pair with Fast DDS matched.
  `security.secure_spdp` is on by default on every secured participant;
  the 13 governance profiles of the security deep matrix match ZeroDDS
  with Fast DDS in both directions, with EC identities (RSA is not needed).

- **Two secured ZeroDDS participants no longer flood each other.** The
  replier answered every handshake final after completion with its cached
  reply, and the initiator answered that with its final again: 143,262
  replies and 162,991 finals in one bench cell. The secure SPDP writer
  answered an ACKNACK that asked for nothing with DATA and a HEARTBEAT,
  which drew the next ACKNACK. Both crowded out discovery (no match, lost
  samples); a final after completion and an ACKNACK past the one sample
  now get no answer.

- **TCP user transport** (`f3b06241837`, `87c830bbedd`). The listener read
  one connection until it closed: a second peer's frames stayed unread and
  its send waited for ever on the handshake. Each connection is now read in
  a thread of its own; connect, handshake and a stalled write are bounded;
  `shutdown` closes the transport instead of waiting for every peer to
  disconnect. With several user transports, every TCP leg gets its accept
  worker.

- **Reliability** (`cd32ebf67b8`, `e9ca5bfdd9f`, `3de606317e1`, `084ae7aa499`):
  a reliable reader requests what a writer sent before the match, a writer
  with nothing written heartbeats a reliable reader until it answers, a GAP
  lets the next DATA through before any HEARTBEAT, and what a HEARTBEAT
  releases is delivered. `LIVELINESS` counts each matched writer
  (`ad1c1b43bd2`).

- **DataReader QoS on `take`/`read`**: `HISTORY`, `RESOURCE_LIMITS`,
  `TIME_BASED_FILTER` and `DESTINATION_ORDER` hold on every read path
  (`26f2c6b0c53`, `ae3e1e8de4d`, `729c7e33a96`); a `TRANSIENT_LOCAL` late
  joiner gets what the writer's `HISTORY` keeps (`f55afcdcd74`).

- **DDS-Security payloads**: a protected payload is padded to four bytes
  before encryption and a received one loses the padding its options word
  names (`2c32030d7b6`, `1b3b2c744f3`); remote endpoint permissions are
  enforced (DDS-Security 8.4.2.9.9/10, `7ccbbc23eed`).

- **Discovery**: every participant an SPDP datagram announces is
  discovered (`ddfe5702dde`); submessages are attributed by the receiver
  state in force, not the datagram header (`d4a4489dd0a`, `7cf44ef40cc`),
  which OpenSplice's bundled SEDP needs; `discovery.no_multicast` sends no
  multicast packet (`c5aa6d5ad6a`).

- **UDP**: a departed participant's connected sockets are closed
  (`ee3ec6ea4eb`), and a bound socket gets a send buffer that holds the
  largest datagram (`b59ae01f433`; macOS limits a datagram to 9216 bytes
  otherwise).

- **A vendor-specific parameter 0x8083 is not read as
  `PID_RELATED_SAMPLE_IDENTITY`** (DDS-RPC 1.0 §7.8.2, DDSI-RTPS 2.5
  §9.6.2.2.1; `1a30e3dd368`). `inline_qos::find_related_sample_identity`
  cleared the vendor bit as well as the must-understand bit, so
  `0x8083` read as DDS-RPC's `0x0083`. It masks the must-understand bit
  only.

- **C++ DDS-RPC: a reply with a remote exception code no longer hides the
  replies behind it** (DDS-RPC 1.0 §7.11.1.4.13; `05c799d7e04`). A reply
  that carries a remote exception code is the exception of the future or
  callback that waits for it. One nobody waits for stays for
  `receive_reply` and `receive_replies` to raise; the samples behind it were
  lost. A synchronous listener that throws answers with the code of what it
  threw (§7.5.2), else `REMOTE_EX_UNKNOWN_EXCEPTION`.

- **`read`/`take` with a QueryCondition apply its expression** (DDS 1.4
  §2.2.2.5.9; `d7324a52745`, `0ab5db250d6`). `zerodds_dr_read_w_condition`,
  `zerodds_dr_take_w_condition` and the `*_next_instance_w_condition` reads
  ignored the expression and returned every sample the state masks let
  through; the trigger value evaluated the expression against a row without
  fields and passed every sample. They read the fields through the reader's
  type now. A QueryCondition that names a field on a reader without a type
  returns `NO_DATA` while the reader holds no sample to judge and
  `PRECONDITION_NOT_MET` as soon as it holds one; `BAD_PARAMETER` stays for
  an expression that does not parse. Tests:
  `read_and_take_with_a_query_condition_return_only_the_samples_the_expression_selects`,
  `a_query_condition_without_a_type_has_no_data_to_judge_when_empty_and_refuses_a_sample_it_cannot_read`.

- **C++: entity operations called the C API instead of returning defaults,
  and `DataWriter::enable` compiled** (DDS 1.4 §2.2.2.1.1.6–8;
  `5d6bf43ff4b`). `enable`, `status_changes` and `instance_handle` returned
  `0` or a nil handle, and `DataWriter::enable` called a C function that
  does not exist, so the header did not compile. `Topic::qos(q)`,
  `Topic::inconsistent_topic_status()`, `Topic::qos()` and
  `DomainParticipant::qos()` reach the C API; `default_participant_qos()`
  returned the default-constructed QoS. Test: `test_entity_operations`.

- **C++: `DataReader::read` kept the samples it read, a throwing decode
  returned the loan, listeners saw the real entity** (DDS 1.2 §7.1.2.5.3.8,
  §7.1.2.4.4, DDS-PSM-Cxx 1.0 §7.3; `56661152a10`, `68c83d552ba`,
  `0482f3b2af3`, `9918966832b`, `41c2384a98c`). `read` called `take`, so
  the samples were consumed and `sample_state` READ never appeared. A decode
  that threw after `zerodds_dr_take` left the loan open. A listener callback
  received a default-constructed entity without a handle and an all-zero
  status, and two writers of one type in one thread shared one listener table;
  each entity owns its table now, deregisters it on close, and the callback
  gets the entity and its status (`DataReader<T>::sample_rejected_status()`
  and `requested_incompatible_qos_status()` are new).
  `ContentFilteredTopic::name()` returned the related topic's name.
  The millisecond conversion of `Duration` wrapped the seconds past `int32_t`
  and `nanosec` took values above 999 999 999; the conversion throws
  `std::overflow_error` and `nanosec` normalises into the seconds. `wait_for_acknowledgments` and `wait_for_matched` turned an
  infinite `Duration` into 2^31 · 1000 − 706 ms; they pass
  `ZERODDS_BLOCKING_INFINITE`. Tests: `test_read_keeps_samples_take_removes_them`,
  `test_take_returns_the_loan_when_decode_throws`,
  `test_listener_table_is_per_entity_and_deregisters_on_destroy`,
  `test_duration_sentinel_conversion_and_overflow`.

- **Java PSM: durations and times keep their units** (`819bfcb46cb`).
  `Duration` and `Time` `split()` left the sub-second part in the unit it was
  given, so 50 ms was 50 ns and every deadline and lease was zero, and a
  duration of any whole number of seconds was infinite. `String` and the
  primitive wrappers (Table 8.1) are topic types of their own; a null member
  that is not optional is an `XcdrException`. The order of Deadline,
  LatencyBudget and Liveliness for request-offered matching treats the
  shorter period as the greater service.

- **Java PSM: the reflective type support follows DDS-Java-PSM 1.0 §8, and
  `byte[]` is `DDS::Bytes`** (`cb60331dfa9`, `62ca1705ea3`). The
  extensibility was FINAL for every class; it is FINAL for a final class
  that extends `Object` directly (§8.6.1.2) and EXTENSIBLE otherwise
  (`ExtensibilityKind.EXTENSIBLE`, PLAIN_CDR2 like FINAL). The fields of a
  base class were left out without a message; they are written before the
  class's own (§8.6.1.1). `java.util.Collection` is a sequence,
  `BigInteger` a UInt64, `BigDecimal` a Float128 and a Java enum an
  enumeration (Table 8.1); reading them failed before. An annotation was
  matched by its simple name; only those of `org.omg.dds.type` and
  `org.zerodds.types` are read (§8.9), and an `@Id` whose value is no number
  is refused with the field's name. The built-in `byte[]` topic announced
  `DDS::Octets` and wrote the octets without framing; `ByteArrayTypeSupport`
  reports `DDS::Bytes` (XTypes 1.3 Annex E), APPENDABLE, and writes a
  DHEADER, the 4-octet `ByteSeq` length and the octets; truncated framing is
  an `XcdrException`. Tests: `ReflectionTypeSupportTest`,
  `ByteArrayTypeSupportTest`.

- **Java PSM: `read()`/`take()` return copies, durability TRANSIENT and
  PERSISTENT survive the writer, and deadline and liveliness listeners
  fire** (DDS-Java-PSM 1.0 §7.7.3; DDS 1.2 §7.1.3.4, §7.1.4.2; `ec59b455ee5`,
  `1d30c852d96`, `68b82f541e0`). A reader returned its own decoded objects,
  so changing a returned sample changed the next `read()`; it deep-copies
  through the `TypeSupport`. TRANSIENT and PERSISTENT were treated as
  TRANSIENT_LOCAL; `org.zerodds.internal.DurabilityService` retains the
  samples beyond the writer and replays them to late readers (TRANSIENT is
  dropped when the participant closes; PERSISTENT outlives it, in memory,
  with no disk in the pure-Java stack). `REQUESTED_DEADLINE_MISSED` and
  `LIVELINESS_CHANGED`/`LIVELINESS_LOST` were evaluated only when the
  application polled the status getter; a daemon-thread lease
  (`org.zerodds.internal.LeaseMonitor`) raises the callback, and
  `DataWriterListener` has `onOfferedDeadlineMissed` and `onLivelinessLost`.
  Entity operations were not reentrant (§7.2.4): `write()` held the writer's
  monitor while it delivered, and delivery held the reader's monitor while
  the listener ran; state changes happen under the entity's own monitor and
  callbacks run without it. Tests: `transientSurvivesTheWriter`,
  `persistentSurvivesTheParticipant` (`DurabilityServiceTest`),
  `requestedDeadlineMissedFiresWithoutPolling` (`LeaseTest`),
  `aCallbackDoesNotHoldTheReaderMonitor` (`ReentrancyTest`).

- **Java PSM: the shipped main sources compile with `--release 8` again**
  (`2e0b194caf5`, `7d991374fd4`). `mvn -Pjava8` builds the main artifact for
  Java 8 (class file version 52); `instanceof` patterns, a `record` and
  Java 9+ library calls (`List.of`, `Set.of`, `String.isBlank`, `Path.of`,
  `Arrays.compareUnsigned`, `AccessibleObject.canAccess`) were used in about
  forty files. The tests stay at Java 17. `javac -Xlint:all -Werror` over the
  binding reports no warning; the value types of §7.3.3 are `Serializable`.
  The binding has no dependency on `crates/zerodds-c-api`: no JNI, no
  `System.loadLibrary`; over RTPS it talks to the Rust stack on the wire.

- **The Data Samples root elements `<data>` and `<sample>` are declared in
  the namespaced schema file** (DDS-XML 1.0 §7.3.1; `5e03ee5e374`). A
  `_nonamespace.xsd` file "specifies neither a targetNamespace nor a root
  element"; the Data Samples chameleon file declared both. They are in
  `dds-xml_data_samples_definitions.xsd` now, and no chameleon file declares
  a root. `serialize_sample_output_validates` validates the namespaced form.
  **Migration:** validate a sample document against the namespaced file.
  Tests: `namespaced_xsds_define_top_level_element` (`schemas`),
  `serialize_sample_output_validates` (`schema_validation`).

## [1.0.0-rc.6] — 2026-07-22

Native-reach + hardening release. 93 commits over rc.5.

### Added

- Native endpoint SDKs (ADR 0013): no-Rust DDS-XRCE endpoints in C89 / C++98 /
  pure-Python / Java-8 for constrained, legacy and big-endian platforms a full
  Rust stack cannot reach. One wire truth — every encoder byte-identical to the
  Rust core (verified native x86-64 and big-endian PowerPC), `wire-fixed`
  codegen (C/Python/Java) + `wire-variable` reflective codec (C/Python/Java,
  C++ via C-reuse), full extensibility (final/appendable/mutable), XRCE
  (`WRITE_DATA`/`DATA`/`HEARTBEAT`/`ACKNACK`) + serial (Annex-C HDLC) framing, a
  single frame-hook integration point, and a live-UDP end-to-end path against a
  real `zerodds-xrce` agent. Runnable C/Python/Java publish + receive examples.
- SpatialDDS 1.6 profile (`zerodds-spatial-dds`) — every published module run
  through ZeroDDS' own IDL→Rust codegen, XCDR-native; SpatialDDS↔ROS 2 semantic
  mapping (`zerodds-spatial-ros2`).
- PostgreSQL durability cold adapter (`zerodds-durability-store-postgres`),
  sqlite-identical semantics, optional TimescaleDB hypertable; 10k-sensor load.
- Routing-Service: preserve_source_timestamp, per-route token-bucket throttle +
  drop metric, tenant-scoped ACL, RS↔RS chaining.
- Durability-Service: source-identity dedup (a restart no longer duplicates a
  writer's history).
- Opt-in multicast-locator allowlist (fail-closed); per-topic security overhead
  measurements; iceoryx delivery selector (doc + CI gate).

### Changed

- rmw multi-distro CI gate is now required (humble/jazzy); docker `cargo chef`
  images carry libclang.

### Fixed

- `durability-store-lakehouse`: O2 P5 source-identity parity — the feature-gated
  (libduckdb) cold adapter now carries `source_guid`/`source_sequence` like the
  sqlite/postgres/file adapters (schema + migration + insert + read).

### Verification

- All CI gates green: fmt, clippy `--all-features`, `cargo-deny`, `zerodds-lint`,
  build (x86_64 + aarch64 + no-std), test, coverage, compliance, ts-node,
  python-tests, tsn-live, sphinx-docs, windows-smoke. Native endpoints
  byte-identical native x86-64 + PowerPC-BE + live-UDP E2E; SpatialDDS 12/12 +
  ROS 2 mapping 7/7; PostgreSQL 8/8 (live PG 17 + TimescaleDB); routing 31/0.

## [1.0.0-rc.5] — 2026-07-08

Hardening + reflective-completeness release. 32 commits over rc.4.

### Added

- Reflective `DynamicData` codec `map<K,V>` support end-to-end — `read_map` /
  `write_map` byte-identical to the compiled `BTreeMap` CdrEncode (XCDR1/XCDR2,
  primitive-pair + non-primitive DHEADER), `DynamicValue::Map`,
  `collection::map_of`, and JSON/SQLite recorder sinks. Works uniformly in the
  TypeObject bridge, `record`, `replay` and `spy`.
- Composite-in-collection reflection: `sequence<Struct>`, `Struct[N]`, and
  arbitrary nesting (`sequence<sequence<Struct>>`) resolve to fully-populated
  `DynamicType`s via `dynamic::collection`, so a bridge-built type is codec-usable.
- `resolve_minimal` — Minimal TypeObjects resolve to working `DynamicType`s (all
  kinds + `EquivalenceHashMinimal` members), wire-identical to their Complete
  twin (member/type names are synthetic hashes, an intrinsic XTypes property).
- Endpoint-delete lifecycle: `Drop` for `DataWriter`/`DataReader` → local
  teardown + SEDP dispose, the `remove_remote_writer/reader` unmatch primitive,
  cache eviction, and builtin-reader dispose. Both directions verified
  cross-host on Linux multicast.
- `DomainParticipantFactory` RAII: weak refs release the runtime (threads +
  sockets) when the user handle drops.
- CoAP-bridge serves `coaps://` over DTLS (ADR 0011), fail-closed on
  `dtls.enabled` — never serves plaintext silently.
- Website: favicon, `robots.txt`, `sitemap.xml`.

### Changed

- Composite `typedef` targets resolve transparently in the TypeObject bridge
  (a typedef has its target's wire form).
- Shared `read_element` / `write_element` factored across the sequence, array
  and map codecs.

### Fixed

- rmw `rmw_wait` segfault on nodes using rcl-internal guard conditions (e.g.
  rclrs): both wait-set casts now gate on `implementation_identifier` before
  touching `gc->data`. Reported via PR #8; verified against the real ROS-Humble
  ABI.
- Reliable SEDP dispose delivery — `write_lifecycle` drains a lagging proxy so
  the dispose reaches cursor-independent readers.
- RUSTSEC-2026-0204: bump `crossbeam-epoch` 0.9.18 → 0.9.20.

### Verification

- `zerodds-types` 324 tests + `recorder-decode` green; `map<K,V>` byte-oracle
  vs the compiled `BTreeMap`; Minimal-vs-Complete wire identity; `cargo check
  --workspace` green. The rmw fix verified on the Linux test host against the real
  ROS-Humble ABI (deterministic before/after segfault harness).

## [1.0.0-rc.4] — 2026-06-30

Cross-vendor wire-conformance and full multi-language parity release.
The headline: **every language binding now encodes byte-identically over
the wire, in every representation (XCDR1, XCDR2, big-endian), and is
anchored against the Cyclone / Fast DDS / OpenDDS / RTI Connext encoders.**
167 commits over rc.3.1.

### Added

- **Reflective dynamic XTypes codec** (`zerodds-types`) — `DynamicData` ↔
  CDR over XCDR1 (classic CDR / PL_CDR1), XCDR2, and `@mutable` (PL_CDR2)
  union framing; registry-aware `TypeObject` → `DynamicType` resolution
  across all 10 type kinds, nested composites, scalar/composite-element
  collections, `wstring`, and bitmask/bitset. Lets `zerodds-spy` /
  `zerodds-record` follow a writer's real `type_name` and decode samples
  with no compile-time type.
- **Recorder decode + replay** (`zerodds-record`, `zerodds-replay`,
  `zerodds-spy`) — `--decode` / `--type-file` / `--out-json` /
  `--out-sqlite` wired into capture; typed per-topic SQLite + JSON/NDJSON
  sinks; type-following live `.zddsrec` capture; `replay` reads
  `.zddsrec | NDJSON | SQLite` byte-exact.
- **`fixed<P,S>` through all seven PSMs** (C++, C, C#, Java, Python,
  TypeScript, Rust) — previously silently dropped on the wire in some
  backends; now byte-identical to the CORBA BCD oracle.
- **XCDR1 (classic CDR / PL_CDR1) encode + decode in every binding** —
  Java, TypeScript, C#, Python, the C backend (`c_mode`), and C++ (which
  was secretly XCDR2 under the hood).
- **Big-endian receive + emit end-to-end** — threaded through the typed
  decode path in C++, C, Python, C#, Java, TypeScript and Rust, the
  C-API loan-batch `SampleInfo`, the WASM / websocket-bridge browser
  path, and durability (stored samples retain + replay their wire
  byte-order).
- **iceoryx ↔ Cyclone same-host zero-copy bridge** (`cyclone-iox`
  feature) — ZeroDDS ↔ Cyclone over iceoryx C++ (POSH); the PSMX chunk
  is stamped with the writer's real RTPS GUID.
- **`zerodds-idlc` default-extensibility flag** —
  `ext-default-{appendable,final,mutable}` cfg option.
- INFO_TS source timestamp wired end-to-end through the writer send path
  (RTPS-F1).
- Per-binding DDS QoS + entity-lifecycle surface completed
  (dispose/status/CFT/partition/policy classes; Java + TypeScript reached
  parity).

### Changed

- **Same-host SHM carrier is now event-driven (shared futex)** instead of
  sleep-poll. Removes a same-host round-trip latency regression
  (≈66 µs → ≈18 µs on the internal bench).
- **DuckDB (lakehouse) durability backend is now opt-in** — the
  self-contained `-sqlite` / `-file` backends are the default; the
  lakehouse adapter + daemon are `publish = false` (they link the system
  `libduckdb`, which has no portable crates.io build).
- Default nested-struct extensibility settled on `@final` (matches our
  cross-vendor `@final`-everywhere bench decision; SX2).

### Fixed

- **Cross-PSM XCDR2 wire divergence** — the 7 backends produced 7
  different encodings (0/42 cross-decode); all converged to one
  byte-identical encoding, anchored against vendor `idlc` output.
- **Canonical XCDR2 encoding**: spurious DHEADER on primitive multi-dim
  arrays (PARRAY §7.4.3.5), `@mutable` EMHEADER length-code compaction
  (LC=4 → compact 0–3/5), `@mutable` decoder now accepts all length codes
  including LC5–7 from Fast DDS, and bitmask holder size honours the
  `@bit_bound` default.
- **`TypeObject` / `TypeIdentifier`** serialization + hash non-conformance
  versus all three vendors; **TYPE_CONSISTENCY** — typed endpoints with a
  complete `TypeIdentifier` now match (was a foundational matching
  failure).
- **DATA_REPRESENTATION (DR1)** — the reader announced only XCDR2 and
  dropped XCDR1 writers (Cyclone / legacy RTI); it now accepts both.
- DCPS runtime hardcoded the XCDR data-representation tag to 0 on the
  same-runtime loopback path (R4 / R4b).
- **CORBA wire**: `fixed<P,S>` BCD packing dropped the most-significant
  digit / emitted the wrong octet count; standalone `wchar` / `wstring`
  GIOP 1.2 wire form corrected.
- **ROS rmw wire codec**: `wstring` / `wchar` fields no longer abort the
  codec; `char` maps to `uint8` (1 byte) per REP-2008, not IDL `wchar`.
- **Code-generation correctness sweep across all backends** — the A–Q bug
  cluster: union (enum / scoped discriminator), map, typedef-to-primitive
  (data loss), nested + forward-declared / mutually-recursive types
  (one was a stack-overflow crash), sequence element types, fixed-size
  array members, `@optional`, `const` declarations, and `@value(N)` enum
  discriminants — fixed across Rust, C++, C, C#, Java, Python and
  TypeScript, with a committed language × IDL-feature conformance matrix.
- KeyHash (≤16 B inline + MD5 over 16 B) and `@key` instance handling;
  `@bit_bound(8/16)` enums narrow-encode (spec / Cyclone), per the
  recorded decision.

### Verification

- Cross-vendor rich-typed XCDR2 + DDS-Security perf/interop matrices over
  all five stacks (`tests/perf/dds-roundtrip-bench/`), plus public proof
  projects in the `zero-dds-snippets` example repo.

## [1.0.0-rc.3] — 2026-06-19

Protocol-reach release: OPC-UA client/server + Pub/Sub, a
production-grade ROS-2 RMW with same-host zero-copy, and the in-DDS
routing service. 121 publishable crates (rc.1 had 97; +24 new).

> rc.2 was never tagged on the GitLab root — it shipped as a GitHub-only
> `zerodds-websocket-bridge` hotfix. Root went rc.1 → rc.3 directly; this
> section therefore also covers what rc.2 carried publicly.

### Added

- **OPC-UA client/server wire stack** — UACP connection protocol +
  secured SecureChannel handshake (Part 6 §6.7) + Session + Read / Write /
  Call + Browse (View §5.9.2) + Discovery (GetEndpoints / FindServers) +
  Subscription / MonitoredItem (§5.13/§5.14), end-to-end over `opc.tcp`.
- **OPC-UA Pub/Sub (Part 14)** — JSON message mapping (§7.2.3), AMQP /
  Kafka / Ethernet carriers, AES256-GCM security policy (AEAD), the
  PubSub Information Model (§9), and SKS `GetSecurityKeys`.
- **`zerodds-routing-service`** — in-DDS domain routing service (v1–v3).
- **`zerodds-secure-permissions`** — CMS / PKCS#7 signer for DDS-Security
  governance + permissions XML.
- **Same-host zero-copy** — real in-place `flatdata` SHM loan on the
  runtime FFI, plus the ROS-2 `RawSameHost` / iceoryx delivery modes and
  the rmw-side loaning path (fixed-POD + CDR interop).
- **ROS-2 RMW shim productionized** — event-driven `rmw_wait` (no
  busy-poll), services + actions in the C-ABI, serialized pub/take
  (rosbag2 path), topic-level graph introspection, endpoint-info
  (REP-2009 layers 1–5), and `rmw_get_node_names` via `ros_discovery_info`.
- `zerodds-admin` grown into a real operator CLI (live + offline,
  `--json`); `xmlc` / traceability binaries wired; `zerodds-monitor` 1.1.
- Complete `TypeObject` for Bitset + Annotation; idl-cpp XCDR2
  `sequence<@appendable/@mutable struct>` (last XCDR2 gap closed);
  C3 initial-announcement burst for WiFi-robust discovery.

## [1.0.0-rc.3.1] — 2026-06-20

Release-machinery hotfix over rc.3 — no library behaviour change.

### Fixed

- The DuckDB lakehouse adapter + durability daemon are `publish = false`
  (source-only): they link the *system* `libduckdb`, so the crates.io
  verify build fails `cannot find -lduckdb` for every consumer. The
  self-contained `-sqlite` / `-file` backends stay published.
- `zerodds-py` pinned to `1.0.0-rc.3` so maturin can build a PyPI wheel —
  PEP 440 cannot represent the dotted SemVer prerelease `1.0.0-rc.3.1`;
  PyPI publish skipped for rc.3.1.
- `c-api` `smoke_ffi` `take_into` test used a `domain_id` outside the SPDP
  port range; rustfmt + `zerodds-lint` compliance in rc.3.1 binding code.

## [1.0.0-rc.2] — 2026-05-15

Hotfix release. Fixes two reported bugs in `zerodds-websocket-bridge`
that prevented the `zerodds-ws-bridged` daemon from being usable in
production-style deployments (zeroCollab Wave 2b ran into both).
No other crates changed materially — workspace-shared version bump
keeps versions consistent across all 90 crates.

### Fixed

- `zerodds-websocket-bridge`: `--topic`, `--auth-token`, `--tls-cert`,
  `--tls-key` and `--metrics` CLI flags were parsed but never applied
  to `DaemonConfig`. The daemon booted with `topics=0 auth-mode=none
  tls=off metrics=off` regardless of CLI overrides. Extracted
  `apply_cli_overrides()` with replace-semantics for scalars and
  additive semantics for `--topic` per spec §2. 8 new unit tests.
  (Resolves GitHub issue #1, PR #5.)
- `zerodds-websocket-bridge`: shipped `ws-bridged.yaml.example` used
  the old nested `participant:/websocket:/routes:/observability:`
  schema; the real parser expects flat top-level keys per spec §3.
  Bridge silently ignored the unknown keys and booted with defaults.
  Example rewritten + spec ref corrected (§4 → §3) + `include_str!`-
  based loadback integration test added to fail CI on future drift.
  (Resolves GitHub issue #3, PR #5.)

### Added

- `zerodds-websocket-bridge`: `load_from_str` now emits a stderr
  WARN-line for unknown top-level config keys instead of silently
  ignoring them (forward-compat preserved, drift visible). Was an
  announced follow-up to issue #3.

## [1.0.0-rc.1] — 2026-05-07

Initial Release Candidate. 90/91 crates RC1-ready under the RC1
guardrails' Definition of Done.

### Added — Layer 0 Foundation

- `dds-foundation` — error types, `PoolBuffer<CAP>`, `BufferPool`,
  fixed-capacity stack-allocated buffers for the hot path.
- `zerodds-monitor` — Prometheus metric registry with
  `serve_prometheus()` HTTP exposition.
- `zerodds-observability-otlp` — OTLP/HTTP/JSON v1.4 exporter for
  traces, metrics (explicit-buckets histograms), and log records.
- `zerodds-time` — OMG Time 1.1 implementation.
- `zerodds-flatdata` — zero-copy memory-mapped storage backend.
- `zerodds-flatdata-derive` — proc-macro derive for flatdata schemas.

### Added — Layer 1 Transport

- `zerodds-transport-udp` — UDP unicast + multicast with PVE-aware
  multicast setup.
- `zerodds-transport-tcp` — TCP transport with connection pooling.
- `zerodds-transport-shm` — shared-memory transport (iceoryx2-compatible).
- `zerodds-transport-tsn` — Time-Sensitive Networking profile (IEEE 802.1).

### Added — Layer 2 Wire Protocol

- `zerodds-rtps` — OMG DDSI-RTPS 2.5 (121/121 normative sections + 3 n/a),
  reliable + best-effort writers, fragmentation, NACK_FRAG, all 8
  submessage types.
- `zerodds-discovery` — SPDP + SEDP, ParameterList encoding,
  cross-vendor live discovery against Cyclone DDS.
- `zerodds-builtin-topics` — DCPSParticipant, DCPSPublication,
  DCPSSubscription, DCPSTopic readers.

### Added — Layer 3 Core Services

- `zerodds-dcps` — OMG DDS DCPS 1.4 (100/100 + 2 n/a), Factory →
  Participant → Pub/Sub → Writer/Reader.
- `zerodds-qos` — all standard QoS policies, validation, XML-Loader.
- `zerodds-types` — OMG DDS-XTypes 1.3 (82/82 + 1 n/a), TypeObject,
  TypeLookup, Assignability.
- `zerodds-security` — OMG DDS-Security 1.2 (50/50 + 3 n/a),
  Auth/AccessControl/Crypto/Logging/DataTagging plugins, CRL, RTPS-AAD.
- `zerodds-sql-filter` — content-filtered topic SQL parser + evaluator.
- `zerodds-listener-callbacks` — listener dispatch with safety guarantees.

### Added — Layer 4 Schema

- `zerodds-cdr` + `zerodds-cdr-derive` — XCDR1 + XCDR2 codec with
  derive-macro for `DdsType`.
- `zerodds-idl` — OMG IDL 4.2 (649/649 + 24 n/a) parser + AST + validator.
- `zerodds-idl-{cpp,csharp,java,rust,ts}` — code generators per language.

### Added — Layer 5 Protocol Bridges

- `zerodds-websocket-bridge` — RFC 6455 + 7692, daemon
  `zerodds-ws-bridged`.
- `zerodds-mqtt-bridge` — OASIS MQTT 5.0, daemon `zerodds-mqtt-bridged`.
- `zerodds-coap-bridge` — RFC 7252 + 7641 + 7959 + 6690, daemon
  `zerodds-coap-bridged`.
- `zerodds-amqp-endpoint` — OASIS AMQP 1.0, daemon
  `zerodds-amqp-bridged`.
- `zerodds-grpc-bridge` — gRPC HTTP/2 + gRPC-Web, service auto-generation
  per topic, gRPC reflection v1alpha, daemon `zerodds-grpc-bridged`.
- `zerodds-corba-dds-bridge` — bidirectional CORBA ↔ DDS bridge with
  GIOP/IIOP, Notify channel, LocateRequest, daemon `zerodds-corba-bridged`.
- `bridge-security` — shared TLS (rustls 0.23) + auth (bearer / JWT-RS256
  / mTLS / SASL-PLAIN) + ACL (wildcard + group matching) + SIGHUP cert
  rotation across all six bridge daemons.
- `zerodds-amqp-bridge` — separate AMQP-bridge crate.
- `zerodds-hpack` + `zerodds-http2` — HTTP/2 + HPACK primitives for
  the gRPC bridge.
- `zerodds-zenoh-bridge` — Zenoh interop bridge.

### Added — Layer 6 Language Bindings

- `zerodds-c-api` — extern "C" API, 185 exported symbols, ABI-snapshot
  test (`abi.snapshot.json`).
- `zerodds-cpp` — C++17 header + ABI bridge.
- `zerodds-cs` — C# / .NET 8 P/Invoke bindings.
- `zerodds-java-jni` — JNI bridge.
- `zerodds-py` — PyO3 Python bindings.
- `zerodds-rs` — pure-Rust convenience layer.
- `zerodds-ts-node` — Node.js NAPI bindings.
- `zerodds-ts-wasm` — browser WASM bindings.
- `sys` — marker crate for the C-FFI foundation.

### Added — Layer 7 Bridging Services

- `zerodds-conformance` — workspace-wide conformance test harness.
- `zerodds-soap` — DDS-SOAP-PSM bridge.
- `zerodds-dlrl` + `zerodds-dlrl-codegen` — OMG DLRL 1.2.
- `zerodds-opcua-gateway` — OPC-UA Pub/Sub gateway.
- `zerodds-rmw-zerodds-shim` + `zerodds-ros2-rmw` — ROS-2 RMW
  implementation for ZeroDDS, REP-2007/2008/2009 service + action
  patterns.
- `zerodds-web` — DDS-Web 1.0.
- `zerodds-xrce` + `zerodds-xrce-agent` + `zerodds-xrce-client` — OMG
  DDS-XRCE 1.0 (82/82 + 13 n/a).

### Added — Layer 8 CORBA + CCM

- `zerodds-corba-codegen` — OMG CORBA 3.3 Annex-A.1 IDL-Mapping.
- `zerodds-corba-cos-event` — OMG CosEventService 1.2.
- `zerodds-corba-csiv2` — OMG CORBA 3.3 Part 2 §10 CSIv2 with SAS
  protocol and TLS mechanism OID.
- `zerodds-corba-giop` — GIOP wire codec for 1.0/1.1/1.2 plus
  Bidirectional GIOP.
- `zerodds-corba-iiop` — IIOP TCP transport.
- `zerodds-corba-ior` — IOR struct, all standard tagged components,
  corbaloc / corbaname URLs.
- `zerodds-corba-cosnaming` — OMG CosNaming 1.3.
- `zerodds-corba-poa` — POA implementation with all 7 policies.
- `zerodds-corba-ir` — Interface Repository.
- `zerodds-ami4ccm` — OMG AMI4CCM 1.1.
- `zerodds-ccm` + `zerodds-corba-ccm` + `zerodds-corba-ccm-lib` +
  `zerodds-corba-ccm-ejb` + `zerodds-corba-dnc` — full OMG CCM 4.0 +
  DDS4CCM 1.1 stack with EJB bridge and Deployment & Configuration.
- `zerodds-rtc` — OMG RTC 1.0.

### Added — Vendor Specifications

Seven Vendor Specifications published in OMG-DDS-stylistic format:

- **DDS-AMQP 1.0** — DDS over OASIS AMQP 1.0.
- **DDS-TS 1.0** — TypeScript PSM for OMG IDL 4.2.
- **DDS-WebSocket-Bridge 1.0**, **DDS-MQTT-Bridge 1.0**,
  **DDS-CoAP-Bridge 1.0**, **DDS-gRPC-Bridge 1.0**,
  **DDS-CORBA-Bridge 1.0**, **DDS-ROS2-Bridge 1.0** — daemon-mode
  bridge specifications with conformance levels L1–L6.
- **ZeroDDS-FFI-Loader 1.0** — cross-language ABI loading specification
  with 185-symbol ABI snapshot baseline.
- **ZeroDDS-Deployment 1.0** — Linux/macOS/Windows deployment with
  systemd / launchd / Windows Service registration.

LaTeX sources under `documentation/specs/`, PDFs auto-built via
`tectonic` on tag push.

### Added — Examples + Demos

- `examples/tutorials/dds-chat/` — 15-chapter curriculum covering
  pub/sub, QoS, security, type evolution, recording, and performance,
  with 9 language ports (Rust, C++, C++/Qt6, C#, Java, Python, TS-Node,
  TS-Browser, Flutter), 7 bridges, and 4 apps (Mobile, Web SPA,
  Qt Desktop, MCU).
- `examples/demos/dds-warehouse/` — 10-station industrial-IoT
  automated high-rack-warehouse demo covering DLRL cache, TimeService,
  RTC robotics, TSN-RT, XRCE sensors, OPC-UA PLC, AMI4CCM,
  CCM/DDS4CCM, CORBA mainframe, COS-Event-Service, DDS-Web, DDS-XML.
- `examples/demos/perf-camera-dds/` — Flutter mobile camera publisher
  → WebSocket bridge → DDS → Qt6 desktop tile-view performance demo.
- `examples/demos/otel/` — OpenTelemetry observability sample with
  Jaeger compose file.

### Added — Packaging

- Linux: `.deb` and `.rpm` packages with systemd units; Arch `PKGBUILD`;
  AppImage with musl-static builds.
- macOS: Homebrew formulae plus `launchd` plists.
- Windows: WiX 4 MSI registering each bridge as a Windows service;
  Scoop and Chocolatey manifests for developer workstations.
- Multi-arch Docker images (`linux/amd64`, `linux/arm64`) with
  `docker-compose.yml` covering all seven bridges plus auxiliary
  brokers.

### Stability

- All `pub` items in RC-tagged crates are stable for `1.0`. Breaking
  changes require a new major version.
- The wire format (XCDR1, XCDR2, RTPS 2.5, GIOP 1.2) is fixed by the
  underlying OMG specifications and will not change in `1.x`.
- Internal APIs (anything `pub(crate)`) and "unstable-" prefixed
  modules are not subject to SemVer.

### Acknowledgements

ZeroDDS exists thanks to the OMG specification authors, the broader
DDS community at Eclipse Cyclone DDS, OpenDDS, eProsima Fast DDS, and
RTI Connext, the Rust language and ecosystem maintainers, and the
mosquitto, RabbitMQ, libcoap, and omniORB teams whose
implementations served as reference points for cross-vendor
verification.
