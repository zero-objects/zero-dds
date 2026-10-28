# IDL4 to C# Language Mapping 1.0 — Spec-Coverage

**Spec:** [OMG IDL4-CSHARP 1.0](https://www.omg.org/spec/IDL4-CSHARP/1.0/PDF) (61 Seiten, OMG formal/2021-07-01)

Item-für-Item-Audit gegen die Spec; jede Anforderung mit Spec-Zitat,
Repo-Pfad, Test-Pfad und Status (`done` / `partial` / `open` /
`n/a`).

**Kontext:** Dieses Dokument prüft die OMG-PSM (Kapitel 7 und 8) gegen
den heutigen Baum. Der frühere PSM-Generator `crates/idl-csharp/src/`
wurde in `d045e43ea86` entfernt; die Seite zitiert deshalb heute die
Wire-Spalte. Diese Spalte ist die C#-Bindung des geteilten Codec-Walks:

- `crates/lang-rules/src/adapter/csharp.rs` — Materialisierer der
  Deklarationen,
- `crates/lang-rules/src/emit/csharp.rs` + `default_csharp.rs` — Codec
  und Table-9-Defaulter,
- `crates/lang-cst/src/csharp/` + `crates/lang-render/src/csharp.rs` —
  Syntaxbaum und Textform,
- `crates/lang-rules/src/rule/csharp.rs` — Regelkatalog,
- `tools/idlc/src/csharp_support.rs` — Treiber.

Erhalten ist die Laufzeit `crates/idl-csharp/runtime/Omg.Types.cs`
(`ISequence<T>`/`IBoundedSequence<T>`/`BoundedList<T>`, `Any`,
`ITopicType<T>` und die Attribut-Brücke `[Key]`/`[Id]`/`[Optional]`/
`[MustUnderstand]`/`[External]`/`[Nested]`/`[Extensibility]`). Der
Materialisierer emittiert diese Formen heute nicht; er schreibt ein
Wire-Datenmodell mit öffentlichen Feldern, `T[]`, Map-Eintragsklassen,
`using`-Aliassen und `[Flags]`-Enums. Wo die PSM eine API-Form
verlangt, die der Walk nicht erzeugt, ist der Status `partial` oder
`open`.

**Offene Punkte:** siehe [`idl4-csharp-1.0.open.md`](idl4-csharp-1.0.open.md).

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

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_file`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_generated_file_carries_the_support_namespace`

**Status:** partial — der Walk transformiert IDL in C#-Quelltext, folgt
Kapitel 7 aber nur im Typkern; die API-Form weicht in den unten
einzeln belegten Punkten ab (Felder statt Properties, `T[]` statt
`ISequence<T>`, keine `I`-Präfixe, kein `System.Exception`-Erbe).

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

**Repo:** —

**Tests:** —

**Status:** open — der Materialisierer kennt kein `@csharp_mapping` und
keinen Umschalter; er folgt fest einer Regel (Typnamen verbatim,
Member erste Buchstabe groß).

### 7.1.1 Kollisionsauflösung: `@`-Präfix bei Keyword, `_`-Präfix bei Spec-Namen

**Spec:** §7.1.1, S. 5 — "the collision shall be resolved by prepending
the '@' character [...] when the name collides with a C# language
keyword, or the '_' character when the name collides with a name
introduced by this specification."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::field_name`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_identifier_that_is_a_csharp_keyword_is_escaped_with_an_at_sign`

**Status:** partial — das `@`-Präfix für C#-Schlüsselwörter ist
implementiert; der `_`-Fall für spec-eingeführte Namen (`Constants`)
fehlt, weil der Materialisierer sie nicht als reserviert führt
(`ZdConstants`).

### 7.1.1.1 IDL Naming Scheme: Namen ohne Case-Transformation

**Spec:** §7.1.1.1, S. 5 — "IDL member names and type identifiers shall
map to C# names and identifiers without case transformation."

**Repo:** —

**Tests:** —

**Status:** open — das Schema ist nicht wählbar. Der Materialisierer
transformiert Member (erster Buchstabe groß) und lässt Typnamen
verbatim, also eine Mischung, die keinem der beiden Schemata entspricht.

### 7.1.1.2 .NET Framework Design Guidelines Naming Scheme

**Spec:** §7.1.1.2, S. 5 — "IDL member names and type identifiers shall
map to C# names and identifiers that follow the coding guidelines
defined in the Framework Design Guidelines of [.NET-GUIDE]."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::capitalise`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_field_carries_the_idl_name_it_was_folded_from`

**Status:** partial — Member werden nur im ersten Buchstaben gehoben,
Typen und Enumeratoren bleiben verbatim; die .NET-Regeln (volles Pascal
Case, `I`-Präfix) sind nicht angewandt.

### 7.1.1.2.1 Pascal-Case-Transformation

**Spec:** §7.1.1.2.1, S. 5 — "The first letter after each underscore
shall be capitalized and all underscores shall be removed. The first
letter of the IDL name shall be capitalized."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::capitalise`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_field_carries_the_idl_name_it_was_folded_from`

**Status:** partial — nur der erste Buchstabe wird gehoben
(`my_field` -> `My_field`); Unterstriche werden nicht entfernt, weil das
volle Pascal Case nicht injektiv über die von IDL 4.2 §7.2.3 erlaubten
Namen wäre (der Grund steht im Modul-Doc).

### 7.1.1.2.2 Camel-Case-Transformation

**Spec:** §7.1.1.2.2, S. 6 — "The first letter after each underscore
shall be capitalized and all underscores shall be removed. The first
letter of the IDL name shall be lower case."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_param`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_interface_is_an_interface_of_signatures_with_the_idl_passing_modes`

**Status:** partial — Operationsparameter werden verbatim übernommen,
nicht in Camel Case transformiert.

### 7.1.2 Reserved Names (C#-Keywords + `Constants`)

**Spec:** §7.1.2, S. 5 — reserviert sind die C#-Keywords aus ECMA-334
§7.4.4 und der Klassenname `Constants` pro Modul-Namespace.

**Repo:** `crates/lang-cst/src/csharp/mod.rs::is_keyword`

**Tests:** `crates/lang-cst/src/csharp/mod.rs::tests::a_keyword_escapes_with_an_at_sign_and_keeps_its_identity`

**Status:** partial — die C#-Keywords sind als reserviert geführt; der
Name `Constants` ist es nicht, weil die Klasse `ZdConstants` heißt.

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

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::scope_items`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_module_is_a_namespace_and_two_modules_do_not_collide`,
`crates/lang-rules/src/adapter/csharp.rs::tests::a_module_reopened_beside_a_nested_one_is_one_namespace_block`,
`crates/lang-rules/src/adapter/csharp.rs:2882: assert!(generated.contains("namespace a"), "{generated}");`

**Status:** done

### 7.2.3 Constants: zwei Alternativen

**Spec:** §7.2.3, S. 7 — "two alternatives for mapping IDL constants
[...] Standalone Constants Mapping [...] Constants Container Mapping."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::constants`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_constant_becomes_a_const_field_of_a_holder_class`

**Status:** partial — nur die Container-Variante existiert, und sie ist
nicht über `@csharp_mapping` wählbar; die Standalone-Variante fehlt.

### 7.2.3.1 Standalone Constants: Klasse pro Konstante mit `Value`

**Spec:** §7.2.3.1, S. 7 — "IDL constants shall be mapped to public
static classes of the same name [...]. The mapped class shall contain a
public const called Value."

**Repo:** —

**Tests:** —

**Status:** open — Konstanten werden als `const`-Felder in einer
gemeinsamen `ZdConstants`-Klasse gehalten, nicht als je eine Klasse pro
Konstante.

### 7.2.3.2 Constants Container: `public static partial class Constants`

**Spec:** §7.2.3.2, S. 8 — "Every scope containing a constant
declaration shall contain a public static partial class. By default,
the mapped class shall be named Constants."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::scope_items`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_constant_becomes_a_const_field_of_a_holder_class`

**Status:** partial — die Klasse ist `public static`, aber weder
`partial` noch `Constants` genannt noch über die Annotation
konfigurierbar.

### 7.2.4.1.1 Integer-Typen (Tab.7.2)

**Spec:** §7.2.4.1.1, Tab.7.2, S. 9 — "int8 -> sbyte; uint8 -> byte;
short/int16 -> short; unsigned short/uint16 -> ushort; long/int32 ->
int; unsigned long/uint32 -> uint; long long/int64 -> long; unsigned
long long/uint64 -> ulong."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_primitive`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_idl_primitive_reaches_the_csharp_type_that_holds_it`,
`crates/lang-rules/src/adapter/csharp.rs:2630: assert!(generated.contains(expected), "{expected}\n{generated}");`

**Status:** done

### 7.2.4.1.2 Floating-Point-Typen (Tab.7.3)

**Spec:** §7.2.4.1.2, Tab.7.3, S. 10 — "float -> float; double ->
double; long double -> decimal."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_primitive`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_idl_primitive_reaches_the_csharp_type_that_holds_it`

**Status:** partial — `float` und `double` stimmen; `long double` wird
als generierter 16-Oktett-Halter `global::ZeroDds.Idl.Float128`
emittiert, nicht als `decimal` (spec-eigene Warnung zur
Präzisionsverfälschung; die Abweichung ist im Modul-Doc begründet).

### 7.2.4.1.3 IDL char -> C# char

**Spec:** §7.2.4.1.3, S. 10 — "The IDL char type shall be mapped to the
C# type char."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_primitive`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_idl_primitive_reaches_the_csharp_type_that_holds_it`,
`crates/lang-rules/src/adapter/csharp.rs:2630: assert!(generated.contains(expected), "{expected}\n{generated}");`

**Status:** done

### 7.2.4.1.4 IDL wchar -> C# char

**Spec:** §7.2.4.1.4, S. 10 — "The IDL wchar type shall be mapped to
the C# type char."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_primitive`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_idl_primitive_reaches_the_csharp_type_that_holds_it`,
`crates/lang-rules/src/adapter/csharp.rs:2630: assert!(generated.contains(expected), "{expected}\n{generated}");`

**Status:** done

### 7.2.4.1.5 IDL boolean -> C# bool

**Spec:** §7.2.4.1.5, S. 10 — "The IDL boolean type shall be mapped to
the C# bool, and the IDL constants TRUE and FALSE shall be mapped to
the corresponding C# boolean literals."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_primitive`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_idl_primitive_reaches_the_csharp_type_that_holds_it`,
`crates/lang-rules/src/adapter/csharp.rs:2630: assert!(generated.contains(expected), "{expected}\n{generated}");`

**Status:** done

### 7.2.4.1.6 IDL octet -> C# byte

**Spec:** §7.2.4.1.6, S. 10 — "The IDL type octet [...] shall be mapped
to the C# type byte."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_primitive`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_idl_primitive_reaches_the_csharp_type_that_holds_it`,
`crates/lang-rules/src/adapter/csharp.rs:2630: assert!(generated.contains(expected), "{expected}\n{generated}");`

**Status:** done

### 7.2.4.2.1 IDL Sequence -> Omg.Types.ISequence<T>

**Spec:** §7.2.4.2.1, S. 10 — "IDL sequences shall be mapped to the C#
Omg.Types.ISequence<T> interface [...]. Implementations of
Omg.Types.ISequence<T> shall extend
System.Collections.Generic.IList<T>."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::type_at` (Array-Arm),
`crates/idl-csharp/runtime/Omg.Types.cs`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_sequence_and_an_array_are_both_csharp_arrays`

**Status:** partial — Sequenzen werden als `T[]` emittiert; die
Laufzeit deklariert `ISequence<T>`/`IBoundedSequence<T>`, der
Materialisierer referenziert sie aber nicht.

### 7.2.4.2.1 Tab.7.4: Sequenzen von Basistypen

**Spec:** §7.2.4.2.1, Tab.7.4, S. 11-12 — `sequence<boolean>` ->
`Omg.Types.ISequence<bool>`, ... `sequence<long double>` ->
`Omg.Types.ISequence<decimal>`.

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::type_at`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_sequence_and_an_array_are_both_csharp_arrays`

**Status:** partial — die Elementtypen stimmen, der Containertyp ist
`T[]` statt `Omg.Types.ISequence<T>`.

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

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::fields_of`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_sequence_and_an_array_are_both_csharp_arrays`

**Status:** partial — Member sind öffentliche, schreibbare Felder,
keine Properties; `@external` ändert die C#-Form nicht.

### 7.2.4.2.2 IDL string -> C# string (UTF-16)

**Spec:** §7.2.4.2.2, S. 12 — "IDL strings, both bounded and unbounded
variants, shall be mapped to C# strings. The resulting strings shall be
encoded in UTF-16 format."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::type_at` (String-Arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_map_is_an_array_of_a_generated_entry_class`,
`crates/lang-rules/src/adapter/csharp.rs:2715: assert!(generated.contains("public string Value;"), "{generated}");`

**Status:** done

### 7.2.4.2.3 IDL wstring -> C# string (UTF-16)

**Spec:** §7.2.4.2.3, S. 12 — wie §7.2.4.2.2 für `wstring`.

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::type_at` (String-Arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** partial — kein Test prüft, dass IDL `wstring` auf C# `string` abgebildet wird; der zitierte Korpus-Lauf prüft nur, dass die Fälle rendern.

### 7.2.4.2.4 IDL fixed -> C# decimal + ArithmeticException

**Spec:** §7.2.4.2.4, S. 12 — "The IDL fixed type shall be mapped to
the C# decimal type. Range checking shall raise a
System.ArithmeticException exception, or a derived exception, if
necessary."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::type_at` (Fixed-Arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_fixed_is_a_string_and_not_a_decimal`

**Status:** partial — `fixed<P,S>` wird als `string` gehalten, der den
Dezimalwert trägt; die Spec verlangt `decimal`. Grund: `decimal` hat
28-29 Stellen, IDL 4.2 erlaubt bis 31 (im Modul-Doc belegt).

### 7.2.4.3.1 IDL struct -> C# public class

**Spec:** §7.2.4.3.1, S. 13 — "An IDL struct shall be mapped to a C#
public class [...] A public property [...] A public default
constructor [...] A public copy constructor [...] A public
constructor that accepts parameters for each member [...] The class
shall implement the IEquatable<T> interface."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::fields_of`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_field_carries_the_idl_name_it_was_folded_from`,
`crates/lang-rules/src/adapter/csharp.rs::tests::a_member_that_would_collide_with_its_type_cannot_come_from_idl`

**Status:** partial — es entsteht eine `public sealed class` mit einem
öffentlichen Feld pro Member; Properties, Copy-/All-Values-Konstruktor
und `IEquatable<T>` fehlen, die Default-Initialisierung ist die
C#-Default-Initialisierung.

### 7.2.4.3.2 IDL union -> C# public class (Discriminator-Protokoll)

**Spec:** §7.2.4.3.2, S. 14-15 — "A public read-only property named
Discriminator [...] A public property with getters and setters for each
member [...] Property getters shall raise a
System.InvalidOperationException if the expected member has not been
set [...] implement the IEquatable<T> interface."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare` (Union-Arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`

**Status:** partial — die Klasse trägt `_d` und ein Feld pro Case; die
`Discriminator`-Property, Getter-Paare, `Set<Member>`-Modifier,
`InvalidOperationException` und `IEquatable<T>` fehlen.

### 7.2.4.3.3 IDL enum -> C# public enum

**Spec:** §7.2.4.3.3, S. 18 — "An IDL enum shall be mapped to a C#
public enum with the same name as the IDL enum type."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare` (Enum-Arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_enumeration_is_a_real_enum_and_a_member_of_it_is_that_type`,
`crates/lang-rules/src/adapter/csharp.rs::tests::an_explicit_enumerator_value_is_written_out`,
`crates/lang-rules/src/adapter/csharp.rs:2803: generated.contains("public enum Colour : int"),`

**Status:** done

### 7.2.4.3.3 @bit_bound-Underlying für Enums

**Spec:** §7.2.4.3.3, S. 18 — "the corresponding C# enum type shall be
sbyte for bit bound values between 1 and 8; short, for [...] 9 and 16;
int, for [...] 17 and 32; and long, for [...] 33 and 64."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare` (Enum-Arm,
`underlying`)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_enumeration_is_a_real_enum_and_a_member_of_it_is_that_type`

**Status:** partial — es wird immer `: int` geschrieben; `@bit_bound`
wählt keinen Underlying-Typ.

### 7.2.4.3.4 Constructed Recursive Types

**Spec:** §7.2.4.3.4, S. 18 — "Constructed recursive types are
supported by mapping the involved types directly to C#."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::data_class`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`,
`crates/lang-rules/src/adapter/csharp.rs:2581: assert!(failures.is_empty(), "{failures:#?}");`

**Status:** done — generierte Aggregattypen sind C#-Klassen
(Referenzsemantik); die rekursiven Korpusfälle (`recursive-*`,
`resolve-forward-declared`) rendern.

### 7.2.4.4 IDL Array -> C# Array + Bounds-Check

**Spec:** §7.2.4.4, S. 19 — "An IDL array shall be mapped to a C# array
[...]. The bounds for the array shall be checked by the setter of the
corresponding property and a System.ArgumentOutOfRangeException shall
be raised if a bounds violation occurs."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::type_at` (Array-Arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_sequence_and_an_array_are_both_csharp_arrays`

**Status:** partial — Arrays werden als gejaggte `T[][]`-Felder
emittiert; einen Setter mit Längenprüfung gibt es nicht (Felder statt
Properties).

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

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::type_at` (Alias-Arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_typedef_is_a_using_alias_and_every_use_takes_the_resolved_type`,
`crates/lang-rules/src/adapter/csharp.rs:2869: assert!(generated.contains("using Row = int[];"), "{generated}");`

**Status:** done — ein `using`-Alias ist kein eigener Typ, und jede
Verwendung nimmt den aufgelösten Zieltyp.

---

## §7.3 Any

### 7.3 IDL any -> Omg.Types.Any

**Spec:** §7.3, S. 22 — "The IDL any type shall be mapped to
Omg.Types.Any type."

**Repo:** `crates/idl-csharp/runtime/Omg.Types.cs` (`Any`),
`crates/lang-rules/src/adapter/csharp.rs::type_at` (Any-Arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_construct_with_no_csharp_data_form_is_named_and_not_left_out`

**Status:** partial — die Laufzeit deklariert `Omg.Types.Any`, der
Materialisierer verweigert `any` aber namentlich
(`MaterialiseError::NoLanguageForm`), statt darauf abzubilden.

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

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::interfaces`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_interface_is_an_interface_of_signatures_with_the_idl_passing_modes`,
`crates/lang-rules/tests/csharp_consumer_assembly.rs::a_second_assembly_implements_the_generated_interface_operations_and_attributes`

**Status:** partial — Methoden und Attribut-Properties sowie
`in`/`out`/`ref` stimmen; das `I`-Präfix fehlt (Name verbatim), und
abgeleitete Interfaces werden flach in die eine Deklaration geschrieben
statt über eine Basisliste zu erben.

### 7.4.1 IDL exception -> C# class : System.Exception

**Spec:** §7.4.1, S. 23 — "An IDL exception shall be mapped to a C#
class [...]. The mapped class shall extend the System.Exception class."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::exceptions`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_exception_is_a_data_class_and_not_a_system_exception`

**Status:** partial — die Klasse existiert mit einem Feld pro Member,
erbt aber bewusst nicht von `System.Exception` (Grund: CS0108/CS0542
bei Membern wie `message`; im Modul-Doc belegt).

### 7.4.2 Interface Forward Declaration

**Spec:** §7.4.2, S. 23 — "An IDL interface forward declaration has no
mapping to the C# language."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::interfaces`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::a_forward_declaration_without_a_definition_is_e002`,
`crates/lang-rules/tests/interface_bindings.rs:272: Some("DDS-TS-E002")`

**Status:** `partial` — der zitierte Test prüft, dass eine Vorwärtsdeklaration ohne Definition als E002 verweigert wird; dass eine mit Definition keine eigene Deklaration erzeugt, prüft keiner.

---

## §7.5 Interfaces – Full

### 7.5 I<Name>Operations + I<Name> + <Name>-Klasse

**Spec:** §7.5, S. 23-24 — "A C# public interface named
I<InterfaceName>Operations [...] A C# public interface named
I<InterfaceName> inheriting from I<InterfaceName>Operations [...] A C#
public class named <InterfaceName> containing the declaration of the
classes and exceptions that result from the mapping of all the types
and exceptions declared within the IDL interface."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::interfaces`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_interface_is_an_interface_of_signatures_with_the_idl_passing_modes`

**Status:** partial — es entsteht genau eine Interface-Deklaration; die
Operations/Container-Aufteilung nach §7.5 und die eingebetteten Typen
in einer `<Name>`-Klasse fehlen.

---

## §7.6 Value Types

### 7.6 IDL valuetype -> abstrakte Klasse + Fabrik-Attribut

**Spec:** §7.6, S. 25 — "An IDL valuetype type shall be mapped to a C#
abstract class. [...] private members shall have the C# protected
access modifier [...] Each valuetype factory operation shall be mapped
[...] to a method returning void [...] annotated with the
FactoryAttribute."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare` (Valuetype-Arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_valuetype_is_a_class_of_its_state_members`

**Status:** partial — es entsteht eine `sealed class` mit den
State-Membern; die abstrakte Klasse, die Trennung public/private ->
protected, Operationen und das `FactoryAttribute` fehlen.

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

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Klauseln verlangen eine Abbildung
auf intermediate IDL, nicht auf C#; die C#-PSM hat hier keinen
eigenen Gegenstand.

---

## §7.14 Extended Data Types

### 7.14.1 Struct mit Single Inheritance -> C# extends

**Spec:** §7.14.1, S. 27 — "An IDL struct that inherits from a base IDL
struct, shall be declared as a C# public class that extends the class
resulting from mapping the base IDL struct."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare` (Structure-Arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** partial — die Basis-Member werden in die abgeleitete
`sealed class` gefaltet (XTypes-1.3-Layout), es gibt aber keine
C#-Basisklasse (`extends`).

### 7.14.2 Union-Discriminatoren: int8, uint8, wchar, octet

**Spec:** §7.14.2, S. 28 — "This building block adds the int8, uint8,
wchar, and octet IDL types to the set of valid types for a
discriminator."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::csharp_member_type`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`,
`crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`,
`crates/lang-rules/src/adapter/csharp.rs:2581: assert!(failures.is_empty(), "{failures:#?}");`

**Status:** done — `csharp_primitive` deckt alle integralen
Discriminatortypen ab; die Korpusfälle `union-discriminator-*`
rendern.

### 7.14.3.1 IDL map -> IDictionary<TKey,TValue>

**Spec:** §7.14.3.1, S. 28 — "An IDL map shall be mapped to a C#
generic System.Collections.Generic.IDictionary<TKey,TValue>."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::map_entry_name`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_map_is_an_array_of_a_generated_entry_class`

**Status:** partial — Maps werden als `ZdEntry_<k>_<v>[]` (Array einer
generierten Zwei-Feld-Klasse) emittiert, nicht als `IDictionary`. Grund
ist die Ordnungsachse (`MapEntryOrder`), im Modul-Doc belegt.

### 7.14.3.2 IDL bitset -> C# struct + IEquatable

**Spec:** §7.14.3.2, S. 29 — "An IDL bitset shall map to a C# struct
with public properties for each named bitfield [...]. The mapped C#
struct shall implement the IEquatable<T> interface."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare` (Bitset-Arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_bitset_is_a_class_of_one_field_per_bitfield`

**Status:** partial — es entsteht eine Klasse mit einem öffentlichen
Feld pro Bitfeld (richtige Holder-Breite), kein `struct` mit
Properties und kein `IEquatable<T>`.

### 7.14.3.3 IDL bitmask -> C# enum `[Flags]` + BitArray

**Spec:** §7.14.3.3, S. 30 — "The IDL bitmask type shall map to a C#
public enum with the same name, followed by the "Flags" suffix. [...]
a System.Collections.BitArray shall be used. The C# enum shall have the
System.FlagsAttribute [...]. The size [...] determines the
corresponding C# enum type."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare` (Bitmask-Arm)

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_bitmask_is_a_flags_enum_over_the_holder_its_bound_calls_for`

**Status:** partial — `[Flags]`-Enum mit korrektem Holder und
Positionswerten ist da; der `Flags`-Suffix am Namen und die
`BitArray`-Verwendung fehlen.

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

**Repo:** —

**Tests:** —

**Status:** open — der Materialisierer verweigert Annotationstypen
(`TypeKind::AnnotationType`); es wird keine Attributklasse erzeugt.

### 7.16.2 Anwenden von Annotationen

**Spec:** §7.16.2, S. 32 — "IDL elements annotated with user-defined
annotations shall map to equivalent C# elements annotated with the
corresponding attribute."

**Repo:** —

**Tests:** —

**Status:** open — Nutzerannotationen werden nicht propagiert; die
einzige emittierte Annotation ist das interne `[IdlName]`.

### 7.16.2.1 Annotationen auf typedef propagieren

**Spec:** §7.16.2.1, S. 33 — "Annotations on an IDL typedef shall be
applied to uses of the typedef in other type declarations."

**Repo:** —

**Tests:** —

**Status:** open — es gibt keine Attributpropagation, weder über
typedef noch sonst.

---

## §7.17 Standardized Annotations

### 7.17.1 @id, @autoid (kein Impact)

**Spec:** §7.17.1, Tab.7.5, S. 33-35 — "@id No impact on language
mapping; @autoid No impact on language mapping."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::fields_of`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** partial — kein Test vergleicht die C#-Form mit und ohne
`@id`/`@autoid`; der zitierte Korpus-Lauf prüft nur, dass die Fälle
rendern.

### 7.17.1 @optional -> Nullable<T> + OptionalAttribute

**Spec:** §7.17.1, Tab.7.5, S. 34 — "IDL elements annotated with
@optional whose type T maps to a C# value type shall map to
System.Nullable<T>. [...] All mapped optional elements shall be
annotated with OptionalAttribute."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::optional`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_optional_value_type_is_nullable_and_a_reference_one_is_not`,
`crates/lang-rules/src/adapter/csharp.rs::tests::an_optional_enum_is_nullable_because_an_enum_is_a_value_type`

**Status:** partial — Werttypen werden `T?`, Referenztypen bleiben
unverändert; das `Omg.Types.OptionalAttribute` wird nicht emittiert.

### 7.17.1 @position, @value

**Spec:** §7.17.1, Tab.7.5, S. 35 — "@position Impacts the mapping of
bitmask types [...]; @value Impacts the mapping of enum types."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_explicit_enumerator_value_is_written_out`,
`crates/lang-rules/src/adapter/csharp.rs::tests::a_bitmask_is_a_flags_enum_over_the_holder_its_bound_calls_for`,
`crates/lang-rules/src/adapter/csharp.rs:2819: assert!(generated.contains("LOW = -1,"), "{generated}");`

**Status:** done

### 7.17.1 @extensibility, @final, @mutable, @appendable (kein Impact)

**Spec:** §7.17.1, Tab.7.5, S. 35 — "No impact on language mapping."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::data_class`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** partial — kein Test vergleicht die C#-Form der
`@extensibility`-Varianten; der zitierte Korpus-Lauf prüft nur, dass die
Fälle rendern.

### 7.17.2 @key -> KeyAttribute

**Spec:** §7.17.2, Tab.7.6, S. 35 — "IDL elements annotated with @key
shall result in equivalent C# elements annotated with KeyAttribute."

**Repo:** —

**Tests:** —

**Status:** open — der Materialisierer wertet `@key` nicht aus und
emittiert kein `[Key]`; `Omg.Types.KeyAttribute` liegt ungenutzt in der
Laufzeit.

### 7.17.2 @must_understand (kein Impact)

**Spec:** §7.17.2, Tab.7.6, S. 35 — "@must_understand No impact on
language mapping."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::fields_of`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** partial — kein Test vergleicht die C#-Form mit und ohne
`@must_understand`; der zitierte Korpus-Lauf prüft nur, dass der Fall
rendert.

### 7.17.2 @default_literal

**Spec:** §7.17.2, Tab.7.6, S. 35 — "The C# element [...] shall be
initialized to element indicated by the annotation."

**Repo:** —

**Tests:** —

**Status:** open — `@default_literal` wird nicht ausgewertet; der
C#-Enum-Default bleibt der Wert 0.

### 7.17.3 Units and Ranges: @default, @range, @min, @max, @unit

**Spec:** §7.17.3, Tab.7.7, S. 37 — "@default [...] initialized to the
value of the annotation; @range/@min/@max [...] shall throw a
System.ArgumentOutOfRangeException [...]; @unit Shall result in the
addition of a UnitAttribute."

**Repo:** —

**Tests:** —

**Status:** open — weder Default-Initialisierung noch
Bereichsprüfungen noch `UnitAttribute` sind implementiert.

### 7.17.4 @bit_bound

**Spec:** §7.17.4, Tab.7.8, S. 38 — "@bit_bound Impacts the mapping of
bitmask as described in Clause 7.14.3.3."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::unsigned_holder`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_bitmask_is_a_flags_enum_over_the_holder_its_bound_calls_for`,
`crates/lang-rules/src/adapter/csharp.rs:2827: assert!(narrow.contains("public enum Small : byte"), "{narrow}");`

**Status:** done — der Holder folgt `byte`/`ushort`/`uint`/`ulong` über
den Bound.

### 7.17.4 @external

**Spec:** §7.17.4, Tab.7.8, S. 38 — "@external [...] map to
System.Nullable<T> [...] Properties created as a result of mapping IDL
sequences annotated with @external shall map to the C#
System.Collections.Generic.IEnumerable<T> interface."

**Repo:** —

**Tests:** —

**Status:** open — `@external` ändert die C#-Form nicht; es gibt weder
`ExternalAttribute`, noch `IEnumerable<T>` für externe Sequenzen.

### 7.17.4 @nested (kein Impact)

**Spec:** §7.17.4, Tab.7.8, S. 38 — "@nested No impact to the language
mapping."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::declare`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** partial — kein Test vergleicht die C#-Form mit und ohne
`@nested`; der zitierte Korpus-Lauf prüft nur, dass der Fall rendert.

### 7.17.5 @verbatim

**Spec:** §7.17.5, Tab.7.9, S. 38 — "Copies verbatim text to the
indicated output position when the indicated language is "*", "c#",
"cs", or "csharp"."

**Repo:** `crates/lang-rules/src/adapter/csharp.rs::weave_verbatim`

**Tests:** `crates/lang-rules/tests/verbatim_end_to_end.rs::csharp_puts_begin_and_end_inside_the_type_body`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::the_wildcard_reaches_every_column`,
`crates/lang-rules/tests/verbatim_end_to_end.rs:234: opening < begin && begin < field && field < end,`

**Status:** done

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

**Repo:** —

**Tests:** —

**Status:** open — die Annotation wird weder erkannt noch ausgewertet.

### 8.1.1 apply_naming_convention

**Spec:** §8.1.1, S. 41 — "If apply_naming_convention is
IDL_NAMING_CONVENTION [...] If [...] DOTNET_NAMING_CONVENTION [...]."

**Repo:** —

**Tests:** —

**Status:** open — kein Umschalter; die Namensform ist fest.

### 8.1.2 constants_container

**Spec:** §8.1.2, S. 42 — "To enable the Standalone Constants Mapping
[...] constants_container shall be set to an empty string. To enable
the Constants Container Mapping [...] set to a valid string."

**Repo:** —

**Tests:** —

**Status:** open — es gibt nur eine feste Container-Klasse
(`ZdConstants`); der Parameter ist nicht implementiert.

### 8.1.3 struct_type

**Spec:** §8.1.3, S. 43 — "struct_type defines the C# type the IDL
struct type map to. By default [...] C# class. This parameter allows
changing the default behavior to map an IDL struct to a C# struct."

**Repo:** —

**Tests:** —

**Status:** open — IDL-Structs werden immer als `class` emittiert; der
Parameter ist nicht implementiert.

---

## Annex A: Platform-Specific Mappings

### A.1 CORBA-Specific Mappings (A.1.1-A.1.6)

**Spec:** Annex A.1, S. 45-48 — `Corba.UserException` für Exceptions,
`Corba.TypeCode`, `Corba.Object`/`Corba.IObject`, `Corba.Any`, die
`Corba.IObject`-Erben für Interfaces und die Value-Type-Regeln.

**Repo:** —

**Tests:** —

**Status:** open — die C#-Spalte hat kein CORBA-Backend. Ein
`emit_corba_traits` existiert nur in `crates/corba-codegen/src/traits.rs`
(Rust-CORBA), nicht in der C#-Spalte; `Corba.*`-Typen werden nicht
emittiert.

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

14 done / 33 partial / 15 open / 17 n/a (informative) / 0 n/a (rejected).

Der PSM-Generator `crates/idl-csharp/src/` ist in `d045e43ea86`
entfallen; die zitierten Tests sind die der heutigen Wire-Spalte
(`crates/lang-rules/`, `crates/lang-cst/`, `crates/lang-render/`) und
der überlebenden Laufzeit `crates/idl-csharp/runtime/Omg.Types.cs`.
Test-Lauf nicht Teil dieses Audits (Dokumentationsänderung); die
Zitate nennen den Test, der das jeweilige Item hält.
