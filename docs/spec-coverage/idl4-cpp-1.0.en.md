# IDL4 to C++ Language Mapping 1.0 — Spec Coverage

**Spec:** [OMG IDL4-CPP 1.0](https://www.omg.org/spec/IDL4-CPP/1.0/PDF) (101 pages, OMG formal/25-03-03)

Item-by-item audit against the spec; every requirement with its spec
quote, repo path, test path and status (`done` / `partial` / `open` /
`n/a`).

**Context:** the C++ column of the IDL transplant lives in four layers
today:

- `crates/lang-rules/src/adapter/cpp.rs` — IDL data types (struct, union,
  enum, bitset, bitmask, typedef, constant, exception, sequence, array,
  map, string, `@optional`, `@external`) to the C++ CST.
- `crates/lang-cst/src/cpp/mod.rs` — the C++ CST (`escape_keyword`,
  `Include`, `Primitive`).
- `crates/lang-render/src/cpp.rs` — the renderer.
- `crates/lang-rules/src/emit/interface_cpp.rs` — interfaces through the
  DDS-RPC 1.0 §7.11.1 binding (not the generic §7.4 form).

Hand-written runtime, not generated from this spec:

- `crates/cpp/include/dds/` — the DDS-PSM-Cxx 1.0 API (Time,
  InstanceHandle, Status, Fixed, Exception, Topic, TopicTraits, Publisher,
  Subscriber, DomainParticipant).
- `runtime/cpp/include/` — the wire runtime (`zerodds/wire.hpp`,
  `dds/rpc.hpp`); the corpus gate compiles against it.

**Finding that carries the page:** the C++ column emits no `omg::types`
anywhere. `grep -rl "omg::types" crates/ tools/ runtime/ endpoints/`
finds exactly one file — `crates/corba-codegen/src/traits.rs`, a Rust
CORBA generator, not this C++ binding. Every item whose requirement is an
`omg::types` alias or trait is therefore `open` or `partial`.

---

## §1 Scope

### 1.1 Mapping IDL v4 -> C++

**Spec:** §1, p. 1 — "This specification defines the mapping of OMG
Interface Definition Language v4 to the C++ programming language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a scope statement of the specification,
with no requirement on an implementation.

---

## §2 Conformance Criteria

### 2.1 Implementation: IDL -> C++ source per Chapter 7

**Spec:** §2, p. 1 — "A conformant implementation shall transform IDL
input into C++ source code output as specified in Chapter 7."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::every_corpus_case_renders_or_names_its_refusal`,
`crates/lang-rules/tests/cpp_corpus_compiles.rs::the_compiler_check_catches_a_deliberate_error`

**Status:** partial — the data-type transformation is live and renders
every corpus case (or names its refusal); the compiler gate checks the
generated headers. Chapter 7 is not fully met: the `omg::types` runtime
(type traits, `Any`, `fixed`, `sequence`/`string` aliases) is missing,
§7.4 interfaces run through the DDS-RPC binding rather than the generic
form, and §7.4.1 exceptions and §7.6 valuetypes are shaped differently.

---

## §3 Normative References

### 3.1 [ISO/IEC-14882] and [OMG-*]

**Spec:** §3, p. 1-2 — [ISO/IEC-14882:1998/2003/2011/2017], [OMG-C++],
[OMG-C++11], [OMG-CORBA-COMP], [OMG-CORBA-IFC], [OMG-IDL4].

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a reference list; the C++ version is
audited in §7.1.3, the IDL version in §3.9.

---

## §4 Terms and Definitions

### 4.1 Building Block, C++, Language Mapping

**Spec:** §4, p. 2 — glossary (Building Block, C++, Language Mapping).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — definitions without a code requirement.

---

## §5 Symbols

### 5.1 Acronyms (Tab.5.1)

**Spec:** §5 Tab.5.1, p. 2-3 — CCM/CORBA/DDS/IDL.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## §6 Additional Information

### 6.1-6.3 Changes, Acknowledgments, Intellectual Property Rights

**Spec:** §6.1-6.3, p. 3.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## §7.1 General

### 7.1.1 Names

**Spec:** §7.1.1, p. 5 — "IDL member names and type identifiers shall
map to equivalent C++ names and type identifiers with no change."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::a_sequence_is_a_vector_and_a_map_is_a_class_over_a_vector_of_pairs` — `crates/lang-rules/src/adapter/cpp.rs:2752: assert!(text.contains("std::vector<std::int32_t> items;"), "{text}");`

**Status:** done — the member name `items` stays unchanged. The only name
change is the reserved-name escaping of §7.1.2.

### 7.1.2 Reserved Names

**Spec:** §7.1.2, p. 5-6 — "The use of any of these names for a
user-defined IDL type or interface (assuming it is also a legal IDL name)
shall result in the mapped name having an underscore ('_') prepended."

**Repo:** `crates/lang-cst/src/cpp/mod.rs` (`escape_keyword`)

**Tests:** `crates/lang-cst/src/cpp/mod.rs::a_keyword_escapes_with_a_trailing_underscore` — `crates/lang-cst/src/cpp/mod.rs:924: assert_eq!(escape_keyword("class"), "class_");`

**Status:** partial — collisions with C++ keywords are detected and
rewritten, but with a **trailing** underscore (`class` -> `class_`), not
with the **leading** underscore §7.1.2 mandates (`_class`). Detection is
complete; the applied mapping deviates.

### 7.1.3 C++ Language Version Requirements

**Spec:** §7.1.3, p. 6 — "The language mappings defined in this
specification require C++11 [ISO/IEC-14882:2011] as the minimum C++
standard version."

**Repo:** `crates/lang-rules/tests/cpp_corpus_compiles.rs`

**Tests:** `crates/lang-rules/tests/cpp_corpus_compiles.rs::every_corpus_case_that_renders_also_compiles` — `crates/lang-rules/tests/cpp_corpus_compiles.rs:159: .args(["-std=c++20", "-fsyntax-only", "-Wall", "-Wextra", "-Werror"])`

**Status:** done — the corpus is syntax-checked under `-std=c++20`; that
meets the C++11 minimum and covers the C++17 constructs actually used
(`std::optional`, `inline constexpr`). The corpus test is `#[ignore]` and
runs with `--include-ignored`.

### 7.1.4 IDL Type Traits (Tab.7.1 and Tab.7.4-7.12)

**Spec:** §7.1.4 Tab.7.1, p. 6-7 — "Type traits shall be available under
the omg::types namespace" (`value_type`, `in_type`, `out_type`,
`inout_type`, `_t`/`_v` aliases); plus the specialisations Tab.7.4
(sequences), Tab.7.5 (strings), Tab.7.6 (wstrings), Tab.7.7 (`digits`,
`scale`), Tab.7.8 (`bit_bound`, `underlying_type`), Tab.7.9
(`dimensions`), Tab.7.10/7.11 (`is_bounded`, `bound`, `key`, `elements`)
and Tab.7.12 (`bit_bound`, `underlying_type`).

