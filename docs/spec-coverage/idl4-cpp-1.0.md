# IDL4 to C++ Language Mapping 1.0 — Spec-Coverage

**Spec:** [OMG IDL4-CPP 1.0](https://www.omg.org/spec/IDL4-CPP/1.0/PDF) (101 Seiten, OMG formal/25-03-03)

Item-für-Item-Audit gegen die Spec; jede Anforderung mit Spec-Zitat,
Repo-Pfad, Test-Pfad und Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** Die C++-Spalte des IDL-Transplantats lebt heute in vier
Schichten:

- `crates/lang-rules/src/adapter/cpp.rs` — IDL-Datentypen (Struct, Union,
  Enum, Bitset, Bitmask, Typedef, Konstante, Exception, Sequenz, Array,
  Map, String, `@optional`, `@external`) nach C++-CST.
- `crates/lang-cst/src/cpp/mod.rs` — der C++-CST (`escape_keyword`,
  `Include`, `Primitive`).
- `crates/lang-render/src/cpp.rs` — der Renderer.
- `crates/lang-rules/src/emit/interface_cpp.rs` — Interfaces über die
  DDS-RPC-1.0-§7.11.1-Bindung (nicht die generische §7.4-Form).

Handgeschriebene Laufzeit, nicht aus dieser Spec erzeugt:

- `crates/cpp/include/dds/` — DDS-PSM-Cxx-1.0-API (Time, InstanceHandle,
  Status, Fixed, Exception, Topic, TopicTraits, Publisher, Subscriber,
  DomainParticipant).
- `runtime/cpp/include/` — Wire-Laufzeit (`zerodds/wire.hpp`,
  `dds/rpc.hpp`); das Korpus-Gate compiliert dagegen.

**Befund, der die Seite trägt:** Die C++-Spalte emittiert an keiner Stelle
`omg::types`. `grep -rl "omg::types" crates/ tools/ runtime/ endpoints/`
findet genau eine Datei — `crates/corba-codegen/src/traits.rs`, einen
Rust-CORBA-Generator, nicht diese C++-Bindung. Alle Items, deren
Anforderung ein `omg::types`-Alias oder -Trait ist, sind deshalb
`open` oder `partial`.

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

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::every_corpus_case_renders_or_names_its_refusal`,
`crates/lang-rules/tests/cpp_corpus_compiles.rs::the_compiler_check_catches_a_deliberate_error`

**Status:** partial — die Datentyp-Transformation ist live und rendert
jeden Korpusfall (bzw. benennt die Verweigerung); das Compiler-Gate prüft
die erzeugten Header. Kapitel 7 ist aber nicht vollständig erfüllt: die
`omg::types`-Laufzeit (Type Traits, `Any`, `fixed`, `sequence`/`string`-
Aliase) fehlt, §7.4-Interfaces laufen über die DDS-RPC-Bindung statt über
die generische Form, §7.4.1-Exceptions und §7.6-Valuetypes sind anders
geformt.

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

**Status:** `n/a (informative)`

---

## §6 Additional Information

### 6.1-6.3 Änderungen, Acknowledgments, Intellectual Property Rights

**Spec:** §6.1-6.3, S. 3.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## §7.1 General

### 7.1.1 Names

**Spec:** §7.1.1, S. 5 — "IDL member names and type identifiers shall
map to equivalent C++ names and type identifiers with no change."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::a_sequence_is_a_vector_and_a_map_is_a_class_over_a_vector_of_pairs` — `crates/lang-rules/src/adapter/cpp.rs:2752: assert!(text.contains("std::vector<std::int32_t> items;"), "{text}");`

**Status:** done — der Member-Name `items` bleibt unverändert. Die einzige
Namensänderung ist das Reserved-Name-Escaping aus §7.1.2.

### 7.1.2 Reserved Names

**Spec:** §7.1.2, S. 5-6 — "The use of any of these names for a
user-defined IDL type or interface (assuming it is also a legal IDL name)
shall result in the mapped name having an underscore ('_') prepended."

**Repo:** `crates/lang-cst/src/cpp/mod.rs` (`escape_keyword`)

**Tests:** `crates/lang-cst/src/cpp/mod.rs::a_keyword_escapes_with_a_trailing_underscore` — `crates/lang-cst/src/cpp/mod.rs:924: assert_eq!(escape_keyword("class"), "class_");`

**Status:** partial — Kollisionen mit C++-Keywords werden erkannt und
umgeschrieben, aber mit einem **angehängten** Unterstrich
(`class` -> `class_`), nicht mit dem von §7.1.2 mandatierten
**vorangestellten** Unterstrich (`_class`). Die Erkennung ist vollständig;
das angewandte Mapping weicht ab.

### 7.1.3 C++ Language Version Requirements

**Spec:** §7.1.3, S. 6 — "The language mappings defined in this
specification require C++11 [ISO/IEC-14882:2011] as the minimum C++
standard version."

**Repo:** `crates/lang-rules/tests/cpp_corpus_compiles.rs`

**Tests:** `crates/lang-rules/tests/cpp_corpus_compiles.rs::every_corpus_case_that_renders_also_compiles` — `crates/lang-rules/tests/cpp_corpus_compiles.rs:159: .args(["-std=c++20", "-fsyntax-only", "-Wall", "-Wextra", "-Werror"])`

**Status:** done — der Korpus wird unter `-std=c++20` syntaktisch geprüft;
das erfüllt die Mindestanforderung C++11 und deckt die tatsächlich
genutzten C++17-Konstrukte (`std::optional`, `inline constexpr`) ab. Der
Korpus-Test ist `#[ignore]` und läuft mit `--include-ignored`.

### 7.1.4 IDL Type Traits (Tab.7.1 und Tab.7.4-7.12)

**Spec:** §7.1.4 Tab.7.1, S. 6-7 — "Type traits shall be available under
the omg::types namespace" (`value_type`, `in_type`, `out_type`,
`inout_type`, `_t`/`_v`-Aliase); dazu die Spezialisierungen Tab.7.4
(Sequences), Tab.7.5 (Strings), Tab.7.6 (Wstrings), Tab.7.7 (`digits`,
`scale`), Tab.7.8 (`bit_bound`, `underlying_type`), Tab.7.9
(`dimensions`), Tab.7.10/7.11 (`is_bounded`, `bound`, `key`, `elements`)
und Tab.7.12 (`bit_bound`, `underlying_type`).

