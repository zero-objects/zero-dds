# IDL4 to C# Language Mapping 1.0 — Spec-Coverage

**Spec:** [OMG IDL4-CSHARP 1.0](https://www.omg.org/spec/IDL4-CSHARP/1.0/PDF) (61 Seiten, OMG formal/2021-07-01)

Item-für-Item-Audit gegen die Spec; jede Anforderung mit Spec-Zitat,
Repo-Pfad, Test-Pfad und Status (`done` / `partial` / `open` /
`n/a`).

**Kontext:** Die Abbildung ist `crates/lang-rules/src/emit/idl4_csharp/`,
ausgewählt mit `zerodds-idlc generate --lang csharp --csharp-mapping idl4`
(`--csharp-naming idl|dotnet`, `--csharp-platform dds|corba`,
`--csharp-corba-module`). Komponenten, Homes, Ports, Konnektoren und
Template-Modul-Instanzen gehen über ihr intermediate IDL
(`crates/lang-rules/src/emit/intermediate_idl.rs`, `crates/ccm`). Die
Laufzeit ist `crates/idl-csharp/runtime/Omg.Types.cs` (`ISequence<T>` mit
den Methoden der Klausel, `SequenceList<T>`, `BoundedList<T>`,
`BoundedDictionary`, `Any`, die Attribute) und für die CORBA-Plattform
`crates/idl-csharp/runtime/Corba.cs`; das Pseudo-IDL des CORBA-Moduls ist
`runtime/corba/CORBA_PIDL.idl`. Jeder Test übersetzt das erzeugte C# mit
Roslyn (`-warnaserror+ -warn:4`) zusammen mit der Laufzeit und führt ein
Prüfprogramm aus, dessen Erwartungen der Spec-Text sind.

Der Standard von `--lang csharp` bleibt die XCDR2-Bindung mit Codec
(`crates/lang-rules/src/adapter/csharp.rs`); sie folgt an mehreren
Stellen nicht dieser Spec und ist hier nicht Gegenstand.

---

## §1 Scope

### 1.1 Abbildung IDL v4 -> C# (ECMA-334)

**Spec:** §1, S. 1 — "This specification defines the mapping of OMG
Interface Definition Language v4 to the C# programming language
[ECMA-334]. The language mapping covers all of the IDL constructs in
the current Interface Definition Language specification [OMG-IDL4]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Scope-Klausel benennt den
Gegenstand der Spec, nicht eine Anforderung an eine Implementierung.

---

## §2 Conformance Criteria

### 2.1 Implementierung: IDL -> C# Source per Kapitel 7

**Spec:** §2, S. 1 — "A conformant implementation shall transform IDL
input into C# source code output as specified in Chapter 7."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/mod.rs::generate`, `tools/idlc/src/main.rs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_6_a_value_type_is_an_abstract_class`, `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_7_9_to_7_13_components_homes_connectors_and_template_instances`, `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_a_1_typecode_object_any_and_user_exception_are_the_annexs_classes`, `tools/idlc/src/main.rs::idl4csharp_7_1_1_the_idl4_mapping_its_naming_scheme_and_platform_are_selectable`

**Status:** `done` — `zerodds-idlc generate --lang csharp --csharp-mapping idl4` bildet IDL nach Kapitel 7 und 8 auf C# ab; jeder Test übersetzt das erzeugte C# mit Roslyn (`-warnaserror+ -warn:4`) zusammen mit `Omg.Types` und führt ein Prüfprogramm aus. Der Standard von `--lang csharp` bleibt die XCDR2-Bindung (Entscheidung, siehe Kontext).

### 2.2 Nutzer: portable Anwendungsquelltexte

**Spec:** §2, S. 1 — "Conformant application source code, as a result,
will be portable across implementations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aussage über die Konformität von
Anwendungscode, keine Implementierungsanforderung.

---

## §3 Normative References

### 3.1 Normative Referenzen (CORBA-IFC, ECMA-334, OMG-IDL4, .NET-GUIDE, .NET-STD)

**Spec:** §3, S. 1 — Liste der normativen Dokumente.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Referenzliste; sie stellt selbst keine
Anforderung an eine Implementierung.

---

## §4 Terms and Definitions

### 4.1 Begriffe (Building Block, C#, Camel Case, Language Mapping, Pascal Case)

**Spec:** §4, S. 2 — Definitionen; "Camel Case" ist Lower Camel Case,
"Pascal Case" ist Upper Camel Case.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar; die Namensregeln selbst
werden in §7.1.1.2.1/§7.1.1.2.2 auditiert.

---

## §5 Symbols

### 5.1 Akronyme (Tab.5.1)

**Spec:** §5, Tab.5.1, S. 2 — Akronym-Liste.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Liste; keine Implementierungsanforderung.

---

## §6 Additional Information

### 6.1/6.2 Keine Änderungen an OMG-Specs; Danksagungen

**Spec:** §6.1/§6.2, S. 3.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hintergrund; keine Implementierungsanforderung.

---

## §7.1 General

### 7.1.1 Zwei Namensschemata, selektiert über @csharp_mapping

**Spec:** §7.1.1, S. 5 — "This specification defines two naming
schemes [...] IDL Naming Scheme [...] .NET Framework Design Guidelines
Naming Scheme. The @csharp_mapping annotation defined in Clause 8.1
provides a mechanism to select the appropriate naming scheme."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/names.rs::Naming`, `crates/lang-rules/src/emit/idl4_csharp/mod.rs::Settings::apply`, `crates/idl-lower/src/annotation.rs::mapping_annotation`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_8_1_csharp_mapping_sets_the_scheme_and_the_struct_type`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`, `crates/idl-lower/tests/mapping_annotations.rs::idl4csharp_8_1_and_idl4cpp_8_1_their_annotations_reach_the_model`, `tools/idlc/src/main.rs::idl4csharp_7_1_1_the_idl4_mapping_its_naming_scheme_and_platform_are_selectable`

**Status:** `done` — die beiden Schemata sind `Naming::Idl` und `Naming::DotNet`; `--csharp-naming` wählt das Schema des Laufs, `@csharp_mapping(apply_naming_convention = ...)` überschreibt es je Modul und Deklaration.

### 7.1.1 Kollisionsauflösung: `@`-Präfix bei Keyword, `_`-Präfix bei Spec-Namen

**Spec:** §7.1.1, S. 5 — "the collision shall be resolved by prepending
the '@' character [...] when the name collides with a C# language
keyword, or the '_' character when the name collides with a name
introduced by this specification."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/names.rs::escape`, `crates/lang-rules/src/emit/idl4_csharp/names.rs::escape_type`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_1_2_reserved_names_take_their_prefix`, `crates/lang-rules/src/emit/idl4_csharp/names.rs::reserved_names_take_their_prefix`

**Status:** `done` — ein C#-Keyword bekommt `@` (`@class`, `@object`), der von der Spec eingeführte Name `Constants` ein `_`.

### 7.1.1.1 IDL Naming Scheme: Namen ohne Case-Transformation

**Spec:** §7.1.1.1, S. 5 — "IDL member names and type identifiers shall
map to C# names and identifiers without case transformation."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/names.rs::Naming`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`

**Status:** `done` — unter `Naming::Idl` bleiben die Namen unverändert, Interfaces bekommen das `I` aus Tabelle 8.1.

### 7.1.1.2 .NET Framework Design Guidelines Naming Scheme

**Spec:** §7.1.1.2, S. 5 — "IDL member names and type identifiers shall
map to C# names and identifiers that follow the coding guidelines
defined in the Framework Design Guidelines of [.NET-GUIDE]."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/names.rs::Naming`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_16_a_user_annotation_is_an_attribute_class`, `crates/lang-rules/src/emit/idl4_csharp/names.rs::table_8_1_gives_each_construct_its_name`

**Status:** `done` — unter `Naming::DotNet` gilt Tabelle 8.1: Pascal Case für Namespaces, Typen, Member, Methoden und Konstanten, Camel Case für Parameter, `I` vor Interfaces, `Flags` nach Bitmasks, `Attribute` nach Annotationen.

### 7.1.1.2.1 Pascal-Case-Transformation

**Spec:** §7.1.1.2.1, S. 5 — "The first letter after each underscore
shall be capitalized and all underscores shall be removed. The first
letter of the IDL name shall be capitalized."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::pascal_case`

**Tests:** `crates/lang-rules/src/emit/idl4_java/names.rs::pascal_case_is_the_clause_examples`, `crates/lang-rules/src/emit/idl4_csharp/names.rs::table_8_1_gives_each_construct_its_name`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`

**Status:** `done` — die Regel und die Beispiele der Klausel, die mit denen von IDL4 to Java §7.1.1.2.1 übereinstimmen.

### 7.1.1.2.2 Camel-Case-Transformation

**Spec:** §7.1.1.2.2, S. 6 — "The first letter after each underscore
shall be capitalized and all underscores shall be removed. The first
letter of the IDL name shall be lower case."

**Repo:** `crates/lang-rules/src/emit/idl4_java/names.rs::camel_case`

**Tests:** `crates/lang-rules/src/emit/idl4_java/names.rs::camel_case_is_the_clause_examples`, `crates/lang-rules/src/emit/idl4_csharp/names.rs::table_8_1_gives_each_construct_its_name`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`

**Status:** `done` — die Regel und die Beispiele der Klausel; Parameter unter dem .NET-Schema in Camel Case (`ioParam`).

### 7.1.2 Reserved Names (C#-Keywords + `Constants`)

**Spec:** §7.1.2, S. 5 — reserviert sind die C#-Keywords aus ECMA-334
§7.4.4 und der Klassenname `Constants` pro Modul-Namespace.

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/names.rs::CSHARP_KEYWORDS`, `crates/lang-rules/src/emit/idl4_csharp/names.rs::CONSTANTS_CLASS`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_1_2_reserved_names_take_their_prefix`, `crates/lang-rules/src/emit/idl4_csharp/names.rs::reserved_names_take_their_prefix`

**Status:** `done` — reserviert sind die 77 Keywords aus ECMA-334 §7.4.4 und `Constants`.

### 7.1.3 Tab.7.1: C#-Sprachversionen pro Feature

**Spec:** §7.1.3, Tab.7.1, S. 6 — Mindestversionen für `IList<T>`,
`IDictionary<TKey,TValue>`, System-Exceptions, `FlagsAttribute`,
`BitArray`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Versionsmatrix; der Generator zielt
auf C# 12/.NET 8, aber die Tabelle stellt keine eigene
Abbildungsanforderung.

### 7.1.4 Mapping Extensibility

**Spec:** §7.1.4, S. 7 — "implementers [...] may extend C# types [...]
to add new constructors and methods, override existing methods, and
implement additional interfaces."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis an Implementierer, keine
Anforderung (der Materialisierer emittiert `sealed`-Klassen).

---

## §7.2 Core Data Types

### 7.2.1 IDL Specification (kein direktes Mapping)

**Spec:** §7.2.1, S. 7 — "There is no direct mapping of the IDL
Specification itself."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Klausel verweist auf die
folgenden Abschnitte.

### 7.2.2 IDL Module -> C# namespace

**Spec:** §7.2.2, S. 7 — "IDL modules shall be mapped to C# namespaces
of the same name. [...] IDL declarations not enclosed in any module
shall be mapped into the global scope."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/mod.rs::namespace_of`, `crates/lang-rules/src/emit/idl4_csharp/mod.rs::Gen::file`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`

**Status:** `done` — ein Modul ist ein Namespace gleichen Namens (`MyMath` unter dem .NET-Schema); Deklarationen außerhalb jedes Moduls stehen im globalen Namespace.

### 7.2.3 Constants: zwei Alternativen

**Spec:** §7.2.3, S. 7 — "two alternatives for mapping IDL constants
[...] Standalone Constants Mapping [...] Constants Container Mapping."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::constant_classes`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`

**Status:** `done` — beide Alternativen: die Standalone-Abbildung (Vorgabe, Entscheidung) und der Container über `@csharp_mapping(constants_container = ...)`.

### 7.2.3.1 Standalone Constants: Klasse pro Konstante mit `Value`

**Spec:** §7.2.3.1, S. 7 — "IDL constants shall be mapped to public
static classes of the same name [...]. The mapped class shall contain a
public const called Value."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::constant_classes`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`

**Status:** `done` — eine `public static class` je Konstante mit `public const ... Value`, das Beispiel `my_math` unter beiden Schemata.

### 7.2.3.2 Constants Container: `public static partial class Constants`

**Spec:** §7.2.3.2, S. 8 — "Every scope containing a constant
declaration shall contain a public static partial class. By default,
the mapped class shall be named Constants."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::constant_classes`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`

**Status:** `done` — `public static partial class` mit dem Namen aus `constants_container`, `MathematicalConstants` im Beispiel; die leere Zeichenkette schaltet zurück auf Standalone.

### 7.2.4.1.1 Integer-Typen (Tab.7.2)

**Spec:** §7.2.4.1.1, Tab.7.2, S. 9 — "int8 -> sbyte; uint8 -> byte;
short/int16 -> short; unsigned short/uint16 -> ushort; long/int32 ->
int; unsigned long/uint32 -> uint; long long/int64 -> long; unsigned
long long/uint64 -> ulong."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::integer`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — Tabelle 7.2 vollständig.

### 7.2.4.1.2 Floating-Point-Typen (Tab.7.3)

**Spec:** §7.2.4.1.2, Tab.7.3, S. 10 — "float -> float; double ->
double; long double -> decimal."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::primitive`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `float`, `double`, `long double` als `decimal`.

### 7.2.4.1.3 IDL char -> C# char

**Spec:** §7.2.4.1.3, S. 10 — "The IDL char type shall be mapped to the
C# type char."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::primitive`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `char` ist `char`; der Setter weist ein Zeichen über U+00FF ab.

### 7.2.4.1.4 IDL wchar -> C# char

**Spec:** §7.2.4.1.4, S. 10 — "The IDL wchar type shall be mapped to
the C# type char."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::primitive`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `wchar` ist `char`.

### 7.2.4.1.5 IDL boolean -> C# bool

**Spec:** §7.2.4.1.5, S. 10 — "The IDL boolean type shall be mapped to
the C# bool, and the IDL constants TRUE and FALSE shall be mapped to
the corresponding C# boolean literals."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::primitive`, `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::cs_value`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `boolean` ist `bool`, `TRUE`/`FALSE` sind `true`/`false`.

### 7.2.4.1.6 IDL octet -> C# byte

**Spec:** §7.2.4.1.6, S. 10 — "The IDL type octet [...] shall be mapped
to the C# type byte."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::integer`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3`

**Status:** `done` — `octet` ist `byte`.

### 7.2.4.2.1 IDL Sequence -> Omg.Types.ISequence<T>

**Spec:** §7.2.4.2.1, S. 10 — "IDL sequences shall be mapped to the C#
Omg.Types.ISequence<T> interface [...]. Implementations of
Omg.Types.ISequence<T> shall extend
System.Collections.Generic.IList<T>."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`, `crates/idl-csharp/runtime/Omg.Types.cs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_2_sequences_strings_and_fixed`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`

**Status:** `done` — eine Sequenz ist `Omg.Types.ISequence<T>` mit allen Methoden, die die Klausel aus `List<T>` aufzählt; die Laufzeit hat `SequenceList<T>` und `BoundedList<T>`.

### 7.2.4.2.1 Tab.7.4: Sequenzen von Basistypen

**Spec:** §7.2.4.2.1, Tab.7.4, S. 11-12 — `sequence<boolean>` ->
`Omg.Types.ISequence<bool>`, ... `sequence<long double>` ->
`Omg.Types.ISequence<decimal>`.

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_2_sequences_strings_and_fixed`

**Status:** `done` — die Elementtypen aus Tabelle 7.4, `ISequence<decimal>` für `long double`.

### 7.2.4.2.1 Bounds-Checking auf bounded Sequences

**Spec:** §7.2.4.2.1, S. 12 — "Bounds checking on bounded sequences may
raise an exception if necessary." (MAY.)

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Spec formuliert eine
Kann-Bestimmung; der Wire-Pfad prüft die Grenze beim Dekodieren
(`runtime/csharp`), die C#-Feldform selbst nicht.

### 7.2.4.2.1 Sequence-/Map-Member als read-only Properties (mit @external)

**Spec:** §7.2.4.2.1, S. 12 — "sequence members of IDL structs and
unions map to read-only properties. [...] sequences and maps that are
marked with the @external annotation shall include both a getter and a
setter."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::member`, `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::write_properties`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_2_a_union_is_a_class_with_a_discriminator_and_member_properties`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_standardized_annotations_shape_the_class`

**Status:** `done` — Sequenz- und Map-Member sind Properties ohne Setter, mit `@external` mit Setter und als `IEnumerable<T>`.

### 7.2.4.2.2 IDL string -> C# string (UTF-16)

**Spec:** §7.2.4.2.2, S. 12 — "IDL strings, both bounded and unbounded
variants, shall be mapped to C# strings. The resulting strings shall be
encoded in UTF-16 format."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_2_sequences_strings_and_fixed`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`

**Status:** `done` — `string` ist `string`; eine Schranke prüft der Setter.

### 7.2.4.2.3 IDL wstring -> C# string (UTF-16)

**Spec:** §7.2.4.2.3, S. 12 — wie §7.2.4.2.2 für `wstring`.

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_2_sequences_strings_and_fixed`

**Status:** `done` — `wstring` ist `string`.

### 7.2.4.2.4 IDL fixed -> C# decimal + ArithmeticException

**Spec:** §7.2.4.2.4, S. 12 — "The IDL fixed type shall be mapped to
the C# decimal type. Range checking shall raise a
System.ArithmeticException exception, or a derived exception, if
necessary."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::checks`, `crates/idl-csharp/runtime/Omg.Types.cs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_2_sequences_strings_and_fixed`

**Status:** `done` — `fixed<P,S>` ist `decimal`; `Omg.Types.Fixed.Check` wirft `System.ArithmeticException` für zu viele Stellen vor oder nach dem Komma.

### 7.2.4.3.1 IDL struct -> C# public class

**Spec:** §7.2.4.3.1, S. 13 — "An IDL struct shall be mapped to a C#
public class [...] A public property [...] A public default
constructor [...] A public copy constructor [...] A public
constructor that accepts parameters for each member [...] The class
shall implement the IEquatable<T> interface."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::structure`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable`

**Status:** `done` — eine `public class` mit Properties, Default-, Kopier- und All-Values-Konstruktor (die Kopie tief) und `IEquatable<T>`, das Beispiel `MyStruct` unter beiden Schemata.

### 7.2.4.3.2 IDL union -> C# public class (Discriminator-Protokoll)

**Spec:** §7.2.4.3.2, S. 14-15 — "A public read-only property named
Discriminator [...] A public property with getters and setters for each
member [...] Property getters shall raise a
System.InvalidOperationException if the expected member has not been
set [...] implement the IEquatable<T> interface."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::union`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_2_a_union_is_a_class_with_a_discriminator_and_member_properties`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_2_int8_uint8_wchar_and_octet_discriminate_a_union`

**Status:** `done` — das Beispiel `AUnion`: `Discriminator` nur lesbar, Getter mit `InvalidOperationException`, Setter auf das erste Label, `Set<Member>(value, discriminator)` mit `ArgumentException`, die beiden `Set<Member>` einer Sequenz, der Default-Member auf dem ersten freien Wert ab null, `IEquatable<T>`. Die Klausel schreibt `System.Collections.IEnumerable<T>`; es ist das generische `System.Collections.Generic.IEnumerable<T>`.

### 7.2.4.3.3 IDL enum -> C# public enum

**Spec:** §7.2.4.3.3, S. 18 — "An IDL enum shall be mapped to a C#
public enum with the same name as the IDL enum type."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::enumeration`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_3_an_enumeration_is_a_csharp_enum_of_its_bit_bound`

**Status:** `done` — ein C#-`enum` mit den `@value`s.

### 7.2.4.3.3 @bit_bound-Underlying für Enums

**Spec:** §7.2.4.3.3, S. 18 — "the corresponding C# enum type shall be
sbyte for bit bound values between 1 and 8; short, for [...] 9 and 16;
int, for [...] 17 and 32; and long, for [...] 33 and 64."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::enumeration`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_3_an_enumeration_is_a_csharp_enum_of_its_bit_bound`

**Status:** `done` — `@bit_bound` wählt `sbyte`, `short`, `int` oder `long`; ohne `@bit_bound` bleibt `int`. Eine Schranke über 32 weist das Front-End im Quelltext nach DDS-XTypes 1.3 §7.3.1.2.1.5 ab („from 1 to 32, inclusive, when this annotation is applied to an enumerated type“); der Test prüft diese Abweisung und erreicht den Bereich `long` über die Schranke 40 im `@bit_bound`-Facet des Modells, das der Generator liest.

### 7.2.4.3.4 Constructed Recursive Types

**Spec:** §7.2.4.3.4, S. 18 — "Constructed recursive types are
supported by mapping the involved types directly to C#."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_4_a_recursive_type_names_its_own_class`

**Status:** `done` — eine Struktur mit einer Sequenz ihrer selbst; die Kopie und der Vergleich laufen durch.

### 7.2.4.4 IDL Array -> C# Array + Bounds-Check

**Spec:** §7.2.4.4, S. 19 — "An IDL array shall be mapped to a C# array
[...]. The bounds for the array shall be checked by the setter of the
corresponding property and a System.ArgumentOutOfRangeException shall
be raised if a bounds violation occurs."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::checks`, `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::array_initial`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_4_an_array_is_a_csharp_array_checked_by_its_setter`

**Status:** `done` — ein C#-Array (rechteckig bei mehreren Dimensionen) in seiner Größe mit konstruierten Elementen; der Setter wirft `ArgumentOutOfRangeException` für eine andere Größe, das Beispiel `MyType`.

### 7.2.4.5 Native Types

**Spec:** §7.2.4.5, S. 21 — "This language mapping specification does
not define any native types, but compliant implementations may provide
the necessary mechanisms to map native types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Spec definiert keine nativen
Typen; der Materialisierer bildet `native` optional auf den Facet-Namen
oder `object` ab (permissiv).

### 7.2.4.6 Naming Data Types (typedef)

**Spec:** §7.2.4.6, S. 21 — "the declaration of types using typedef in
IDL shall not result in the creation of any C# type. Instead, the use
of an IDL typedef type shall be replaced with the type referenced by
the typedef statement."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_6_a_typedef_is_replaced_and_its_annotations_reach_its_uses`

**Status:** `done` — kein C#-Typ, jede Verwendung trägt den Zieltyp.

---

## §7.3 Any

### 7.3 IDL any -> Omg.Types.Any

**Spec:** §7.3, S. 22 — "The IDL any type shall be mapped to
Omg.Types.Any type."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`, `crates/idl-csharp/runtime/Omg.Types.cs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_3_any_is_omg_types_any`

**Status:** `done` — `any` ist `Omg.Types.Any` mit `Insert`, `Extract<T>` und `Type`; auf der CORBA-Plattform `Corba.Any` (A.1.4).

---

## §7.4 Interfaces – Basic

### 7.4 IDL interface -> C# public interface

**Spec:** §7.4, S. 22 — "Each IDL interface shall be mapped to a C#
public interface with the same name [...] prepending the "I" prefix.
[...] If the IDL interface derives from other IDL interfaces, the
equivalent C# interface shall be declared to extend the C# interfaces
[...]. Each attribute [...] shall map to a property [...] Each
operation [...] shall map to a method [...] IDL out arguments shall be
mapped to C# out parameters, inout to ref, and in to parameters without
modifier."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::top_level_interface`, `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::operation_method`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`

**Status:** `done` — das Beispiel `AnInterface`: `IAnInterface`, Basen als Basisinterfaces, Attribute als Properties (ohne Setter für `readonly`), `out` als `out`, `inout` als `ref`.

### 7.4.1 IDL exception -> C# class : System.Exception

**Spec:** §7.4.1, S. 23 — "An IDL exception shall be mapped to a C#
class [...]. The mapped class shall extend the System.Exception class."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::exception`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`, `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_a_1_1_and_a_1_5_exceptions_and_interfaces_on_the_corba_platform`

**Status:** `done` — eine Klasse, die `System.Exception` erweitert, mit Properties und den drei Konstruktoren; eine Property, die `System.Exception` verdecken würde, bekommt `_`.

### 7.4.1 Exception-Klasse erbt von System.Exception

**Spec:** §7.4.1, S. 23 — "The mapped class shall extend the System.Exception class."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::exception`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`, `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_a_1_1_and_a_1_5_exceptions_and_interfaces_on_the_corba_platform`

**Status:** `done` — die Klasse erweitert `System.Exception`, auf der CORBA-Plattform `Corba.UserException`.

### 7.4.2 Interface Forward Declaration

**Spec:** §7.4.2, S. 23 — "An IDL interface forward declaration has no
mapping to the C# language."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::top_level_interface`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out`

**Status:** `done` — eine Vorwärtsdeklaration erzeugt keine Datei; die Definition hat ihre eine.

---

## §7.5 Interfaces – Full

### 7.5 I<Name>Operations + I<Name> + <Name>-Klasse

**Spec:** §7.5, S. 23-24 — "A C# public interface named
I<InterfaceName>Operations [...] A C# public interface named
I<InterfaceName> inheriting from I<InterfaceName>Operations [...] A C#
public class named <InterfaceName> containing the declaration of the
classes and exceptions that result from the mapping of all the types
and exceptions declared within the IDL interface."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::top_level_interface`, `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::embedded_declarations`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_5_a_full_interface_is_operations_interface_and_class`

**Status:** `done` — das Beispiel `FullInterface`: `IFullInterfaceOperations`, `IFullInterface` und die Klasse `FullInterface` mit `S` und `PI`; die Klasse implementiert `IFullInterface` abstrakt (Entscheidung: die Klausel lässt das Erben offen, ein Implementieren ohne Rumpf ist in C# nur abstrakt möglich).

---

## §7.6 Value Types

### 7.6 IDL valuetype -> abstrakte Klasse + Fabrik-Attribut

**Spec:** §7.6, S. 25 — "An IDL valuetype type shall be mapped to a C#
abstract class. [...] private members shall have the C# protected
access modifier [...] Each valuetype factory operation shall be mapped
[...] to a method returning void [...] annotated with the
FactoryAttribute."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::valuetype`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_6_a_value_type_is_an_abstract_class`

**Status:** `done` — die Beispiele `VT1` und `VT2`: abstrakte Klassen, private Member `protected`, Operationen abstrakt, die Factory `void` mit `[Omg.Types.Factory]`, Basis und unterstütztes Interface.

---

## §7.7-§7.13 CORBA, Components, Template Modules

### 7.7/7.8 CORBA-spezifische Interfaces und Value Types

**Spec:** §7.7/§7.8, S. 27 — "CORBA-specific mappings are defined in
Clause A.1 of Annex A."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf Annex A.1; die
CORBA-Abbildung selbst steht unter A.1.

### 7.9-7.13 Components (Basic/Homes/Ports/Connectors) und Template Modules

**Spec:** §7.9-§7.13, S. 27 — "Basic components have no direct language
mapping; they shall be mapped to intermediate IDL [...]. Template
module instances have no direct language mapping."

**Repo:** `crates/lang-rules/src/emit/intermediate_idl.rs::intermediate_idl`, `crates/idl-lower/src/lower.rs::template_module_inst`

**Tests:** `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_7_9_to_7_13_components_homes_connectors_and_template_instances`

**Status:** `done` — Komponenten, Homes, Ports und Konnektoren über das äquivalente IDL aus CCM 4.0 §6, eine Template-Modul-Instanz als Namespace mit den eingesetzten Parametern.

### 7.9-7.12 Komponenten, Homes, Ports und Connectoren als Intermediate-IDL

**Spec:** §7.9–§7.12, S. 27 — "Basic components have no direct language mapping; they shall be mapped to intermediate IDL"; ebenso Homes, Ports und Connectoren.

**Repo:** `crates/lang-rules/src/emit/intermediate_idl.rs::intermediate_idl`, `crates/ccm/src/transform.rs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_7_9_to_7_13_components_homes_connectors_and_template_instances`, `crates/ccm/tests/equivalent_idl.rs::idl_7_4_11_4_2_an_extended_port_is_the_basic_ports_of_its_port_type`, `crates/ccm/tests/equivalent_idl.rs::idl_7_4_11_4_3_a_connector_maps_through_its_ports_as_a_component`

**Status:** `done` — `ICashier : Components.ICCMObject` mit `provide_`, `connect_`, `subscribe_`, `get_consumer_` und den Basis-Ports des Port Types; das Home als Explicit, Implicit und Äquivalent; der Konnektor über seine Ports. Die `Components`-Deklarationen kommen aus dem IDL der Anwendung (Test: Transkription von CCM 4.0 §6).

---

## §7.14 Extended Data Types

### 7.14.1 Struct mit Single Inheritance -> C# extends

**Spec:** §7.14.1, S. 27 — "An IDL struct that inherits from a base IDL
struct, shall be declared as a C# public class that extends the class
resulting from mapping the base IDL struct."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::structure`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_1_a_derived_structure_extends_its_base`

**Status:** `done` — das Beispiel `ChildStruct : MyStruct`: die Klasse erweitert die der Basis, der All-Values-Konstruktor nimmt das Basisobjekt zuerst; der Kopierkonstruktor ruft den der Basis (Entscheidung, gleiche Wirkung wie der All-Values-Konstruktor der Basis).

### 7.14.2 Union-Discriminatoren: int8, uint8, wchar, octet

**Spec:** §7.14.2, S. 28 — "This building block adds the int8, uint8,
wchar, and octet IDL types to the set of valid types for a
discriminator."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::union`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_2_int8_uint8_wchar_and_octet_discriminate_a_union`

**Status:** `done` — `int8`, `uint8`, `wchar` und `octet` diskriminieren wie §7.2.4.3.2.

### 7.14.3.1 IDL map -> IDictionary<TKey,TValue>

**Spec:** §7.14.3.1, S. 28 — "An IDL map shall be mapped to a C#
generic System.Collections.Generic.IDictionary<TKey,TValue>."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`, `crates/idl-csharp/runtime/Omg.Types.cs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_3_1_a_map_is_an_idictionary`

**Status:** `done` — `IDictionary<TKey, TValue>` als Property ohne Setter; eine beschränkte Map ist `Omg.Types.BoundedDictionary` und wirft `ArgumentOutOfRangeException`.

### 7.14.3.2 IDL bitset -> C# struct + IEquatable

**Spec:** §7.14.3.2, S. 29 — "An IDL bitset shall map to a C# struct
with public properties for each named bitfield [...]. The mapped C#
struct shall implement the IEquatable<T> interface."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::bitset`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_3_2_a_bit_set_is_a_struct`

**Status:** `done` — das Beispiel `MyBitset`: ein `struct` mit Properties der Typen der Klausel und `IEquatable<T>`; der Setter weist einen Wert ab, der nicht in die Bits passt.

### 7.14.3.3 IDL bitmask -> C# enum `[Flags]` + BitArray

**Spec:** §7.14.3.3, S. 30 — "The IDL bitmask type shall map to a C#
public enum with the same name, followed by the "Flags" suffix. [...]
a System.Collections.BitArray shall be used. The C# enum shall have the
System.FlagsAttribute [...]. The size [...] determines the
corresponding C# enum type."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::bitmask`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_3_3_a_bit_mask_is_a_flags_enum_and_a_bit_array`

**Status:** `done` — `<Name>Flags` mit `[Flags]`, `1 << position`, dem Typ nach `@bit_bound`, und `System.Collections.BitArray` für einen Member.

---

## §7.15 Anonymous Types

### 7.15 Anonymous Types: kein Impact

**Spec:** §7.15, S. 31 — "No impact to the C# language mapping."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Klausel stellt keine Anforderung.

---

## §7.16 Annotations

### 7.16.1 Definieren von Annotationen -> Attributklasse

**Spec:** §7.16.1, S. 31 — "User-defined annotations are propagated to
the generated code as C# attributes inheriting from the System.Attribute
class. [...] named [...] appending the "Attribute" suffix [...] shall
have a public constructor with default values [...] [AttributeUsage(
AttributeTargets.All, AllowMultiple = true)]."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::annotation_type`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_16_a_user_annotation_is_an_attribute_class`

**Status:** `done` — eine Attributklasse mit `[AttributeUsage(AttributeTargets.All, AllowMultiple = true)]`, Properties, Defaults im Default-Konstruktor; `MyAnnotationAttribute` unter dem .NET-Schema.

### 7.16.2 Anwenden von Annotationen

**Spec:** §7.16.2, S. 32 — "IDL elements annotated with user-defined
annotations shall map to equivalent C# elements annotated with the
corresponding attribute."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::user_annotations`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_16_a_user_annotation_is_an_attribute_class`

**Status:** `done` — ein annotiertes Element trägt das Attribut, zwei Anwendungen zwei Attribute.

### 7.16.2.1 Annotationen auf typedef propagieren

**Spec:** §7.16.2.1, S. 33 — "Annotations on an IDL typedef shall be
applied to uses of the typedef in other type declarations."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/behaviour.rs::Gen::member_annotations`, `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::cs_type_of`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_6_a_typedef_is_replaced_and_its_annotations_reach_its_uses`

**Status:** `done` — `@max(100)` am `typedef` prüft der Setter jeder Verwendung, auch als Sequenzelement; eine Nutzer-Annotation am `typedef` trägt der Member.

---

## §7.17 Standardized Annotations

### 7.17.1 @id, @autoid (kein Impact)

**Spec:** §7.17.1, Tab.7.5, S. 33-35 — "@id No impact on language
mapping; @autoid No impact on language mapping."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::member`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — die Klasse ist mit `@id` und `@autoid` dieselbe wie ohne.

### 7.17.1 @optional -> Nullable<T> + OptionalAttribute

**Spec:** §7.17.1, Tab.7.5, S. 34 — "IDL elements annotated with
@optional whose type T maps to a C# value type shall map to
System.Nullable<T>. [...] All mapped optional elements shall be
annotated with OptionalAttribute."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::member`, `crates/idl-csharp/runtime/Omg.Types.cs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_standardized_annotations_shape_the_class`

**Status:** `done` — `System.Nullable<T>` für einen Werttyp, ein Referenztyp unverändert, `[Omg.Types.Optional]` an der Property und am Parameter des All-Values-Konstruktors.

### 7.17.1 @position, @value

**Spec:** §7.17.1, Tab.7.5, S. 35 — "@position Impacts the mapping of
bitmask types [...]; @value Impacts the mapping of enum types."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::enumeration`, `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::bitmask`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_3_3_an_enumeration_is_a_csharp_enum_of_its_bit_bound`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_3_3_a_bit_mask_is_a_flags_enum_and_a_bit_array`

**Status:** `done` — `@value` setzt Enum-Werte, `@position` Flag-Werte.

### 7.17.1 @extensibility, @final, @mutable, @appendable (kein Impact)

**Spec:** §7.17.1, Tab.7.5, S. 35 — "No impact on language mapping."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::structure`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — die Klasse ist mit jeder Extensibility-Annotation dieselbe wie ohne.

### 7.17.2 @key -> KeyAttribute

**Spec:** §7.17.2, Tab.7.6, S. 35 — "IDL elements annotated with @key
shall result in equivalent C# elements annotated with KeyAttribute."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::member`, `crates/idl-csharp/runtime/Omg.Types.cs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_standardized_annotations_shape_the_class`

**Status:** `done` — `[Omg.Types.Key]` an der Property.

### 7.17.2 @must_understand (kein Impact)

**Spec:** §7.17.2, Tab.7.6, S. 35 — "@must_understand No impact on
language mapping."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::member`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — die Klasse ist mit `@must_understand` dieselbe wie ohne.

### 7.17.2 @default_literal

**Spec:** §7.17.2, Tab.7.6, S. 35 — "The C# element [...] shall be
initialized to element indicated by the annotation."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/types.rs::Gen::default_enumerator`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_standardized_annotations_shape_the_class`

**Status:** `done` — der Default-Konstruktor setzt den Enumerator, den `@default_literal` nennt.

### 7.17.3 Units and Ranges: @default, @range, @min, @max, @unit

**Spec:** §7.17.3, Tab.7.7, S. 37 — "@default [...] initialized to the
value of the annotation; @range/@min/@max [...] shall throw a
System.ArgumentOutOfRangeException [...]; @unit Shall result in the
addition of a UnitAttribute."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::member`, `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::range_checks`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_standardized_annotations_shape_the_class`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_4_6_a_typedef_is_replaced_and_its_annotations_reach_its_uses`

**Status:** `done` — `@default` ist der Wert des Default-Konstruktors, `@range`/`@min`/`@max` werfen im Setter `ArgumentOutOfRangeException`, `@unit` ist `[Omg.Types.Unit("m")]`.

### 7.17.4 @bit_bound

**Spec:** §7.17.4, Tab.7.8, S. 38 — "@bit_bound Impacts the mapping of
bitmask as described in Clause 7.14.3.3."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::bitmask`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_14_3_3_a_bit_mask_is_a_flags_enum_and_a_bit_array`

**Status:** `done` — der Typ des Flags-Enums folgt `@bit_bound`.

### 7.17.4 @external

**Spec:** §7.17.4, Tab.7.8, S. 38 — "@external [...] map to
System.Nullable<T> [...] Properties created as a result of mapping IDL
sequences annotated with @external shall map to the C#
System.Collections.Generic.IEnumerable<T> interface."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::member`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_standardized_annotations_shape_the_class`

**Status:** `done` — `System.Nullable<T>` für einen Werttyp, `IEnumerable<T>` mit Setter für eine Sequenz, Kopien flach.

### 7.17.4 @nested (kein Impact)

**Spec:** §7.17.4, Tab.7.8, S. 38 — "@nested No impact to the language
mapping."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::structure`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_the_annotations_without_impact_leave_the_class_unchanged`

**Status:** `done` — die Klasse ist mit `@nested` dieselbe wie ohne.

### 7.17.5 @verbatim

**Spec:** §7.17.5, Tab.7.9, S. 38 — "Copies verbatim text to the
indicated output position when the indicated language is "*", "c#",
"cs", or "csharp"."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::structure`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_17_5_verbatim_text_is_copied_for_its_four_names`

**Status:** `done` — Text für `*`, `c#`, `cs` und `csharp` steht an seiner Stelle, Text für eine andere Sprache nicht.

### 7.17.6 @service, @oneway, @ami

**Spec:** §7.17.6, Tab.7.10, S. 38 — "Options are "CORBA", "DDS", "*".
Impact is platform-specific."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Spec erklärt den Impact für
plattformspezifisch; der Materialisierer dokumentiert `raises`/`oneway`
nur, ohne C#-Klausel.

---

## §8 IDL to C# Language Mapping Annotations

### 8.1 @csharp_mapping-Annotation

**Spec:** §8.1, S. 41 — "@annotation csharp_mapping { enum
NamingConvention {IDL_NAMING_CONVENTION, DOTNET_NAMING_CONVENTION};
NamingConvention apply_naming_convention; string constants_container
default "Constants"; string struct_type default "class"; }"

**Repo:** `crates/ast/src/facet/mapping.rs`, `crates/idl-lower/src/annotation.rs::mapping_annotation`

**Tests:** `crates/idl-lower/tests/mapping_annotations.rs::idl4csharp_8_1_and_idl4cpp_8_1_their_annotations_reach_the_model`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_8_1_csharp_mapping_sets_the_scheme_and_the_struct_type`

**Status:** `done` — `@csharp_mapping` ist eingebaut, wird gegen die Deklaration der Klausel geprüft (`IDL0007`) und erreicht das Modell als `Facets::mappings`.

### 8.1.1 apply_naming_convention

**Spec:** §8.1.1, S. 41 — "If apply_naming_convention is
IDL_NAMING_CONVENTION [...] If [...] DOTNET_NAMING_CONVENTION [...]."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/names.rs::Naming`, `crates/lang-rules/src/emit/idl4_csharp/mod.rs::Settings::apply`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_8_1_csharp_mapping_sets_the_scheme_and_the_struct_type`, `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`

**Status:** `done` — beide Spalten der Tabelle 8.1; eine Einstellung an einer Deklaration gewinnt über die ihres Moduls.

### 8.1.2 constants_container

**Spec:** §8.1.2, S. 42 — "To enable the Standalone Constants Mapping
[...] constants_container shall be set to an empty string. To enable
the Constants Container Mapping [...] set to a valid string."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::constant_classes`, `crates/lang-rules/src/emit/idl4_csharp/mod.rs::Settings::apply`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container`

**Status:** `done` — ein Name schaltet den Container ein, die leere Zeichenkette die Standalone-Abbildung.

### 8.1.3 struct_type

**Spec:** §8.1.3, S. 43 — "struct_type defines the C# type the IDL
struct type map to. By default [...] C# class. This parameter allows
changing the default behavior to map an IDL struct to a C# struct."

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/data.rs::Gen::structure`, `crates/lang-rules/src/emit/idl4_csharp/mod.rs::Settings::apply`

**Tests:** `crates/lang-rules/tests/csharp_idl4_mapping.rs::idl4csharp_8_1_csharp_mapping_sets_the_scheme_and_the_struct_type`

**Status:** `done` — `struct_type = "struct"` erzeugt ein C#-`struct`, kopiert als Wert.

---

## Annex A: Platform-Specific Mappings

### A.1 CORBA-Specific Mappings (A.1.1-A.1.6)

**Spec:** Annex A.1, S. 45-48 — `Corba.UserException` für Exceptions,
`Corba.TypeCode`, `Corba.Object`/`Corba.IObject`, `Corba.Any`, die
`Corba.IObject`-Erben für Interfaces und die Value-Type-Regeln.

**Repo:** `crates/lang-rules/src/emit/idl4_csharp/mod.rs::Platform`, `crates/lang-rules/src/emit/idl4_csharp/mod.rs::namespace_of`, `crates/idl-csharp/runtime/Corba.cs`, `runtime/corba/CORBA_PIDL.idl`

**Tests:** `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_a_1_2_the_corba_module_is_the_corba_namespace_under_the_idl_naming_scheme`, `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_a_1_typecode_object_any_and_user_exception_are_the_annexs_classes`, `crates/lang-rules/tests/csharp_idl4_corba.rs::idl4csharp_a_1_1_and_a_1_5_exceptions_and_interfaces_on_the_corba_platform`

**Status:** `done` — `--csharp-platform corba`: Exceptions erweitern `Corba.UserException`, `TypeCode`, `IObject`/`Object` und `Any` sind die Klassen der Annex mit dem Verhalten aus CORBA 3.3 Teil 1 §8.11.1, Interfaces erweitern `Corba.IObject`, ihre Klassen `Corba.Object`; `--csharp-corba-module` schreibt das C# des CORBA-Moduls (Namespace `Corba`, IDL-Schema). Native Typen des CORBA-Moduls sind `object`; eine Operation eines Interface mit Namen und Parametern einer Operation von `Object`, aber anderem Ergebnis, ist explizit implementiert (Entscheidungen).

### A.2 DDS-Specific Mappings

**Spec:** Annex A.2, S. 49 — "DDS requires no additional
platform-specific language mappings."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Klausel stellt keine zusätzliche
Anforderung.

### Annex B: Building Block Traceability Matrix

**Spec:** Annex B, S. 51 — nicht-normative Matrix.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — ausdrücklich non-normativ.

---

## Audit-Status

65 done / 0 partial / 0 open / 0 blocked / 16 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9310 von `main` (`python3 ci/coverage_results.py --document idl4-csharp-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-ccm --all-targets` (Job `merge-rust-ccm`) — 2 Tests grün.
* `cargo nextest run --profile merge -p zerodds-idl-lower --all-targets` (Job `merge-rust-idl-lower`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-idlc --all-targets` (Job `merge-rust-idlc`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-lang-rules --all-targets` (Job `merge-rust-lang-rules`) — 4 Tests grün.
* `python3 ci/run_integration.py merge-codegen-csharp` (Job `merge-codegen-csharp`) — 28 Tests grün.

36 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.
