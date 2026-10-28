# idl-4.2 — offene Punkte

Aggregat aus `idl-4.2.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §7.2.5 Other Characters Recognized by IDL (Punctuation)

**Status:** `partial` — die Operatorleiter und `#` sind einzeln belegt; ein Test über alle 28 Zeichen aus Tabelle 7-7 und alle fünf aus Tabelle 7-8 existiert nicht. Auch die iso-8859-1-Buchstaben aus Tabelle 7-2 sind nicht einzeln probiert.

## §7.2.6.2.2 Escape Sequences

**Status:** `partial` — die dezimalen und hexadezimalen Formen sind im Lexer-Test belegt, die `\u`-/`\U`-Form nur als `.idl`-Fixture unter `tools/idlc/tests/idl-4.2-gaps/`, nicht als Rust-Test.

## §7.4.1.3 Syntax (Produktionen 1–68)

**Status:** `partial` — die Produktionen sind erkannt, aber Produktion 62 erlaubt `native_type` auch als Member-Typ-Spec (`<type_spec> ::+ <native_type> | …`); `struct S { native H; };` ist nicht abgedeckt, `parse_type_spec` hat keinen `Native`-Zweig. Die Lücke ist in [`idl-4.2.gaps.md`](idl-4.2.gaps.md) §A gemessen.

## §7.4.1.4.4.2.2 Floating-Point Types

**Status:** `open` — `float` und `double` sind modelliert und getestet. `long double` bildet der Baum als `Float128` ab, aber Rust stable hat kein `f128`; die Arithmetik eines `long double`-Ausdrucks wird deshalb abgewiesen (`EvalError::LongDouble`), und die Klausel verlangt den vollen IEEE-double-extended-Wert mit mindestens 15 Exponenten- und 64 Mantissenbits.

## §7.4.1.4.4.6 Native Types

**Status:** `partial` — die Deklaration ist belegt, die Verwendung als Member-Typ (`struct S { native H; };`) nicht; siehe das Item zu §7.4.1.3.

## §7.4.3.4.3.2.1 Inheritance Rules

**Status:** `open` — es gibt weder eine Diagnose für ein doppelt genanntes Basis-Interface noch einen transitiven Gang im Frontend; der Abgleich `inheritance_rules` in `crates/idl-lower/src/validate.rs` gilt nur für `TypeKind::Structure`. Das Schlüsselwort `virtual` kommt im heutigen Baum nirgends vor. Die Abflachung in `crates/lang-rules` geschieht je Spalte, nicht im Frontend.

## §7.4.3.4.3.3.1 Operations

**Status:** `partial` — `oneway` und `raises` sind getestet. Für die Richtungen `in`/`out`/`inout` und für die `context (…)`-Klausel gibt es keinen Test, der über das Frontend geht: die `context`-Klausel wird geparst und gelowert (`OperationNode::contexts`), aber keine committete `.idl`-Datei des Baums enthält eine, und `ParamDirection` wird nur in handgebauten Modell-Tests geprüft.

## §7.4.3.4.3.3.2 Attributes

**Status:** `partial` — `readonly attribute` und die Ableitung der Operationen sind belegt; `getraises` und `setraises` werden geparst und gelowert (`add_attribute_raises`), aber kein Test behauptet etwas über sie. Nur die Fixture `crates/idl-parse/tests/fixtures/spec_features/attr_raises.idl` führt sie durch die Pipeline, ohne Aussage über ihr Ergebnis.

## §7.4.4.4 Mehrfachvererbung und Mehrdeutigkeit

**Status:** `partial` — die Abflachung über einen Diamanten und die Mehrdeutigkeitsmeldung auf Modellebene sind belegt. Eine Diagnose im Frontend für die Ambiguität aus zwei vererbten, *verschieden* deklarierten Elementen gibt es nicht: `inheritance_rules` in `crates/idl-lower/src/validate.rs` kennt nur `TypeKind::Structure` als Vererbungsträger.

## §7.4.4.4 Attribut und Operation mit demselben Namen

**Status:** `open` — `one_namespace_per_scope` sammelt Attribut- und Operationsnamen in *eine* Namensliste, aber kein Test lässt ein Attribut und eine Operation desselben Namens in einem Interface entstehen. Die naheliegende Gegenprobe fehlt.

