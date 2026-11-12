# IDL4 to C++ Language Mapping 1.0 — Spec-Coverage

**Spec:** [OMG IDL4-CPP 1.0](https://www.omg.org/spec/IDL4-CPP/1.0/PDF) (101 Seiten, OMG formal/25-03-03)

Item-für-Item-Audit gegen die Spec; jede Anforderung mit Spec-Zitat,
Repo-Pfad, Test-Pfad und Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** Die Abbildung dieser Spezifikation ist `--cpp-mapping idl4` von
`zerodds-idlc`:

- `crates/lang-rules/src/emit/idl4_cpp/` — der Emitter (`generate`), mit
  `Options` für Plattform (`--cpp-platform`: keine, `dds`, `corba`),
  Sprachstand (`--cpp-standard`: `c++11`, `c++03`) und das CORBA-Modul
  (`--cpp-corba-module`).
- `runtime/cpp/include/omg/types.hpp` — der Namensraum `omg::types`
  (Kapitel 7 und Annex C).
- `runtime/cpp/include/omg/corba.hpp` und `omg/corba_module.hpp` — die
  CORBA-spezifischen Abbildungen von Annex A.1; `corba_module.hpp` ist aus
  `orb.idl` und `runtime/corba/CORBA_PIDL.idl` erzeugt.

Die anderen C++-Spalten (`--cpp-mapping psm`, `native`, `classic`) folgen
DDS-PSM-Cxx bzw. der klassischen Abbildung und sind nicht Gegenstand
dieses Dokuments; Annex D betrifft `classic`.

Jeder Test erzeugt den Header einer IDL-Datei, übersetzt ihn mit einem
Prüfprogramm unter `-std=c++11` und `-std=c++17` (Annex C: `-std=c++03`)
mit Warnungen als Fehler und führt es aus.

---

## §1 Scope

### 1.1 Mapping IDL v4 -> C++

**Spec:** §1, S. 1 — "This specification defines the mapping of OMG
Interface Definition Language v4 to the C++ programming language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Scope-Aussage der Spezifikation ohne
Anforderung an eine Implementierung.

---

## §2 Conformance Criteria

### 2.1 Implementation: IDL -> C++-Quelltext nach Kapitel 7

**Spec:** §2, S. 1 — "A conformant implementation shall transform IDL
input into C++ source code output as specified in Chapter 7."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`generate`); `tools/idlc/src/generate.rs` (`--cpp-mapping idl4`)

**Tests:** `tools/idlc/tests/idl4_mappings_take_the_whole_model.rs::idl4cpp_7_3_a_structure_with_an_any_member_is_written_by_the_command_line`

**Status:** `done` — `zerodds-idlc generate --lang cpp --cpp-mapping idl4` schreibt die Abbildung von Kapitel 7; jede Klausel steht unten als Item mit ihrem Test.

---

## §3 Normative References

### 3.1 [ISO/IEC-14882] und [OMG-*]

**Spec:** §3, S. 1-2 — [ISO/IEC-14882:1998/2003/2011/2017],
[OMG-C++], [OMG-C++11], [OMG-CORBA-COMP], [OMG-CORBA-IFC], [OMG-IDL4].

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Referenzliste; die C++-Version wird in
§7.1.3, die IDL-Version in §3.9 auditiert.

---

## §4 Terms and Definitions

### 4.1 Building Block, C++, Language Mapping

**Spec:** §4, S. 2 — Glossar (Building Block, C++, Language Mapping).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Definitionen ohne Code-Anforderung.

---

## §5 Symbols

### 5.1 Akronyme (Tab.5.1)

**Spec:** §5 Tab.5.1, S. 2-3 — CCM/CORBA/DDS/IDL.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Abkürzungen.

---

## §6 Additional Information

### 6.1-6.3 Änderungen, Acknowledgments, Intellectual Property Rights

**Spec:** §6.1-6.3, S. 3.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Änderungen an anderen Specs, Danksagung, Rechtehinweise.

---

## §7.1 General

### 7.1.1 Names

**Spec:** §7.1.1, S. 5 — "IDL member names and type identifiers shall
map to equivalent C++ names and type identifiers with no change."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`names::name`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`

**Status:** `done` — die Member und Typen tragen im C++ die Namen der IDL.

### 7.1.2 Reserved Names

**Spec:** §7.1.2, S. 5-6 — "The use of any of these names for a
user-defined IDL type or interface (assuming it is also a legal IDL name)
shall result in the mapped name having an underscore ('_') prepended."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`names::RESERVED`, `names::name`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_1_2_a_reserved_name_gets_an_underscore`

**Status:** `done` — ein Name der Liste bekommt den vorangestellten Unterstrich.

### 7.1.3 C++ Language Version Requirements

**Spec:** §7.1.3, S. 6 — "The language mappings defined in this
specification require C++11 [ISO/IEC-14882:2011] as the minimum C++
standard version."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`Standard`); `crates/lang-rules/tests/cpp-idl4/support.rs` (`compile_and_run`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_1_the_basic_types_map_by_tables_7_2_7_3_7_13_and_7_14`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_c_the_cxx03_mapping_follows_annex_c`

**Status:** `done` — jeder Test der Abbildung übersetzt den erzeugten Header mit `-std=c++11` und `-std=c++17`; die Regeln von Annex C mit `-std=c++03`.

### 7.1.4 IDL Type Traits (Tab.7.1 und Tab.7.4-7.12)

**Spec:** §7.1.4 Tab.7.1, S. 6-7 — "Type traits shall be available under
the omg::types namespace" (`value_type`, `in_type`, `out_type`,
`inout_type`, `_t`/`_v`-Aliase); dazu die Spezialisierungen Tab.7.4
(Sequences), Tab.7.5 (Strings), Tab.7.6 (Wstrings), Tab.7.7 (`digits`,
`scale`), Tab.7.8 (`bit_bound`, `underlying_type`), Tab.7.9
(`dimensions`), Tab.7.10/7.11 (`is_bounded`, `bound`, `key`, `elements`)
und Tab.7.12 (`bit_bound`, `underlying_type`).

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

**Spec:** §7.2.1, S. 7 — "There is no direct mapping of the IDL
Specification itself."

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — "There is no direct mapping of the IDL Specification itself."

### 7.2.2 Modules

**Spec:** §7.2.2, S. 7 — "IDL modules shall be mapped to C++ namespaces
of the same name. [...] IDL declarations not enclosed in any module shall
be mapped into the global scope."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`namespace_of`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_are_constexpr`

**Status:** `done`

### 7.2.3 Constants