**Repo:** —

**Tests:** —

**Status:** open — kein `omg::types`-Namespace. `grep -rl "omg::types"
crates/ tools/ runtime/ endpoints/` findet nur
`crates/corba-codegen/src/traits.rs` (ein Rust-CORBA-Generator). Die
C++-Spalte schreibt `std::`-Typen direkt; kein Trait der Tabellen 7.1 und
7.4-7.12 wird emittiert.

---

## §7.2 Core Data Types

### 7.2.1 IDL Specification

**Spec:** §7.2.1, S. 7 — "There is no direct mapping of the IDL
Specification itself."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### 7.2.2 Modules

**Spec:** §7.2.2, S. 7 — "IDL modules shall be mapped to C++ namespaces
of the same name. [...] IDL declarations not enclosed in any module shall
be mapped into the global scope."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::a_module_is_a_namespace_and_not_a_prefix` — `crates/lang-rules/src/adapter/cpp.rs:1987: assert!(text.contains("namespace a {"), "{text}");`

**Status:** done — Module werden zu `namespace`-Blöcken, Referenzen aus
dem Wurzel-Scope geschrieben (`::a::P`), kein `a_b_P`-Präfix.

### 7.2.3 Constants

**Spec:** §7.2.3, S. 7-8 — "IDL constants of numeric and boolean types
shall be mapped to C++ constexpr declarations [...] IDL constants of
string type shall be mapped to a constexpr declaration of
omg::types::string_view type [...] of wstring type [...] omg::types::wstring_view
[...] The constant value of wide character and wide string constants
shall be preceded by L."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (Konstantenschleife,
`cpp_constant`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::every_corpus_case_renders_or_names_its_refusal`

**Status:** partial — numerische und boolesche Konstanten werden als
`inline constexpr` emittiert. String-, Wstring- und Fixed-Konstanten
werden als `const char *` ausgegeben, nicht als `constexpr
omg::types::string_view` oder `std::string_view`; das `L`-Präfix für Wide-
Literale fehlt, weil `wstring` als UTF-8 gehalten wird. Keine Assertion
deckt die Konstantenform direkt.

---

## §7.2.4 Data Types

### 7.2.4.1 Basic Types (Tab.7.2/7.3)

**Spec:** §7.2.4.1.1-§7.2.4.1.6, S. 8-9 — Integer (Tab.7.2), Floating
Point (Tab.7.3), char, wchar, boolean, octet.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`cpp_primitive`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::the_tag_keeps_apart_what_the_cpp_spelling_collapses`,
`crates/lang-rules/src/adapter/cpp.rs::an_array_nests_with_the_outer_dimension_outermost`

**Status:** partial — `long -> std::int32_t` ist assertiert (Array-,
Interface- und Verbatim-Tests), und `octet`/`uint8` sowie `wchar`/
`unsigned short` fallen auf denselben Typ (Tag-Test). Die übrigen
Tabellenbreiten (`short -> int16_t`, `long long -> int64_t`, `char`,
`wchar`, `boolean`, `octet`, `float`) haben keine eigene Assertion.
`long double` wird bewusst auf `zerodds::wire::Float128` abgebildet statt
auf das plattformabhängige C++ `long double` (§7.2.4.1.2). `cpp_primitive`
ist erschöpfend, ein fehlender Grundtyp ist ein Compile-Fehler.

### 7.2.4.2.1 Sequences -> std::vector<T>

**Spec:** §7.2.4.2.1, S. 9 — "IDL sequences shall be mapped to a C++
std::vector<T>, or to a type named omg::types::sequence<T> [...]."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`value_type`,
Sequence-Zweig)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::a_sequence_is_a_vector_and_a_map_is_a_class_over_a_vector_of_pairs` — `crates/lang-rules/src/adapter/cpp.rs:2752: assert!(text.contains("std::vector<std::int32_t> items;"), "{text}");`

**Status:** done — `std::vector<T>` ist eine von §7.2.4.2.1 zugelassene
Form (die `omg::types::sequence`-Alternative wird in §7.1.4 auditiert).

### 7.2.4.2.1 omg::types::sequence/bounded_sequence

**Spec:** §7.2.4.2.1 Tab.7.4, S. 9 — "implementers shall always define
omg::types::sequence<T> [...] omg::types::bounded_sequence<T,N> [...] is_bounded
[...] bound."

**Repo:** —

**Tests:** —

**Status:** open — weder `omg::types::sequence<T>` noch
`omg::types::bounded_sequence<T,N>` noch die `is_bounded`/`bound`-
Spezialisierungen existieren.

### 7.2.4.2.2 Strings -> std::string

**Spec:** §7.2.4.2.2, S. 10 — "IDL strings shall be mapped to C++
std::string, or to a type named omg::types::string."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`value_type`,
String-Zweig)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::the_tag_keeps_apart_what_the_cpp_spelling_collapses`