**Repo:** —

**Tests:** —

**Status:** open — no `omg::types` namespace. `grep -rl "omg::types"
crates/ tools/ runtime/ endpoints/` finds only
`crates/corba-codegen/src/traits.rs` (a Rust CORBA generator). The C++
column writes `std::` types directly; no trait of Tables 7.1 and 7.4-7.12
is emitted.

---

## §7.2 Core Data Types

### 7.2.1 IDL Specification

**Spec:** §7.2.1, p. 7 — "There is no direct mapping of the IDL
Specification itself."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### 7.2.2 Modules

**Spec:** §7.2.2, p. 7 — "IDL modules shall be mapped to C++ namespaces
of the same name. [...] IDL declarations not enclosed in any module shall
be mapped into the global scope."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::a_module_is_a_namespace_and_not_a_prefix` — `crates/lang-rules/src/adapter/cpp.rs:1987: assert!(text.contains("namespace a {"), "{text}");`

**Status:** done — modules become `namespace` blocks, references are
written from the root scope (`::a::P`), no `a_b_P` prefix.

### 7.2.3 Constants

**Spec:** §7.2.3, p. 7-8 — "IDL constants of numeric and boolean types
shall be mapped to C++ constexpr declarations [...] IDL constants of
string type shall be mapped to a constexpr declaration of
omg::types::string_view type [...] of wstring type [...] omg::types::wstring_view
[...] The constant value of wide character and wide string constants
shall be preceded by L."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (constant loop,
`cpp_constant`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::every_corpus_case_renders_or_names_its_refusal`

**Status:** partial — numeric and boolean constants are emitted as
`inline constexpr`. String, wstring and fixed constants are emitted as
`const char *`, not as `constexpr omg::types::string_view` or
`std::string_view`; the `L` prefix for wide literals is missing because
`wstring` is held as UTF-8. No assertion covers the constant form
directly.

---

## §7.2.4 Data Types

### 7.2.4.1 Basic Types (Tab.7.2/7.3)

**Spec:** §7.2.4.1.1-§7.2.4.1.6, p. 8-9 — integers (Tab.7.2), floating
point (Tab.7.3), char, wchar, boolean, octet.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`cpp_primitive`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::the_tag_keeps_apart_what_the_cpp_spelling_collapses`,
`crates/lang-rules/src/adapter/cpp.rs::an_array_nests_with_the_outer_dimension_outermost`

**Status:** partial — `long -> std::int32_t` is asserted (array,
interface and verbatim tests), and `octet`/`uint8` as well as `wchar`/
`unsigned short` collapse onto one type (tag test). The remaining table
widths (`short -> int16_t`, `long long -> int64_t`, `char`, `wchar`,
`boolean`, `octet`, `float`) have no assertion of their own. `long double`
is deliberately mapped to `zerodds::wire::Float128` instead of the
platform-dependent C++ `long double` (§7.2.4.1.2). `cpp_primitive` is
exhaustive, so a missing primitive is a compile error.

### 7.2.4.2.1 Sequences -> std::vector<T>

**Spec:** §7.2.4.2.1, p. 9 — "IDL sequences shall be mapped to a C++
std::vector<T>, or to a type named omg::types::sequence<T> [...]."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`value_type`, sequence
arm)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::a_sequence_is_a_vector_and_a_map_is_a_class_over_a_vector_of_pairs` — `crates/lang-rules/src/adapter/cpp.rs:2752: assert!(text.contains("std::vector<std::int32_t> items;"), "{text}");`

**Status:** done — `std::vector<T>` is one of the forms §7.2.4.2.1 allows
(the `omg::types::sequence` alternative is audited in §7.1.4).

### 7.2.4.2.1 omg::types::sequence/bounded_sequence

**Spec:** §7.2.4.2.1 Tab.7.4, p. 9 — "implementers shall always define
omg::types::sequence<T> [...] omg::types::bounded_sequence<T,N> [...] is_bounded
[...] bound."

**Repo:** —

**Tests:** —

**Status:** open — neither `omg::types::sequence<T>` nor
`omg::types::bounded_sequence<T,N>` nor the `is_bounded`/`bound`
specialisations exist.

### 7.2.4.2.2 Strings -> std::string

**Spec:** §7.2.4.2.2, p. 10 — "IDL strings shall be mapped to C++
std::string, or to a type named omg::types::string."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`value_type`, string
arm)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::the_tag_keeps_apart_what_the_cpp_spelling_collapses`

