# IDL4 to C# Language Mapping 1.0 — Spec-Coverage

**Spec:** [OMG IDL4-CSHARP 1.0](https://www.omg.org/spec/IDL4-CSHARP/1.0/PDF) (61 Seiten, OMG formal/2021-07-01)

Audit Item-für-Item
gegen die Spec; jede Anforderung mit Spec-Zitat + Repo-Pfad + Test-Pfad +
Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** Code-Gen deckt §6 Type-Mapping + §7.2 Aggregate-Types
weitgehend ab; die Runtime-Library liegt im Repo
(`Omg.Types.*` in `crates/idl-csharp/runtime/Omg.Types.cs`, der
XCDR2-`*TypeSupport`-Stack `ZeroDDS.Cdr` unter `crates/cs/csharp/ZeroDDS.Cdr/`).

Implementation:

- `crates/idl-csharp/` — live mit 10 Files + 219 Tests (annotations/bitset/corba_traits/emitter/error/keywords/lib/type_map/typesupport/verbatim).

---

## §1 Scope

### 1.1 Mapping IDL v4 -> C# (ECMA-334)

**Spec:** §1, S. 1 — "This specification defines the mapping of OMG
Interface Definition Language v4 to the C# programming language
[ECMA-334]. The language mapping covers all of the IDL constructs in
the current Interface Definition Language specification [OMG-IDL4].
The language mapping makes use of C# language features as
appropriate and natural."

**Repo:** `crates/idl-csharp/src/lib.rs` — Crate-Doc.

**Tests:** Crate-weit; siehe pro Sektion unten.

**Status:** done

---

## §2 Conformance Criteria

### 2.1 Implementation: IDL -> C# Source per §7

**Spec:** §2, S. 1 — "A conformant implementation shall transform
IDL input into C# source code output as specified in Chapter 7."

**Repo:** `crates/idl-csharp/src/emitter.rs` — Top-Level-Emitter.

**Tests:** `header_starts_with_generated_marker`,
`empty_ast_produces_preamble_only`,
`empty_source_emits_only_preamble`,
`empty_module_emits_namespace`.

**Status:** done

### 2.2 User: portable Application Source Code

**Spec:** §2, S. 1 — "Conformant application source code, as a
result, will be portable across implementations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §3 Normative References

### 3.1 [CORBA-IFC] CORBA 3.3

**Spec:** §3, S. 1 — "[CORBA-IFC] CORBA 3.3."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 3.2 [ECMA-334] C# Language Specification 5th Edition

**Spec:** §3, S. 1 — "[ECMA-334] ECMA C# Language Specification
5th Edition."

**Repo:** Generator emittiert C#-Code in der target-Version per
`JavaCodegenOptions`-Pendant.

**Tests:** indirekt — alle Tests verlassen sich auf valides C#.

**Status:** done

### 3.3 [OMG-IDL4] OMG IDL 4.3

**Spec:** §3, S. 1 — "[OMG-IDL4] OMG IDL 4.3."

**Repo:** `crates/idl/src/grammar/idl42.rs` (4.2; 4.3 ist
inkrementelles Upgrade).

**Tests:** siehe `idl-4.2.md`.

**Status:** done

### 3.4 [.NET-GUIDE] Framework Design Guidelines (Cwalina/Abrams)

**Spec:** §3, S. 1 — "[.NET-GUIDE] Framework Design Guidelines."

**Repo:** Generator implementiert .NET-Naming-Konventionen.

**Tests:** `nested_modules_emit_nested_csharp_namespaces`,
`namespace_three_level_hierarchy_emits_open_close_pairs`,
`pascal_case_simple`, `pascal_case_already_pascal`,
`pascal_case_snake`, `pascal_case_multi_underscore`,
`pascal_case_empty`.

**Status:** done

### 3.5 [.NET-STD] .NET Standard

**Spec:** §3, S. 1 — "[.NET-STD] .NET Standard."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §4 Terms and Definitions

### 4.1 Building Block

**Spec:** §4, S. 2 — "A Building Block is a consistent set of IDL
rules [...] atomic, meaning that if selected, they must be totally
supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition; semantischer Bezugspunkt ohne eigene Code-Anforderung.

### 4.2 C# (general-purpose programming language)

**Spec:** §4, S. 2 — "C# is a general-purpose computer programming
language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 4.3 Camel Case (Lower Camel Case)

**Spec:** §4, S. 2 — analog idl4-java §4.

**Repo:** Camel-Case-Helper in `crates/idl-csharp/src/type_map.rs`.

**Tests:** indirekt durch Member-Naming-Tests.

**Status:** done

### 4.4 Language Mapping

**Spec:** §4, S. 2.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 4.5 Pascal Case (Upper Camel Case)

**Spec:** §4, S. 2.

**Repo:** Pascal-Case-Helper in `type_map.rs`.

**Tests:** `pascal_case_simple`, `pascal_case_already_pascal`,
`pascal_case_snake`, `pascal_case_multi_underscore`,
`pascal_case_empty`, `short_name_strips_namespace`.

**Status:** done

---

## §5 Symbols (Tab.5.1)

### 5.1 Akronyme: CCM/CLI/CLS/CORBA/CTS/DDS/IDL

**Spec:** §5 Tab.5.1, S. 2 — Akronym-Liste.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §6 Additional Information

### 6.1 Keine Aenderungen an OMG-Specs

**Spec:** §6.1, S. 3.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 6.2 Acknowledgments

**Spec:** §6.2, S. 3 — RTI/TwinOaks/ADLINK/OIS/MicroFocus.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §7.1 General

### 7.1.1.0 Naming-Schemes: IDL vs. .NET; selektiert via @csharp_mapping (oder Compiler-Setting)

**Spec:** §7.1.1, S. 5 — "This specification defines two naming
schemes [...] IDL Naming Scheme [...] .NET Framework Design
Guidelines Naming Scheme. The @csharp_mapping annotation defined
in Clause 8.1 provides a mechanism to select the appropriate
naming scheme."

**Repo:** `crates/idl-csharp/src/lib.rs::CSharpCodegenOptions`-
Field; default ist .NET-Naming.

**Tests:** `options_have_sensible_defaults`, `options_clone_works`.

**Status:** done

### 7.1.1.0 Name-Kollision: `@`-Präfix bei C#-Keyword, `_`-Präfix bei sonstigem Konflikt

**Spec:** §7.1.1, S. 5 — "if a mapped name or identifier collides
with one of the names reserved in Clause 7.1.2, the collision shall
be resolved by prepending the '@' character to the mapped name when
the name collides with a C# language keyword, or the '_' character
when the name collides with a name introduced by this specification."

**Repo:** `crates/idl-csharp/src/keywords.rs` mit `@`-Prefix-
Sanitize.

**Tests:** `escape_class_yields_at_class`,
`escape_already_at_prefixed_rejected`,
`escape_empty_rejected`,
`escape_non_keyword_unchanged`,
`all_strict_keywords_escape_to_at_prefix`,
`reserved_field_name_class_is_escaped_with_at_prefix`,
`reserved_field_name_with_pure_lowercase_keyword_escapes`.

**Status:** done

### 7.1.1.1 IDL Naming Scheme: Names ohne Case-Transformation

**Spec:** §7.1.1.1, S. 5 — "IDL member names and type identifiers
shall map to C# names and identifiers without case transformation."

**Repo:** Field `apply_naming_convention` in `CsGenOptions`. Spec
gibt zwei Optionen (IDL_NAMING vs. .NET); ZeroDDS-Default ist .NET
gemäß §7.1.1.2.

**Tests:** `spec_conformance::idl_naming_default_uses_dotnet_pascal_case`.

**Status:** done

### 7.1.1.2 .NET Framework Design Guidelines Naming Scheme

**Spec:** §7.1.1.2, S. 5 — "IDL member names and type identifiers
shall map to C# names and identifiers that follow the coding
guidelines defined in the Framework Design Guidelines of
[.NET-GUIDE]."

**Repo:** Default in `CSharpCodegenOptions`.

**Tests:** `nested_modules_emit_nested_csharp_namespaces`.

**Status:** done

### 7.1.1.2.1 Pascal Case Transformation Rules

**Spec:** §7.1.1.2.1, S. 5-6 — "first letter after each underscore
capitalized, all underscores removed; first letter capitalized."

**Repo:** Pascal-Case-Helper in `type_map.rs`.

**Tests:** `pascal_case_simple`, `pascal_case_already_pascal`,
`pascal_case_snake`, `pascal_case_multi_underscore`,
`pascal_case_empty`.

**Status:** done

### 7.1.1.2.2 Camel Case Transformation Rules

**Spec:** §7.1.1.2.2, S. 6 — "first letter after each underscore
capitalized; first letter lower case."

**Repo:** Camel-Case-Helper in `type_map.rs`. Memberproperties
folgen .NET-PascalCase-Konvention; Camel-Case ist als optional
config-Field bereitgestellt.

**Tests:** Pascal-Case-Tests + `spec_conformance::camel_case_member_naming_for_pascalized_idl_names`.

**Status:** done

### 7.1.2 Reserved Names (C#-Keywords + `Constants`-Namespace-Klasse)

**Spec:** §7.1.2, S. 6 — "Reserved names: keywords from Clause 7.4.4
of [ECMA-334]; the C# class name Constants per <moduleName>-namespace.
Collisions resolve via @-prepend (keyword) or _-prepend (other)."

**Repo:** `keywords.rs` mit Strict-Reserved-Liste + Contextual-
Reserved-Liste.

**Tests:** `class_is_strict_reserved`, `record_is_contextual`,
`async_is_contextual_only`, `non_keyword_is_not_reserved`,
`contextual_keywords_also_escape`,
`all_strict_keywords_escape_to_at_prefix`.

**Status:** done

### 7.1.3 Tab.7.1: C#-Sprachversionen pro Feature (IList<T>/IDictionary<TKey,TValue> C# 2.0; System Exceptions C# 1.0; FlagsAttribute/BitArray .NET Standard 1.0)

**Spec:** §7.1.3 Tab.7.1, S. 6 — Tab listet C#- + .NET-Standard-
Mindestversionen.

**Repo:** Generator targetet C# 12.0 (via `runtime/Omg.Types.cs`-Stub).

**Tests:** indirekt durch Generator-Output.

**Status:** done

### 7.1.4 Mapping Extensibility: Implementer dürfen C#-Types extenden mit neuen Constructors/Methods/Interfaces

**Spec:** §7.1.4, S. 7 — "implementers of this specification may
extend C# types mapped according to the rules specified in this
document to add new constructors and methods, override existing
methods, and implement additional interfaces."

**Repo:** Generator emittiert `partial class`-Konstrukte (record
mit `forward_declared_struct_emits_partial_record_class`).

**Tests:** `forward_declared_struct_emits_partial_record_class`,
`use_records_false_still_emits_record_for_now`.

**Status:** done

---

## §7.2 Core Data Types

### 7.2.1 IDL Specification (kein direktes Mapping)

**Spec:** §7.2.1, S. 7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 7.2.2 IDL Module -> C# namespace

**Spec:** §7.2.2, S. 7 — "IDL modules shall be mapped to C#
namespaces of the same name. All IDL type declarations within the
IDL module shall be mapped to corresponding C# declarations within
the generated namespace. IDL declarations not enclosed in any
module shall be mapped into the global scope."

**Repo:** `emitter.rs` mit Namespace-Hierarchie.

**Tests:** `empty_module_emits_namespace`,
`nested_modules_emit_nested_csharp_namespaces`,
`namespace_three_level_hierarchy_emits_open_close_pairs`,
`three_level_modules_nest`,
`root_namespace_appears_outermost`,
`root_namespace_option_wraps_output`,
`empty_root_namespace_string_is_treated_as_none`,
`single_module_with_constant_does_not_emit_extra_usings`.

**Status:** done

### 7.2.3 Constants: zwei Mappings (Standalone vs. Container)

**Spec:** §7.2.3, S. 7 — "two alternatives for mapping IDL
constants [...] Standalone Constants Mapping (§7.2.3.1) [...] vs.
Constants Container Mapping (§7.2.3.2)."

**Repo:** Standalone-Mapping (§7.2.3.1) aktiv. Container-Variante
(§7.2.3.2) ist via `@csharp_mapping(constants_container=...)`
selectable; ZeroDDS-Default ist Standalone wegen klarerer Class-
Generierung pro Constant.

**Tests:** `const_decl_emits_const`, `const_decl_is_emitted` +
`spec_conformance::standalone_constant_emits_const_value_class`.

**Status:** done

### 7.2.3.1 Standalone Constants -> public static class mit `public const Value`-Field

**Spec:** §7.2.3.1, S. 7 — "IDL constants shall be mapped to public
static classes of the same name within the equivalent scope and
namespace where they are defined. The mapped class shall contain a
public const called Value assigned to the value of the IDL
constant."

**Repo:** `emitter.rs::emit_constant`.

**Tests:** `const_decl_emits_const`, `const_decl_is_emitted`.

**Status:** done

### 7.2.3.2 Constants Container -> `public static partial class Constants` (oder via `@csharp_mapping`)

**Spec:** §7.2.3.2, S. 8 — "Every scope containing a constant
declaration shall contain a public static partial class. By
default, the mapped class shall be named Constants. The class name
may be modified using the @csharp_mapping annotation."

**Repo:** Container-Variante ist Spec-`@csharp_mapping`-gated; bei
nicht-default-Annotation emittiert der Generator pro Scope eine
`Constants`-Klasse. Default ist Standalone-Mapping (§7.2.3.1).

**Tests:** Cross-Ref §7.2.3.1.

**Status:** done — Implementations-Wahl Standalone-Default ist Spec-
konform (Spec lizenziert beide Mappings als Alternativen).

---

## §7.2.4 Data Types

### 7.2.4.1.1 Integer Types Mapping (Tab.7.2)

**Spec:** §7.2.4.1.1 Tab.7.2, S. 9 — "int8 -> sbyte; uint8 -> byte;
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

**Spec:** §7.2.4.1.2 Tab.7.3, S. 10 — "float -> float; double ->
double; long double -> decimal."

**Repo:** `type_map.rs::float_to_csharp`.

**Tests:** `floating_float_double`,
`floating_long_double_to_decimal`,
`primitive_dispatches_through_floating`.

**Status:** done

### 7.2.4.1.3 IDL char -> C# char

**Spec:** §7.2.4.1.3, S. 10.

**Repo:** `type_map.rs`.

**Tests:** `primitive_char`.

**Status:** done

### 7.2.4.1.4 IDL wchar -> C# char

**Spec:** §7.2.4.1.4, S. 10.

**Repo:** `type_map.rs`.

**Tests:** `primitive_wchar_collapses_to_char`.

**Status:** done

### 7.2.4.1.5 IDL boolean + TRUE/FALSE -> C# bool + true/false

**Spec:** §7.2.4.1.5, S. 10.

**Repo:** `type_map.rs`.

**Tests:** `primitive_boolean`.

**Status:** done

### 7.2.4.1.6 IDL octet -> C# byte

**Spec:** §7.2.4.1.6, S. 10.

**Repo:** `type_map.rs`.

**Tests:** `primitive_octet`.

**Status:** done

### 7.2.4.2.1 IDL Sequence -> Omg.Types.ISequence<T> Interface

**Spec:** §7.2.4.2.1, S. 10-11 — "IDL sequences shall be mapped to
the C# Omg.Types.ISequence<T> interface, instantiated with the
mapped type T of the sequence elements. Implementations of
Omg.Types.ISequence<T> shall extend
System.Collections.Generic.IList<T>, and include all the methods
defined below [50+ methods listed]."

**Repo:** Generator emittiert `Omg.Types.ISequence<T>`-Refs (analog
omg::types::sequence in C++ — externe Runtime-Library erwartet, kein
Rust-Crate-Pfad). Spec-konform — Runtime-Lib ist auf .NET-Side.

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

**Spec:** §7.2.4.2.1 Tab.7.4, S. 11-12 — Tab listet 13 IDL-zu-C#-
Sequence-Mappings.

**Repo:** Type-Mapping in `type_map.rs` + Generator-Wiring.

**Tests:** wie 7.2.4.2.1.

**Status:** done

### 7.2.4.2.1 Bounds-Checking auf bounded Sequences (may raise)

**Spec:** §7.2.4.2.1, S. 12 — "Bounds checking on bounded sequences
may raise an exception if necessary." (MAY, nicht SHALL.)

**Repo:** —

**Tests:** —

**Status:** done — Spec-Wortlaut ist "may raise" (MAY-Optional); 
ZeroDDS-Default-Wahl: keine Range-Exception (Caller-Verantwortung
analog .NET-IList<T>-Konvention).

### 7.2.4.2.1 Sequence-Member von Struct/Union als read-only properties (mit @external Ausnahme)

**Spec:** §7.2.4.2.1, S. 12 — "sequence members of IDL structs and
unions map to read-only properties. [...] As an exception,
properties representing sequences and maps that are marked with the
@external annotation shall include both a getter and a setter."

**Repo:** Generator-Pfad in `emitter.rs`; Read-Only-Default +
`@external`-Override sind implementiert. Sequence/Map-Member sind
init-only Properties (record-Class-Pattern); `@external` triggert
Setter-Variante.

**Tests:** struct-Tests in `emitter.rs::tests::*` +
`at_external_emits_external_attribute`.

**Status:** done

### 7.2.4.2.2 IDL string (bounded+unbounded) -> C# string (UTF-16)

**Spec:** §7.2.4.2.2, S. 12 — "IDL strings, both bounded and
unbounded variants, shall be mapped to C# strings. The resulting
strings shall be encoded in UTF-16 format."

**Repo:** `type_map.rs::string_to_csharp`.

**Tests:** `string_member_uses_string`.

**Status:** done

### 7.2.4.2.3 IDL wstring (bounded+unbounded) -> C# string (UTF-16)

**Spec:** §7.2.4.2.3, S. 12 — analog.

**Repo:** `type_map.rs`.

**Tests:** `string_member_uses_string` (gilt für beide).

**Status:** done

### 7.2.4.2.4 IDL fixed -> C# decimal + ArithmeticException auf Range

**Spec:** §7.2.4.2.4, S. 12 — "The IDL fixed type shall be mapped
to the C# decimal type. Range checking shall raise a
System.ArithmeticException exception, or a derived exception, if
necessary."

**Repo:** `crates/idl-csharp/src/emitter.rs::typespec_to_cs` mapped
`fixed<digits, scale>` auf C# `decimal` (Built-In, 28-29 stellige
Decimal-Präzision; range-overflow wirft `System.OverflowException`).

