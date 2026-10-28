# OMG IDL 4.2 — Negativliste

**Spec:** [OMG IDL 4.2](https://www.omg.org/spec/IDL/4.2/), formal/18-01-05,
132 Seiten. PDF im lokalen Cache unter `internal/standards/cache/omg/idl-4.2.pdf`
(gitignored).

**Werkzeug:** `tools/idl-4.2-gap-scan/scan.sh`. **Belege:**
`tools/idl-4.2-gap-scan/cases.tsv` (357 Proben) und
`tools/idl-4.2-gap-scan/expected.tsv` (der aufgezeichnete Stand).

Die übrigen Seiten unter `docs/spec-coverage/` sind Positivlisten: sie
nennen, was betrachtet wurde, und wie weit es kam. Was nie betrachtet
wurde, kann in ihnen nicht stehen, weil ihr Inhalt aus dem Baum stammt
und nicht aus der Spezifikation. Diese Seite ist die andere Richtung. Ihr
Nenner ist Annex A der Spezifikation — alle 227 Grammatikproduktionen —
plus die 24 standardisierten Annotationen aus Klausel 8 und die
normativen Klauseln aus 7.2 (Lexik), 7.3 (Präprozessor), 7.4.13/7.4.15
(Semantik von Bit Sets, Bit Masks und Annotationsanwendung) und 7.5
(Namen und Sichtbarkeit).

Jede Zeile hier hat einen Versuch hinter sich: die IDL, die gefüttert
wurde, und was zurückkam. Keine Zeile steht auf einer Vermutung.

## Wie gemessen wurde

Pro Konstrukt eine minimale `.idl`-Datei, durch alle sechs Unterbefehle
von `zerodds-idlc`. Ein Konstrukt kann in jedem einzeln fehlen, und die
erste Fassung dieser Erhebung hat nur `check` und `generate` gemessen —
die anderen vier standen als Annahme da. Das ist dieselbe Art
Positivliste, gegen die diese Seite angetreten ist, und deshalb ist die
Messung jetzt eine Matrix Probe × Unterbefehl:

| Spalte | Kommando | Frage |
|---|---|---|
| `check` | `zerodds-idlc check` | erkennt das Frontend das Konstrukt |
| `strict` | `zerodds-idlc check --strict` | ändert `--strict` daran etwas |
| `lint` | `zerodds-idlc lint` | kommt es durch die Regeln, die eine gültige Datei noch brechen kann |
| `format` | `zerodds-idlc format` | gibt es eine kanonische Form |
| `trip` | `format`, neu parsen, vergleichen | hält der Round-Trip |
| `dump` | `zerodds-idlc dump-ast` | zeigt das Modell das Konstrukt |
| `deps` | `zerodds-idlc print-deps` | trägt der Abhängigkeitslauf es |
| `rust` | `zerodds-idlc generate --lang rust` | gibt es eine Emission |
| `c` | `zerodds-idlc generate --lang c` | dasselbe im zweiten Backend |

`trip` ist die Spalte gegen den stillen Verlust. Ein Konstrukt, das der
Parser kennt und der Formatierer nicht, verschwindet beim Reformatieren:
keine Fehlermeldung, nur eine Datei, aus der etwas fehlt. Ein Vergleich
der beiden formatierten Texte fängt das nicht — der zweite `format` eines
Textes, der das Konstrukt schon verloren hat, reproduziert den Verlust.
Verglichen wird deshalb das **Modell**: `dump-ast` und `print-deps` der
Quelle gegen `dump-ast` und `print-deps` der formatierten Ausgabe, plus
Idempotenz des Formats. `-` steht, wo es nichts zu vergleichen gibt, weil
`format` oder `check` die Datei abgewiesen hat.

Dazu eine Messung, die nicht am Werkzeug hängt: `fixtures` zählt, wie
viele der 748 committeten `*.idl`-Dateien im Baum das Konstrukt
überhaupt enthalten, per Textmuster (Spalte `witness` in `cases.tsv`).
Das ist schwächer als „es gibt einen Test dafür" — eine Null ist
trotzdem eine Tatsache: keine IDL im Baum trägt das Konstrukt, also kann
nichts im Baum es ausgeübt haben. Ein `-` heißt, dass kein Textmuster das
Konstrukt isoliert; dort ist die Fixture-Frage mit dieser Methode nicht
messbar und steht als ungeprüft.

## Die Zahl

357 Proben. Die Befunde verteilen sich so — F überschneidet sich mit den
übrigen Kategorien, weil dieselbe Probe in `check` einen Befund haben kann
und in `format` einen anderen:

| | Kategorie | Proben |
|---|---|---|
| A | nicht erkannt — das Frontend weist ab | 14 → 13 → **0** |
| B | akzeptiert, obwohl die Spezifikation eine Zurückweisung verlangt | 19 → **2** |
| C | erkannt, nicht emittiert | 12 → 75 → **74** |
| D | emittiert, aber kein `.idl` im Baum enthält es | 33 → 26 → **0** |
| E | `--strict` ohne Wirkung | 4 → **0** |
| F | `format` nimmt an, was `check` abweist | 21 → **0** |
| G | `format` weist ab, was `check` annimmt | 3 → **0** |

A ist leer: die drei Grammatikformen aus A.1 bis A.3 und die acht
Präprozessorformen aus A.5 gehen durch `check`. C ist um 63 Proben
gewachsen, ohne dass etwas übersehen wurde: die C-Spalte lehnt jede
`interface` ab, seit `7b651ac2643` eine Interface-Bindung verweigert, was
sie nicht vollständig abbilden kann, und die Rust-Spalte lehnt den
Objekttyp ab, den ein Port oder Konnektor trägt (§C unten); die Union
ohne `default:` ist aus C heraus (§C). G kam aus A.5 und ist leer:
`format` prüft die Datei wie `check` nach dem Präprozessor (§G).

Auf Produktionsebene, gezählt über die Proben der Kategorie `grammar` in
`cases.tsv` (161 Produktionen tragen eine Probe): 73 haben einen Befund —
keine eine zurückgewiesene Form, 64 eine nicht emittierte, 14 eine ohne
Fixture (mit Überschneidungen). Vor der Neuaufzeichnung waren es nach
derselben Zählung 27 (3, 12, 14). Die Zahlen früherer Fassungen dieser
Seite (227 Produktionen, 37 mit Befund) zählten anders abgegrenzt und
sind mit diesen nicht vergleichbar.

Kategorie B ist keine der vier Kategorien aus dem Auftrag. Sie musste
dazu, weil das Frontend nicht nur übersehen kann, was die Spezifikation
erlaubt, sondern auch, was sie verbietet — und im zweiten Fall entsteht
keine Fehlermeldung, sondern ein Modell mit falschem Inhalt. Drei der 19
Zeilen sind belegt genau das: der deklarierte Typ einer Konstanten
verschwindet und wird durch den Typ des Literals ersetzt.

## Die Matrix

Der Stand je Unterbefehl über alle 357 Proben. Pro Probe steht er in
`tools/idl-4.2-gap-scan/expected.tsv`, eine Zeile je Probe, eine Spalte
je Unterbefehl — das ist der Beleg, den diese Seite zitiert.

| Spalte | `ok` | `err` | `-` |
|---|---|---|---|
| `check` | 324 | 33 | — |
| `strict` | 321 | 36 | — |
| `lint` | 324 | 33 | — |
| `format` | 321 | 36 | — |
| `trip` | 321 | — | 36 |
| `dump` | 324 | 33 | — |
| `deps` | 324 | 33 | — |
| `rust` | 307 | 50 | — |
| `c` | 249 | 108 | — |

`check`, `lint`, `dump` und `deps` stimmen Probe für Probe überein: alle
vier gehen über `analyse()`. `format` steht drei Proben darunter, das ist
G. `strict` steht drei Proben darunter — die drei Vendordirektiven aus E.
`trip` ist überall dort `-`, wo `check` oder `format` die Datei
abgewiesen hat, und `ok` sonst: kein stiller Verlust im Formatierer.

`rust` und `c` stehen darunter, und das ist Kategorie C: 50
beziehungsweise 108 Proben, die das Frontend erkennt und ein Backend
bewusst nicht emittiert.

Die Fixture-Zahlen in den Tabellen dieser Seite sind vom Stand vor der
Neuaufzeichnung, gezählt über 748 Dateien; `expected.tsv` trägt sie über
die 821 Dateien, die der Baum jetzt hat.

---

## A — nicht erkannt

Die Spezifikation erlaubt es, `zerodds-idlc check` weist es ab.

### A.1 `(17) <literal>` — benachbarte String-Literale (geschlossen)

**Jetzt:** `ok` in allen neun Spalten (`5cd950c38f2`). Beleg:
`crates/idl-lower/tests/idl_4_2_grammar.rs`. Der Befund, wie er stand:

**Spec:** §7.2.6.3 — „Adjacent string literals are concatenated.
Characters in concatenated strings are kept distinct."

**IDL:** `const string K = "ab" "cd";`

**Ergebnis:** `error: [IDL0002] expected ';' after the constant` (1:23),
gefolgt von `[IDL0001] expected a declaration`.

**Fixtures:** 0 von 748.

**Probe:** `bb1-017-lit-string-concat`

### A.2 `(64) <type_declarator>` — `<constr_type_dcl>` in einem `typedef` (geschlossen)

**Jetzt:** `ok` in allen neun Spalten (`5cd950c38f2`). Beleg:
`crates/idl-lower/tests/idl_4_2_grammar.rs`. Der Befund, wie er stand:

**Spec:** §7.4.1.4.4.7 „Naming Data Types", Produktion 64 —
`<type_declarator> ::= { <simple_type_spec> | <template_type_spec> | <constr_type_dcl> } <any_declarators>`.
Die dritte Alternative ist die Definition eines Struct, Union oder Enum
direkt im `typedef`.

**IDL:** `typedef struct Inline { long m; } Ali;`

**Ergebnis:** `error: [IDL0001] expected a type` (1:9), drei Folgefehler.

**Fixtures:** 0 von 748.

**Probe:** `bb1-064-typedef-constr`

### A.3 `(112) <export> ::+ …` — `typeid`, `typeprefix` und `import` im Interface-Body (geschlossen)

**Jetzt:** `check` bis `deps` und `rust` `ok` (`5cd950c38f2`); `c` weist
ab wie jedes `interface` (Kategorie C). Beleg:
`crates/idl-lower/tests/idl_4_2_grammar.rs`. Der Befund, wie er stand:

**Spec:** §7.4.6.3, Produktion 112 —
`<export> ::+ <type_id_dcl> ";" | <type_prefix_dcl> ";" | <import_dcl> ";" | <op_oneway_dcl>`.

Auf Dateiebene funktionieren alle drei (Proben `bb6-113-typeid`,
`bb6-114-typeprefix`, `bb6-115-import-scoped`, alle `ok`). Nur die
Erweiterung von `<export>` fehlt, also die Form im Rumpf eines Interface.

**IDL / Ergebnis:**

| IDL | erste Fehlermeldung |
|---|---|
| `interface Ia { typeid Ia "IDL:Ia:2.0"; };` | `[IDL0001] expected a type` (1:16) |
| `interface Ia { typeprefix Ia "ex.org"; };` | `[IDL0001] expected a type` (1:16) |
| `interface Ia { import Mb; };` | `[IDL0001] expected a type` (2:19) |

**Fixtures:** nicht per Textsuche messbar.

**Proben:** `bb6-112-export-typeid`, `bb6-112-export-typeprefix`,
`bb6-112-export-import`

### A.4 §7.4.15.4.2 — unbekannte Annotation mit Bezeichner als Argument (geschlossen)

**Spec:** §7.4.15.4.2 — „in case an unknown annotation is encountered, it
should be ignored by the IDL-processing tools."

Zwei der drei Formen wurden ignoriert, wie die Klausel es verlangt. Die
dritte nicht:

| IDL | vorher | jetzt |
|---|---|---|
| `@nosuchthing struct Sa { long m; };` | 0 Fehler | 0 Fehler |
| `@nosuchthing(v = 1) struct Sa { long m; };` | 0 Fehler | 0 Fehler |
| `@nosuchthing(v = SOME_NAME) struct Sa { long m; };` | `[IDL0010] a name does not resolve: … ["SOME_NAME"]` | **0 Fehler** |

Ein Bezeichner als Argumentwert wurde aufgelöst, obwohl die Annotation,
zu der er gehört, ignoriert werden soll. Damit scheiterte jede Datei mit
`@RTI_topic_kind(NO_KEY)` oder `@allowed_data_representation(XCDR2)` —
die Form, in der Vendorannotationen ihre Enum-Werte schreiben. Das ist
kein Erhebungsfall: es ist die IDL, die Anwender vorlegen.

**Der Fix.** `Lowering::annotation` in
`crates/idl-lower/src/annotation.rs` stellt fest, ob die Annotation
bekannt ist — `BUILTIN_ANNOTATIONS` oder im Scope als `AnnotationDef`
gebunden. Ist sie es nicht, wird ein Argument, dessen Wert ein Bezeichner
ist, nicht als Ausdruck gelowert. Der Mechanismus existierte bereits für
die enumerierten Parameter der Builtins (`@autoid(HASH)`,
`@extensibility(FINAL)`) und trägt jetzt beide Fälle: der Bezeichner
gehört zum Typ der Annotation und steht in keinem Scope der Datei.

Ein **Literal** wird weiter gelowert — `@nosuchthing(v = 1)` behält seine
1. Nur ein Bezeichner hat nichts, worauf er in dieser Datei zeigen
könnte.

**Ein zweiter Befund derselben Familie, mit gefixt.** `@verbatim` fehlte
in der Liste der Builtins mit enumeriertem Parameter, obwohl IDL 4.2
§8.3.5.1 `PlacementKind placement` deklariert. Jedes
`@verbatim(placement = BEFORE_DECLARATION)` im Baum wurde über den
Enumerator abgewiesen. `verbatim_annotation` liest ohnehin alle drei
Parameter aus `Argument::text` und `Argument::identifier`, also geht
nichts verloren.

`crates/idl-parse/tests/fixtures/spec_features/rti_connext_pragmas.idl`
trug beide Defekte in einer Datei und geht jetzt durch `check`.

**Tests:** `tools/idlc/tests/annotation_arguments.rs` — sechs Tests auf
Compose-Ebene, weil die Diagnose aus der Resolve-Stufe kam. Die erste
Fassung stand in `idl-lower` und wäre mit und ohne Fix grün gewesen; eine
Mutation hat das gezeigt.
Mutation: die Bedingung abschalten (3 rot), Literale mit verwerfen
(1 rot), `verbatim` aus der Liste nehmen (2 rot), eine deklarierte
Annotation als unbekannt behandeln (1 rot).

**Probe:** `sem-7.4.15-unknown-annotation-enum-arg` — jetzt `ok`.

### A.5 §7.3 — der Präprozessor ist nicht der aus ISO/IEC 14882:2003 (geschlossen)

**Jetzt:** alle acht Proben `ok` in `check`, `lint`, `dump`, `deps`,
`rust` und `c` (`f05fd8b6991`): funktionsartige Makros, `#`, `##` mit
Platzhaltern, der Rescan mit Hide-Sets, `#if` über einen vollständigen
Konstantenausdruck, `__LINE__`, `__FILE__`, Zeilenfortsetzung. Neuer Code
`IDL0022`. Beleg: `crates/idl-parse/tests/preprocessor_iso_14882.rs`.
`format` weist drei davon ab, siehe G. Der Befund, wie er stand:

**Spec:** §7.3 — „IDL shall be preprocessed according to the
specification of the preprocessor in ISO/IEC 14882:2003."

Sechs Formen dieses Präprozessors fehlen. Zwei davon melden sich selbst
mit eigener Diagnose — das ist die bessere Hälfte des Befunds, weil ein
stiller Ausfall an dieser Stelle Text verschluckt hätte.

| IDL | Ergebnis |
|---|---|
| `#define TWICE(x) ((x) * 2)` + `const long K = TWICE(3);` | `[IDL0002] expected ';' after the constant` (1:22) — funktionsartige Makros werden nicht expandiert |
| `#define CAT(a, b) a##b` + `struct CAT(S, a) { … };` | `[IDL0002] expected '{' opening the struct body` (1:12) — `##` |
| `#define STR(x) #x` + `const string K = STR(ab);` | `[IDL0002] expected ';' after the constant` (2:4) — `#` als Stringize-Operator |
| `#define NN \` + Fortsetzungszeile `4` | `[IDL0001] expected a constant expression` (3:12) — Zeilenfortsetzung mit Backslash |
| `#define AA 1` + `#define BB AA` + `const long K = BB;` | `[IDL0006] macro 'BB' expands to 'AA', which is itself a macro; nested expansion is not performed` |
| `#if 2 * 3 > 5` | `[IDL0006] '#if 2 * 3 > 5' is not a form this preprocessor evaluates` |
| `const long K = __LINE__;` | `[IDL0010] a name does not resolve: … ["_LINE__"]` — vordefinierte Makros gibt es nicht |
| `const string K = __FILE__;` | `[IDL0010] a name does not resolve: … ["_FILE__"]` |

Was funktioniert und hier festgehalten wird, damit es das bleibt:
`#include` in beiden Formen, `#define` ohne Parameter, `#undef`,
`#ifdef`, `#ifndef`, `#if`/`#else`/`#elif`/`#endif` mit einer Konstanten,
`defined(X)`, `#line`, `#pragma`, `#pragma once`.

**Proben:** `lex-7.3-define-function`, `lex-7.3-token-paste`,
`lex-7.3-stringize`, `lex-7.3-line-continuation`, `lex-7.3-nested-macro`,
`lex-7.3-if-arithmetic`, `lex-7.3-predefined-line`,
`lex-7.3-predefined-file`

---

## B — akzeptiert, obwohl die Spezifikation eine Zurückweisung verlangt

Hier gibt es keine Fehlermeldung, sondern ein Ergebnis. In den ersten
drei Zeilen war dieses Ergebnis nachweislich falsch; diese drei sind
geschlossen.

### B.1 §7.4.1.4.3 — der deklarierte Typ einer Konstanten (geschlossen)

**Spec:** §7.4.1.4.3 „Constants" — eine Konstantendeklaration besteht
unter anderem aus „a value expression (`<const_expr>`), which shall be
consistent with the type declared for the constant". §7.2.6.3 —
„Attempts to assign a wide string literal to a non-wide string constant
or to assign a non-wide string literal to a wide string constant result
in a compile-time diagnostic."

Der deklarierte Typ wurde verworfen; der Typ des Literals ersetzte ihn:

| IDL | `check` vorher | erzeugtes Rust vorher | `check` jetzt |
|---|---|---|---|
| `const long K = "x";` | 0 Fehler | `pub const K: &'static str = "x";` | **[IDL0019]** |
| `const string Ss = L"x";` | 0 Fehler | `pub const Ss: &'static str = "x";` | **[IDL0019]** |
| `const wstring Ss = "x";` | 0 Fehler | `pub const Ss: &'static str = "x";` | **[IDL0019]** |

Ein Anwender, der `const long` schrieb und einen `&str` bekam, erfuhr es
vom Rust-Compiler an der Verwendungsstelle, nicht hier.

**Der Fix.** `Lowering::check_const_value_form` in
`crates/idl-lower/src/lower.rs` vergleicht die Form des Wertes mit dem
deklarierten Typ. Der Kommentar an der Stelle behauptete die Regel schon
— „the expression is checked against it rather than used to derive it" —
und es stand keine Prüfung darunter. Neuer Diagnosecode `IDL0019
TypeMismatch`; die Codes werden angehängt, nie umnummeriert.

**Wo die Prüfung schweigt, und warum.** Sie greift nur, wenn der
deklarierte Typ hier zu einem Primitive, `string`/`wstring` oder `fixed`
auflöst **und** der Ausdruck sich auf eine Literalform festlegt. Ein
benannter Typ, ein Alias, eine Enumeration, eine Referenz auf eine andere
Konstante und eine ungelöste Referenz gehen wortlos durch. Eine
Zurückweisung, die zu weit greift, bricht gültiges IDL — teurer als der
Fehler, den sie behebt. Gemessen: keine der 748 committeten
`*.idl`-Dateien meldet IDL0019.

**Tests:** `crates/idl-lower/tests/validation.rs` —
`a_constant_whose_value_is_not_of_its_declared_type_is_refused`,
`the_wide_and_narrow_string_pair_is_the_case_the_clause_names` und
`every_form_the_specification_admits_still_gets_through` (28 Formen, die
weiter durchgehen müssen). Mutation: `if admitted || true` macht die
ersten beiden rot, `_ => (false, …)` statt `_ => return` den dritten.

**Proben:** `scope-7.4-const-type-mismatch`,
`scope-7.2.6.3-wide-to-narrow`, `scope-7.2.6.3-narrow-to-wide` — alle
drei jetzt `err` in allen neun Spalten.

### B.2 §7.5.2 — drei Sichtbarkeitsregeln (eine geschlossen, zwei bewusst offen)

**Spec:** §7.5.2 — „The name of a module, structure, union, map,
interface, value type, event type, exception or home may not be redefined
within the immediate scope of the module, …". Die Spezifikation gibt
beide Fälle als Beispiel mit dem Kommentar `// Error`.

| IDL | Spec sagt | vorher | jetzt |
|---|---|---|---|
| `module Ma { typedef short Ma; };` | Error (Beispiel §7.5.2) | 0 Fehler | **[IDL0008]** |
| `interface Ia { void ia(in short jj); };` | Error (Beispiel §7.5.2) | 0 Fehler | 0 Fehler — offen |
| `module Inner2 { typedef Inner1::Ss1 Ss2; typedef string inner1; };` | Error (Beispiel §7.5.2) | 0 Fehler | 0 Fehler — offen |

**Die erste ist geschlossen.** `check_redefines_enclosing_scope` in
`crates/idl-lower/src/lower.rs`, gerufen aus `check_collision`, vergleicht
den Namen einer Deklaration mit dem Namen des Scopes, in dem sie steht —
case-insensitiv, weil §7.2.3 jeden Bezeichnervergleich in IDL so macht.

**Nur der unmittelbare Scope**, und das ist die ganze Sicherheit der
Regel: `module Ma { module Mb { typedef short Ma; }; };` bleibt zulässig,
weil `Ma` nicht der Name des Scopes `Mb` ist. Der Unterschied zwischen
dieser Regel und einer, die gültiges IDL bricht, liegt genau darin.

Die Regel bestätigt eine Entscheidung, die der Baum schon getroffen
hatte: Commit `aab2fe27404` hielt fest, dass
`module common { struct common { … } }` kein zulässiges IDL ist, und
korrigierte Kommentare in den Java- und TypeScript-Adaptern, die das
Gegenteil behaupteten. Der Interop-Fall
`tests/interop-corpus/resolve-module-named-as-type/type.idl` misst, welche
Vendoren ihn abweisen; er steht jetzt auf der Ausnahmeliste des
Korpus-Tests.

**Zwei bleiben offen, und zwar begründet.**

- **Die Operation, die mit dem Interface-Namen kollidiert.** §7.5.2 zählt
  für die Redefinitionsregel „module, structure, union, map, interface,
  value type, event type, exception or home" auf — Operationen stehen
  nicht darin; der Fall stammt aus dem Introduction-Beispiel. Ob die
  Regel für jeden `<export>` gilt oder nur für Operationen, ist
  Auslegung, und zu weit greifen bricht CORBA-Interfaces.
- **Die „identifier introduction"-Regel.** Sie verlangt, jede
  Namensbenutzung pro Scope case-insensitiv zu verfolgen. Das ist das
  höchste Über-Reichweiten-Risiko der ganzen Liste. Die beiden
  Gegenproben (`scope-7.5.2-not-introduced-ok`,
  `scope-7.5.2-rooted-no-introduce`) gehen heute durch — richtig, aber
  aus dem falschen Grund, weil die Regel gar nicht existiert. Eine
  Umsetzung braucht deutlich mehr Gegenproben als diese zwei.

Was §7.5.2 sonst **hält**: Modul-Wiedereröffnung, Redefinition in einem
verschachtelten Scope, `enum`-Namen im umgebenden Scope
(`scope-7.5.2-enum-name-in-scope` wird korrekt abgewiesen), und die
Case-Kollisionsregel aus §7.2.3, die sogar Bezeichner gegen Keywords
prüft.

**Tests:** `crates/idl-lower/tests/validation.rs` —
`a_declaration_may_not_reuse_the_name_of_its_own_scope` und
`only_the_immediate_scope_is_compared`.
Mutation: die Bedingung abschalten (1 rot); den Scope-Namen durch den
Deklarationsnamen ersetzen, also jede Deklaration treffen (33 rot).

**Proben:** `scope-7.5.2-redefine-own-name` — jetzt `err`;
`scope-7.5.2-op-clashes-iface` und `scope-7.5.2-introduced-ident` bleiben
`ok`.

### B.3 §7.4.1.4.4 — direkte Rekursion (geschlossen)

**Spec:** §7.4.1.4.4.4.4 — eine Struktur darf sich nur über eine Sequence
(oder ein `@optional`-Member, oder eine Map) selbst enthalten.

**IDL:** `struct Sa { Sa m; };`

**Vorher:** `check` meldete 0 Fehler. `generate` meldete
`SpecRefuses { detail: "NodeId(3) closes a type cycle with nothing on it
that terminates a value — no sequence, no map and no @optional member —
which IDL 4.2 §7.4.1.4.4.4.4 does not admit; …" }`.

Die Regel war also da und zitierte die Klausel; sie saß nur in der
Layout-Stufe statt im Frontend. Wer `check` benutzte, um zu erfahren, ob
eine Datei gültiges IDL ist, erfuhr es nicht.

**Der Fix — herausgelöst, nicht nachgebaut.**
`zerodds_ast_layout::types_that_close_an_inadmissible_cycle` fährt
denselben `plan`-Lauf und behält genau diesen einen Fehler. Es ist
ausdrücklich **keine** zweite Implementierung des Zyklustests: die
Barrieren-Buchführung — ein Zyklus ist genau dann zulässig, wenn eine
Sequence, eine Map oder ein `@optional`-Member auf ihm liegt — lebt im
Abstieg, und eine zweimal geschriebene Regel driftet. Jeder andere
`LayoutError` wird verworfen; eine ungelöste Referenz meldet die
Resolve-Stufe, und eine Kodierung, die einen Typ nicht trägt, ist
`generate`s Sache.

Aufgerufen wird sie in `zerodds_idl_compose::compose_source_with`, nach
der Resolve-Stufe (ein Zyklus ist eine Eigenschaft des aufgelösten
Typgraphen). Damit bekommt jeder Aufrufer des Frontends dieselbe Antwort
— `check`, ein `build.rs` über `zerodds-build`, eine Wire-Probe — statt
nur die CLI. Das Verhalten von `ast-layout` ist unverändert.

Die Meldung nennt den Typ mit vollem Namen. Die der Layout-Stufe nannte
eine `NodeId`, was außerhalb eines Debuggers schlechter ist als nichts.

**Zulässige Rekursion bleibt zulässig,** und das ist der teurere Teil der
Regel: `sequence<Sa>`, `sequence<Sa, 4>`, `@optional Sa`, `map<long, Sa>`
und gegenseitige Rekursion über Sequences gehen weiter durch. Der
Interop-Korpus hält sieben Rekursionsfälle; genau einer —
`recursive-array` — ist die unzulässige Form, und der Test liest alle
sieben von der Platte statt sie nachzuschreiben.

**Tests:** `crates/idl-compose/tests/recursion.rs`, vier Tests.
Mutation: den Aufruf auskommentieren (3 rot), im Extraktor den falschen
`LayoutError` behalten (3 rot).

**Probe:** `scope-7.4-recursive-struct` — jetzt `err`.

### B.4 §7.4.13 — sieben Schranken an Bit Sets und Bit Masks (geschlossen)

**Spec:** §7.4.13.4.3.2 — „The first one (`<positive_int_const>`) is the
number of bits that can be stored (its size). The maximum value is 64";
„When the destination type is given, the number of stored bits cannot
exceed its size (i.e., 1 for boolean, 8 for octet, …)". §7.4.13.4.3.3 —
„A bit mask must contain at least one value and no more than its size
expressed in bits"; „By default, the size of a bit mask is 32";
`@bit_bound` „must be lower than or equal to 64. Accordingly the number
of values cannot then exceed the value given to @bit_bound";
`@position` „Possible positions range from 0 … up to (size – 1)".

| IDL | vorher | jetzt |
|---|---|---|
| `bitset Ba { bitfield<65> f1; };` | `check` 0 Fehler; erst der C-Writer verweigert: `NoWriter { … "bitset wider than the sixty-four bits an anchor holds" }` | **[IDL0014]** |
| `bitset Ba { bitfield<9, octet> f1; };` | 0 Fehler, Emission in beiden Sprachen | **[IDL0014]** |
| `bitset Ba { bitfield<2, boolean> f1; };` | 0 Fehler, Emission in beiden Sprachen | **[IDL0014]** |
| `@bit_bound(65) bitmask Ba { F1 };` | 0 Fehler | **[IDL0014]** |
| `@bit_bound(2) bitmask Ba { F1, F2, F3 };` | 0 Fehler | **[IDL0014]** |
| `bitmask Ba { F01 … F33 };` (33 Werte, Default-Größe 32) | 0 Fehler | **[IDL0014]** |
| `@bit_bound(2) bitmask Ba { @position(5) F1 };` | 0 Fehler | **[IDL0014]** |

Die eine Schranke, die das Frontend schon hielt, ist die untere:
`bitmask Ba { };` wird mit `[IDL0002] a 'bitmask' needs at least one bit
value` abgewiesen.

**Der Fix.** `Lowering::check_bit_bounds` in
`crates/idl-lower/src/complete.rs`, aus `complete()` gerufen und nicht
aus der Deklaration: `@bit_bound` und `@position` sind Annotationen und
sind erst angewandt, wenn die Datei fertig gelowert ist. Diagnosecode
`IDL0014 InvalidBound`, der vorhandene Code für eine Schranke außerhalb
ihres zulässigen Bereichs.

Die Zielbreiten sind die, die §7.4.13.4.3.2 nennt: 1 für `boolean`, 8 für
`octet`/`int8`/`uint8`, 16, 32, 64. Ein Zieltyp, der hier nicht auflöst
oder keiner dieser Typen ist, führt zu keiner Meldung — dieselbe Regel
wie in B.1: schweigen, wo die Antwort nicht feststeht.

**Eine Datei im Baum trifft es.**
`tools/wire-oracle/cases/bitmask_past_bound/type.idl` schreibt
`@bit_bound(4)` mit `@position(7)`, absichtlich: der Fall misst, was
fremde Generatoren mit einer Position außerhalb des Bereichs tun. Er wird
von `tools/byte-gate/gate.sh` nur an Cyclone, Fast DDS, OpenDDS und RTI
gegeben — die ZeroDDS-Seite kompiliert ihn nicht —, also bleibt die
Messung, die er trägt, unverändert. Kein Test im Baum fährt `check` über
alle committeten `.idl`-Dateien.

**Tests:** `crates/idl-lower/tests/validation.rs` — fünf Tests, einer je
Regel, plus `the_bit_forms_the_specification_admits_still_get_through`
mit neun Formen, die weiter durchgehen müssen.
Mutation: `if false && …` vor jeder der vier Vergleiche macht je einen
Test rot; `MAX_BITFIELD_BITS`/`MAX_BIT_BOUND` von 64 auf 63 macht drei
rot.

**Proben:** `sem-7.4.13-bitfield-over-64`,
`sem-7.4.13-bitfield-dest-octet`, `sem-7.4.13-bitfield-dest-bool`,
`sem-7.4.13-bitbound-over-64`, `sem-7.4.13-bitmask-over-bound`,
`sem-7.4.13-bitmask-over-default`, `sem-7.4.13-position-out-of-range` —
alle sieben jetzt `err`.

### B.5 §7.4.15.4.2 — vier Regeln zur Annotationsanwendung (geschlossen)

**Spec:** §7.4.15.4.2 — „Members with no default value must be given a
value"; „A constant expression, whose type must be compatible with the
member's declaration"; „In case there is only one member, the annotation
application may be shortened to the name of the annotation prefixed with
@ and followed with the constant value of that unique member"; und
schließlich „malformed definitions shall be treated as an error".

| IDL | vorher | jetzt |
|---|---|---|
| `@annotation Aa { long v; };` + `@Aa struct …` — Pflichtmember ohne Wert | 0 Fehler | **[IDL0007]** |
| `@Aa("x")` auf `long v default 0` — Typ passt nicht | 0 Fehler | **[IDL0007]** |
| `@Aa(nosuch = 1)` — Membername existiert nicht | 0 Fehler | **[IDL0007]** |
| `@Aa(1)` auf eine Annotation mit zwei Membern | 0 Fehler | **[IDL0007]** |

**Der Defekt darunter war größer als die vier Regeln.** Keine der vier
konnte greifen, weil eine Annotation ihre Member gar nicht im Modell
hatte. Regel (222) schreibt den Namen eines Annotationsmembers als
`<simple_declarator>`, und `parse_annotation_decl` liest ihn mit einem
blanken `expect(Ident)` ohne `Declarator`-Knoten. `Lowering::declared`
läuft über `Declarator`-Kinder, fand keine, gab eine leere Liste zurück —
und jeder Member jeder Annotation verschwand ohne ein Wort.
`@annotation Aa { long v; };` erreichte das Modell mit null Membern.

`annotation_decl` liest den Namen jetzt direkt (`first_ident` liest die
eigenen Tokens des Knotens, nicht die seiner Nachfahren, also ist der
Name von `Ea v;` gleich `v` und nicht `Ea`) und meldet einen Member ohne
Namen, statt ihn fallen zu lassen.

**Und der Rumpf ist ein eigener Scope.** Regel (221) lässt neben den
Membern ein `enum`, ein `const` und ein `typedef` zu —
`@annotation Mode { enum Kind { READ, WRITE }; Kind kind default READ; };`.
Die wurden geparst und nie gelowert, was niemandem auffiel, solange die
Member selbst verschwanden. Mit gelesenen Membern hatte der Typ eines
Members nichts, worauf er auflösen konnte:
`crates/idl-parse/tests/fixtures/spec_features/user_annotations.idl` fiel
sofort um. `annotation_decl` schiebt jetzt einen Scope und lowert die
drei zulässigen Formen darin.

**Die Grenze zur Ignorier-Regel ist die der Klausel selbst.** Dieselbe
§7.4.15.4.2 verlangt, eine *unbekannte* Annotation zu ignorieren. Keine
der vier Regeln erreicht deshalb eine Annotation, die die Datei nicht
deklariert hat: `check_annotation_application` wird nur mit einer
aufgelösten Deklaration gerufen. `@RTI_topic_kind(NO_KEY)` bleibt
unberührt.

**Die Typprüfung ist dieselbe wie in B.1.**
`Lowering::value_disagrees_with` beantwortet beide Klauseln — §7.4.1.4.3
für den Wert einer Konstanten und §7.4.15.4.2 für ein Annotationsargument.
Eine Lesart, damit die beiden nicht auseinanderlaufen.

**Offen geblieben, weil nichts es entscheidet:** ob ein Enumerator, der
im Rumpf einer Annotation deklariert ist, an einer Anwendungsstelle
unqualifiziert geschrieben werden darf — `@Mode(WRITE)`. Weder §7.5 noch
Regel (221) sagen es, und keine Fixture im Baum übt es. Es steht deshalb
keine Behauptung darüber im Test.

**Tests:** `crates/idl-lower/tests/validation.rs` — fünf Tests, einer je
Regel plus `none_of_the_four_rules_reaches_an_unknown_annotation` und
`an_annotations_declared_members_reach_the_model`;
`tools/idlc/tests/annotation_arguments.rs` für den Rumpf-Scope.
Mutation: jede der vier Bedingungen einzeln abschalten (je 1 rot), den
Aufruf ganz entfernen (4 rot).

**Proben:** `sem-7.4.15-missing-required`,
`sem-7.4.15-param-type-mismatch`, `sem-7.4.15-unknown-param-name`,
`sem-7.4.15-shorthand-two-members` — alle vier jetzt `err`.
`sem-7.4.15-unknown-annotation` und `-unknown-annotation-args` bleiben
`ok`; sie sind die Gegenproben und kein Befund.

### B.6 §7.3 — `#error` (geschlossen)

**Spec:** IDL 4.2 §7.3 übernimmt den Präprozessor aus ISO/IEC
14882:2003. Dessen §16.5: „#error causes the implementation to produce a
diagnostic message that includes the specified sequence of preprocessing
tokens."

**IDL:** `#error stop here`

**Vorher:** `check` meldete 0 Fehler; `generate --lang rust` schrieb eine
Datei ohne Inhalt. Die Direktive wurde still übergangen — die eine
Direktive, die es gibt, um einen Build anzuhalten, war die einzige, die
das nicht konnte.

**Jetzt:** `[IDL0020]` mit den Tokens in der Meldung, wie die Klausel es
verlangt. Neuer Diagnosecode, angehängt; `IDL0019` bleibt `TypeMismatch`,
weil Codes nie umnummeriert werden.

**Die einzige Feinheit der Regel** ist `if keeping`: ein `#error` in
einem Zweig, den die Konditionale herausgeschnitten hat, ist Text, der
nie erreicht wurde. Ihn zu melden würde jede Datei abweisen, die eines
absichert — also fast jede, die überhaupt eines hat.

**Tests:** `crates/idl-parse/tests/preprocess.rs` — drei Tests, darunter
`an_error_directive_in_a_branch_that_is_cut_out_says_nothing` über
`#ifdef`, `#else` und `#if 0`.
Mutation: die Direktive nicht melden (2 rot); `if keeping` entfernen, also
auch im toten Zweig melden (1 rot).

**Probe:** `lex-7.3-error-directive` — jetzt `err`.

---

## C — erkannt, nicht emittiert

Alle elf Zeilen sind **bewusst abgelehnt**: das Backend nennt in jedem
Fall den Grund und, wo es einen gibt, die Klausel. Das ist keine Lücke im
Sinne von „übersehen", muss aber unterscheidbar sein, weil eine
Anwendung, die das Konstrukt benutzt, nichts bekommt.

| Konstrukt | Produktion | Rust | C | Grund, wie ihn das Werkzeug nennt | Fixtures |
|---|---|---|---|---|---|
| `any` als Member | 69, 70 | — | — | `NoLanguageForm { construct: "any" }`, in C zusätzlich `NoWriter { … "'any', which DDS-XTypes has no encoding for; only CORBA CDR writes one, as a TypeCode ahead of the value" }` | 9 |
| `typedef any` | 70 | — | — | dito | 1 |
| `any` als Annotationsmember | 223, 224 | ok | — | wie oben | – |
| `Object` | 117, 118 | — | — | `NoLanguageForm { construct: "Object" }` | 4 |
| `ValueBase` | 131, 132 | — | — | `NoLanguageForm { construct: "ValueBase" }` | 1 |
| `fixed<9,2>` | 42 | — | ok | `NoWriter { member: "m", kind: "fixed" }` | 17 |
| `valuetype Va string;` (Value Box) | 125, 126 | ok | — | `NoWriter { … "boxed value, whose presence flag has no settled wire form" }` | 1 |
| `abstract valuetype` | 125, 127 | ok | — | `NoLanguageForm { … "an aggregate with no members — C23 §6.7.2.1p8 leaves a structure with no named member undefined" }` | 0 |
| `abstract eventtype` (Definition) | 166, 168 | ok | — | dito | – |
| `abstract eventtype` (Forward + Definition) | 167 | ok | — | dito | 1 |
| `struct Sa { };` (leerer Struct, BB Extended Data-Types) | 195 | ok | — | dito | 3 |

**Geschlossen:** die Union ohne `default:`, deren Default-Diskriminator
keinen Zweig trifft (Probe `bb1-050-union-no-default`, Produktionen 49,
50), ist in der Rust-Spalte `ok`. Das `enum` der Union hat die Variante
`_NoMember(<Diskriminator>)` für einen Diskriminator, der kein Member
wählt (IDL 4.2 §7.4.1.4.4.4), und eine Variante, deren Fall den
Diskriminator nicht festlegt (mehrere Labels, `default:`), trägt ihn vor
dem Member. Test:
`crates/lang-rules/tests/rust_corpus_compiles.rs::a_union_keeps_its_discriminator_through_rustc_and_back`.

Die letzten vier hängen an einer einzigen Ursache: C hat keine Form für
einen Aggregat-Typ ohne Member, und `abstract valuetype`,
`abstract eventtype` und der leere Struct aus Produktion 195 laufen alle
darauf zu. Rust emittiert alle vier.

Die 17 Fixtures mit `fixed<…>` und die 9 mit `any` sind der Hinweis, dass
diese beiden Konstrukte im Baum vorkommen, aber nicht durch die
Rust-Emission gehen.

### C.2 Die Interface-Familie in C, der Objekttyp an Ports und Konnektoren in Rust

63 weitere Proben, alle mit Begründung abgelehnt:

- **C, 62 Proben** — jede Probe mit einem `interface`, auch einer, die
  nur an einer Komponente, einem Home oder einem Porttyp hängt: „C has no
  complete DDS-RPC 1.0 binding for an interface, and this model
  declares 1 …", mit der Liste der Spalten, die es können. Seit
  `7b651ac2643` verweigert eine Interface-Bindung, was sie nicht
  vollständig abbilden kann, statt einen Teil zu erzeugen. Proben:
  `bb3-074` bis `bb3-095`, `bb4-097-export-*`, `bb5-103-value-supports`,
  `bb6-113-typeid`, `bb6-114-typeprefix`, `bb6-119-local-interface`,
  `bb6-120-oneway`, `bb6-123-op-with-context`, `bb7-129`, `bb7-130`,
  `bb8-135` bis `bb8-143`, `bb9-145` bis `bb9-150`, `bb10-155` bis
  `bb10-165`, `bb11-173` bis `bb11-182`, `ann-8.3.6.1-service*`,
  `ann-8.3.6.2-oneway`, `ann-8.3.6.3-ami*`,
  `scope-7.5.2-op-clashes-iface`, `scope-7.5.3-type-name-lookup`.
- **Rust, 12 Proben** — ein Port (`provides`, `uses`, `port`,
  `mirrorport`), ein Porttyp oder Konnektor hält eine Objektreferenz, und
  die Rust-Spalte hat für `Object` keine Form:
  `NoLanguageForm { construct: "Object", language: "Rust" }`. Proben:
  `bb8-141-provides`, `bb8-143-uses`, `bb10-157-interface-type-obj`,
  `bb10-158-uses-multiple`, `bb11-173` bis `bb11-182`.

Nicht bestimmt: welcher Commit nach dem letzten Aufzeichnen die zwölf
Rust-Proben von `ok` auf `err` gebracht hat; die Ablehnung von `Object`
selbst besteht länger.

---

## G — `format` weist ab, was `check` annimmt

`zerodds-idlc format` liest den Tokenstrom vor dem Präprozessor, damit
Kommentare und Direktiven erhalten bleiben. Ein objektartiges Makro steht
dort als Bezeichner und ist gültige Syntax; der Aufruf eines
funktionsartigen Makros nicht. `format` weist die Datei deshalb ab —
mit Diagnose, ohne etwas zu verlieren —, während `check` sie nach der
Expansion annimmt.

| IDL | `format` |
|---|---|
| `#define TWICE(x) ((x) * 2)` + `const long K = TWICE(3);` | `[IDL0002] expected ';' after the constant` |

**Proben:** `lex-7.3-define-function`, `lex-7.3-token-paste`,
`lex-7.3-stringize`.

**Geschlossen:** `zerodds-idlc format` entscheidet die Gültigkeit wie
`check`, nach dem Präprozessor, und legt den Text danach über die Tokens
der Quelle aus (`format::canonical_of_checked`); ein Makroaufruf bleibt
die Tokenfolge, als die er geschrieben ist. Die drei Proben sind in
`format` und im Rundlauf `trip` jetzt `ok`. Test:
`tools/idlc/src/format.rs::tests::a_function_like_macro_call_keeps_its_tokens`.

---

## D — emittiert, aber kein `.idl` im Baum enthält es (geschlossen)

Beide Backends erzeugen Quelltext, und keine der committeten
`*.idl`-Dateien trug das Konstrukt. Nach der Neuaufzeichnung waren es
26. Jetzt hat jedes davon eine Fixture unter
`tools/idlc/tests/idl-4.2-gaps/`, eine je Probe und nach ihr benannt, und
`tools/idlc/tests/idl_4_2_gaps.rs` fährt sie durch `zerodds-idlc check`
und `zerodds-idlc generate --lang rust` und `--lang c`. Die Fixtures
tragen die IDL der Probe, damit der Text die Fixture-Frage beantwortet,
die `scan.sh` mit der `witness`-Spalte stellt.

Zuvor herausgefallen waren elf: `@range`, `@min`, `@max`, `@unit`,
`const octet` und `union Ua;` hatten eine Fixture bekommen; `@ami`,
`interface Ia;`, `interface Ia : Ib`, `local interface` und `context`
emittiert die C-Spalte nicht mehr (C.2). Vier waren hinzugekommen, weil
sie vorher nicht durch `check` kamen: benachbarte String-Literale,
`typedef struct … Ali;`, das funktionsartige Makro und `##`.

Die 26, die die Fixture verlassen haben:

| Konstrukt | Produktion / Klausel | Probe |
|---|---|---|
| `const fixed` | 6, 43 | `bb1-006-const-fixed` |
| `const wchar` | 6 | `bb1-006-const-wchar` |
| `const wstring` | 6 | `bb1-006-const-wstring` |
| `^` im konstanten Ausdruck | 9 | `bb1-009-xor-expr` |
| `>>` im konstanten Ausdruck | 11 | `bb1-011-shift-right` |
| `~` als unärer Operator | 15 | `bb1-015-unary-tilde` |
| Fixed-Point-Literal | 17 | `bb1-017-lit-fixed` |
| Wide-String-Literal | 17 | `bb1-017-lit-wstring` |
| benachbarte String-Literale | 17 | `bb1-017-lit-string-concat` |
| `typedef struct … Ali;` | 64 | `bb1-064-typedef-constr` |
| `valuetype Va;` (Forward) | 99, 110 | `bb5-110-value-forward` |
| `int64` | 212 | `bb13-212-int64` |
| oktaler Zeichen-Escape | 7.2.6.2 | `lex-7.2.6.2-char-escape-octal` |
| hexadezimaler Zeichen-Escape | 7.2.6.2 | `lex-7.2.6.2-char-escape-hex` |
| Unicode-Escape | 7.2.6.2 | `lex-7.2.6.2-wchar-escape-unicode` |
| Gleitkomma mit führendem Punkt | 7.2.6.4 | `lex-7.2.6.4-float-leading-dot` |
| Gleitkomma mit nachgestelltem Punkt | 7.2.6.4 | `lex-7.2.6.4-float-trailing-dot` |
| Fixed-Point-Literal | 7.2.6.5 | `lex-7.2.6.5-fixed-literal` |
| funktionsartiges Makro | 7.3 | `lex-7.3-define-function` |
| `#ifndef` | 7.3 | `lex-7.3-ifndef` |
| `#if`/`#else` | 7.3 | `lex-7.3-if-else` |
| `#elif` | 7.3 | `lex-7.3-elif` |
| `defined(X)` | 7.3 | `lex-7.3-defined-operator` |
| `#undef` | 7.3 | `lex-7.3-undef` |
| `##` | 7.3 | `lex-7.3-token-paste` |
| `#line` | 7.3 | `lex-7.3-line-directive` |

Drei davon waren mit der `witness`-Suche vorher nicht messbar: die
Muster der beiden Zeichen-Escapes und des Unicode-Escapes in
`cases.tsv` waren über-escaped (`'\\\\[0-7]`, `'\\\\x`, `\\\\u…`) und
trafen nicht einmal die IDL der eigenen Probe. Sie stehen jetzt als
`'\\[0-7]`, `'\\x` und `\\u[0-9a-fA-F]` im Korpus.

---

## E — `--strict` (geschlossen)

`tools/idlc/README.md` und `zerodds-idlc --help` beschreiben `--strict`
als „OMG IDL 4.2 only; reject every vendor extension". Der Schalter wurde
geparst und dann verworfen — `tools/idlc/src/main.rs`, `analyse()`:
`let _ = options.strict;` — und `Parser::over()` verdrahtete
`Dialect::ALL_VENDORS` fest. Über alle Proben gab es keine einzige, bei
der `check` und `check --strict` verschiedene Ergebnisse lieferten.

**Der Weg, den der Dialekt jetzt nimmt.** `Options::dialect()` in
`tools/idlc/src/main.rs` ist die eine Stelle, an der `--strict` gelesen
wird; jeder der sechs Unterbefehle geht durch sie — fünf über
`analyse()`, `format` über `format::canonical_under`. Der Dialekt
erreicht beide Stufen, in denen Vendorkonstrukte sitzen:
`zerodds_idl_parse::parse_preprocessed_with` für die Grammatik und
`zerodds_idl_lower::Options::dialect` für die Pragmas.

**Gemessen, nach dem Fix:**

| IDL | Vendor | `check` | `check --strict` |
|---|---|---|---|
| `keylist Sa (m);` | RTI Connext | 0 Fehler, 1 Warnung | **1 Fehler** |
| `#pragma keylist Sa m` | Cyclone DDS | 0 Fehler, 1 Warnung | **1 Fehler** |
| `#pragma DCPS_DATA_TYPE "Sa"` | OpenDDS | 0 Fehler, 1 Warnung | **1 Fehler** |
| `#pragma DCPS_DATA_KEY Sa m` | OpenDDS | 0 Fehler, 1 Warnung | **1 Fehler** |
| `@Key` (Fast-DDS-Schreibweise) | — | 0 Fehler | 0 Fehler |

Die drei Direktiven sind die, auf die das Frontend **handelt**: alle drei
machen Member zu Keys, also baut ein Frontend, das sie ignoriert, aus
derselben Datei einen anderen Typ. Genau das ist die Frage, die
`--strict` beantwortet.

**`@Key` bleibt zulässig, und das ist kein Rest der Lücke.** IDL 4.2
§7.4.15.4.2: „in case an unknown annotation is encountered, it should be
ignored by the IDL-processing tools." `crates/idl-lower/src/annotation.rs`
erkennt Annotationen case-sensitiv, `@Key` ist deshalb keine bekannte
Annotation, das Frontend handelt nicht darauf, und eine Zurückweisung
unter `--strict` bräche gültiges IDL, um nichts zu gaten. Dasselbe gilt
für `@RTI_*` und `@RTPS_*`. Wäre `@Key` als Key gelesen worden, stünde er
in der Tabelle oben.

**Tests:** `tools/idlc/tests/strict.rs` — die Matrix aus sechs
Unterbefehlen × vier Vendorerweiterungen × zwei Dialekten, plus die
Gegenprobe, dass dieselben Dateien ohne `--strict` durch jeden
Unterbefehl gehen; `crates/idl-lower/tests/dialects.rs`,
`strict_rejects_every_vendor_extension_the_front_end_acts_on` und
`strict_does_not_refuse_an_unknown_annotation`.

**Proben:** `dialect-rti-keylist`, `dialect-cyclone-pragma-keylist`,
`dialect-opendds-pragma-datatype` — `strict` jetzt `err`;
`dialect-fastdds-key-capital` bleibt `ok`.

**Eine Probe war falsch gebaut.** `lex-7.3-pragma` stand für §7.3, also
dafür, dass der Präprozessor `#pragma` überhaupt verarbeitet — und
benutzte dafür `#pragma keylist Sa m`, eine Cyclone-Direktive. Damit maß
sie zwei Achsen zugleich und wurde unter `--strict` aus einem Grund rot,
der mit §7.3 nichts zu tun hat. Sie schreibt jetzt `#pragma vendorless
Sa`.

---

## F — `format` nahm an, was `check` abwies (geschlossen)

Kein Befund über ein Konstrukt, sondern über einen Unterbefehl. Er fiel
erst auf, als die Erhebung von zwei Spalten auf die Matrix über alle
sechs erweitert wurde.

`zerodds-idlc format` lief nicht über dieselbe Pipeline wie `check`.
`tools/idlc/src/format.rs`, `canonical()`, ruft
`zerodds_idl_parse::parse(source)` — den nackten Parser. Kein
Präprozessor, keine Lowering-Stufe, keine Resolve-Stufe. Der Formatierer
wies eine Datei ab, die nicht *parste*; alles, was erst danach auffiel,
ging klaglos durch. Gemessen: **21 der 356 Proben** hatten `check err`
und `format ok`, und über den committeten Baum 12 der 748
`*.idl`-Dateien. Der Nutzer bekam eine formatierte Datei zurück, die sein
Werkzeug im nächsten Schritt verwarf.

| Stufe, die den Fehler meldet | Proben |
|---|---|
| Präprozessor | `lex-7.3-line-continuation`, `lex-7.3-nested-macro`, `lex-7.3-if-arithmetic`, `lex-7.3-predefined-line`, `lex-7.3-predefined-file` |
| Lowering (Lexik/Scoping) | `lex-7.2.3-case-collision`, `lex-7.2.3-keyword-collision`, `scope-7.5.2-enum-name-in-scope`, `scope-7.4-dup-case-label`, `scope-7.4-two-defaults`, `scope-7.4-enum-dup` |
| Lowering (Semantik) | `sem-7.4.13-bitmask-empty`, `sem-7.4.1-seq-bound-zero`, `sem-7.4.1-string-bound-zero`, `sem-7.4.1-array-size-zero`, `sem-7.4.1-array-size-negative`, `sem-8.3.1.1-id-collision`, `sem-8.3.1.2-autoid-bad-arg`, `sem-8.3.1.6-ext-bad-arg`, `sem-8.3.1.3-key-and-optional` |
| Resolve | `scope-7.4-unknown-type` |

**Der Fix.** `run_format` in `tools/idlc/src/main.rs` läuft jetzt zuerst
durch `analyse()` — dieselbe Pipeline, die `check` fährt — und weist die
Datei ab, sobald ein Fehler gemeldet wird. Auf dem Erfolgspfad wird
nichts ausgegeben: der formatierte Text geht nach stdout, und eine
Warnung davor landete in der Datei, in die ein Aufrufer umlenkt.

Alle 21 Proben stehen jetzt auf `format err`. Die Bibliotheksfunktion
`format::canonical_under` behält ihren Vertrag auf Parse-Ebene; der
Corpus-Test in `tools/idlc/src/format.rs` misst weiter über sie.

**Die andere Richtung gibt es nicht:** keine Probe hat `check ok` und
`format err`. `lint`, `dump-ast` und `print-deps` laufen über `analyse()`
wie `check` und stimmen mit ihm in allen 357 Proben überein.

**Round-Trip:** über die 325 Proben, die `check` und `format` beide
annehmen, hält er vollständig — dasselbe Modell, dieselben
Abhängigkeiten, idempotentes Format. Kein stiller Verlust im
Formatierer. Das gilt ausdrücklich auch für die 26 Proben, die bis zu
dieser Fixture-Arbeit die Kategorie D ausmachten: der Formatierer
verliert keine davon.

## Der Baum selbst

Die Erhebung misst Proben. Diese Seite hatte bis hierher keine Aussage
darüber, ob die 748 committeten `*.idl`-Dateien des Baums selbst gültiges
IDL sind — und es gab keinen Test, der `check` über sie alle fuhr. Ein
Konstrukt, das in einer eigenen Datei falsch wurde, fiel nur auf, wenn
zufällig ein anderer Test sie las.

Aufgefallen ist das an `tools/wire-oracle/cases/bitmask_past_bound/type.idl`:
absichtlich spezifikationswidrig, nur an fremde Generatoren gegeben, und
von einer Datei, die still falsch geworden ist, nicht zu unterscheiden.
Es brauchte eine neue Regel (B.4), damit sie überhaupt sichtbar wurde.

`tools/idlc/tests/corpus.rs` fährt jetzt `check` über jede committete
`.idl`. **13 Dateien** werden abgewiesen, alle 13 mit einem
geschriebenen Grund in `INTENDED`, je Datei die erlaubten Codes und ein
Satz, der sagt, wofür die Datei da ist. Drei Gruppen:

| Gruppe | Dateien | Was sie sind |
|---|---|---|
| Probe-Eingaben | 6 | `tools/hashid-gate` (2), `tools/inventory-probes` (2), `tools/try-construct-gate` (2) — die Zurückweisung *ist* die Messung |
| Grammatik-Fixtures | 4 | `ccm_components`, `corba_value_repository`, `fastdds_pragmas`, `zerodds_security` — für Syntaxabdeckung kuratiert, nicht für ein Modell, das schließt |
| Wire-Messung | 1 | `bitmask_past_bound` — fragt vier Fremdvendoren, was sie mit verbotenem `@position` tun |
| ORB-Korpus | 1 | `05-typecode` — `CORBA::TypeCode`, das jeder ORB vordeklariert |
| RTI-Fixture | — | war die 13., geht seit A.4 durch |

Ein zweiter Test prüft die Gegenrichtung: ein Eintrag, dessen Datei
wieder durchgeht, ist veraltet und macht den Test rot. Eine Ausnahme
ohne Grund lässt er gar nicht erst zu.

**Ein echter Defekt kam dabei heraus.**
`examples/demos/perf-camera-dds/idl/camera.idl` schrieb `unsigned octet
quality_hint;` — `unsigned octet` steht in keiner Produktion von IDL 4.2,
`octet` ist bereits vorzeichenlos. Ein ausgeliefertes Beispiel, das kein
konformes Frontend übersetzt. Korrigiert zu `octet quality_hint;`.

---

## Was diese Erhebung nicht abdeckt

- **Emission nur in zwei Sprachen.** `rust` und `c`. Die übrigen
  Backends unter `crates/lang-rules/src/default_*.rs` sind ungeprüft.
- **Kein Wire-Vergleich.** Gemessen wurde, ob Quelltext entsteht, nicht
  ob die erzeugten Bytes stimmen. Das ist die Frage der
  Conformance-Fixtures und beantwortet diese Seite nicht.
- **`fixtures` ist Textsuche.** Ein Treffer belegt nicht, dass die Datei
  in einem Test benutzt wird. Eine Null belegt, dass das Konstrukt im
  Baum nicht vorkommt.
- **Kein Vergleich mit anderen Frontends.** Ob `fastddsgen`, `idlc` von
  Cyclone oder `tao_idl` dieselbe IDL nehmen, ist nicht gemessen.
- **Klausel 9 (Profile).** Die vier CORBA/CCM- und drei DDS-Profile sind
  Zusammenstellungen der Building Blocks; ob `zerodds-idlc` ein
  Profil-Argument braucht, ist eine Produktfrage und keine Lücke gegen
  die Grammatik. Ungeprüft.
- **Tabelle 7-6 als Ganzes.** Die Spezifikation listet 84 Keywords.
  Geprüft ist ein Vertreter (`typedef boolean BOOLEAN;` wird mit
  `[IDL0017]` abgewiesen) und die Escape-Form (`long _long;` wird
  akzeptiert, 150 Fixtures im Baum benutzen sie). Die übrigen 83
  Keywords sind nicht einzeln als Bezeichner probiert. Auch §7.2.1
  Tokens und §7.2.5 Punctuation sind nicht Zeichen für Zeichen belegt.

## Das Werkzeug

```
tools/idl-4.2-gap-scan/scan.sh              messen und vergleichen
tools/idl-4.2-gap-scan/scan.sh --record     expected.tsv neu schreiben
tools/idl-4.2-gap-scan/scan.sh --report     die volle Messung ausgeben
IDLC=/pfad/zu/zerodds-idlc tools/idl-4.2-gap-scan/scan.sh
```

Exit-Codes:

| | |
|---|---|
| 0 | gemessen; die Lücke ist genau die aus `expected.tsv` |
| 1 | eine Probe, die durchkam, kommt nicht mehr durch — die Lücke ist gewachsen |
| 2 | nichts gemessen: kein `zerodds-idlc`. Nie ein Pass. |
| 3 | eine Probe kommt jetzt durch, die `expected.tsv` als scheiternd führt — die Lücke ist geschrumpft und diese Seite ist veraltet |
| 4 | `cases.tsv` und `expected.tsv` nennen verschiedene Proben |

Laufzeit auf einem M1: 36 Sekunden für 356 Proben über 748
`.idl`-Dateien. Vor der Matrix waren es 16 Sekunden bei zwei gemessenen
Unterbefehlen; die elf Aufrufe je Probe kosten den Rest.

Exit 3 ist nicht bloß Kosmetik. Eine Negativliste, die eine geschlossene
Lücke weiter als offen führt, ist schlechter als keine: sie kostet
Vertrauen in die übrigen Zeilen.

## Methodennotiz

Übertragbar auf CORBA 3.3, die DDS-Specs und die 47 eigenen Specs unter
`docs/specs/` — mit sehr verschiedenen Kosten.

**Was billig war.** Der Nenner. IDL 4.2 hat Annex A: eine konsolidierte,
durchnummerierte Grammatik, aus der sich mit `pdftotext -layout` und
zwanzig Zeilen Python alle 227 Produktionen lückenlos ziehen lassen
(Lückenprobe: `range(1, 228)` gegen die geparsten Nummern). Ohne diesen
Annex hätte das Aufsammeln der Produktionen aus fünfzehn Building-Block-
Klauseln ein Vielfaches gekostet, und die Vollständigkeit wäre nicht
prüfbar gewesen.

**Was teuer war.** Die Proben. 356 minimale `.idl`-Dateien von Hand, und
zweimal eine Runde Nacharbeit, weil die ersten Fassungen aus dem
falschen Grund scheiterten: ein Bezeichner `Alias` kollidiert mit dem
Keyword `alias`, und eine Union ohne `default:` wird vom Backend aus
einem Grund abgelehnt, der mit der geprüften Produktion nichts zu tun
hat. Jede Fehlmessung sah aus wie ein Befund. Das ist die eigentliche
Arbeit an so einer Erhebung: unterscheiden, ob die Probe oder der Baum
falsch ist.

**Wo der Volltext gebraucht wird.** Für die Kategorien A, C, D reicht die
Grammatik. Für Kategorie B — akzeptiert, obwohl verboten — braucht es den
Fließtext, und zwar wörtlich. Alle 19 B-Zeilen stammen aus Sätzen, die in
keiner Produktion stehen: „Adjacent string literals are concatenated",
„The maximum value is 64", „Members with no default value must be given a
value". Ohne PDF im Volltext wären sie unauffindbar gewesen. Und dieselbe
Klausel hat zwei Behauptungen widerlegt, die ich sonst als Lücke
aufgeschrieben hätte: unbekannte Annotationen *sollen* ignoriert werden,
und `struct S { long s; }` *ist* ein Fehler. Ohne den Text wären das zwei
falsche Zeilen gewesen.

**Was das für die anderen Specs heißt.**

| Korpus | Nenner vorhanden? | geschätzter Aufwand |
|---|---|---|
| CORBA 3.3 | Ja — Part 1 Annex A trägt dieselbe konsolidierte Grammatik, plus die GIOP/IIOP-Nachrichtentypen als Tabellen. Der Fließtext ist mit ~1.150 Seiten über drei Teile aber achtmal so lang wie IDL 4.2. | Grammatikachse etwa wie hier, ein bis zwei Tage. Die B-Achse (verbotene Formen) ist der Kostentreiber und pro Teil separat zu machen. Die Proben brauchen einen anderen Treiber, weil `zerodds-idlc` nicht die ganze Fläche abdeckt: für GIOP/POA/IR muss die Probe ein Rust-Test sein, keine `.idl`-Datei. |
| DDS-Specs (DCPS 1.4, XTypes 1.3, DDSI-RTPS 2.5, Security 1.2) | Nein, nicht in dieser Form. Der Nenner ist dort keine Grammatik, sondern eine Menge von Tabellen: QoS-Policies, Submessage-Kinds, TypeKinds, Plugin-Operationen. Die sind maschinenlesbar aus dem PDF zu holen, aber jede Spec braucht ihren eigenen Extraktor. | Pro Spec ein Tag für den Extraktor plus die Proben. Der Vorteil: die Proben sind hier keine Textdateien, sondern Bytes, und `tools/byte-gate` und `tools/unspecified-reach` sind die vorhandenen Treiber dafür. Die vier Kategorien werden zu: kennt der Decoder das Feld / schreibt der Encoder es / gibt es einen Testvektor / ist es bewusst nicht implementiert. |
| Eigene Specs unter `docs/specs/` | Ja, und billiger als bei allen anderen — die 47 Dokumente sind Markdown mit nummerierten Klauseln im Baum. Ein Extraktor für alle 47. | Der Nenner ist in Stunden zu haben. Der Haken ist ein anderer: eine selbst geschriebene Spec kann nicht gegen sich selbst falsch sein. Was eine Negativliste dort findet, ist eine Klausel ohne Implementierung — nützlich, aber nicht dasselbe wie eine Lücke gegen einen fremden Nenner. Der Erkenntniswert steht hinter CORBA und den DDS-Specs. |

**Die Reihenfolge, die ich empfehle.** DDS-XTypes 1.3 vor CORBA 3.3: der
Nenner ist kleiner, die Proben sind Bytes statt Text, und der
Wire-Bereich ist der, in dem ein übersehenes Konstrukt still falsche
Daten liefert statt eine Fehlermeldung. Die eigenen Specs zuletzt.

**Eine Regel, die die Erhebung selbst gelernt hat.** „Kommt im Baum nicht
vor" ist kein Beleg für „ist nicht erlaubt" — das war der Anlass. Die
Umkehrung gilt aber auch: „das Frontend weist es ab" ist kein Beleg für
„die Spezifikation verbietet es". Beide Richtungen brauchen den
Spec-Text, und beide sind in dieser Erhebung vorgekommen.

## Decision-Records

Keine. Diese Seite erhebt und schließt nichts.