**Status:** partial — `string` is held as `std::string` (an allowed
form). There is no direct assertion for the data member (only the tag
test shows that `string` and `wstring` fall onto the same C++ type);
`omg::types::string` is missing.

### 7.2.4.2.2 omg::types::string/bounded_string

**Spec:** §7.2.4.2.2 Tab.7.5, p. 10 — "implementers shall always define
omg::types::string [...] omg::types::bounded_string<N> [...] is_bounded
[...] bound."

**Repo:** —

**Tests:** —

**Status:** open — no `omg::types::string`/`bounded_string<N>` aliases,
no `is_bounded`/`bound` traits.

### 7.2.4.2.3 Wstrings

**Spec:** §7.2.4.2.3, p. 11 — "IDL wstrings shall be mapped to C++
std::wstrings, or to a type named omg::types::wstring [...]."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`value_type`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::the_tag_keeps_apart_what_the_cpp_spelling_collapses`

**Status:** partial — `wstring` is held as `std::string` with UTF-8, not
as `std::wstring` and not as `omg::types::wstring`; the spec form is not
met. No direct assertion.

### 7.2.4.2.3 omg::types::wstring/bounded_wstring

**Spec:** §7.2.4.2.3 Tab.7.6, p. 11 — "omg::types::wstring [...]
omg::types::bounded_wstring<N> [...] is_bounded [...] bound."

**Repo:** —

**Tests:** —

**Status:** open — no `omg::types::wstring`/`bounded_wstring<N>`, no
traits.

### 7.2.4.2.4 Fixed Type

**Spec:** §7.2.4.2.4, p. 11-12 — "The IDL fixed type shall map to a C++
class named fixed [...] under the omg::types namespace" (about 30 methods
plus free functions and the `digits`/`scale` traits Tab.7.7).

**Repo:** —

**Tests:** —

**Status:** open — the data-type column maps `fixed<P,S>` to `std::string`
in `crates/lang-rules/src/adapter/cpp.rs`. The hand-written
`dds::core::Fixed<P,S>` in `crates/cpp/include/dds/core/Fixed.hpp` is the
DDS-PSM-Cxx runtime (a different spec) and offers only BCD round-trip plus
string conversion, not the ~30 spec methods; `omg::types::fixed` and the
`digits`/`scale` traits are missing.

### 7.2.4.3.1 Structures


**Spec:** §7.2.4.3.1, p. 13 — "An IDL struct shall be mapped to a C++
struct [...] default constructor [...] copy constructor [...] move
constructor [...] copy assignment [...] move assignment [...] comparison
operators, including at least 'equal to' and 'not equal to' [...]
destructor [...] swap function in the namespace."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs` (class with accessors, `==`, `!=`, `swap`), `crates/lang-rules/src/adapter/cpp.rs` (structure branch)

**Tests:** `crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader` (`crates/cpp/tests/generated_psm_roundtrip.cpp`: constructors, copy, move, `swap`, equality)

