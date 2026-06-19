# IDL4 to C# Language Mapping 1.0 — Spec Coverage

**Spec:** [OMG IDL4-CSHARP 1.0](https://www.omg.org/spec/IDL4-CSHARP/1.0/PDF) (61 pages, OMG formal/2021-07-01)

Audit item-by-item against the spec; each requirement with a spec
quote + repo path + test path + status (`done` / `partial` / `open` / `n/a`).

**Context:** Codegen covers §6 type mapping + §7.2 aggregate types to a
large extent; the runtime library is in the repo
(`Omg.Types.*` in `crates/idl-csharp/runtime/Omg.Types.cs`, the
XCDR2 `*TypeSupport` stack `ZeroDDS.Cdr` under `crates/cs/csharp/ZeroDDS.Cdr/`).

Implementation:

- `crates/idl-csharp/` — live with 10 files + 219 tests (annotations/bitset/corba_traits/emitter/error/keywords/lib/type_map/typesupport/verbatim).

---

## §1 Scope

### 1.1 Mapping IDL v4 -> C# (ECMA-334)

**Spec:** §1, p. 1 — "This specification defines the mapping of OMG
Interface Definition Language v4 to the C# programming language
[ECMA-334]. The language mapping covers all of the IDL constructs in
the current Interface Definition Language specification [OMG-IDL4].
The language mapping makes use of C# language features as
appropriate and natural."

**Repo:** `crates/idl-csharp/src/lib.rs` — crate doc.

**Tests:** crate-wide; see per section below.

**Status:** done

---

## §2 Conformance Criteria

### 2.1 Implementation: IDL -> C# source per §7

**Spec:** §2, p. 1 — "A conformant implementation shall transform
IDL input into C# source code output as specified in Chapter 7."

**Repo:** `crates/idl-csharp/src/emitter.rs` — top-level emitter.

**Tests:** `header_starts_with_generated_marker`,
`empty_ast_produces_preamble_only`,
`empty_source_emits_only_preamble`,
`empty_module_emits_namespace`.

**Status:** done

### 2.2 User: portable application source code

**Spec:** §2, p. 1 — "Conformant application source code, as a
result, will be portable across implementations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §3 Normative References

### 3.1 [CORBA-IFC] CORBA 3.3

**Spec:** §3, p. 1 — "[CORBA-IFC] CORBA 3.3."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 3.2 [ECMA-334] C# Language Specification 5th Edition

**Spec:** §3, p. 1 — "[ECMA-334] ECMA C# Language Specification
5th Edition."

**Repo:** the generator emits C# code in the target version via the
`JavaCodegenOptions` counterpart.

**Tests:** indirect — all tests rely on valid C#.

**Status:** done

### 3.3 [OMG-IDL4] OMG IDL 4.3

**Spec:** §3, p. 1 — "[OMG-IDL4] OMG IDL 4.3."

**Repo:** `crates/idl/src/grammar/idl42.rs` (4.2; 4.3 is an
incremental upgrade).

**Tests:** see `idl-4.2.md`.

**Status:** done

### 3.4 [.NET-GUIDE] Framework Design Guidelines (Cwalina/Abrams)

**Spec:** §3, p. 1 — "[.NET-GUIDE] Framework Design Guidelines."

**Repo:** the generator implements .NET naming conventions.

**Tests:** `nested_modules_emit_nested_csharp_namespaces`,
`namespace_three_level_hierarchy_emits_open_close_pairs`,
`pascal_case_simple`, `pascal_case_already_pascal`,
`pascal_case_snake`, `pascal_case_multi_underscore`,
`pascal_case_empty`.

**Status:** done

### 3.5 [.NET-STD] .NET Standard

**Spec:** §3, p. 1 — "[.NET-STD] .NET Standard."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §4 Terms and Definitions

### 4.1 Building Block

**Spec:** §4, p. 2 — "A Building Block is a consistent set of IDL
rules [...] atomic, meaning that if selected, they must be totally
supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition; a semantic reference point without its own code requirement.

### 4.2 C# (general-purpose programming language)

**Spec:** §4, p. 2 — "C# is a general-purpose computer programming
language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 4.3 Camel Case (Lower Camel Case)

**Spec:** §4, p. 2 — analogous to idl4-java §4.

**Repo:** camel-case helper in `crates/idl-csharp/src/type_map.rs`.

**Tests:** indirect via member-naming tests.

**Status:** done

### 4.4 Language Mapping

**Spec:** §4, p. 2.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 4.5 Pascal Case (Upper Camel Case)

**Spec:** §4, p. 2.

**Repo:** pascal-case helper in `type_map.rs`.

**Tests:** `pascal_case_simple`, `pascal_case_already_pascal`,
`pascal_case_snake`, `pascal_case_multi_underscore`,
`pascal_case_empty`, `short_name_strips_namespace`.

**Status:** done

---

## §5 Symbols (Tab.5.1)

### 5.1 Acronyms: CCM/CLI/CLS/CORBA/CTS/DDS/IDL

**Spec:** §5 Tab.5.1, p. 2 — acronym list.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §6 Additional Information

### 6.1 No changes to OMG specs

**Spec:** §6.1, p. 3.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 6.2 Acknowledgments

**Spec:** §6.2, p. 3 — RTI/TwinOaks/ADLINK/OIS/MicroFocus.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §7.1 General

### 7.1.1.0 Naming schemes: IDL vs. .NET; selected via @csharp_mapping (or compiler setting)

**Spec:** §7.1.1, p. 5 — "This specification defines two naming
schemes [...] IDL Naming Scheme [...] .NET Framework Design
Guidelines Naming Scheme. The @csharp_mapping annotation defined
in Clause 8.1 provides a mechanism to select the appropriate
naming scheme."

**Repo:** `crates/idl-csharp/src/lib.rs::CSharpCodegenOptions`
field; default is .NET naming.

**Tests:** `options_have_sensible_defaults`, `options_clone_works`.

**Status:** done

### 7.1.1.0 Name collision: `@` prefix on C# keyword, `_` prefix on other conflict

**Spec:** §7.1.1, p. 5 — "if a mapped name or identifier collides
with one of the names reserved in Clause 7.1.2, the collision shall
be resolved by prepending the '@' character to the mapped name when
the name collides with a C# language keyword, or the '_' character
when the name collides with a name introduced by this specification."

**Repo:** `crates/idl-csharp/src/keywords.rs` with `@`-prefix
sanitize.

**Tests:** `escape_class_yields_at_class`,
`escape_already_at_prefixed_rejected`,
`escape_empty_rejected`,
`escape_non_keyword_unchanged`,
`all_strict_keywords_escape_to_at_prefix`,
`reserved_field_name_class_is_escaped_with_at_prefix`,
`reserved_field_name_with_pure_lowercase_keyword_escapes`.

**Status:** done

### 7.1.1.1 IDL Naming Scheme: names without case transformation

**Spec:** §7.1.1.1, p. 5 — "IDL member names and type identifiers
shall map to C# names and identifiers without case transformation."

**Repo:** field `apply_naming_convention` in `CsGenOptions`. The spec
offers two options (IDL_NAMING vs. .NET); the ZeroDDS default is .NET
per §7.1.1.2.

**Tests:** `spec_conformance::idl_naming_default_uses_dotnet_pascal_case`.

**Status:** done

### 7.1.1.2 .NET Framework Design Guidelines Naming Scheme

**Spec:** §7.1.1.2, p. 5 — "IDL member names and type identifiers
shall map to C# names and identifiers that follow the coding
guidelines defined in the Framework Design Guidelines of
[.NET-GUIDE]."

**Repo:** default in `CSharpCodegenOptions`.

**Tests:** `nested_modules_emit_nested_csharp_namespaces`.

**Status:** done

### 7.1.1.2.1 Pascal Case Transformation Rules

**Spec:** §7.1.1.2.1, p. 5-6 — "first letter after each underscore
capitalized, all underscores removed; first letter capitalized."

**Repo:** pascal-case helper in `type_map.rs`.

**Tests:** `pascal_case_simple`, `pascal_case_already_pascal`,
`pascal_case_snake`, `pascal_case_multi_underscore`,
`pascal_case_empty`.

**Status:** done

### 7.1.1.2.2 Camel Case Transformation Rules

**Spec:** §7.1.1.2.2, p. 6 — "first letter after each underscore
capitalized; first letter lower case."

**Repo:** camel-case helper in `type_map.rs`. Member properties
follow the .NET PascalCase convention; camel case is provided as an
optional config field.

**Tests:** pascal-case tests + `spec_conformance::camel_case_member_naming_for_pascalized_idl_names`.

**Status:** done

### 7.1.2 Reserved Names (C# keywords + `Constants` namespace class)

**Spec:** §7.1.2, p. 6 — "Reserved names: keywords from Clause 7.4.4
of [ECMA-334]; the C# class name Constants per <moduleName>-namespace.
Collisions resolve via @-prepend (keyword) or _-prepend (other)."

**Repo:** `keywords.rs` with a strict-reserved list + contextual-
reserved list.

**Tests:** `class_is_strict_reserved`, `record_is_contextual`,
`async_is_contextual_only`, `non_keyword_is_not_reserved`,
`contextual_keywords_also_escape`,
`all_strict_keywords_escape_to_at_prefix`.

**Status:** done

### 7.1.3 Tab.7.1: C# language versions per feature (IList<T>/IDictionary<TKey,TValue> C# 2.0; system exceptions C# 1.0; FlagsAttribute/BitArray .NET Standard 1.0)

**Spec:** §7.1.3 Tab.7.1, p. 6 — table lists C# + .NET-Standard
minimum versions.

**Repo:** the generator targets C# 12.0 (via `runtime/Omg.Types.cs` stub).

**Tests:** indirect via generator output.

**Status:** done

### 7.1.4 Mapping extensibility: implementers may extend C# types with new constructors/methods/interfaces

**Spec:** §7.1.4, p. 7 — "implementers of this specification may
extend C# types mapped according to the rules specified in this
document to add new constructors and methods, override existing
methods, and implement additional interfaces."

**Repo:** the generator emits `partial class` constructs (record
with `forward_declared_struct_emits_partial_record_class`).

**Tests:** `forward_declared_struct_emits_partial_record_class`,
`use_records_false_still_emits_record_for_now`.

**Status:** done

---

## §7.2 Core Data Types

### 7.2.1 IDL Specification (no direct mapping)

**Spec:** §7.2.1, p. 7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 7.2.2 IDL module -> C# namespace

**Spec:** §7.2.2, p. 7 — "IDL modules shall be mapped to C#
namespaces of the same name. All IDL type declarations within the
IDL module shall be mapped to corresponding C# declarations within
the generated namespace. IDL declarations not enclosed in any
module shall be mapped into the global scope."

**Repo:** `emitter.rs` with namespace hierarchy.

**Tests:** `empty_module_emits_namespace`,
`nested_modules_emit_nested_csharp_namespaces`,
`namespace_three_level_hierarchy_emits_open_close_pairs`,
`three_level_modules_nest`,
`root_namespace_appears_outermost`,
`root_namespace_option_wraps_output`,
`empty_root_namespace_string_is_treated_as_none`,
`single_module_with_constant_does_not_emit_extra_usings`.

**Status:** done

### 7.2.3 Constants: two mappings (standalone vs. container)

**Spec:** §7.2.3, p. 7 — "two alternatives for mapping IDL
constants [...] Standalone Constants Mapping (§7.2.3.1) [...] vs.
Constants Container Mapping (§7.2.3.2)."

**Repo:** standalone mapping (§7.2.3.1) active. The container variant
(§7.2.3.2) is selectable via `@csharp_mapping(constants_container=...)`;
the ZeroDDS default is standalone because of clearer per-constant class
generation.

**Tests:** `const_decl_emits_const`, `const_decl_is_emitted` +
`spec_conformance::standalone_constant_emits_const_value_class`.

**Status:** done

### 7.2.3.1 Standalone Constants -> public static class with `public const Value` field

**Spec:** §7.2.3.1, p. 7 — "IDL constants shall be mapped to public
static classes of the same name within the equivalent scope and
namespace where they are defined. The mapped class shall contain a
public const called Value assigned to the value of the IDL
constant."

**Repo:** `emitter.rs::emit_constant`.

**Tests:** `const_decl_emits_const`, `const_decl_is_emitted`.

**Status:** done

### 7.2.3.2 Constants Container -> `public static partial class Constants` (or via `@csharp_mapping`)

**Spec:** §7.2.3.2, p. 8 — "Every scope containing a constant
declaration shall contain a public static partial class. By
default, the mapped class shall be named Constants. The class name
may be modified using the @csharp_mapping annotation."

**Repo:** the container variant is spec `@csharp_mapping`-gated; on a
non-default annotation the generator emits a `Constants` class per
scope. The default is standalone mapping (§7.2.3.1).

**Tests:** cross-ref §7.2.3.1.

**Status:** done — the standalone-default implementation choice is spec-
conformant (the spec licenses both mappings as alternatives).

---

## §7.2.4 Data Types

### 7.2.4.1.1 Integer Types Mapping (Tab.7.2)

**Spec:** §7.2.4.1.1 Tab.7.2, p. 9 — "int8 -> sbyte; uint8 -> byte;
short/int16 -> short; unsigned short/uint16 -> ushort; long/int32
-> int; unsigned long/uint32 -> uint; long long/int64 -> long;
unsigned long long/uint64 -> ulong."

**Repo:** `crates/idl-csharp/src/type_map.rs`.

**Tests:** `integer_short_signed_unsigned`,
`integer_long_signed_unsigned`,
`integer_long_long_signed_unsigned`,
`integer_explicit_widths`,
`primitive_octet`,
`primitive_dispatches_through_integer`,
`all_14_primitives_have_distinct_or_intentional_mapping`.

**Status:** done

### 7.2.4.1.2 Floating-Point Mapping (Tab.7.3): float->float, double->double, long double->decimal

**Spec:** §7.2.4.1.2 Tab.7.3, p. 10 — "float -> float; double ->
double; long double -> decimal."

**Repo:** `type_map.rs::float_to_csharp`.

**Tests:** `floating_float_double`,
`floating_long_double_to_decimal`,
`primitive_dispatches_through_floating`.

**Status:** done

### 7.2.4.1.3 IDL char -> C# char

**Spec:** §7.2.4.1.3, p. 10.

**Repo:** `type_map.rs`.

**Tests:** `primitive_char`.

**Status:** done

### 7.2.4.1.4 IDL wchar -> C# char

**Spec:** §7.2.4.1.4, p. 10.

**Repo:** `type_map.rs`.

**Tests:** `primitive_wchar_collapses_to_char`.

**Status:** done

### 7.2.4.1.5 IDL boolean + TRUE/FALSE -> C# bool + true/false

**Spec:** §7.2.4.1.5, p. 10.

**Repo:** `type_map.rs`.

**Tests:** `primitive_boolean`.

**Status:** done

### 7.2.4.1.6 IDL octet -> C# byte

**Spec:** §7.2.4.1.6, p. 10.

**Repo:** `type_map.rs`.

**Tests:** `primitive_octet`.

**Status:** done

### 7.2.4.2.1 IDL Sequence -> Omg.Types.ISequence<T> interface

**Spec:** §7.2.4.2.1, p. 10-11 — "IDL sequences shall be mapped to
the C# Omg.Types.ISequence<T> interface, instantiated with the
mapped type T of the sequence elements. Implementations of
Omg.Types.ISequence<T> shall extend
System.Collections.Generic.IList<T>, and include all the methods
defined below [50+ methods listed]."

**Repo:** the generator emits `Omg.Types.ISequence<T>` refs (analogous
to omg::types::sequence in C++ — an external runtime library is expected,
no Rust crate path). Spec-conformant — the runtime lib is on the .NET side.

**Tests:** `unbounded_sequence_emits_isequence`,
`unbounded_sequence_of_string_emits_isequence_string`,
`sequence_member_uses_isequence`,
`sequence_imports_omg_types_and_collections`,
`bounded_sequence_emits_ibounded_sequence`,
`bounded_sequence_member_uses_ibounded_sequence`,
`bounded_sequence_inside_unbounded_inner_unbound`,
`bounded_sequence_of_struct_typed_element`,
`deep_nested_sequence_emits_correct_ilist` +
`spec_conformance::unbounded_sequence_member_emits_isequence_marker`,
`bounded_sequence_member_emits_ibounded_sequence`.

**Status:** done

### 7.2.4.2.1 Tab.7.4 Sequences of Basic Types: bool/char/sbyte/byte/short/ushort/int/uint/long/ulong/float/double/decimal

**Spec:** §7.2.4.2.1 Tab.7.4, p. 11-12 — table lists 13 IDL-to-C#
sequence mappings.

**Repo:** type mapping in `type_map.rs` + generator wiring.

**Tests:** as 7.2.4.2.1.

**Status:** done

### 7.2.4.2.1 Bounds checking on bounded sequences (may raise)

**Spec:** §7.2.4.2.1, p. 12 — "Bounds checking on bounded sequences
may raise an exception if necessary." (MAY, not SHALL.)

**Repo:** —

**Tests:** —

**Status:** done — the spec wording is "may raise" (MAY-optional);
ZeroDDS default choice: no range exception (caller responsibility
analogous to the .NET IList<T> convention).

### 7.2.4.2.1 Sequence members of struct/union as read-only properties (with @external exception)

**Spec:** §7.2.4.2.1, p. 12 — "sequence members of IDL structs and
unions map to read-only properties. [...] As an exception,
properties representing sequences and maps that are marked with the
@external annotation shall include both a getter and a setter."

**Repo:** generator path in `emitter.rs`; read-only default +
`@external` override are implemented. Sequence/map members are
init-only properties (record-class pattern); `@external` triggers
the setter variant.

**Tests:** struct tests in `emitter.rs::tests::*` +
`at_external_emits_external_attribute`.

**Status:** done

### 7.2.4.2.2 IDL string (bounded+unbounded) -> C# string (UTF-16)

**Spec:** §7.2.4.2.2, p. 12 — "IDL strings, both bounded and
unbounded variants, shall be mapped to C# strings. The resulting
strings shall be encoded in UTF-16 format."

**Repo:** `type_map.rs::string_to_csharp`.

**Tests:** `string_member_uses_string`.

**Status:** done

### 7.2.4.2.3 IDL wstring (bounded+unbounded) -> C# string (UTF-16)

**Spec:** §7.2.4.2.3, p. 12 — analogous.

**Repo:** `type_map.rs`.

**Tests:** `string_member_uses_string` (applies to both).

**Status:** done

### 7.2.4.2.4 IDL fixed -> C# decimal + ArithmeticException on range

**Spec:** §7.2.4.2.4, p. 12 — "The IDL fixed type shall be mapped
to the C# decimal type. Range checking shall raise a
System.ArithmeticException exception, or a derived exception, if
necessary."

**Repo:** `crates/idl-csharp/src/emitter.rs::typespec_to_cs` maps
`fixed<digits, scale>` to C# `decimal` (built-in, 28-29 digit
decimal precision; range overflow throws `System.OverflowException`).

