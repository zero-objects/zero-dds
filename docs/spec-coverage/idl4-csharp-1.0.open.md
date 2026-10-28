# idl4-csharp-1.0 — offene Punkte

Aggregat aus `idl4-csharp-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## 2.1 Implementierung: IDL -> C# Source per Kapitel 7

**Status:** `partial` — der Walk transformiert IDL in C#-Quelltext, folgt Kapitel 7 aber nur im Typkern; die API-Form weicht in den unten einzeln belegten Punkten ab (Felder statt Properties, `T[]` statt `ISequence<T>`, keine `I`-Präfixe, kein `System.Exception`-Erbe).

## 7.1.1 Zwei Namensschemata, selektiert über @csharp_mapping

**Status:** `open` — der Materialisierer kennt kein `@csharp_mapping` und keinen Umschalter; er folgt fest einer Regel (Typnamen verbatim, Member erste Buchstabe groß).

## 7.1.1 Kollisionsauflösung: `@`-Präfix bei Keyword, `_`-Präfix bei Spec-Namen

**Status:** `partial` — das `@`-Präfix für C#-Schlüsselwörter ist implementiert; der `_`-Fall für spec-eingeführte Namen (`Constants`) fehlt, weil der Materialisierer sie nicht als reserviert führt (`ZdConstants`).

## 7.1.1.1 IDL Naming Scheme: Namen ohne Case-Transformation

**Status:** `open` — das Schema ist nicht wählbar. Der Materialisierer transformiert Member (erster Buchstabe groß) und lässt Typnamen verbatim, also eine Mischung, die keinem der beiden Schemata entspricht.

## 7.1.1.2 .NET Framework Design Guidelines Naming Scheme

**Status:** `partial` — Member werden nur im ersten Buchstaben gehoben, Typen und Enumeratoren bleiben verbatim; die .NET-Regeln (volles Pascal Case, `I`-Präfix) sind nicht angewandt.

## 7.1.1.2.1 Pascal-Case-Transformation

**Status:** `partial` — nur der erste Buchstabe wird gehoben (`my_field` -> `My_field`); Unterstriche werden nicht entfernt, weil das volle Pascal Case nicht injektiv über die von IDL 4.2 §7.2.3 erlaubten Namen wäre (der Grund steht im Modul-Doc).

## 7.1.1.2.2 Camel-Case-Transformation

**Status:** `partial` — Operationsparameter werden verbatim übernommen, nicht in Camel Case transformiert.

## 7.1.2 Reserved Names (C#-Keywords + `Constants`)

**Status:** `partial` — die C#-Keywords sind als reserviert geführt; der Name `Constants` ist es nicht, weil die Klasse `ZdConstants` heißt.

## 7.2.3 Constants: zwei Alternativen

**Status:** `partial` — nur die Container-Variante existiert, und sie ist nicht über `@csharp_mapping` wählbar; die Standalone-Variante fehlt.

## 7.2.3.1 Standalone Constants: Klasse pro Konstante mit `Value`

**Status:** `open` — Konstanten werden als `const`-Felder in einer gemeinsamen `ZdConstants`-Klasse gehalten, nicht als je eine Klasse pro Konstante.

## 7.2.3.2 Constants Container: `public static partial class Constants`

**Status:** `partial` — die Klasse ist `public static`, aber weder `partial` noch `Constants` genannt noch über die Annotation konfigurierbar.

## 7.2.4.1.2 Floating-Point-Typen (Tab.7.3)

**Status:** `partial` — `float` und `double` stimmen; `long double` wird als generierter 16-Oktett-Halter `global::ZeroDds.Idl.Float128` emittiert, nicht als `decimal` (spec-eigene Warnung zur Präzisionsverfälschung; die Abweichung ist im Modul-Doc begründet).

## 7.2.4.2.1 IDL Sequence -> Omg.Types.ISequence<T>

**Status:** `partial` — Sequenzen werden als `T[]` emittiert; die Laufzeit deklariert `ISequence<T>`/`IBoundedSequence<T>`, der Materialisierer referenziert sie aber nicht.

## 7.2.4.2.1 Tab.7.4: Sequenzen von Basistypen

**Status:** `partial` — die Elementtypen stimmen, der Containertyp ist `T[]` statt `Omg.Types.ISequence<T>`.

## 7.2.4.2.1 Sequence-/Map-Member als read-only Properties (mit @external)

**Status:** `partial` — Member sind öffentliche, schreibbare Felder, keine Properties; `@external` ändert die C#-Form nicht.

## 7.2.4.2.3 IDL wstring -> C# string (UTF-16)

**Status:** `partial` — kein Test prüft, dass IDL `wstring` auf C# `string` abgebildet wird; der zitierte Korpus-Lauf prüft nur, dass die Fälle rendern.

## 7.2.4.2.4 IDL fixed -> C# decimal + ArithmeticException

**Status:** `partial` — `fixed<P,S>` wird als `string` gehalten, der den Dezimalwert trägt; die Spec verlangt `decimal`. Grund: `decimal` hat 28-29 Stellen, IDL 4.2 erlaubt bis 31 (im Modul-Doc belegt).

## 7.2.4.3.1 IDL struct -> C# public class

**Status:** `partial` — es entsteht eine `public sealed class` mit einem öffentlichen Feld pro Member; Properties, Copy-/All-Values-Konstruktor und `IEquatable<T>` fehlen, die Default-Initialisierung ist die C#-Default-Initialisierung.

## 7.2.4.3.2 IDL union -> C# public class (Discriminator-Protokoll)

**Status:** `partial` — die Klasse trägt `_d` und ein Feld pro Case; die `Discriminator`-Property, Getter-Paare, `Set<Member>`-Modifier, `InvalidOperationException` und `IEquatable<T>` fehlen.

## 7.2.4.3.3 @bit_bound-Underlying für Enums

**Status:** `partial` — es wird immer `: int` geschrieben; `@bit_bound` wählt keinen Underlying-Typ.

## 7.2.4.4 IDL Array -> C# Array + Bounds-Check

**Status:** `partial` — Arrays werden als gejaggte `T[][]`-Felder emittiert; einen Setter mit Längenprüfung gibt es nicht (Felder statt Properties).

## 7.3 IDL any -> Omg.Types.Any

**Status:** `partial` — die Laufzeit deklariert `Omg.Types.Any`, der Materialisierer verweigert `any` aber namentlich (`MaterialiseError::NoLanguageForm`), statt darauf abzubilden.

## 7.4 IDL interface -> C# public interface

**Status:** `partial` — Methoden und Attribut-Properties sowie `in`/`out`/`ref` stimmen; das `I`-Präfix fehlt (Name verbatim), und abgeleitete Interfaces werden flach in die eine Deklaration geschrieben statt über eine Basisliste zu erben.

## 7.4.1 IDL exception -> C# class : System.Exception

**Status:** `partial` — die Klasse existiert mit einem Feld pro Member, erbt aber bewusst nicht von `System.Exception` (Grund: CS0108/CS0542 bei Membern wie `message`; im Modul-Doc belegt).

## 7.4.2 Interface Forward Declaration

**Status:** `partial` — der zitierte Test prüft, dass eine Vorwärtsdeklaration ohne Definition als E002 verweigert wird; dass eine mit Definition keine eigene Deklaration erzeugt, prüft keiner.

## 7.5 I<Name>Operations + I<Name> + <Name>-Klasse

**Status:** `partial` — es entsteht genau eine Interface-Deklaration; die Operations/Container-Aufteilung nach §7.5 und die eingebetteten Typen in einer `<Name>`-Klasse fehlen.

## 7.6 IDL valuetype -> abstrakte Klasse + Fabrik-Attribut

**Status:** `partial` — es entsteht eine `sealed class` mit den State-Membern; die abstrakte Klasse, die Trennung public/private -> protected, Operationen und das `FactoryAttribute` fehlen.

## 7.14.1 Struct mit Single Inheritance -> C# extends

**Status:** `partial` — die Basis-Member werden in die abgeleitete `sealed class` gefaltet (XTypes-1.3-Layout), es gibt aber keine C#-Basisklasse (`extends`).

## 7.14.3.1 IDL map -> IDictionary<TKey,TValue>

**Status:** `partial` — Maps werden als `ZdEntry_<k>_<v>[]` (Array einer generierten Zwei-Feld-Klasse) emittiert, nicht als `IDictionary`. Grund ist die Ordnungsachse (`MapEntryOrder`), im Modul-Doc belegt.

## 7.14.3.2 IDL bitset -> C# struct + IEquatable

**Status:** `partial` — es entsteht eine Klasse mit einem öffentlichen Feld pro Bitfeld (richtige Holder-Breite), kein `struct` mit Properties und kein `IEquatable<T>`.

## 7.14.3.3 IDL bitmask -> C# enum `[Flags]` + BitArray

**Status:** `partial` — `[Flags]`-Enum mit korrektem Holder und Positionswerten ist da; der `Flags`-Suffix am Namen und die `BitArray`-Verwendung fehlen.

## 7.16.1 Definieren von Annotationen -> Attributklasse

**Status:** `open` — der Materialisierer verweigert Annotationstypen (`TypeKind::AnnotationType`); es wird keine Attributklasse erzeugt.

## 7.16.2 Anwenden von Annotationen

**Status:** `open` — Nutzerannotationen werden nicht propagiert; die einzige emittierte Annotation ist das interne `[IdlName]`.

## 7.16.2.1 Annotationen auf typedef propagieren

**Status:** `open` — es gibt keine Attributpropagation, weder über typedef noch sonst.

## 7.17.1 @id, @autoid (kein Impact)

**Status:** `partial` — kein Test vergleicht die C#-Form mit und ohne `@id`/`@autoid`; der zitierte Korpus-Lauf prüft nur, dass die Fälle rendern.

## 7.17.1 @optional -> Nullable<T> + OptionalAttribute

**Status:** `partial` — Werttypen werden `T?`, Referenztypen bleiben unverändert; das `Omg.Types.OptionalAttribute` wird nicht emittiert.

## 7.17.1 @extensibility, @final, @mutable, @appendable (kein Impact)

**Status:** `partial` — kein Test vergleicht die C#-Form der `@extensibility`-Varianten; der zitierte Korpus-Lauf prüft nur, dass die Fälle rendern.

## 7.17.2 @key -> KeyAttribute

**Status:** `open` — der Materialisierer wertet `@key` nicht aus und emittiert kein `[Key]`; `Omg.Types.KeyAttribute` liegt ungenutzt in der Laufzeit.

## 7.17.2 @must_understand (kein Impact)

**Status:** `partial` — kein Test vergleicht die C#-Form mit und ohne `@must_understand`; der zitierte Korpus-Lauf prüft nur, dass der Fall rendert.

## 7.17.2 @default_literal

**Status:** `open` — `@default_literal` wird nicht ausgewertet; der C#-Enum-Default bleibt der Wert 0.

## 7.17.3 Units and Ranges: @default, @range, @min, @max, @unit

**Status:** `open` — weder Default-Initialisierung noch Bereichsprüfungen noch `UnitAttribute` sind implementiert.

## 7.17.4 @external

**Status:** `open` — `@external` ändert die C#-Form nicht; es gibt weder `ExternalAttribute`, noch `IEnumerable<T>` für externe Sequenzen.

## 7.17.4 @nested (kein Impact)

**Status:** `partial` — kein Test vergleicht die C#-Form mit und ohne `@nested`; der zitierte Korpus-Lauf prüft nur, dass der Fall rendert.

## 8.1 @csharp_mapping-Annotation

**Status:** `open` — die Annotation wird weder erkannt noch ausgewertet.

## 8.1.1 apply_naming_convention

**Status:** `open` — kein Umschalter; die Namensform ist fest.

## 8.1.2 constants_container

**Status:** `open` — es gibt nur eine feste Container-Klasse (`ZdConstants`); der Parameter ist nicht implementiert.

## 8.1.3 struct_type

**Status:** `open` — IDL-Structs werden immer als `class` emittiert; der Parameter ist nicht implementiert.

## A.1 CORBA-Specific Mappings (A.1.1-A.1.6)

**Status:** `open` — die C#-Spalte hat kein CORBA-Backend. Ein `emit_corba_traits` existiert nur in `crates/corba-codegen/src/traits.rs` (Rust-CORBA), nicht in der C#-Spalte; `Corba.*`-Typen werden nicht emittiert.