**Status:** done — the default mapping (`--cpp-mapping psm`) writes a class with the accessors, the constructor over all members, `==`, `!=` and `swap` in the namespace (§7.12); copy and move are the implicit functions. The option `native` writes the structure of the codecs with public members, `swap` and, for pointer members, the generated copy and move functions, but no `==`. Log: `.agent/REPORT.md`.

### 7.2.4.3.1 Default initialization

**Spec:** §7.2.4.3.1, p. 13 — "A default constructor that initializes all
built-in data types to their default value [...] enumerators to their
first value, and the rest of members using their default constructor."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`member_default`)

**Tests:** `tools/idlc/tests/cpp_headers_together.rs::two_generated_headers_share_a_translation_unit_and_their_structs_start_at_their_defaults` — `tools/idlc/tests/cpp_headers_together.rs:126: status.success(),`

**Status:** done — the test compiles and runs default-constructed
structs; the exit code is non-zero if a built-in is not 0, `bool` is not
`false`, `double` is not 0.0, the enum is not its first literal, or an
array/sequence is not empty.

### 7.2.4.3.2 Unions

**Spec:** §7.2.4.3.2, p. 14-15 — class with `_d() const`/`void _d(T)`,
member accessor/modifier per branch, multi-label modifier with a
discriminator parameter, and `_default()`.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (union arm)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`

**Status:** partial — the union is emitted as a class with a `_d` field
and one field per branch (asserted: `struct U {`, `std::int32_t _d;`).
The required accessor/modifier methods, the multi-label variant and
`_default()` are missing; the branches are public fields.

### 7.2.4.3.3 Enumerations

**Spec:** §7.2.4.3.3, p. 16 — "An IDL enum shall be mapped to a C++
scoped enum class with the same name as the IDL enum type."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (enumeration arm)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::an_enumeration_takes_an_underlying_type_from_its_bit_bound` — `crates/lang-rules/src/adapter/cpp.rs:2050: assert!(narrow.contains("enum class E : std::uint8_t {"), "{narrow}");`

**Status:** done — a scoped `enum class` with the same name; the
underlying type is chosen from `@bit_bound` and the enumerator values.

### 7.2.4.3.3 Enumeration Type Traits

**Spec:** §7.2.4.3.3 Tab.7.8, p. 17 — "bit_bound [...] underlying_type."

**Repo:** —

**Tests:** —

**Status:** open — the `bit_bound`/`underlying_type` specialisations are
not emitted (see §7.1.4).

### 7.2.4.3.4 Constructed Recursive Types

**Spec:** §7.2.4.3.4, p. 16 — "Constructed recursive types are supported
by mapping the involved types directly to C++."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (forward declarations,
sequence/`shared_ptr` arms)

**Tests:** `crates/lang-rules/tests/external_is_an_indirection.rs::a_recursive_external_member_is_a_shared_pointer_and_not_a_value`

**Status:** partial — forward declarations for aggregates and
`std::vector<T>`/`std::shared_ptr<T>` allow self-reference; only the
`@external` recursion case is evidenced (`std::shared_ptr<::m::Node>
next;`). The general case (self-reference through `sequence<Node>` without
`@external`) has no assertion of its own.

### 7.2.4.4 Arrays

**Spec:** §7.2.4.4, p. 17 — "An IDL array shall be mapped to a C++
std::array of the mapped element type, or to a type named
omg::types::array [...] Multidimensional arrays shall be constructed
nesting std::array."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (array arm)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::an_array_nests_with_the_outer_dimension_outermost` — `crates/lang-rules/src/adapter/cpp.rs:2023: assert!(text.contains("std::array<std::array<std::int32_t, 3>, 2> a = {};"), "{text}");`

**Status:** done — `std::array`, multidimensional, nested
outermost-first; one of the forms §7.2.4.4 allows.

### 7.2.4.4 omg::types::array and dimensions

**Spec:** §7.2.4.4 Tab.7.9, p. 17 — "omg::types::array [...] dimensions."

**Repo:** —

**Tests:** —

**Status:** open — `omg::types::array` and the `dimensions` trait are
missing.

### 7.2.4.5 Native Types

**Spec:** §7.2.4.5, p. 17 — "This language mapping specification does not
define any native types, but compliant implementations may provide the
necessary mechanisms."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec defines no native types; the
adapter optionally maps `native` to `void *` or a facet name.

### 7.2.4.6 Naming Data Types

**Spec:** §7.2.4.6, p. 17 — "IDL typedefs shall be mapped to type alias
declarations."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (alias arm)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::every_corpus_case_renders_or_names_its_refusal`

**Status:** partial — `typedef` is emitted as a `cpp::Alias` (C++ `using`);
no assertion checks the alias declaration directly, and the corpus
compile does not distinguish an alias form from a macro form.

---

## §7.3 Any