## §7.4.5.4.1.3.2 Initializers

**Status:** `partial` — kein Test prüft, dass die `factory`-Deklaration als Operation des Value Type erscheint; der zitierte Test prüft nur, dass sie nicht als State-Member gezählt wird.

## §7.4.6.4.1.3 Repository Id Conflict

**Status:** `partial` — die Herleitung ist eindeutig (ein Präfix, ein `typeid`); ein eigener Konflikt-Test für zwei Deklarationen mit derselben abgeleiteten Id existiert nicht. Der vorhandene Test prüft die negative Seite (ein Verweis auf nichts wird gemeldet).

## §7.4.6.4.4 Use of Native Types

**Status:** `partial` — die Deklaration ist belegt, die Verwendung *als Member-Typ-Spec* (Produktion 62) nicht; siehe das Item zu §7.4.1.3.

## §7.4.6.4.6 Context Expressions

**Status:** `open` — die Klausel ist geparst und gelowert (`OperationNode::contexts`), aber es gibt dazu keinen Test: keine committete `.idl`-Datei des Baums schreibt ein `context`, und die Erhebung unter `tools/idl-4.2-gap-scan/` führt die Probe `bb6-123-op-with-context` mit `fixtures = 0`.

## §7.4.6.4.7 CORBA Module

**Status:** `open` — es gibt kein `module CORBA` in irgendeiner committeten `.idl`-Datei des Baums, kein Symbol in `crates/idl-parse/src` oder `crates/idl-lower/src` und keine Probe. `Object` und `ValueBase` sind als eingebaute Typnamen erreichbar (§7.4.2.5), aber nicht über ein Modul, dessen Inhalt man auflösen könnte.

## §7.4.7.4.3 Value Inheritance Rules

**Status:** `open` — es gibt keine Prüfung dieser Kombinationen. Der einzige Abgleich von Vererbung im Frontend, `inheritance_rules` in `crates/idl-lower/src/validate.rs`, gilt ausschliesslich für `TypeKind::Structure { base }`; ein `Valuetype` trägt dort keine Basis.

## §7.4.7.4.4 Custom Marshaling

**Status:** `partial` — das Schlüsselwort `custom` wird erkannt und im Modell als `ValuetypeKind::Custom` geführt. Die Marshal-/Unmarshal- Operationen werden nicht als solche erkannt und nicht auf Skelett-Operationen abgebildet; der Test prüft die Variante, nicht die Regel.

## §7.4.7.4.5 Truncatable

**Status:** `partial` — `truncatable` wird als Modifikator erfasst und in Tabelle A.8 ausgewertet. Das Abschneiden selbst wird in `crates/corba-codegen/src/skeleton.rs` nicht als solches geführt und hat keinen Test.

## §7.4.7.4.6 Value Base

**Status:** `partial` — im Modell ist `ValueBase` vorhanden und an jeder Position erreichbar; kein Sprachbackend emittiert eine Form dafür. Die Probe `bb7-132-valuebase` steht in `expected.tsv` für Rust *und* C auf `err` (`NoLanguageForm { construct: "ValueBase" }`).

## §7.4.8.4.2.1 Facets und §7.4.8.4.2.2 Receptacles

**Status:** `partial` — im Modell und in der CCM-Umschreibung getestet. Beide Backends lehnen das Konstrukt ab, weil der von einem Port getragene `Object`-Typ keine Sprachform hat (`NoLanguageForm { construct: "Object", language: "Rust" }`, dieselbe Meldung für C); das ist in [`idl-4.2.gaps.md`](idl-4.2.gaps.md) §C.2 festgehalten.

## §7.4.9.4.1 Home Header

**Status:** `partial` — `home` mit `manages` und die Vererbung sind abgedeckt. `abstract home` und `home <identifier> for <scoped_name>` sind es nicht: das Keyword `abstract` erreicht in `crates/idl-parse/src/decl.rs::parse_interface` eine `expect(TokenKind::Interface)`-Stelle, und die Zeichenkette `home for` kommt im Baum nicht vor.

## §7.4.10.4.4.2 Object Root