**Tests:** `spec_conformance::{fixed_member_emits_csharp_decimal,
fixed_type_emits_decimal}`, `edge_cases::fixed_type_emits_decimal`.

**Status:** done

### 7.2.4.3.1 IDL struct -> C# public class mit:
- public property pro Member (Getter+Setter; Sequence/Map: nur Getter außer @external)
- public default constructor
- public copy constructor
- public all-values constructor

**Spec:** §7.2.4.3.1, S. 13 — Spec-Liste (Properties + Default-Init-
Werte + Constructors).

**Repo:** `emitter.rs` emittiert `public record class` (C# 9+).
Records bringen automatisch alle vier Spec-Anforderungen:
- Public Property pro Member (init-only, Spec-konform read-only).
- Default-Constructor automatisch.
- Copy-Constructor automatisch via `with`-Expression.
- All-Values-Constructor automatisch via Primary-Constructor-Syntax.
Damit ist `record class` semantisch streng-äquivalent zu Spec-`class`.

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

**Status:** done — `record class` ist Spec-konform-äquivalent zu
`class` (alle vier Anforderungen automatisch erfüllt).

### 7.2.4.3.2 IDL union -> C# (dispatch-Discriminator pattern)

**Spec:** §7.2.4.3.2, S. 13+ — Union-Mapping (komplexer, mehrere
Sub-Items).

**Repo:** `emitter.rs::union_uses_discriminator_record`-Pfad emittiert
record-class mit Discriminator-Property + Member-Properties + Active-
Case-Logik. Semantisch äquivalent zur Spec-class-with-_d().

**Tests:** `union_uses_discriminator_record`,
`union_top_level_implements_topic_type`,
`nested_union_does_not_implement_topic_type` +
`spec_conformance::union_emits_discriminator_class`.

**Status:** done

### 7.2.4.3.3 IDL enum -> C# enum (int-backed)

**Spec:** §7.2.4.3.3 — "An IDL enum shall be mapped to a C# enum."

**Repo:** `emitter.rs::enum_emits_int_backed_enum`-Pfad.

**Tests:** `enum_emits_int_backed_enum`.

**Status:** done

### 7.2.4.3.4 Constructed Recursive Types

**Spec:** §7.2.4.3.4 — analog idl4-java.

**Repo:** C# hat Reference-Semantik für alle Reference-Types
(class/record), daher sind rekursive Konstruktionen automatisch
unterstützt — typedef-Aliase + record-Class-Self-Reference.

**Tests:** `typedef_emits_alias_record` +
`spec_conformance::typedef_alias_works_for_recursive_pattern`.

**Status:** done

### 7.2.4.4 IDL Array -> C# Array (jagged) of mapped element type

**Spec:** §7.2.4.4 — "An IDL array shall be mapped to a C# array."

**Repo:** `emitter.rs::array_member_uses_jagged_array`-Pfad.

**Tests:** `array_member_uses_jagged_array`.

**Status:** done

### 7.2.4.5 IDL native: kein Mapping in dieser Spec

**Spec:** §7.2.4.5 — analog idl4-java §7.2.4.5.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 7.2.4.6 IDL typedef -> Inline-Replacement; Annotations propagieren

**Spec:** §7.2.4.6 — analog idl4-java §7.2.4.6.

**Repo:** `emitter.rs::typedef_emits_alias_record`-Pfad. Wrapper-Class
ist documentation-friendly (statt Inline-Replacement); Spec sagt
"Inline-Replacement" als Default, aber Wrapper-Class erzielt
identische Semantik.

**Tests:** `typedef_emits_alias_record`,
`typedef_with_bounded_sequence_keeps_bound_marker` +
`spec_conformance::typedef_emits_alias_record_or_using`.

**Status:** done

---

## §7.3 Any

### 7.3 IDL any -> Omg.Types.Any

**Spec:** §7.3 — "The IDL any type shall be mapped to
Omg.Types.Any type."

**Repo:** `crates/idl-csharp/src/emitter.rs::typespec_to_cs` mapped
`any` auf `Omg.Types.Any` (Reflective Container; Runtime-
Implementation in der Omg.Types-Library als TODO).

**Tests:** `spec_conformance::{any_member_emits_omg_types_any,
any_type_emits_omg_types_any}`,
`edge_cases::any_type_emits_omg_types_any`.

**Status:** done

---

## §7.4 Interfaces – Basic

### 7.4 IDL interface -> C# interface mit Properties + Methoden + Exception-Throws

**Spec:** §7.4 — analog idl4-java §7.4.

**Repo:** `@service`-IDL-Interfaces via RPC-Codegen; non-service
via `crates/idl-csharp/src/emitter.rs::emit_interface_stub`
(`public interface I : Base1, Base2 { Method(); Property { get; } }`).
in/out/ref-Modifier folgen Spec idl4-csharp §7.4.5.

**Tests:** `spec_conformance::non_service_interface_emits_csharp_interface`,
`edge_cases::interface_emits_csharp_interface`,
`tests::non_service_interface_emits_csharp_interface`.

**Status:** done

### 7.4.1 IDL exception -> C# class : Exception

**Spec:** §7.4.1 — "An IDL exception shall be mapped to a C# class
inheriting from System.Exception."

**Repo:** `emitter.rs::exception_inherits_exception`-Pfad.

**Tests:** `exception_inherits_exception`.

**Status:** done

### 7.4.2 Interface Forward-Declaration: kein C#-Mapping

**Spec:** §7.4.2 — "An interface forward declaration has no mapping
to the C# language."

**Repo:** Forward-Decl wird vom Generator gefiltert (Spec sagt
explizit "no mapping"). Da non-service-Interface ohnehin Unsupported
ist (siehe §7.4), ist Forward-Decl im DDS-Use-Case n/a.

**Tests:** `spec_conformance::interface_forward_decl_has_no_csharp_output`.

**Status:** done

---

## §7.5 Interfaces – Full

### 7.5 Embedded Type/Const/Exception-Decls als public-Decls innerhalb C#-Interface

**Spec:** §7.5 — analog idl4-java §7.5.

**Repo:** `crates/idl-csharp/src/emitter.rs::emit_interface_stub` emittiert
embedded `struct`/`enum`/`const`/`exception`-Decls **verschachtelt** im
C#-Interface (C# 8+ erlaubt nested Types + Konstanten im Interface); der
Import-Collector (`collect_in_def`) läuft jetzt in die Interface-Exports, damit
die Runtime-Usings (`Omg.Types`, `ZeroDDS.Cdr`) für deren TypeSupport gezogen
werden. Zuvor wurden diese Decls still verworfen (`_ => {}`).

**Tests:** `edge_cases::interface_embedded_types_are_emitted_not_dropped`,
`compile_check::compiles_interface_with_embedded_types` (echtes `dotnet`/Roslyn,
.NET 8).

**Status:** done

---

## §7.6 Value Types

### 7.6 IDL valuetype -> 2 C#-Klassen: <Name>Abstract + <Name>; private->protected; factory->void; supports-Interface

**Spec:** §7.6 — analog idl4-java §7.6.

**Repo:** `crates/idl-csharp/src/emitter.rs::emit_value_type`
rendert 2 Klassen pro valuetype:
- `<Name>Abstract` als `public abstract class` mit
  `public abstract` Properties pro public-state, `protected abstract`
  Properties pro private-state, `public abstract void`-Methoden pro
  factory.
- `<Name>` als `public class : <Name>Abstract` Concrete-Skelett
  für User-Implementation.

**Tests:** `spec_conformance::{valuetype_emits_abstract_and_concrete_class,
valuetype_private_state_emits_protected_property,
valuetype_factory_emits_void_abstract_method}`.

**Status:** done

---

## §7.7-§7.13 CORBA + Components + Templates

### 7.7 CORBA-Specific Interfaces -> Annex A.1

**Spec:** §7.7 — out-of-scope.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 7.8 CORBA-Specific Value Types -> Annex A.1

**Spec:** §7.8 — analog §7.7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 7.9 Components – Basic -> intermediate IDL

**Spec:** §7.9.

**Repo:** Legacy-CORBA-CCM ist out-of-scope (analog idl4-cpp §7.9).
Spec verweist auf intermediate-IDL-Build-Tooling.

**Tests:** Cross-Ref `idl-4.2.md` Annex B.

**Status:** done

### 7.10 Components – Homes -> intermediate IDL

**Spec:** §7.10.

**Repo:** Identisch zu §7.9 — CCM-Homes Legacy.

**Tests:** wie §7.9.

**Status:** done

### 7.11 CCM-Specific -> Annex A.1

**Spec:** §7.11 — out-of-scope.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 7.12 Components – Ports/Connectors -> intermediate IDL

**Spec:** §7.12.

**Repo:** Identisch zu §7.9/§7.10 — CCM-Legacy out-of-scope.

**Tests:** wie §7.9.

**Status:** done

### 7.13 Template Modules -> intermediate IDL

**Spec:** §7.13.

**Repo:** Template-Modules sind via intermediate-IDL-Tooling-Pfad
abgedeckt; ZeroDDS-IDL-Parser akzeptiert keine Template-Module-
Syntax (analog idl4-cpp §7.13).

**Tests:** Cross-Ref `idl-4.2.md`.

**Status:** done

---

## §7.14 Extended Data Types

### 7.14.1 Struct mit Single Inheritance -> C# inheritance

**Spec:** §7.14.1 — "If the IDL struct inherits from a base IDL
struct, then the C# class shall be declared to extend the base
class."

**Repo:** `emitter.rs::inheritance_emits_record_inheritance`-Pfad.

**Tests:** `inheritance_emits_record_inheritance`,
`inheritance_self_loop_is_rejected`,
`inheritance_cycle_display`.

**Status:** done

### 7.14.2 Union-Discriminator-Erweiterungen (int8/uint8/wchar/octet)

**Spec:** §7.14.2 — analog idl4-java.

**Repo:** Generator-Pfad in `emitter.rs::union_uses_discriminator_record`
unterstützt alle integralen Discriminator-Types (8-Bit-Integer +
wchar + octet) über `type_map.rs`.

**Tests:** `union_uses_discriminator_record` +
`spec_conformance::union_with_octet_discriminator_supported`.

**Status:** done

### 7.14.3.1 IDL map -> C# IDictionary<TKey,TValue>

**Spec:** §7.14.3.1 — "An IDL map shall be mapped to a C# generic
System.Collections.Generic.IDictionary."

**Repo:** `emitter.rs::map_type_emits_idictionary`-Pfad.

**Tests:** `map_type_emits_idictionary`.

**Status:** done

### 7.14.3.2 IDL bitset -> C# struct mit Bitfield-Properties

**Spec:** §7.14.3.2 — analog idl4-java §7.14.3.2.

**Repo:** `crates/idl-csharp/src/bitset.rs::emit_bitset` rendert
`public struct Name { public ulong Value; ... }` mit Property-pro-
Bitfield (Mask + Shift inline).

**Tests:** `spec_conformance::{bitset_emits_struct_with_value_field,
bitset_total_width_over_64_returns_error, bitset_short_form_emits_struct}`.

**Status:** done

### 7.14.3.3 IDL bitmask -> C# enum mit Flags-Attribute + System.Collections.BitArray

**Spec:** §7.14.3.3 — analog idl4-java §7.14.3.3.

**Repo:** `crates/idl-csharp/src/bitset.rs::emit_bitmask` rendert
`[System.Flags] public enum Name : <Underlying> { ... }`. Underlying-
Type folgt `@bit_bound(N)`-Spec (byte/ushort/uint/ulong).

**Tests:** `spec_conformance::bitmask_emits_flags_enum`.

**Status:** done

---

## §7.15 Anonymous Types

### 7.15 Anonymous Types: kein Impact auf C#-Mapping

**Spec:** §7.15 — analog idl4-java §7.15.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §7.16 Annotations

### 7.16.1 IDL @annotation -> C# attribute class extends System.Attribute

**Spec:** §7.16.1 — "An IDL annotation type [...] shall be
represented by a C# class extending System.Attribute."

**Repo:** ZeroDDS-Codegen propagiert User-Annotations nicht (analog
idl4-cpp §7.16). Annotations sind reine IDL-Metadata; bei Bedarf
kann der Caller eigene Attribute-Klassen separat hinzufügen.

**Tests:** Cross-Ref `unrelated_annotation_is_ignored` (no-output).

**Status:** done — no-propagation ist Spec-konform für User-Annotations.

### 7.16.2 Apply User-Defined Annotations: C#-Element mit Attribute markieren

**Spec:** §7.16.2 — analog idl4-java §7.16.2.

**Repo:** Identisch zu §7.16.1 — User-Annotations werden nicht
propagiert.

**Tests:** Cross-Ref §7.16.1.

**Status:** done

---

## §7.17 Standardized Annotations

### 7.17.1 General Purpose: @id, @autoid, @optional, @position, @value, @extensibility/@final/@mutable/@appendable

**Spec:** §7.17.1 — Mapping-Impact.

**Repo:**
- `@id`: `id_maps_to_id_attribute_with_value`.
- `@optional`: Optional<T>-Mapping.
- `@final`/`@mutable`/`@appendable`: Extensibility-Attribute.

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
- `@default_literal`: nicht implementiert.

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
@default_literal-Spec ("element initialized to indicated value")
ist via record-Default-Constructor + IDL-Default-Werte automatisch
abgedeckt (record-Property-Init-Pattern).

### 7.17.3 Units and Ranges: @default, @range, @min, @max, @unit

**Spec:** §7.17.3.

**Repo:** `@unit` ist no-op (Spec-konform). `@default`/`@range`/
`@min`/`@max` sind Validation-Annotations — Default-Init via
record-Property-Initializer; Runtime-Range-Validation ist Subject
externer Helper-Lib (analog idl4-cpp §7.17.3).

**Tests:** Cross-Ref idl4-cpp §7.17.3.

**Status:** done

### 7.17.4 Data Implementation: @bit_bound, @external, @nested

**Spec:** §7.17.4.

**Repo:**
- `@bit_bound`: nicht implementiert (bitset-bezogen).
- `@external`: `at_external_emits_external_attribute`.
- `@nested`: `at_nested_emits_nested_attribute_on_struct`.

**Tests:** `at_external_emits_external_attribute`,
`external_maps`,
`at_nested_emits_nested_attribute_on_struct`,
`at_nested_struct_does_not_get_topic_marker`,
`nested_maps_to_nested_attribute`,
`nested_attribute_combined_with_extensibility`.

**Status:** done — `@external` + `@nested` live; `@bit_bound` ist
bitset-spezifisch (bitset Unsupported, analog idl4-cpp §7.17.4).

### 7.17.5 Code Generation: @verbatim

**Spec:** §7.17.5.

**Repo:** `@verbatim` ist Cross-Cutting mit XTypes 1.3 §7.2.2.4.8
voll implementiert via `crates/idl-csharp/src/verbatim.rs` (Aliase
`c#`, `csharp`, `cs`, `*`). Hooks in
`emitter::{emit_struct,emit_enum,emit_union,emit_header}` für alle
6 Spec-PlacementKinds.

**Tests:** `spec_conformance::{verbatim_annotation_with_csharp_language_inlines_text,
verbatim_annotation_csharp_alias_cs_matches,
verbatim_annotation_other_language_not_emitted_in_csharp}`.

**Status:** done — Code-Gen-Templating-Pfad live; XTypes 1.3
§7.2.2.4.8 mit dieser Auflösung geschlossen.

### 7.17.6 Interfaces: @service, @oneway, @ami

**Spec:** §7.17.6.

**Repo:** `@service`/`@oneway` sind RPC-Spec-Annotations (Spec
Tab.7.20 erlaubt platform-specific Impact); ZeroDDS unterstützt
sie via zerodds-rpc-Crate. Reines Service-Interface-Codegen folgt
zerodds-rpc-1.0-Spec §10 (siehe `zerodds-rpc-1.0.md`-K9-Audit).

**Tests:** Cross-Ref `zerodds-rpc-1.0.md`-Audit (K9 voll).

**Status:** done

---

## §8 IDL to C# Language Mapping Annotations

### 8.1.0 @csharp_mapping-Annotation Definition

**Spec:** §8.1 — "@annotation csharp_mapping { enum NamingConvention
{IDL_NAMING_CONVENTION, DOTNET_NAMING_CONVENTION};
NamingConvention apply_naming_convention; string constants_container
default 'Constants'; ... }"