### 7.3 Any -> omg::types::Any

**Spec:** §7.3, p. 18 — "The IDL any type shall be mapped to a C++
omg::types::Any type."

**Repo:** —

**Tests:** —

**Status:** open — `any` is refused in
`crates/lang-rules/src/adapter/cpp.rs` (`cpp_value_type`,
`TypeKind::Any`) with `NoLanguageForm` (test
`an_any_member_is_refused_by_name_at_any_depth`, also for
`sequence<any>`). No `omg::types::Any`.

---

## §7.4 Interfaces – Basic

### 7.4 Interfaces -> C++ classes

**Spec:** §7.4, p. 18-19 — "Each IDL interface shall be mapped to a C++
class [...] Each attribute [...] pair of pure virtual methods [...] Each
operation [...] pure virtual method [...] IDL out and inout arguments
shall be passed by reference [...] in arguments [...] by value [...] and
as a reference to const otherwise. Parameters of interface type T shall
be mapped to std::shared_ptr<T>, or to omg::types::ref_type<T>. [...]
weak_ref_type<T>."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_synchronous_operations_of_rule_six`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_names_an_attribute_accessor_after_the_attribute`

**Status:** partial — interfaces are emitted through the DDS-RPC 1.0
§7.11.1 binding as classes with pure-virtual operations and attribute
accessors (asserted: `in` by value/const ref, `out`/`inout` by reference,
accessor named after the attribute). The binding follows DDS-RPC 1.0 and
adds `IAsync`/`Requester`/`Replier`/`Client`/`Service`, which §7.4 does
not know; `omg::types::ref_type<T>`/`weak_ref_type<T>` for interface
parameters are missing.

### 7.4.1 Exceptions

**Spec:** §7.4.1, p. 19-20 — "An IDL exception shall be mapped to a C++
class [...] shall extend the std::exception class [...] copy constructor,
move constructor, assignment operator, move operator, and destructor
[...] what() method [...] explicit constructor with one parameter for
each exception member [...] and a parameter of type const char *."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (exception loop)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::every_corpus_case_renders_or_names_its_refusal`

**Status:** partial — an IDL exception is emitted as a `struct` with its
members. The required `class X : public std::exception` derivation,
`what()`, copy/move constructor, assignment operators and the explicit
`(member..., const char* what)` constructor are entirely missing.

### 7.4.2 Interface Forward Declaration

**Spec:** §7.4.2, p. 20 — "An IDL interface forward declaration shall be
mapped to a partial forward declaration in C++."

**Repo:** —

**Tests:** —

**Status:** open — interface forward declarations are not emitted; the
data-type adapter writes `Forward` only for `Structure`, `Union` and
`Valuetype`. `interface Foo;` without a definition is refused by
`crates/lang-rules/src/adapter/interface.rs` as `ForwardWithoutDefinition`
(test
`crates/lang-rules/tests/interface_bindings.rs::a_forward_declaration_without_a_definition_is_e002`).

---

## §7.5 Interfaces – Full

### 7.5 Embedded declarations and static constexpr

**Spec:** §7.5, p. 20 — "adding to mapped class the declaration every
type, exception, or constant declaration in the IDL interface body. [...]
constexpr declarations shall be marked as static."

**Repo:** —

**Tests:** —

**Status:** open — the DDS-RPC binding in
`crates/lang-rules/src/emit/interface_cpp.rs` emits topic types and
operation classes, but no nested type/exception/constant declarations
inside the interface body and no `static constexpr` inside the class.

---

## §7.6 Value Types

### 7.6 Valuetype -> abstract class

**Spec:** §7.6, p. 21 — public/private state members -> public/protected
pure-virtual accessor and modifier methods; factory operations ->
`<ValueTypeName>_factory`; inheritance via public virtual; supports
interface -> all operations as pure virtual.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (valuetype arm)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::every_corpus_case_renders_or_names_its_refusal`

**Status:** partial — `valuetype` is treated like a `struct`: its members
become fields. The required public/protected pure-virtual accessor/modifier
methods, the `<ValueTypeName>_factory` class, the public-virtual
inheritance and the operations of a supported interface are missing.

---

## §7.7-§7.13 CORBA, Components and Template Modules

### 7.7/7.8 CORBA-Specific Interfaces and Value Types

**Spec:** §7.7-7.8, p. 22 — refers to Annex A.1.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the clause itself defines no mapping;
Annex A.1 is audited separately.

### 7.9/7.10 Components – Basic and Homes

**Spec:** §7.9-7.10, p. 22 — "mapped to equivalent IDL, as specified in
[OMG-CORBA-COMP]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no direct C++ mapping; the path is
intermediate-IDL tooling, not codegen.

### 7.11 CCM-Specific

**Spec:** §7.11, p. 22 — refers to Annex A.1.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### 7.12/7.13 Components – Ports/Connectors and Template Modules

**Spec:** §7.12-7.13, p. 22 — "mapped to intermediate IDL, as specified
in [OMG-IDL4]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — intermediate-IDL path with no direct
C++ mapping.

---

## §7.14 Extended Data Types

### 7.14.1 Structures with Single Inheritance

**Spec:** §7.14.1, p. 23 — "shall inherit from the mapped base struct
using public inheritance. [...] swap method [...] which shall ensure that
all inherited members are swapped."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`all_members_of`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::every_corpus_case_renders_or_names_its_refusal`