**Status:** `open` — die Form wird geparst und gelowert (`ComponentRole::Facet` mit `Ref::Broken`), aber es gibt keinen Test: die einzige Stelle im Baum, die `provides Object` schreibt, ist die Fixture `crates/idl-parse/tests/fixtures/spec_features/ccm_components.idl`, und die wird nur auf Parse-Diagnosen geprüft. Beide Backends lehnen die Probe `bb8-141-provides` ab, weil `Object` keine Sprachform hat.

## §7.4.11.4.1.1 Port Type Declaration

**Status:** `partial` — im Modell vorhanden und als `InterfaceKind::PortType` getestet; `Service::is_remote` schliesst `PortType` von jeder DDS-RPC-Bindung aus. Kein Sprachbackend emittiert eine Porttyp-Bindung (die Proben `bb11-173` und `bb11-174` stehen auf `err`), und es gibt keinen Test, der behauptet, dass `ComponentRole::PortTypeRole` das Modell erreicht.

## §7.4.11.4.3 Connectors

**Status:** `partial` — `connector` ist im Modell als `InterfaceKind::Connector` vorhanden; die `port`/`mirrorport`-Mitglieder werden geparst und gelowert, aber die CCM-Umschreibung verwirft sie bewusst (`crates/ccm/src/transform.rs`, Zeile 255 ff., mit dem Kommentar, die Zuordnung über den Porttyp sei nicht §6 dieser Specification). Kein Test behauptet etwas über `ComponentRole::ConnectorRole`.

## §7.4.13.4.1 Void Content

**Status:** `partial` — der leere Struct wird akzeptiert und das Modell trägt ihn; das C-Backend weist ihn ab (`NoLanguageForm`, C23 §6.7.2.1p8 lässt eine Struktur ohne benanntes Mitglied undefiniert). Rust emittiert ihn.

## §7.4.13.4.3.3 Doppelte @position

**Status:** `open` — `check_bitmask_bounds` prüft `@bit_bound`, die Anzahl der Werte und den Bereich von `@position`, aber nicht, dass zwei `@position`-Werte gleich sind. `bitmask B { @position(1) A, @position(1) C };` wird akzeptiert; der spätere Wert überschreibt den früheren im Modell.

## §7.4.13.4.6 Ranges for all Integer Types (Table 7-26)

**Status:** `partial` — die Tabelle `primitive_range` bildet genau die Spalten aus Tabelle 7-26 ab, wird aber nur für Case-Labels eines Diskriminators gelesen. Der Wertebereich einer *Konstante* wird nicht geprüft: `value_disagrees_with` vergleicht nur die Form des Literalwerts, nicht seinen Zahlwert; `const octet K = 300;` und `const int8 K = 200;` gehen durch.

## §7.4.14.3 Rule (216) — template_type_spec als type_spec

**Status:** `open` — die Klausel ist nicht als eigene Form geprüft. Der Pfad, den sie öffnen würde (`struct S { sequence<long> m; };`), ist im heutigen Parser nicht erreichbar: `parse_member` verlangt einen `Declarator`, und `sequence<long> m;` wird als Deklarator gelesen. Der Test `a_sequence_member_carries_its_element_type` prüft die Template-Modul-Form, nicht die anonyme.

## §7.4.14.3 Rule (217) — array_declarator als declarator

**Status:** `partial` — die Array-Dimensionen werden am Deklarator erkannt und dem Deklarator zugeordnet; getestet ist das über den `typedef`-Weg. Die anonyme Form `struct S { long arr[3]; };` hat keinen eigenen Test.

## §7.4.14.4 Verschachtelte Templates und `>>`

**Status:** `partial` — die Verschachtelung funktioniert auch ohne Whitespace (`sequence<sequence<long>>`), was die Klausel nicht fordert. Der umgekehrte Fall, die vom Spec verlangte Schreibweise mit Whitespace (`sequence<sequence<long> >`), wird von keinem Test berührt: der Split-Zweig wird nur für das einzelne `>>`-Token ausgelöst.

## §7.4.15.4.1 Defining Annotations