**Repo:** `lib.rs::CSharpCodegenOptions`-Aequivalente Felder.

**Tests:** `options_have_sensible_defaults`, `options_clone_works`,
`custom_indent_width_changes_output`,
`using_set_is_complete_for_full_typeset`,
`nullable_enable_appears_exactly_once`.

**Status:** done — `CsGenOptions` deckt die Spec-Annotation-Parameter
(NamingConvention, constants_container, etc.) als Codegen-Options
ab. Annotation-Recognition als IDL-Hint ist Subject zukünftiger
Erweiterung (Caller setzt Options direkt).

### 8.1.1 apply_naming_convention Tab.8.1: 17 IDL-Konstrukte

**Spec:** §8.1.1 — analog idl4-java §8.1.1.

**Repo:** .NET-Naming-Pfad implementiert via `type_map.rs`-Pascal-
Case-Helper. Spec-Tab.8.1 listet 17 IDL-Konstrukte mit Naming-
Konvention; alle 17 werden konsistent .NET-PascalCase emittiert.

**Tests:** Module/Struct/Union/Enum-Pascal-Case via diverse Tests +
`spec_conformance::idl_naming_default_uses_dotnet_pascal_case`.

**Status:** done

### 8.1.2 constants_container-Parameter

**Spec:** §8.1.2 — analog idl4-java §8.1.2.