**Status:** partial — base members are copied flat into the derived
struct (`all_members_of`), not as `struct Child : Base` with public
inheritance; the `swap` function for inherited members is missing. The
flat form is deliberate so the member order matches the wire plan.

### 7.14.2 Union Discriminators

**Spec:** §7.14.2, p. 23 — "adds the wchar, and octet IDL types to the set
of valid types for a discriminator. [...] int8 and uint8 are valid union
discriminators."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (union arm)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`

**Status:** partial — the discriminator runs through `cpp_value_type` and
accepts any integral type; only `switch (long)` is asserted. `wchar`,
`octet`, `int8` and `uint8` as discriminators have no assertion of their
own.

### 7.14.3.1 Maps


**Spec:** §7.14.3.1, p. 23-24 — "IDL maps shall be mapped to a C++
std::map<Key,T> [...] or to a type named omg::types::map<Key,T> [...]
omg::types::bounded_map<Key, T, N>."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs` (map branch)

**Tests:** `crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** done — in the default mapping (`psm`) `map<K,V>` is `std::map<K,V>`; `--cpp-mapping native` and `classic` are named options (`zerodds::Map`, the order of the entries as an axis of the codec). There is no `omg::types::map` or `bounded_map`; the spec admits `std::map` alone. Log: `.agent/REPORT.md`.

### 7.14.3.1 Map Type Traits

**Spec:** §7.14.3.1 Tab.7.10/7.11, p. 24 — "is_bounded [...] bound [...]
key [...] elements."

**Repo:** —

**Tests:** —

**Status:** open — no `omg::types::map`/`bounded_map` and none of the
four traits.

### 7.14.3.2 Bitsets

**Spec:** §7.14.3.2, p. 24-25 — "IDL bitset types shall be mapped to C++
structs that meet the C++ requirements for aggregate initialization. The
only members of these structs are bit fields. [...] shall derive (using
public inheritance) from the struct resulting from mapping the base
type."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (bitset arm)

**Tests:** `crates/lang-rules/tests/cpp_reader_generates.rs::every_kind_table_9_answers_for_gets_a_defaulter`

**Status:** partial — bitsets are emitted as a struct with separate
integer members, packed in the codec, not as C++ bit-fields; base
bit-fields are folded in flat, not through public inheritance. The spec
requires bit-fields as the only members.

### 7.14.3.3 Bitmask Type

**Spec:** §7.14.3.3, p. 25-26 — "struct type containing [...] unscoped
enum named _flags [...] private member named _value [...] default
constructor and a copy constructor [...] !=, &=, and ^= bitwise operators
[...] function call operator that returns _value."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (bitmask arm)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::every_corpus_case_renders_or_names_its_refusal`

**Status:** partial — a bitmask is emitted as an integer alias
(uint8/16/32/64) plus `inline constexpr` bit constants, not as a `struct`
with an unscoped `_flags` enum, a private `_value`, constructors,
`!=`/`&=`/`^=` operators and a call operator. The
`bit_bound`/`underlying_type` traits are missing.

### 7.14.4 8-bit Integer Types (Tab.7.13)

**Spec:** §7.14.4 Tab.7.13, p. 26 — "int8 -> int8_t; uint8 -> uint8_t."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`cpp_primitive`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::the_tag_keeps_apart_what_the_cpp_spelling_collapses`

**Status:** partial — `octet`/`uint8` fall onto the same `std::uint8_t`
(tag test). `int8 -> std::int8_t` has no assertion of its own.

### 7.14.5 Explicitly-Named Integer Types (Tab.7.14)

**Spec:** §7.14.5 Tab.7.14, p. 26-27 — "int16/uint16/int32/uint32/int64/uint64
-> int16_t/uint16_t/int32_t/uint32_t/int64_t/uint64_t."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`cpp_primitive`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::the_tag_keeps_apart_what_the_cpp_spelling_collapses`

**Status:** partial — the explicit widths are mapped in `cpp_primitive`,
but no assertion names `int16`/`uint16`/`int32`/`uint32`/`int64`/`uint64`
individually.

---

## §7.15 Anonymous Types

### 7.15 Anonymous Types

**Spec:** §7.15, p. 27 — "No impact to the C++ language mapping."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no mapping impact.

---

## §7.16 User-Defined Annotations

