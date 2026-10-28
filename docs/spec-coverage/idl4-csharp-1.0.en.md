# IDL4 to C# Language Mapping 1.0 — Spec Coverage

**Spec:** [OMG IDL4-CSHARP 1.0](https://www.omg.org/spec/IDL4-CSHARP/1.0/PDF) (61 pages, OMG formal/2021-07-01)

Item-by-item audit against the spec; each requirement with a spec
quote, repo path, test path and status (`done` / `partial` / `open` /
`n/a`).

**Context:** This document audits the OMG PSM (Chapters 7 and 8)
against today's tree. The former PSM generator `crates/idl-csharp/src/`
was removed in `d045e43ea86`; the page therefore cites the wire column
today. That column is the C# binding of the shared codec walk:

- `crates/lang-rules/src/adapter/csharp.rs` — declaration materialiser,
- `crates/lang-rules/src/emit/csharp.rs` + `default_csharp.rs` — codec
  and Table 9 defaulters,
- `crates/lang-cst/src/csharp/` + `crates/lang-render/src/csharp.rs` —
  syntax tree and text form,
- `crates/lang-rules/src/rule/csharp.rs` — rule catalogue,
- `tools/idlc/src/csharp_support.rs` — driver.

The surviving runtime is `crates/idl-csharp/runtime/Omg.Types.cs`
(`ISequence<T>`/`IBoundedSequence<T>`/`BoundedList<T>`, `Any`,
`ITopicType<T>` and the attribute bridge `[Key]`/`[Id]`/`[Optional]`/
`[MustUnderstand]`/`[External]`/`[Nested]`/`[Extensibility]`). The
materialiser does not emit those forms today; it writes a wire data
model with public fields, `T[]`, map entry classes, `using` aliases and
`[Flags]` enums. Where the PSM demands an API shape the walk does not
produce, the status is `partial` or `open`.

**Open items:** see [`idl4-csharp-1.0.open.en.md`](idl4-csharp-1.0.open.en.md).

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

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_file`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_generated_file_carries_the_support_namespace`

**Status:** partial — the walk transforms IDL into C# source but
follows Chapter 7 only in the type core; the API shape deviates in the
points itemised below (fields instead of properties, `T[]` instead of
`ISequence<T>`, no `I` prefixes, no `System.Exception` base).

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

**Repo:** —

**Tests:** —

**Status:** open — the materialiser knows no `@csharp_mapping` and no
switch; it follows one fixed rule (type names verbatim, members with the
first letter raised).

### 7.1.1 Collision resolution: `@` prefix for keywords, `_` prefix for spec names

**Spec:** §7.1.1, p. 5 — "the collision shall be resolved by prepending
the '@' character [...] when the name collides with a C# language
keyword, or the '_' character when the name collides with a name
introduced by this specification."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::field_name`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_identifier_that_is_a_csharp_keyword_is_escaped_with_an_at_sign`

**Status:** partial — the `@` prefix for C# keywords is implemented;
the `_` case for spec-introduced names (`Constants`) is missing because
the materialiser does not treat them as reserved (`ZdConstants`).

### 7.1.1.1 IDL naming scheme: names without case transformation

**Spec:** §7.1.1.1, p. 5 — "IDL member names and type identifiers shall
map to C# names and identifiers without case transformation."

**Repo:** —

**Tests:** —

**Status:** open — the scheme is not selectable. The materialiser
transforms members (first letter raised) and leaves type names verbatim,
a mixture matching neither scheme.

### 7.1.1.2 .NET Framework Design Guidelines naming scheme

**Spec:** §7.1.1.2, p. 5 — "IDL member names and type identifiers shall
map to C# names and identifiers that follow the coding guidelines
defined in the Framework Design Guidelines of [.NET-GUIDE]."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::capitalise`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_field_carries_the_idl_name_it_was_folded_from`

**Status:** partial — members are only raised in the first letter,
types and enumerators stay verbatim; the .NET rules (full Pascal Case,
`I` prefix) are not applied.

### 7.1.1.2.1 Pascal Case transformation

**Spec:** §7.1.1.2.1, p. 5 — "The first letter after each underscore
shall be capitalized and all underscores shall be removed. The first
letter of the IDL name shall be capitalized."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::capitalise`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_field_carries_the_idl_name_it_was_folded_from`

**Status:** partial — only the first letter is raised (`my_field` ->
`My_field`); underscores are not removed because a full Pascal Case is
not injective over the names IDL 4.2 §7.2.3 permits (the reason is in
the module doc).

### 7.1.1.2.2 Camel Case transformation

**Spec:** §7.1.1.2.2, p. 6 — "The first letter after each underscore
shall be capitalized and all underscores shall be removed. The first
letter of the IDL name shall be lower case."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_param`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_interface_is_an_interface_of_signatures_with_the_idl_passing_modes`

**Status:** partial — operation parameters are taken verbatim, not
transformed into Camel Case.

### 7.1.2 Reserved names (C# keywords + `Constants`)

**Spec:** §7.1.2, p. 5 — reserved are the C# keywords of ECMA-334
§7.4.4 and the class name `Constants` per module namespace.

**Repo:** `crates/lang-cst/src/csharp/mod.rs::is_keyword`

**Tests:** `crates/lang-cst/src/csharp/mod.rs::tests::a_keyword_escapes_with_an_at_sign_and_keeps_its_identity`

**Status:** partial — the C# keywords are tracked as reserved; the name
`Constants` is not, because the class is called `ZdConstants`.

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

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::scope_items`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_module_is_a_namespace_and_two_modules_do_not_collide`,
`crates/lang-rules/src/adapter/csharp.rs::tests::a_module_reopened_beside_a_nested_one_is_one_namespace_block`,
`crates/lang-rules/src/adapter/csharp.rs:2882: assert!(generated.contains("namespace a"), "{generated}");`

**Status:** done

### 7.2.3 Constants: two alternatives

**Spec:** §7.2.3, p. 7 — "two alternatives for mapping IDL constants
[...] Standalone Constants Mapping [...] Constants Container Mapping."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::constants`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_constant_becomes_a_const_field_of_a_holder_class`

**Status:** partial — only the container variant exists, and it is not
selectable via `@csharp_mapping`; the standalone variant is missing.

### 7.2.3.1 Standalone constants: one class per constant with `Value`

**Spec:** §7.2.3.1, p. 7 — "IDL constants shall be mapped to public
static classes of the same name [...]. The mapped class shall contain a
public const called Value."

**Repo:** —

**Tests:** —

**Status:** open — constants are held as `const` fields in a shared
`ZdConstants` class, not as one class per constant.

### 7.2.3.2 Constants container: `public static partial class Constants`

**Spec:** §7.2.3.2, p. 8 — "Every scope containing a constant
declaration shall contain a public static partial class. By default,
the mapped class shall be named Constants."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::scope_items`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_constant_becomes_a_const_field_of_a_holder_class`

**Status:** partial — the class is `public static`, but neither
`partial` nor named `Constants`, and not configurable via the
annotation.

### 7.2.4.1.1 Integer types (Tab.7.2)

**Spec:** §7.2.4.1.1, Tab.7.2, p. 9 — "int8 -> sbyte; uint8 -> byte;
short/int16 -> short; unsigned short/uint16 -> ushort; long/int32 ->
int; unsigned long/uint32 -> uint; long long/int64 -> long; unsigned
long long/uint64 -> ulong."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_primitive`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_idl_primitive_reaches_the_csharp_type_that_holds_it`,
`crates/lang-rules/src/adapter/csharp.rs:2630: assert!(generated.contains(expected), "{expected}\n{generated}");`

**Status:** done

### 7.2.4.1.2 Floating-point types (Tab.7.3)

**Spec:** §7.2.4.1.2, Tab.7.3, p. 10 — "float -> float; double ->
double; long double -> decimal."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_primitive`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_idl_primitive_reaches_the_csharp_type_that_holds_it`

**Status:** partial — `float` and `double` match; `long double` is
emitted as the generated sixteen-octet holder
`global::ZeroDds.Idl.Float128`, not as `decimal` (the spec itself warns
about precision loss; the deviation is justified in the module doc).

### 7.2.4.1.3 IDL char -> C# char

**Spec:** §7.2.4.1.3, p. 10 — "The IDL char type shall be mapped to the
C# type char."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_primitive`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_idl_primitive_reaches_the_csharp_type_that_holds_it`,
`crates/lang-rules/src/adapter/csharp.rs:2630: assert!(generated.contains(expected), "{expected}\n{generated}");`

**Status:** done

### 7.2.4.1.4 IDL wchar -> C# char

**Spec:** §7.2.4.1.4, p. 10 — "The IDL wchar type shall be mapped to
the C# type char."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_primitive`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_idl_primitive_reaches_the_csharp_type_that_holds_it`,
`crates/lang-rules/src/adapter/csharp.rs:2630: assert!(generated.contains(expected), "{expected}\n{generated}");`

**Status:** done

### 7.2.4.1.5 IDL boolean -> C# bool

**Spec:** §7.2.4.1.5, p. 10 — "The IDL boolean type shall be mapped to
the C# bool, and the IDL constants TRUE and FALSE shall be mapped to
the corresponding C# boolean literals."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_primitive`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_idl_primitive_reaches_the_csharp_type_that_holds_it`,
`crates/lang-rules/src/adapter/csharp.rs:2630: assert!(generated.contains(expected), "{expected}\n{generated}");`

**Status:** done

### 7.2.4.1.6 IDL octet -> C# byte

**Spec:** §7.2.4.1.6, p. 10 — "The IDL type octet [...] shall be mapped
to the C# type byte."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_primitive`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_idl_primitive_reaches_the_csharp_type_that_holds_it`,
`crates/lang-rules/src/adapter/csharp.rs:2630: assert!(generated.contains(expected), "{expected}\n{generated}");`

**Status:** done

### 7.2.4.2.1 IDL sequence -> Omg.Types.ISequence<T>

**Spec:** §7.2.4.2.1, p. 10 — "IDL sequences shall be mapped to the C#
Omg.Types.ISequence<T> interface [...]. Implementations of
Omg.Types.ISequence<T> shall extend
System.Collections.Generic.IList<T>."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::type_at` (array
arm), `crates/idl-csharp/runtime/Omg.Types.cs`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_sequence_and_an_array_are_both_csharp_arrays`

**Status:** partial — sequences are emitted as `T[]`; the runtime
declares `ISequence<T>`/`IBoundedSequence<T>`, but the materialiser does
not reference it.

### 7.2.4.2.1 Tab.7.4: sequences of basic types

**Spec:** §7.2.4.2.1, Tab.7.4, p. 11-12 — `sequence<boolean>` ->
`Omg.Types.ISequence<bool>`, ... `sequence<long double>` ->
`Omg.Types.ISequence<decimal>`.

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::type_at`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_sequence_and_an_array_are_both_csharp_arrays`

**Status:** partial — the element types match, the container type is
`T[]` instead of `Omg.Types.ISequence<T>`.

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

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::fields_of`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_sequence_and_an_array_are_both_csharp_arrays`

**Status:** partial — members are public, writable fields, not
properties; `@external` does not change the C# form.

### 7.2.4.2.2 IDL string -> C# string (UTF-16)

**Spec:** §7.2.4.2.2, p. 12 — "IDL strings, both bounded and unbounded
variants, shall be mapped to C# strings. The resulting strings shall be
encoded in UTF-16 format."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::type_at` (string arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_map_is_an_array_of_a_generated_entry_class`,
`crates/lang-rules/src/adapter/csharp.rs:2715: assert!(generated.contains("public string Value;"), "{generated}");`

**Status:** done

### 7.2.4.2.3 IDL wstring -> C# string (UTF-16)

**Spec:** §7.2.4.2.3, p. 12 — as §7.2.4.2.2 for `wstring`.

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::type_at` (string arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** partial — no test checks that IDL `wstring` maps to C# `string`; the cited corpus run only checks that the cases render.

### 7.2.4.2.4 IDL fixed -> C# decimal + ArithmeticException

**Spec:** §7.2.4.2.4, p. 12 — "The IDL fixed type shall be mapped to
the C# decimal type. Range checking shall raise a
System.ArithmeticException exception, or a derived exception, if
necessary."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::type_at` (fixed arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_fixed_is_a_string_and_not_a_decimal`

**Status:** partial — `fixed<P,S>` is held as a `string` carrying the
decimal; the spec demands `decimal`. Reason: `decimal` has 28-29
digits, IDL 4.2 permits up to 31 (evidenced in the module doc).

### 7.2.4.3.1 IDL struct -> C# public class

**Spec:** §7.2.4.3.1, p. 13 — "An IDL struct shall be mapped to a C#
public class [...] A public property [...] A public default
constructor [...] A public copy constructor [...] A public
constructor that accepts parameters for each member [...] The class
shall implement the IEquatable<T> interface."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::fields_of`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_field_carries_the_idl_name_it_was_folded_from`,
`crates/lang-rules/src/adapter/csharp.rs::tests::a_member_that_would_collide_with_its_type_cannot_come_from_idl`

**Status:** partial — a `public sealed class` with one public field per
member is produced; properties, the copy/all-values constructors and
`IEquatable<T>` are missing, and initialisation is C# default
initialisation.

### 7.2.4.3.2 IDL union -> C# public class (discriminator protocol)

**Spec:** §7.2.4.3.2, p. 14-15 — "A public read-only property named
Discriminator [...] A public property with getters and setters for each
member [...] Property getters shall raise a System.InvalidOperationException
if the expected member has not been set [...] implement the
IEquatable<T> interface."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare` (union arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`

**Status:** partial — the class carries `_d` and one field per case;
the `Discriminator` property, getter pairs, `Set<Member>` modifiers,
`InvalidOperationException` and `IEquatable<T>` are missing.

### 7.2.4.3.3 IDL enum -> C# public enum

**Spec:** §7.2.4.3.3, p. 18 — "An IDL enum shall be mapped to a C#
public enum with the same name as the IDL enum type."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare` (enum arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_enumeration_is_a_real_enum_and_a_member_of_it_is_that_type`,
`crates/lang-rules/src/adapter/csharp.rs::tests::an_explicit_enumerator_value_is_written_out`,
`crates/lang-rules/src/adapter/csharp.rs:2803: generated.contains("public enum Colour : int"),`

**Status:** done

### 7.2.4.3.3 @bit_bound underlying type for enums

**Spec:** §7.2.4.3.3, p. 18 — "the corresponding C# enum type shall be
sbyte for bit bound values between 1 and 8; short, for [...] 9 and 16;
int, for [...] 17 and 32; and long, for [...] 33 and 64."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare` (enum arm,
`underlying`)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_enumeration_is_a_real_enum_and_a_member_of_it_is_that_type`

**Status:** partial — `: int` is always written; `@bit_bound` selects
no underlying type.

### 7.2.4.3.4 Constructed recursive types

**Spec:** §7.2.4.3.4, p. 18 — "Constructed recursive types are
supported by mapping the involved types directly to C#."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::data_class`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`,
`crates/lang-rules/src/adapter/csharp.rs:2581: assert!(failures.is_empty(), "{failures:#?}");`

**Status:** done — generated aggregate types are C# classes (reference
semantics); the recursive corpus cases (`recursive-*`,
`resolve-forward-declared`) render.

### 7.2.4.4 IDL array -> C# array + bounds check

**Spec:** §7.2.4.4, p. 19 — "An IDL array shall be mapped to a C# array
[...]. The bounds for the array shall be checked by the setter of the
corresponding property and a System.ArgumentOutOfRangeException shall
be raised if a bounds violation occurs."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::type_at` (array arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_sequence_and_an_array_are_both_csharp_arrays`

**Status:** partial — arrays are emitted as jagged `T[][]` fields;
there is no setter with a length check (fields instead of properties).

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

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::type_at` (alias arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_typedef_is_a_using_alias_and_every_use_takes_the_resolved_type`,
`crates/lang-rules/src/adapter/csharp.rs:2869: assert!(generated.contains("using Row = int[];"), "{generated}");`

**Status:** done — a `using` alias is not a type of its own, and every
use takes the resolved target type.

---

## §7.3 Any

### 7.3 IDL any -> Omg.Types.Any

**Spec:** §7.3, p. 22 — "The IDL any type shall be mapped to
Omg.Types.Any type."

**Repo:** `crates/idl-csharp/runtime/Omg.Types.cs` (`Any`),
`crates/lang-rules/src/adapter/csharp.rs::type_at` (any arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_construct_with_no_csharp_data_form_is_named_and_not_left_out`

**Status:** partial — the runtime declares `Omg.Types.Any`, but the
materialiser refuses `any` by name (`MaterialiseError::NoLanguageForm`)
instead of mapping to it.

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

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::interfaces`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_interface_is_an_interface_of_signatures_with_the_idl_passing_modes`,
`crates/lang-rules/tests/csharp_consumer_assembly.rs::a_second_assembly_implements_the_generated_interface_operations_and_attributes`

**Status:** partial — methods and attribute properties as well as
`in`/`out`/`ref` match; the `I` prefix is missing (name verbatim), and
derived interfaces are flattened into the single declaration instead of
inheriting through a base list.

### 7.4.1 IDL exception -> C# class : System.Exception

**Spec:** §7.4.1, p. 23 — "An IDL exception shall be mapped to a C#
class [...]. The mapped class shall extend the System.Exception class."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::exceptions`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_exception_is_a_data_class_and_not_a_system_exception`

**Status:** partial — the class exists with one field per member but
deliberately does not derive from `System.Exception` (reason: CS0108/
CS0542 for members like `message`; evidenced in the module doc).

### 7.4.2 Interface forward declaration

**Spec:** §7.4.2, p. 23 — "An IDL interface forward declaration has no
mapping to the C# language."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::interfaces`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::a_forward_declaration_without_a_definition_is_e002`,
`crates/lang-rules/tests/interface_bindings.rs:272: Some("DDS-TS-E002")`

**Status:** `partial` — the quoted test checks that a forward declaration without a definition is refused as E002; none checks that one with a definition produces no declaration of its own.

---

## §7.5 Interfaces – Full

### 7.5 I<Name>Operations + I<Name> + <Name> class

**Spec:** §7.5, p. 23-24 — "A C# public interface named
I<InterfaceName>Operations [...] A C# public interface named
I<InterfaceName> inheriting from I<InterfaceName>Operations [...] A C#
public class named <InterfaceName> containing the declaration of the
classes and exceptions that result from the mapping of all the types
and exceptions declared within the IDL interface."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::interfaces`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_interface_is_an_interface_of_signatures_with_the_idl_passing_modes`

**Status:** partial — exactly one interface declaration is produced;
the §7.5 Operations/container split and the embedded types in a
`<Name>` class are missing.

---

## §7.6 Value Types

### 7.6 IDL valuetype -> abstract class + factory attribute

**Spec:** §7.6, p. 25 — "An IDL valuetype type shall be mapped to a C#
abstract class. [...] private members shall have the C# protected
access modifier [...] Each valuetype factory operation shall be mapped
[...] to a method returning void [...] annotated with the
FactoryAttribute."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare` (valuetype arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_valuetype_is_a_class_of_its_state_members`

**Status:** partial — a `sealed class` with the state members is
produced; the abstract class, the public/private -> protected split,
operations and the `FactoryAttribute` are missing.

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

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the clauses demand a mapping to
intermediate IDL, not to C#; the C# PSM has no subject of its own here.

---

## §7.14 Extended Data Types

### 7.14.1 Struct with single inheritance -> C# extends

**Spec:** §7.14.1, p. 27 — "An IDL struct that inherits from a base IDL
struct, shall be declared as a C# public class that extends the class
resulting from mapping the base IDL struct."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare` (structure arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** partial — the base members are folded into the derived
`sealed class` (XTypes 1.3 layout), but there is no C# base class
(`extends`).

### 7.14.2 Union discriminators: int8, uint8, wchar, octet

**Spec:** §7.14.2, p. 28 — "This building block adds the int8, uint8,
wchar, and octet IDL types to the set of valid types for a
discriminator."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_member_type`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`,
`crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`,
`crates/lang-rules/src/adapter/csharp.rs:2581: assert!(failures.is_empty(), "{failures:#?}");`

**Status:** done — `csharp_primitive` covers every integral
discriminator type; the corpus cases `union-discriminator-*` render.

### 7.14.3.1 IDL map -> IDictionary<TKey,TValue>

**Spec:** §7.14.3.1, p. 28 — "An IDL map shall be mapped to a C#
generic System.Collections.Generic.IDictionary<TKey,TValue>."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::map_entry_name`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_map_is_an_array_of_a_generated_entry_class`

**Status:** partial — maps are emitted as `ZdEntry_<k>_<v>[]` (an array
of a generated two-field class), not as `IDictionary`. The reason is
the order axis (`MapEntryOrder`), evidenced in the module doc.

### 7.14.3.2 IDL bitset -> C# struct + IEquatable

**Spec:** §7.14.3.2, p. 29 — "An IDL bitset shall map to a C# struct
with public properties for each named bitfield [...]. The mapped C#
struct shall implement the IEquatable<T> interface."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare` (bitset arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_bitset_is_a_class_of_one_field_per_bitfield`

**Status:** partial — a class with one public field per bitfield
(correct holder width) is produced, not a `struct` with properties and
`IEquatable<T>`.

### 7.14.3.3 IDL bitmask -> C# enum `[Flags]` + BitArray

**Spec:** §7.14.3.3, p. 30 — "The IDL bitmask type shall map to a C#
public enum with the same name, followed by the "Flags" suffix. [...] a
System.Collections.BitArray shall be used. The C# enum shall have the
System.FlagsAttribute [...]. The size [...] determines the
corresponding C# enum type."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare` (bitmask arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_bitmask_is_a_flags_enum_over_the_holder_its_bound_calls_for`

**Status:** partial — a `[Flags]` enum with the correct holder and
position values is in place; the `Flags` suffix on the name and the
`BitArray` use are missing.

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

**Repo:** —

**Tests:** —

**Status:** open — the materialiser refuses annotation types
(`TypeKind::AnnotationType`); no attribute class is produced.

### 7.16.2 Applying annotations

**Spec:** §7.16.2, p. 32 — "IDL elements annotated with user-defined
annotations shall map to equivalent C# elements annotated with the
corresponding attribute."

**Repo:** —

**Tests:** —

**Status:** open — user annotations are not propagated; the only
emitted annotation is the internal `[IdlName]`.

### 7.16.2.1 Annotations on typedefs propagate

**Spec:** §7.16.2.1, p. 33 — "Annotations on an IDL typedef shall be
applied to uses of the typedef in other type declarations."

**Repo:** —

**Tests:** —

**Status:** open — there is no attribute propagation, neither through
typedefs nor otherwise.

---

## §7.17 Standardized Annotations

### 7.17.1 @id, @autoid (no impact)

**Spec:** §7.17.1, Tab.7.5, p. 33-35 — "@id No impact on language
mapping; @autoid No impact on language mapping."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::fields_of`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** partial — no test compares the C# form with and without
`@id`/`@autoid`; the cited corpus run only checks that the cases render.

### 7.17.1 @optional -> Nullable<T> + OptionalAttribute

**Spec:** §7.17.1, Tab.7.5, p. 34 — "IDL elements annotated with
@optional whose type T maps to a C# value type shall map to
System.Nullable<T>. [...] All mapped optional elements shall be
annotated with OptionalAttribute."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::optional`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_optional_value_type_is_nullable_and_a_reference_one_is_not`,
`crates/lang-rules/src/adapter/csharp.rs::tests::an_optional_enum_is_nullable_because_an_enum_is_a_value_type`

**Status:** partial — value types become `T?`, reference types stay
unchanged; the `Omg.Types.OptionalAttribute` is not emitted.

### 7.17.1 @position, @value

**Spec:** §7.17.1, Tab.7.5, p. 35 — "@position Impacts the mapping of
bitmask types [...]; @value Impacts the mapping of enum types."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_explicit_enumerator_value_is_written_out`,
`crates/lang-rules/src/adapter/csharp.rs::tests::a_bitmask_is_a_flags_enum_over_the_holder_its_bound_calls_for`,
`crates/lang-rules/src/adapter/csharp.rs:2819: assert!(generated.contains("LOW = -1,"), "{generated}");`

**Status:** done

### 7.17.1 @extensibility, @final, @mutable, @appendable (no impact)

**Spec:** §7.17.1, Tab.7.5, p. 35 — "No impact on language mapping."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::data_class`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** partial — no test compares the C# form of the
`@extensibility` variants; the cited corpus run only checks that the
cases render.

### 7.17.2 @key -> KeyAttribute

**Spec:** §7.17.2, Tab.7.6, p. 35 — "IDL elements annotated with @key
shall result in equivalent C# elements annotated with KeyAttribute."

**Repo:** —

**Tests:** —

**Status:** open — the materialiser does not evaluate `@key` and emits
no `[Key]`; `Omg.Types.KeyAttribute` sits unused in the runtime.

### 7.17.2 @must_understand (no impact)

**Spec:** §7.17.2, Tab.7.6, p. 35 — "@must_understand No impact on
language mapping."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::fields_of`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** partial — no test compares the C# form with and without
`@must_understand`; the cited corpus run only checks that the case
renders.

### 7.17.2 @default_literal

**Spec:** §7.17.2, Tab.7.6, p. 35 — "The C# element [...] shall be
initialized to element indicated by the annotation."

**Repo:** —

**Tests:** —

**Status:** open — `@default_literal` is not evaluated; the C# enum
default remains the value 0.

### 7.17.3 Units and ranges: @default, @range, @min, @max, @unit

**Spec:** §7.17.3, Tab.7.7, p. 37 — "@default [...] initialized to the
value of the annotation; @range/@min/@max [...] shall throw a
System.ArgumentOutOfRangeException [...]; @unit Shall result in the
addition of a UnitAttribute."

**Repo:** —

**Tests:** —

**Status:** open — neither default initialisation nor range checks nor
`UnitAttribute` are implemented.

### 7.17.4 @bit_bound

**Spec:** §7.17.4, Tab.7.8, p. 38 — "@bit_bound Impacts the mapping of
bitmask as described in Clause 7.14.3.3."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::unsigned_holder`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_bitmask_is_a_flags_enum_over_the_holder_its_bound_calls_for`,
`crates/lang-rules/src/adapter/csharp.rs:2827: assert!(narrow.contains("public enum Small : byte"), "{narrow}");`

**Status:** done — the holder follows `byte`/`ushort`/`uint`/`ulong` by
the bound.

### 7.17.4 @external

**Spec:** §7.17.4, Tab.7.8, p. 38 — "@external [...] map to
System.Nullable<T> [...] Properties created as a result of mapping IDL
sequences annotated with @external shall map to the C#
System.Collections.Generic.IEnumerable<T> interface."

**Repo:** —

**Tests:** —

**Status:** open — `@external` does not change the C# form; there is
neither an `ExternalAttribute` nor `IEnumerable<T>` for external
sequences.

### 7.17.4 @nested (no impact)

**Spec:** §7.17.4, Tab.7.8, p. 38 — "@nested No impact to the language
mapping."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** partial — no test compares the C# form with and without
`@nested`; the cited corpus run only checks that the case renders.

### 7.17.5 @verbatim

**Spec:** §7.17.5, Tab.7.9, p. 38 — "Copies verbatim text to the
indicated output position when the indicated language is "*", "c#",
"cs", or "csharp"."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::weave_verbatim`

**Tests:** `crates/lang-rules/tests/verbatim_end_to_end.rs::csharp_puts_begin_and_end_inside_the_type_body`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::the_wildcard_reaches_every_column`,
`crates/lang-rules/tests/verbatim_end_to_end.rs:234: opening < begin && begin < field && field < end,`

**Status:** done

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

**Repo:** —

**Tests:** —

**Status:** open — the annotation is neither recognised nor evaluated.

### 8.1.1 apply_naming_convention

**Spec:** §8.1.1, p. 41 — "If apply_naming_convention is
IDL_NAMING_CONVENTION [...] If [...] DOTNET_NAMING_CONVENTION [...]."

**Repo:** —

**Tests:** —

**Status:** open — no switch; the naming shape is fixed.

### 8.1.2 constants_container

**Spec:** §8.1.2, p. 42 — "To enable the Standalone Constants Mapping
[...] constants_container shall be set to an empty string. To enable
the Constants Container Mapping [...] set to a valid string."

**Repo:** —

**Tests:** —

**Status:** open — there is only one fixed container class
(`ZdConstants`); the parameter is not implemented.

### 8.1.3 struct_type

**Spec:** §8.1.3, p. 43 — "struct_type defines the C# type the IDL
struct type map to. By default [...] C# class. This parameter allows
changing the default behavior to map an IDL struct to a C# struct."

**Repo:** —

**Tests:** —

**Status:** open — IDL structs are always emitted as `class`; the
parameter is not implemented.

---

## Annex A: Platform-Specific Mappings

### A.1 CORBA-specific mappings (A.1.1-A.1.6)

**Spec:** Annex A.1, p. 45-48 — `Corba.UserException` for exceptions,
`Corba.TypeCode`, `Corba.Object`/`Corba.IObject`, `Corba.Any`, the
`Corba.IObject` inheritance for interfaces and the value-type rules.

**Repo:** —

**Tests:** —

**Status:** open — the C# column has no CORBA backend. An
`emit_corba_traits` exists only in `crates/corba-codegen/src/traits.rs`
(Rust CORBA), not in the C# column; no `Corba.*` types are emitted.

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

14 done / 33 partial / 15 open / 17 n/a (informative) / 0 n/a (rejected).

The PSM generator `crates/idl-csharp/src/` was dropped in
`d045e43ea86`; the cited tests are those of today's wire column
(`crates/lang-rules/`, `crates/lang-cst/`, `crates/lang-render/`) and
of the surviving runtime `crates/idl-csharp/runtime/Omg.Types.cs`.
No test run is part of this audit (documentation change); the citations
name the test that holds the item.