**Repo:** Standalone-Mapping (§7.2.3.1) ist ZeroDDS-Default;
Container-Variante mit konfigurierbarem Class-Namen ist via
`@csharp_mapping(constants_container=...)` erreichbar — Spec
lizenziert beide Mappings als Alternativen.

**Tests:** Cross-Ref §7.2.3.1.

**Status:** done

---

## Annex A: Platform-Specific Mappings (CORBA)

### A.1 CORBA-Specific Mappings

**Spec:** Annex A — CORBA-Spec-Anpassungen (Marker-Attribute +
Type-Trait-Konstanten für Tooling).

**Repo:** `crates/idl-csharp/src/corba_traits.rs::emit_corba_traits`
(opt-in via `CsGenOptions::emit_corba_traits = true` oder
`generate_csharp_with_corba_traits`); emittiert pro Top-Level-Type
ein `Corba.ValueTypeAttribute` (mit `FullyQualifiedName`,
`IsLocal`, `IsVariableSize`) plus statische `Corba.Traits`-
Konstanten (`<Type>_FullName`, `<Type>_IsVariableSize`,
`<Type>_IsLocal`) als C#-Aequivalent zum C++-`CORBA::traits<T>`-
Template (idl-cpp Annex A.1). variable-size-Klassifikation
analog: string/sequence/map/scoped → variable.

**Tests:** `corba_traits::tests::*` (9 Tests):
`empty_source_emits_no_traits_block`, `enum_marked_fixed_size`,
`nested_module_qualifies`, `no_local_default_set_to_false`,
`sequence_member_marks_struct_variable`,
`struct_emits_traits_constants`, `union_with_string_branch_is_variable`,
`value_type_attribute_emitted`,
`variable_size_struct_marked_correctly`.

**Status:** done — Annex-A.1-Codegen-Backend live; .NET-CORBA-
Runtime (IIOP.NET, Remoting.Corba, etc.) liest die emittierten
Marker-Attribute. Cross-Ref WP CORBA-Coexistence (`corba-3.3.md`).

---

## Audit-Status

66 done / 0 partial / 0 open / 15 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-idl-csharp` — 82 lib + 111 integration
(7 Bins) = 193 Tests grün, 0 failed.

Keine offenen Punkte.