### 7.16 User-Defined Annotations

**Spec:** §7.16, p. 27 — "User-defined annotations are not propagated to
the generated C++ code."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the C++ column carries no user
annotations into its output; the clause imposes no emission requirement.

---

## §7.17 Standardized Annotations

### 7.17.1 General Purpose: @optional


**Spec:** §7.17.1 Tab.7.15, p. 27 — "IDL elements annotated with
@optional of type T shall map to std::optional<T> in implementations of
this specification supporting C++17 and above."

**Repo:** —

**Tests:** —

**Status:** `open` — no mapping writes `std::optional<T>`: the default mapping (`psm`) writes `dds::core::optional<T>` (DDS-PSM-Cxx), the option `native` a pointer `T *` (DDS-XTypes 1.3 §7.5.1.2.4.2), the option `classic` `zerodds::classic::Optional<T>`. The form of IDL4 to C++ 1.0 §7.17.1 is missing as a named option. Evidence of the forms that exist: `crates/lang-rules/src/adapter/cpp_psm.rs` (`dds::core::optional<T>`), `crates/lang-rules/src/adapter/cpp.rs` (`T *`), `crates/lang-rules/src/adapter/cpp_classic.rs` (`zerodds::classic::Optional<T>`); tests `crates/lang-rules/src/adapter/cpp.rs::tests::an_optional_member_is_a_plain_pointer_and_not_a_second_field`, `crates/cpp/tests/generated_psm_roundtrip.cpp`.

### 7.17.1 General Purpose: @value, @position, @id, @autoid, @extensibility

**Spec:** §7.17.1 Tab.7.15, p. 27-28 — `@value` provides the enumerator
value; `@position` affects the bitmask; `@id`, `@autoid`,
`@extensibility`, `@final`, `@mutable`, `@appendable` have no impact.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::an_enumeration_takes_an_underlying_type_from_its_bit_bound`

**Status:** partial — `@value` reaches the enumerator value (the adapter
writes every value out) but is not asserted directly; `@position` hangs
on the bitmask, which is itself partial (§7.14.3.3); `@id`, `@autoid`,
`@extensibility`, `@final`, `@mutable` and `@appendable` are, as the spec
intends, not carried into the C++.

### 7.17.2 Data Modeling: @key, @must_understand, @default_literal

**Spec:** §7.17.2 Tab.7.16, p. 28 — `@key`/`@must_understand` have no
impact; "The C++ element declared as result of the mappings [...] shall
be initialized to the element indicated by [@default_literal]."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`member_default`)

**Tests:** `tools/idlc/tests/cpp_headers_together.rs::two_generated_headers_share_a_translation_unit_and_their_structs_start_at_their_defaults`

**Status:** partial — `@key` and `@must_understand` have no impact, as
the spec says. `@default_literal` is not evaluated: the member default
takes the **first** enumerator, not the one marked `@default_literal`.

### 7.17.3 Units and Ranges: @default, @range, @min, @max, @unit

**Spec:** §7.17.3 Tab.7.17, p. 28-29 — `@range` -> `omg::types::ranged<T,
min, max>` with `std::out_of_range`; `@min`/`@max` -> setters throw
`std::out_of_range`; `@unit` no impact; `@default` initialises.

**Repo:** —

**Tests:** —

**Status:** open — no `omg::types::ranged<T,min,max>`, no setters (the
data members are public fields) and therefore no `std::out_of_range`
check for `@range`/`@min`/`@max`. `@default` is evaluated in the codec
defaulter (`crates/lang-rules/src/default_cpp.rs`), but not as the C++
member initialiser of the generated struct.

### 7.17.4 Data Implementation: @bit_bound

**Spec:** §7.17.4 Tab.7.18, p. 29-30 — "int8_t, for bit bound values
between 1 and 8; int16_t [...] int32_t [...] int64_t."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`enum_underlying`,
`cpp_holder`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::an_enumeration_takes_an_underlying_type_from_its_bit_bound`

**Status:** partial — for enums the underlying type is asserted; for
bitmasks the adapter follows the same width rule, but the bitmask
mapping is itself partial (§7.14.3.3).

### 7.17.4 Data Implementation: @external

**Spec:** §7.17.4 Tab.7.18, p. 29-30 — "mapped to std::shared_ptr<T> or
to the equivalent omg::types::ref_type<T> [...] Structures containing
members annotated with @external shall make a deep copy of the external
member in their copy constructor."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`held_externally`,
`shared_ptr` arms)

**Tests:** `crates/lang-rules/tests/external_is_an_indirection.rs::a_recursive_external_member_is_a_shared_pointer_and_not_a_value` — `crates/lang-rules/tests/external_is_an_indirection.rs:118: generated.contains("std::shared_ptr<::m::Node> next;"),`