**Status:** `partial` — das Front-End gibt einem Annotationsmember ohne Default und ohne Wert den Default seines Typs (XTypes 1.3 §7.3.1.2.5); IDL 4.2 verlangt hier einen Wert. Die beiden Specs widersprechen sich (in `internal/spec-coverage/dds-xtypes-1.3-audit-findings.md` geführt).

## §7.4.16 Abhängigkeitsgitter (Figure 7-2)

**Status:** `open` — es gibt im heutigen Baum keine Abbildung dieses Gitters. `crates/ast/src/profile/mod.rs::Profile` ist eine flache Enumeration anderer Specificationen (`Idl42Core`, `XTypes13`, `Corba33`, `Ros2`, `DdsXml`, `FlatData`, `Vendor`), und zwischen den Profilen gibt es keine Kante. Der einzige Ort, an dem die Namen der Bausteine vorkommen, ist die Prosa in `docs/idl-building-blocks.md`.

## §7.5.1 Mehrdeutigkeit

**Status:** `partial` — die Mehrdeutigkeitsmeldung auf Modellebene ist belegt und nennt alle Kandidaten. Das Spec-Beispiel (§7.5.1: `interface C: A, B` mit einem Attribut, dessen Typ in beiden Basen unterschiedlich belegt ist) wird von keinem Test getrieben.

## §7.5.1 Sichtbarkeit durch Inheritance und mehrere globale Namen

**Status:** `open` — es gibt keinen Test, der prüft, dass ein aus einer Basis geerbter Bezeichner unter zwei globalen Namen auflösbar ist und dass zwei Wege zu derselben Deklaration kein Konflikt sind. Die Abflachung in `crates/lang-rules` ist eine Sprachspalte, keine Aussage über die Namen.

## §7.5.2 Einführung eines Bezeichners durch Benutzung

**Status:** `open` — es gibt keine `use`-Unterstützung und keine Gegenprüfung, die den Namen in den einführenden Scope überträgt. Die Erhebung unter `tools/idl-4.2-gap-scan/` führt die Probe `scope-7.5.2-introduced-ident` mit `expect=reject`, gemessen wird aber `ok`; die beiden Gegenproben `scope-7.5.2-not-introduced-ok` und `scope-7.5.2-rooted-no-introduce` gehen heute durch, richtig, aber aus dem falschen Grund, weil die Regel gar nicht existiert.

## §7.5.2 Nur das erste Glied eines qualifizierten Namens wird eingeführt

**Status:** `open` — dieselbe fehlende Regel wie im Item darüber. Die Probe `scope-7.5.2-rooted-no-introduce` geht durch, misst aber nichts: ohne Einführungs-Tracking kann sie nicht fehlschlagen.

## §7.5.3 Special Scoping Rules for Type Names

**Status:** `partial` — die Sperre gilt für zwei Öffnungen desselben Moduls und für die Strukturvererbung. Der in der Klausel genannte Ausnahmefall, die Wiederverwendung in einem abgeleiteten Interface, ist nicht als positiver Fall getestet; die Klausel fordert ihn.

## §7.5.3 Potential Scope

**Status:** `open` — der Begriff Potential Scope hat im Baum keine Entsprechung. `crates/idl-lower/src/scope.rs` führt einen Scope-Stack ohne Verwendungszählung, also ohne die Buchführung, aus der die Regel folgen würde. Auch die positive Note der Klausel (Redefinition nach Benutzung in einem Modul) hat keinen Test.

## §8.2.2 Rules for Using Standardized Annotations

**Status:** `partial` — kein Test prüft die vier Anforderungen des Absatzes an eine Spezifikation, die eine standardisierte Annotation verwendet; die zitierten Tests prüfen nur, dass unbekannte Annotationen ignoriert werden und dass die Schreibweise zählt.

## §8.3.2.2 `@must_understand` Annotation

**Status:** `partial` — das Flag erreicht das Member-Facet; die Bedeutung des Flags (ein Middleware-Empfänger, der die Erweiterung nicht versteht, muss die Nachricht zurückweisen) ist eine XTypes-Anforderung und wird hier nur als Flag geprüft. Das ist der Umfang der Klausel 8.

## §8.3.3.1 `@default` Annotation

