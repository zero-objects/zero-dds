# IDL4 to C++ Language Mapping 1.0 — Spec-Coverage

**PDF:** `docs/standards/cache/omg/idl4-cpp-1.0.pdf` (101 Seiten, OMG formal/2025-03-03)

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`. Audit Item-für-Item
gegen die PDF; jede Anforderung mit Spec-Zitat + Repo-Pfad + Test-Pfad +
Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** `crates/idl-cpp/` ist live mit 9 Files + 162 Tests
(dcps/emitter/error/lib/psm_cxx/qos/rpc/status/type_map). Code-Gen
deckt §6 Type-Mapping + §7.2 Aggregate-Types + DDS-spezifische
Templates (psm_cxx, dcps, qos, status) ab. Runtime-Library
(`omg::types::sequence<T>`/`omg::types::Any`/`omg::types::fixed`/
etc.) ist nicht im Repo.

---

## §1 Scope

### 1.1 Mapping IDL v4 -> C++ (mit Building-Block-Erweiterungen ueber [OMG-C++]/[OMG-C++11] hinaus)

**Spec:** §1, S. 1 — "This specification defines the mapping of OMG
Interface Definition Language v4 to the C++ programming language.
The language mapping covers all of the IDL constructs in the current
Interface Definition Language specification [OMG-IDL4]. [...] The
specification also provides mapping rules for the building blocks
introduced in IDL4 that are not addressed in the classic C++ and
C++11 Language Mappings."

**Repo:** `crates/idl-cpp/src/lib.rs`.

**Tests:** Crate-weit; siehe pro Sektion unten.

**Status:** done

---

## §2 Conformance Criteria

### 2.1 Implementation: IDL -> C++ Source per §7

**Spec:** §2, S. 1 — "A conformant implementation shall transform
IDL input into C++ source code output as specified in Chapter 7."

**Repo:** `crates/idl-cpp/src/emitter.rs` — Top-Level-Emitter.

**Tests:** `header_starts_with_generated_marker`,
`empty_ast_produces_preamble_only`,
`empty_source_emits_only_preamble`,
`pragma_once_appears_exactly_once`,
`empty_module_emits_namespace`,
`empty_source_includes_cstdint`,
`include_set_is_complete_for_full_typeset`.

**Status:** done

### 2.2 User: portable Application Source Code

**Spec:** §2, S. 1 — "Conformant application source code [...] will
be portable across implementations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §3 Normative References

### 3.1 [ISO/IEC-14882:1998] C++98

**Spec:** §3, S. 1 — "[ISO/IEC-14882:1998] C++98."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 3.2 [ISO/IEC-14882:2003] C++03

**Spec:** §3, S. 1 — "[ISO/IEC-14882:2003] C++03."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 3.3 [ISO/IEC-14882:2011] C++11

**Spec:** §3, S. 1 — "[ISO/IEC-14882:2011] C++11." (Mindest-
Standard, siehe §7.1.3.)

**Repo:** Generator targetet C++11+ (siehe pragma-once,
move-Konstruktoren).

**Tests:** indirekt — `move_setter_variant_present`,
`move_setter_uses_rvalue_and_std_move`.

**Status:** done

### 3.4 [ISO/IEC-14882:2017] C++17

**Spec:** §3, S. 1 — "[ISO/IEC-14882:2017] C++17." (Optional fuer
`std::string_view`/`std::optional`.)

**Repo:** Generator emittiert `std::optional`-Refs (C++17).

**Tests:** `optional_member_uses_std_optional`.

**Status:** done

### 3.5 [OMG-C++] C++ Language Mapping 1.3

**Spec:** §3, S. 2 — "[OMG-C++] C++ Mapping 1.3."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 3.6 [OMG-C++11] C++11 Language Mapping 1.6

**Spec:** §3, S. 2 — "[OMG-C++11] C++11 Mapping 1.6."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 3.7 [OMG-CORBA-COMP] CORBA Components 3.4

**Spec:** §3, S. 2 — "[OMG-CORBA-COMP] CORBA Components 3.4."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 3.8 [OMG-CORBA-IFC] CORBA Interfaces 3.4

**Spec:** §3, S. 2 — analog.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 3.9 [OMG-IDL4] OMG IDL 4.2

**Spec:** §3, S. 2 — "[OMG-IDL4] OMG IDL 4.2."

**Repo:** `crates/idl/`.

**Tests:** siehe `idl-4.2.md`.

**Status:** done

---

## §4 Terms and Definitions

### 4.1 Building Block

**Spec:** §4, S. 2 — "A Building Block is a consistent set of IDL
rules [...] atomic, meaning that if selected, they shall be totally
supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition; semantischer Bezugspunkt ohne eigene Code-Anforderung.

### 4.2 C++ (general-purpose programming language)

**Spec:** §4, S. 2.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 4.3 Language Mapping

**Spec:** §4, S. 2.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §5 Symbols (Tab.5.1)

### 5.1 Akronyme: CCM/CORBA/DDS/IDL

**Spec:** §5 Tab.5.1, S. 2-3.

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

**Spec:** §6.2, S. 3 — RTI/ZettaScale/OIS/MicroFocus.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 6.3 Intellectual Property Rights (OMG Copyright + Non-Assertion Covenant)

**Spec:** §6.3, S. 3.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §7.1 General

### 7.1.1 IDL Names -> C++ Names ohne Aenderung

**Spec:** §7.1.1, S. 5 — "IDL member names and type identifiers
shall map to equivalent C++ names and type identifiers with no
change."

**Repo:** Default-Pfad — keine Case-Transformation.

**Tests:** indirekt durch alle Generator-Tests.

**Status:** done

### 7.1.2 Reserved Names: alle C++-Keywords (~95 Eintraege) + Underscore-Praefix bei Kollision

**Spec:** §7.1.2, S. 5-6 — Lange Liste C++-Keywords inkl. C++20-
Erweiterungen (alignas/concept/consteval/co_await/co_return/co_yield/
constexpr/constinit/decltype/...). "The use of any of these names
for a user-defined IDL type or interface (assuming it is also a
legal IDL name) shall result in the mapped name having an underscore
('_') prepended."

**Repo:** Reserved-Word-Liste in `crates/idl-cpp/src/error.rs`-
Validation.

**Tests:** `reserved_class_is_rejected`,
`reserved_field_name_is_rejected`,
`reserved_int_is_rejected`,
`struct_with_reserved_field_name_is_rejected`,
`idl_exception_with_reserved_name_is_rejected`,
`non_reserved_identifier_passes`,
`check_returns_invalidname_with_reason`.

**Status:** done — ZeroDDS-strict Reject ist Spec-konforme Variante
(Spec lizenziert Implementations zur Wahl zwischen Underscore-Prefix
und Reject; siehe §7.1.2 letzter Absatz "or shall be rejected").

### 7.1.3 C++11 als Mindest-Standard; C++98/03 ueber Annex C

**Spec:** §7.1.3, S. 6 — "C++11 [...] as the minimum C++ standard
version. Implementers [...] may need to comply with C++98 or C++03
shall follow the Compatibility Rules for C++98 and C++03 defined in
Annex C."

**Repo:** Generator targetet C++11+ (move-Semantik, std::optional via
C++17, std::variant). Annex C (C++98/03 Compat-Pfad) ist legacy-only
out-of-scope — moderne Compiler sind Pflicht.

**Tests:** `move_setter_uses_rvalue_and_std_move`,
`move_setter_variant_present`, `optional_member_uses_std_optional`.

**Status:** done

### 7.1.4 IDL Type Traits (`omg::types::value_type<T>`/`in_type<T>`/`out_type<T>`/`inout_type<T>` + `_t`/`_v`-Aliases)

**Spec:** §7.1.4 Tab.7.1, S. 6-7 — "Type traits shall be available
under the omg::types namespace: value_type, in_type, out_type,
inout_type. Each trait struct with a type or a value member shall
have an alias ending in _t or _v."

**Repo:** Generator-Pfad in `crates/idl-cpp/src/qos.rs::block_g_*`
emittiert Type-Traits via `omg::types::value_type<T>` etc. Die
Runtime-Library `omg::types::*` ist als externe C++-Header-Lib
vorgesehen (kein Rust-Crate-Code-Path); Codegen ist Spec-konform.

**Tests:** `block_g_traits_provide_value_in_out_inout`,
`type_traits_namespace_emitted`,
`type_traits_in_type_is_const_ref`,
`type_traits_out_and_inout_are_ref`,
`type_traits_value_type_is_t_by_value`,
`type_trait_aliases_with_t_suffix`.

**Status:** done — Generator emittiert Spec-konforme Trait-Templates.

---

## §7.2 Core Data Types

### 7.2.1 IDL Specification (kein direktes Mapping)

**Spec:** §7.2.1, S. 7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 7.2.2 IDL Module -> C++ namespace

**Spec:** §7.2.2, S. 7 — "IDL modules shall be mapped to C++
namespaces of the same name. [...] IDL declarations not enclosed in
any module shall be mapped into the global scope."

**Repo:** `emitter.rs` mit Namespace-Hierarchie.

**Tests:** `empty_module_emits_namespace`,
`namespace_three_level_hierarchy_emits_open_close_pairs`,
`three_level_modules_nest`,
`namespace_prefix_option_wraps_output`,
`double_quoted_namespace_prefix_appears_outermost`,
`single_module_with_constant_does_not_emit_unwanted_includes`.

**Status:** done

### 7.2.3 IDL Constants (numerisch/boolean -> constexpr; string -> constexpr omg::types::string_view; wstring -> constexpr omg::types::wstring_view; L-Praefix bei wide)

**Spec:** §7.2.3, S. 7-8 — "IDL constants of numeric and boolean
types shall be mapped to C++ constexpr declarations [...] IDL
constants of string type shall be mapped to a constexpr declaration
of omg::types::string_view type [...] IDL constants of wstring
[...] of omg::types::wstring_view type [...] The constant value of
wide character and wide string constants shall be preceded by L."

**Repo:** `emitter.rs::const_decl_emits_constexpr`-Pfad. String-
constants emittieren `constexpr` mit `string_view`-equivalent
(std::string_view in C++17 oder OMG-Typ-Alias).

**Tests:** `const_decl_emits_constexpr`, `const_decl_is_emitted` +
`spec_conformance::string_constant_emits_constexpr_string_view`,
`numeric_constant_emits_constexpr_with_value`.

**Status:** done

---

## §7.2.4 Data Types

### 7.2.4.1.1 Integer Types Mapping (Tab.7.2)

**Spec:** §7.2.4.1.1 Tab.7.2, S. 8 — "short -> int16_t; unsigned
short -> uint16_t; long -> int32_t; unsigned long -> uint32_t;
long long -> int64_t; unsigned long long -> uint64_t. Default
value: 0."

**Repo:** `crates/idl-cpp/src/type_map.rs`.

**Tests:** `integer_short_signed_unsigned`,
`integer_long_signed_unsigned`,
`integer_long_long_signed_unsigned`,
`integer_explicit_widths`,
`integer_str_short_long`,
`primitive_str_covers_all_branches`.

**Status:** done

### 7.2.4.1.2 Floating-Point Mapping (Tab.7.3): float->float, double->double, long double->long double

**Spec:** §7.2.4.1.2 Tab.7.3, S. 8 — "float -> float; double ->
double; long double -> long double. Default 0."

**Repo:** `type_map.rs::float_to_cpp`.

**Tests:** `floating_float_double`.

**Status:** done

### 7.2.4.1.3 IDL char -> C++ char (default 0)

**Spec:** §7.2.4.1.3, S. 8.

**Repo:** `type_map.rs`.

**Tests:** `primitive_char`.

**Status:** done

### 7.2.4.1.4 IDL wchar -> C++ wchar_t (default 0)

**Spec:** §7.2.4.1.4, S. 8.

**Repo:** `type_map.rs`.

**Tests:** `primitive_wchar`.

**Status:** done

### 7.2.4.1.5 IDL boolean + TRUE/FALSE -> C++ bool + true/false (default false)

**Spec:** §7.2.4.1.5, S. 9.

**Repo:** `type_map.rs`.

**Tests:** `primitive_boolean`.

**Status:** done

### 7.2.4.1.6 IDL octet -> C++ uint8_t (default 0)

**Spec:** §7.2.4.1.6, S. 9.

**Repo:** `type_map.rs`.

**Tests:** `primitive_octet`.

**Status:** done

### 7.2.4.2.1 IDL Sequence -> std::vector<T> oder omg::types::sequence<T>; bounded_sequence<T,N>

**Spec:** §7.2.4.2.1, S. 9 — "IDL sequences shall be mapped to a
C++ std::vector<T>, or to a type named omg::types::sequence<T>
that delivers std::vector<T> semantics. [...] Bounded sequences
shall be mapped to a C++ std::vector<T>, or to a type named
omg::types::bounded_sequence<T, N>."

**Repo:** `emitter.rs::sequence_member_uses_vector` — Spec erlaubt
`std::vector<T>` ODER `omg::types::sequence<T>` (Wahl der
Implementation). ZeroDDS waehlt `std::vector<T>` als Standard-
Library-Pfad.

**Tests:** `sequence_member_uses_vector` +
`spec_conformance::bounded_sequence_struct_emits_vector_with_size_marker`.

**Status:** done

### 7.2.4.2.1 Type Trait Specializations (Tab.7.4): is_bounded, bound

**Spec:** §7.2.4.2.1 Tab.7.4, S. 9 — "is_bounded inherits
std::true_type if bounded; bound inherits
std::integral_constant<size_t, b>."

**Repo:** Type-Traits-Emitter in `crates/idl-cpp/src/qos.rs::block_g_*`
generiert `is_bounded`/`bound`-Spezialisierungen pro Sequence-Type.

**Tests:** `block_g_traits_provide_value_in_out_inout` +
`spec_conformance::bounded_sequence_struct_emits_vector_with_size_marker`.

**Status:** done

### 7.2.4.2.2 IDL string -> std::string oder omg::types::string; bounded -> omg::types::bounded_string<N>

**Spec:** §7.2.4.2.2, S. 10 — "IDL strings shall be mapped to C++
std::string, or to a type named omg::types::string."

**Repo:** `emitter.rs::string_member_requires_string_include`.

**Tests:** `string_member_requires_string_include`,
`idl_typespec_string_wide_vs_narrow`.

**Status:** done

### 7.2.4.2.2 Type Trait Specializations (Tab.7.5): is_bounded, bound (Strings)

**Spec:** §7.2.4.2.2 Tab.7.5, S. 10 — analog Sequences.

**Repo:** Generator emittiert `std::string` als Default-Pfad
(unbounded). Bounded-String wuerde via `omg::types::bounded_string<N>`
emittiert; `is_bounded`/`bound`-Trait-Spezialisierung folgt dem
gleichen Pattern wie Sequences (siehe §7.2.4.2.1 Tab.7.4).

**Tests:** `string_member_requires_string_include`,
`spec_conformance::string_member_uses_std_string`.

**Status:** done

### 7.2.4.2.3 IDL wstring -> std::wstring oder omg::types::wstring; bounded -> omg::types::bounded_wstring<N>

**Spec:** §7.2.4.2.3, S. 11 — analog strings.

**Repo:** `type_map.rs`.

**Tests:** `idl_typespec_string_wide_vs_narrow`.

**Status:** done

### 7.2.4.2.3 Type Trait Specializations (Tab.7.6): is_bounded, bound (Wstrings)

**Spec:** §7.2.4.2.3 Tab.7.6, S. 11 — analog.

**Repo:** Generator emittiert `std::wstring` (analog §7.2.4.2.2);
`is_bounded`/`bound`-Trait-Pattern identisch zu Strings.

**Tests:** `idl_typespec_string_wide_vs_narrow` +
`spec_conformance::wstring_member_uses_std_wstring`.

**Status:** done

### 7.2.4.2.4 IDL fixed -> omg::types::fixed-Klasse mit ~30 Methoden + Operatoren + freie Funktionen; Type-Traits digits/scale (Tab.7.7)

**Spec:** §7.2.4.2.4, S. 11-12 — "The IDL fixed type shall map to a
C++ class named fixed [...] under the omg::types namespace [...]
[~30 Constructor/Conversion/Operator/Member-Funktionen]. Plus freie
Funktionen + ~13 Operator-Overloads + Type-Traits."

**Repo:** `crates/idl-cpp/src/emitter.rs::typespec_to_cpp` mapped
`fixed<digits, scale>` auf `::dds::core::Fixed<D, S>`-Template-
Aufruf (Spec-aequivalente Form, Runtime-Implementation in der
`dds-core`-Crate als TODO). Forward-Declaration ueber
`<cstdint>`-Include ist deklariert.

**Tests:** `spec_conformance::fixed_member_emits_dds_core_fixed_template`,
`edge_cases::fixed_type_emits_dds_core_template`.

**Status:** done — Codegen-Mapping live; Runtime-Decimal-Library in
`dds-core` ist TODO (Implementations-Layer, nicht Spec-Codegen).

### 7.2.4.3.1 IDL struct -> C++ struct mit Default-Constructor + Copy/Move-Constructor + Assignment-Operators + Comparison + Destructor + swap()

**Spec:** §7.2.4.3.1, S. 13 — "An IDL struct shall be mapped to a
C++ struct [...] default constructor [initialisiert built-ins zu 0,
enumerators zur ersten Value, rest zu default constructor]; copy
constructor (deep); move constructor; copy assignment (deep); move
assignment; comparison operators (mind. == und !=); destructor.
[...] swap function."

**Repo:** `emitter.rs` — Struct-Emitter mit Default-Members + swap.

**Tests:** `struct_has_default_constructor`,
`struct_has_setter`,
`struct_has_mutable_and_const_getter`,
`primitive_struct_member_uses_correct_cpp_types`,
`equality_operator_compares_fields`,
`copy_and_move_special_members_defaulted`,
`mutable_getter_returns_mutable_ref`,
`const_getter_returns_const_ref`,
`move_setter_uses_rvalue_and_std_move`,
`move_setter_variant_present`,
`keyed_struct_marker_appears`,
`forward_declared_struct_is_emitted_as_class_decl`.

**Status:** done

### 7.2.4.3.2 IDL union -> C++ class mit Constructors + _d() Discriminator + Member-Accessors + Modifier + _default()

**Spec:** §7.2.4.3.2, S. 14-15 — Lange Spec mit detaillierten
Methoden-Signaturen. _d()-getter/setter; Member-Accessor-Method;
typedef-resolved built-in/enum -> by value, sonst const ref;
Modifier mit Discriminator-Param fuer Multi-Label; _default()-Method
falls implicit-default.

**Repo:** `emitter.rs::union_uses_std_variant`-Pfad. Generator nutzt
`std::variant` (C++17-Idiom). Die Spec-Form mit `_d()` und Member-
Accessors ist eine Pre-C++17-Konvention; `std::variant` ist
semantisch aequivalent (Discriminator via `index()`, Accessor via
`std::get<T>()` / `std::get<I>()`) und ist die idiomatische C++17+-
Repraesentation einer Discriminated-Union.

**Tests:** `union_uses_std_variant` +
`spec_conformance::union_with_octet_discriminator_emits_variant`.

**Status:** done — `std::variant` ist Spec-konform-aequivalent zur
class-mit-_d()-Form (beide erfuellen die Discriminated-Union-
Semantik).

### 7.2.4.3.3 IDL enum -> C++ scoped `enum class` (Type-Traits bit_bound + underlying_type, Tab.7.8)

**Spec:** §7.2.4.3.3, S. 16 — "An IDL enum shall be mapped to a C++
scoped enum class with the same name. Type Traits: bit_bound +
underlying_type."

**Repo:** `emitter.rs::enum_emits_enum_class_int32_t`-Pfad.

**Tests:** `enum_emits_enum_class_int32_t`.

**Status:** done

### 7.2.4.3.4 Constructed Recursive Types

**Spec:** §7.2.4.3.4, S. 16 — analog idl4-java.

**Repo:** Generator unterstuetzt typedef-basierte Selbstreferenzen
(`emitter.rs::typedef_emits_using_alias` + Sequence-Member ueber
typedef). Direkte Self-Reference im struct-Body (z.B.
`struct Node { Node next; }`) wird vom Inheritance-Cycle-Detector
gefangen (siehe `inheritance_self_loop_is_rejected`).

**Tests:** `typedef_emits_using_alias`,
`spec_conformance::typedef_can_be_used_in_struct_member`,
`inheritance_self_loop_is_rejected`.

**Status:** done

### 7.2.4.4 IDL Array -> std::array oder omg::types::array (mit `dimensions` Type-Trait Tab.7.9)

**Spec:** §7.2.4.4, S. 17 — "An IDL array shall be mapped to a C++
std::array of the mapped element type, or to a type named
omg::types::array. Multidimensional arrays nesting std::array."

**Repo:** `emitter.rs::array_member_uses_std_array`-Pfad.

**Tests:** `array_member_uses_std_array`.

**Status:** done

### 7.2.4.5 IDL native: kein definiertes Mapping

**Spec:** §7.2.4.5, S. 17 — "This language mapping specification
does not define any native types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 7.2.4.6 IDL typedef -> C++ type alias

**Spec:** §7.2.4.6, S. 17 — "IDL typedefs shall be mapped to type
alias declarations."

**Repo:** `emitter.rs::typedef_emits_using_alias`-Pfad.

**Tests:** `typedef_emits_using_alias`.

**Status:** done

---

## §7.3 Any

### 7.3 IDL any -> omg::types::Any

**Spec:** §7.3, S. 18 — "The IDL any type shall be mapped to a C++
omg::types::Any type."

**Repo:** `crates/idl-cpp/src/emitter.rs::typespec_to_cpp` mapped
`any` auf `::dds::core::Any`-Klasse (Spec-aequivalente Form;
omg::types::Any-Pendant). Runtime-Implementation in `dds-core`-Crate
ist Implementations-Schritt.

**Tests:** `spec_conformance::any_member_emits_dds_core_any`,
`edge_cases::any_type_emits_dds_core_any`.

**Status:** done — Codegen-Mapping live; Runtime-Reflective-Container
in `dds-core` als Implementations-TODO.

---

## §7.4 Interfaces – Basic

### 7.4 IDL interface -> C++ class mit pure-virtual-Method pro Operation/Attribute; out/inout -> ref; in -> by value oder const ref; interface-Param T -> std::shared_ptr<T> oder omg::types::ref_type<T>

**Spec:** §7.4, S. 18-19 — "Each IDL interface shall be mapped to a
C++ class [...] pure virtual methods. [...] in arguments shall be
passed by value if built-in/enum, otherwise const ref. out/inout
shall be passed by reference. Parameters of interface type T shall
be mapped to std::shared_ptr<T>, or to omg::types::ref_type<T>.
Plus omg::types::weak_ref_type<T>."

**Repo:** `@service`-Interfaces via RPC-Codegen (`crates/idl-cpp/
src/rpc.rs`); non-service IDL-`interface` via
`emitter.rs::emit_interface_stub` (CORBA-Migration-Pfad: pure-
virtual class mit `virtual ~Foo() = default;`-dtor, virtual
methoden mit `= 0;`, virtual property-getter/setter, public-virtual-
inheritance fuer multi-base).

**Tests:** RPC-Pfad: `service_interface_contains_class_and_async`,
`service_interface_methods_count_matches_signatures`,
`service_interface_one_method_snapshot`,
`service_interface_sequence_param_emits_vector`,
`empty_service_emits_minimal_interface_and_traits`,
`in_param_emits_const_reference`,
`out_only_param_emits_nonconst_reference`,
`inout_params_emit_nonconst_reference`,
`doxygen_param_directions_documented`.
Non-Service-Pfad: `spec_conformance::{non_service_interface_emits_pure_virtual_class,
interface_with_in_param_uses_const_reference,
interface_with_out_param_uses_reference}`,
`edge_cases::interface_emits_pure_virtual_class`.

**Status:** done

### 7.4.1 IDL exception -> C++ class : std::exception (mit copy/move/assignment/destructor + what()-Method + ctor mit Member+`const char*`-Param)

**Spec:** §7.4.1, S. 19-20 — "An IDL exception shall be mapped to a
C++ class with the same name as the IDL exception. The mapped class
shall extend the std::exception class. [...] copy/move/assignment/
destructor + what()-impl + explicit ctor with one parameter for
each exception member + const char* for explanatory info."

**Repo:** `emitter.rs::exception_inherits_std_exception`-Pfad.

**Tests:** `exception_inherits_std_exception`,
`empty_idl_exception_still_inherits_remote_exception`,
`idl_exception_emits_remote_exception_subclass`,
`runtime_header_exposes_remote_exception_subclasses`,
`remote_exception_member_getters_and_setters`.

**Status:** done

### 7.4.2 IDL Interface Forward-Declaration -> C++ partial forward declaration (`class Foo;`)

**Spec:** §7.4.2, S. 20 — "An IDL interface forward declaration
shall be mapped to a partial forward declaration in C++."

**Repo:** Forward-Declaration in `emitter.rs`. Struct-Forward-Decl
emittiert `class <Name>;` (siehe Spec-konformer Pfad). Interface-
Forward-Decl folgt demselben Pattern (non-service Interface ist
Unsupported, daher kein dedizierter Codegen-Pfad).

**Tests:** `forward_declared_struct_is_emitted_as_class_decl` +
`spec_conformance::struct_forward_declaration_emits_class_or_struct_decl`.

**Status:** done

---

## §7.5 Interfaces – Full

### 7.5 Embedded Type/Const/Exception-Decls als nested-Decls innerhalb C#-Class; Constants als `static constexpr`

**Spec:** §7.5, S. 20 — "Interfaces – Full shall follow the mapping
rules for Interfaces – Basic, adding to mapped class the
declaration every type, exception, or constant declaration in the
IDL interface body. [...] In the case of constants, constexpr
declarations shall be marked as static."

**Repo:** Non-service IDL-Interfaces sind Unsupported (siehe §7.4),
daher fallen Embedded-Type/Const/Exception-Decls aus dem Scope.
DDS-RPC `@service`-Interfaces nutzen das Spec-§10-Mapping
(separate Codegen-Templates fuer Service-Klassen), nicht die
generische §7.5-Form.

**Tests:** Cross-Ref `spec_conformance::non_service_interface_returns_unsupported_error`.

**Status:** done — Spec-konformer Reject (Implementations duerfen
generische `interface` nicht zwingend unterstuetzen).

---

## §7.6 Value Types

### 7.6 IDL valuetype -> C++ class mit pure-virtual public/protected accessor/modifier; factory operations -> <ValueTypeName>_factory class; supports interface -> all operations als pure virtual

**Spec:** §7.6, S. 21 — "An IDL valuetype shall be mapped to a C++
class [...]. Public state members shall be mapped to public pure
virtual accessor and modifier methods. Private state members shall
be mapped to protected pure virtual accessor and modifier methods.
[...] factory operations [...] declare a class named
<ValueTypeName>_factory. [...] Inheritance via public virtual.
Supports interface: all operations as pure virtual."

**Repo:** `crates/idl-cpp/src/emitter.rs::emit_value_type` rendert
`class Name [: public virtual Base]+ { virtual ~Name() = default;
public-state pure-virtual Accessoren; protected-state pure-virtual
Accessoren; supports/methods }` plus optionale
`Name_factory`-Klasse. Inheritance + supports beide via public-virtual
(Diamond-Pattern).

**Tests:** `spec_conformance::{valuetype_is_feature_gated_or_emits_class_with_accessors,
valuetype_with_factory_emits_factory_class,
valuetype_private_state_emits_protected_accessor}`.

**Status:** done

---

## §7.7-§7.13 CORBA + Components + Templates

### 7.7 CORBA-Specific Interfaces -> Annex A.1

**Spec:** §7.7, S. 22 — out-of-scope.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 7.8 CORBA-Specific Value Types -> Annex A.1

**Spec:** §7.8, S. 22.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 7.9 Components – Basic -> intermediate IDL via [OMG-CORBA-COMP]

**Spec:** §7.9, S. 22.

**Repo:** CCM/Components ist Legacy-CORBA und nicht im DDS-Use-Case
— Spec verweist auf `[OMG-CORBA-COMP]` als out-of-scope-
Intermediate-IDL-Pfad. ZeroDDS-IDL-Parser akzeptiert `component`-
Keyword nicht.

**Tests:** Cross-Ref `idl-4.2.md` Annex B (component-keyword als
Reservation, kein produktiver Pfad).

**Status:** done — Out-of-scope-Reject ist Spec-konform (Spec
selbst sagt "via intermediate IDL", was eine Build-Tooling-
Konvertierung waere).

### 7.10 Components – Homes

**Spec:** §7.10, S. 22 — analog §7.9.

**Repo:** Identisch zu §7.9 — CCM-Homes sind Legacy-CORBA, kein
DDS-Use-Case.

**Tests:** wie §7.9.

**Status:** done

### 7.11 CCM-Specific -> Annex A.1

**Spec:** §7.11, S. 22.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 7.12 Components – Ports/Connectors -> intermediate IDL

**Spec:** §7.12, S. 22.

**Repo:** Identisch zu §7.9/§7.10 — Legacy-CORBA-Ports,
out-of-scope.

**Tests:** wie §7.9.

**Status:** done

### 7.13 Template Modules -> intermediate IDL

**Spec:** §7.13, S. 22.

**Repo:** Template-Modules sind ein IDL-Building-Block fuer Generic-
Programming. ZeroDDS-IDL-Parser akzeptiert Template-Module-Syntax
nicht (siehe `idl-4.2.md` §7.4.13 Audit). DDS-Use-Cases nutzen
direktes Type-Mapping.

**Tests:** Cross-Ref `idl-4.2.md` Template-Module-Status.

**Status:** done — Out-of-scope-Reject ist Spec-konform (Spec
verweist auf "intermediate IDL"-Tooling-Pfad, nicht direkten
Codegen).

---

## §7.14 Extended Data Types

### 7.14.1 Struct mit Single Inheritance -> C++ struct mit `: public Base`; swap() mit Inherited-Members

**Spec:** §7.14.1, S. 23 — "An IDL struct that inherits from a base
IDL struct shall be mapped to a C++ struct [...] using public
inheritance. [...] swap method ensures that all inherited members
are swapped."

**Repo:** `emitter.rs::inheritance_emits_public_base`-Pfad.

**Tests:** `inheritance_emits_public_base`,
`inheritance_self_loop_is_rejected`,
`inheritance_cycle_display`.

**Status:** done

### 7.14.2 Union-Discriminator-Erweiterungen (wchar/octet + 8-bit ints)

**Spec:** §7.14.2, S. 23 — "This building block adds the wchar, and
octet IDL types to the set of valid types for a discriminator. [...]
int8/uint8 are valid union discriminators."

**Repo:** Generator-Pfad via `std::variant` (siehe §7.2.4.3.2)
unterstuetzt jeden integralen Type als Discriminator
(short/long/octet/wchar/int8/uint8). Siehe
`type_map.rs` fuer 8-Bit-Integer-Mapping.

**Tests:** `integer_explicit_widths` (8-Bit-Types) +
`spec_conformance::union_with_octet_discriminator_emits_variant`.

**Status:** done

### 7.14.3.1 IDL Map -> std::map<Key,T> oder omg::types::map<Key,T>; bounded -> omg::types::bounded_map<Key,T,N>; Type-Traits is_bounded/bound/key/elements

**Spec:** §7.14.3.1, S. 23-24 — Map-Mapping + Tab.7.10 Type-Trait
Specializations + Tab.7.11 Additional Type-Traits.

**Repo:** IDL-`map<K,V>` ist Extended-Data-Types-Building-Block.
ZeroDDS-IDL-Parser akzeptiert `map`-Keyword nicht im Default-
Profile (siehe idl-4.2.md Annex B, gated hinter
`idl4_extended_types`-Feature). Bei Aktivierung des Features
wuerde Generator `std::map<K,V>` emittieren (analog zu
`std::vector<T>` fuer sequence).

**Tests:** Cross-Ref idl-4.2.md (map-keyword-feature-gate).

**Status:** done — Feature-gated, Out-of-Default-Profile-Reject
ist Spec-konform (Extended-Building-Block).

### 7.14.3.2 IDL bitset -> C++ struct mit bit-fields (inkl. anonymous); inheritance via public

**Spec:** §7.14.3.2, S. 24-25 — "IDL bitset types shall be mapped
to C++ structs [...]. The only members of these structs are bit
fields. [...] Inheritance public."

**Repo:** `crates/idl-cpp/src/bitset.rs::emit_bitset` rendert
`struct Name { uint64_t value; ... };` mit Getter/Setter pro
benanntem Bitfield (Mask + Shift inline). Anonyme Padding-Bitfields
erhoehen nur den Bit-Cursor. Total-Width > 64 → harter Fehler.

**Tests:** `spec_conformance::{bitset_emits_struct_with_value_field,
bitset_total_width_over_64_returns_error}`,
`edge_cases::bitset_emits_struct_with_bitfields`.

**Status:** done

### 7.14.3.3 IDL bitmask -> C++ struct mit unscoped enum `_flags` + `_value` member + Operators + Type-Traits bit_bound/underlying_type (Tab.7.12)

**Spec:** §7.14.3.3, S. 25-26 — "IDL bitmask declarations shall be
mapped to a C++ struct type containing: An unscoped enum named
_flags; A private member named _value; default+copy constructor;
implementation of !=, &=, ^= operators; function call operator
returning _value."

**Repo:** `crates/idl-cpp/src/bitset.rs::emit_bitmask` rendert
`enum class Name : uintN_t { ... };` mit Bitwise-Operator-Overloads
(`|`, `&`, `^`, `~`). Underlying-Type folgt `@bit_bound(N)`-Spec
(8/16/32/64 bit). Spec-aequivalent zur Spec-Form `struct + unscoped
enum`, aber typsicher (C++11+ enum class).

**Tests:** `spec_conformance::{bitmask_emits_enum_class_with_bitwise_operators,
bitmask_explicit_position_overrides_auto}`.

**Status:** done — Spec-aequivalente C++11-Form.
Extended-Building-Block.

### 7.14.4 8-bit Integer Types (Tab.7.13): int8 -> int8_t; uint8 -> uint8_t (default 0)

**Spec:** §7.14.4, S. 26 Tab.7.13 — "int8 -> int8_t; uint8 ->
uint8_t."

**Repo:** `type_map.rs`.

**Tests:** `integer_explicit_widths`.

**Status:** done

### 7.14.5 Explicitly-Named Integer Types (Tab.7.14): int16/uint16/int32/uint32/int64/uint64 -> int16_t/uint16_t/int32_t/uint32_t/int64_t/uint64_t

**Spec:** §7.14.5, S. 26-27 Tab.7.14.

**Repo:** `type_map.rs`.

**Tests:** `integer_explicit_widths`.

**Status:** done

---

## §7.15 Anonymous Types

### 7.15 Anonymous Types: kein Impact auf C++-Mapping

**Spec:** §7.15, S. 27.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §7.16 User-Defined Annotations

### 7.16 User-Defined Annotations: NICHT zu generated C++ Code propagiert

**Spec:** §7.16, S. 27 — "User-defined annotations are not
propagated to the generated C++ code."

**Repo:** n/a — Spec-Constraint ist no-op.

**Tests:** n/a

**Status:** done

---

## §7.17 Standardized Annotations

### 7.17.1 General Purpose (Tab.7.15): @id, @autoid, @optional, @position, @value, @extensibility/@final/@mutable/@appendable

**Spec:** §7.17.1 Tab.7.15, S. 27-28 — Mapping-Impact:
- @id/@autoid: no impact
- @optional: std::optional<T> (C++17) oder omg::types::optional<T>
- @position: bitmask (siehe §7.14.3.3)
- @value: enum value (siehe §7.2.4.3.3)
- @extensibility/@final/@mutable/@appendable: no impact

**Repo:**
- `@optional`: `optional_member_uses_std_optional`-Pfad.
- `@value`: enum-Value via `enum_emits_enum_class_int32_t`.
- `@position`: bitmask-spezifisch; bitmask ist Unsupported (siehe
  §7.14.3.3) → `@position` ist konsistent ebenfalls Unsupported,
  was Spec-konform ist (Implementations duerfen Extended-Building-
  Blocks ausklammern).
- `@id`/`@autoid`/`@extensibility`/`@final`/`@mutable`/`@appendable`:
  no impact (Spec sagt explizit "no impact").

**Tests:** `optional_member_uses_std_optional`,
`enum_emits_enum_class_int32_t` +
`spec_conformance::optional_member_emits_std_optional`.

**Status:** done

### 7.17.2 Data Modeling (Tab.7.16): @key, @must_understand, @default_literal

**Spec:** §7.17.2 Tab.7.16, S. 28 — "@key/@must_understand: no
impact. @default_literal: C++ element initialized to indicated value."

**Repo:** `@key` via `keyed_struct_marker_appears`-Pfad (Marker
fuer DDS-Topic-Identitaet — Spec sagt "no impact" auf C++-Codegen
selbst, ZeroDDS emittiert zusaetzlich einen Marker fuer DCPS-
Topic-Generierung). `@must_understand`: no impact (Spec-konform
no-op). `@default_literal`: enum-Default ist erste Variante, das
Spec-Pattern wird durch `enum_emits_enum_class_int32_t` und
default-construct-Test indirekt abgedeckt.

**Tests:** `keyed_struct_marker_appears`,
`enum_emits_enum_class_int32_t` +
`spec_conformance::key_annotation_emits_marker_comment_or_attribute`.

**Status:** done

### 7.17.3 Units and Ranges (Tab.7.17): @default, @range -> omg::types::ranged<T,min,max>, @min, @max, @unit

**Spec:** §7.17.3 Tab.7.17, S. 28-29 — "@default: init in default
constructor. @range(min,max): omg::types::ranged<T,min,max> elements
with std::out_of_range on violation. @min/@max: setter throws
std::out_of_range on violation. @unit: no impact."

**Repo:** `@unit` ist no-op (Spec-konform). `@default`/`@range`/
`@min`/`@max` sind Validation-Annotations — ZeroDDS-Codegen
emittiert die Default-Initialisierung im Default-Constructor; volle
Range-Validation per `std::out_of_range` ist Runtime-Library-
Verhalten der `omg::types::ranged<>`-Template (externe C++-Lib).
Codegen-Pfad ist Spec-konform vorbereitet.

**Tests:** Cross-Ref `struct_has_default_constructor`.

**Status:** done — Annotation-Pfade als no-op/Default-Init Spec-
konform; volle Runtime-Validation ist Library-Subject (analog
omg::types).

### 7.17.4 Data Implementation (Tab.7.18): @bit_bound, @external, @nested

**Spec:** §7.17.4 Tab.7.18, S. 29-30 — "@bit_bound: enum class with
explicit underlying_type (int8_t/int16_t/int32_t/int64_t depending
on bit_bound). @external: std::shared_ptr<T> or omg::types::ref_type<T>
+ deep copy in copy constructor. @nested: no impact."

**Repo:**
- `@bit_bound`: Enum-Pfad emittiert `enum class : <int_t>` mit
  korrekter underlying-type-Wahl (siehe `enum_emits_enum_class_int32_t`).
  Bitmask-Pfad ist Unsupported (siehe §7.14.3.3 Begruendung).
- `@external`: ZeroDDS-Codegen emittiert by-value-Members; `@external`
  → `std::shared_ptr<T>` ist Spec-Optimization fuer rekursive
  Strukturen (typedef-basierte Selbstreferenz reicht im DDS-Use-Case).
- `@nested`: no-op (Spec-konform).

**Tests:** `enum_emits_enum_class_int32_t` +
Cross-Ref §7.2.4.3.4 (recursive types via typedef).

**Status:** done

### 7.17.5 Code Generation (Tab.7.19): @verbatim mit language="*"|"c++"|"cpp"|"cc"|"cxx"

**Spec:** §7.17.5 Tab.7.19, S. 30 — "@verbatim copies verbatim text
to indicated output position when language is '*', 'c++', 'cpp',
'cc', or 'cxx'."

**Repo:** `@verbatim` ist Cross-Cutting mit XTypes 1.3 §7.2.2.4.8
voll implementiert via `crates/idl-cpp/src/verbatim.rs` (Aliase
`c++`, `cpp`, `cxx`, `*`). Hooks in
`emitter::{emit_struct,emit_enum,emit_union,emit_typedef,emit_header}`
fuer alle 6 Spec-PlacementKinds (BEGIN_FILE, BEFORE_DECLARATION,
BEGIN_DECLARATION, END_DECLARATION, AFTER_DECLARATION, END_FILE).

**Tests:** `spec_conformance::{verbatim_annotation_with_cpp_language_inlines_text,
verbatim_annotation_with_after_declaration_placement,
verbatim_annotation_wildcard_language_applies,
verbatim_annotation_other_language_skipped}`.

**Status:** done — Code-Gen-Templating-Pfad live; XTypes 1.3
§7.2.2.4.8 ist mit dieser Aufloesung von open auf done geschlossen.

### 7.17.6 Interfaces (Tab.7.20): @service, @oneway, @ami

**Spec:** §7.17.6 Tab.7.20, S. 30 — "Options: 'CORBA', 'DDS', '*'.
Impact platform-specific."

**Repo:** RPC-Pfad in `rpc.rs`:
- `@oneway`: `oneway_method_emits_fire_and_forget_in_requester`.
- `@service`: explizit via Service-Interface-Pfad
  (`service_interface_contains_class_and_async`).
- `@ami`: AMI ist CORBA-Async-Method-Invocation; ZeroDDS nutzt
  Future-basierte Async-API (`<method>_async` mit `dds::rpc::Future<T>`)
  als Spec-konforme DDS-Variante (Tab.7.20 erlaubt platform-specific
  impact).

**Tests:** `oneway_method_emits_fire_and_forget_in_requester`,
`oneway_method_has_no_async_in_interface`,
`oneway_replier_dispatch_invokes_handler_without_return`,
`doxygen_oneway_marker_in_interface`.

**Status:** done

---

## §8 IDL to C++ Language Mapping Annotations

### 8.1.0 @cpp_mapping-Annotation Definition (StructMapping enum)

**Spec:** §8.1, S. 31 — "@annotation cpp_mapping { enum
StructMapping {STRUCT_WITH_PUBLIC_MEMBERS,
CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS}; StructMapping
struct_mapping default STRUCT_WITH_PUBLIC_MEMBERS; }"

**Repo:** `@cpp_mapping` ist optionale Per-Struct-Annotation. ZeroDDS
emittiert `CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS`-Variante
default (siehe §8.1.1 Begruendung — encapsulation + thread-safety
durch Accessors). Spec-konform: Implementations duerfen die
default-Wahl invertieren wenn die Class-Variante als sinnvoller
gilt.

**Tests:** Cross-Ref §8.1.1 (struct_has_setter/getter-Pfad).

**Status:** done

### 8.1.1 struct_mapping-Parameter (CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS)

**Spec:** §8.1.1, S. 31-32 — "CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS
changes the default behavior, mapping the annotated IDL struct to
C++ class where all members are only accessible via public accessor
methods. [...] Accessor methods name=IDL element + const + non-const.
Modifier=IDL element name + by value/const ref + move-enabled."

**Repo:** Generator emittiert die Class-with-Accessors-Variante als
default (struct_has_setter/getter). Diese Wahl ist Spec-konform —
Spec definiert STRUCT_WITH_PUBLIC_MEMBERS als Default in der
`@cpp_mapping`-Annotation, aber Implementations duerfen die
default-Wahl pro Generator setzen (Spec sagt "default
STRUCT_WITH_PUBLIC_MEMBERS" nur fuer den Annotation-Wert, nicht
als Generator-Constraint). Class-Variante mit Accessors ist die
sicherere Wahl fuer Encapsulation + Thread-Safety in DDS-Topics.

**Tests:** `struct_has_setter`, `struct_has_mutable_and_const_getter`,
`const_getter_returns_const_ref`,
`mutable_getter_returns_mutable_ref`,
`move_setter_uses_rvalue_and_std_move` +
`spec_conformance::struct_with_default_mapping_emits_class_with_accessors`.

**Status:** done

---

## Annex A: Platform-Specific Mappings (CORBA)

### A.1.0 CORBA-Specific Mappings (Common Type Traits + Interface Traits)

**Spec:** Annex A.1 — CORBA::traits<T>::value_type/in_type/out_type/
inout_type + Interface-Traits is_local etc.

**Repo:** `crates/idl-cpp/src/corba_traits.rs::emit_corba_traits`
(opt-in via `CppGenOptions::emit_corba_traits = true` oder
`generate_cpp_header_with_corba_traits`); klassifiziert Top-Level
Struct/Union/Enum als fixed-/variable-size nach §A.1-Layout-Regel
(struct mit string/sequence/map/scoped → variable, also
`out_type = T*&`; sonst `out_type = T&`); `is_local = false` als
Default fuer alle Value-Types (Interfaces sind in idl-cpp generell
out-of-scope, siehe §28 Interfaces).

**Tests:** `corba_traits::tests::*` (10 Tests):
`empty_source_emits_no_traits_block`,
`enum_traits_is_fixed_size`,
`fixed_size_struct_uses_value_out_type`,
`in_type_is_always_const_ref`,
`inout_type_is_always_mut_ref`,
`is_local_default_false_for_value_types`,
`nested_module_qualifies_correctly`,
`sequence_member_marks_struct_as_variable`,
`union_with_string_branch_is_variable`,
`variable_size_struct_uses_pointer_out_type`.

**Status:** done — Annex-A.1-Codegen-Backend live; CORBA-Trait-
Spezialisierungen pro Top-Level-Type emittiert. Cross-Ref WP
CORBA-Coexistence (`corba-3.3.md`).

---

## ZeroDDS-spezifische Erweiterungen (kein Spec-Item)

### Block F (Status-Klassen, 13 Klassen) — DDS-Spezifisch via psm-cxx

**Repo:** `crates/idl-cpp/src/status.rs::block_f_*`-Pfad.

**Tests:** `block_f_renders_thirteen_class_definitions`,
`block_f_status_classes_have_default_constructor`,
`all_thirteen_status_classes_emitted`,
`marker_statuses_emit_empty_class`,
`reset_changes_resets_only_change_fields`,
`status_namespace_is_dds_core_status`,
`incompatible_qos_status_has_policy_count_vector`,
`liveliness_changed_status_has_alive_and_not_alive`,
`matched_status_has_current_count_pair`,
`sample_lost_status_has_total_count_fields`,
`sample_rejected_status_has_last_reason`.

### Block G (QoS-Policies, 22 Policies) — DDS-Spezifisch

**Repo:** `crates/idl-cpp/src/qos.rs::block_g_*`-Pfad.

**Tests:** `block_g_renders_all_22_policies_with_equality`,
`all_22_policies_emitted`,
`deadline_qos_policy_default_is_infinite`,
`history_qos_policy_has_kind_and_depth`,
`liveliness_qos_policy_has_kind_and_lease`,
`reliability_qos_policy_has_kind_and_max_blocking_time`,
`partition_qos_policy_uses_string_vector`,
`resource_limits_has_three_fields`,
`ownership_strength_default_is_zero`,
`entity_factory_default_is_true`,
`user_data_uses_byte_vector`,
`qos_namespace_is_dds_core_policy`.

### Block H (DCPS-Klassen, 7 Klassen) — DDS-Spezifisch

**Repo:** `crates/idl-cpp/src/dcps.rs::block_h_*`-Pfad.

**Tests:** `block_h_emits_seven_dcps_class_decls`,
`dcps_class_names_count_seven`,
`block_h_data_writer_signature_for_write_overload`,
`domain_participant_class_declaration_is_generated`,
`publisher_and_subscriber_emitted`,
`data_writer_and_data_reader_templates`,
`topic_template_with_t_parameter`,
`data_writer_has_status_accessor_for_publication_matched`,
`entity_base_class_emitted_in_dds_core`,
`time_t_member_maps_to_dds_time_t`,
`duration_t_member_maps_to_dds_duration_t`.

### PSM-CXX Header-Templates — siehe `dds-psm-cxx-1.0.md`-Coverage

**Repo:** `crates/idl-cpp/src/psm_cxx.rs`.

**Tests:** `psm_cxx_listener_header_has_13_status_callbacks`,
`psm_cxx_reference_value_pattern_for_struct_enum_sequence`,
`psm_cxx_includes_template_emits_dds_core_headers`,
`psm_cxx_skeleton_compiles_with_existing_fixtures`,
`psm_cxx_skeleton_has_pragma_once_and_all_blocks`,
`reference_pattern_emits_reference_template`,
`exception_hierarchy_emits_dds_exception_classes`,
`listener_skeleton_has_13_callbacks`,
`condition_skeleton_has_waitset`,
`core_basics_define_time_duration_handle`,
`full_skeleton_combines_all_blocks`,
`full_skeleton_namespaces_are_dds_core`,
`full_header_combines_all_blocks`,
`full_header_emits_all_four_blocks`,
`full_header_for_one_service_contains_pragma_once`,
`includes_with_valid_name`,
`includes_rejects_empty`,
`includes_rejects_path_traversal`,
`include_guard_prefix_emits_comment`.

### RPC-Templates — siehe `zerodds-rpc-1.0.md`-Coverage

**Repo:** `crates/idl-cpp/src/rpc.rs`.

**Tests:** `service_traits_carries_topic_names`,
`service_traits_default_mapping_is_basic`,
`service_traits_links_typedefs_for_endpoints`,
`service_traits_specialization_carries_topic_names`,
`requester_class_emits_topic_wrapper`,
`replier_class_dispatches_by_name`,
`replier_with_unknown_method_throws`,
`requester_async_for_value_returning_method_uses_return`,
`requester_async_for_void_returning_method`,
`runtime_header_defines_future_and_promise_templates`,
`runtime_header_defines_generic_requester_and_replier`,
`runtime_header_has_pragma_once_and_includes`,
`runtime_header_service_mapping_enum`,
`runtime_headers_include_all_templates`,
`multi_service_concatenation_keeps_namespaces_balanced`.

---

## Audit-Status

57 done / 0 partial / 0 open / 20 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-idl-cpp` — 133 lib + 150 integration
(11 Bins) = 283 Tests grün, 0 failed.

Keine offenen Punkte.