**Status:** partial — `@external T` is held as `std::shared_ptr<T>`
(asserted). The required deep copy in the copy constructor is missing:
the compiler-generated copy constructor copies the `shared_ptr` shallowly,
so the object is shared.

### 7.17.4 Data Implementation: @nested

**Spec:** §7.17.4 Tab.7.18, p. 29-30 — "No impact on the language
mapping."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no mapping impact.

### 7.17.5 Code Generation: @verbatim

**Spec:** §7.17.5 Tab.7.19, p. 30 — "Copies verbatim text to the
indicated output position when the indicated language is '*', 'c++',
'cpp', 'cc', or 'cxx'."

**Repo:** `crates/lang-rules/src/verbatim.rs`,
`crates/lang-rules/src/adapter/cpp.rs` (`weave_verbatim`)

**Tests:** `crates/lang-rules/tests/verbatim_end_to_end.rs::cpp_answers_to_its_alias_spellings_case_insensitively` — `crates/lang-rules/tests/verbatim_end_to_end.rs:150: generated.contains("// vb-alias"),`

**Status:** partial — all six placements (BEGIN_FILE,
BEFORE/BEGIN/END/AFTER_DECLARATION, END_FILE) and the languages `c++`,
`cpp`, `cxx` as well as `*` are live (asserted). `cc` — explicitly named
in Tab.7.19 — is missing from `verbatim::aliases::CPP`
(`&["c++", "cpp", "cxx"]`).

### 7.17.6 Interfaces: @service, @oneway, @ami

**Spec:** §7.17.6 Tab.7.20, p. 30 — "Options are 'CORBA', 'DDS', '*'.
Impact is platform-specific."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_asynchronous_operations_of_rule_seven`

**Status:** partial — `@oneway` (no reply topic, `future<void>` async) and
`@service` (service name) are handled through the DDS-RPC binding; `@ami`
has no path of its own. Tab.7.20 explicitly calls the impact
platform-specific.

---

## §8 IDL to C++ Language Mapping Annotations

### 8.1 @cpp_mapping Annotation

**Spec:** §8.1, p. 31 — "enum StructMapping { STRUCT_WITH_PUBLIC_MEMBERS,
CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS }; StructMapping struct_mapping
default STRUCT_WITH_PUBLIC_MEMBERS."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the annotation is not emitted as an IDL
declaration; the default behaviour of the C++ column (structs with public
members) matches `STRUCT_WITH_PUBLIC_MEMBERS`.

### 8.1.1 CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS

**Spec:** §8.1.1, p. 31-32 — "mapping the annotated IDL struct to C++
class where all members are only accessible via public accessor methods,
and may only be modified through public modifier methods."

**Repo:** —

**Tests:** —

**Status:** open — no path emits the annotated struct as a class with
private members, accessors/modifiers and an initialising constructor.
`@cpp_mapping` is not read by `crates/lang-rules/src/adapter/cpp.rs`.

---

## Annex A: Platform-Specific Mappings

### A.1 CORBA-Specific Mappings

**Spec:** Annex A.1, p. 33-66 — `CORBA::traits<>` (`value_type`,
`in_type`, `out_type`, `inout_type`, tables A.1-A.15),
`CORBA::servant_traits<>`, TypeCode, ORB, Object, Any, Value Types,
Abstract Interfaces, Server Side Mapping, DSI, PortableServer.

**Repo:** —

**Tests:** —

**Status:** open — no `CORBA::traits<>`/`CORBA::servant_traits<>` in the
C++ column and no `omg::types`. `crates/corba-codegen/src/traits.rs` is a
Rust CORBA generator, not this C++ binding.

### A.2 DDS-Specific Mappings

**Spec:** Annex A.2, p. 66 — "DDS requires no additional platform-specific
language mappings."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec requires no additional
DDS-specific C++ mapping.

---

## Annex B: Building Block Traceability Matrix

**Spec:** Annex B (non-normative), p. 67.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Annex C: Compatibility Rules for C++98 and C++03

**Spec:** Annex C (normative), p. 69-77 — alternative mappings for C++98
and C++03.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the C++ column targets C++17/20
(§7.1.3); the C++98/03 path is optional and is not implemented.

---

## Annex D: IDL4 Mapping Rules for Classic C++ Mapping Specifications

**Spec:** Annex D, p. 79-89 — mappings for the classic C++ and C++11
specs.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the tree uses its own binding, not the
classic mapping specs.

---

## Audit Status

8 done / 26 partial / 14 open / 19 n/a (informative) / 0 n/a (rejected).

Open items: `idl4-cpp-1.0.open.en.md`.

Test run (not executed in this pass, since only documentation changed):
the corpus gate is
`cargo test -p zerodds-lang-rules --test cpp_corpus_compiles -- --include-ignored`;
it is `#[ignore]` because it invokes a C++ compiler once per corpus case.