**Status:** `partial` — die Annotation ist in der Builtin-Tabelle und der Zweig `default` setzt `XTypesFacet::default_value`. Kein Rust-Test behauptet etwas über dieses Feld; die Erhebung führt die Probe `ann-8.3.3.1-default` mit `ok`, und `crates/idl-lower/tests/annotations.rs` hat keinen Fall dafür.

## §8.3.3.2 `@range` Annotation

**Status:** `partial` — `min`/`max` werden gelesen und ins Facet geschrieben; die Bedingung `max >= min` wird nicht geprüft und hat keinen Test. `@range(5, 1)` wird akzeptiert.

## §8.3.6.2 `@oneway` Annotation

**Status:** `partial` — `@oneway` ist in der Builtin-Tabelle, die Erhebung führt die Probe `ann-8.3.6.2-oneway` mit `ok`. Kein Rust-Test schreibt `@oneway` in eine `.idl`-Zeile und prüft das Facet (`crates/ast/tests/node_space.rs::a_oneway_operation_is_flagged_and_a_normal_one_is_not` prüft die Schlüsselwort-Form, nicht die Annotation-Form). Die Nebenspannung der Klausel — void-Rückgabetyp und keine `out`/`inout`-Parameter — wird nirgends geprüft.

## §9.1 Overview

**Status:** `partial` — die Profilmechanik existiert und ist tabellarisch total geprüft, aber sie beschreibt *andere* Specificationen (`XTypes13`, `Corba33`, `Ros2`, `DdsXml`, `FlatData`, `Vendor`). Die sieben Profile dieser Klausel sind keine davon.

## §9.2.1 Plain CORBA Profile

**Status:** `open` — es gibt keine Abbildung dieses Profils im Baum. Weder `Profile` noch `IdlFeatures` (früher `crates/idl/src/features/mod.rs`, heute entfernt) kennen eine solche Building-Block-Kombination; der einzige Ort, an dem der Name des Profils vorkommt, ist die Prosa in `docs/idl-building-blocks.md`.

## §9.2.2 Minimum CORBA Profile

**Status:** `open` — wie §9.2.1; kein Symbol, kein Test.

## §9.2.3 CCM Profile

**Status:** `partial` — die Bausteine des Profils sind einzeln erkannt und getestet. Das Profil selbst — eine benannte, schaltbare Kombination — gibt es nicht; es gibt keinen Mechanismus, der die Bausteine gemeinsam an- und abschaltet.

## §9.2.4 CCM with Generic Interaction Support Profile

**Status:** `partial` — beide zusätzlichen Bausteine sind implementiert und getestet; die words `GIS` und `Generic Interaction Support` kommen im Frontend nicht vor. Wie bei §9.2.3 gibt es keinen Profilschalter.

## §9.3.1 Plain DDS Profile

**Status:** `open` — kein Profil mit dieser Zusammensetzung. Die beiden Bausteine sind einzeln getestet, die Anonymous Types sind es nicht (siehe §7.4.14.3), und es gibt keine Prüfung, die die beiden zusammen von einem CORBA-Frontend unterscheidet.

## §9.3.2 Extensible DDS Profile

**Status:** `partial` — der XTypes-Anteil des Profils ist als `Profile::XTypes13` modelliert und tabellarisch geprüft; die vier Annotations-Gruppen dieser Klausel (General Purpose, Data Modeling, Data Implementation, Code Generation) existieren als solche nicht, die Annotationen sind eine flache Builtin-Tabelle (§8.1).

## §9.3.3 RPC over DDS Profile

**Status:** `partial` — die Binding-Regel für Interfaces ist implementiert und getestet, einschliesslich der Liste der Sprachen, die sie nicht vollständig abbilden können. Das Profil als schaltbare Bausteinmenge gibt es nicht.

## Annex A Konsolidierte Grammatik (227 Produktionen)

**Status:** `partial` — Annex A ist in `tools/idl-4.2-gap-scan/cases.tsv` enumerate und die Sollwerte in `expected.tsv` festgeschrieben, nicht in einem Rust-Test: 161 der 227 Produktionen tragen eine Probe, 66 keine. Die Messung dieser Zeilen steht in [`idl-4.2.gaps.md`](idl-4.2.gaps.md); der Werkzeuglauf ist `tools/idl-4.2-gap-scan/scan.sh`.
