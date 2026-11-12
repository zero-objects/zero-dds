# IDL4 to C++ Language Mapping 1.0 — Spec Coverage

**Spec:** [OMG IDL4-CPP 1.0](https://www.omg.org/spec/IDL4-CPP/1.0/PDF) (101 pages, OMG formal/25-03-03)

Item-by-item audit against the spec; every requirement with its spec
quote, repo path, test path and status (`done` / `partial` / `open` /
`n/a`).

**Context:** the mapping of this specification is `--cpp-mapping idl4` of
`zerodds-idlc`:

- `crates/lang-rules/src/emit/idl4_cpp/` — the emitter (`generate`), with
  `Options` for the platform (`--cpp-platform`: none, `dds`, `corba`), the
  language standard (`--cpp-standard`: `c++11`, `c++03`) and the CORBA
  module (`--cpp-corba-module`).
- `runtime/cpp/include/omg/types.hpp` — the namespace `omg::types`
  (Chapter 7 and Annex C).
- `runtime/cpp/include/omg/corba.hpp` and `omg/corba_module.hpp` — the
  CORBA-specific mappings of Annex A.1; `corba_module.hpp` is generated
  from `orb.idl` and `runtime/corba/CORBA_PIDL.idl`.

The other C++ columns (`--cpp-mapping psm`, `native`, `classic`) follow
DDS-PSM-Cxx and the classic mapping and are not the subject of this
document; Annex D concerns `classic`.

Every test generates the header of an IDL file, compiles it with a check
program under `-std=c++11` and `-std=c++17` (Annex C: `-std=c++03`) with
warnings as errors, and runs it.

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

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`generate`); `tools/idlc/src/generate.rs` (`--cpp-mapping idl4`)

**Tests:** `tools/idlc/tests/idl4_mappings_take_the_whole_model.rs::idl4cpp_7_3_a_structure_with_an_any_member_is_written_by_the_command_line`

**Status:** `done` — `zerodds-idlc generate --lang cpp --cpp-mapping idl4` writes the mapping of Chapter 7; each clause is an item of its own below, with its test.

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

**Status:** n/a (informative) — abbreviations.

---

## §6 Additional Information

### 6.1-6.3 Changes, Acknowledgments, Intellectual Property Rights

**Spec:** §6.1-6.3, p. 3.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — changes to other specs, acknowledgements, rights notices.

---

## §7.1 General

### 7.1.1 Names

**Spec:** §7.1.1, p. 5 — "IDL member names and type identifiers shall
map to equivalent C++ names and type identifiers with no change."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`names::name`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`

**Status:** `done` — members and types carry their IDL names in C++.

### 7.1.2 Reserved Names

**Spec:** §7.1.2, p. 5-6 — "The use of any of these names for a
user-defined IDL type or interface (assuming it is also a legal IDL name)
shall result in the mapped name having an underscore ('_') prepended."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`names::RESERVED`, `names::name`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_1_2_a_reserved_name_gets_an_underscore`

**Status:** `done` — a name of the list gets the prepended underscore.

### 7.1.3 C++ Language Version Requirements

**Spec:** §7.1.3, p. 6 — "The language mappings defined in this
specification require C++11 [ISO/IEC-14882:2011] as the minimum C++
standard version."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`Standard`); `crates/lang-rules/tests/cpp-idl4/support.rs` (`compile_and_run`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_1_the_basic_types_map_by_tables_7_2_7_3_7_13_and_7_14`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_c_the_cxx03_mapping_follows_annex_c`

**Status:** `done` — every test of the mapping compiles the generated header with `-std=c++11` and `-std=c++17`; the rules of Annex C with `-std=c++03`.

### 7.1.4 IDL Type Traits (Tab.7.1 and Tab.7.4-7.12)

**Spec:** §7.1.4 Tab.7.1, p. 6-7 — "Type traits shall be available under
the omg::types namespace" (`value_type`, `in_type`, `out_type`,
`inout_type`, `_t`/`_v` aliases); plus the specialisations Tab.7.4
(sequences), Tab.7.5 (strings), Tab.7.6 (wstrings), Tab.7.7 (`digits`,
`scale`), Tab.7.8 (`bit_bound`, `underlying_type`), Tab.7.9
(`dimensions`), Tab.7.10/7.11 (`is_bounded`, `bound`, `key`, `elements`)
and Tab.7.12 (`bit_bound`, `underlying_type`).