**Status:** partial — `string` wird als `std::string` gehalten
(zugelassene Form). Für den Data-Member existiert keine direkte
Assertion (nur der Tag-Test zeigt, dass `string` und `wstring` auf
denselben C++-Typ fallen); `omg::types::string` fehlt.

### 7.2.4.2.2 omg::types::string/bounded_string

**Spec:** §7.2.4.2.2 Tab.7.5, S. 10 — "implementers shall always define
omg::types::string [...] omg::types::bounded_string<N> [...] is_bounded
[...] bound."

**Repo:** —

**Tests:** —

**Status:** open — keine `omg::types::string`/`bounded_string<N>`-Aliase,
keine `is_bounded`/`bound`-Traits.

### 7.2.4.2.3 Wstrings

**Spec:** §7.2.4.2.3, S. 11 — "IDL wstrings shall be mapped to C++
std::wstrings, or to a type named omg::types::wstring [...]."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`value_type`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::the_tag_keeps_apart_what_the_cpp_spelling_collapses`

**Status:** partial — `wstring` wird als `std::string` mit UTF-8 gehalten,
nicht als `std::wstring` und nicht als `omg::types::wstring`; die Spec-
Form ist damit nicht erfüllt. Keine direkte Assertion.

### 7.2.4.2.3 omg::types::wstring/bounded_wstring

**Spec:** §7.2.4.2.3 Tab.7.6, S. 11 — "omg::types::wstring [...]
omg::types::bounded_wstring<N> [...] is_bounded [...] bound."

**Repo:** —

**Tests:** —

**Status:** open — kein `omg::types::wstring`/`bounded_wstring<N>`, keine
Traits.

### 7.2.4.2.4 Fixed Type

**Spec:** §7.2.4.2.4, S. 11-12 — "The IDL fixed type shall map to a C++
class named fixed [...] under the omg::types namespace" (rund 30
Methoden plus freie Funktionen und die `digits`/`scale`-Traits Tab.7.7).

**Repo:** —

**Tests:** —

**Status:** open — die Datentyp-Spalte bildet `fixed<P,S>` in
`crates/lang-rules/src/adapter/cpp.rs` auf `std::string` ab. Die
handgeschriebene `dds::core::Fixed<P,S>` in
`crates/cpp/include/dds/core/Fixed.hpp` ist die DDS-PSM-Cxx-Laufzeit
(eine andere Spec) und bietet nur BCD-Round-Trip plus
String-Konvertierung, nicht die ~30 Spec-Methoden; `omg::types::fixed`
und die `digits`/`scale`-Traits fehlen.

### 7.2.4.3.1 Structures


**Spec:** §7.2.4.3.1, S. 13 — "An IDL struct shall be mapped to a C++
struct [...] default constructor [...] copy constructor [...] move
constructor [...] copy assignment [...] move assignment [...] comparison
operators, including at least 'equal to' and 'not equal to' [...]
destructor [...] swap function in the namespace."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs` (Klasse mit Zugriffsfunktionen, `==`, `!=`, `swap`), `crates/lang-rules/src/adapter/cpp.rs` (Structure-Zweig)

