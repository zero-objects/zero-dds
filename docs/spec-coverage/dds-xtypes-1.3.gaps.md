# OMG DDS-XTypes 1.3 — Negativliste

**Spec:** [OMG DDS-XTypes 1.3](https://www.omg.org/spec/DDS-XTypes/1.3/),
formal/2020-02-04, 340 Seiten. PDF im lokalen Cache unter
`internal/standards/cache/omg/dds-xtypes-1.3.pdf` (gitignored).

**Werkzeug:** `tools/xtypes-1.3-gap-scan/scan.sh`. **Nenner:**
`tools/xtypes-1.3-gap-scan/roster.py` und die von ihm geschriebene
`roster.tsv`, dazu `anchors.tsv`. **Belege:**
`tools/xtypes-1.3-gap-scan/cases.tsv` (199 Proben) und
`expected.tsv` (der aufgezeichnete Stand).

Die übrigen Seiten unter `docs/spec-coverage/` sind Positivlisten: sie
nennen, was betrachtet wurde, und wie weit es kam. Was nie betrachtet
wurde, kann in ihnen nicht stehen, weil ihr Inhalt aus dem Baum stammt
und nicht aus der Spezifikation. Diese Seite ist die andere Richtung.

Jede Zeile hier hat einen Versuch hinter sich: die Oktette oder die IDL,
die gefüttert wurden, und was zurückkam. Keine Zeile steht auf einer
Vermutung.

## Der Nenner, und warum er hier teurer ist

OMG IDL 4.2 hat Annex A: eine konsolidierte, durchnummerierte Grammatik,
aus der ein vollständiges Verzeichnis mit Lückenprobe fällt
(`range(1, 228)` gegen die geparsten Nummern). **XTypes 1.3 hat keine
Grammatik.** Sein Verzeichnis ist eine Menge normativer Tabellen und
IDL-Blöcke, und jede braucht ihren eigenen Extraktor und ihr eigenes
Vollständigkeitsargument. `roster.py` zieht sieben Achsen aus dem PDF und
druckt pro Achse die Probe:

| Achse | Nenner | Vollständigkeitsprobe |
|---|---:|---|
| `typekind` | 28 | Annex B erklärt die TypeKind-Konstanten **zweimal** — einmal als `const octet TK_x`, einmal als `const TypeKind TK_x`. Beide Listen müssen in jedem Namen und jedem Wert übereinstimmen, und kein Wert darf zweimal vorkommen. |
| `typeid` | 31 | Jede `TI_*`-Konstante, die Annex B erklärt, muss ein `case` der `union TypeIdentifier` sein, und jedes `case`-Label eine erklärte Konstante. Dazu die Zeilen aus Tabelle 29. |
| `annotation` | 24 | §7.3.1.2.1 hat sechzehn nummerierte Unterklauseln; Lückenprobe `range(1, 17)`. Dazu die 19 Annotationen, deren Anwendbarkeit Tabelle 21 einschränkt. |
| `assign` | 55 | §7.2.4.4 hat acht Unterklauseln (Lückenprobe `range(1, 9)`), Tabellen 14 bis 19 tragen die Definitionen. Die 55 Regeln stehen in `anchors.tsv`, **jede mit einem wörtlichen Textstück aus dem PDF**, ohne das `roster.py` scheitert. |
| `typeobject` | 13 | `MinimalTypeObject` und `CompleteTypeObject` sind zwei Unionen über denselben Diskriminator; die Probe ist, dass ihre `case`-Mengen gleich sind. `TypeObject` selbst trägt genau `EK_COMPLETE` und `EK_MINIMAL`. |
| `typelookup` | 16 | §7.6.3.3 hat vier Unterklauseln (Lückenprobe `range(1, 5)`), §7.6.3.3.4 genau zwei Operationen, §7.6.3.3.3 zehn `TypeLookup_*`-Typen. |
| `datarep` | 16 | Tabelle 59 trägt vier Urteile, Tabelle 60 zwölf Zeilen aus Repräsentation, Extensibility, Encoding-Version, Endianness und Identifier-Wert. Alle zwölf müssen sich parsen lassen. |

**183 Einträge über sieben Achsen, 19 Proben, alle halten.**

Was das belegt und was nicht: dass jede Regel, die hier steht, in der
Spezifikation steht, ist geprüft. Dass es *alle* Regeln der genannten
Klauseln sind, ist die Behauptung dessen, der sie gelesen hat, und keine
Probe. Für `typekind`, `typeid`, `typeobject` und `typelookup` fällt der
Nenner aus dem IDL und ist damit maschinell vollständig; für `assign`,
`annotation` und `datarep` ist er von Hand aufgesammelt und nur
einzelbelegt.

## Wie gemessen wurde

Die Proben sind hier Oktette, nicht Text. Pro Probe drei Messungen; was
sie bedeuten, hängt an der Operation, und die Tabelle steht im Kopf von
`tools/xtypes-1.3-gap-scan/probe/src/main.rs`:

| Operation | `decode` | `encode` | `rule` |
|---|---|---|---|
| `ti` | die Oktette lesen sich als `TypeIdentifier` | zurückgeschrieben ergeben sie dieselben Oktette | §7.2.4.1: der Identifier ist von sich selbst zuweisbar |
| `to` | die Oktette lesen sich als `TypeObject` | dasselbe | §7.6.3: das Objekt wird zu einem Modell |
| `todh` | der Typ baut aus IDL | das codierte `TypeObject` beginnt mit einem DHEADER über den Rest (§7.4.2 für `@extensibility(APPENDABLE)`) | — |
| `idl` | die IDL parst, senkt und löst auf | das `MinimalTypeObject` übersteht Codieren, Decodieren, Codieren byte-gleich | das `CompleteTypeObject` baut |
| `assign` | beide Typen bauen aus einem IDL-Text | — | §7.2.4.4: `is_assignable(T2, T1)` sagte ja |
| `datarep` | — | — | §7.6.3.1 Tabelle 59: die Aushandlung nahm an |
| `encaps` | — | — | §7.6.3.1.2 Tabelle 60: die Paarung steht in der Matrix |
| `tl` | die Oktette lesen sich als der genannte TypeLookup-Typ | zurückgeschrieben ergeben sie dieselben Oktette | — |

Dazu eine Messung, die nicht am Code hängt: `corpus` zählt, wie viele der
10.722 committeten Quelldateien des Baums die Zeichenkette der Probe
überhaupt enthalten (Spalte `witness` in `cases.tsv`). `*.md` ist
absichtlich nicht im Korpus — eine Seite, die ein Konstrukt nennt, ist
kein Beleg dafür, dass etwas es ausübt, und diese Seite zählte sich sonst
selbst. Eine Null ist eine Tatsache: nichts im Baum nennt das Konstrukt,
also kann nichts im Baum es ausgeübt haben. Ein `-` heißt, dass keine
Zeichenkette das Konstrukt isoliert.

## Die Zahl

Die Erhebung fand **199 Proben, 166 ohne Befund**. Die 33 übrigen
verteilten sich so:

| | Kategorie | Proben | davon geschlossen |
|---|---|---:|---:|
| A | nicht erkannt — der Baum weist ab, was die Spezifikation erlaubt | 17 | 16 |
| B | akzeptiert, obwohl die Spezifikation etwas anderes verlangt | 9 | 7 |
| C | erkannt, aber nicht so emittiert | 6 | 5 |
| D | emittiert, und keine Quelldatei des Baums nennt es | 1 | 1 |
| E | bewusst so, mit Grund | 3 | — |

Kategorie B ist die, die der Vorgänger für OMG IDL 4.2 ergänzt hat und
die auch hier die wichtigste ist: dort entsteht keine Fehlermeldung,
sondern ein Modell mit falschem Inhalt. Sechs der neun B-Zeilen sind
Zuweisbarkeitsregeln, die stillschweigend durchgehen — zwei Endpunkte
passen zusammen, die es nach §7.2.4.4 nicht dürfen.

## Was daraus wurde

**Dreißig der 33 sind geschlossen. Drei bleiben offen, weil sie keine
Lücke im Baum sind.**

Jede geschlossene Zeile unten trägt einen `**Geschlossen:**`-Absatz mit
dem Test, der sie hält. Die drei übrigen — A.8, B.8/B.9 und C.4 — sind
Stellen, an denen die Spezifikation sich selbst widerspricht: dort ist
die Frage nicht, ob der Baum falsch ist, sondern welchem der beiden
Sätze er folgt, und das ist eine Entscheidung und keine Reparatur. Sie
stehen als **B7**, **B8** und **B9** in
[`docs/omg-xtypes-issues.md`](../omg-xtypes-issues.md), im Format des
OMG-Issue-Formulars.

Die Probenzahl ist von 199 auf **201** gestiegen: zwei Fälle sind
dazugekommen, die die neue Rahmung von der alten unterscheiden
(`tl-getTypes_in-extra-member`, `tl-call-unknown-operation`). Elf Proben
haben ein neues `arg` bekommen — die `tl-*`- und `to-*`-Oktette waren
gegen die Form des Baums geschrieben und nicht gegen die der Klausel,
und der Kopf von `cases.tsv` sagt, dass eine solche Probe nichts misst.

**Der Vendor-Vergleich, den diese Seite als fehlend führte, ist
gelaufen.** Am 2026-09-06 auf dem Linux-Testhost, mit den dort
installierten Toolchains: `idlc` (CycloneDDS) und `rtiddsgen` +
`DDS_TypeObjectV2_create_from_typecode` (RTI Connext 7.7.0) über
dieselbe IDL. Für `Plain` COMPLETE melden beide den Hash
`7c0c6fab7d066a0c56b8e93c0ea7`; Cyclones Strom ist 80 Oktette und
beginnt `4c000000` — ein DHEADER von 76 —, RTIs Blob ist 104 Oktette und
trägt genau diese 80 ab Offset 24. ZeroDDS erzeugt sie byte-gleich.
`crates/types/tests/typeobject_vendor_dheader.rs` hält alle vier Fälle
in beiden Formen; `tools/typeobject-bytes/record.sh` leitet sie neu aus
den Vendoren ab.

---

## A — nicht erkannt

Die Spezifikation erlaubt oder verlangt es, der Baum weist es ab.

### A.1 `TK_ANNOTATION` hat im Baum kein TypeObject

**Spec:** Annex B, `union MinimalTypeObject switch (octet)` —
`case TK_ANNOTATION: MinimalAnnotationType annotation_type;`. Tabelle 29
führt `TK_ANNOTATION` zusätzlich als TypeIdentifier-Diskriminator.

**Gefüttert:** `@annotation A { long v; };`

**Ergebnis:** ``no type named `A` `` — die Deklaration parst und senkt,
aber das Modell trägt danach keinen Typ `A`, also kann `from_ast` kein
`MinimalAnnotationType` bauen. Die Variante gibt es
(`crates/types/src/type_object/minimal/annotation_type.rs`); erzeugt wird
sie aus IDL nie.

**Corpus:** 4 von 10.722.

**Probe:** `tk-annotation`

**Geschlossen.** `from_ast::annotation_type_object` baut beide Formen,
und `Ast::annotation_defs` ist das Gegenstück zu `Ast::types` für die
andere Deklarationsart, die ein TypeObject hat. Ein `TypeNode` wird
dafür *nicht* angelegt: `@Name` soll auf die Deklaration auflösen und
nicht auf einen Typ. Test:
`crates/types/tests/type_object_annotations.rs::an_annotation_declaration_has_a_type_object`.

### A.2 Sechs der zehn TypeLookup-Typen aus §7.6.3.3.3 gibt es nicht

**Spec:** §7.6.3.3.3 erklärt zehn `TypeLookup_*`-Typen: die vier
`_In`/`_Out`-Strukturen, die beiden `_Result`-Unionen, `TypeLookup_Call`,
`TypeLookup_Return` und die beiden RPC-Hüllen `TypeLookup_Request`
(`@RPCRequestType`) und `TypeLookup_Reply` (`@RPCReplyType`).

**Ergebnis:** `crates/types/src/type_lookup.rs` hat vier Gegenstücke —
`GetTypesRequest`, `GetTypesReply`, `GetTypeDependenciesRequest`,
`GetTypeDependenciesReply`. Die sechs übrigen fehlen. `TypeLookup_Call`,
`_Return`, `_Request` und `_Reply` sind die Nachrichten, in denen die
vier über die Built-in-Endpunkte aus Tabelle 61 laufen; ohne sie gibt es
keinen Aufruf, nur die Nutzlast eines Aufrufs.

**Corpus:** 0 von 10.722 für jeden der sechs Namen.

**Proben:** `tl-gettypes_result`, `tl-gettypedependencies_result`,
`tl-call`, `tl-return`, `tl-request`, `tl-reply`

**Geschlossen.** Alle zehn Typen stehen in
`crates/types/src/type_lookup.rs`, jeder mit der Rahmung seiner
Extensibility: die vier Parameter-Typen `MUTABLE`, die sechs übrigen
appendable, weil §7.3.1.2.1.8 sagt, dass ein Typ ohne
`@extensibility`-Angabe appendable ist. `RequestHeader` ist dort lokal
erklärt und nicht aus `zerodds-rpc` genommen, weil jene Kiste von dieser
abhängt. Tests: `type_lookup_call_roundtrips_both_operations`,
`type_lookup_return_roundtrips_both_operations`,
`type_lookup_request_and_reply_roundtrip`,
`get_types_result_roundtrips_both_arms`,
`get_type_dependencies_result_roundtrips_both_arms`.

### A.3 Der `default:`-Arm von `MinimalTypeObject` wird abgewiesen

**Spec:** Annex B — `default: MinimalExtendedType extended_type;`, mit
dem Kommentar `// =========== Future extensibility ============`.

**Gefüttert:** `F1 C0 00000000` — EK_MINIMAL, dann ein TypeKind, den
diese Fassung nicht kennt.

**Ergebnis:** `UnknownTypeKind { kind: 192 }`.

**Probe:** `to-extended-arm`

**Geschlossen.** `MinimalTypeObject` und `CompleteTypeObject` haben eine
`Extended`-Variante, die den TypeKind und die Member-Liste wörtlich hält
— den DHEADER ausgenommen, weil das eine Länge ist, die der Encoder
schreibt, und eine Byte-Reihenfolge, die der Wert nicht trägt. Tests:
`a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`an_extended_type_kind_keeps_the_member_list_it_arrived_with`.

### A.4 `@non_serialized`-Member zählen bei der Kompatibilitätsprüfung mit

**Spec:** §7.2.4.4.2 — „Members that are marked as non-serialized, see
Sub Clause 7.3.1.2.1.14, shall be ignored during type compatibility
checking."

**Gefüttert:** Leser `@final struct R { long a; };`, Schreiber
`@final struct W { long a; @non_serialized long b; };`

**Ergebnis:** `No("final struct member count mismatch")`.

**Corpus:** 8 von 10.722.

**Proben:** `as-nonser`, `an-non-serialized-effect`

**Geschlossen.** Der Member steht gar nicht mehr im TypeObject, was
beides auf einmal ist: er ist nicht auf der Leitung, und die Regeln, die
nur das TypeObject lesen, können ihn nicht sehen. Der Member-*Index*
läuft trotzdem weiter, weil eine `@non_serialized`-Deklaration eine
Position besetzt. Test:
`crates/types/tests/table19_key_members.rs::a_non_serialized_member_does_not_enter_the_comparison`.

### A.5 `@ignore_literal_names` wirkt nicht

**Spec:** Tabelle 18, Zeile `ENUMERATION_TYPE` — „Any literals that have
the same name in T1 and T2 also have the same value, and any literals
that have the same value in T1 and T2 also have the same name. **This
behavior may be modified with the `@ignore_literal_names` annotation**,
see 7.3.1.2.1.11."

**Gefüttert:** `@ignore_literal_names @appendable enum R { A, B };` gegen
`@ignore_literal_names @appendable enum W { X, Y };` — gleiche Werte,
andere Namen.

**Ergebnis:** `No("an enum literal name and its value do not agree across
the two types")`. Die Annotation wird vom Frontend angenommen und ändert
das Urteil nicht.

**Corpus:** 4 von 10.722.

**Proben:** `as-enum-ignore-names`, `an-ignore-literal-names-effect`

**Geschlossen**, mit einer benannten Nebenwirkung. Die Annotation geht
jetzt in `EnumTypeFlag::IGNORE_LITERAL_NAMES`, und der Vergleich
verlangt sie auf **beiden** Seiten, wie §7.3.1.2.1.11 es sagt ("in both
enumerated types"); er stand auf `||`, was eine Seite allein über die
Namen der anderen entscheiden ließ. Damit wird ein Bit eines
Flag-Wortes geschrieben, das Annex B `// Unused` nennt — die
Extensibility-Bits desselben Wortes werden weiterhin **nicht**
geschrieben, siehe B.8/B.9 und `omg-xtypes-issues.md` §B8. Test:
`crates/types/tests/table19_key_members.rs::ignore_literal_names_needs_the_annotation_on_both_types`.

### A.6 Tabelle 18 stellt keine Bedingung an die Bitmask-Flags, der Baum schon

**Spec:** Tabelle 18 — „BITMASK_TYPE if and only if T1.bound ==
T2.bound"; die Spalte „Object construction" sagt „Preserve bit values by
index **for all bits identified in both T1 and T2**", rechnet also
ausdrücklich mit Bits, die nur eine Seite kennt.

**Gefüttert:** zwei Bitmasken mit `@bit_bound(8)`, deren Flags auf
verschiedenen Positionen sitzen; und eine, bei der der Schreiber ein Flag
mehr hat.

**Ergebnis:** beide Male `No("bitmask writer flag unknown in reader")`.

**Proben:** `as-bm-same-bound`, `as-bm-writer-extra-flag`

**Geschlossen.** Die Flag-Prüfung ist weg; der Bound bleibt, weil er die
Wire-Breite ist. Die Spalte "Object construction" ist das Argument
dafür, nicht nur das Fehlen einer Bedingung: "for all bits identified in
both T1 and T2" rechnet ausdrücklich mit Bits, die nur eine Seite kennt,
und sagt, was mit ihnen geschieht. Test:
`crates/types/tests/table19_key_members.rs::two_bitmasks_of_the_same_bound_are_assignable_whatever_their_flags`.

### A.7 Die leere DataRepresentation-Liste

**Spec:** §7.6.3.1 — „The default value of the DataRepresentationQosPolicy
shall be an empty list of preferences. An empty list of preferences shall
be taken to be equivalent to a list containing the single element XCDR."

**Gefüttert:** einmal ein leeres Angebot gegen `[XCDR]`, einmal `[XCDR]`
gegen eine leere Anforderung.

**Ergebnis:** beide Male `NoOverlap`. `negotiate_representation` wendet
den Default nicht an; zwei Endpunkte mit Default-QoS finden über diese
Funktion nicht zusammen.

**Proben:** `dr-empty-offer`, `dr-empty-request`

**Geschlossen.** `negotiate_representation` setzt für eine leere Liste
die Einzelliste `[XCDR]` ein, auf beiden Seiten. Die Ersetzung ist
`XCDR` und nicht "irgendetwas": ein leeres Angebot gegen einen Leser,
der nur XCDR2 nimmt, findet weiterhin nichts. Test:
`crates/types/src/qos.rs::an_empty_list_of_preferences_is_the_single_element_xcdr`.

### A.8 XCDR1 mit `APPENDABLE` wird abgewiesen, und Tabelle 60 führt es

**Spec:** Tabelle 60, dritte und vierte Zeile —
`XCDR | APPENDABLE | 1 | Big Endian | CDR_BE | {0x00, 0x00}` und
`XCDR | APPENDABLE | 1 | Little Endian | CDR_LE | {0x00, 0x01}`.

**Gefüttert:** `check_data_repr_extensibility(Xcdr1, Appendable)`.

**Ergebnis:** ``Err("Tab.59 §7.6.3.1: XCDR1 does not support APPENDABLE
directly — the encoder must choose FINAL")``. Die Ablehnung ist gewollt
und trägt ihren Grund im Quelltext; die Fundstelle, auf die sie sich
beruft, ist Tabelle 59, und die sagt dazu nichts. Tabelle 60 sagt das
Gegenteil.

**Probe:** `enc-xcdr-appendable`

**Offen, und nicht als Lücke im Baum.** Der Widerspruch steht als
**B7** in [`docs/omg-xtypes-issues.md`](../omg-xtypes-issues.md). Die
Ablehnung bleibt, wie sie ist: sie zu ändern hieße, eine der beiden
Tabellen zu wählen, und das Wählen ist Sache der RTF.

---

## B — akzeptiert, obwohl die Spezifikation etwas anderes verlangt

Hier entsteht keine Fehlermeldung, sondern ein Modell oder ein Urteil mit
falschem Inhalt.

### B.1 `TK_NONE` ist ein gültiger Typ und von sich selbst zuweisbar

**Spec:** Tabelle 29, erste Zeile — `TK_NONE | N/A | Invalid identifier`.

**Gefüttert:** das Oktett `00`.

**Ergebnis:** `decode ok`, `encode ok`, und
`is_assignable(None, None)` ist `Yes` — die Identitätsabkürzung aus
§7.2.4.1 greift, bevor irgendetwas prüft, ob der Identifier gültig ist.
Zwei Endpunkte, die beide den ungültigen Identifier tragen, passen damit
zusammen.

**Corpus:** 58 von 10.722.

**Probe:** `ti-prim-none`

**Geschlossen** in der `rule`-Spalte, und nur dort. `is_assignable`
weist `TK_NONE` auf beiden Seiten ab, vor der Identitätsabkürzung.
Decodieren und Encodieren bleiben erlaubt: `TypeInformation` schreibt
`TK_NONE` für eine Äquivalenzform, die der Sender ausgelassen hat
(§7.6.3.2.1), und ein Strom, der das trägt, ist nicht malformed. Test:
`crates/types/tests/table19_key_members.rs::tk_none_is_not_assignable_from_itself`.

### B.2 `EK_BOTH` geht als TypeIdentifier-Diskriminator durch

**Spec:** Annex B — die `union TypeIdentifier` hat `case EK_COMPLETE:`
und `case EK_MINIMAL:` und keinen Fall für `EK_BOTH = 0xF3`. `EK_BOTH`
gehört in den `PlainCollectionHeader`, nicht an diese Stelle.

**Gefüttert:** `F3` und vierzehn Oktette Hash.

**Ergebnis:** angenommen als `Unknown(0xF3)`; beim Zurückschreiben bleibt
ein Oktett übrig, die vierzehn Hash-Oktette sind weg.

**Corpus:** 5 von 10.722.

**Probe:** `ti-ek-both`

**Geschlossen.** `EK_BOTH` am TypeIdentifier-Diskriminator wird
abgewiesen. Im `PlainCollectionHeader`, wo es hingehört, ändert sich
nichts — das ist ein Oktett-Feld und kein Union-Diskriminator. Test:
`ek_both_is_not_a_type_identifier_discriminator`.

### B.3 Die SCC-Form des Baums lässt zwei Felder aus, und liest sie auch nicht

**Spec:** Annex B —

```idl
@extensibility(APPENDABLE) @nested
struct StronglyConnectedComponentId {
    TypeObjectHashId sc_component_id;
    long             scc_length;
    long             scc_index;
};
```

`TypeObjectHashId` ist selbst eine Union mit dem Diskriminator
`EK_COMPLETE`/`EK_MINIMAL` vor dem Hash, und die Datei trägt oben „The
types in this file shall be serialized with XCDR encoding version 2" —
ein `APPENDABLE`-Objekt bekommt unter Version 2 einen DHEADER (§7.4.2).

**Gefüttert:** die Form des Baums, `B0` und direkt vierzehn Hash-Oktette,
ohne DHEADER und ohne den `TypeObjectHashId`-Diskriminator.

**Ergebnis:** `decode ok`, `encode ok` — der Strom, den kein
konformer Leser annehmen dürfte, geht durch und kommt unverändert
zurück. Folge: `StronglyConnectedComponentId` im Baum hat kein Feld für
die Äquivalenzart, und §7.3.4.6 unterscheidet genau daran („those with
discriminator `TI_STRONG_COMPONENT` where the contained
`TypeObjectHashId` has discriminator `EK_MINIMAL`"). Minimale und
vollständige SCC-Identifier sind im Baum nicht unterscheidbar.

**Corpus:** 5 von 10.722.

**Probe:** `ti-scc-tree-form`; die Gegenrichtung steht unter C.5.

**Geschlossen.** `StronglyConnectedComponentId` hat jetzt das Feld
`equivalence_kind` und die APPENDABLE-Rahmung; die 24-Oktett-Form des
Baums wird abgewiesen. Tests:
`a_component_identifier_is_the_thirty_two_octets_of_annex_b`,
`a_minimal_and_a_complete_component_identifier_differ`,
`a_component_identifier_takes_only_the_two_equivalence_kinds`.

### B.4 Der `@key` am Union-Diskriminator erreicht das TypeObject nicht

**Spec:** Tabelle 19, Zeile `UNION_TYPE`, dritter Punkt — „Either the
discriminators of both T1 and T2 are keys or neither are keys."

**Gefüttert:** `@final union R switch (@key long) { case 1: long a; };`
gegen `@final union W switch (long) { case 1: long a; };`

**Ergebnis:** `Yes`. `crates/types/src/from_ast.rs` setzt
`discriminator_flags` fest auf
`UnionDiscriminatorFlag::IS_MUST_UNDERSTAND | try_construct_bits(…)`;
`IS_KEY` kommt nie hinein, also kann die Regel nichts unterscheiden.

**Corpus:** 192 von 10.722.

**Probe:** `as-union-disc-key`; die Gegenprobe mit `@key` auf beiden
Seiten (`as-union-disc-key-both`) ergibt ebenfalls `Yes`, wie sie soll.

**Geschlossen.** Das Frontend trägt `@key` am Diskriminator in
`XTypesFacet::discriminator_is_key`, `from_ast` in
`UnionDiscriminatorFlag::IS_KEY`, und `check_unions` vergleicht die
beiden. Welche Lesart gilt, entscheidet eine Achse und nicht dieses
Frontend: `KeyOnUnionDiscriminator::Refused` ist der Default und meldet
die Annotation — die gemessene Ablehnung durch Cyclone, RTI und Fast DDS
(2026-08-10, `/root/n11-keyunion/`) ist der Grund —, und `Carried` ist
die Bitte darum, bis hin zu einem KeyHash über den Diskriminator allein
(`keyhash::union_key_discriminator`). Tests:
`crates/types/tests/table19_key_members.rs::one_keyed_union_discriminator_and_one_unkeyed_are_not_assignable`,
`crates/idl-lower/tests/annotations.rs::a_key_on_a_union_discriminator_is_refused_with_the_clause_that_permits_it`
und `…::a_key_on_a_union_discriminator_reaches_the_facet_when_carried`.

### B.5–B.7 Drei der Schlüsselregeln aus Tabelle 19 greifen nicht

**Spec:** Tabelle 19, Zeile `STRUCTURE_TYPE`. Die Regeln stehen dort
wörtlich, und die Tabelle schließt mit „Note: The rules regarding key
members ensure that the key of T2 can be transformed faithfully into the
key of T1 without aliasing or loss of information."

| Regel (Zitat) | gefüttert (Leser gegen Schreiber) | Ergebnis | Probe |
|---|---|---|---|
| „For any string key member m2 in T2, the m1 member of T1 with the same member ID verifies m1.type.length >= m2.type.length." | `@key string<8>` gegen `@key string<16>` | `Yes` | `as-struct-key-string` |
| „For any enumerated key member m2 in T2, the m1 member of T1 with the same member ID verifies that all literals in m2.type appear as literals in m1.type." | `@key E1` mit `{ A }` gegen `@key E2` mit `{ A, B }` | `Yes` | `as-struct-key-enum` |
| „For any sequence or map key member m2 in T2, the m1 member of T1 with the same member ID verifies m1.type.length >= m2.type.length." | `@key sequence<long,4>` gegen `@key sequence<long,8>` | `Yes` | `as-struct-key-seqmap` |

Alle drei sind Fälle, in denen der Schlüssel des Schreibers nicht in den
Schlüssel des Lesers passt und die Endpunkte trotzdem zusammenfinden.

**Geschlossen.** `key_members_fit` in `assignability.rs` prüft die drei
Regeln, und zwar nur für Member mit `IS_KEY`: ein `string<8>`-Leser
*ist* von einem `string<16>`-Schreiber zuweisbar, solange der Member
kein Schlüssel ist — dann entscheidet `@try_construct`, und erst der
Schlüssel macht den Verlust unzulässig. Die Gegenrichtung ist
mitgeprüft. Tests in `crates/types/tests/table19_key_members.rs`:
`a_shorter_string_key_in_the_reader_is_not_assignable`,
`a_writer_enum_key_literal_the_reader_lacks_is_not_assignable`,
`a_shorter_sequence_key_in_the_reader_is_not_assignable`,
`a_shorter_string_member_that_is_not_a_key_is_still_assignable`.

### B.8/B.9 Die Extensibility einer Aufzählung wird nicht verglichen

**Spec:** Tabelle 18, Zeile `ENUMERATION_TYPE`, erster Punkt —
„T1.extensibility == T2.extensibility", und dritter Punkt — „If
extensibility is final T1 and T2 have the same literals."

**Gefüttert:** `@final enum R { A, B };` gegen
`@appendable enum W { A, B };`, und `@final enum R { A, B };` gegen
`@final enum W { A, B, C };`

**Ergebnis:** beide Male `Yes` zum Zeitpunkt der Erhebung; seit der
Vendor-Messung in E.3 beide Male `No`.

**Warum:** dieselbe Spezifikation erklärt in Annex B
`typedef TypeFlag EnumTypeFlag; // Unused. No flags apply`, und
`from_ast.rs` schrieb entsprechend `EnumTypeFlag(0)`. RTI, OpenDDS und
Cyclone schreiben die Extensibility-Position von `TypeFlag` in das Wort,
und RTI rechnet den Hash einer empfangenen Aufzählung aus seiner eigenen
Darstellung nach; `from_ast.rs` schreibt die Bits jetzt ebenso, und
`check_enumerations` vergleicht sie, sobald beide Seiten sie tragen.
Siehe E.3.

**Proben:** `as-enum-ext`, `as-enum-final`

**Offen, und nicht als Lücke im Baum.** Der Widerspruch steht als
**B8** in [`docs/omg-xtypes-issues.md`](../omg-xtypes-issues.md). Eine
Hälfte davon ist inzwischen entschieden und dort benannt: das eine Bit,
das §7.3.1.2.1.11 für `@ignore_literal_names` braucht, wird geschrieben
(A.5); die Extensibility-Bits werden es nicht.

---

## C — erkannt, aber nicht so emittiert

Der Baum liest den Wert und schreibt etwas anderes zurück.

### C.1/C.2 Das äußere `TypeObject` bekommt keinen DHEADER

**Spec:** Annex B —

```idl
@extensibility(APPENDABLE) @nested
union TypeObject switch (octet) { // EquivalenceKind
case EK_COMPLETE: CompleteTypeObject complete;
case EK_MINIMAL:  MinimalTypeObject  minimal;
};
```

und im Kopf derselben Datei: „The types in this file shall be serialized
with XCDR encoding version 2". §7.4.2 gibt einem `APPENDABLE`-Objekt
unter Encoding-Version 2 einen DHEADER — einen `UInt32` mit der Länge
alles Folgenden.

**Gefüttert:** `@final struct S { long a; };` und
`@mutable struct S { @id(1) long a; };`, jeweils als
`MinimalTypeObject` gebaut und codiert.

**Ergebnis:** 35 Oktette, die ersten vier ergeben 86.513 beziehungsweise
283.121, der Rest ist 31 Oktette lang. Es gibt keinen DHEADER; das erste
Oktett ist der EquivalenceKind. Die *inneren* `APPENDABLE`-Strukturen
bekommen ihren DHEADER — `framing::encode_appendable` hat 19 Aufrufe in
`crates/types/src/type_object/` —, das äußere Union nicht.

**Nicht gemessen:** ob ein Vendor an derselben Stelle einen DHEADER
schreibt. Der Baum tauscht `TypeObject`-Oktette mit keinem Vendor aus
(die Discovery matcht über `type_name`, siehe den Abschnitt „Wiring
status" in `crates/types/src/lib.rs`), also gibt es im Baum kein
aufgezeichnetes Vendor-Byte, gegen das sich das vergleichen ließe. Der
nächste Schritt für diesen Befund ist ein `PID_TYPE_OBJECT` von Fast DDS
oder Cyclone auf dem Linux-Testhost.

**Proben:** `to-dheader-minimal-struct`, `to-dheader-complete-struct`

**Geschlossen, und gemessen.** `TypeObject::encode_into` schreibt den
DHEADER; `hash::minimal_hash_input` ruft genau diesen Encoder auf und
ergänzt nichts mehr. Ein Pfad, nicht zwei — ein zweiter DHEADER würde
jedes Oktett jedes Hashes verschieben, und ein Peer, der über einen Hash
anderer Meinung ist, sieht aus wie ein Typkonflikt und nicht wie ein
Rahmungsfehler.

Der Vendor-Vergleich, den dieser Abschnitt als nächsten Schritt nannte,
ist gelaufen: siehe "Was daraus wurde" oben. CycloneDDS' `idlc` und RTI
Connext 7.7.0 auf dem Testhost, dieselben Oktette, derselbe Hash.

Die Gegenprobe zur RTI-Ablehnung, die diesen Befund begleitet: die
angekündigte Größe ist die Länge des Stroms, über den der Hash geht, und
das ist die Zahl, die RTI für dasselbe Objekt zählt — die Ablehnung war
ein `serializedSize mismatch` von vier Oktetten. Tests:
`crates/types/tests/typeobject_vendor_dheader.rs` (vier Fälle, beide
Formen, plus die Hash- und Größengleichheit) und
`crates/discovery/src/type_lookup/server.rs::a_dependency_is_announced_at_its_hashed_length`.

### C.3 `TI_EXTENDED` verschluckt seinen Rumpf

**Spec:** Annex B — `default: ExtendedTypeDefn extended_defn;`, und
`ExtendedTypeDefn` ist `@extensibility(MUTABLE)`, hat unter
Encoding-Version 2 also eine eigene Rahmung, die ein Leser überspringen
kann.

**Gefüttert:** `C0` und acht Nulloktette.

**Ergebnis:** decodiert zu `Unknown(0xC0)`, zurückgeschrieben als **ein**
Oktett. Der Leser hat vom Rumpf kein Oktett gelesen. An eingebetteter
Stelle — dem Elementtyp einer Plain Sequence etwa — verschiebt das jedes
folgende Feld des Stroms, ohne dass irgendwo ein Fehler entsteht.

**Corpus:** 3 von 10.722.

**Probe:** `ti-extended`

**Geschlossen.** `TypeIdentifier::Extended` hält Diskriminator und
Member-Liste; die Rahmung wird gelesen und geschrieben. Tests:
`the_extended_arm_carries_its_body`,
`an_extended_arm_with_a_member_list_round_trips_verbatim`,
`the_extended_arm_encodes_its_dheader_too`.

### C.4 `TK_ANNOTATION` als TypeIdentifier verliert seinen Hash

**Spec:** Tabelle 29, vorletzte Zeile — `TK_ANNOTATION | Annotation
Declaration | Not plain type. Has TypeObject. Uses Hash of the TypeObject
representation of the Annotation declaration`. Die `union TypeIdentifier`
in Annex B hat für `TK_ANNOTATION` **keinen** Fall; `roster.py` hält
diesen Widerspruch als Probe fest, damit er nicht stillschweigend
verschwindet.

**Gefüttert:** `50` und vierzehn Oktette Hash.

**Ergebnis:** decodiert zu `Unknown(0x50)`, zurückgeschrieben als ein
Oktett.

**Corpus:** 3 von 10.722.

**Probe:** `ti-annotation-hash`

**Offen, und nicht als Lücke im Baum.** Der Widerspruch steht als
**B9** in [`docs/omg-xtypes-issues.md`](../omg-xtypes-issues.md). Das
Verhalten hat sich trotzdem geändert, und zum Besseren: mit der Rahmung
des `default:`-Arms (C.3) wird `50` samt vierzehn Hash-Oktetten
**abgewiesen** statt stillschweigend auf ein Oktett verkürzt. Der Baum
folgt damit Annex B, laut. Welcher der beiden Sätze der richtige ist,
entscheidet die Seite in `omg-xtypes-issues.md` nicht — sie meldet ihn.

### C.5 Die SCC-Form der Spezifikation kommt verkürzt zurück

**Gefüttert:** die Form aus Annex B — `B0`, drei Füll-Oktette, ein
DHEADER von 24, dann `F1` als `TypeObjectHashId`-Diskriminator, vierzehn
Hash-Oktette, ein Füll-Oktett, `scc_length` und `scc_index`. 32 Oktette.

**Ergebnis:** `decode ok`, und zurück kommen **24** Oktette. Der Leser
hat den DHEADER als die ersten Hash-Oktette gelesen. Der Befund ist B.3
von der anderen Seite.

**Corpus:** 5 von 10.722.

**Probe:** `ti-scc-spec-form`

**Geschlossen** mit B.3, siehe dort.

### C.6 `TypeLookup_getTypes_Out` hat nur das erste seiner zwei Felder

**Spec:** §7.6.3.3.3 —

```idl
@extensibility(MUTABLE)
struct TypeLookup_getTypes_Out {
    @hashid sequence<TypeIdentifierTypeObjectPair> types;
    @hashid sequence<TypeIdentifierPair>           complete_to_minimal;
};
```

**Ergebnis:** `GetTypesReply` im Baum hat `types: Vec<ReplyTypeObject>`
und sonst nichts. Das ist weder `TypeIdentifierTypeObjectPair` — die
Antwort trägt die TypeObjects ohne den Identifier, nach dem gefragt
wurde — noch gibt es `complete_to_minimal`. Acht Nulloktette (zwei leere
Sequenzen) hinein, vier heraus.

**Dazu die Rahmung.** Alle sechs Request/Reply-Strukturen aus §7.6.3.3.3
sind `@extensibility(MUTABLE)` und tragen ihre Member mit `@hashid`;
unter Encoding-Version 2 heißt das DHEADER und EMHEADER pro Member. Der
Baum schreibt eine nackte Sequenz. Die Probe `tl-mutable-framing`
füttert `04000000 00000000` — einen DHEADER von 4 und dann eine leere
Sequenz — an `GetTypesRequest`: der DHEADER wird als Elementzahl gelesen,
es entstehen **vier `TK_NONE`-Identifier**, und der Strom kommt
byte-gleich zurück. Kein Fehler, nirgends.

**Corpus:** 7 von 10.722.

**Proben:** `tl-gettypes-out`, `tl-mutable-framing`

**Geschlossen.** `GetTypesReply` trägt beide Member, `types` als
`sequence<TypeIdentifierTypeObjectPair>`; die vier Parameter-Typen haben
die MUTABLE-Rahmung mit den `@hashid`-Member-IDs aus §7.3.1.2.1.1. Die
Ableitung hat ihre eigene Gegenprobe: die Klausel druckt
`TypeLookup_getTypes_HashId = 0x018252d3` und
`TypeLookup_getDependencies_HashId = 0x05aafb31`, und dieselbe Funktion
reproduziert beide. `04000000 00000000` wird jetzt abgewiesen. Tests:
`a_dheader_of_four_is_not_a_sequence_length_of_four`,
`an_empty_get_types_request_carries_its_member_header`,
`get_types_reply_carries_types_and_complete_to_minimal`,
`member_ids_are_the_hashid_of_their_name`,
`the_operation_hashes_are_the_ones_the_clause_prints`.

---

## D — emittiert, und keine Quelldatei des Baums nennt es

### D.1 `@default_nested`

**Spec:** §7.3.1.2.1.7 — `@annotation default_nested { boolean value
default TRUE; };`, angewandt auf eine Modul-Deklaration.

**Gefüttert:** `@default_nested(TRUE) module M { @final struct S { long
a; }; };` — geht durch Frontend, `MinimalTypeObject` und
`CompleteTypeObject`.

**Corpus:** 0 von 10.722. Keine `.rs`, keine `.idl`, keine `.tsv` des
Baums enthält die Zeichenkette `@default_nested`.

**Probe:** `an-default_nested`

**Geschlossen**, und die Null war die Spitze eines tieferen Befunds: die
Annotation stand nicht in `BUILTIN_ANNOTATIONS`, und eine
Modul-Deklaration bekam ihre Annotationen überhaupt nicht angewandt.
Ein ganzes Modul, das als nested erklärt war, gab `IS_NESTED`-freie
TypeObjects aus — ein Topic-Typ pro Endpunkt, den der Autor nicht
wollte. `Ast::effective_nested` läuft jetzt die Modulkette hoch. Test:
`crates/idl-lower/tests/annotations.rs::default_nested_on_a_module_reaches_the_types_in_it`.

Die übrigen 23 Annotationen der Achse und alle 28 TypeKinds haben
mindestens eine Fundstelle. Dass eine Fundstelle keine Ausübung belegt,
steht oben.

---

## E — bewusst so, mit Grund

Keine Lücke. Jede Zeile nennt ihren Beleg; die Seite hält sie getrennt,
damit sie unterscheidbar bleiben.

### E.1 Der Must-Understand-Bit am Union-Diskriminator wird immer gesetzt

`crates/types/src/from_ast.rs` setzt `UnionDiscriminatorFlag::
IS_MUST_UNDERSTAND` unbedingt und begründet das an Ort und Stelle mit
§7.2.2.4.4.4.6 („shall always be true") und mit
`docs/omg-xtypes-issues.md` §B2, wo festgehalten ist, dass keine
Implementierung dieses Bit setzt. Das ist eine bewusste Abweichung vom
gemessenen Vendor-Verhalten zugunsten des Spec-Textes.

### E.2 Der Platzhalter im SCC-Hash ist offen erklärt

`from_ast.rs` trägt `Scc::PLACEHOLDER_IS_UNSETTLED` und sagt dazu, was
gemessen ist und was nicht: die Reihenfolge, die Nummerierung ab eins,
die vierzehn MD5-Oktette und der XCDR2-Strom sind die der Spezifikation,
der Platzhalter aus Schritt 1 ist es nicht, und keine Implementierung der
Interop-Matrix berechnet einen SCC-Identifier, gegen den sich das prüfen
ließe. Das ist keine Lücke gegen die Spezifikation, sondern eine
offengelegte Unsicherheit.

### E.3 `EnumTypeFlag` folgt den Vendoren gegen Annex B

Annex B: `typedef TypeFlag EnumTypeFlag; // Unused. No flags apply`.
Tabelle 18 verlangt trotzdem den Vergleich der Extensibility zweier
Aufzählungen. Gemessen: RTI Connext 7.7.0, OpenDDS und Cyclone schreiben
die Extensibility-Position von `TypeFlag` in das Wort und `IS_DEFAULT`
(Bit 6) auf das erste Literal einer Aufzählung ohne `@default_literal`;
Fast DDS schreibt beides nicht. RTI hasht eine empfangene Aufzählung aus
der eigenen Darstellung neu, ohne die Bits fand es das Objekt unter dem
angekündigten Hash nicht. Der Baum schreibt die Bits wie die drei
Vendoren; `docs/omg-xtypes-issues.md` §B8 hält den Widerspruch und die
Messung.

---

## Was diese Erhebung nicht abdeckt

- **Kein Vendor-Vergleich in den Proben selbst.** Keine der 201 Proben
  läuft gegen Fast DDS, Cyclone, OpenDDS oder RTI; das Probe-Binary
  spricht mit keinem. Für C.1/C.2 ist der Vergleich inzwischen
  *daneben* gelaufen und aufgezeichnet —
  `crates/types/tests/typeobject_vendor_dheader.rs` hält Cyclones und
  RTIs Oktette, `tools/typeobject-bytes/record.sh` leitet sie neu ab —,
  aber er ist ein eigener Test und keine Spalte dieser Erhebung.
- **Die XML-Typrepräsentation (§7.3.2) und die XSD-Repräsentation
  (§7.3.3) sind ungeprüft.** Beide haben eigene Tabellen (26, 27, 28) und
  hätten je einen eigenen Extraktor gebraucht; `crates/xml` implementiert
  sie und ist von dieser Erhebung nicht berührt.
- **Die XCDR-Encoding-Regeln (§7.4.3.5) sind ungeprüft.** Die
  Serialisierungs-VM mit ihren Tabellen 36 bis 40 ist die größte
  Einzeltabelle der Spezifikation und die Achse, auf der
  `tools/byte-gate` und `tools/wire-oracle` messen — sie hätte diese
  Erhebung verdoppelt. Der Zusammenhang steht in
  `docs/spec-coverage/`-Positivseiten und in `docs/omg-xtypes-issues.md`.
- **Das Dynamic Language Binding (§7.5.2) ist ungeprüft.** Tabellen 48
  bis 58 beschreiben 11 Interfaces mit ihren Operationen; gemessen ist
  hier nur, dass ein `CompleteTypeObject` zu einem Modell wird
  (`rule`-Spalte der `to`-Proben).
- **Endianness in Tabelle 60.** Gemessen sind die sechs Paare aus
  Repräsentation und Extensibility. Welchen der zwölf Identifier-Werte
  der Encoder tatsächlich schreibt, entscheidet
  `crates/rtps/src/participant_message_data.rs`, und das ist von dieser
  Erhebung nicht berührt.
- **`corpus` ist Zeichenkettensuche.** Ein Treffer belegt nicht, dass die
  Datei etwas ausübt. Eine Null belegt, dass die Zeichenkette im Baum
  nicht vorkommt.
- **Die Achsen `assign`, `annotation` und `datarep` haben einen von Hand
  aufgesammelten Nenner.** Jede einzelne Regel ist belegt; dass es alle
  sind, ist nicht geprüft.

## Das Werkzeug

```
tools/xtypes-1.3-gap-scan/roster.py           den Nenner ziehen und prüfen
tools/xtypes-1.3-gap-scan/roster.py --check   nur prüfen, nichts schreiben
tools/xtypes-1.3-gap-scan/scan.sh             messen und vergleichen
tools/xtypes-1.3-gap-scan/scan.sh --record    expected.tsv neu schreiben
tools/xtypes-1.3-gap-scan/scan.sh --report    die volle Messung ausgeben
tools/xtypes-1.3-gap-scan/scan.sh --explain   dazu die erste Diagnose je Probe
PROBE=/pfad/zu/xtypes-gap-probe tools/xtypes-1.3-gap-scan/scan.sh
```

Exit-Codes von `scan.sh`:

| | |
|---|---|
| 0 | gemessen; die Lücke ist genau die aus `expected.tsv` |
| 1 | eine Probe, die durchkam, kommt nicht mehr durch — die Lücke ist gewachsen |
| 2 | nichts gemessen: kein Probe-Binary. Nie ein Pass. |
| 3 | eine Probe kommt jetzt durch, die `expected.tsv` als scheiternd führt — die Lücke ist geschrumpft und diese Seite ist veraltet |
| 4 | `cases.tsv` und `expected.tsv` nennen verschiedene Proben |

Exit-Codes von `roster.py`: 0 alle Proben halten, 1 eine
Vollständigkeitsprobe ist gescheitert (dann wird `roster.tsv` **nicht**
geschrieben), 2 kein PDF oder kein `pdftotext` — nichts extrahiert.

Alle fünf Pfade von `scan.sh` und alle drei von `roster.py` sind
ausgelöst worden.

Laufzeit auf einem M1: 10 Sekunden für 199 Proben über 10.722 committete
Quelldateien. Das Probe-Binary ist ein eigener Workspace unter
`tools/xtypes-1.3-gap-scan/probe/`; es steht absichtlich nicht in der
Member-Liste der Wurzel, damit ein Messinstrument die `Cargo.lock` des
Produkts nicht anfasst.

## Methodennotiz — was XTypes gegenüber IDL 4.2 gekostet hat

Die Schätzung des Vorgängers lautete „pro Spec ein Tag für den Extraktor
plus die Proben". Sie trägt, aber die Verteilung ist eine andere.

**Was hier billiger war.** Die Proben. Ein Oktettstrom aus Annex B ist in
einer Zeile geschrieben (`80F3000020 04` ist eine Plain Sequence von
`long`), und der Treiber ist ein Programm mit sieben Operationen statt
356 einzelner Dateien. Von den 199 Proben mussten zwei überarbeitet
werden, weil sie aus dem falschen Grund scheiterten — beim Vorgänger
waren es zwei ganze Runden Nacharbeit.

**Was hier teurer war.** Der Nenner. IDL 4.2 hat Annex A; XTypes hat
sieben Tabellen in sieben Formen, und `roster.py` ist 437 Zeilen für das,
was dort zwanzig Zeilen Python waren. Drei Fallen haben je eine Runde gekostet:
`pdftotext` setzt ein Form Feed vor jede Klausel am Seitenanfang, sodass
sechs von 24 Klauselnummern nicht am Zeilenanfang stehen und die
Lückenprobe eine Lücke meldet, die es nicht gibt; das Inhaltsverzeichnis
wiederholt jede Tabellenüberschrift, sodass eine Suche über den Volltext
den Eintrag findet statt der Tabelle; und Tabelle 29 enthält den String
`7.3.4.3` in einer ihrer eigenen Zellen, sodass ein Block, der dort
endet, vor seiner letzten Zeile aufhört — genau der Zeile mit dem
Befund C.4.

**Wo der Volltext gebraucht wird.** Anders als bei IDL 4.2 für alle
Kategorien. Die Grammatik-Achsen (`typekind`, `typeid`, `typeobject`,
`typelookup`) fallen aus dem IDL von Annex B; die Achsen, auf denen 24
der 33 Befunde liegen (`assign`, `annotation`, `datarep`), stehen
ausschließlich im Fließtext und in Tabellenzellen. Ohne PDF im Volltext
wären sie unauffindbar gewesen. Zwei Behauptungen hat derselbe Text
widerlegt, die sonst als Lücke hier stünden: ein `TI_STRING8_SMALL` mit
Bound 0 ist kein ungültiger Bound, sondern der unbeschränkte String
(Tabelle 29: „It is used for unbounded strings"); und ein Array von
`@appendable`-Elementen ist sehr wohl stark zuweisbar, weil §7.2.4.2
appendable Typen unter Encoding-Version 2 zu delimited erklärt.

**Was das für CORBA 3.3 heißt.** Die Reihenfolge, die der Vorgänger
empfohlen hat, hat sich bestätigt: der Wire-Bereich ist der, in dem ein
übersehenes Konstrukt still falsche Daten liefert statt einer
Fehlermeldung — sechs der neun B-Zeilen hier sind genau das. Für CORBA
gilt der Kostenschnitt umgekehrt: Part 1 Annex A trägt die konsolidierte
Grammatik, der Nenner ist also wieder billig, und der Fließtext ist mit
~1.150 Seiten über drei Teile achtmal so lang wie IDL 4.2. Der
Kostentreiber wird dort die B-Achse sein, und die braucht den Volltext
Klausel für Klausel. Ein Tag pro Teil ist die realistische Schätzung,
nicht ein Tag für CORBA.

**Eine Regel, die diese Erhebung dazugelernt hat.** Wo die Spezifikation
sich selbst widerspricht, ist die Frage nicht, ob der Baum falsch ist,
sondern welcher der beiden Sätze er befolgt. Drei Befunde hier sind von
dieser Art (A.8, B.8/B.9, C.4), und jeder von ihnen wäre als „Lücke im
Baum" falsch aufgeschrieben. `docs/omg-xtypes-issues.md` ist der Ort für
diese Hälfte.

## Decision-Records

Die Erhebung selbst trifft keine. Die Umsetzung hat vier getroffen, und
sie stehen an Ort und Stelle im Quelltext, nicht hier:

1. **`@key` am Union-Diskriminator steht auf einer Achse, nicht in der
   Hand des Frontends.** `crates/ast/src/facet/xtypes.rs`,
   `discriminator_is_key`, und `zerodds_idl_lower::KeyOnUnionDiscriminator`.
   Tabelle 19 braucht das Bit im TypeObject; drei gemessene Vendoren
   weisen eine keyed-Diskriminator-IDL ab, also wäre ein KeyHash darüber
   einer, den niemand nachrechnet. `Refused` ist der Default und meldet
   die Annotation mit der Klausel, die sie zulässt; `Carried` trägt sie,
   und der KeyHash folgt.
2. **`EnumTypeFlag` trägt die Extensibility-Bits und
   `IGNORE_LITERAL_NAMES` an Bit 5.** `crates/types/src/from_ast.rs`,
   `enumeration`. Die Extensibility-Bits folgen der Messung in E.3;
   §7.3.1.2.1.11 nennt ein Verhalten, für das es keinen anderen Platz
   gibt, und Bit 5 liegt über den fünf `TypeFlag`-Positionen.
3. **Der DHEADER des `TypeObject` steht im Encoder, nicht im
   Hash-Pfad.** `crates/types/src/type_object/mod.rs`, `encode_into`.
   Zwei Pfade, die ihn unterschiedlich zählen, sind schlimmer als einer,
   der ihn nirgends schreibt.
4. **Eine `@annotation`-Deklaration bekommt kein `TypeNode`.**
   `crates/types/src/from_ast.rs`, `annotation_type_object`. Ein `@Name`
   soll auf die Deklaration auflösen; das TypeObject wird aus der
   Deklaration gebaut statt aus einem Typ, den es nicht gibt.