**Spec:** §7.2.3, S. 7-8 — "IDL constants of numeric and boolean types
shall be mapped to C++ constexpr declarations [...] IDL constants of
string type shall be mapped to a constexpr declaration of
omg::types::string_view type [...] of wstring type [...] omg::types::wstring_view
[...] The constant value of wide character and wide string constants
shall be preceded by L."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs` (`constant`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_are_constexpr`

**Status:** `done` — `constexpr` für jeden Konstantentyp, `omg::types::string_view`/`wstring_view` für Zeichenketten.

---

## §7.2.4 Data Types

### 7.2.4.1 Basic Types (Tab.7.2/7.3)

**Spec:** §7.2.4.1.1-§7.2.4.1.6, S. 8-9 — Integer (Tab.7.2), Floating
Point (Tab.7.3), char, wchar, boolean, octet.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs` (`integer_text`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_1_the_basic_types_map_by_tables_7_2_7_3_7_13_and_7_14`

**Status:** `done`

### 7.2.4.1.2 long double -> long double

**Spec:** §7.2.4.1.2, Tab.7.3, S. 8 — "long double | long double".

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_1_the_basic_types_map_by_tables_7_2_7_3_7_13_and_7_14`

**Status:** `done` — `long double` ist `long double`.

### 7.2.4.2.1 Sequences -> std::vector<T>

**Spec:** §7.2.4.2.1, S. 9 — "IDL sequences shall be mapped to a C++
std::vector<T>, or to a type named omg::types::sequence<T> [...]."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs`; `runtime/cpp/include/omg/types.hpp` (`sequence`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done`

### 7.2.4.2.1 omg::types::sequence/bounded_sequence

**Spec:** §7.2.4.2.1 Tab.7.4, S. 9 — "implementers shall always define
omg::types::sequence<T> [...] omg::types::bounded_sequence<T,N> [...] is_bounded
[...] bound."

**Repo:** `runtime/cpp/include/omg/types.hpp` (`sequence`, `bounded_sequence`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done`

### 7.2.4.2.2 Strings -> std::string

**Spec:** §7.2.4.2.2, S. 10 — "IDL strings shall be mapped to C++
std::string, or to a type named omg::types::string."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs`; `runtime/cpp/include/omg/types.hpp` (`string`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done`

### 7.2.4.2.2 omg::types::string/bounded_string

**Spec:** §7.2.4.2.2 Tab.7.5, S. 10 — "implementers shall always define
omg::types::string [...] omg::types::bounded_string<N> [...] is_bounded
[...] bound."

**Repo:** `runtime/cpp/include/omg/types.hpp` (`string`, `bounded_string`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done`

### 7.2.4.2.3 Wstrings

**Spec:** §7.2.4.2.3, S. 11 — "IDL wstrings shall be mapped to C++
std::wstrings, or to a type named omg::types::wstring [...]."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs`; `runtime/cpp/include/omg/types.hpp` (`wstring`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done` — `wstring` ist `std::wstring`.

### 7.2.4.2.3 omg::types::wstring/bounded_wstring

**Spec:** §7.2.4.2.3 Tab.7.6, S. 11 — "omg::types::wstring [...]
omg::types::bounded_wstring<N> [...] is_bounded [...] bound."

**Repo:** `runtime/cpp/include/omg/types.hpp` (`wstring`, `bounded_wstring`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done`

### 7.2.4.2.4 Fixed Type

**Spec:** §7.2.4.2.4, S. 11-12 — "The IDL fixed type shall map to a C++
class named fixed [...] under the omg::types namespace" (rund 30
Methoden plus freie Funktionen und die `digits`/`scale`-Traits Tab.7.7).

**Repo:** `runtime/cpp/include/omg/types.hpp` (`fixed`, `fixed_n`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_4_fixed_is_omg_types_fixed_with_digits_and_scale`

**Status:** `done`

### 7.2.4.3.1 Structures


**Spec:** §7.2.4.3.1, S. 13 — "An IDL struct shall be mapped to a C++
struct [...] default constructor [...] copy constructor [...] move
constructor [...] copy assignment [...] move assignment [...] comparison
operators, including at least 'equal to' and 'not equal to' [...]
destructor [...] swap function in the namespace."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`

**Status:** `done`

### 7.2.4.3.1 Default-Initialisierung

**Spec:** §7.2.4.3.1, S. 13 — "A default constructor that initializes all
built-in data types to their default value [...] enumerators to their
first value, and the rest of members using their default constructor."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_1_the_basic_types_map_by_tables_7_2_7_3_7_13_and_7_14`

**Status:** `done`

### 7.2.4.3.2 Unions

**Spec:** §7.2.4.3.2, S. 14-15 — Klasse mit `_d() const`/`void _d(T)`,
Member-Accessor/-Modifier je Zweig, Multi-Label-Modifier mit
Discriminator-Parameter und `_default()`.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_2_a_union_is_a_class_with_d_accessors_and_modifiers`

**Status:** `done`

### 7.2.4.3.3 Enumerations

**Spec:** §7.2.4.3.3, S. 16 — "An IDL enum shall be mapped to a C++
scoped enum class with the same name as the IDL enum type."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_3_an_enumeration_is_an_enum_class_of_its_bit_bound`

**Status:** `done`

### 7.2.4.3.3 Enumeration Type Traits

**Spec:** §7.2.4.3.3 Tab.7.8, S. 17 — "bit_bound [...] underlying_type."

**Repo:** `runtime/cpp/include/omg/types.hpp` (`bit_bound`, `underlying_type`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_3_an_enumeration_is_an_enum_class_of_its_bit_bound`

**Status:** `done`

### 7.2.4.3.4 Constructed Recursive Types

**Spec:** §7.2.4.3.4, S. 16 — "Constructed recursive types are supported
by mapping the involved types directly to C++."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/order.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_4_a_recursive_type_maps_directly`

**Status:** `done`

### 7.2.4.4 Arrays

**Spec:** §7.2.4.4, S. 17 — "An IDL array shall be mapped to a C++
std::array of the mapped element type, or to a type named
omg::types::array [...] Multidimensional arrays shall be constructed
nesting std::array."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_4_and_7_2_4_6_arrays_and_typedefs_are_aliases`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`

**Status:** `done`

### 7.2.4.4 omg::types::array und dimensions

**Spec:** §7.2.4.4 Tab.7.9, S. 17 — "omg::types::array [...] dimensions."

**Repo:** `runtime/cpp/include/omg/types.hpp` (`array`, `dimensions`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_4_and_7_2_4_6_arrays_and_typedefs_are_aliases`

**Status:** `done`

### 7.2.4.5 Native Types

**Spec:** §7.2.4.5, S. 17 — "This language mapping specification does not
define any native types, but compliant implementations may provide the
necessary mechanisms."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Spec definiert keine nativen Typen;
der Adapter bildet `native` optional auf `void *` oder einen
Facetten-Namen ab.

### 7.2.4.6 Naming Data Types

**Spec:** §7.2.4.6, S. 17 — "IDL typedefs shall be mapped to type alias
declarations."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_4_and_7_2_4_6_arrays_and_typedefs_are_aliases`

**Status:** `done`

---

## §7.3 Any

### 7.3 Any -> omg::types::Any

**Spec:** §7.3, S. 18 — "The IDL any type shall be mapped to a C++
omg::types::Any type."

**Repo:** `runtime/cpp/include/omg/types.hpp` (`Any`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_3_any_is_omg_types_any`,
`tools/idlc/tests/idl4_mappings_take_the_whole_model.rs::idl4cpp_7_3_a_structure_with_an_any_member_is_written_by_the_command_line`

**Status:** `done`

---

## §7.4 Interfaces – Basic

### 7.4 Interfaces -> C++-Klassen

**Spec:** §7.4, S. 18-19 — "Each IDL interface shall be mapped to a C++
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

**Spec:** §7.4.1, S. 19-20 — "An IDL exception shall be mapped to a C++
class [...] shall extend the std::exception class [...] copy constructor,
move constructor, assignment operator, move operator, and destructor
[...] what() method [...] explicit constructor with one parameter for
each exception member [...] and a parameter of type const char *."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs` (`exception`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_4_1_an_exception_extends_std_exception`

**Status:** `done`

### 7.4.2 Interface Forward Declaration

**Spec:** §7.4.2, S. 20 — "An IDL interface forward declaration shall be
mapped to a partial forward declaration in C++."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`interface`); `crates/lang-rules/src/emit/idl4_cpp/order.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_4_an_interface_is_a_class_of_pure_virtual_methods`

**Status:** `done`

---

## §7.5 Interfaces – Full

### 7.5 Embedded Declarations und static constexpr

**Spec:** §7.5, S. 20 — "adding to mapped class the declaration every
type, exception, or constant declaration in the IDL interface body. [...]
constexpr declarations shall be marked as static."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`embedded_declarations`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_5_an_interface_body_declares_in_its_class`

**Status:** `done`

---

## §7.6 Value Types

### 7.6 Valuetype -> abstrakte Klasse

**Spec:** §7.6, S. 21 — public/private state members -> public/protected
pure-virtual accessor and modifier methods; factory operations ->
`<ValueTypeName>_factory`; inheritance via public virtual; supports
interface -> all operations as pure virtual.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`valuetype`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_6_a_value_type_is_an_abstract_class_with_its_factory`

**Status:** `done`

---

## §7.7-§7.13 CORBA, Components und Template Modules

### 7.7/7.8 CORBA-Specific Interfaces und Value Types

**Spec:** §7.7-7.8, S. 22 — verweist auf Annex A.1.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Klausel selbst definiert kein
Mapping; Annex A.1 wird gesondert auditiert.

### 7.9/7.10 Components – Basic und Homes

**Spec:** §7.9-7.10, S. 22 — "mapped to equivalent IDL, as specified in
[OMG-CORBA-COMP]."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`generate`); `crates/ccm/src/transform.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_components.rs::idl4cpp_7_9_and_7_12_a_component_is_its_equivalent_interface_in_cpp`,
`crates/lang-rules/tests/cpp_idl4_components.rs::idl4cpp_7_10_a_home_is_its_explicit_implicit_and_equivalent_interfaces`

**Status:** `done`

### 7.11 CCM-Specific

**Spec:** §7.11, S. 22 — verweist auf Annex A.1.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Verweis auf Annex A.1.

### 7.12/7.13 Components – Ports/Connectors und Template Modules

**Spec:** §7.12-7.13, S. 22 — "mapped to intermediate IDL, as specified
in [OMG-IDL4]."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`generate`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_components.rs::idl4cpp_7_12_a_connector_maps_through_its_ports`,
`crates/lang-rules/tests/cpp_idl4_components.rs::idl4cpp_7_13_a_template_module_instance_is_a_namespace_of_its_own`

**Status:** `done`

### 7.12 Ports und Connectoren als Intermediate-IDL

**Spec:** §7.12, S. 22 — Ports und Connectoren werden "mapped to intermediate IDL, as specified in [OMG-IDL4]".

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`generate`); `crates/ccm/src/transform.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_components.rs::idl4cpp_7_9_and_7_12_a_component_is_its_equivalent_interface_in_cpp`,
`crates/lang-rules/tests/cpp_idl4_components.rs::idl4cpp_7_12_a_connector_maps_through_its_ports`

**Status:** `done`

---

## §7.14 Extended Data Types

### 7.14.1 Structures with Single Inheritance

**Spec:** §7.14.1, S. 23 — "shall inherit from the mapped base struct
using public inheritance. [...] swap method [...] which shall ensure that
all inherited members are swapped."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`

**Status:** `done`

### 7.14.1 Public Inheritance und swap der geerbten Member

**Spec:** §7.14.1, S. 23 — "shall inherit from the mapped base struct using public inheritance. [...] swap method [...] which shall ensure that all inherited members are swapped."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`

**Status:** `done`

### 7.14.2 Union Discriminators

**Spec:** §7.14.2, S. 23 — "adds the wchar, and octet IDL types to the set
of valid types for a discriminator. [...] int8 and uint8 are valid union
discriminators."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_2_a_union_is_a_class_with_d_accessors_and_modifiers`

**Status:** `done`

### 7.14.3.1 Maps


**Spec:** §7.14.3.1, S. 23-24 — "IDL maps shall be mapped to a C++
std::map<Key,T> [...] or to a type named omg::types::map<Key,T> [...]
omg::types::bounded_map<Key, T, N>."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs`; `runtime/cpp/include/omg/types.hpp` (`map`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done`

### 7.14.3.1 Map Type Traits

**Spec:** §7.14.3.1 Tab.7.10/7.11, S. 24 — "is_bounded [...] bound [...]
key [...] elements."

**Repo:** `runtime/cpp/include/omg/types.hpp` (`map`, `bounded_map`, `key`, `elements`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_2_template_types_are_the_omg_types_and_their_traits`

**Status:** `done`

### 7.14.3.2 Bitsets

**Spec:** §7.14.3.2, S. 24-25 — "IDL bitset types shall be mapped to C++
structs that meet the C++ requirements for aggregate initialization. The
only members of these structs are bit fields. [...] shall derive (using
public inheritance) from the struct resulting from mapping the base
type."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_14_3_bit_sets_are_bit_fields_and_bit_masks_structs_with_flags`

**Status:** `done`

### 7.14.3.3 Bitmask Type

**Spec:** §7.14.3.3, S. 25-26 — "struct type containing [...] unscoped
enum named _flags [...] private member named _value [...] default
constructor and a copy constructor [...] !=, &=, and ^= bitwise operators
[...] function call operator that returns _value."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_14_3_bit_sets_are_bit_fields_and_bit_masks_structs_with_flags`

**Status:** `done`

### 7.14.4 8-bit Integer Types (Tab.7.13)

**Spec:** §7.14.4 Tab.7.13, S. 26 — "int8 -> int8_t; uint8 -> uint8_t."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs` (`integer_text`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_1_the_basic_types_map_by_tables_7_2_7_3_7_13_and_7_14`

**Status:** `done`

### 7.14.5 Explicitly-Named Integer Types (Tab.7.14)

**Spec:** §7.14.5 Tab.7.14, S. 26-27 — "int16/uint16/int32/uint32/int64/uint64
-> int16_t/uint16_t/int32_t/uint32_t/int64_t/uint64_t."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/types.rs` (`integer_text`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_1_the_basic_types_map_by_tables_7_2_7_3_7_13_and_7_14`

**Status:** `done`

---

## §7.15 Anonymous Types

### 7.15 Anonymous Types

**Spec:** §7.15, S. 27 — "No impact to the C++ language mapping."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — kein Mapping-Impact.

---

## §7.16 User-Defined Annotations

### 7.16 User-Defined Annotations

**Spec:** §7.16, S. 27 — "User-defined annotations are not propagated to
the generated C++ code."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die C++-Spalte trägt keine
Nutzer-Annotationen in die Ausgabe; die Klausel stellt keine
Emissionsanforderung.

---

## §7.17 Standardized Annotations

### 7.17.1 General Purpose: @optional


**Spec:** §7.17.1 Tab.7.15, S. 27 — "IDL elements annotated with
@optional of type T shall map to std::optional<T> in implementations of
this specification supporting C++17 and above."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`; `runtime/cpp/include/omg/types.hpp` (`optional`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_17_the_standardized_annotations_shape_the_struct`

**Status:** `done`

### 7.17.1 General Purpose: @value, @position, @id, @autoid, @extensibility

**Spec:** §7.17.1 Tab.7.15, S. 27-28 — `@value` liefert den
Enumeratorwert; `@position` wirkt auf die Bitmask; `@id`, `@autoid`,
`@extensibility`, `@final`, `@mutable`, `@appendable` sind ohne Impact.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_3_an_enumeration_is_an_enum_class_of_its_bit_bound`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_14_3_bit_sets_are_bit_fields_and_bit_masks_structs_with_flags`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_17_the_standardized_annotations_shape_the_struct`

**Status:** `done` — `@value` setzt den Enumeratorwert, `@position` das Bit der Bitmask; `@id`, `@autoid`, `@extensibility`, `@final`, `@mutable` und `@appendable` ändern die Abbildung nicht.

### 7.17.2 Data Modeling: @key, @must_understand, @default_literal

**Spec:** §7.17.2 Tab.7.16, S. 28 — `@key`/`@must_understand` ohne
Impact; "The C++ element declared as result of the mappings [...]
shall be initialized to the element indicated by [@default_literal]."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_17_the_standardized_annotations_shape_the_struct`

**Status:** `done`

### 7.17.3 Units and Ranges: @default, @range, @min, @max, @unit

**Spec:** §7.17.3 Tab.7.17, S. 28-29 — `@range` -> `omg::types::ranged<T,
min, max>` mit `std::out_of_range`; `@min`/`@max` -> Setter werfen
`std::out_of_range`; `@unit` ohne Impact; `@default` initialisiert.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`; `runtime/cpp/include/omg/types.hpp` (`ranged`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_17_the_standardized_annotations_shape_the_struct`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`

**Status:** `done`

### 7.17.4 Data Implementation: @bit_bound

**Spec:** §7.17.4 Tab.7.18, S. 29-30 — "int8_t, for bit bound values
between 1 and 8; int16_t [...] int32_t [...] int64_t."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_3_an_enumeration_is_an_enum_class_of_its_bit_bound`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_14_3_bit_sets_are_bit_fields_and_bit_masks_structs_with_flags`

**Status:** `done`

### 7.17.4 Data Implementation: @external

**Spec:** §7.17.4 Tab.7.18, S. 29-30 — "mapped to std::shared_ptr<T> or
to the equivalent omg::types::ref_type<T> [...] Structures containing
members annotated with @external shall make a deep copy of the external
member in their copy constructor."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`; `runtime/cpp/include/omg/types.hpp` (`ref_type`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_17_the_standardized_annotations_shape_the_struct`

**Status:** `done`

### 7.17.4 Data Implementation: @nested

**Spec:** §7.17.4 Tab.7.18, S. 29-30 — "No impact on the language
mapping."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — kein Mapping-Impact.

### 7.17.5 Code Generation: @verbatim

**Spec:** §7.17.5 Tab.7.19, S. 30 — "Copies verbatim text to the
indicated output position when the indicated language is '*', 'c++',
'cpp', 'cc', or 'cxx'."

**Repo:** `crates/lang-rules/src/verbatim.rs` (`file_slots`, `declaration_slots`); `crates/lang-rules/src/emit/idl4_cpp/mod.rs`, `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_17_5_verbatim_text_is_copied_for_its_languages`

**Status:** `done` — die sechs Platzierungen von Tabelle 7.19 an ihrer Stelle, für `*`, `c++`, `cpp`, `cc` und `cxx`.

### 7.17.6 Interfaces: @service, @oneway, @ami

**Spec:** §7.17.6 Tab.7.20, S. 30 — "Options are 'CORBA', 'DDS', '*'.
Impact is platform-specific."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Tabelle 7.20 nennt für `@service`, `@oneway` und `@ami` keine Abbildungsregel: „Impact is platform-specific.“

---

## §8 IDL to C++ Language Mapping Annotations

### 8.1 @cpp_mapping Annotation

**Spec:** §8.1, S. 31 — "enum StructMapping { STRUCT_WITH_PUBLIC_MEMBERS,
CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS }; StructMapping struct_mapping
default STRUCT_WITH_PUBLIC_MEMBERS."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_8_1_1_class_with_public_accessors_and_modifiers`,
`crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_7_2_4_3_1_a_structure_is_a_struct_with_comparison_and_swap`

**Status:** `done` — `struct_mapping` wählt die Form; ohne die Annotation gilt `STRUCT_WITH_PUBLIC_MEMBERS`.

### 8.1.1 CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS

**Spec:** §8.1.1, S. 31-32 — "mapping the annotated IDL struct to C++
class where all members are only accessible via public accessor methods,
and may only be modified through public modifier methods."

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_8_1_1_class_with_public_accessors_and_modifiers`

**Status:** `done`

---

## Annex A: Platform-Specific Mappings

### A.1.1 Traits (Tab.A.1)

**Spec:** Annex A.1.1, S. 33 — „all IDL defined types have a type trait specialization that provides the following member types“: `value_type`, `in_type`, `out_type`, `inout_type` (Tab.A.1).

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`CORBA::traits`, `detail::common_traits`, `detail::reference_traits`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_1_corba_traits_have_the_members_of_tables_a_1_to_a_14`

**Status:** `done`

### A.1.1.1 Traits für Interfaces (Tab.A.2)

**Spec:** Annex A.1.1.1, S. 33 — `is_local`, `is_abstract`, `ref_type`, `weak_ref_type`.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_1_corba_traits_have_the_members_of_tables_a_1_to_a_14`,
`crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_5_and_a_1_6_object_references_widen_narrow_and_answer_nil`

**Status:** `done`

### A.1.1.2-A.1.1.4 Traits für Strings, Sequenzen und Arrays (Tab.A.3-A.7)

**Spec:** Annex A.1.1.2-A.1.1.4, S. 33-34 — `is_bounded`, `bound`, `element_traits`, `dimensions` und die übrigen Member der Tabellen A.3 bis A.7.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`CORBA::traits` für `std::basic_string`, `basic_bounded_string`, `std::vector`, `bounded_sequence`, `std::array`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_1_corba_traits_have_the_members_of_tables_a_1_to_a_14`

**Status:** `done`

### A.1.1.5-A.1.1.6 Traits für Valuetypes und Valueboxes (Tab.A.8, A.9)

**Spec:** Annex A.1.1.5-A.1.1.6, S. 35 — `is_abstract`, `is_truncatable`, `factory_type`, `obv_type`; `boxed_traits`.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_1_corba_traits_have_the_members_of_tables_a_1_to_a_14`,
`crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.1.7-A.1.1.8 Traits für Maps und Bitmasks (Tab.A.10-A.12)

**Spec:** Annex A.1.1.7-A.1.1.8, S. 35-36 — `key_traits`, `value_traits`, `is_bounded`, `bound`; `bit_bound`, `underlying_type`, spezialisiert auf den `<Bitmask>Bits`-Typ.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`CORBA::traits` für `std::map`, `bounded_map`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_1_corba_traits_have_the_members_of_tables_a_1_to_a_14`

**Status:** `done`

### A.1.1.9 Parameters (Tab.A.13, A.14)

**Spec:** Annex A.1.1.9, S. 36 — die Übergabearten für `in`, `out`, `inout` und Rückgabe: Basistypen, Enumerationen und Referenzen als Wert, die übrigen Typen als `const T&` bzw. `T&`.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_by_value`); `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_1_corba_traits_have_the_members_of_tables_a_1_to_a_14`

**Status:** `done`

### A.1.1.10 Helper Methods

**Spec:** Annex A.1.1.10, S. 37 — `CORBA::traits<T>::narrow(...)` und `CORBA::make_reference<T>(Args&&...)`, „which shall deliver std::make_shared semantics“; „Declaring a reference and initializing it with its default constructor shall result in a nil reference.“

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`CORBA::make_reference`, `detail::reference_access`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_1_corba_traits_have_the_members_of_tables_a_1_to_a_14`,
`crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_5_and_a_1_6_object_references_widen_narrow_and_answer_nil`

**Status:** `done`

### A.1.1.11 Servant References (Tab.A.15)

**Spec:** Annex A.1.1.11, S. 37 — `CORBA::servant_traits<T>` mit `base_type`, `ref_type`, `weak_ref_type`.

**Repo:** —

**Tests:** —

**Status:** `open` — `CORBA::servant_traits` ist in `omg/corba.hpp` nur deklariert; der Emitter schreibt keine Spezialisierung.

### A.1.2 Exceptions

**Spec:** Annex A.1.2, S. 38-40 — eine IDL-Exception ist eine Klasse auf `CORBA::UserException`; `CORBA::Exception` und `CORBA::SystemException` mit `raise()`, `_name()`, `_rep_id()`, `minor()`, `completed()`, wie die Klausel sie deklariert.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`Exception`, `UserException`, `SystemException`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_exception_members`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_2_exceptions_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.2.1 UnknownUserException

**Spec:** Annex A.1.2.1, S. 40 — `CORBA::UnknownUserException` mit `exception()`, das die Exception als `Any` liefert.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`UnknownUserException`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_2_exceptions_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.2.2 Any Insertion and Extraction for Exceptions

**Spec:** Annex A.1.2.2, S. 40 — Einfüge- und Ausleseoperatoren für Exceptions; `operator<<=(Any&, const Exception&)` fügt den abgeleiteten Typ ein, `operator>>=(const Any&, SystemException&)` liest jede System-Exception.

**Repo:** `runtime/cpp/include/omg/corba.hpp`; `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_exception_extras`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_2_exceptions_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.2.3 Union Field Method Exceptions

**Spec:** Annex A.1.2.3, S. 41 — der Zugriff auf ein nicht gewähltes Union-Member wirft `CORBA::BAD_PARAM`.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/data.rs`

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_2_exceptions_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.2.4 Bounded String, Wstring, Sequence, and Map Exceptions

**Spec:** Annex A.1.2.4, S. 41 — „the mapped C++ entity shall throw a CORBA::BAD_PARAM system exception when the application detects that specified bounds have been exceeded“.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`detail::bound_exceeded`, `OMG_TYPES_BOUND_EXCEEDED`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_2_exceptions_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.3 TypeCode

**Spec:** Annex A.1.3, S. 41-42 — `CORBA::TypeCode` als Klasse der Klausel; für jeden IDL-Typ `T` eine Konstante `_tc_T`; die übrigen Typen der Deklaration nach Kapitel 7 im Namensraum `CORBA`.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`TypeCode`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`typecode_of`, `corba_type_extras`); `runtime/cpp/include/omg/corba_module.hpp`

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_3_every_type_has_its_tc_constant_and_typecode_is_the_clauses_class`,
`crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_3_the_corba_module_header_is_the_cpp_of_its_idl`

**Status:** `done`

### A.1.3 TypeCode für map, bitset und bitmask

**Spec:** Annex A.1.3, S. 41-42, mit A.1.7.2, S. 45 — eine `_tc_`-Konstante und die `Any`-Operatoren „for each separate IDL type T“.

**Repo:** —

**Tests:** —

**Status:** `open` — für `map`, `bitset` und `bitmask` schreibt der Emitter weder `_tc_` noch `Any`-Operatoren; `TCKind` aus CORBA 3.3 hat für sie keine Art.

### A.1.4 ORB

**Spec:** Annex A.1.4, S. 42 — „This pseudo interface has to be implemented as a regular local interface.“

**Repo:** `runtime/cpp/include/omg/corba_module.hpp` (`CORBA::ORB`); `runtime/corba/CORBA_PIDL.idl`

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_3_the_corba_module_header_is_the_cpp_of_its_idl`

**Status:** `done`

### A.1.4 ORB_init

**Spec:** Annex A.1.4, S. 42 — `omg::types::ref_type<ORB> ORB_init(int& argc, char** argv, const omg::types::string& orb_identifier = omg::types::string())` mit der Auswertung von `-ORBid` und der übrigen `-ORB<suffix>`-Argumente.

**Repo:** —

**Tests:** —

**Status:** `open` — `ORB_init` und eine Implementierung von `CORBA::ORB` fehlen; `Object::_get_orb` wirft `INITIALIZE`.

### A.1.5 Object

**Spec:** Annex A.1.5, S. 43 — `CORBA::Object` nach den Regeln für Interfaces – Full im Namensraum `CORBA`; „all operation names in interface Object have leading underscores“.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`Object`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_5_and_a_1_6_object_references_widen_narrow_and_answer_nil`

**Status:** `done`

### A.1.5.1-A.1.5.2 Widening Object References

**Spec:** Annex A.1.5.1-A.1.5.2, S. 43 — implizites Widening von `B` zu `A` und zu `Object`; jede Objektreferenz weitet sich zu `omg::types::ref_type<Object>`.

**Repo:** `runtime/cpp/include/omg/types.hpp` (`ref_type`); `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`interface`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_5_and_a_1_6_object_references_widen_narrow_and_answer_nil`

**Status:** `done`

### A.1.5.3 Nil Object Reference

**Spec:** Annex A.1.5.3, S. 43 — nil ist `nullptr`; „Any attempt to invoke an operation through a nil object reference shall result in an INV_OBJREF exception.“

**Repo:** `runtime/cpp/include/omg/types.hpp` (`ref_type::operator->`, `OMG_TYPES_NIL_REFERENCE`); `runtime/cpp/include/omg/corba.hpp` (`detail::nil_reference`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_5_and_a_1_6_object_references_widen_narrow_and_answer_nil`

**Status:** `done`

### A.1.5.4 Narrowing Object Reference

**Spec:** Annex A.1.5.4, S. 44 — `CORBA::traits<T>::narrow` liefert eine gültige Referenz, wenn der Laufzeittyp das Interface unterstützt, sonst nil; nil für nil; die beiden Beispiele der Klausel.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_5_and_a_1_6_object_references_widen_narrow_and_answer_nil`

**Status:** `done`

### A.1.6 LocalObject

**Spec:** Annex A.1.6, S. 44 — `LocalObject` ist von `Object` abgeleitet; die Klasse eines lokalen Interface ist (indirekt) davon abgeleitet und als `CORBA::traits<>::base_type` verfügbar; Referenzen entstehen über `CORBA::make_reference<T>`.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`LocalObject`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_object_base`, `corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_5_and_a_1_6_object_references_widen_narrow_and_answer_nil`

**Status:** `done`

### A.1.7.1-A.1.7.2 Any: Insertion

**Spec:** Annex A.1.7.1-A.1.7.2, S. 45 — `void operator<<=(Any&, T)` für die als Wert übergebenen Typen, `operator<<=(Any&, const T&)` und `operator<<=(Any&, T&&)` für Arrays, Strings, Structs, Unions, Sequenzen, `any` und Exceptions; der TypeCode folgt aus dem C++-Typ.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`Any`, die `operator<<=`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_type_extras`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_7_any_inserts_and_extracts_every_type`

**Status:** `done` — für `map`, `bitset` und `bitmask` siehe das Item „A.1.3 TypeCode für map, bitset und bitmask“.

### A.1.7.3 Any: Extraction

**Spec:** Annex A.1.7.3, S. 46 — `bool operator>>=(const Any&, T&)`; bei anderem Typ `false` und die Variable unverändert; eine Objektreferenz ist als Basis-Objektreferenz auslesbar, eine abstrakte Referenz als Basis-Objekt- oder Basis-Valuetype-Referenz, eine Valuetype-Referenz als Basis-Valuetype-Referenz.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (die `operator>>=`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_type_extras`, `corba_interface_extras`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_7_any_inserts_and_extracts_every_type`

**Status:** `done`

### A.1.7.4 TypeCode Replacement

**Spec:** Annex A.1.7.4, S. 46-47 — `type()` liefert den TypeCode; `type(tc)` ersetzt ihn durch einen äquivalenten und wirft sonst `BAD_TYPECODE`; ein Alias wird als sein Typ eingefügt.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`Any::type`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_7_any_inserts_and_extracts_every_type`

**Status:** `done`

### A.1.8, A.1.8.1 Value Types und ihre Datenmember

**Spec:** Annex A.1.8-A.1.8.1, S. 47-48 — die abstrakte Basisklasse als `traits<>::base_type`, die OBV-Klasse als `traits<>::obv_type`; öffentliche Zustandsmember als öffentliche, private als geschützte rein virtuelle Zugriffs- und Änderungsfunktionen.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`valuetype`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_valuetype_tail`, `corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.8.2 Constructors, Assignment Operators, and Destructors

**Spec:** Annex A.1.8.2, S. 48-49 — geschützte Konstruktoren und geschützter virtueller Destruktor; die OBV-Klasse mit einem Konstruktor, der je Member einen Initialwert nimmt, die Member eines konkreten Basis-Valuetype zuerst; Standardwerte: nil, 0, erster Enumerator.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`valuetype`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_valuetype_tail`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.8.3-A.1.8.4 Valuetype Operations

**Spec:** Annex A.1.8.3-A.1.8.4, S. 49-50 — Operationen sind öffentliche rein virtuelle, nicht `const` deklarierte Funktionen; `CORBA::traits<>::narrow` für Valuetypes; das Beispiel `Example`.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`valuetype`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.8.5 ValueBase Default Methods

**Spec:** Annex A.1.8.5, S. 50 — `ValueBase` mit `_copy_value()`, das nil liefert, geschützten Konstruktoren und Destruktor, gelöschten Zuweisungen.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`ValueBase`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.8.6 Value boxes

**Spec:** Annex A.1.8.6, S. 51-52 — eine konkrete, `final` deklarierte Klasse auf `ValueBase` mit `_value`-Funktionen, geschützten Konstruktoren und Destruktor und gelöschten Zuweisungen, erzeugt über `CORBA::make_reference`.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`valuebox`); `runtime/cpp/include/omg/corba.hpp` (`detail::reference_access`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.8.7 Abstract Valuetypes

**Spec:** Annex A.1.8.7, S. 52 — wie konkrete Valuetypes, ohne OBV-Trait.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_valuetype_tail`, `corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.8.8 Valuetype Inheritance

**Spec:** Annex A.1.8.8, S. 52-53 — konkrete und abstrakte Basis-Valuetypes als `public virtual`-Basen; unterstützte Interfaces werden nicht geerbt, außer abstrakte; ihre Operationen sind rein virtuelle Funktionen der Klasse.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`valuetype`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`,
`crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_9_abstract_interfaces_derive_from_abstract_base`

**Status:** `done`

### A.1.8.8 Skeleton eines Valuetype, das ein Interface unterstützt

**Spec:** Annex A.1.8.8, S. 52-53 — „the IDL compiler generates a skeleton for this value type; this skeleton is available through the CORBA::servant_traits<>::base_type trait“.

**Repo:** —

**Tests:** —

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

### A.1.8.9.1-A.1.8.9.2 Valuetype Factories

**Spec:** Annex A.1.8.9.1-A.1.8.9.2, S. 53-55 — `ValueFactoryBase` mit privatem `create_for_unmarshal`; je Valuetype mit Initialisierern eine Fabrikklasse als `traits<>::factory_type`; für ein Valuetype ohne Operationen und Initialisierer eine konkrete Fabrik.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`ValueFactoryBase`, `detail::factory_access`); `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`valuetype`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_valuetype_tail`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.8.9.1, A.1.8.9.3 Fabriken am ORB

**Spec:** Annex A.1.8.9.1 und A.1.8.9.3, S. 53-55 — `ORB::register_value_factory` liefert nil, wenn keine Fabrik registriert war; beim Unmarshalling sucht die Implementierung die Fabrik über `ORB::lookup_value_factory`, ohne Fabrik `MARSHAL`.

**Repo:** —

**Tests:** —

**Status:** `open` — `CORBA::ORB` hat die Operationen als rein virtuelle Funktionen; eine Implementierung des ORB fehlt.

### A.1.8.10 Custom Marshaling

**Spec:** Annex A.1.8.10, S. 55 — `CORBA::CustomMarshal`, `CORBA::DataOutputStream` und `CORBA::DataInputStream` folgen den Valuetype-Regeln.

**Repo:** `runtime/cpp/include/omg/corba_module.hpp`

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_8_value_types_are_the_classes_of_the_clause`

**Status:** `done`

### A.1.9, A.1.9.1 Abstract Interface Base

**Spec:** Annex A.1.9-A.1.9.1, S. 55-56 — die Klasse eines abstrakten Interface ist nicht von `CORBA::Object` abgeleitet, sondern von `CORBA::AbstractBase` mit `_to_object` und `_to_value` und geschützten Konstruktoren, Zuweisungen und Destruktor.

**Repo:** `runtime/cpp/include/omg/corba.hpp` (`AbstractBase`); `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_object_base`, `corba_interface_members`); `crates/lang-rules/src/emit/idl4_cpp/behaviour.rs` (`valuetype`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_9_abstract_interfaces_derive_from_abstract_base`

**Status:** `done`

### A.1.9.2 Client Side Mapping für abstrakte Interfaces

**Spec:** Annex A.1.9.2, S. 56 — geschützter Standard- und Kopierkonstruktor und Destruktor; abstrakte Interfaces immer als `public virtual`-Basen; `Any`-Operatoren; implizites Widening abgeleiteter Interfaces und unterstützender Valuetypes.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/corba.rs` (`corba_interface_members`, `corba_needs_object_base`, `corba_traits`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_9_abstract_interfaces_derive_from_abstract_base`,
`crates/lang-rules/tests/cpp_idl4_corba.rs::idl4cpp_a_1_7_any_inserts_and_extracts_every_type`

**Status:** `done`

### A.1.10.1 Implementing Interfaces

**Spec:** Annex A.1.10.1, S. 56 — eine Implementierungsklasse definiert je Operation eine nicht statische Memberfunktion mit dem abgebildeten Namen; die Abbildung ist vererbungsbasiert.

**Repo:** —

**Tests:** —

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

### A.1.10.2 PortableServer::Servant

**Spec:** Annex A.1.10.2, S. 57 — `PortableServer::Servant` mit `_default_POA()`, `_get_interface()`, `_is_a()`, `_non_existent()` und geschützten Konstruktoren, Zuweisungen und Destruktor; `_default_POA` liefert die Root-POA des Standard-ORB.

**Repo:** —

**Tests:** —

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

### A.1.10.3 Servant References

**Spec:** Annex A.1.10.3, S. 58 — `CORBA::servant_traits<Foo>::ref_type` (`CORBA::servant_reference<>`) und `weak_ref_type` (`CORBA::weak_servant_reference<>`); Servants entstehen über `CORBA::make_reference<>`.

**Repo:** —

**Tests:** —

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

### A.1.10.4 Servant Argument Passing

**Spec:** Annex A.1.10.4, S. 58 — die POA hält Servants als Referenzen; `ServantActivator` und `ServantLocator` übergeben und liefern `omg::types::ref_type<PortableServer::Servant>`.

**Repo:** —

**Tests:** —

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

### A.1.10.5 Skeleton Operations

**Spec:** Annex A.1.10.5, S. 58-59 — jede Skeleton-Klasse hat `_this()` mit den drei Bedeutungen der Klausel (innerhalb eines Aufrufs, implizite Aktivierung, schon aktivierter Servant) und `PortableServer::WrongPolicy`.

**Repo:** —

**Tests:** —

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

### A.1.10.6 Inheritance-Based Interface Implementation

**Spec:** Annex A.1.10.6, S. 59-61 — die Skeleton-Klasse als `CORBA::servant_traits<T>::base_type` mit `PortableServer::Servant` als virtueller Basis; virtuelle Vererbung der Skeletons von Basis-Interfaces; ein abstraktes Basis-Interface als `protected virtual`-Basis.

**Repo:** —

**Tests:** —

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

### A.1.10.7 Implementing Operations

**Spec:** Annex A.1.10.7, S. 61-64 — die Signatur einer Implementierungsfunktion ist die abgebildete Signatur der Operation; delegationsbasierte Implementierung über `CORBA::servant_traits<T>::tie_type<U>` mit `_tied_object()`.

**Repo:** —

**Tests:** —

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

### A.1.11.1 Mapping of ServerRequest

**Spec:** Annex A.1.11.1, S. 64 — „The ServerRequest pseudo object maps to a C++ class that follows the local interface mapping.“

**Repo:** —

**Tests:** —

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

### A.1.11.2 Mapping of PortableServer Dynamic Implementation Routine

**Spec:** Annex A.1.11.2, S. 64-65 — `PortableServer::DynamicImplementation` auf `Servant` mit `_this()`, `invoke(ref_type<ServerRequest>)` und `_primary_interface(const ObjectId&, ref_type<POA>)`; `_this()` außerhalb eines Aufrufs wirft `PortableServer::WrongPolicy`.

**Repo:** —

**Tests:** —

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

### A.1.12 PortableServer Functions

**Spec:** Annex A.1.12, S. 65 — `ObjectId_to_string`, `ObjectId_to_wstring`, `string_to_ObjectId`, `wstring_to_ObjectId`; `BAD_PARAM`, wenn die Umwandlung unzulässige Zeichen ergäbe.

**Repo:** —

**Tests:** —

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

### A.1.13.1 Mapping for Cookie

**Spec:** Annex A.1.13.1, S. 65 — `PortableServer::ServantLocator::Cookie` ist `void*`; `preinvoke()` nimmt `Cookie&`, `postinvoke()` `Cookie`.

**Repo:** —

**Tests:** —

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

### A.1.13.2 ServantManagers and AdapterActivators

**Spec:** Annex A.1.13.2, S. 66 — Servants für `AdapterActivator`, `ServantActivator` und `ServantLocator` werden vererbungsbasiert implementiert wie jeder andere Servant.

**Repo:** —

**Tests:** —

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

### A.1.13.3 Server Side Mapping for Abstract Interfaces

**Spec:** Annex A.1.13.3, S. 66 — serverseitiger Code für ein abstraktes Interface nur, wenn ein Interface davon abgeleitet ist oder ein Valuetype es über ein Interface unterstützt; kein eigenes Skeleton.

**Repo:** —

**Tests:** —

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

### A.2 DDS-Specific Mappings

**Spec:** Annex A.2, S. 66 — "DDS requires no additional platform-specific
language mappings."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Spec fordert keine zusätzliche
DDS-spezifische C++-Abbildung.

---

## Annex B: Building Block Traceability Matrix

**Spec:** Annex B (non-normative), S. 67.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Annex B ist „non-normative“.

---

## Annex C: Compatibility Rules for C++98 and C++03

### Annex C.2-C.3 Abbildung für C++98 und C++03

**Spec:** Annex C.2-C.3 (normativ), S. 69-76 — `const`-Konstanten und `std::string` (C.2.1.1), `omg::types::int32_t` usw. (Tab.C.1-C.3), keine Move-Operationen, einfaches `enum` mit `enum_prefix`/`enum_suffix` (C.2.1.2.2.1, C.3.1), C-Arrays, `typedef`, `T*` für Interface-Parameter (C.2.2), `@optional` und `@external` (Tab.C.4, C.5), die Bitmask von C.2.4.1.1.

**Repo:** `crates/lang-rules/src/emit/idl4_cpp/mod.rs` (`Standard::Cxx03`); `runtime/cpp/include/omg/types.hpp`; `tools/idlc/src/main.rs` (`--cpp-standard`)

**Tests:** `crates/lang-rules/tests/cpp_idl4_mapping.rs::idl4cpp_c_the_cxx03_mapping_follows_annex_c`

**Status:** `done`

### Annex C.4 CORBA-Specific Mappings für C++98 und C++03

**Spec:** Annex C.4, S. 76-77 — die CORBA-spezifischen Abbildungen (Exceptions, TypeCode, Any) in der Form für C++98 und C++03.

**Repo:** —

**Tests:** —

**Status:** `open` — `omg/corba.hpp` setzt C++11 voraus; die CORBA-Plattform wird nur für C++11 und neuer erzeugt.

---

## Annex D: IDL4 Mapping Rules for Classic C++ Mapping Specifications

**Spec:** Annex D, S. 79-89 — Mappings für die klassischen C++- und
C++11-Specs.

**Repo:** `crates/lang-rules/src/adapter/cpp_classic.rs` (`CppMapping::Classic`), `runtime/cpp/include/zerodds/map.hpp` (`zerodds::Map`)

**Tests:** `tools/idlc/tests/cpp_classic.rs::xtypes_7_5_1_the_classic_types_are_cpp03_and_the_codec_writes_the_native_bytes`

**Status:** partial — ZeroDDS hat eine klassische Abbildung nach [OMG-C++] (`--cpp-mapping classic`, C++03), für die Annex D gilt. Die Map-Klasse folgt D.2.1.3.1 nicht (eigenes Item); die übrigen Regeln von D.2 sind nicht einzeln gegen die Abbildung geprüft. D.3 betrifft die C++11-Abbildung [OMG-C++11], die ZeroDDS nicht hat.

### Annex D.2.1.3.1 Maps in der klassischen Abbildung

**Spec:** Annex D.2.1.3.1, S. 80 — eine Map wird eine C++-Klasse mit Default-Konstruktor, `operator[]`, `ULong maximum() const`, `ULong length() const`, `void clear()`, `Boolean insert(const K&, const T&, Boolean replace = TRUE)`, `Boolean insert_or_assign(const K&, const T&)` und `Boolean erase(const K&)`.

**Repo:** —

**Tests:** —

**Status:** open — `zerodds::Map` (`runtime/cpp/include/zerodds/map.hpp`) hat `get_size`, `get_max_size`, `set_max_size`, `insert` und `erase` mit `DDS::ReturnCode_t`, aber nicht `maximum()`, `length()`, `clear()`, `insert_or_assign` und die Rückgaben als `Boolean`.

---

## Audit-Status

86 done / 1 partial / 20 open / 0 blocked / 15 n/a (informative).

Test-Lauf: `cargo test -p zerodds-lang-rules --test cpp_idl4_mapping --test cpp_idl4_corba --test cpp_idl4_components` und `cargo test -p zerodds-idlc` auf dem Linux-Testhost — 35 zitierte Tests grün, 0 failed.

Offene Punkte: `idl4-cpp-1.0.open.md`.