**Tests:** `crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader` (`crates/cpp/tests/generated_psm_roundtrip.cpp`: Konstruktoren, Kopie, Verschiebung, `swap`, Gleichheit)

**Status:** done — die Standardabbildung (`--cpp-mapping psm`) erzeugt eine Klasse mit den Zugriffsfunktionen, dem Konstruktor über alle Member, `==`, `!=` und `swap` im Namensraum (§7.12); Kopieren und Verschieben sind die impliziten Funktionen. Die Option `native` schreibt die Struktur der Codecs mit öffentlichen Membern, `swap` und, bei Zeigermembern, den erzeugten Kopier- und Verschiebefunktionen, aber ohne `==`. Log: `.agent/REPORT.md`.

### 7.2.4.3.1 Default-Initialisierung

**Spec:** §7.2.4.3.1, S. 13 — "A default constructor that initializes all
built-in data types to their default value [...] enumerators to their
first value, and the rest of members using their default constructor."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`member_default`)

**Tests:** `tools/idlc/tests/cpp_headers_together.rs::two_generated_headers_share_a_translation_unit_and_their_structs_start_at_their_defaults` — `tools/idlc/tests/cpp_headers_together.rs:126: status.success(),`

**Status:** done — der Test compiliert und führt default-konstruierte
Structs aus; der Exit-Code ist ungleich 0, wenn ein Built-in nicht 0,
`bool` nicht `false`, `double` nicht 0.0, das Enum nicht die erste
Variante oder Array/Sequenz nicht leer sind.

### 7.2.4.3.2 Unions

**Spec:** §7.2.4.3.2, S. 14-15 — Klasse mit `_d() const`/`void _d(T)`,
Member-Accessor/-Modifier je Zweig, Multi-Label-Modifier mit
Discriminator-Parameter und `_default()`.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (Union-Zweig)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`

**Status:** partial — die Union wird als Klasse mit `_d`-Feld und einem
Feld pro Zweig emittiert (assertiert: `struct U {`, `std::int32_t _d;`).
Die geforderten Zugriffs-/Modifier-Methoden, die Multi-Label-Variante und
`_default()` fehlen; die Zweige sind öffentliche Felder.

### 7.2.4.3.3 Enumerations

**Spec:** §7.2.4.3.3, S. 16 — "An IDL enum shall be mapped to a C++
scoped enum class with the same name as the IDL enum type."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (Enumeration-Zweig)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::an_enumeration_takes_an_underlying_type_from_its_bit_bound` — `crates/lang-rules/src/adapter/cpp.rs:2050: assert!(narrow.contains("enum class E : std::uint8_t {"), "{narrow}");`

**Status:** done — scoped `enum class` mit gleichem Namen; das underlying
type wird aus `@bit_bound` und den Enumeratorwerten gewählt.

### 7.2.4.3.3 Enumeration Type Traits

**Spec:** §7.2.4.3.3 Tab.7.8, S. 17 — "bit_bound [...] underlying_type."

**Repo:** —

**Tests:** —

**Status:** open — die `bit_bound`/`underlying_type`-Spezialisierungen
werden nicht emittiert (siehe §7.1.4).

### 7.2.4.3.4 Constructed Recursive Types

**Spec:** §7.2.4.3.4, S. 16 — "Constructed recursive types are supported
by mapping the involved types directly to C++."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (Forward-Declarations,
Sequence-/`shared_ptr`-Zweige)

**Tests:** `crates/lang-rules/tests/external_is_an_indirection.rs::a_recursive_external_member_is_a_shared_pointer_and_not_a_value`

**Status:** partial — Forward-Declarations für Aggregate und
`std::vector<T>`/`std::shared_ptr<T>` erlauben Selbstreferenz; belegt ist
nur der `@external`-Rekursivfall (`std::shared_ptr<::m::Node> next;`). Der
allgemeine Fall (Selbstreferenz über `sequence<Node>` ohne `@external`)
hat keine eigene Assertion.

### 7.2.4.4 Arrays

**Spec:** §7.2.4.4, S. 17 — "An IDL array shall be mapped to a C++
std::array of the mapped element type, or to a type named
omg::types::array [...] Multidimensional arrays shall be constructed
nesting std::array."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (Array-Zweig)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::an_array_nests_with_the_outer_dimension_outermost` — `crates/lang-rules/src/adapter/cpp.rs:2023: assert!(text.contains("std::array<std::array<std::int32_t, 3>, 2> a = {};"), "{text}");`

**Status:** done — `std::array`, mehrdimensional outermost-first genestet;
eine von §7.2.4.4 zugelassene Form.

### 7.2.4.4 omg::types::array und dimensions

**Spec:** §7.2.4.4 Tab.7.9, S. 17 — "omg::types::array [...] dimensions."

**Repo:** —

**Tests:** —