**Repo:** `runtime/cpp/include/omg/types.hpp` (`value_type`, `in_type`, `out_type`, `inout_type`, `is_bounded`, `bound`, `digits`, `scale`, `bit_bound`, `underlying_type`, `dimensions`, `key`, `elements`, `*_t`, `*_v`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_1_4_the_type_traits_give_value_in_out_and_inout_types`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_4_fixed_is_omg_types_fixed_with_digits_and_scale`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_3_an_enumeration_is_an_enum_class_of_its_bit_bound`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_4_and_7_2_4_6_arrays_and_typedefs_are_aliases`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_14_3_bit_sets_are_bit_fields_and_bit_masks_structs_with_flags`

**Status:** `done`

---

## §7.2 Core Data Types

### 7.2.1 IDL Specification

**Spec:** §7.2.1, p. 7 — "There is no direct mapping of the IDL
Specification itself."

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — "There is no direct mapping of the IDL Specification itself."

### 7.2.2 Modules

**Spec:** §7.2.2, p. 7 — "IDL modules shall be mapped to C++ namespaces
of the same name. [...] IDL declarations not enclosed in any module shall
be mapped into the global scope."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`namespace_of`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_are_constexpr`

**Status:** `done`

### 7.2.3 Constants

**Spec:** §7.2.3, p. 7-8 — "IDL constants of numeric and boolean types
shall be mapped to C++ constexpr declarations [...] IDL constants of
string type shall be mapped to a constexpr declaration of
omg::types::string_view type [...] of wstring type [...] omg::types::wstring_view
[...] The constant value of wide character and wide string constants
shall be preceded by L."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs` (`constant`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_are_constexpr`

**Status:** `done` — `constexpr` for every constant type, `omg::types::string_view`/`wstring_view` for strings.

---

## §7.2.4 Data Types

### 7.2.4.1 Basic Types (Tab.7.2/7.3)

**Spec:** §7.2.4.1.1-§7.2.4.1.6, p. 8-9 — integers (Tab.7.2), floating
point (Tab.7.3), char, wchar, boolean, octet.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs` (`integer_text`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_1_the_basic_types_map_by_tables_7_2_7_3_7_13_and_7_14`

**Status:** `done`

### 7.2.4.1.2 long double -> long double

**Spec:** §7.2.4.1.2, Tab.7.3, p. 8 — "long double | long double".

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_1_the_basic_types_map_by_tables_7_2_7_3_7_13_and_7_14`

**Status:** `done` — `long double` is `long double`.

### 7.2.4.2.1 Sequences -> std::vector<T>

**Spec:** §7.2.4.2.1, p. 9 — "IDL sequences shall be mapped to a C++
std::vector<T>, or to a type named omg::types::sequence<T> [...]."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs`; `runtime/cpp/include/omg/types.hpp` (`sequence`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done`

### 7.2.4.2.1 omg::types::sequence/bounded_sequence

**Spec:** §7.2.4.2.1 Tab.7.4, p. 9 — "implementers shall always define
omg::types::sequence<T> [...] omg::types::bounded_sequence<T,N> [...] is_bounded
[...] bound."

**Repo:** `runtime/cpp/include/omg/types.hpp` (`sequence`, `bounded_sequence`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done`

### 7.2.4.2.2 Strings -> std::string

**Spec:** §7.2.4.2.2, p. 10 — "IDL strings shall be mapped to C++
std::string, or to a type named omg::types::string."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs`; `runtime/cpp/include/omg/types.hpp` (`string`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done`

### 7.2.4.2.2 omg::types::string/bounded_string

**Spec:** §7.2.4.2.2 Tab.7.5, p. 10 — "implementers shall always define
omg::types::string [...] omg::types::bounded_string<N> [...] is_bounded
[...] bound."

**Repo:** `runtime/cpp/include/omg/types.hpp` (`string`, `bounded_string`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done`

### 7.2.4.2.3 Wstrings

**Spec:** §7.2.4.2.3, p. 11 — "IDL wstrings shall be mapped to C++
std::wstrings, or to a type named omg::types::wstring [...]."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs`; `runtime/cpp/include/omg/types.hpp` (`wstring`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done` — `wstring` is `std::wstring`.

### 7.2.4.2.3 omg::types::wstring/bounded_wstring

**Spec:** §7.2.4.2.3 Tab.7.6, p. 11 — "omg::types::wstring [...]
omg::types::bounded_wstring<N> [...] is_bounded [...] bound."

**Repo:** `runtime/cpp/include/omg/types.hpp` (`wstring`, `bounded_wstring`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done`

### 7.2.4.2.4 Fixed Type

**Spec:** §7.2.4.2.4, p. 11-12 — "The IDL fixed type shall map to a C++
class named fixed [...] under the omg::types namespace" (about 30 methods
plus free functions and the `digits`/`scale` traits Tab.7.7).

**Repo:** `runtime/cpp/include/omg/types.hpp` (`fixed`, `fixed_n`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_4_fixed_is_omg_types_fixed_with_digits_and_scale`

**Status:** `done`

### 7.2.4.3.1 Structures


**Spec:** §7.2.4.3.1, p. 13 — "An IDL struct shall be mapped to a C++
struct [...] default constructor [...] copy constructor [...] move
constructor [...] copy assignment [...] move assignment [...] comparison
operators, including at least 'equal to' and 'not equal to' [...]
destructor [...] swap function in the namespace."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`

**Status:** `done`

### 7.2.4.3.1 Default initialization

**Spec:** §7.2.4.3.1, p. 13 — "A default constructor that initializes all
built-in data types to their default value [...] enumerators to their
first value, and the rest of members using their default constructor."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_1_the_basic_types_map_by_tables_7_2_7_3_7_13_and_7_14`

**Status:** `done`

### 7.2.4.3.2 Unions

**Spec:** §7.2.4.3.2, p. 14-15 — class with `_d() const`/`void _d(T)`,
member accessor/modifier per branch, multi-label modifier with a
discriminator parameter, and `_default()`.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_2_a_union_is_a_class_with_d_accessors_and_modifiers`

**Status:** `done`

### 7.2.4.3.3 Enumerations

**Spec:** §7.2.4.3.3, p. 16 — "An IDL enum shall be mapped to a C++
scoped enum class with the same name as the IDL enum type."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_3_an_enumeration_is_an_enum_class_of_its_bit_bound`

**Status:** `done`

### 7.2.4.3.3 Enumeration Type Traits

**Spec:** §7.2.4.3.3 Tab.7.8, p. 17 — "bit_bound [...] underlying_type."

**Repo:** `runtime/cpp/include/omg/types.hpp` (`bit_bound`, `underlying_type`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_3_an_enumeration_is_an_enum_class_of_its_bit_bound`

**Status:** `done`

### 7.2.4.3.4 Constructed Recursive Types

**Spec:** §7.2.4.3.4, p. 16 — "Constructed recursive types are supported
by mapping the involved types directly to C++."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/order.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_4_a_recursive_type_maps_directly`

**Status:** `done`

### 7.2.4.4 Arrays

**Spec:** §7.2.4.4, p. 17 — "An IDL array shall be mapped to a C++
std::array of the mapped element type, or to a type named
omg::types::array [...] Multidimensional arrays shall be constructed
nesting std::array."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_4_and_7_2_4_6_arrays_and_typedefs_are_aliases`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`

**Status:** `done`

### 7.2.4.4 omg::types::array and dimensions

**Spec:** §7.2.4.4 Tab.7.9, p. 17 — "omg::types::array [...] dimensions."

**Repo:** `runtime/cpp/include/omg/types.hpp` (`array`, `dimensions`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_4_and_7_2_4_6_arrays_and_typedefs_are_aliases`

**Status:** `done`

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

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_4_and_7_2_4_6_arrays_and_typedefs_are_aliases`

**Status:** `done`

---

## §7.3 Any

### 7.3 Any -> omg::types::Any

**Spec:** §7.3, p. 18 — "The IDL any type shall be mapped to a C++
omg::types::Any type."

**Repo:** `runtime/cpp/include/omg/types.hpp` (`Any`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_3_any_is_omg_types_any`,
`tools/idlc/tests/idl4_mappings_take_the_whole_model.rs::idl4cpp_7_3_a_structure_with_an_any_member_is_written_by_the_command_line`

**Status:** `done`

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

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`interface`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_4_an_interface_is_a_class_of_pure_virtual_methods`

**Status:** `done`

### 7.4.1 Exceptions

**Spec:** §7.4.1, p. 19-20 — "An IDL exception shall be mapped to a C++
class [...] shall extend the std::exception class [...] copy constructor,
move constructor, assignment operator, move operator, and destructor
[...] what() method [...] explicit constructor with one parameter for
each exception member [...] and a parameter of type const char *."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs` (`exception`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_4_1_an_exception_extends_std_exception`

**Status:** `done`

### 7.4.2 Interface Forward Declaration

**Spec:** §7.4.2, p. 20 — "An IDL interface forward declaration shall be
mapped to a partial forward declaration in C++."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`interface`); `crates/lang-rules/src/emit/idl4_cpp/order.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_4_an_interface_is_a_class_of_pure_virtual_methods`

**Status:** `done`

---

## §7.5 Interfaces – Full

### 7.5 Embedded declarations and static constexpr

**Spec:** §7.5, p. 20 — "adding to mapped class the declaration every
type, exception, or constant declaration in the IDL interface body. [...]
constexpr declarations shall be marked as static."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`embedded_declarations`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_5_an_interface_body_declares_in_its_class`

**Status:** `done`

---

## §7.6 Value Types

### 7.6 Valuetype -> abstract class

**Spec:** §7.6, p. 21 — public/private state members -> public/protected
pure-virtual accessor and modifier methods; factory operations ->
`<ValueTypeName>_factory`; inheritance via public virtual; supports
interface -> all operations as pure virtual.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`valuetype`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_6_a_value_type_is_an_abstract_class_with_its_factory`

**Status:** `done`

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

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`generate`); `crates/ccm/src/transform.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_components.rs::idl4cpp_7_9_and_7_12_a_component_is_its_equivalent_interface_in_cpp`,
`crates/lang-rules/tests/cpp_idl4_components.rs::idl4cpp_7_10_a_home_is_its_explicit_implicit_and_equivalent_interfaces`

**Status:** `done`

### 7.11 CCM-Specific

**Spec:** §7.11, p. 22 — refers to Annex A.1.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — reference to Annex A.1.

### 7.12/7.13 Components – Ports/Connectors and Template Modules

**Spec:** §7.12-7.13, p. 22 — "mapped to intermediate IDL, as specified
in [OMG-IDL4]."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`generate`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_components.rs::idl4cpp_7_12_a_connector_maps_through_its_ports`,
`crates/lang-rules/tests/cpp_idl4_components.rs::idl4cpp_7_13_a_template_module_instance_is_a_namespace_of_its_own`

**Status:** `done`

### 7.12 Ports and connectors as intermediate IDL

**Spec:** §7.12, p. 22 — ports and connectors are "mapped to intermediate IDL, as specified in [OMG-IDL4]".

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`generate`); `crates/ccm/src/transform.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_components.rs::idl4cpp_7_9_and_7_12_a_component_is_its_equivalent_interface_in_cpp`,
`crates/lang-rules/tests/cpp_idl4_components.rs::idl4cpp_7_12_a_connector_maps_through_its_ports`

**Status:** `done`

---

## §7.14 Extended Data Types

### 7.14.1 Structures with Single Inheritance

**Spec:** §7.14.1, p. 23 — "shall inherit from the mapped base struct
using public inheritance. [...] swap method [...] which shall ensure that
all inherited members are swapped."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`

**Status:** `done`

### 7.14.1 Public inheritance and swap of the inherited members

**Spec:** §7.14.1, p. 23 — "shall inherit from the mapped base struct using public inheritance. [...] swap method [...] which shall ensure that all inherited members are swapped."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`

**Status:** `done`

### 7.14.2 Union Discriminators

**Spec:** §7.14.2, p. 23 — "adds the wchar, and octet IDL types to the set
of valid types for a discriminator. [...] int8 and uint8 are valid union
discriminators."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_2_a_union_is_a_class_with_d_accessors_and_modifiers`

**Status:** `done`

### 7.14.3.1 Maps


**Spec:** §7.14.3.1, p. 23-24 — "IDL maps shall be mapped to a C++
std::map<Key,T> [...] or to a type named omg::types::map<Key,T> [...]
omg::types::bounded_map<Key, T, N>."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs`; `runtime/cpp/include/omg/types.hpp` (`map`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done`

### 7.14.3.1 Map Type Traits

**Spec:** §7.14.3.1 Tab.7.10/7.11, p. 24 — "is_bounded [...] bound [...]
key [...] elements."

**Repo:** `runtime/cpp/include/omg/types.hpp` (`map`, `bounded_map`, `key`, `elements`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done`

### 7.14.3.2 Bitsets

**Spec:** §7.14.3.2, p. 24-25 — "IDL bitset types shall be mapped to C++
structs that meet the C++ requirements for aggregate initialization. The
only members of these structs are bit fields. [...] shall derive (using
public inheritance) from the struct resulting from mapping the base
type."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_14_3_bit_sets_are_bit_fields_and_bit_masks_structs_with_flags`

**Status:** `done`

### 7.14.3.3 Bitmask Type

**Spec:** §7.14.3.3, p. 25-26 — "struct type containing [...] unscoped
enum named _flags [...] private member named _value [...] default
constructor and a copy constructor [...] !=, &=, and ^= bitwise operators
[...] function call operator that returns _value."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_14_3_bit_sets_are_bit_fields_and_bit_masks_structs_with_flags`

**Status:** `done`

### 7.14.4 8-bit Integer Types (Tab.7.13)

**Spec:** §7.14.4 Tab.7.13, p. 26 — "int8 -> int8_t; uint8 -> uint8_t."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs` (`integer_text`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_1_the_basic_types_map_by_tables_7_2_7_3_7_13_and_7_14`

**Status:** `done`

### 7.14.5 Explicitly-Named Integer Types (Tab.7.14)

**Spec:** §7.14.5 Tab.7.14, p. 26-27 — "int16/uint16/int32/uint32/int64/uint64
-> int16_t/uint16_t/int32_t/uint32_t/int64_t/uint64_t."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs` (`integer_text`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_1_the_basic_types_map_by_tables_7_2_7_3_7_13_and_7_14`

**Status:** `done`

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

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`; `runtime/cpp/include/omg/types.hpp` (`optional`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_17_the_standardized_annotations_shape_the_struct`

**Status:** `done`

### 7.17.1 General Purpose: @value, @position, @id, @autoid, @extensibility

**Spec:** §7.17.1 Tab.7.15, p. 27-28 — `@value` provides the enumerator
value; `@position` affects the bitmask; `@id`, `@autoid`,
`@extensibility`, `@final`, `@mutable`, `@appendable` have no impact.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_3_an_enumeration_is_an_enum_class_of_its_bit_bound`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_14_3_bit_sets_are_bit_fields_and_bit_masks_structs_with_flags`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_17_the_standardized_annotations_shape_the_struct`

**Status:** `done` — `@value` sets the enumerator's value, `@position` the bit of the bitmask; `@id`, `@autoid`, `@extensibility`, `@final`, `@mutable` and `@appendable` do not change the mapping.

### 7.17.2 Data Modeling: @key, @must_understand, @default_literal

**Spec:** §7.17.2 Tab.7.16, p. 28 — `@key`/`@must_understand` have no
impact; "The C++ element declared as result of the mappings [...] shall
be initialized to the element indicated by [@default_literal]."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_17_the_standardized_annotations_shape_the_struct`

**Status:** `done`

### 7.17.3 Units and Ranges: @default, @range, @min, @max, @unit

**Spec:** §7.17.3 Tab.7.17, p. 28-29 — `@range` -> `omg::types::ranged<T,
min, max>` with `std::out_of_range`; `@min`/`@max` -> setters throw
`std::out_of_range`; `@unit` no impact; `@default` initialises.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`; `runtime/cpp/include/omg/types.hpp` (`ranged`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_17_the_standardized_annotations_shape_the_struct`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`

**Status:** `done`

### 7.17.4 Data Implementation: @bit_bound

**Spec:** §7.17.4 Tab.7.18, p. 29-30 — "int8_t, for bit bound values
between 1 and 8; int16_t [...] int32_t [...] int64_t."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_3_an_enumeration_is_an_enum_class_of_its_bit_bound`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_14_3_bit_sets_are_bit_fields_and_bit_masks_structs_with_flags`

**Status:** `done`

### 7.17.4 Data Implementation: @external

**Spec:** §7.17.4 Tab.7.18, p. 29-30 — "mapped to std::shared_ptr<T> or
to the equivalent omg::types::ref_type<T> [...] Structures containing
members annotated with @external shall make a deep copy of the external
member in their copy constructor."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`; `runtime/cpp/include/omg/types.hpp` (`ref_type`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_17_the_standardized_annotations_shape_the_struct`

**Status:** `done`

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

**Repo:** `crates/lang-rules/src/verbatim.rs` (`file_slots`, `declaration_slots`); `crates/lang-rules/src/emit/idl4_cpp/mod.rs`, `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_17_5_verbatim_text_is_copied_for_its_languages`

**Status:** `done` — the six placements of Table 7.19, each at its position, for `*`, `c++`, `cpp`, `cc` and `cxx`.

### 7.17.6 Interfaces: @service, @oneway, @ami

**Spec:** §7.17.6 Tab.7.20, p. 30 — "Options are 'CORBA', 'DDS', '*'.
Impact is platform-specific."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Table 7.20 names no mapping rule for `@service`, `@oneway` and `@ami`: "Impact is platform-specific."

---

## §8 IDL to C++ Language Mapping Annotations

### 8.1 @cpp_mapping Annotation

**Spec:** §8.1, p. 31 — "enum StructMapping { STRUCT_WITH_PUBLIC_MEMBERS,
CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS }; StructMapping struct_mapping
default STRUCT_WITH_PUBLIC_MEMBERS."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_8_1_1_class_with_public_accessors_and_modifiers`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`

**Status:** `done` — `struct_mapping` selects the form; without the annotation `STRUCT_WITH_PUBLIC_MEMBERS` holds.

### 8.1.1 CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS

**Spec:** §8.1.1, p. 31-32 — "mapping the annotated IDL struct to C++
class where all members are only accessible via public accessor methods,
and may only be modified through public modifier methods."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_8_1_1_class_with_public_accessors_and_modifiers`

**Status:** `done`

---

## Annex A: Platform-Specific Mappings

### A.1.1 Traits (Table A.1)

**Spec:** Annex A.1.1, p. 33 — "all IDL defined types have a type trait specialization that provides the following member types": `value_type`, `in_type`, `out_type`, `inout_type` (Table A.1).

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`CORBA::traits`, `detail::common_traits`, `detail::reference_traits`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_1_corba_traits_have_the_members_of_tables_a_1_to_a_14`

**Status:** `done`

### A.1.1.1 Traits for interfaces (Table A.2)

**Spec:** Annex A.1.1.1, p. 33 — `is_local`, `is_abstract`, `ref_type`, `weak_ref_type`.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_1_corba_traits_have_the_members_of_tables_a_1_to_a_14`,
`crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_5_and_a_1_6_object_references_widen_narrow_and_answer_nil`

**Status:** `done`

### A.1.1.2-A.1.1.4 Traits for strings, sequences and arrays (Tables A.3-A.7)

**Spec:** Annex A.1.1.2-A.1.1.4, p. 33-34 — `is_bounded`, `bound`, `element_traits`, `dimensions` and the other members of Tables A.3 to A.7.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`CORBA::traits` für `std::basic_string`, `basic_bounded_string`, `std::vector`, `bounded_sequence`, `std::array`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_1_corba_traits_have_the_members_of_tables_a_1_to_a_14`

**Status:** `done`

### A.1.1.5-A.1.1.6 Traits for valuetypes and valueboxes (Tables A.8, A.9)

**Spec:** Annex A.1.1.5-A.1.1.6, p. 35 — `is_abstract`, `is_truncatable`, `factory_type`, `obv_type`; `boxed_traits`.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_1_corba_traits_have_the_members_of_tables_a_1_to_a_14`,
`crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.1.7-A.1.1.8 Traits for maps and bitmasks (Tables A.10-A.12)

**Spec:** Annex A.1.1.7-A.1.1.8, p. 35-36 — `key_traits`, `value_traits`, `is_bounded`, `bound`; `bit_bound`, `underlying_type`, specialized on the `<Bitmask>Bits` type.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`CORBA::traits` für `std::map`, `bounded_map`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_1_corba_traits_have_the_members_of_tables_a_1_to_a_14`

**Status:** `done`

### A.1.1.9 Parameters (Tables A.13, A.14)

**Spec:** Annex A.1.1.9, p. 36 — the passing modes for `in`, `out`, `inout` and return: basic types, enumerations and references by value, the other types as `const T&` and `T&`.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_by_value`); `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_1_corba_traits_have_the_members_of_tables_a_1_to_a_14`

**Status:** `done`

### A.1.1.10 Helper Methods

**Spec:** Annex A.1.1.10, p. 37 — `CORBA::traits<T>::narrow(...)` and `CORBA::make_reference<T>(Args&&...)`, "which shall deliver std::make_shared semantics"; "Declaring a reference and initializing it with its default constructor shall result in a nil reference."

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`CORBA::make_reference`, `detail::reference_access`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_1_corba_traits_have_the_members_of_tables_a_1_to_a_14`,
`crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_5_and_a_1_6_object_references_widen_narrow_and_answer_nil`

**Status:** `done`

### A.1.1.11 Servant References (Table A.15)

**Spec:** Annex A.1.1.11, p. 37 — `CORBA::servant_traits<T>` with `base_type`, `ref_type`, `weak_ref_type`.

**Repo:** —

**Tests:** —

**Status:** `open` — `CORBA::servant_traits` is only declared in `omg/corba.hpp`; the emitter writes no specialization.

### A.1.2 Exceptions

**Spec:** Annex A.1.2, p. 38-40 — an IDL exception is a class on `CORBA::UserException`; `CORBA::Exception` and `CORBA::SystemException` with `raise()`, `_name()`, `_rep_id()`, `minor()`, `completed()`, as the clause declares them.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`Exception`, `UserException`, `SystemException`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_exception_members`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_2_exceptions_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.2.1 UnknownUserException

**Spec:** Annex A.1.2.1, p. 40 — `CORBA::UnknownUserException` with `exception()`, which returns the exception as an `Any`.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`UnknownUserException`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_2_exceptions_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.2.2 Any Insertion and Extraction for Exceptions

**Spec:** Annex A.1.2.2, p. 40 — insertion and extraction operators for exceptions; `operator<<=(Any&, const Exception&)` inserts the derived type, `operator>>=(const Any&, SystemException&)` extracts any system exception.

**Repo:** `runtime/cpp/include/omg/corba.hpp`; `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_exception_extras`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_2_exceptions_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.2.3 Union Field Method Exceptions

**Spec:** Annex A.1.2.3, p. 41 — the access to a union member that is not selected throws `CORBA::BAD_PARAM`.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_2_exceptions_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.2.4 Bounded String, Wstring, Sequence, and Map Exceptions

**Spec:** Annex A.1.2.4, p. 41 — "the mapped C++ entity shall throw a CORBA::BAD_PARAM system exception when the application detects that specified bounds have been exceeded".

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`detail::bound_exceeded`, `OMG_TYPES_BOUND_EXCEEDED`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_2_exceptions_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.3 TypeCode

**Spec:** Annex A.1.3, p. 41-42 — `CORBA::TypeCode` as the clause's class; for every IDL type `T` a constant `_tc_T`; the other types of the declaration mapped by Chapter 7 into the namespace `CORBA`.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`TypeCode`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`typecode_of`, `corba_type_extras`); `runtime/cpp/include/omg/corba_module.hpp`

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_3_every_type_has_its_tc_constant_and_typecode_is_the_clauses_class`,
`crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_3_the_corba_module_header_is_the_cpp_of_its_idl`

**Status:** `done`

### A.1.3 TypeCode for map, bitset and bitmask

**Spec:** Annex A.1.3, p. 41-42, with A.1.7.2, p. 45 — a `_tc_` constant and the `Any` operators "for each separate IDL type T".

**Repo:** —

**Tests:** —

**Status:** `open` — for `map`, `bitset` and `bitmask` the emitter writes neither `_tc_` nor `Any` operators; `TCKind` of CORBA 3.3 has no kind for them.

### A.1.4 ORB

**Spec:** Annex A.1.4, p. 42 — "This pseudo interface has to be implemented as a regular local interface."

**Repo:** `runtime/cpp/include/omg/corba_module.hpp` (`CORBA::ORB`); `runtime/corba/CORBA_PIDL.idl`

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_3_the_corba_module_header_is_the_cpp_of_its_idl`

**Status:** `done`

### A.1.4 ORB_init

**Spec:** Annex A.1.4, p. 42 — `omg::types::ref_type<ORB> ORB_init(int& argc, char** argv, const omg::types::string& orb_identifier = omg::types::string())` with the evaluation of `-ORBid` and the other `-ORB<suffix>` arguments.

**Repo:** —

**Tests:** —

**Status:** `open` — `ORB_init` and an implementation of `CORBA::ORB` are missing; `Object::_get_orb` throws `INITIALIZE`.

### A.1.5 Object

**Spec:** Annex A.1.5, p. 43 — `CORBA::Object` by the rules for Interfaces – Full in the namespace `CORBA`; "all operation names in interface Object have leading underscores".

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`Object`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_5_and_a_1_6_object_references_widen_narrow_and_answer_nil`

**Status:** `done`

### A.1.5.1-A.1.5.2 Widening Object References

**Spec:** Annex A.1.5.1-A.1.5.2, p. 43 — implicit widening of `B` to `A` and to `Object`; any object reference widens to `omg::types::ref_type<Object>`.

**Repo:** `runtime/cpp/include/omg/types.hpp` (`ref_type`); `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`interface`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_5_and_a_1_6_object_references_widen_narrow_and_answer_nil`

**Status:** `done`

### A.1.5.3 Nil Object Reference

**Spec:** Annex A.1.5.3, p. 43 — nil is `nullptr`; "Any attempt to invoke an operation through a nil object reference shall result in an INV_OBJREF exception."

**Repo:** `runtime/cpp/include/omg/types.hpp` (`ref_type::operator->`, `OMG_TYPES_NIL_REFERENCE`); `runtime/cpp/include/omg/corba.hpp` (`detail::nil_reference`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_5_and_a_1_6_object_references_widen_narrow_and_answer_nil`

**Status:** `done`

### A.1.5.4 Narrowing Object Reference

**Spec:** Annex A.1.5.4, p. 44 — `CORBA::traits<T>::narrow` returns a valid reference when the runtime type supports the interface, nil otherwise; nil for nil; the two examples of the clause.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_5_and_a_1_6_object_references_widen_narrow_and_answer_nil`

**Status:** `done`

### A.1.6 LocalObject

**Spec:** Annex A.1.6, p. 44 — `LocalObject` derives from `Object`; the class of a local interface derives (indirectly) from it and is available as `CORBA::traits<>::base_type`; references are made by `CORBA::make_reference<T>`.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`LocalObject`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_object_base`, `corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_5_and_a_1_6_object_references_widen_narrow_and_answer_nil`

**Status:** `done`

### A.1.7.1-A.1.7.2 Any: Insertion

**Spec:** Annex A.1.7.1-A.1.7.2, p. 45 — `void operator<<=(Any&, T)` for the types passed by value, `operator<<=(Any&, const T&)` and `operator<<=(Any&, T&&)` for arrays, strings, structs, unions, sequences, `any` and exceptions; the TypeCode follows from the C++ type.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`Any`, die `operator<<=`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_type_extras`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_7_any_inserts_and_extracts_every_type`

**Status:** `done` — for `map`, `bitset` and `bitmask` see the item "A.1.3 TypeCode for map, bitset and bitmask".

### A.1.7.3 Any: Extraction

**Spec:** Annex A.1.7.3, p. 46 — `bool operator>>=(const Any&, T&)`; `false` and the variable unchanged for another type; an object reference is extractable as a base object reference, an abstract reference as a base object or base valuetype reference, a valuetype reference as a base valuetype reference.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (die `operator>>=`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_type_extras`, `corba_interface_extras`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_7_any_inserts_and_extracts_every_type`

**Status:** `done`

### A.1.7.4 TypeCode Replacement

**Spec:** Annex A.1.7.4, p. 46-47 — `type()` returns the TypeCode; `type(tc)` replaces it with an equivalent one and throws `BAD_TYPECODE` otherwise; an alias is inserted as its type.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`Any::type`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_7_any_inserts_and_extracts_every_type`

**Status:** `done`

### A.1.8, A.1.8.1 Value types and their data members

**Spec:** Annex A.1.8-A.1.8.1, p. 47-48 — the abstract base class as `traits<>::base_type`, the OBV class as `traits<>::obv_type`; public state members as public, private ones as protected pure virtual accessors and modifiers.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`valuetype`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_valuetype_tail`, `corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.8.2 Constructors, Assignment Operators, and Destructors

**Spec:** Annex A.1.8.2, p. 48-49 — protected constructors and a protected virtual destructor; the OBV class with a constructor that takes an initializer per member, the members of a concrete base valuetype first; defaults: nil, 0, the first enumerator.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`valuetype`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_valuetype_tail`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.8.3-A.1.8.4 Valuetype Operations

**Spec:** Annex A.1.8.3-A.1.8.4, p. 49-50 — operations are public pure virtual functions not declared `const`; `CORBA::traits<>::narrow` for valuetypes; the example `Example`.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`valuetype`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.8.5 ValueBase Default Methods

**Spec:** Annex A.1.8.5, p. 50 — `ValueBase` with `_copy_value()`, which returns nil, protected constructors and destructor, deleted assignments.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`ValueBase`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.8.6 Value boxes

**Spec:** Annex A.1.8.6, p. 51-52 — a concrete class declared `final` on `ValueBase` with `_value` functions, protected constructors and destructor and deleted assignments, made by `CORBA::make_reference`.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`valuebox`); `runtime/cpp/include/omg/corba.hpp` (`detail::reference_access`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.8.7 Abstract Valuetypes

**Spec:** Annex A.1.8.7, p. 52 — as concrete valuetypes, without the OBV trait.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_valuetype_tail`, `corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.8.8 Valuetype Inheritance

**Spec:** Annex A.1.8.8, p. 52-53 — concrete and abstract base valuetypes as `public virtual` bases; supported interfaces are not inherited, except abstract ones; their operations are pure virtual functions of the class.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`valuetype`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`,
`crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_9_abstract_interfaces_derive_from_abstract_base`

**Status:** `done`

### A.1.8.8 Skeleton of a valuetype that supports an interface

**Spec:** Annex A.1.8.8, p. 52-53 — "the IDL compiler generates a skeleton for this value type; this skeleton is available through the CORBA::servant_traits<>::base_type trait".

**Repo:** —

**Tests:** —

**Status:** `open` — neither `PortableServer` nor a skeleton is generated; `runtime/cpp/include/omg/` has no object adapter.

### A.1.8.9.1-A.1.8.9.2 Valuetype Factories

**Spec:** Annex A.1.8.9.1-A.1.8.9.2, p. 53-55 — `ValueFactoryBase` with a private `create_for_unmarshal`; per valuetype with initializers a factory class as `traits<>::factory_type`; for a valuetype without operations and initializers a concrete factory.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`ValueFactoryBase`, `detail::factory_access`); `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`valuetype`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_valuetype_tail`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.8.9.1, A.1.8.9.3 Factories at the ORB

**Spec:** Annex A.1.8.9.1 and A.1.8.9.3, p. 53-55 — `ORB::register_value_factory` returns nil when no factory was registered; on unmarshalling the implementation looks the factory up through `ORB::lookup_value_factory`, `MARSHAL` without one.

**Repo:** —

**Tests:** —

**Status:** `open` — `CORBA::ORB` has the operations as pure virtual functions; an implementation of the ORB is missing.

### A.1.8.10 Custom Marshaling

**Spec:** Annex A.1.8.10, p. 55 — `CORBA::CustomMarshal`, `CORBA::DataOutputStream` and `CORBA::DataInputStream` follow the valuetype rules.

**Repo:** `runtime/cpp/include/omg/corba_module.hpp`

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.9, A.1.9.1 Abstract Interface Base

**Spec:** Annex A.1.9-A.1.9.1, p. 55-56 — the class of an abstract interface does not derive from `CORBA::Object` but from `CORBA::AbstractBase` with `_to_object` and `_to_value` and protected constructors, assignments and destructor.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`AbstractBase`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_object_base`, `corba_interface_members`); `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`valuetype`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_9_abstract_interfaces_derive_from_abstract_base`

**Status:** `done`

### A.1.9.2 Client side mapping for abstract interfaces

**Spec:** Annex A.1.9.2, p. 56 — a protected default and copy constructor and destructor; abstract interfaces always as `public virtual` bases; `Any` operators; implicit widening of derived interfaces and supporting valuetypes.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_interface_members`, `corba_needs_object_base`, `corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_9_abstract_interfaces_derive_from_abstract_base`,
`crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_7_any_inserts_and_extracts_every_type`

**Status:** `done`

### A.1.10.1 Implementing Interfaces

**Spec:** Annex A.1.10.1, p. 56 — an implementation class defines a non-static member function with the mapped name per operation; the mapping is inheritance-based.

**Repo:** —

**Tests:** —

**Status:** `open` — neither `PortableServer` nor a skeleton is generated; `runtime/cpp/include/omg/` has no object adapter.

### A.1.10.2 PortableServer::Servant

**Spec:** Annex A.1.10.2, p. 57 — `PortableServer::Servant` with `_default_POA()`, `_get_interface()`, `_is_a()`, `_non_existent()` and protected constructors, assignments and destructor; `_default_POA` returns the root POA of the default ORB.

**Repo:** —

**Tests:** —

**Status:** `open` — neither `PortableServer` nor a skeleton is generated; `runtime/cpp/include/omg/` has no object adapter.

### A.1.10.3 Servant References

**Spec:** Annex A.1.10.3, p. 58 — `CORBA::servant_traits<Foo>::ref_type` (`CORBA::servant_reference<>`) and `weak_ref_type` (`CORBA::weak_servant_reference<>`); servants are made by `CORBA::make_reference<>`.

**Repo:** —

**Tests:** —

**Status:** `open` — neither `PortableServer` nor a skeleton is generated; `runtime/cpp/include/omg/` has no object adapter.

### A.1.10.4 Servant Argument Passing

**Spec:** Annex A.1.10.4, p. 58 — the POA holds servants as references; `ServantActivator` and `ServantLocator` pass and return `omg::types::ref_type<PortableServer::Servant>`.

**Repo:** —

**Tests:** —

**Status:** `open` — neither `PortableServer` nor a skeleton is generated; `runtime/cpp/include/omg/` has no object adapter.

### A.1.10.5 Skeleton Operations

**Spec:** Annex A.1.10.5, p. 58-59 — every skeleton class has `_this()` with the three purposes of the clause (within an invocation, implicit activation, a servant already activated) and `PortableServer::WrongPolicy`.

**Repo:** —

**Tests:** —

**Status:** `open` — neither `PortableServer` nor a skeleton is generated; `runtime/cpp/include/omg/` has no object adapter.

### A.1.10.6 Inheritance-Based Interface Implementation

**Spec:** Annex A.1.10.6, p. 59-61 — the skeleton class as `CORBA::servant_traits<T>::base_type` with `PortableServer::Servant` as a virtual base; virtual inheritance of the skeletons of base interfaces; an abstract base interface as a `protected virtual` base.

**Repo:** —

**Tests:** —

**Status:** `open` — neither `PortableServer` nor a skeleton is generated; `runtime/cpp/include/omg/` has no object adapter.

### A.1.10.7 Implementing Operations

**Spec:** Annex A.1.10.7, p. 61-64 — the signature of an implementation function is the mapped signature of the operation; delegation-based implementation through `CORBA::servant_traits<T>::tie_type<U>` with `_tied_object()`.

**Repo:** —

**Tests:** —

**Status:** `open` — neither `PortableServer` nor a skeleton is generated; `runtime/cpp/include/omg/` has no object adapter.

### A.1.11.1 Mapping of ServerRequest

**Spec:** Annex A.1.11.1, p. 64 — "The ServerRequest pseudo object maps to a C++ class that follows the local interface mapping."

**Repo:** —

**Tests:** —

**Status:** `open` — neither `PortableServer` nor a skeleton is generated; `runtime/cpp/include/omg/` has no object adapter.

### A.1.11.2 Mapping of PortableServer Dynamic Implementation Routine

**Spec:** Annex A.1.11.2, p. 64-65 — `PortableServer::DynamicImplementation` on `Servant` with `_this()`, `invoke(ref_type<ServerRequest>)` and `_primary_interface(const ObjectId&, ref_type<POA>)`; `_this()` outside an invocation throws `PortableServer::WrongPolicy`.

**Repo:** —

**Tests:** —

**Status:** `open` — neither `PortableServer` nor a skeleton is generated; `runtime/cpp/include/omg/` has no object adapter.

### A.1.12 PortableServer Functions

**Spec:** Annex A.1.12, p. 65 — `ObjectId_to_string`, `ObjectId_to_wstring`, `string_to_ObjectId`, `wstring_to_ObjectId`; `BAD_PARAM` when the conversion would result in illegal characters.

**Repo:** —

**Tests:** —

**Status:** `open` — neither `PortableServer` nor a skeleton is generated; `runtime/cpp/include/omg/` has no object adapter.

### A.1.13.1 Mapping for Cookie

**Spec:** Annex A.1.13.1, p. 65 — `PortableServer::ServantLocator::Cookie` is `void*`; `preinvoke()` takes `Cookie&`, `postinvoke()` `Cookie`.

**Repo:** —

**Tests:** —

**Status:** `open` — neither `PortableServer` nor a skeleton is generated; `runtime/cpp/include/omg/` has no object adapter.

### A.1.13.2 ServantManagers and AdapterActivators

**Spec:** Annex A.1.13.2, p. 66 — servants for `AdapterActivator`, `ServantActivator` and `ServantLocator` are implemented inheritance-based like any other servant.

**Repo:** —

**Tests:** —

**Status:** `open` — neither `PortableServer` nor a skeleton is generated; `runtime/cpp/include/omg/` has no object adapter.

### A.1.13.3 Server Side Mapping for Abstract Interfaces

**Spec:** Annex A.1.13.3, p. 66 — server side code for an abstract interface only when an interface derives from it or a valuetype supports it through an interface; no skeleton of its own.

**Repo:** —

**Tests:** —

**Status:** `open` — neither `PortableServer` nor a skeleton is generated; `runtime/cpp/include/omg/` has no object adapter.

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

**Status:** n/a (informative) — Annex B is "non-normative".

---

## Annex C: Compatibility Rules for C++98 and C++03

### Annex C.2-C.3 Mapping for C++98 and C++03

**Spec:** Annex C.2-C.3 (normative), p. 69-76 — `const` constants and `std::string` (C.2.1.1), `omg::types::int32_t` etc. (Tables C.1-C.3), no move operations, a plain `enum` with `enum_prefix`/`enum_suffix` (C.2.1.2.2.1, C.3.1), C arrays, `typedef`, `T*` for interface parameters (C.2.2), `@optional` and `@external` (Tables C.4, C.5), the bitmask of C.2.4.1.1.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`Standard::Cxx03`); `runtime/cpp/include/omg/types.hpp`; `tools/idlc/src/main.rs` (`--cpp-standard`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_c_the_cxx03_mapping_follows_annex_c`

**Status:** `done`

### Annex C.4 CORBA-Specific Mappings for C++98 and C++03

**Spec:** Annex C.4, p. 76-77 — the CORBA-specific mappings (exceptions, TypeCode, Any) in their form for C++98 and C++03.

**Repo:** —

**Tests:** —

**Status:** `open` — `omg/corba.hpp` requires C++11; the CORBA platform is generated for C++11 and later only.

---

## Annex D: IDL4 Mapping Rules for Classic C++ Mapping Specifications

**Spec:** Annex D, p. 79-89 — mappings for the classic C++ and C++11
specs.

**Repo:** `crates/lang-rules/src/adapter/cpp_classic.rs` (`CppMapping::Classic`), `runtime/cpp/include/zerodds/map.hpp` (`zerodds::Map`)

**Tests:** `tools/idlc/tests/cpp_classic.rs::xtypes_7_5_1_the_classic_types_are_cpp03_and_the_codec_writes_the_native_bytes`

**Status:** partial — ZeroDDS has a classic mapping per [OMG-C++] (`--cpp-mapping classic`, C++03), to which Annex D applies. The map class does not follow D.2.1.3.1 (own item); the other rules of D.2 are not checked one by one against the mapping. D.3 concerns the C++11 mapping [OMG-C++11], which ZeroDDS does not have.

### Annex D.2.1.3.1 Maps in the classic mapping

**Spec:** Annex D.2.1.3.1, p. 80 — a map becomes a C++ class with a default constructor, `operator[]`, `ULong maximum() const`, `ULong length() const`, `void clear()`, `Boolean insert(const K&, const T&, Boolean replace = TRUE)`, `Boolean insert_or_assign(const K&, const T&)` and `Boolean erase(const K&)`.

**Repo:** —

**Tests:** —

**Status:** open — `zerodds::Map` (`runtime/cpp/include/zerodds/map.hpp`) has `get_size`, `get_max_size`, `set_max_size`, `insert` and `erase` with `DDS::ReturnCode_t`, but not `maximum()`, `length()`, `clear()`, `insert_or_assign` and the returns as `Boolean`.

---

## Audit Status

86 done / 1 partial / 20 open / 0 blocked / 15 n/a (informative).

Test run: `cargo test -p zerodds-lang-rules --test cpp_idl4_mapping --test cpp_idl4_corba --test cpp_idl4_components` and `cargo test -p zerodds-idlc` on the Linux test host — 35 cited tests green, 0 failed.

Open items: `idl4-cpp-1.0.open.en.md`.
