# IDL4 to C# Language Mapping 1.0 — Spec Coverage

**Spec:** [OMG IDL4-CSHARP 1.0](https://www.omg.org/spec/IDL4-CSHARP/1.0/PDF) (61 pages, OMG formal/2021-07-01)

Item-by-item audit against the spec; each requirement with a spec
quote, repo path, test path and status (`done` / `partial` / `open` /
`n/a`).

**Context:** The mapping is `crates/lang-rules/src/emit/idl4_csharp/`,
selected with `zerodds-idlc generate --lang csharp --csharp-mapping idl4`
(`--csharp-naming idl|dotnet`, `--csharp-platform dds|corba`,
`--csharp-corba-module`). Components, homes, ports, connectors and
template module instances go through their intermediate IDL
(`crates/lang-rules/src/emit/intermediate_idl.rs`, `crates/ccm`). The
runtime is `crates/idl-csharp/runtime/Omg.Types.cs` (`ISequence<T>` with
the clause's methods, `SequenceList<T>`, `BoundedList<T>`,
`BoundedDictionary`, `Any`, the attributes) and, for the CORBA platform,
`crates/idl-csharp/runtime/Corba.cs`; the pseudo IDL of the CORBA module
is `runtime/corba/CORBA_PIDL.idl`. Every test compiles the generated C#
with Roslyn (`-warnaserror+ -warn:4`) together with the runtime and runs a
check program whose expectations are the spec text.

The default of `--lang csharp` stays the XCDR2 binding with its codec
(`crates/lang-rules/src/adapter/csharp.rs`); in several places it does not
follow this spec and is not the subject here.

---

## §1 Scope

### 1.1 Mapping IDL v4 -> C# (ECMA-334)

**Spec:** §1, p. 1 — "This specification defines the mapping of OMG
Interface Definition Language v4 to the C# programming language
[ECMA-334]. The language mapping covers all of the IDL constructs in
the current Interface Definition Language specification [OMG-IDL4]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the scope clause names what the spec
is about, not a requirement on an implementation.

---

## §2 Conformance Criteria

### 2.1 Implementation: IDL -> C# source per Chapter 7

**Spec:** §2, p. 1 — "A conformant implementation shall transform IDL
input into C# source code output as specified in Chapter 7."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/mod.rs::generate`, `tools/idlc/src/main.rs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_6_a_value_type_is_an_abstract_class`, `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_7_9_to_7_13_components_homes_connectors_and_template_instances`, `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_a_1_typecode_object_any_and_user_exception_are_the_annexs_classes`, `tools/idlc/src/main.rs::idl4csharp_7_1_1_the_idl4_mapping_its_naming_scheme_and_platform_are_selectable`

**Status:** `done` — `zerodds-idlc generate --lang csharp --csharp-mapping idl4` maps IDL to C# by chapters 7 and 8; every test compiles the generated C# with Roslyn (`-warnaserror+ -warn:4`) together with `Omg.Types` and runs a check program. The default of `--lang csharp` stays the XCDR2 binding (a decision, see the context).

### 2.2 User: portable application source code

**Spec:** §2, p. 1 — "Conformant application source code, as a result,
will be portable across implementations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a statement about the conformance of
application code, not an implementation requirement.

---

## §3 Normative References

### 3.1 Normative references (CORBA-IFC, ECMA-334, OMG-IDL4, .NET-GUIDE, .NET-STD)

**Spec:** §3, p. 1 — list of the normative documents.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a reference list; it imposes no
requirement on an implementation by itself.

---

## §4 Terms and Definitions

### 4.1 Terms (Building Block, C#, Camel Case, Language Mapping, Pascal Case)

**Spec:** §4, p. 2 — definitions; "Camel Case" is Lower Camel Case,
"Pascal Case" is Upper Camel Case.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary; the naming rules
themselves are audited in §7.1.1.2.1/§7.1.1.2.2.

---

## §5 Symbols

### 5.1 Acronyms (Tab.5.1)

**Spec:** §5, Tab.5.1, p. 2 — acronym list.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a list; no implementation requirement.

---

## §6 Additional Information

### 6.1/6.2 No changes to OMG specs; acknowledgments

**Spec:** §6.1/§6.2, p. 3.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — background; no implementation requirement.

---

## §7.1 General

### 7.1.1 Two naming schemes, selected via @csharp_mapping

**Spec:** §7.1.1, p. 5 — "This specification defines two naming
schemes [...] IDL Naming Scheme [...] .NET Framework Design Guidelines
Naming Scheme. The @csharp_mapping annotation defined in Clause 8.1
provides a mechanism to select the appropriate naming scheme."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/names.rs::Naming`, `crates/lang-rules/src/emit/idl4_csharp/mod.rs::Settings::apply`, `crates/idl-lower/src/annotation.rs::mapping_annotation`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_8_1_csharp_mapping_sets_the_scheme_and_the_struct_type`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`, `crates/idl-lower/tests/mapping_annotations.rs::idl4csharp_8_1_and_idl4cpp_8_1_their_annotations_reach_the_model`, `tools/idlc/src/main.rs::idl4csharp_7_1_1_the_idl4_mapping_its_naming_scheme_and_platform_are_selectable`

**Status:** `done` — the two schemes are `Naming::Idl` and `Naming::DotNet`; `--csharp-naming` selects the run's scheme and `@csharp_mapping(apply_naming_convention = ...)` overrides it per module and declaration.

### 7.1.1 Collision resolution: `@` prefix for keywords, `_` prefix for spec names

**Spec:** §7.1.1, p. 5 — "the collision shall be resolved by prepending
the '@' character [...] when the name collides with a C# language
keyword, or the '_' character when the name collides with a name
introduced by this specification."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/names.rs::escape`, `crates/lang-rules/src/emit/idl4_csharp/names.rs::escape_type`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_1_2_reserved_names_take_their_prefix`, `crates/lang-rules/src/emit/idl4_csharp/names.rs::reserved_names_take_their_prefix`

**Status:** `done` — a C# keyword takes `@` (`@class`, `@object`), the name `Constants` the spec introduces a `_`.

### 7.1.1.1 IDL naming scheme: names without case transformation

**Spec:** §7.1.1.1, p. 5 — "IDL member names and type identifiers shall
map to C# names and identifiers without case transformation."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/names.rs::Naming`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`

**Status:** `done` — under `Naming::Idl` names stay unchanged, interfaces take the `I` of Table 8.1.

### 7.1.1.2 .NET Framework Design Guidelines naming scheme

**Spec:** §7.1.1.2, p. 5 — "IDL member names and type identifiers shall
map to C# names and identifiers that follow the coding guidelines
defined in the Framework Design Guidelines of [.NET-GUIDE]."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/names.rs::Naming`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_16_a_user_annotation_is_an_attribute_class`, `crates/lang-rules/src/emit/idl4_csharp/names.rs::table_8_1_gives_each_construct_its_name`

**Status:** `done` — under `Naming::DotNet` Table 8.1 holds: Pascal case for namespaces, types, members, methods and constants, camel case for parameters, `I` before interfaces, `Flags` after bit masks, `Attribute` after annotations.

### 7.1.1.2.1 Pascal Case transformation

**Spec:** §7.1.1.2.1, p. 5 — "The first letter after each underscore
shall be capitalized and all underscores shall be removed. The first
letter of the IDL name shall be capitalized."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::pascal_case`

**Tests:** `crates/lang-rules/src/emit/idl4_java/names.rs::pascal_case_is_the_clause_examples`, `crates/lang-rules/src/emit/idl4_csharp/names.rs::table_8_1_gives_each_construct_its_name`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`

**Status:** `done` — the clause's rule and examples, which are those of IDL4 to Java §7.1.1.2.1.

### 7.1.1.2.2 Camel Case transformation

**Spec:** §7.1.1.2.2, p. 6 — "The first letter after each underscore
shall be capitalized and all underscores shall be removed. The first
letter of the IDL name shall be lower case."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::camel_case`

**Tests:** `crates/lang-rules/src/emit/idl4_java/names.rs::camel_case_is_the_clause_examples`, `crates/lang-rules/src/emit/idl4_csharp/names.rs::table_8_1_gives_each_construct_its_name`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`

**Status:** `done` — the clause's rule and examples; parameters in camel case under the .NET scheme (`ioParam`).

### 7.1.2 Reserved names (C# keywords + `Constants`)

**Spec:** §7.1.2, p. 5 — reserved are the C# keywords of ECMA-334
§7.4.4 and the class name `Constants` per module namespace.

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/names.rs::CSHARP_KEYWORDS`, `crates/lang-rules/src/emit/idl4_csharp/names.rs::CONSTANTS_CLASS`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_1_2_reserved_names_take_their_prefix`, `crates/lang-rules/src/emit/idl4_csharp/names.rs::reserved_names_take_their_prefix`

**Status:** `done` — reserved are the 77 keywords of ECMA-334 §7.4.4 and `Constants`.

### 7.1.3 Tab.7.1: C# language versions per feature

**Spec:** §7.1.3, Tab.7.1, p. 6 — minimum versions for `IList<T>`,
`IDictionary<TKey,TValue>`, system exceptions, `FlagsAttribute`,
`BitArray`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a version matrix; the generator
targets C# 12/.NET 8, but the table states no mapping requirement of its
own.

### 7.1.4 Mapping extensibility

**Spec:** §7.1.4, p. 7 — "implementers [...] may extend C# types [...]
to add new constructors and methods, override existing methods, and
implement additional interfaces."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a permission to implementers, not a
requirement (the materialiser emits `sealed` classes).

---

## §7.2 Core Data Types

### 7.2.1 IDL specification (no direct mapping)

**Spec:** §7.2.1, p. 7 — "There is no direct mapping of the IDL
Specification itself."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the clause points to the following
sections.

### 7.2.2 IDL module -> C# namespace

**Spec:** §7.2.2, p. 7 — "IDL modules shall be mapped to C# namespaces
of the same name. [...] IDL declarations not enclosed in any module
shall be mapped into the global scope."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/mod.rs::namespace_of`, `crates/lang-rules/src/emit/idl4_csharp/mod.rs::Gen::file`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`

**Status:** `done` — a module is a namespace of the same name (`MyMath` under the .NET scheme); declarations outside any module are in the global namespace.

### 7.2.3 Constants: two alternatives

**Spec:** §7.2.3, p. 7 — "two alternatives for mapping IDL constants
[...] Standalone Constants Mapping [...] Constants Container Mapping."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::constant_classes`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`

**Status:** `done` — both alternatives: the standalone mapping (the default, a decision) and the container through `@csharp_mapping(constants_container = ...)`.

### 7.2.3.1 Standalone constants: one class per constant with `Value`

**Spec:** §7.2.3.1, p. 7 — "IDL constants shall be mapped to public
static classes of the same name [...]. The mapped class shall contain a
public const called Value."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::constant_classes`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`

**Status:** `done` — a `public static class` per constant with `public const ... Value`, the example `my_math` under both schemes.

### 7.2.3.2 Constants container: `public static partial class Constants`

**Spec:** §7.2.3.2, p. 8 — "Every scope containing a constant
declaration shall contain a public static partial class. By default,
the mapped class shall be named Constants."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::constant_classes`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`

**Status:** `done` — `public static partial class` named by `constants_container`, `MathematicalConstants` in the example; the empty string switches back to standalone.

### 7.2.4.1.1 Integer types (Tab.7.2)

**Spec:** §7.2.4.1.1, Tab.7.2, p. 9 — "int8 -> sbyte; uint8 -> byte;
short/int16 -> short; unsigned short/uint16 -> ushort; long/int32 ->
int; unsigned long/uint32 -> uint; long long/int64 -> long; unsigned
long long/uint64 -> ulong."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::integer`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — Table 7.2 in full.

### 7.2.4.1.2 Floating-point types (Tab.7.3)

**Spec:** §7.2.4.1.2, Tab.7.3, p. 10 — "float -> float; double ->
double; long double -> decimal."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::primitive`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `float`, `double`, `long double` as `decimal`.

### 7.2.4.1.3 IDL char -> C# char

**Spec:** §7.2.4.1.3, p. 10 — "The IDL char type shall be mapped to the
C# type char."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::primitive`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `char` is `char`; the setter refuses a character above U+00FF.

### 7.2.4.1.4 IDL wchar -> C# char

**Spec:** §7.2.4.1.4, p. 10 — "The IDL wchar type shall be mapped to
the C# type char."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::primitive`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `wchar` is `char`.

### 7.2.4.1.5 IDL boolean -> C# bool

**Spec:** §7.2.4.1.5, p. 10 — "The IDL boolean type shall be mapped to
the C# bool, and the IDL constants TRUE and FALSE shall be mapped to
the corresponding C# boolean literals."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::primitive`, `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::cs_value`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `boolean` is `bool`, `TRUE`/`FALSE` are `true`/`false`.

### 7.2.4.1.6 IDL octet -> C# byte

**Spec:** §7.2.4.1.6, p. 10 — "The IDL type octet [...] shall be mapped
to the C# type byte."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::integer`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `octet` is `byte`.

### 7.2.4.2.1 IDL sequence -> Omg.Types.ISequence<T>

**Spec:** §7.2.4.2.1, p. 10 — "IDL sequences shall be mapped to the C#
Omg.Types.ISequence<T> interface [...]. Implementations of
Omg.Types.ISequence<T> shall extend
System.Collections.Generic.IList<T>."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`, `crates/idl-csharp/runtime/Omg.Types.cs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_2_sequences_strings_and_fixed`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`

**Status:** `done` — a sequence is `Omg.Types.ISequence<T>` with every method the clause takes from `List<T>`; the runtime has `SequenceList<T>` and `BoundedList<T>`.

### 7.2.4.2.1 Tab.7.4: sequences of basic types

**Spec:** §7.2.4.2.1, Tab.7.4, p. 11-12 — `sequence<boolean>` ->
`Omg.Types.ISequence<bool>`, ... `sequence<long double>` ->
`Omg.Types.ISequence<decimal>`.

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_2_sequences_strings_and_fixed`

**Status:** `done` — the element types of Table 7.4, `ISequence<decimal>` for `long double`.

### 7.2.4.2.1 Bounds checking on bounded sequences

**Spec:** §7.2.4.2.1, p. 12 — "Bounds checking on bounded sequences may
raise an exception if necessary." (MAY.)

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec states a may; the wire path
checks the bound on decode (`runtime/csharp`), the C# field form itself
does not.

### 7.2.4.2.1 Sequence/map members as read-only properties (with @external)

**Spec:** §7.2.4.2.1, p. 12 — "sequence members of IDL structs and
unions map to read-only properties. [...] sequences and maps that are
marked with the @external annotation shall include both a getter and a
setter."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::member`, `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::write_properties`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_2_a_union_is_a_class_with_a_discriminator_and_member_properties`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_standardized_annotations_shape_the_class`

**Status:** `done` — sequence and map members are properties without a setter, with `@external` with a setter and as `IEnumerable<T>`.

### 7.2.4.2.2 IDL string -> C# string (UTF-16)

**Spec:** §7.2.4.2.2, p. 12 — "IDL strings, both bounded and unbounded
variants, shall be mapped to C# strings. The resulting strings shall be
encoded in UTF-16 format."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_2_sequences_strings_and_fixed`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`

**Status:** `done` — `string` is `string`; the setter checks a bound.

### 7.2.4.2.3 IDL wstring -> C# string (UTF-16)

**Spec:** §7.2.4.2.3, p. 12 — as §7.2.4.2.2 for `wstring`.

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_2_sequences_strings_and_fixed`

**Status:** `done` — `wstring` is `string`.

### 7.2.4.2.4 IDL fixed -> C# decimal + ArithmeticException

**Spec:** §7.2.4.2.4, p. 12 — "The IDL fixed type shall be mapped to
the C# decimal type. Range checking shall raise a
System.ArithmeticException exception, or a derived exception, if
necessary."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::checks`, `crates/idl-csharp/runtime/Omg.Types.cs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_2_sequences_strings_and_fixed`

**Status:** `done` — `fixed<P,S>` is `decimal`; `Omg.Types.Fixed.Check` throws `System.ArithmeticException` for too many digits before or after the point.

### 7.2.4.3.1 IDL struct -> C# public class

**Spec:** §7.2.4.3.1, p. 13 — "An IDL struct shall be mapped to a C#
public class [...] A public property [...] A public default
constructor [...] A public copy constructor [...] A public
constructor that accepts parameters for each member [...] The class
shall implement the IEquatable<T> interface."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::structure`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`

**Status:** `done` — a `public class` with properties, the default, copy and all-values constructors (the copy deep) and `IEquatable<T>`, the example `MyStruct` under both schemes.

### 7.2.4.3.2 IDL union -> C# public class (discriminator protocol)

**Spec:** §7.2.4.3.2, p. 14-15 — "A public read-only property named
Discriminator [...] A public property with getters and setters for each
member [...] Property getters shall raise a System.InvalidOperationException
if the expected member has not been set [...] implement the
IEquatable<T> interface."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::union`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_2_a_union_is_a_class_with_a_discriminator_and_member_properties`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_2_int8_uint8_wchar_and_octet_discriminate_a_union`

**Status:** `done` — the example `AUnion`: `Discriminator` read-only, getters with `InvalidOperationException`, setters on the first label, `Set<Member>(value, discriminator)` with `ArgumentException`, the two `Set<Member>` of a sequence, the default member at the first free value from zero, `IEquatable<T>`. The clause writes `System.Collections.IEnumerable<T>`; it is the generic `System.Collections.Generic.IEnumerable<T>`.

### 7.2.4.3.3 IDL enum -> C# public enum

**Spec:** §7.2.4.3.3, p. 18 — "An IDL enum shall be mapped to a C#
public enum with the same name as the IDL enum type."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::enumeration`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_3_an_enumeration_is_a_csharp_enum_of_its_bit_bound`

**Status:** `done` — a C# `enum` with the `@value`s.

### 7.2.4.3.3 @bit_bound underlying type for enums

**Spec:** §7.2.4.3.3, p. 18 — "the corresponding C# enum type shall be
sbyte for bit bound values between 1 and 8; short, for [...] 9 and 16;
int, for [...] 17 and 32; and long, for [...] 33 and 64."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::enumeration`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_3_an_enumeration_is_a_csharp_enum_of_its_bit_bound`

**Status:** `done` — `@bit_bound` selects `sbyte`, `short`, `int` or `long`; without `@bit_bound` it stays `int`. The front end refuses a bound above 32 in the source under DDS-XTypes 1.3 §7.3.1.2.1.5 ("from 1 to 32, inclusive, when this annotation is applied to an enumerated type"); the test checks that refusal and reaches the `long` range through the bound 40 in the model's `@bit_bound` facet, which is what the generator reads.

### 7.2.4.3.4 Constructed recursive types

**Spec:** §7.2.4.3.4, p. 18 — "Constructed recursive types are
supported by mapping the involved types directly to C#."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_4_a_recursive_type_names_its_own_class`

**Status:** `done` — a structure holding a sequence of itself; copy and comparison go through.

### 7.2.4.4 IDL array -> C# array + bounds check

**Spec:** §7.2.4.4, p. 19 — "An IDL array shall be mapped to a C# array
[...]. The bounds for the array shall be checked by the setter of the
corresponding property and a System.ArgumentOutOfRangeException shall
be raised if a bounds violation occurs."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::checks`, `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::array_initial`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_4_an_array_is_a_csharp_array_checked_by_its_setter`

**Status:** `done` — a C# array (rectangular for several dimensions) at its size with constructed elements; the setter throws `ArgumentOutOfRangeException` for another size, the example `MyType`.

### 7.2.4.5 Native types

**Spec:** §7.2.4.5, p. 21 — "This language mapping specification does
not define any native types, but compliant implementations may provide
the necessary mechanisms to map native types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec defines no native types; the
materialiser optionally maps `native` to the facet name or `object`
(permissive).

### 7.2.4.6 Naming data types (typedef)

**Spec:** §7.2.4.6, p. 21 — "the declaration of types using typedef in
IDL shall not result in the creation of any C# type. Instead, the use
of an IDL typedef type shall be replaced with the type referenced by
the typedef statement."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_6_a_typedef_is_replaced_and_its_annotations_reach_its_uses`

**Status:** `done` — no C# type, every use carries the target type.

---

## §7.3 Any

### 7.3 IDL any -> Omg.Types.Any

**Spec:** §7.3, p. 22 — "The IDL any type shall be mapped to
Omg.Types.Any type."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`, `crates/idl-csharp/runtime/Omg.Types.cs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_3_any_is_omg_types_any`

**Status:** `done` — `any` is `Omg.Types.Any` with `Insert`, `Extract<T>` and `Type`; on the CORBA platform `Corba.Any` (A.1.4).

---

## §7.4 Interfaces – Basic

### 7.4 IDL interface -> C# public interface

**Spec:** §7.4, p. 22 — "Each IDL interface shall be mapped to a C#
public interface with the same name [...] prepending the "I" prefix.
[...] If the IDL interface derives from other IDL interfaces, the
equivalent C# interface shall be declared to extend the C# interfaces
[...]. Each attribute [...] shall map to a property [...] Each
operation [...] shall map to a method [...] IDL out arguments shall be
mapped to C# out parameters, inout to ref, and in to parameters without
modifier."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::top_level_interface`, `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::operation_method`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`

**Status:** `done` — the example `AnInterface`: `IAnInterface`, bases as base interfaces, attributes as properties (no setter for `readonly`), `out` as `out`, `inout` as `ref`.

### 7.4.1 IDL exception -> C# class : System.Exception

**Spec:** §7.4.1, p. 23 — "An IDL exception shall be mapped to a C#
class [...]. The mapped class shall extend the System.Exception class."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::exception`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`, `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_a_1_1_and_a_1_5_exceptions_and_interfaces_on_the_corba_platform`

**Status:** `done` — a class extending `System.Exception` with properties and the three constructors; a property that would hide a member of `System.Exception` takes `_`.

### 7.4.1 Exception class derives from System.Exception

**Spec:** §7.4.1, p. 23 — "The mapped class shall extend the System.Exception class."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::exception`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`, `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_a_1_1_and_a_1_5_exceptions_and_interfaces_on_the_corba_platform`

**Status:** `done` — the class extends `System.Exception`, on the CORBA platform `Corba.UserException`.

### 7.4.2 Interface forward declaration

**Spec:** §7.4.2, p. 23 — "An IDL interface forward declaration has no
mapping to the C# language."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::top_level_interface`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`

**Status:** `done` — a forward declaration produces no file; the definition has its one.

---

## §7.5 Interfaces – Full

### 7.5 I<Name>Operations + I<Name> + <Name> class

**Spec:** §7.5, p. 23-24 — "A C# public interface named
I<InterfaceName>Operations [...] A C# public interface named
I<InterfaceName> inheriting from I<InterfaceName>Operations [...] A C#
public class named <InterfaceName> containing the declaration of the
classes and exceptions that result from the mapping of all the types
and exceptions declared within the IDL interface."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::top_level_interface`, `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::embedded_declarations`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_5_a_full_interface_is_operations_interface_and_class`

**Status:** `done` — the example `FullInterface`: `IFullInterfaceOperations`, `IFullInterface` and the class `FullInterface` with `S` and `PI`; the class implements `IFullInterface` abstractly (a decision: the clause leaves the inheritance open, and implementing without a body is possible in C# only abstractly).

---

## §7.6 Value Types

### 7.6 IDL valuetype -> abstract class + factory attribute

**Spec:** §7.6, p. 25 — "An IDL valuetype type shall be mapped to a C#
abstract class. [...] private members shall have the C# protected
access modifier [...] Each valuetype factory operation shall be mapped
[...] to a method returning void [...] annotated with the
FactoryAttribute."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::valuetype`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_6_a_value_type_is_an_abstract_class`

**Status:** `done` — the examples `VT1` and `VT2`: abstract classes, private members `protected`, operations abstract, the factory `void` with `[Omg.Types.Factory]`, the base and the supported interface.

---

## §7.7-§7.13 CORBA, Components, Template Modules

### 7.7/7.8 CORBA-specific interfaces and value types

**Spec:** §7.7/§7.8, p. 27 — "CORBA-specific mappings are defined in
Clause A.1 of Annex A."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a reference to Annex A.1; the CORBA
mapping itself is under A.1.

### 7.9-7.13 Components (basic/homes/ports/connectors) and template modules

**Spec:** §7.9-§7.13, p. 27 — "Basic components have no direct language
mapping; they shall be mapped to intermediate IDL [...]. Template
module instances have no direct language mapping."

**Repo:** `crates/lang-rules/src/emit/intermediate_idl.rs::intermediate_idl`, `crates/idl-lower/src/lower.rs::template_module_inst`

**Tests:** `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_7_9_to_7_13_components_homes_connectors_and_template_instances`

**Status:** `done` — components, homes, ports and connectors through the equivalent IDL of CCM 4.0 §6, a template module instance as a namespace with the parameters substituted.

### 7.9-7.12 Components, homes, ports and connectors as intermediate IDL

**Spec:** §7.9–§7.12, p. 27 — "Basic components have no direct language mapping; they shall be mapped to intermediate IDL"; likewise homes, ports and connectors.

**Repo:** `crates/lang-rules/src/emit/intermediate_idl.rs::intermediate_idl`, `crates/ccm/src/transform.rs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_7_9_to_7_13_components_homes_connectors_and_template_instances`, `crates/ccm/tests/equivalent_idl.rs::idl_7_4_11_4_2_an_extended_port_is_the_basic_ports_of_its_port_type`, `crates/ccm/tests/equivalent_idl.rs::idl_7_4_11_4_3_a_connector_maps_through_its_ports_as_a_component`

**Status:** `done` — `ICashier : Components.ICCMObject` with `provide_`, `connect_`, `subscribe_`, `get_consumer_` and the basic ports of the port type; the home as explicit, implicit and equivalent; the connector through its ports. The `Components` declarations come from the application's IDL (test: a transcription of CCM 4.0 §6).

---

## §7.14 Extended Data Types

### 7.14.1 Struct with single inheritance -> C# extends

**Spec:** §7.14.1, p. 27 — "An IDL struct that inherits from a base IDL
struct, shall be declared as a C# public class that extends the class
resulting from mapping the base IDL struct."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::structure`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_1_a_derived_structure_extends_its_base`

**Status:** `done` — the example `ChildStruct : MyStruct`: the class extends the base's, the all values constructor takes the base object first; the copy constructor calls the base's copy constructor (a decision, the same effect as the base's all values constructor).

### 7.14.2 Union discriminators: int8, uint8, wchar, octet

**Spec:** §7.14.2, p. 28 — "This building block adds the int8, uint8,
wchar, and octet IDL types to the set of valid types for a
discriminator."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::union`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_2_int8_uint8_wchar_and_octet_discriminate_a_union`

**Status:** `done` — `int8`, `uint8`, `wchar` and `octet` discriminate as §7.2.4.3.2 says.

### 7.14.3.1 IDL map -> IDictionary<TKey,TValue>

**Spec:** §7.14.3.1, p. 28 — "An IDL map shall be mapped to a C#
generic System.Collections.Generic.IDictionary<TKey,TValue>."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`, `crates/idl-csharp/runtime/Omg.Types.cs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_3_1_a_map_is_an_idictionary`

**Status:** `done` — `IDictionary<TKey, TValue>` as a property without a setter; a bounded map is `Omg.Types.BoundedDictionary` and throws `ArgumentOutOfRangeException`.

### 7.14.3.2 IDL bitset -> C# struct + IEquatable

**Spec:** §7.14.3.2, p. 29 — "An IDL bitset shall map to a C# struct
with public properties for each named bitfield [...]. The mapped C#
struct shall implement the IEquatable<T> interface."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::bitset`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_3_2_a_bit_set_is_a_struct`

**Status:** `done` — the example `MyBitset`: a `struct` with properties of the clause's types and `IEquatable<T>`; the setter refuses a value that does not fit the bits.

### 7.14.3.3 IDL bitmask -> C# enum `[Flags]` + BitArray

**Spec:** §7.14.3.3, p. 30 — "The IDL bitmask type shall map to a C#
public enum with the same name, followed by the "Flags" suffix. [...] a
System.Collections.BitArray shall be used. The C# enum shall have the
System.FlagsAttribute [...]. The size [...] determines the
corresponding C# enum type."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::bitmask`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_3_3_a_bit_mask_is_a_flags_enum_and_a_bit_array`

**Status:** `done` — `<Name>Flags` with `[Flags]`, `1 << position`, the type by `@bit_bound`, and `System.Collections.BitArray` for a member.

---

## §7.15 Anonymous Types

### 7.15 Anonymous types: no impact

**Spec:** §7.15, p. 31 — "No impact to the C# language mapping."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the clause states no requirement.

---

## §7.16 Annotations

### 7.16.1 Defining annotations -> attribute class

**Spec:** §7.16.1, p. 31 — "User-defined annotations are propagated to
the generated code as C# attributes inheriting from the System.Attribute
class. [...] named [...] appending the "Attribute" suffix [...] shall
have a public constructor with default values [...] [AttributeUsage(
AttributeTargets.All, AllowMultiple = true)]."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::annotation_type`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_16_a_user_annotation_is_an_attribute_class`

**Status:** `done` — an attribute class with `[AttributeUsage(AttributeTargets.All, AllowMultiple = true)]`, properties, the defaults in the default constructor; `MyAnnotationAttribute` under the .NET scheme.

### 7.16.2 Applying annotations

**Spec:** §7.16.2, p. 32 — "IDL elements annotated with user-defined
annotations shall map to equivalent C# elements annotated with the
corresponding attribute."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::user_annotations`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_16_a_user_annotation_is_an_attribute_class`

**Status:** `done` — an annotated element carries the attribute, two applications two attributes.

### 7.16.2.1 Annotations on typedefs propagate

**Spec:** §7.16.2.1, p. 33 — "Annotations on an IDL typedef shall be
applied to uses of the typedef in other type declarations."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::member_annotations`, `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_6_a_typedef_is_replaced_and_its_annotations_reach_its_uses`

**Status:** `done` — `@max(100)` on the `typedef` is checked by the setter of every use, as a sequence element too; a user annotation on the `typedef` is carried by the member.

---

## §7.17 Standardized Annotations

### 7.17.1 @id, @autoid (no impact)

**Spec:** §7.17.1, Tab.7.5, p. 33-35 — "@id No impact on language
mapping; @autoid No impact on language mapping."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::member`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — the class with `@id` and `@autoid` is the one without.

### 7.17.1 @optional -> Nullable<T> + OptionalAttribute

**Spec:** §7.17.1, Tab.7.5, p. 34 — "IDL elements annotated with
@optional whose type T maps to a C# value type shall map to
System.Nullable<T>. [...] All mapped optional elements shall be
annotated with OptionalAttribute."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::member`, `crates/idl-csharp/runtime/Omg.Types.cs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_standardized_annotations_shape_the_class`

**Status:** `done` — `System.Nullable<T>` for a value type, a reference type unchanged, `[Omg.Types.Optional]` on the property and on the all values constructor's parameter.

### 7.17.1 @position, @value

**Spec:** §7.17.1, Tab.7.5, p. 35 — "@position Impacts the mapping of
bitmask types [...]; @value Impacts the mapping of enum types."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::enumeration`, `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::bitmask`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_3_an_enumeration_is_a_csharp_enum_of_its_bit_bound`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_3_3_a_bit_mask_is_a_flags_enum_and_a_bit_array`

**Status:** `done` — `@value` sets enum values, `@position` flag values.

### 7.17.1 @extensibility, @final, @mutable, @appendable (no impact)

**Spec:** §7.17.1, Tab.7.5, p. 35 — "No impact on language mapping."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::structure`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — the class with any extensibility annotation is the one without.

### 7.17.2 @key -> KeyAttribute

**Spec:** §7.17.2, Tab.7.6, p. 35 — "IDL elements annotated with @key
shall result in equivalent C# elements annotated with KeyAttribute."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::member`, `crates/idl-csharp/runtime/Omg.Types.cs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_standardized_annotations_shape_the_class`

**Status:** `done` — `[Omg.Types.Key]` on the property.

### 7.17.2 @must_understand (no impact)

**Spec:** §7.17.2, Tab.7.6, p. 35 — "@must_understand No impact on
language mapping."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::member`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — the class with `@must_understand` is the one without.

### 7.17.2 @default_literal

**Spec:** §7.17.2, Tab.7.6, p. 35 — "The C# element [...] shall be
initialized to element indicated by the annotation."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::default_enumerator`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_standardized_annotations_shape_the_class`

**Status:** `done` — the default constructor sets the enumerator `@default_literal` names.

### 7.17.3 Units and ranges: @default, @range, @min, @max, @unit

**Spec:** §7.17.3, Tab.7.7, p. 37 — "@default [...] initialized to the
value of the annotation; @range/@min/@max [...] shall throw a
System.ArgumentOutOfRangeException [...]; @unit Shall result in the
addition of a UnitAttribute."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::member`, `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::range_checks`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_standardized_annotations_shape_the_class`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_6_a_typedef_is_replaced_and_its_annotations_reach_its_uses`

**Status:** `done` — `@default` is the default constructor's value, `@range`/`@min`/`@max` throw `ArgumentOutOfRangeException` in the setter, `@unit` is `[Omg.Types.Unit("m")]`.

### 7.17.4 @bit_bound

**Spec:** §7.17.4, Tab.7.8, p. 38 — "@bit_bound Impacts the mapping of
bitmask as described in Clause 7.14.3.3."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::bitmask`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_3_3_a_bit_mask_is_a_flags_enum_and_a_bit_array`

**Status:** `done` — the type of the flags enum follows `@bit_bound`.

### 7.17.4 @external

**Spec:** §7.17.4, Tab.7.8, p. 38 — "@external [...] map to
System.Nullable<T> [...] Properties created as a result of mapping IDL
sequences annotated with @external shall map to the C#
System.Collections.Generic.IEnumerable<T> interface."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::member`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_standardized_annotations_shape_the_class`

**Status:** `done` — `System.Nullable<T>` for a value type, `IEnumerable<T>` with a setter for a sequence, copies shallow.

### 7.17.4 @nested (no impact)

**Spec:** §7.17.4, Tab.7.8, p. 38 — "@nested No impact to the language
mapping."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::structure`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — the class with `@nested` is the one without.

### 7.17.5 @verbatim

**Spec:** §7.17.5, Tab.7.9, p. 38 — "Copies verbatim text to the
indicated output position when the indicated language is "*", "c#",
"cs", or "csharp"."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::structure`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_5_verbatim_text_is_copied_for_its_four_names`

**Status:** `done` — text for `*`, `c#`, `cs` and `csharp` is in its place, text for another language is not.

### 7.17.6 @service, @oneway, @ami

**Spec:** §7.17.6, Tab.7.10, p. 38 — "Options are "CORBA", "DDS", "*".
Impact is platform-specific."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec declares the impact
platform-specific; the materialiser documents `raises`/`oneway` only,
without a C# clause.

---

## §8 IDL to C# Language Mapping Annotations

### 8.1 @csharp_mapping annotation

**Spec:** §8.1, p. 41 — "@annotation csharp_mapping { enum
NamingConvention {IDL_NAMING_CONVENTION, DOTNET_NAMING_CONVENTION};
NamingConvention apply_naming_convention; string constants_container
default "Constants"; string struct_type default "class"; }"

**Repo:** `crates/ast/src/facet/mapping.rs`, `crates/idl-lower/src/annotation.rs::mapping_annotation`

**Tests:** `crates/idl-lower/tests/mapping_annotations.rs::idl4csharp_8_1_and_idl4cpp_8_1_their_annotations_reach_the_model`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_8_1_csharp_mapping_sets_the_scheme_and_the_struct_type`

**Status:** `done` — `@csharp_mapping` is built in, checked against the clause's declaration (`IDL0007`), and reaches the model as `Facets::mappings`.

### 8.1.1 apply_naming_convention

**Spec:** §8.1.1, p. 41 — "If apply_naming_convention is
IDL_NAMING_CONVENTION [...] If [...] DOTNET_NAMING_CONVENTION [...]."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/names.rs::Naming`, `crates/lang-rules/src/emit/idl4_csharp/mod.rs::Settings::apply`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_8_1_csharp_mapping_sets_the_scheme_and_the_struct_type`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`

**Status:** `done` — both columns of Table 8.1; a setting on a declaration wins over its module's.

### 8.1.2 constants_container

**Spec:** §8.1.2, p. 42 — "To enable the Standalone Constants Mapping
[...] constants_container shall be set to an empty string. To enable
the Constants Container Mapping [...] set to a valid string."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::constant_classes`, `crates/lang-rules/src/emit/idl4_csharp/mod.rs::Settings::apply`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`

**Status:** `done` — a name switches the container on, the empty string the standalone mapping.

### 8.1.3 struct_type

**Spec:** §8.1.3, p. 43 — "struct_type defines the C# type the IDL
struct type map to. By default [...] C# class. This parameter allows
changing the default behavior to map an IDL struct to a C# struct."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::structure`, `crates/lang-rules/src/emit/idl4_csharp/mod.rs::Settings::apply`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_8_1_csharp_mapping_sets_the_scheme_and_the_struct_type`

**Status:** `done` — `struct_type = "struct"` produces a C# `struct`, copied by value.

---

## Annex A: Platform-Specific Mappings

### A.1 CORBA-specific mappings (A.1.1-A.1.6)

**Spec:** Annex A.1, p. 45-48 — `Corba.UserException` for exceptions,
`Corba.TypeCode`, `Corba.Object`/`Corba.IObject`, `Corba.Any`, the
`Corba.IObject` inheritance for interfaces and the value-type rules.

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/mod.rs::Platform`, `crates/lang-rules/src/emit/idl4_csharp/mod.rs::namespace_of`, `crates/idl-csharp/runtime/Corba.cs`, `runtime/corba/CORBA_PIDL.idl`

**Tests:** `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_a_1_2_the_corba_module_is_the_corba_namespace_under_the_idl_naming_scheme`, `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_a_1_typecode_object_any_and_user_exception_are_the_annexs_classes`, `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_a_1_1_and_a_1_5_exceptions_and_interfaces_on_the_corba_platform`

**Status:** `done` — `--csharp-platform corba`: exceptions extend `Corba.UserException`, `TypeCode`, `IObject`/`Object` and `Any` are the annex's classes with the behaviour of CORBA 3.3 part 1 §8.11.1, interfaces extend `Corba.IObject`, their classes `Corba.Object`; `--csharp-corba-module` writes the C# of the CORBA module (namespace `Corba`, IDL scheme). Native types of the CORBA module are `object`; an interface operation with the name and parameters of an operation of `Object` but another result is implemented explicitly (decisions).

### A.2 DDS-specific mappings

**Spec:** Annex A.2, p. 49 — "DDS requires no additional
platform-specific language mappings."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the clause states no additional
requirement.

### Annex B: Building Block Traceability Matrix

**Spec:** Annex B, p. 51 — non-normative matrix.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — expressly non-normative.

---

## Audit-Status

65 done / 0 partial / 0 open / 0 blocked / 16 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9310 of `main` (`python3 ci/coverage_results.py --document idl4-csharp-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-ccm --all-targets` (job `merge-rust-ccm`) — 2 tests green.
* `cargo nextest run --profile merge -p zerodds-idl-lower --all-targets` (job `merge-rust-idl-lower`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-idlc --all-targets` (job `merge-rust-idlc`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-lang-rules --all-targets` (job `merge-rust-lang-rules`) — 4 tests green.
* `python3 ci/run_integration.py merge-codegen-csharp` (job `merge-codegen-csharp`) — 28 tests green.

36 cited tests green, 0 not green.

Open items: none.
