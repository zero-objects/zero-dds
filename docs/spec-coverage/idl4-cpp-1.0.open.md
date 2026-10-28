# idl4-cpp-1.0 — offene Punkte

Aggregat aus `idl4-cpp-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## 2.1 Implementation: IDL -> C++-Quelltext nach Kapitel 7

**Status:** `partial` — die Datentyp-Transformation ist live und rendert jeden Korpusfall (bzw. benennt die Verweigerung); das Compiler-Gate prüft die erzeugten Header. Kapitel 7 ist aber nicht vollständig erfüllt: die `omg::types`-Laufzeit (Type Traits, `Any`, `fixed`, `sequence`/`string`- Aliase) fehlt, §7.4-Interfaces laufen über die DDS-RPC-Bindung statt über die generische Form, §7.4.1-Exceptions und §7.6-Valuetypes sind anders geformt.

## 7.1.2 Reserved Names

**Status:** `partial` — Kollisionen mit C++-Keywords werden erkannt und umgeschrieben, aber mit einem **angehängten** Unterstrich (`class` -> `class_`), nicht mit dem von §7.1.2 mandatierten **vorangestellten** Unterstrich (`_class`). Die Erkennung ist vollständig; das angewandte Mapping weicht ab.

## 7.1.4 IDL Type Traits (Tab.7.1 und Tab.7.4-7.12)

**Status:** `open` — kein `omg::types`-Namespace. `grep -rl "omg::types" crates/ tools/ runtime/ endpoints/` findet nur `crates/corba-codegen/src/traits.rs` (ein Rust-CORBA-Generator). Die C++-Spalte schreibt `std::`-Typen direkt; kein Trait der Tabellen 7.1 und 7.4-7.12 wird emittiert.

## 7.2.3 Constants

**Status:** `partial` — numerische und boolesche Konstanten werden als `inline constexpr` emittiert. String-, Wstring- und Fixed-Konstanten werden als `const char *` ausgegeben, nicht als `constexpr omg::types::string_view` oder `std::string_view`; das `L`-Präfix für Wide- Literale fehlt, weil `wstring` als UTF-8 gehalten wird. Keine Assertion deckt die Konstantenform direkt.

## 7.2.4.1 Basic Types (Tab.7.2/7.3)

**Status:** `partial` — `long -> std::int32_t` ist assertiert (Array-, Interface- und Verbatim-Tests), und `octet`/`uint8` sowie `wchar`/ `unsigned short` fallen auf denselben Typ (Tag-Test). Die übrigen Tabellenbreiten (`short -> int16_t`, `long long -> int64_t`, `char`, `wchar`, `boolean`, `octet`, `float`) haben keine eigene Assertion. `long double` wird bewusst auf `zerodds::wire::Float128` abgebildet statt auf das plattformabhängige C++ `long double` (§7.2.4.1.2). `cpp_primitive` ist erschöpfend, ein fehlender Grundtyp ist ein Compile-Fehler.

## 7.2.4.2.1 omg::types::sequence/bounded_sequence

**Status:** `open` — weder `omg::types::sequence<T>` noch `omg::types::bounded_sequence<T,N>` noch die `is_bounded`/`bound`- Spezialisierungen existieren.

## 7.2.4.2.2 Strings -> std::string

**Status:** `partial` — `string` wird als `std::string` gehalten (zugelassene Form). Für den Data-Member existiert keine direkte Assertion (nur der Tag-Test zeigt, dass `string` und `wstring` auf denselben C++-Typ fallen); `omg::types::string` fehlt.

## 7.2.4.2.2 omg::types::string/bounded_string

**Status:** `open` — keine `omg::types::string`/`bounded_string<N>`-Aliase, keine `is_bounded`/`bound`-Traits.

## 7.2.4.2.3 Wstrings

**Status:** `partial` — `wstring` wird als `std::string` mit UTF-8 gehalten, nicht als `std::wstring` und nicht als `omg::types::wstring`; die Spec- Form ist damit nicht erfüllt. Keine direkte Assertion.

## 7.2.4.2.3 omg::types::wstring/bounded_wstring

**Status:** `open` — kein `omg::types::wstring`/`bounded_wstring<N>`, keine Traits.

## 7.2.4.2.4 Fixed Type

**Status:** `open` — die Datentyp-Spalte bildet `fixed<P,S>` in `crates/lang-rules/src/adapter/cpp.rs` auf `std::string` ab. Die handgeschriebene `dds::core::Fixed<P,S>` in `crates/cpp/include/dds/core/Fixed.hpp` ist die DDS-PSM-Cxx-Laufzeit (eine andere Spec) und bietet nur BCD-Round-Trip plus String-Konvertierung, nicht die ~30 Spec-Methoden; `omg::types::fixed` und die `digits`/`scale`-Traits fehlen.

## 7.2.4.3.2 Unions

**Status:** `partial` — die Union wird als Klasse mit `_d`-Feld und einem Feld pro Zweig emittiert (assertiert: `struct U {`, `std::int32_t _d;`). Die geforderten Zugriffs-/Modifier-Methoden, die Multi-Label-Variante und `_default()` fehlen; die Zweige sind öffentliche Felder.

## 7.2.4.3.3 Enumeration Type Traits

**Status:** `open` — die `bit_bound`/`underlying_type`-Spezialisierungen werden nicht emittiert (siehe §7.1.4).

## 7.2.4.3.4 Constructed Recursive Types

**Status:** `partial` — Forward-Declarations für Aggregate und `std::vector<T>`/`std::shared_ptr<T>` erlauben Selbstreferenz; belegt ist nur der `@external`-Rekursivfall (`std::shared_ptr<::m::Node> next;`). Der allgemeine Fall (Selbstreferenz über `sequence<Node>` ohne `@external`) hat keine eigene Assertion.

## 7.2.4.4 omg::types::array und dimensions

**Status:** `open` — `omg::types::array` und der `dimensions`-Trait fehlen.

## 7.2.4.6 Naming Data Types

**Status:** `partial` — `typedef` wird als `cpp::Alias` (C++ `using`) emittiert; keine Assertion prüft die Alias-Deklaration direkt, und der Korpus-Compile unterscheidet eine Alias- nicht von einer Makro-Form.

## 7.3 Any -> omg::types::Any

**Status:** `open` — `any` wird in `crates/lang-rules/src/adapter/cpp.rs` (`cpp_value_type`, `TypeKind::Any`) mit `NoLanguageForm` abgelehnt (Test `an_any_member_is_refused_by_name_at_any_depth`, auch für `sequence<any>`). Kein `omg::types::Any`.

## 7.4 Interfaces -> C++-Klassen

**Status:** `partial` — Interfaces werden über die DDS-RPC-1.0-§7.11.1-Bindung als Klassen mit pure-virtual Operationen und Attribut-Accessoren emittiert (assertiert: `in` by value/const ref, `out`/`inout` by reference, Accessor nach Attributnamen). Die Bindung folgt DDS-RPC 1.0 und fügt `IAsync`/`Requester`/`Replier`/`Client`/ `Service` hinzu, die §7.4 nicht kennt; `omg::types::ref_type<T>`/ `weak_ref_type<T>` für Interface-Parameter fehlen.

## 7.4.1 Exceptions

**Status:** `partial` — eine IDL-Exception wird als `struct` mit ihren Membern emittiert. Die geforderte Ableitung `class X : public std::exception`, `what()`, Copy-/Move-Konstruktor, Zuweisungsoperatoren und der explizite `(member..., const char* what)`-Konstruktor fehlen vollständig.

## 7.4.2 Interface Forward Declaration

**Status:** `open` — Interface-Vorwärtsdeklarationen werden nicht emittiert; der Datentyp-Adapter schreibt `Forward` nur für `Structure`, `Union` und `Valuetype`. `interface Foo;` ohne Definition wird von `crates/lang-rules/src/adapter/interface.rs` als `ForwardWithoutDefinition` abgelehnt (Test `crates/lang-rules/tests/interface_bindings.rs::a_forward_declaration_without_a_definition_is_e002`).

## 7.5 Embedded Declarations und static constexpr

**Status:** `open` — die DDS-RPC-Bindung in `crates/lang-rules/src/emit/interface_cpp.rs` emittiert Topic-Typen und Operationsklassen, aber keine verschachtelten Typ-/Exception-/Konstanten- Deklarationen im Interface-Rumpf und kein `static constexpr` innerhalb der Klasse.

## 7.6 Valuetype -> abstrakte Klasse

**Status:** `partial` — `valuetype` wird wie ein `struct` behandelt: seine Member werden Felder. Die geforderten public/protected pure-virtual Accessor-/Modifier-Methoden, die `<ValueTypeName>_factory`-Klasse, die public-virtual Vererbung und die Operationen unterstützter Interfaces fehlen.

## 7.14.1 Structures with Single Inheritance

**Status:** `partial` — Basis-Member werden flach in den abgeleiteten Struct kopiert (`all_members_of`), nicht als `struct Child : Base` mit public inheritance; die `swap`-Funktion für geerbte Member fehlt. Die flache Form ist bewusst gewählt, damit die Member-Reihenfolge dem Wire-Plan entspricht.

## 7.14.2 Union Discriminators

**Status:** `partial` — der Discriminator läuft durch `cpp_value_type` und akzeptiert jeden integralen Typ; assertiert ist nur `switch (long)`. `wchar`, `octet`, `int8` und `uint8` als Discriminator haben keine eigene Assertion.

## 7.14.3.1 Map Type Traits

**Status:** `open` — kein `omg::types::map`/`bounded_map` und keine der vier Traits.

## 7.14.3.2 Bitsets

**Status:** `partial` — Bitsets werden als Struct mit getrennten Integer-Membern emittiert und im Codec gepackt, nicht als C++-Bitfields; Basis-Bitfields werden flach einbezogen, nicht per public inheritance. Die Spec verlangt Bitfields als einzige Member.

## 7.14.3.3 Bitmask Type

**Status:** `partial` — eine Bitmask wird als Integer-Alias (uint8/16/32/64) plus `inline constexpr`-Bit-Konstanten emittiert, nicht als `struct` mit unscoped `_flags`-Enum, privatem `_value`, Konstruktoren, `!=`/`&=`/`^=`-Operatoren und Aufrufoperator. Die `bit_bound`/`underlying_type`-Traits fehlen.

## 7.14.4 8-bit Integer Types (Tab.7.13)

**Status:** `partial` — `octet`/`uint8` fallen auf denselben `std::uint8_t` (Tag-Test). `int8 -> std::int8_t` hat keine eigene Assertion.

## 7.14.5 Explicitly-Named Integer Types (Tab.7.14)

**Status:** `partial` — die expliziten Breiten sind in `cpp_primitive` abgebildet, aber keine Assertion nennt `int16`/`uint16`/`int32`/`uint32`/ `int64`/`uint64` einzeln.

## 7.17.1 General Purpose: @optional

**Status:** `open` — keine Abbildung schreibt `std::optional<T>`: die Standardabbildung (`psm`) schreibt `dds::core::optional<T>` (DDS-PSM-Cxx), die Option `native` einen Zeiger `T *` (DDS-XTypes 1.3 §7.5.1.2.4.2), die Option `classic` `zerodds::classic::Optional<T>`. Die Form von IDL4 to C++ 1.0 §7.17.1 fehlt als benannte Option. Belege der vorhandenen Formen: `crates/lang-rules/src/adapter/cpp_psm.rs` (`dds::core::optional<T>`), `crates/lang-rules/src/adapter/cpp.rs` (`T *`), `crates/lang-rules/src/adapter/cpp_classic.rs` (`zerodds::classic::Optional<T>`); Tests `crates/lang-rules/src/adapter/cpp.rs::tests::an_optional_member_is_a_plain_pointer_and_not_a_second_field`, `crates/cpp/tests/generated_psm_roundtrip.cpp`.

## 7.17.1 General Purpose: @value, @position, @id, @autoid, @extensibility

**Status:** `partial` — `@value` erreicht den Enumeratorwert (der Adapter schreibt jeden Wert aus), ist aber nicht direkt assertiert; `@position` hängt an der Bitmask, die selbst partial ist (§7.14.3.3); `@id`, `@autoid`, `@extensibility`, `@final`, `@mutable` und `@appendable` werden bestimmungsgemäß nicht ins C++ übertragen.

## 7.17.2 Data Modeling: @key, @must_understand, @default_literal

**Status:** `partial` — `@key` und `@must_understand` sind laut Spec ohne Impact. `@default_literal` wird nicht ausgewertet: der Member-Default nimmt die **erste** Enumeratorvariante, nicht die mit `@default_literal` markierte.

## 7.17.3 Units and Ranges: @default, @range, @min, @max, @unit

**Status:** `open` — kein `omg::types::ranged<T,min,max>`, keine Setter (die Datenmember sind öffentliche Felder) und damit keine `std::out_of_range`-Prüfung für `@range`/`@min`/`@max`. `@default` wird im Codec-Defaulter (`crates/lang-rules/src/default_cpp.rs`) ausgewertet, aber nicht als C++-Member-Initialisierer des generierten Structs.

## 7.17.4 Data Implementation: @bit_bound

**Status:** `partial` — für Enums ist das underlying type assertiert; für Bitmasken folgt der Adapter derselben Breitenregel, aber die Bitmask- Abbildung ist selbst partial (§7.14.3.3).

## 7.17.4 Data Implementation: @external

**Status:** `partial` — `@external T` wird als `std::shared_ptr<T>` gehalten (assertiert). Die geforderte tiefe Kopie im Copy-Konstruktor fehlt: der compiler-generierte Copy-Konstruktor kopiert den `shared_ptr` flach, das Objekt wird also geteilt.

## 7.17.5 Code Generation: @verbatim

**Status:** `partial` — alle sechs Platzierungen (BEGIN_FILE, BEFORE/BEGIN/END/AFTER_DECLARATION, END_FILE) und die Sprachen `c++`, `cpp`, `cxx` sowie `*` sind live (assertiert). `cc` — von Tab.7.19 ausdrücklich genannt — fehlt in `verbatim::aliases::CPP` (`&["c++", "cpp", "cxx"]`).

## 7.17.6 Interfaces: @service, @oneway, @ami

**Status:** `partial` — `@oneway` (keine Reply-Topic, `future<void>`-Async) und `@service` (Service-Name) werden über die DDS-RPC-Bindung behandelt; `@ami` hat keinen eigenen Pfad. Tab.7.20 erklärt den Impact ausdrücklich für plattformspezifisch.

## 8.1.1 CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS

**Status:** `open` — kein Pfad emittiert die annotierte Struct als Klasse mit privaten Membern, Accessoren/Modifikatoren und initialisierendem Konstruktor. `@cpp_mapping` wird von `crates/lang-rules/src/adapter/cpp.rs` nicht gelesen.

## A.1 CORBA-Specific Mappings

**Status:** `open` — kein `CORBA::traits<>`/`CORBA::servant_traits<>` in der C++-Spalte und kein `omg::types`. `crates/corba-codegen/src/traits.rs` ist ein Rust-CORBA-Generator und nicht diese C++-Bindung.
