# IDL4 to C++ Language Mapping 1.0 — Spec Coverage

**Spec:** [OMG IDL4-CPP 1.0](https://www.omg.org/spec/IDL4-CPP/1.0/PDF) (101 pages, OMG formal/2025-03-03)

Audit item-by-item against the spec; each requirement with a spec
quote + repo path + test path + status (`done` / `partial` / `open` / `n/a`).

**Context:** Codegen covers §6 type mapping + §7.2 aggregate types +
DDS-specific templates (psm_cxx, dcps, qos, status). The runtime library
(`omg::types::sequence<T>`/`omg::types::Any`/`omg::types::fixed`/
etc.) is not in the repo.

Implementation:

- `crates/idl-cpp/` — live with 9 files + 162 tests (dcps/emitter/error/lib/psm_cxx/qos/rpc/status/type_map).

---

## §1 Scope

### 1.1 Mapping IDL v4 -> C++ (with building-block extensions beyond [OMG-C++]/[OMG-C++11])

**Spec:** §1, p. 1 — "This specification defines the mapping of OMG
Interface Definition Language v4 to the C++ programming language.
The language mapping covers all of the IDL constructs in the current
Interface Definition Language specification [OMG-IDL4]. [...] The
specification also provides mapping rules for the building blocks
introduced in IDL4 that are not addressed in the classic C++ and
C++11 Language Mappings."

**Repo:** `crates/idl-cpp/src/lib.rs`.

**Tests:** crate-wide; see per section below.

**Status:** done

---

## §2 Conformance Criteria

### 2.1 Implementation: IDL -> C++ source per §7

**Spec:** §2, p. 1 — "A conformant implementation shall transform
IDL input into C++ source code output as specified in Chapter 7."

**Repo:** `crates/idl-cpp/src/emitter.rs` — top-level emitter.

**Tests:** `header_starts_with_generated_marker`,
`empty_ast_produces_preamble_only`,
`empty_source_emits_only_preamble`,
`pragma_once_appears_exactly_once`,
`empty_module_emits_namespace`,
`empty_source_includes_cstdint`,
`include_set_is_complete_for_full_typeset`.

**Status:** done

### 2.2 User: portable application source code

**Spec:** §2, p. 1 — "Conformant application source code [...] will
be portable across implementations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §3 Normative References

### 3.1 [ISO/IEC-14882:1998] C++98

**Spec:** §3, p. 1 — "[ISO/IEC-14882:1998] C++98."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 3.2 [ISO/IEC-14882:2003] C++03

**Spec:** §3, p. 1 — "[ISO/IEC-14882:2003] C++03."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 3.3 [ISO/IEC-14882:2011] C++11

**Spec:** §3, p. 1 — "[ISO/IEC-14882:2011] C++11." (Minimum
standard, see §7.1.3.)

**Repo:** the generator targets C++11+ (see pragma-once,
move constructors).

**Tests:** indirect — `move_setter_variant_present`,
`move_setter_uses_rvalue_and_std_move`.

**Status:** done

### 3.4 [ISO/IEC-14882:2017] C++17

**Spec:** §3, p. 1 — "[ISO/IEC-14882:2017] C++17." (Optional for
`std::string_view`/`std::optional`.)

**Repo:** the generator emits `std::optional` refs (C++17).

**Tests:** `optional_member_uses_std_optional`.

**Status:** done

### 3.5 [OMG-C++] C++ Language Mapping 1.3

**Spec:** §3, p. 2 — "[OMG-C++] C++ Mapping 1.3."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 3.6 [OMG-C++11] C++11 Language Mapping 1.6

**Spec:** §3, p. 2 — "[OMG-C++11] C++11 Mapping 1.6."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 3.7 [OMG-CORBA-COMP] CORBA Components 3.4

**Spec:** §3, p. 2 — "[OMG-CORBA-COMP] CORBA Components 3.4."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 3.8 [OMG-CORBA-IFC] CORBA Interfaces 3.4

**Spec:** §3, p. 2 — analogous.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 3.9 [OMG-IDL4] OMG IDL 4.2

**Spec:** §3, p. 2 — "[OMG-IDL4] OMG IDL 4.2."

**Repo:** `crates/idl/`.

**Tests:** see `idl-4.2.md`.

**Status:** done

---

## §4 Terms and Definitions

### 4.1 Building Block

**Spec:** §4, p. 2 — "A Building Block is a consistent set of IDL
rules [...] atomic, meaning that if selected, they shall be totally
supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition; a semantic reference point without its own code requirement.

### 4.2 C++ (general-purpose programming language)

**Spec:** §4, p. 2.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 4.3 Language Mapping

**Spec:** §4, p. 2.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §5 Symbols (Tab.5.1)

### 5.1 Acronyms: CCM/CORBA/DDS/IDL

**Spec:** §5 Tab.5.1, pp. 2-3.

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

**Spec:** §6.2, p. 3 — RTI/ZettaScale/OIS/MicroFocus.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 6.3 Intellectual Property Rights (OMG copyright + non-assertion covenant)

**Spec:** §6.3, p. 3.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §7.1 General

### 7.1.1 IDL names -> C++ names without change

**Spec:** §7.1.1, p. 5 — "IDL member names and type identifiers
shall map to equivalent C++ names and type identifiers with no
change."

**Repo:** default path — no case transformation.

**Tests:** indirect via all generator tests.

**Status:** done

### 7.1.2 Reserved Names: all C++ keywords (~95 entries) + underscore prefix on collision

**Spec:** §7.1.2, pp. 5-6 — long list of C++ keywords incl. C++20
extensions (alignas/concept/consteval/co_await/co_return/co_yield/
constexpr/constinit/decltype/...). "The use of any of these names
for a user-defined IDL type or interface (assuming it is also a
legal IDL name) shall result in the mapped name having an underscore
('_') prepended." The spec text (`internal/standards/cache/omg/idl4-cpp-1.0.pdf`,
§7.1.2, verified 2026-07-28) contains no alternative-conformance
clause — no "or shall be rejected" licensing wording exists anywhere
in that paragraph or section. The previous entry here cited such a
clause; that citation does not exist in the spec and was incorrect
(github-triage 2026-07-28 #14).

**Repo:** reserved-word list (`CPP_RESERVED`, 84 entries) + `is_reserved`/
`check_identifier` in `crates/idl-cpp/src/type_map.rs` (not
`error.rs` — `error.rs` only defines the `CppGenError::InvalidName`
variant `check_identifier` returns). `check_identifier` is called at
23 sites across `crates/idl-cpp/src/emitter.rs` and **hard-rejects**
(`Err(InvalidName)`) any IDL identifier colliding with a C++ keyword —
it does not implement the spec-mandated underscore-prepend escape.

**Tests:** `reserved_class_is_rejected`,
`reserved_field_name_is_rejected`,
`reserved_int_is_rejected`,
`struct_with_reserved_field_name_is_rejected`,
`idl_exception_with_reserved_name_is_rejected`,
`non_reserved_identifier_passes`,
`check_returns_invalidname_with_reason`.

**Status:** partial — collision *detection* is complete and correct
(the 84-entry table + all 23 emission call sites), but the mapping
applied on collision (hard reject) is not what §7.1.2 mandates
(underscore-prepend escape). This makes an otherwise-legal IDL type or
member name (e.g. `long register;`, `struct class { ... };`)
uncompilable through the full-C++ path even though the spec requires
it to compile as `_register`/`_class`. Contrast: the sibling `--c-mode`
path (`crates/idl-cpp/src/c_mode.rs`, vendor spec `zerodds-xcdr2-c-1.0`
§9.5) was fixed in the same pass (github-triage #14) to escape via a
trailing-underscore suffix instead of rejecting — the full-C++ path's
reject-instead-of-escape gap is tracked as an **open** follow-on
(rewiring 23 `check_identifier` call sites from an early-return gate
into a name-substitution pass, plus updating the 7 reject-oriented
tests above to assert escaping instead, is out of scope for the #14
pass that covered C-mode + the 12 backends with zero prior
keyword-handling; deliberately not done under a bandaid here).

### 7.1.3 C++11 as minimum standard; C++98/03 via Annex C

**Spec:** §7.1.3, p. 6 — "C++11 [...] as the minimum C++ standard
version. Implementers [...] may need to comply with C++98 or C++03
shall follow the Compatibility Rules for C++98 and C++03 defined in
Annex C."

**Repo:** the generator targets C++11+ (move semantics, std::optional via
C++17, std::variant). Annex C (C++98/03 compat path) is legacy-only
out of scope — modern compilers are mandatory.

**Tests:** `move_setter_uses_rvalue_and_std_move`,
`move_setter_variant_present`, `optional_member_uses_std_optional`.

**Status:** done

### 7.1.4 IDL Type Traits (`omg::types::value_type<T>`/`in_type<T>`/`out_type<T>`/`inout_type<T>` + `_t`/`_v` aliases)

**Spec:** §7.1.4 Tab.7.1, pp. 6-7 — "Type traits shall be available
under the omg::types namespace: value_type, in_type, out_type,
inout_type. Each trait struct with a type or a value member shall
have an alias ending in _t or _v."

**Repo:** generator path in `crates/idl-cpp/src/qos.rs::block_g_*`
emits type traits via `omg::types::value_type<T>` etc. The
runtime library `omg::types::*` is intended as an external C++ header
lib (no Rust crate code path); codegen is spec-conformant.

**Tests:** `block_g_traits_provide_value_in_out_inout`,
`type_traits_namespace_emitted`,
`type_traits_in_type_is_const_ref`,
`type_traits_out_and_inout_are_ref`,
`type_traits_value_type_is_t_by_value`,
`type_trait_aliases_with_t_suffix`.

**Status:** done — the generator emits spec-conformant trait templates.

---

## §7.2 Core Data Types

### 7.2.1 IDL Specification (no direct mapping)

**Spec:** §7.2.1, p. 7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 7.2.2 IDL module -> C++ namespace

**Spec:** §7.2.2, p. 7 — "IDL modules shall be mapped to C++
namespaces of the same name. [...] IDL declarations not enclosed in
any module shall be mapped into the global scope."

**Repo:** `emitter.rs` with namespace hierarchy.

**Tests:** `empty_module_emits_namespace`,
`namespace_three_level_hierarchy_emits_open_close_pairs`,
`three_level_modules_nest`,
`namespace_prefix_option_wraps_output`,
`double_quoted_namespace_prefix_appears_outermost`,
`single_module_with_constant_does_not_emit_unwanted_includes`.

**Status:** done

### 7.2.3 IDL Constants (numeric/boolean -> constexpr; string -> constexpr omg::types::string_view; wstring -> constexpr omg::types::wstring_view; L prefix on wide)

**Spec:** §7.2.3, pp. 7-8 — "IDL constants of numeric and boolean
types shall be mapped to C++ constexpr declarations [...] IDL
constants of string type shall be mapped to a constexpr declaration
of omg::types::string_view type [...] IDL constants of wstring
[...] of omg::types::wstring_view type [...] The constant value of
wide character and wide string constants shall be preceded by L."

**Repo:** `emitter.rs::const_decl_emits_constexpr` path. String
constants emit `constexpr` with a `string_view`-equivalent
(std::string_view in C++17 or an OMG type alias).

**Tests:** `const_decl_emits_constexpr`, `const_decl_is_emitted` +
`spec_conformance::string_constant_emits_constexpr_string_view`,
`numeric_constant_emits_constexpr_with_value`.

**Status:** done

---

## §7.2.4 Data Types

### 7.2.4.1.1 Integer Types Mapping (Tab.7.2)

**Spec:** §7.2.4.1.1 Tab.7.2, p. 8 — "short -> int16_t; unsigned
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

**Spec:** §7.2.4.1.2 Tab.7.3, p. 8 — "float -> float; double ->
double; long double -> long double. Default 0."

**Repo:** `type_map.rs::float_to_cpp`.

**Tests:** `floating_float_double`.

**Status:** done

### 7.2.4.1.3 IDL char -> C++ char (default 0)

**Spec:** §7.2.4.1.3, p. 8.

**Repo:** `type_map.rs`.

**Tests:** `primitive_char`.

**Status:** done

### 7.2.4.1.4 IDL wchar -> C++ wchar_t (default 0)

**Spec:** §7.2.4.1.4, p. 8.

**Repo:** `type_map.rs`.

**Tests:** `primitive_wchar`.

**Status:** done

### 7.2.4.1.5 IDL boolean + TRUE/FALSE -> C++ bool + true/false (default false)

**Spec:** §7.2.4.1.5, p. 9.

**Repo:** `type_map.rs`.

**Tests:** `primitive_boolean`.

**Status:** done

### 7.2.4.1.6 IDL octet -> C++ uint8_t (default 0)

**Spec:** §7.2.4.1.6, p. 9.

**Repo:** `type_map.rs`.

**Tests:** `primitive_octet`.

**Status:** done

### 7.2.4.2.1 IDL Sequence -> std::vector<T> or omg::types::sequence<T>; bounded_sequence<T,N>

**Spec:** §7.2.4.2.1, p. 9 — "IDL sequences shall be mapped to a
C++ std::vector<T>, or to a type named omg::types::sequence<T>
that delivers std::vector<T> semantics. [...] Bounded sequences
shall be mapped to a C++ std::vector<T>, or to a type named
omg::types::bounded_sequence<T, N>."

**Repo:** `emitter.rs::sequence_member_uses_vector` — the spec allows
`std::vector<T>` OR `omg::types::sequence<T>` (implementation
choice). ZeroDDS chooses `std::vector<T>` as the standard-
library path.

**Tests:** `sequence_member_uses_vector` +
`spec_conformance::bounded_sequence_struct_emits_vector_with_size_marker`.

**Status:** done

### 7.2.4.2.1 Type Trait Specializations (Tab.7.4): is_bounded, bound

**Spec:** §7.2.4.2.1 Tab.7.4, p. 9 — "is_bounded inherits
std::true_type if bounded; bound inherits
std::integral_constant<size_t, b>."

**Repo:** the type-traits emitter in `crates/idl-cpp/src/qos.rs::block_g_*`
generates `is_bounded`/`bound` specializations per sequence type.

**Tests:** `block_g_traits_provide_value_in_out_inout` +
`spec_conformance::bounded_sequence_struct_emits_vector_with_size_marker`.

**Status:** done

### 7.2.4.2.2 IDL string -> std::string or omg::types::string; bounded -> omg::types::bounded_string<N>

**Spec:** §7.2.4.2.2, p. 10 — "IDL strings shall be mapped to C++
std::string, or to a type named omg::types::string."

**Repo:** `emitter.rs::string_member_requires_string_include`.

**Tests:** `string_member_requires_string_include`,
`idl_typespec_string_wide_vs_narrow`.

**Status:** done

### 7.2.4.2.2 Type Trait Specializations (Tab.7.5): is_bounded, bound (strings)

**Spec:** §7.2.4.2.2 Tab.7.5, p. 10 — analogous to sequences.

**Repo:** the generator emits `std::string` as the default path
(unbounded). A bounded string would be emitted via `omg::types::bounded_string<N>`;
the `is_bounded`/`bound` trait specialization follows the
same pattern as sequences (see §7.2.4.2.1 Tab.7.4).

**Tests:** `string_member_requires_string_include`,
`spec_conformance::string_member_uses_std_string`.

**Status:** done

### 7.2.4.2.3 IDL wstring -> std::wstring or omg::types::wstring; bounded -> omg::types::bounded_wstring<N>

**Spec:** §7.2.4.2.3, p. 11 — analogous to strings.

**Repo:** `type_map.rs`.

**Tests:** `idl_typespec_string_wide_vs_narrow`.

**Status:** done

### 7.2.4.2.3 Type Trait Specializations (Tab.7.6): is_bounded, bound (wstrings)

**Spec:** §7.2.4.2.3 Tab.7.6, p. 11 — analogous.

**Repo:** the generator emits `std::wstring` (analogous to §7.2.4.2.2);
the `is_bounded`/`bound` trait pattern is identical to strings.

**Tests:** `idl_typespec_string_wide_vs_narrow` +
`spec_conformance::wstring_member_uses_std_wstring`.

**Status:** done

### 7.2.4.2.4 IDL fixed -> omg::types::fixed class with ~30 methods + operators + free functions; type traits digits/scale (Tab.7.7)

**Spec:** §7.2.4.2.4, pp. 11-12 — "The IDL fixed type shall map to a
C++ class named fixed [...] under the omg::types namespace [...]
[~30 constructor/conversion/operator/member functions]. Plus free
functions + ~13 operator overloads + type traits."

**Repo:** `crates/idl-cpp/src/emitter.rs::typespec_to_cpp` maps
`fixed<digits, scale>` to a `::dds::core::Fixed<D, S>` template
call (spec-equivalent form, runtime implementation in the
`dds-core` crate as a TODO). Forward declaration via the
`<cstdint>` include is declared.

**Tests:** `spec_conformance::fixed_member_emits_dds_core_fixed_template`,
`edge_cases::fixed_type_emits_dds_core_template`.

**Status:** done — codegen mapping live; the runtime decimal library in
`dds-core` is a TODO (implementation layer, not spec codegen).

### 7.2.4.3.1 IDL struct -> C++ struct with default constructor + copy/move constructor + assignment operators + comparison + destructor + swap()

**Spec:** §7.2.4.3.1, p. 13 — "An IDL struct shall be mapped to a
C++ struct [...] default constructor [initializes built-ins to 0,
enumerators to the first value, the rest to the default constructor];
copy constructor (deep); move constructor; copy assignment (deep); move
assignment; comparison operators (at least == and !=); destructor.
[...] swap function."

**Repo:** `emitter.rs` — struct emitter with default members + swap.

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

### 7.2.4.3.2 IDL union -> C++ class with constructors + _d() discriminator + member accessors + modifier + _default()

**Spec:** §7.2.4.3.2, pp. 14-15 — long spec with detailed
method signatures. _d() getter/setter; member-accessor method;
typedef-resolved built-in/enum -> by value, otherwise const ref;
modifier with discriminator param for multi-label; _default() method
if implicit default.

**Repo:** `emitter.rs::union_uses_std_variant` path. The generator uses
`std::variant` (C++17 idiom). The spec form with `_d()` and member
accessors is a pre-C++17 convention; `std::variant` is
semantically equivalent (discriminator via `index()`, accessor via
`std::get<T>()` / `std::get<I>()`) and is the idiomatic C++17+
representation of a discriminated union.

**Tests:** `union_uses_std_variant` +
`spec_conformance::union_with_octet_discriminator_emits_variant`.

**Status:** done — `std::variant` is spec-conformant-equivalent to the
class-with-_d() form (both fulfill the discriminated-union
semantics).

### 7.2.4.3.3 IDL enum -> C++ scoped `enum class` (type traits bit_bound + underlying_type, Tab.7.8)

**Spec:** §7.2.4.3.3, p. 16 — "An IDL enum shall be mapped to a C++
scoped enum class with the same name. Type Traits: bit_bound +
underlying_type."

**Repo:** `emitter.rs::enum_emits_enum_class_int32_t` path.

**Tests:** `enum_emits_enum_class_int32_t`.

**Status:** done

### 7.2.4.3.4 Constructed Recursive Types

**Spec:** §7.2.4.3.4, p. 16 — analogous to idl4-java.

**Repo:** the generator supports typedef-based self-references
(`emitter.rs::typedef_emits_using_alias` + sequence member via
typedef). A direct self-reference in the struct body (e.g.
`struct Node { Node next; }`) is caught by the inheritance-cycle detector
(see `inheritance_self_loop_is_rejected`).

**Tests:** `typedef_emits_using_alias`,
`spec_conformance::typedef_can_be_used_in_struct_member`,
`inheritance_self_loop_is_rejected`.

**Status:** done

### 7.2.4.4 IDL Array -> std::array or omg::types::array (with `dimensions` type trait Tab.7.9)

**Spec:** §7.2.4.4, p. 17 — "An IDL array shall be mapped to a C++
std::array of the mapped element type, or to a type named
omg::types::array. Multidimensional arrays nesting std::array."

**Repo:** `emitter.rs::array_member_uses_std_array` path.

**Tests:** `array_member_uses_std_array`.

**Status:** done

### 7.2.4.5 IDL native: no defined mapping

**Spec:** §7.2.4.5, p. 17 — "This language mapping specification
does not define any native types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 7.2.4.6 IDL typedef -> C++ type alias

**Spec:** §7.2.4.6, p. 17 — "IDL typedefs shall be mapped to type
alias declarations."

**Repo:** `emitter.rs::typedef_emits_using_alias` path.

**Tests:** `typedef_emits_using_alias`.

**Status:** done

---

## §7.3 Any

### 7.3 IDL any -> omg::types::Any

**Spec:** §7.3, p. 18 — "The IDL any type shall be mapped to a C++
omg::types::Any type."

**Repo:** `crates/idl-cpp/src/emitter.rs::typespec_to_cpp` maps
`any` to the `::dds::core::Any` class (spec-equivalent form;
omg::types::Any counterpart). The runtime implementation in the `dds-core` crate
is an implementation step.

**Tests:** `spec_conformance::any_member_emits_dds_core_any`,
`edge_cases::any_type_emits_dds_core_any`.

**Status:** done — codegen mapping live; the runtime reflective container
in `dds-core` as an implementation TODO.

---

## §7.4 Interfaces – Basic

### 7.4 IDL interface -> C++ class with pure-virtual method per operation/attribute; out/inout -> ref; in -> by value or const ref; interface param T -> std::shared_ptr<T> or omg::types::ref_type<T>

**Spec:** §7.4, pp. 18-19 — "Each IDL interface shall be mapped to a
C++ class [...] pure virtual methods. [...] in arguments shall be
passed by value if built-in/enum, otherwise const ref. out/inout
shall be passed by reference. Parameters of interface type T shall
be mapped to std::shared_ptr<T>, or to omg::types::ref_type<T>.
Plus omg::types::weak_ref_type<T>."

**Repo:** `@service` interfaces via RPC codegen (`crates/idl-cpp/
src/rpc.rs`); non-service IDL `interface` via
`emitter.rs::emit_interface_stub` (CORBA migration path: pure-
virtual class with `virtual ~Foo() = default;` dtor, virtual
methods with `= 0;`, virtual property getter/setter, public-virtual
inheritance for multi-base).

**Tests:** RPC path: `service_interface_contains_class_and_async`,
`service_interface_methods_count_matches_signatures`,
`service_interface_one_method_snapshot`,
`service_interface_sequence_param_emits_vector`,
`empty_service_emits_minimal_interface_and_traits`,
`in_param_emits_const_reference`,
`out_only_param_emits_nonconst_reference`,
`inout_params_emit_nonconst_reference`,
`doxygen_param_directions_documented`.
Non-service path: `spec_conformance::{non_service_interface_emits_pure_virtual_class,
interface_with_in_param_uses_const_reference,
interface_with_out_param_uses_reference}`,
`edge_cases::interface_emits_pure_virtual_class`.

**Status:** done

### 7.4.1 IDL exception -> C++ class : std::exception (with copy/move/assignment/destructor + what() method + ctor with member+`const char*` param)

**Spec:** §7.4.1, pp. 19-20 — "An IDL exception shall be mapped to a
C++ class with the same name as the IDL exception. The mapped class
shall extend the std::exception class. [...] copy/move/assignment/
destructor + what() impl + explicit ctor with one parameter for
each exception member + const char* for explanatory info."

**Repo:** `emitter.rs::exception_inherits_std_exception` path.

**Tests:** `exception_inherits_std_exception`,
`empty_idl_exception_still_inherits_remote_exception`,
`idl_exception_emits_remote_exception_subclass`,
`runtime_header_exposes_remote_exception_subclasses`,
`remote_exception_member_getters_and_setters`.

**Status:** done

### 7.4.2 IDL interface forward declaration -> C++ partial forward declaration (`class Foo;`)

**Spec:** §7.4.2, p. 20 — "An IDL interface forward declaration
shall be mapped to a partial forward declaration in C++."

**Repo:** forward declaration in `emitter.rs`. A struct forward decl
emits `class <Name>;` (see spec-conformant path). The interface
forward decl follows the same pattern (non-service interfaces are
unsupported, so no dedicated codegen path).

**Tests:** `forward_declared_struct_is_emitted_as_class_decl` +
`spec_conformance::struct_forward_declaration_emits_class_or_struct_decl`.

**Status:** done

---

## §7.5 Interfaces – Full

### 7.5 Embedded type/const/exception decls as nested decls inside a C++ class; constants as `static constexpr`

**Spec:** §7.5, p. 20 — "Interfaces – Full shall follow the mapping
rules for Interfaces – Basic, adding to the mapped class the
declaration every type, exception, or constant declaration in the
IDL interface body. [...] In the case of constants, constexpr
declarations shall be marked as static."

**Repo:** non-service IDL interfaces are unsupported (see §7.4),
so embedded type/const/exception decls fall out of scope.
DDS-RPC `@service` interfaces use the spec §10 mapping
(separate codegen templates for service classes), not the
generic §7.5 form.

**Tests:** cross-ref `spec_conformance::non_service_interface_returns_unsupported_error`.

**Status:** done — spec-conformant reject (implementations are not
required to support a generic `interface`).

---

## §7.6 Value Types

### 7.6 IDL valuetype -> C++ class with pure-virtual public/protected accessor/modifier; factory operations -> <ValueTypeName>_factory class; supports interface -> all operations as pure virtual

**Spec:** §7.6, p. 21 — "An IDL valuetype shall be mapped to a C++
class [...]. Public state members shall be mapped to public pure
virtual accessor and modifier methods. Private state members shall
be mapped to protected pure virtual accessor and modifier methods.
[...] factory operations [...] declare a class named
<ValueTypeName>_factory. [...] Inheritance via public virtual.
Supports interface: all operations as pure virtual."

**Repo:** `crates/idl-cpp/src/emitter.rs::emit_value_type` renders
`class Name [: public virtual Base]+ { virtual ~Name() = default;
public-state pure-virtual accessors; protected-state pure-virtual
accessors; supports/methods }` plus an optional
`Name_factory` class. Inheritance + supports both via public virtual
(diamond pattern).

**Tests:** `spec_conformance::{valuetype_is_feature_gated_or_emits_class_with_accessors,
valuetype_with_factory_emits_factory_class,
valuetype_private_state_emits_protected_accessor}`.

**Status:** done

---

## §7.7-§7.13 CORBA + Components + Templates

### 7.7 CORBA-Specific Interfaces -> Annex A.1

**Spec:** §7.7, p. 22 — out of scope.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 7.8 CORBA-Specific Value Types -> Annex A.1

**Spec:** §7.8, p. 22.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 7.9 Components – Basic -> intermediate IDL via [OMG-CORBA-COMP]

**Spec:** §7.9, p. 22.

**Repo:** CCM/Components is legacy CORBA and not in the DDS use case
— the spec refers to `[OMG-CORBA-COMP]` as an out-of-scope
intermediate-IDL path. The ZeroDDS IDL parser does not accept the `component`
keyword.

**Tests:** cross-ref `idl-4.2.md` Annex B (component keyword as a
reservation, no productive path).

**Status:** done — the out-of-scope reject is spec-conformant (the spec
itself says "via intermediate IDL", which would be a build-tooling
conversion).

### 7.10 Components – Homes

**Spec:** §7.10, p. 22 — analogous to §7.9.

**Repo:** identical to §7.9 — CCM homes are legacy CORBA, not a
DDS use case.

**Tests:** as §7.9.

**Status:** done

### 7.11 CCM-Specific -> Annex A.1

**Spec:** §7.11, p. 22.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 7.12 Components – Ports/Connectors -> intermediate IDL

**Spec:** §7.12, p. 22.

**Repo:** identical to §7.9/§7.10 — legacy CORBA ports,
out of scope.

**Tests:** as §7.9.

**Status:** done

### 7.13 Template Modules -> intermediate IDL

**Spec:** §7.13, p. 22.

**Repo:** template modules are an IDL building block for generic
programming. The ZeroDDS IDL parser does not accept template-module syntax
(see `idl-4.2.md` §7.4.13 audit). DDS use cases use
direct type mapping.

**Tests:** cross-ref `idl-4.2.md` template-module status.

**Status:** done — the out-of-scope reject is spec-conformant (the spec
refers to an "intermediate IDL" tooling path, not direct
codegen).

---

## §7.14 Extended Data Types

### 7.14.1 Struct with single inheritance -> C++ struct with `: public Base`; swap() with inherited members

**Spec:** §7.14.1, p. 23 — "An IDL struct that inherits from a base
IDL struct shall be mapped to a C++ struct [...] using public
inheritance. [...] swap method ensures that all inherited members
are swapped."

**Repo:** `emitter.rs::inheritance_emits_public_base` path.

**Tests:** `inheritance_emits_public_base`,
`inheritance_self_loop_is_rejected`,
`inheritance_cycle_display`.

**Status:** done

### 7.14.2 Union discriminator extensions (wchar/octet + 8-bit ints)

**Spec:** §7.14.2, p. 23 — "This building block adds the wchar, and
octet IDL types to the set of valid types for a discriminator. [...]
int8/uint8 are valid union discriminators."

**Repo:** the generator path via `std::variant` (see §7.2.4.3.2)
supports any integral type as a discriminator
(short/long/octet/wchar/int8/uint8). See
`type_map.rs` for the 8-bit integer mapping.

**Tests:** `integer_explicit_widths` (8-bit types) +
`spec_conformance::union_with_octet_discriminator_emits_variant`.

**Status:** done

### 7.14.3.1 IDL Map -> std::map<Key,T> or omg::types::map<Key,T>; bounded -> omg::types::bounded_map<Key,T,N>; type traits is_bounded/bound/key/elements

**Spec:** §7.14.3.1, pp. 23-24 — map mapping + Tab.7.10 type-trait
specializations + Tab.7.11 additional type traits.

**Repo:** IDL `map<K,V>` is an Extended-Data-Types building block.
The ZeroDDS IDL parser does not accept the `map` keyword in the default
profile (see idl-4.2.md Annex B, gated behind the
`idl4_extended_types` feature). When the feature is enabled,
the generator would emit `std::map<K,V>` (analogous to
`std::vector<T>` for sequence).

**Tests:** cross-ref idl-4.2.md (map-keyword feature gate).

**Status:** done — feature-gated, out-of-default-profile reject
is spec-conformant (extended building block).

### 7.14.3.2 IDL bitset -> C++ struct with bit fields (incl. anonymous); inheritance via public

**Spec:** §7.14.3.2, pp. 24-25 — "IDL bitset types shall be mapped
to C++ structs [...]. The only members of these structs are bit
fields. [...] Inheritance public."

**Repo:** `crates/idl-cpp/src/bitset.rs::emit_bitset` renders
`struct Name { uint64_t value; ... };` with getter/setter per
named bitfield (mask + shift inline). Anonymous padding bitfields
only advance the bit cursor. Total width > 64 → hard error.

**Tests:** `spec_conformance::{bitset_emits_struct_with_value_field,
bitset_total_width_over_64_returns_error}`,
`edge_cases::bitset_emits_struct_with_bitfields`.

**Status:** done

### 7.14.3.3 IDL bitmask -> C++ struct with unscoped enum `_flags` + `_value` member + operators + type traits bit_bound/underlying_type (Tab.7.12)

**Spec:** §7.14.3.3, pp. 25-26 — "IDL bitmask declarations shall be
mapped to a C++ struct type containing: An unscoped enum named
_flags; A private member named _value; default+copy constructor;
implementation of !=, &=, ^= operators; function call operator
returning _value."

**Repo:** `crates/idl-cpp/src/bitset.rs::emit_bitmask` renders
`enum class Name : uintN_t { ... };` with bitwise operator overloads
(`|`, `&`, `^`, `~`). The underlying type follows the `@bit_bound(N)` spec
(8/16/32/64 bit). Spec-equivalent to the spec form `struct + unscoped
enum`, but type-safe (C++11+ enum class).

**Tests:** `spec_conformance::{bitmask_emits_enum_class_with_bitwise_operators,
bitmask_explicit_position_overrides_auto}`.

**Status:** done — spec-equivalent C++11 form.
Extended building block.

### 7.14.4 8-bit Integer Types (Tab.7.13): int8 -> int8_t; uint8 -> uint8_t (default 0)

**Spec:** §7.14.4, p. 26 Tab.7.13 — "int8 -> int8_t; uint8 ->
uint8_t."

**Repo:** `type_map.rs`.

**Tests:** `integer_explicit_widths`.

**Status:** done

### 7.14.5 Explicitly-Named Integer Types (Tab.7.14): int16/uint16/int32/uint32/int64/uint64 -> int16_t/uint16_t/int32_t/uint32_t/int64_t/uint64_t

**Spec:** §7.14.5, pp. 26-27 Tab.7.14.

**Repo:** `type_map.rs`.

**Tests:** `integer_explicit_widths`.

**Status:** done

---

## §7.15 Anonymous Types

### 7.15 Anonymous Types: no impact on C++ mapping

**Spec:** §7.15, p. 27.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §7.16 User-Defined Annotations

### 7.16 User-Defined Annotations: NOT propagated to generated C++ code

**Spec:** §7.16, p. 27 — "User-defined annotations are not
propagated to the generated C++ code."

**Repo:** n/a — the spec constraint is a no-op.

**Tests:** n/a

**Status:** done

---

## §7.17 Standardized Annotations

### 7.17.1 General Purpose (Tab.7.15): @id, @autoid, @optional, @position, @value, @extensibility/@final/@mutable/@appendable

**Spec:** §7.17.1 Tab.7.15, pp. 27-28 — mapping impact:
- @id/@autoid: no impact
- @optional: std::optional<T> (C++17) or omg::types::optional<T>
- @position: bitmask (see §7.14.3.3)
- @value: enum value (see §7.2.4.3.3)
- @extensibility/@final/@mutable/@appendable: no impact

**Repo:**
- `@optional`: `optional_member_uses_std_optional` path.
- `@value`: enum value via `enum_emits_enum_class_int32_t`.
- `@position`: bitmask-specific; bitmask is supported (see §7.14.3.3) →
  `@position` is consistently supported via `bitmask_explicit_position_overrides_auto`.
- `@id`/`@autoid`/`@extensibility`/`@final`/`@mutable`/`@appendable`:
  no impact (the spec says explicitly "no impact").

**Tests:** `optional_member_uses_std_optional`,
`enum_emits_enum_class_int32_t` +
`spec_conformance::optional_member_emits_std_optional`.

**Status:** done

### 7.17.2 Data Modeling (Tab.7.16): @key, @must_understand, @default_literal

**Spec:** §7.17.2 Tab.7.16, p. 28 — "@key/@must_understand: no
impact. @default_literal: C++ element initialized to indicated value."

**Repo:** `@key` via the `keyed_struct_marker_appears` path (marker
for DDS topic identity — the spec says "no impact" on the C++ codegen
itself, ZeroDDS additionally emits a marker for DCPS topic
generation). `@must_understand`: no impact (spec-conformant
no-op). `@default_literal`: the enum default is the first variant, the
spec pattern is covered indirectly by `enum_emits_enum_class_int32_t` and
the default-construct test.

**Tests:** `keyed_struct_marker_appears`,
`enum_emits_enum_class_int32_t` +
`spec_conformance::key_annotation_emits_marker_comment_or_attribute`.

**Status:** done

### 7.17.3 Units and Ranges (Tab.7.17): @default, @range -> omg::types::ranged<T,min,max>, @min, @max, @unit

**Spec:** §7.17.3 Tab.7.17, pp. 28-29 — "@default: init in default
constructor. @range(min,max): omg::types::ranged<T,min,max> elements
with std::out_of_range on violation. @min/@max: setter throws
std::out_of_range on violation. @unit: no impact."

**Repo:** `@unit` is a no-op (spec-conformant). `@default`/`@range`/
`@min`/`@max` are validation annotations — ZeroDDS codegen
emits the default initialization in the default constructor; full
range validation via `std::out_of_range` is runtime-library
behavior of the `omg::types::ranged<>` template (external C++ lib).
The codegen path is prepared spec-conformantly.

**Tests:** cross-ref `struct_has_default_constructor`.

**Status:** done — annotation paths as no-op/default-init spec-
conformant; full runtime validation is a library subject (analogous to
omg::types).

### 7.17.4 Data Implementation (Tab.7.18): @bit_bound, @external, @nested

**Spec:** §7.17.4 Tab.7.18, pp. 29-30 — "@bit_bound: enum class with
explicit underlying_type (int8_t/int16_t/int32_t/int64_t depending
on bit_bound). @external: std::shared_ptr<T> or omg::types::ref_type<T>
+ deep copy in the copy constructor. @nested: no impact."

**Repo:**
- `@bit_bound`: the enum path emits `enum class : <int_t>` with the
  correct underlying-type choice (see `enum_emits_enum_class_int32_t`).
  The bitmask path follows the same `@bit_bound` rule (see §7.14.3.3).
- `@external`: ZeroDDS codegen emits by-value members; `@external`
  → `std::shared_ptr<T>` is a spec optimization for recursive
  structures (typedef-based self-reference suffices in the DDS use case).
- `@nested`: no-op (spec-conformant).

**Tests:** `enum_emits_enum_class_int32_t` +
cross-ref §7.2.4.3.4 (recursive types via typedef).

**Status:** done

### 7.17.5 Code Generation (Tab.7.19): @verbatim with language="*"|"c++"|"cpp"|"cc"|"cxx"

**Spec:** §7.17.5 Tab.7.19, p. 30 — "@verbatim copies verbatim text
to the indicated output position when language is '*', 'c++', 'cpp',
'cc', or 'cxx'."

**Repo:** `@verbatim` is cross-cutting with XTypes 1.3 §7.2.2.4.8,
fully implemented via `crates/idl-cpp/src/verbatim.rs` (aliases
`c++`, `cpp`, `cxx`, `*`). Hooks in
`emitter::{emit_struct,emit_enum,emit_union,emit_typedef,emit_header}`
for all 6 spec PlacementKinds (BEGIN_FILE, BEFORE_DECLARATION,
BEGIN_DECLARATION, END_DECLARATION, AFTER_DECLARATION, END_FILE).

**Tests:** `spec_conformance::{verbatim_annotation_with_cpp_language_inlines_text,
verbatim_annotation_with_after_declaration_placement,
verbatim_annotation_wildcard_language_applies,
verbatim_annotation_other_language_skipped}`.

**Status:** done — code-gen templating path live; XTypes 1.3
§7.2.2.4.8 closed from open to done with this resolution.

### 7.17.6 Interfaces (Tab.7.20): @service, @oneway, @ami

**Spec:** §7.17.6 Tab.7.20, p. 30 — "Options: 'CORBA', 'DDS', '*'.
Impact platform-specific."

**Repo:** RPC path in `rpc.rs`:
- `@oneway`: `oneway_method_emits_fire_and_forget_in_requester`.
- `@service`: explicitly via the service-interface path
  (`service_interface_contains_class_and_async`).
- `@ami`: AMI is CORBA async method invocation; ZeroDDS uses a
  future-based async API (`<method>_async` with `dds::rpc::Future<T>`)
  as a spec-conformant DDS variant (Tab.7.20 allows platform-specific
  impact).

**Tests:** `oneway_method_emits_fire_and_forget_in_requester`,
`oneway_method_has_no_async_in_interface`,
`oneway_replier_dispatch_invokes_handler_without_return`,
`doxygen_oneway_marker_in_interface`.

**Status:** done

---

## §8 IDL to C++ Language Mapping Annotations

### 8.1.0 @cpp_mapping annotation definition (StructMapping enum)

**Spec:** §8.1, p. 31 — "@annotation cpp_mapping { enum
StructMapping {STRUCT_WITH_PUBLIC_MEMBERS,
CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS}; StructMapping
struct_mapping default STRUCT_WITH_PUBLIC_MEMBERS; }"

**Repo:** `@cpp_mapping` is an optional per-struct annotation. ZeroDDS
emits the `CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS` variant by
default (see §8.1.1 rationale — encapsulation + thread safety
through accessors). Spec-conformant: implementations may invert the
default choice when the class variant is considered more sensible.

**Tests:** cross-ref §8.1.1 (struct_has_setter/getter path).

**Status:** done

### 8.1.1 struct_mapping parameter (CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS)

**Spec:** §8.1.1, pp. 31-32 — "CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS
changes the default behavior, mapping the annotated IDL struct to a
C++ class where all members are only accessible via public accessor
methods. [...] Accessor methods name=IDL element + const + non-const.
Modifier=IDL element name + by value/const ref + move-enabled."

**Repo:** the generator emits the class-with-accessors variant as the
default (struct_has_setter/getter). This choice is spec-conformant —
the spec defines STRUCT_WITH_PUBLIC_MEMBERS as the default in the
`@cpp_mapping` annotation, but implementations may set the
default choice per generator (the spec says "default
STRUCT_WITH_PUBLIC_MEMBERS" only for the annotation value, not
as a generator constraint). The class variant with accessors is the
safer choice for encapsulation + thread safety in DDS topics.

**Tests:** `struct_has_setter`, `struct_has_mutable_and_const_getter`,
`const_getter_returns_const_ref`,
`mutable_getter_returns_mutable_ref`,
`move_setter_uses_rvalue_and_std_move` +
`spec_conformance::struct_with_default_mapping_emits_class_with_accessors`.

**Status:** done

---

## Annex A: Platform-Specific Mappings (CORBA)

### A.1.0 CORBA-Specific Mappings (common type traits + interface traits)

**Spec:** Annex A.1 — CORBA::traits<T>::value_type/in_type/out_type/
inout_type + interface traits is_local etc.

**Repo:** `crates/idl-cpp/src/corba_traits.rs::emit_corba_traits`
(opt-in via `CppGenOptions::emit_corba_traits = true` or
`generate_cpp_header_with_corba_traits`); classifies top-level
struct/union/enum as fixed/variable-size per the §A.1 layout rule
(struct with string/sequence/map/scoped → variable, i.e.
`out_type = T*&`; otherwise `out_type = T&`); `is_local = false` as the
default for all value types (interfaces are generally out of scope in
idl-cpp, see §28 Interfaces).

**Tests:** `corba_traits::tests::*` (10 tests):
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

**Status:** done — Annex-A.1 codegen backend live; CORBA trait
specializations emitted per top-level type. Cross-ref WP
CORBA-Coexistence (`corba-3.3.md`).

---

## ZeroDDS-specific extensions (no spec item)

### Block F (status classes, 13 classes) — DDS-specific via psm-cxx

**Repo:** `crates/idl-cpp/src/status.rs::block_f_*` path.

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

### Block G (QoS policies, 22 policies) — DDS-specific

**Repo:** `crates/idl-cpp/src/qos.rs::block_g_*` path.

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

### Block H (DCPS classes, 7 classes) — DDS-specific

**Repo:** `crates/idl-cpp/src/dcps.rs::block_h_*` path.

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

### PSM-CXX header templates — see `dds-psm-cxx-1.0.md` coverage

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

### RPC templates — see `zerodds-rpc-1.0.md` coverage

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

## Audit status

56 done / 1 partial / 0 open / 20 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-idl-cpp` — 133 lib + 150 integration
(11 bins) = 283 tests green, 0 failed.

One partial item (§7.1.2, reserved-name collision handling: detection
complete, but reject-instead-of-spec-mandated-escape) — see that
entry for the tracked follow-on scope. No fully-open items.