**Status:** open — `omg::types::array` und der `dimensions`-Trait fehlen.

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

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (Alias-Zweig)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::every_corpus_case_renders_or_names_its_refusal`

**Status:** partial — `typedef` wird als `cpp::Alias` (C++ `using`)
emittiert; keine Assertion prüft die Alias-Deklaration direkt, und der
Korpus-Compile unterscheidet eine Alias- nicht von einer Makro-Form.

---

## §7.3 Any

### 7.3 Any -> omg::types::Any

**Spec:** §7.3, S. 18 — "The IDL any type shall be mapped to a C++
omg::types::Any type."

**Repo:** —

**Tests:** —

**Status:** open — `any` wird in `crates/lang-rules/src/adapter/cpp.rs`
(`cpp_value_type`, `TypeKind::Any`) mit `NoLanguageForm` abgelehnt
(Test `an_any_member_is_refused_by_name_at_any_depth`, auch für
`sequence<any>`). Kein `omg::types::Any`.

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

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_synchronous_operations_of_rule_six`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_names_an_attribute_accessor_after_the_attribute`

**Status:** partial — Interfaces werden über die
DDS-RPC-1.0-§7.11.1-Bindung als Klassen mit pure-virtual Operationen und
Attribut-Accessoren emittiert (assertiert: `in` by value/const ref,
`out`/`inout` by reference, Accessor nach Attributnamen). Die Bindung
folgt DDS-RPC 1.0 und fügt `IAsync`/`Requester`/`Replier`/`Client`/
`Service` hinzu, die §7.4 nicht kennt; `omg::types::ref_type<T>`/
`weak_ref_type<T>` für Interface-Parameter fehlen.

### 7.4.1 Exceptions

**Spec:** §7.4.1, S. 19-20 — "An IDL exception shall be mapped to a C++
class [...] shall extend the std::exception class [...] copy constructor,
move constructor, assignment operator, move operator, and destructor
[...] what() method [...] explicit constructor with one parameter for
each exception member [...] and a parameter of type const char *."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (Exceptions-Schleife)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::every_corpus_case_renders_or_names_its_refusal`

**Status:** partial — eine IDL-Exception wird als `struct` mit ihren
Membern emittiert. Die geforderte Ableitung `class X : public
std::exception`, `what()`, Copy-/Move-Konstruktor, Zuweisungsoperatoren
und der explizite `(member..., const char* what)`-Konstruktor fehlen
vollständig.

### 7.4.2 Interface Forward Declaration

**Spec:** §7.4.2, S. 20 — "An IDL interface forward declaration shall be
mapped to a partial forward declaration in C++."

**Repo:** —

**Tests:** —

**Status:** open — Interface-Vorwärtsdeklarationen werden nicht
emittiert; der Datentyp-Adapter schreibt `Forward` nur für `Structure`,
`Union` und `Valuetype`. `interface Foo;` ohne Definition wird von
`crates/lang-rules/src/adapter/interface.rs` als `ForwardWithoutDefinition`
abgelehnt (Test
`crates/lang-rules/tests/interface_bindings.rs::a_forward_declaration_without_a_definition_is_e002`).

---

## §7.5 Interfaces – Full

### 7.5 Embedded Declarations und static constexpr

**Spec:** §7.5, S. 20 — "adding to mapped class the declaration every
type, exception, or constant declaration in the IDL interface body. [...]
constexpr declarations shall be marked as static."

**Repo:** —

**Tests:** —

**Status:** open — die DDS-RPC-Bindung in
`crates/lang-rules/src/emit/interface_cpp.rs` emittiert Topic-Typen und
Operationsklassen, aber keine verschachtelten Typ-/Exception-/Konstanten-
Deklarationen im Interface-Rumpf und kein `static constexpr` innerhalb
der Klasse.

---

## §7.6 Value Types

### 7.6 Valuetype -> abstrakte Klasse

**Spec:** §7.6, S. 21 — public/private state members -> public/protected
pure-virtual accessor and modifier methods; factory operations ->
`<ValueTypeName>_factory`; inheritance via public virtual; supports
interface -> all operations as pure virtual.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (Valuetype-Zweig)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::every_corpus_case_renders_or_names_its_refusal`

**Status:** partial — `valuetype` wird wie ein `struct` behandelt: seine
Member werden Felder. Die geforderten public/protected pure-virtual
Accessor-/Modifier-Methoden, die `<ValueTypeName>_factory`-Klasse, die
public-virtual Vererbung und die Operationen unterstützter Interfaces
fehlen.

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

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — kein direktes C++-Mapping; der Pfad ist
Intermediate-IDL-Tooling, nicht Codegen.

### 7.11 CCM-Specific

**Spec:** §7.11, S. 22 — verweist auf Annex A.1.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### 7.12/7.13 Components – Ports/Connectors und Template Modules

**Spec:** §7.12-7.13, S. 22 — "mapped to intermediate IDL, as specified
in [OMG-IDL4]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Intermediate-IDL-Pfad ohne direktes
C++-Mapping.

---

## §7.14 Extended Data Types

### 7.14.1 Structures with Single Inheritance

**Spec:** §7.14.1, S. 23 — "shall inherit from the mapped base struct
using public inheritance. [...] swap method [...] which shall ensure that
all inherited members are swapped."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`all_members_of`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::every_corpus_case_renders_or_names_its_refusal`