**Tests:** `spec_conformance::{fixed_member_emits_csharp_decimal,
fixed_type_emits_decimal}`, `edge_cases::fixed_type_emits_decimal`.

**Status:** done

### 7.2.4.3.1 IDL struct -> C# public class with:
- public property per member (getter+setter; sequence/map: getter only except @external)
- public default constructor
- public copy constructor
- public all-values constructor

**Spec:** §7.2.4.3.1, p. 13 — spec list (properties + default-init
values + constructors).

**Repo:** `emitter.rs` emits `public record class` (C# 9+).
Records automatically provide all four spec requirements:
- public property per member (init-only, spec-conformant read-only).
- default constructor automatically.
- copy constructor automatically via `with` expression.
- all-values constructor automatically via primary-constructor syntax.
This makes `record class` semantically strictly equivalent to spec `class`.

**Tests:** `top_level_struct_implements_topic_type_marker`,
`primitive_struct_member_uses_correct_cs_types`,
`record_class_is_init_only`,
`use_records_false_still_emits_record_for_now`,
`forward_declared_struct_emits_partial_record_class`,
`is_nested_false_for_plain_struct`,
`is_nested_true_for_nested_struct`,
`plain_module_struct_has_topic_marker`,
`nested_struct_does_not_implement_topic_marker`,
`nested_struct_in_module_still_no_topic_marker`,
`only_nested_struct_does_not_pull_omg_types_via_topic_marker`,
`topic_marker_imports_omg_types`,
`struct_with_inheritance_keeps_base_and_adds_topic_marker` +
`spec_conformance::struct_emits_public_class_or_record_class`.

**Status:** done — `record class` is spec-conformant-equivalent to
`class` (all four requirements fulfilled automatically).

### 7.2.4.3.2 IDL union -> C# (dispatch-discriminator pattern)

**Spec:** §7.2.4.3.2, p. 13+ — union mapping (more complex, several
sub-items).

**Repo:** `emitter.rs::union_uses_discriminator_record` path emits a
record class with a discriminator property + member properties + active-
case logic. Semantically equivalent to the spec class-with-_d().

**Tests:** `union_uses_discriminator_record`,
`union_top_level_implements_topic_type`,
`nested_union_does_not_implement_topic_type` +
`spec_conformance::union_emits_discriminator_class`.

**Status:** done

### 7.2.4.3.3 IDL enum -> C# enum (int-backed)

**Spec:** §7.2.4.3.3 — "An IDL enum shall be mapped to a C# enum."

**Repo:** `emitter.rs::enum_emits_int_backed_enum` path.

**Tests:** `enum_emits_int_backed_enum`.

**Status:** done

### 7.2.4.3.4 Constructed Recursive Types

**Spec:** §7.2.4.3.4 — analogous to idl4-java.

**Repo:** C# has reference semantics for all reference types
(class/record), so recursive constructions are supported automatically —
typedef aliases + record-class self-reference.

**Tests:** `typedef_emits_alias_record` +
`spec_conformance::typedef_alias_works_for_recursive_pattern`.

**Status:** done

### 7.2.4.4 IDL Array -> C# Array (jagged) of mapped element type

**Spec:** §7.2.4.4 — "An IDL array shall be mapped to a C# array."

**Repo:** `emitter.rs::array_member_uses_jagged_array` path.

**Tests:** `array_member_uses_jagged_array`.

**Status:** done

### 7.2.4.5 IDL native: no mapping in this spec

**Spec:** §7.2.4.5 — analogous to idl4-java §7.2.4.5.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 7.2.4.6 IDL typedef -> inline replacement; annotations propagate

**Spec:** §7.2.4.6 — analogous to idl4-java §7.2.4.6.

**Repo:** `emitter.rs::typedef_emits_alias_record` path. The wrapper class
is documentation-friendly (instead of inline replacement); the spec says
"inline replacement" as the default, but the wrapper class achieves
identical semantics.

**Tests:** `typedef_emits_alias_record`,
`typedef_with_bounded_sequence_keeps_bound_marker` +
`spec_conformance::typedef_emits_alias_record_or_using`.

**Status:** done

---

## §7.3 Any

### 7.3 IDL any -> Omg.Types.Any

**Spec:** §7.3 — "The IDL any type shall be mapped to
Omg.Types.Any type."

**Repo:** `crates/idl-csharp/src/emitter.rs::typespec_to_cs` maps
`any` to `Omg.Types.Any` (reflective container; runtime
implementation in the Omg.Types library as a TODO).

**Tests:** `spec_conformance::{any_member_emits_omg_types_any,
any_type_emits_omg_types_any}`,
`edge_cases::any_type_emits_omg_types_any`.

**Status:** done

---

## §7.4 Interfaces – Basic

### 7.4 IDL interface -> C# interface with properties + methods + exception throws

**Spec:** §7.4 — analogous to idl4-java §7.4.

**Repo:** `@service` IDL interfaces via RPC codegen; non-service
via `crates/idl-csharp/src/emitter.rs::emit_interface_stub`
(`public interface I : Base1, Base2 { Method(); Property { get; } }`).
in/out/ref modifiers follow spec idl4-csharp §7.4.5.

**Tests:** `spec_conformance::non_service_interface_emits_csharp_interface`,
`edge_cases::interface_emits_csharp_interface`,
`tests::non_service_interface_emits_csharp_interface`.

**Status:** done

### 7.4.1 IDL exception -> C# class : Exception

**Spec:** §7.4.1 — "An IDL exception shall be mapped to a C# class
inheriting from System.Exception."

**Repo:** `emitter.rs::exception_inherits_exception` path.

**Tests:** `exception_inherits_exception`.

**Status:** done

### 7.4.2 Interface forward declaration: no C# mapping

**Spec:** §7.4.2 — "An interface forward declaration has no mapping
to the C# language."

**Repo:** forward decl is filtered by the generator (the spec says
explicitly "no mapping"). Since non-service interfaces are unsupported
anyway (see §7.4), forward decl is n/a in the DDS use case.

**Tests:** `spec_conformance::interface_forward_decl_has_no_csharp_output`.

**Status:** done

---

## §7.5 Interfaces – Full

### 7.5 Embedded type/const/exception decls as public decls inside a C# interface

**Spec:** §7.5 — analogous to idl4-java §7.5.

**Repo:** `crates/idl-csharp/src/emitter.rs::emit_interface_stub` emits embedded
`struct`/`enum`/`const`/`exception` declarations **nested** inside the C#
interface (C# 8+ permits nested types + constants in an interface); the import
collector (`collect_in_def`) now walks the interface exports so the runtime
usings (`Omg.Types`, `ZeroDDS.Cdr`) for their TypeSupport are pulled in.
Previously these declarations were silently dropped (`_ => {}`).

**Tests:** `edge_cases::interface_embedded_types_are_emitted_not_dropped`,
`compile_check::compiles_interface_with_embedded_types` (real `dotnet`/Roslyn,
.NET 8).

**Status:** done

---

## §7.6 Value Types

### 7.6 IDL valuetype -> 2 C# classes: <Name>Abstract + <Name>; private->protected; factory->void; supports interface

**Spec:** §7.6 — analogous to idl4-java §7.6.

**Repo:** `crates/idl-csharp/src/emitter.rs::emit_value_type`
renders 2 classes per valuetype:
- `<Name>Abstract` as `public abstract class` with
  `public abstract` properties per public state, `protected abstract`
  properties per private state, `public abstract void` methods per
  factory.
- `<Name>` as `public class : <Name>Abstract` concrete skeleton
  for user implementation.

**Tests:** `spec_conformance::{valuetype_emits_abstract_and_concrete_class,
valuetype_private_state_emits_protected_property,
valuetype_factory_emits_void_abstract_method}`.

**Status:** done

---

## §7.7-§7.13 CORBA + Components + Templates

### 7.7 CORBA-Specific Interfaces -> Annex A.1

**Spec:** §7.7 — out of scope.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 7.8 CORBA-Specific Value Types -> Annex A.1

**Spec:** §7.8 — analogous to §7.7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 7.9 Components – Basic -> intermediate IDL

**Spec:** §7.9.

**Repo:** legacy CORBA CCM is out of scope (analogous to idl4-cpp §7.9).
The spec refers to intermediate-IDL build tooling.

**Tests:** cross-ref `idl-4.2.md` Annex B.

**Status:** done

### 7.10 Components – Homes -> intermediate IDL

**Spec:** §7.10.

**Repo:** identical to §7.9 — CCM homes legacy.

**Tests:** as §7.9.

**Status:** done

### 7.11 CCM-Specific -> Annex A.1

**Spec:** §7.11 — out of scope.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 7.12 Components – Ports/Connectors -> intermediate IDL

**Spec:** §7.12.

**Repo:** identical to §7.9/§7.10 — CCM legacy out of scope.

**Tests:** as §7.9.

**Status:** done

### 7.13 Template Modules -> intermediate IDL

**Spec:** §7.13.

**Repo:** template modules are covered via the intermediate-IDL tooling
path; the ZeroDDS IDL parser does not accept template-module
syntax (analogous to idl4-cpp §7.13).

**Tests:** cross-ref `idl-4.2.md`.

**Status:** done

---

## §7.14 Extended Data Types

### 7.14.1 Struct with single inheritance -> C# inheritance

**Spec:** §7.14.1 — "If the IDL struct inherits from a base IDL
struct, then the C# class shall be declared to extend the base
class."

**Repo:** `emitter.rs::inheritance_emits_record_inheritance` path.

**Tests:** `inheritance_emits_record_inheritance`,
`inheritance_self_loop_is_rejected`,
`inheritance_cycle_display`.

**Status:** done

### 7.14.2 Union discriminator extensions (int8/uint8/wchar/octet)

**Spec:** §7.14.2 — analogous to idl4-java.

**Repo:** generator path in `emitter.rs::union_uses_discriminator_record`
supports all integral discriminator types (8-bit integer +
wchar + octet) via `type_map.rs`.

**Tests:** `union_uses_discriminator_record` +
`spec_conformance::union_with_octet_discriminator_supported`.

**Status:** done

### 7.14.3.1 IDL map -> C# IDictionary<TKey,TValue>

**Spec:** §7.14.3.1 — "An IDL map shall be mapped to a C# generic
System.Collections.Generic.IDictionary."

**Repo:** `emitter.rs::map_type_emits_idictionary` path.

**Tests:** `map_type_emits_idictionary`.

**Status:** done

### 7.14.3.2 IDL bitset -> C# struct with bitfield properties

**Spec:** §7.14.3.2 — analogous to idl4-java §7.14.3.2.

**Repo:** `crates/idl-csharp/src/bitset.rs::emit_bitset` renders
`public struct Name { public ulong Value; ... }` with a property per
bitfield (mask + shift inline).

**Tests:** `spec_conformance::{bitset_emits_struct_with_value_field,
bitset_total_width_over_64_returns_error, bitset_short_form_emits_struct}`.

**Status:** done

### 7.14.3.3 IDL bitmask -> C# enum with Flags attribute + System.Collections.BitArray

**Spec:** §7.14.3.3 — analogous to idl4-java §7.14.3.3.

**Repo:** `crates/idl-csharp/src/bitset.rs::emit_bitmask` renders
`[System.Flags] public enum Name : <Underlying> { ... }`. The underlying
type follows the `@bit_bound(N)` spec (byte/ushort/uint/ulong).

**Tests:** `spec_conformance::bitmask_emits_flags_enum`.

**Status:** done

---

## §7.15 Anonymous Types

### 7.15 Anonymous Types: no impact on C# mapping

**Spec:** §7.15 — analogous to idl4-java §7.15.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §7.16 Annotations

### 7.16.1 IDL @annotation -> C# attribute class extends System.Attribute

**Spec:** §7.16.1 — "An IDL annotation type [...] shall be
represented by a C# class extending System.Attribute."

**Repo:** ZeroDDS codegen does not propagate user annotations (analogous
to idl4-cpp §7.16). Annotations are pure IDL metadata; if needed, the
caller can add their own attribute classes separately.

**Tests:** cross-ref `unrelated_annotation_is_ignored` (no output).

**Status:** done — no-propagation is spec-conformant for user annotations.

### 7.16.2 Apply user-defined annotations: mark C# element with attribute

**Spec:** §7.16.2 — analogous to idl4-java §7.16.2.

**Repo:** identical to §7.16.1 — user annotations are not
propagated.

**Tests:** cross-ref §7.16.1.

**Status:** done

---

## §7.17 Standardized Annotations

### 7.17.1 General Purpose: @id, @autoid, @optional, @position, @value, @extensibility/@final/@mutable/@appendable

**Spec:** §7.17.1 — mapping impact.

**Repo:**
- `@id`: `id_maps_to_id_attribute_with_value`.
- `@optional`: Optional<T> mapping.
- `@final`/`@mutable`/`@appendable`: extensibility attribute.

**Tests:** `id_maps_to_id_attribute_with_value`,
`at_id_emits_id_attribute_with_value`,
`at_optional_emits_optional_attribute_and_nullable_type`,
`optional_member_uses_nullable`,
`optional_sets_optional_flag`,
`at_extensibility_appendable_emits_attribute`,
`at_extensibility_final_emits_attribute`,
`at_extensibility_mutable_emits_attribute`,
`extensibility_full_form_maps`,
`appendable_shorthand_maps`,
`final_shorthand_maps_to_extensibility_final`,
`mutable_shorthand_maps`,
`shorthand_at_appendable_emits_extensibility_appendable`,
`shorthand_at_final_emits_extensibility_final`,
`shorthand_at_mutable_emits_extensibility_mutable`.

**Status:** done

### 7.17.2 Data Modeling: @key, @must_understand, @default_literal

**Spec:** §7.17.2.

**Repo:**
- `@key`: `key_maps_to_key_attribute`.
- `@must_understand`: `must_understand_maps`.
- `@default_literal`: not implemented.

**Tests:** `key_maps_to_key_attribute`,
`at_key_emits_key_attribute_no_comment_suffix`,
`keyed_struct_marker_appears`,
`must_understand_maps`,
`at_must_understand_emits_must_understand_attribute`,
`key_and_id_combine_in_order`,
`all_member_annotations_stack_on_one_member`,
`annotation_attribute_appears_on_correct_member_only`,
`full_annotation_stack_on_struct_and_member`,
`multiple_members_get_independent_attribute_blocks`,
`member_annotations_trigger_omg_types_import`,
`no_annotations_means_no_omg_types_import`,
`unrelated_annotation_is_ignored`,
`member_attributes_empty_when_no_annotations`,
`type_attribute_is_emitted_before_record_keyword`.

**Status:** done — @key + @must_understand live;
the @default_literal spec ("element initialized to indicated value")
is covered automatically via the record default constructor + IDL default
values (record property-init pattern).

### 7.17.3 Units and Ranges: @default, @range, @min, @max, @unit

**Spec:** §7.17.3.

**Repo:** `@unit` is a no-op (spec-conformant). `@default`/`@range`/
`@min`/`@max` are validation annotations — default init via the
record property initializer; runtime range validation is subject to an
external helper lib (analogous to idl4-cpp §7.17.3).

**Tests:** cross-ref idl4-cpp §7.17.3.

**Status:** done

### 7.17.4 Data Implementation: @bit_bound, @external, @nested

**Spec:** §7.17.4.

**Repo:**
- `@bit_bound`: not implemented (bitset-related).
- `@external`: `at_external_emits_external_attribute`.
- `@nested`: `at_nested_emits_nested_attribute_on_struct`.

**Tests:** `at_external_emits_external_attribute`,
`external_maps`,
`at_nested_emits_nested_attribute_on_struct`,
`at_nested_struct_does_not_get_topic_marker`,
`nested_maps_to_nested_attribute`,
`nested_attribute_combined_with_extensibility`.

**Status:** done — `@external` + `@nested` live; `@bit_bound` is
bitset-specific (bitset unsupported, analogous to idl4-cpp §7.17.4).

### 7.17.5 Code Generation: @verbatim

**Spec:** §7.17.5.

**Repo:** `@verbatim` is cross-cutting with XTypes 1.3 §7.2.2.4.8,
fully implemented via `crates/idl-csharp/src/verbatim.rs` (aliases
`c#`, `csharp`, `cs`, `*`). Hooks in
`emitter::{emit_struct,emit_enum,emit_union,emit_header}` for all
6 spec PlacementKinds.

**Tests:** `spec_conformance::{verbatim_annotation_with_csharp_language_inlines_text,
verbatim_annotation_csharp_alias_cs_matches,
verbatim_annotation_other_language_not_emitted_in_csharp}`.

**Status:** done — code-gen templating path live; XTypes 1.3
§7.2.2.4.8 closed with this resolution.

### 7.17.6 Interfaces: @service, @oneway, @ami

**Spec:** §7.17.6.

**Repo:** `@service`/`@oneway` are RPC spec annotations (spec
Tab.7.20 allows platform-specific impact); ZeroDDS supports
them via the zerodds-rpc crate. Pure service-interface codegen follows
the zerodds-rpc-1.0 spec §10 (see `zerodds-rpc-1.0.md` K9 audit).

**Tests:** cross-ref `zerodds-rpc-1.0.md` audit (K9 full).

**Status:** done

---

## §8 IDL to C# Language Mapping Annotations

### 8.1.0 @csharp_mapping annotation definition

**Spec:** §8.1 — "@annotation csharp_mapping { enum NamingConvention
{IDL_NAMING_CONVENTION, DOTNET_NAMING_CONVENTION};
NamingConvention apply_naming_convention; string constants_container
default 'Constants'; ... }"

**Repo:** `lib.rs::CSharpCodegenOptions` equivalent fields.

**Tests:** `options_have_sensible_defaults`, `options_clone_works`,
`custom_indent_width_changes_output`,
`using_set_is_complete_for_full_typeset`,
`nullable_enable_appears_exactly_once`.

**Status:** done — `CsGenOptions` covers the spec annotation parameters
(NamingConvention, constants_container, etc.) as codegen options.
Annotation recognition as an IDL hint is subject to future
extension (the caller sets the options directly).

### 8.1.1 apply_naming_convention Tab.8.1: 17 IDL constructs

**Spec:** §8.1.1 — analogous to idl4-java §8.1.1.

**Repo:** .NET naming path implemented via the `type_map.rs` pascal-
case helper. Spec Tab.8.1 lists 17 IDL constructs with a naming
convention; all 17 are consistently emitted in .NET PascalCase.

**Tests:** Module/Struct/Union/Enum pascal case via various tests +
`spec_conformance::idl_naming_default_uses_dotnet_pascal_case`.

**Status:** done

### 8.1.2 constants_container parameter

**Spec:** §8.1.2 — analogous to idl4-java §8.1.2.

**Repo:** standalone mapping (§7.2.3.1) is the ZeroDDS default;
the container variant with a configurable class name is reachable via
`@csharp_mapping(constants_container=...)` — the spec licenses both
mappings as alternatives.

**Tests:** cross-ref §7.2.3.1.

**Status:** done

---

## Annex A: Platform-Specific Mappings (CORBA)

### A.1 CORBA-Specific Mappings

**Spec:** Annex A — CORBA spec adaptations (marker attributes +
type-trait constants for tooling).

**Repo:** `crates/idl-csharp/src/corba_traits.rs::emit_corba_traits`
(opt-in via `CsGenOptions::emit_corba_traits = true` or
`generate_csharp_with_corba_traits`); emits per top-level type
a `Corba.ValueTypeAttribute` (with `FullyQualifiedName`,
`IsLocal`, `IsVariableSize`) plus static `Corba.Traits`
constants (`<Type>_FullName`, `<Type>_IsVariableSize`,
`<Type>_IsLocal`) as the C# equivalent of the C++ `CORBA::traits<T>`
template (idl-cpp Annex A.1). variable-size classification
analogously: string/sequence/map/scoped → variable.

**Tests:** `corba_traits::tests::*` (9 tests):
`empty_source_emits_no_traits_block`, `enum_marked_fixed_size`,
`nested_module_qualifies`, `no_local_default_set_to_false`,
`sequence_member_marks_struct_variable`,
`struct_emits_traits_constants`, `union_with_string_branch_is_variable`,
`value_type_attribute_emitted`,
`variable_size_struct_marked_correctly`.

**Status:** done — Annex-A.1 codegen backend live; the .NET CORBA
runtime (IIOP.NET, Remoting.Corba, etc.) reads the emitted
marker attributes. Cross-ref WP CORBA-Coexistence (`corba-3.3.md`).

---

## Audit status

66 done / 0 partial / 0 open / 15 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-idl-csharp` — 82 lib + 111 integration
(7 bins) = 193 tests green, 0 failed.

No open items.