**Status:** partial — Basis-Member werden flach in den abgeleiteten
Struct kopiert (`all_members_of`), nicht als `struct Child : Base` mit
public inheritance; die `swap`-Funktion für geerbte Member fehlt. Die
flache Form ist bewusst gewählt, damit die Member-Reihenfolge dem
Wire-Plan entspricht.

### 7.14.2 Union Discriminators

**Spec:** §7.14.2, S. 23 — "adds the wchar, and octet IDL types to the set
of valid types for a discriminator. [...] int8 and uint8 are valid union
discriminators."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (Union-Zweig)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`

**Status:** partial — der Discriminator läuft durch `cpp_value_type` und
akzeptiert jeden integralen Typ; assertiert ist nur `switch (long)`.
`wchar`, `octet`, `int8` und `uint8` als Discriminator haben keine eigene
Assertion.

### 7.14.3.1 Maps


**Spec:** §7.14.3.1, S. 23-24 — "IDL maps shall be mapped to a C++
std::map<Key,T> [...] or to a type named omg::types::map<Key,T> [...]
omg::types::bounded_map<Key, T, N>."

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs` (Map-Zweig)

**Tests:** `crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** done — `map<K,V>` ist in der Standardabbildung (`psm`) `std::map<K,V>`; `--cpp-mapping native` und `classic` sind benannte Optionen (`zerodds::Map`, Reihenfolge der Entries als Achse des Codec). `omg::types::map` und `bounded_map` gibt es nicht; die Spec lässt `std::map` genügen. Log: `.agent/REPORT.md`.

### 7.14.3.1 Map Type Traits

**Spec:** §7.14.3.1 Tab.7.10/7.11, S. 24 — "is_bounded [...] bound [...]
key [...] elements."

**Repo:** —

**Tests:** —

**Status:** open — kein `omg::types::map`/`bounded_map` und keine der
vier Traits.

### 7.14.3.2 Bitsets

**Spec:** §7.14.3.2, S. 24-25 — "IDL bitset types shall be mapped to C++
structs that meet the C++ requirements for aggregate initialization. The
only members of these structs are bit fields. [...] shall derive (using
public inheritance) from the struct resulting from mapping the base
type."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (Bitset-Zweig)

**Tests:** `crates/lang-rules/tests/cpp_reader_generates.rs::every_kind_table_9_answers_for_gets_a_defaulter`

**Status:** partial — Bitsets werden als Struct mit getrennten
Integer-Membern emittiert und im Codec gepackt, nicht als C++-Bitfields;
Basis-Bitfields werden flach einbezogen, nicht per public inheritance. Die
Spec verlangt Bitfields als einzige Member.

### 7.14.3.3 Bitmask Type

**Spec:** §7.14.3.3, S. 25-26 — "struct type containing [...] unscoped
enum named _flags [...] private member named _value [...] default
constructor and a copy constructor [...] !=, &=, and ^= bitwise operators
[...] function call operator that returns _value."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (Bitmask-Zweig)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::every_corpus_case_renders_or_names_its_refusal`

**Status:** partial — eine Bitmask wird als Integer-Alias
(uint8/16/32/64) plus `inline constexpr`-Bit-Konstanten emittiert, nicht
als `struct` mit unscoped `_flags`-Enum, privatem `_value`, Konstruktoren,
`!=`/`&=`/`^=`-Operatoren und Aufrufoperator. Die
`bit_bound`/`underlying_type`-Traits fehlen.

### 7.14.4 8-bit Integer Types (Tab.7.13)

**Spec:** §7.14.4 Tab.7.13, S. 26 — "int8 -> int8_t; uint8 -> uint8_t."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`cpp_primitive`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::the_tag_keeps_apart_what_the_cpp_spelling_collapses`

**Status:** partial — `octet`/`uint8` fallen auf denselben
`std::uint8_t` (Tag-Test). `int8 -> std::int8_t` hat keine eigene
Assertion.

### 7.14.5 Explicitly-Named Integer Types (Tab.7.14)

**Spec:** §7.14.5 Tab.7.14, S. 26-27 — "int16/uint16/int32/uint32/int64/uint64
-> int16_t/uint16_t/int32_t/uint32_t/int64_t/uint64_t."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`cpp_primitive`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::the_tag_keeps_apart_what_the_cpp_spelling_collapses`

**Status:** partial — die expliziten Breiten sind in `cpp_primitive`
abgebildet, aber keine Assertion nennt `int16`/`uint16`/`int32`/`uint32`/
`int64`/`uint64` einzeln.

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

**Repo:** —

**Tests:** —

**Status:** `open` — keine Abbildung schreibt `std::optional<T>`: die Standardabbildung (`psm`) schreibt `dds::core::optional<T>` (DDS-PSM-Cxx), die Option `native` einen Zeiger `T *` (DDS-XTypes 1.3 §7.5.1.2.4.2), die Option `classic` `zerodds::classic::Optional<T>`. Die Form von IDL4 to C++ 1.0 §7.17.1 fehlt als benannte Option. Belege der vorhandenen Formen: `crates/lang-rules/src/adapter/cpp_psm.rs` (`dds::core::optional<T>`), `crates/lang-rules/src/adapter/cpp.rs` (`T *`), `crates/lang-rules/src/adapter/cpp_classic.rs` (`zerodds::classic::Optional<T>`); Tests `crates/lang-rules/src/adapter/cpp.rs::tests::an_optional_member_is_a_plain_pointer_and_not_a_second_field`, `crates/cpp/tests/generated_psm_roundtrip.cpp`.

### 7.17.1 General Purpose: @value, @position, @id, @autoid, @extensibility

**Spec:** §7.17.1 Tab.7.15, S. 27-28 — `@value` liefert den
Enumeratorwert; `@position` wirkt auf die Bitmask; `@id`, `@autoid`,
`@extensibility`, `@final`, `@mutable`, `@appendable` sind ohne Impact.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::an_enumeration_takes_an_underlying_type_from_its_bit_bound`

**Status:** partial — `@value` erreicht den Enumeratorwert (der Adapter
schreibt jeden Wert aus), ist aber nicht direkt assertiert; `@position`
hängt an der Bitmask, die selbst partial ist (§7.14.3.3); `@id`,
`@autoid`, `@extensibility`, `@final`, `@mutable` und `@appendable` werden
bestimmungsgemäß nicht ins C++ übertragen.

### 7.17.2 Data Modeling: @key, @must_understand, @default_literal

**Spec:** §7.17.2 Tab.7.16, S. 28 — `@key`/`@must_understand` ohne
Impact; "The C++ element declared as result of the mappings [...]
shall be initialized to the element indicated by [@default_literal]."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`member_default`)

**Tests:** `tools/idlc/tests/cpp_headers_together.rs::two_generated_headers_share_a_translation_unit_and_their_structs_start_at_their_defaults`

**Status:** partial — `@key` und `@must_understand` sind laut Spec ohne
Impact. `@default_literal` wird nicht ausgewertet: der Member-Default
nimmt die **erste** Enumeratorvariante, nicht die mit `@default_literal`
markierte.

### 7.17.3 Units and Ranges: @default, @range, @min, @max, @unit

**Spec:** §7.17.3 Tab.7.17, S. 28-29 — `@range` -> `omg::types::ranged<T,
min, max>` mit `std::out_of_range`; `@min`/`@max` -> Setter werfen
`std::out_of_range`; `@unit` ohne Impact; `@default` initialisiert.

**Repo:** —

**Tests:** —

**Status:** open — kein `omg::types::ranged<T,min,max>`, keine Setter
(die Datenmember sind öffentliche Felder) und damit keine
`std::out_of_range`-Prüfung für `@range`/`@min`/`@max`. `@default` wird
im Codec-Defaulter (`crates/lang-rules/src/default_cpp.rs`) ausgewertet,
aber nicht als C++-Member-Initialisierer des generierten Structs.

### 7.17.4 Data Implementation: @bit_bound

**Spec:** §7.17.4 Tab.7.18, S. 29-30 — "int8_t, for bit bound values
between 1 and 8; int16_t [...] int32_t [...] int64_t."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`enum_underlying`,
`cpp_holder`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::an_enumeration_takes_an_underlying_type_from_its_bit_bound`

**Status:** partial — für Enums ist das underlying type assertiert; für
Bitmasken folgt der Adapter derselben Breitenregel, aber die Bitmask-
Abbildung ist selbst partial (§7.14.3.3).

### 7.17.4 Data Implementation: @external

**Spec:** §7.17.4 Tab.7.18, S. 29-30 — "mapped to std::shared_ptr<T> or
to the equivalent omg::types::ref_type<T> [...] Structures containing
members annotated with @external shall make a deep copy of the external
member in their copy constructor."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (`held_externally`,
`shared_ptr`-Zweige)

**Tests:** `crates/lang-rules/tests/external_is_an_indirection.rs::a_recursive_external_member_is_a_shared_pointer_and_not_a_value` — `crates/lang-rules/tests/external_is_an_indirection.rs:118: generated.contains("std::shared_ptr<::m::Node> next;"),`

**Status:** partial — `@external T` wird als `std::shared_ptr<T>`
gehalten (assertiert). Die geforderte tiefe Kopie im Copy-Konstruktor
fehlt: der compiler-generierte Copy-Konstruktor kopiert den `shared_ptr`
flach, das Objekt wird also geteilt.

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

**Repo:** `crates/lang-rules/src/verbatim.rs`,
`crates/lang-rules/src/adapter/cpp.rs` (`weave_verbatim`)

**Tests:** `crates/lang-rules/tests/verbatim_end_to_end.rs::cpp_answers_to_its_alias_spellings_case_insensitively` — `crates/lang-rules/tests/verbatim_end_to_end.rs:150: generated.contains("// vb-alias"),`

**Status:** partial — alle sechs Platzierungen (BEGIN_FILE,
BEFORE/BEGIN/END/AFTER_DECLARATION, END_FILE) und die Sprachen `c++`,
`cpp`, `cxx` sowie `*` sind live (assertiert). `cc` — von Tab.7.19
ausdrücklich genannt — fehlt in `verbatim::aliases::CPP`
(`&["c++", "cpp", "cxx"]`).

### 7.17.6 Interfaces: @service, @oneway, @ami

**Spec:** §7.17.6 Tab.7.20, S. 30 — "Options are 'CORBA', 'DDS', '*'.
Impact is platform-specific."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_asynchronous_operations_of_rule_seven`

**Status:** partial — `@oneway` (keine Reply-Topic, `future<void>`-Async)
und `@service` (Service-Name) werden über die DDS-RPC-Bindung behandelt;
`@ami` hat keinen eigenen Pfad. Tab.7.20 erklärt den Impact ausdrücklich
für plattformspezifisch.

---

## §8 IDL to C++ Language Mapping Annotations

### 8.1 @cpp_mapping Annotation

**Spec:** §8.1, S. 31 — "enum StructMapping { STRUCT_WITH_PUBLIC_MEMBERS,
CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS }; StructMapping struct_mapping
default STRUCT_WITH_PUBLIC_MEMBERS."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Annotation wird nicht als
IDL-Deklaration emittiert; das Default-Verhalten der C++-Spalte
(Strukturen mit öffentlichen Membern) entspricht
`STRUCT_WITH_PUBLIC_MEMBERS`.

### 8.1.1 CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS

**Spec:** §8.1.1, S. 31-32 — "mapping the annotated IDL struct to C++
class where all members are only accessible via public accessor methods,
and may only be modified through public modifier methods."

**Repo:** —

**Tests:** —

**Status:** open — kein Pfad emittiert die annotierte Struct als Klasse
mit privaten Membern, Accessoren/Modifikatoren und initialisierendem
Konstruktor. `@cpp_mapping` wird von
`crates/lang-rules/src/adapter/cpp.rs` nicht gelesen.

---

## Annex A: Platform-Specific Mappings

### A.1 CORBA-Specific Mappings

**Spec:** Annex A.1, S. 33-66 — `CORBA::traits<>` (`value_type`,
`in_type`, `out_type`, `inout_type`, Tabellen A.1-A.15),
`CORBA::servant_traits<>`, TypeCode, ORB, Object, Any, Value Types,
Abstract Interfaces, Server Side Mapping, DSI, PortableServer.

**Repo:** —

**Tests:** —

**Status:** open — kein `CORBA::traits<>`/`CORBA::servant_traits<>` in
der C++-Spalte und kein `omg::types`. `crates/corba-codegen/src/traits.rs`
ist ein Rust-CORBA-Generator und nicht diese C++-Bindung.

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

**Status:** `n/a (informative)`

---

## Annex C: Compatibility Rules for C++98 and C++03

**Spec:** Annex C (normative), S. 69-77 — alternative Mappings für C++98
und C++03.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die C++-Spalte zielt auf C++17/20
(§7.1.3); der C++98/03-Pfad ist optional und wird nicht implementiert.

---

## Annex D: IDL4 Mapping Rules for Classic C++ Mapping Specifications

**Spec:** Annex D, S. 79-89 — Mappings für die klassischen C++- und
C++11-Specs.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — der Baum nutzt seine eigene Bindung,
nicht die klassischen Mapping-Specs.

---

## Audit-Status

9 done / 24 partial / 15 open / 19 n/a (informative) / 0 n/a (rejected).

Offene Punkte: `idl4-cpp-1.0.open.md`.

Test-Lauf (nicht in diesem Durchgang ausgeführt, da nur Dokumentation
geändert wird): Das Korpus-Gate ist
`cargo test -p zerodds-lang-rules --test cpp_corpus_compiles -- --include-ignored`;
es ist `#[ignore]`, weil es einen C++-Compiler je Korpusfall aufruft.
