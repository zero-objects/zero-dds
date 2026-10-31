# DDS-TS 1.0 — Spec-Coverage

**Quelle:** `documentation/specs/dds-ts-1.0/main.tex`
(`zerodds/v1.0`, ZeroDDS Vendor-Spec).

Audit Item-für-Item gegen
die TeX-Quelle.

Implementation:

- `crates/lang-rules/` — TypeScript-Regeln, Materialisierung und Emitter;
  Laufzeit unter `runtime/typescript/src/`.

**Der Mechanismus ist ein anderer als beim Verfassen dieser Spec.**
`crates/idl-ts/` gab es bis Commit `d045e43ea86`: ein Codegen je Sprache,
der eine Descriptor-Runtime (`@zerodds/types` mit Registry, Type-Guards,
`DdsAny`) belieferte, gegen die zur Laufzeit reflektiert wurde. Heute
erzeugt ein geteilter Walk (`crates/lang-rules/src/emit/{encode,decode}.rs`)
direkt Reader- und Writer-Code, und die Sprachdatei liefert nur noch die
Sprachentscheidungen. Der erzeugte TypeScript importiert `Reader`,
`Writer` und die Rahmungs-Politiken aus `runtime/typescript/src/`.

Die Descriptor-Runtime steht wieder, als zweite Hälfte neben dem Codec und
nicht an dessen Stelle: `runtime/typescript/src/{types,branded,dds_any,
registry,equal}.ts` trägt die Ch8-Symbole, `wasm.ts` und `test_backend.ts`
das Annex-C-Profil, und
`crates/lang-rules/src/adapter/typescript.rs::ts_descriptors` erzeugt je
IDL-Typ die Seitentabelle, den Type-Guard und den `registerType`-Aufruf.
Der erzeugte Code importiert weiterhin `Reader` und `Writer` für die
Kodierung; der Deskriptor beschreibt den Typ, er kodiert ihn nicht.

**Modul-Mapping:**

| Modul | Spec-Bereich |
|---|---|
| `crates/lang-rules/src/rule/typescript.rs` | Ch7 Namensgebung, reservierte Wörter, Typabbildung |
| `crates/lang-rules/src/adapter/typescript.rs` | Ch7 Deklaration der erzeugten Typen; §7.4.1–§7.8.1 Deskriptor-Seitentabellen |
| `crates/lang-rules/src/emit/typescript.rs` | Ch7 Rendern; Sprachentscheidungen des geteilten Walks |
| `crates/lang-rules/src/default_typescript.rs` | §7.10 Default-Werte |
| `crates/lang-rules/src/emit/{encode,decode}.rs` | Ch9 Wire-Format — geteilt über alle Sprachen |
| `runtime/typescript/src/{reader,writer,endianness,policy,float128,error,index}.ts` | XCDR-Laufzeit |
| `runtime/typescript/src/{types,branded,dds_any,registry,equal}.ts` | Ch8 Descriptor-Runtime |
| `runtime/typescript/src/{wasm,test_backend}.ts` | Annex C WASM-Bindungen |
| `tools/idlc/` | Ch6 Toolchain |

Test-Lauf: `cargo test -p zerodds-lang-rules typescript` und
`runtime/typescript/run-tests.sh`.

---

## Status of Document

### Front-Matter

**Spec:** Status of Document — informativ; erklärt
Vendor-Status und Quellen-Bindung an `tsc` 5.x.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch1 Scope

### §1.1 Purpose

**Spec:** Ch1 §Purpose — definiert PSM für IDL 4.2 → TypeScript.

**Repo:** `tools/idlc/` (Treiber), `crates/lang-rules/src/{rule,adapter,emit}/typescript.rs`

**Tests:** —

**Status:** `n/a (informative)`

### §1.2 Out of Scope

**Spec:** Ch1 §Out of Scope — listet nicht-redefinierte Bereiche
(XCDR, JS-Runtime, DDS-RPC-Wire).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch2 Conformance

### §2.1 IDL-Codegen Profile

**Spec:** Ch2 §IDL-Codegen Profile — fünf normative Pflicht-Items
(zitiert aus TeX §sec:profile-codegen):

1. Type-Mapping aus Ch7 (`ch:type-mapping`) für jeden Primitive/
   Constructed/Template-Type von OMG IDL 4.2.
2. Annotation-Mapping aus §7.12 (`sec:annotation-mapping`) für jede
   X-Types-1.3-Annotation.
3. Type Descriptor + Type-Guard-Funktion pro emittierten Typ
   (Ch8 + §7.4 Struct-Mapping).
4. Output ist gültiger TypeScript-≥-5.0-Source, kompilierbar mit
   `tsc --strict` ohne `experimentalDecorators` /
   `emitDecoratorMetadata`.
5. Annex-B-§B.1-Conformance-Tests bestanden.

**Repo:** `crates/lang-rules/src/rule/typescript.rs` (Namensgebung,
reservierte Wörter), `crates/lang-rules/src/adapter/typescript.rs`
(`ts_items` für die Typen, `ts_descriptors` für die Seitentabellen,
`ts_items_with_descriptors` für beides in einem Namensraum),
`crates/lang-rules/src/emit/typescript.rs` (Rendern),
`crates/lang-rules/src/generate.rs` (der Treiber-Pfad, der beide Hälften
plus `descriptor_import` schreibt).

Punkte 1, 2 und 4 sind erfüllt: das Type-Mapping deckt den
Konstruktvorrat, die X-Types-Annotationen wirken über den geteilten Walk,
und der erzeugte Source übersetzt unter `tsc --strict` ohne
Decorator-Optionen — gemessen in
`crates/lang-rules/tests/typescript_descriptors.rs::the_generated_types_type_check`
über neun Formen. Punkt 3 ist erfüllt: `ts_descriptors` schreibt je Typ
`<Type>Type`, `is<Type>` und `registerType(<Type>Type)`.

Punkt 5 ist erfüllt: alle sechzehn Marken aus Annex B §B.1 haben einen
Test. Die Class-Freedom-Klausel aus B.1.2 eingeschlossen — `pair`
schreibt heute `interface P` neben `function newP(): P` und keine
Klasse.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::the_generated_types_type_check`,
`::b_1_1_every_primitive_round_trips`,
`::b_1_2_a_struct_gets_an_interface_a_descriptor_and_a_guard`;
`crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`,
`::a_sequence_counts_and_loops`, `::a_wide_integer_is_a_bigint_literal`;
`crates/lang-rules/tests/typescript_reader_generates.rs::a_final_struct_gets_a_reader`,
`::an_appendable_struct_gets_a_reader`, `::a_mutable_struct_gets_a_reader`,
`::an_optional_member_gets_a_reader`;
`crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`.

**Status:** `done`

### §2.2 Descriptor-Runtime Profile

**Spec:** Ch2 §Descriptor-Runtime Profile — Pflicht-Surface:
`@zerodds/types`-Library exportiert `DdsTypeDescriptor`,
`DdsMemberDescriptor`, `DdsTypeRef`, `ExtensibilityKind`,
`DdsAny`, `DdsException`, `Char`, `WChar`, `makeChar`,
`makeWChar`, plus Reflection-API.

**Repo:** `runtime/typescript/src/types.ts` (`DdsTypeDescriptor`,
`DdsMemberDescriptor`, `DdsTypeRef`, `ExtensibilityKind`,
`DescriptorKind`, `PrimitiveName`, `DdsAny`, `DdsException`,
`GuardedTypeOf`), `runtime/typescript/src/branded.ts` (`Char`, `WChar`,
`makeChar`, `makeWChar`, `LongDouble`, `makeLongDouble`),
`runtime/typescript/src/registry.ts` (`registerType`, `lookupType`,
`getKey`, `getTopic`, `withDefaults`, `isOneOf`),
`runtime/typescript/src/dds_any.ts` (`boxAny`, `unboxAny`),
`runtime/typescript/src/equal.ts` (`equalKey`, strukturelle Äquivalenz
für `registerType`); alle über `runtime/typescript/src/index.ts`
exportiert.

Der Import-Pfad ist `runtime/typescript/src/` statt `@zerodds/types`.
Ch8 nennt den Paketnamen ausdrücklich vendor-spezifisch und bindet nur
die Symbolnamen; die sind unverändert und unpräfixiert.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` — die neun Marken `"B.2.1"`, `"B.2.2"`, `"B.2.3"`, `"B.2.4"`, `"B.2.5"`, `"B.2.6"`, `"B.2.7"`, `"B.2.8"`, `"B.2.9"`, gefahren über `runtime/typescript/run-tests.sh`.

**Status:** `done`

### §2.3 Operations Profile

**Spec:** Ch2 §Operations Profile — Pflicht-Surface: Mapping aus
Ch11 (interface → Client/Handler-Pair, ServiceDescriptor),
plus Operations-Descriptor-Types (`ServiceDescriptor`,
`OperationDescriptor`, `OperationParameterDescriptor`,
`AttributeDescriptor`, `ParameterMode`, `isOneOf`).

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs::ts_interface_items` (Client/Handler-Paar je Interface), `runtime/typescript/src/operations.ts` (`ServiceDescriptor`, `OperationDescriptor`, `OperationParameterDescriptor`, `AttributeDescriptor`, `ParameterMode`, `hasAggregatedResult`)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_maps_an_interface_to_a_client_a_handler_and_a_descriptor`, `crates/lang-rules/tests/typescript_operations_profile.rs::the_generated_operations_profile_type_checks`

**Status:** done

### §2.4 WASM-Bindings Profile

**Spec:** Ch2 §WASM-Bindings Profile — Codegen+Descriptor-Runtime
+ Handle-Types/Sample-Types/DCPS-Operations aus Annex C.

**Repo:** `runtime/typescript/src/wasm.ts` (`ParticipantHandle`,
`TopicHandle`, `PublisherHandle`, `SubscriberHandle`,
`DataWriterHandle`, `DataReaderHandle`, `DdsGuid`, `makeDdsGuid`,
`SampleInfo`, `Sample`, `DataAvailableCallback`, `WasmBackend`,
`DdsBindings`, `bindWasmBackend`),
`runtime/typescript/src/test_backend.ts` (`createTestBackend`).

Die Codegen- und Descriptor-Runtime-Profile, auf denen §2.4 aufsetzt,
sind §2.1 und §2.2. Der XCDR2-Träger über die Grenze ist `Uint8Array`
und nichts anderes: `writeSample` nimmt Oktette, `takeSamples` gibt
Oktette.

**Tests:** `runtime/typescript/tests/wasm-bindings.ts` — `"C.5.1"`, `"C.5.2"`, `"C.5.3"`, `"C.5.4"`, gefahren über `runtime/typescript/run-tests.sh`.

**Status:** `done`

---

## Ch3 Normative References

**Spec:** Ch3 — Tabelle der referenzierten Standards.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch4 Terms and Definitions

**Spec:** Ch4 — Glossar (IDL, Codegen, Type/Service Descriptor, ESM).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch5 Symbols and Abbreviations

**Spec:** Ch5 — Abkürzungs-Liste.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch6 Overview

### §6.1 Mapping Philosophy

**Spec:** Ch6 §Mapping Philosophy — informativ; structural typing,
uniform interface, Type Descriptor, Service Descriptor.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### §6.2 Runtime Reflection Strategy

**Spec:** Ch6 §Runtime Reflection Strategy — informativ;
Side-Table-Begründung mit OMG-X-Types-Cross-Reference.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### §6.3 Toolchain

**Spec:** Ch6 §Toolchain — `tsc ≥ 5.0`, ESM-Output,
`crates/idl-ts/` (bis `d045e43ea86`, heute `tools/idlc`).

**Repo:** `tools/idlc/` — Aufruf, Optionen und Ausgabe des Codegens

**Tests:** —

**Status:** `n/a (informative)`

### §6.4 Code-Style Toggle

**Spec:** Ch6 §Code-Style Toggle — informativer Hinweis auf
`--camelcase`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch7 Type Mapping (Normative)

### §7.1 Module Mapping

**Spec:** Ch7 §Module Mapping — IDL `module M { … }` SHALL →
`export namespace M { … }`. Nested mit declaration-merging.
Qualifizierte Refs als dotted paths. Optional `--module-per-file`.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{module_chain, module_handle, scope_decls}` (Modul als `export namespace`, verschachtelt über die Kette), `crates/lang-rules/src/emit/typescript.rs::module_prefix` (qualifizierte Namen als Punktpfad)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done` — `export namespace` mit declaration-merging
voll abgedeckt; qualified-name-Resolver via `typespec_to_ts`
Scoped-Pfad. `--module-per-file` ist optionaler Codegen-Flag
(out of scope dieser Welle, im idlc-Tool zu integrieren).

### §7.2 Constant Mapping

**Spec:** Ch7 §Constant Mapping — `const T name = expr;` SHALL
→ `export const name: T' = expr';`. Const-Evaluation, BigInt-
Suffix für 64-bit, Decimal/Hex/Octal/Binary, cyclic-ref-reject.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{const_object, declare}` (Konstanten-Deklaration), `crates/lang-rules/src/emit/typescript.rs::{constant, literal, wide_int}` (typisiertes Literal, `n`-Suffix ab 64 Bit)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §7.3 Primitive Types

**Spec:** Ch7 §Primitive Types — Tabelle mit 14 IDL-Typen →
TS-Mapping. `char/wchar → Char/WChar` (Branded), `long double
→ LongDouble`, `any → DdsAny`.

**Repo:** `crates/lang-rules/src/emit/typescript.rs::{scalar, int, unsigned_int, wide_int}` (Primitive auf `number` bzw. `bigint`), `crates/lang-rules/src/adapter/typescript.rs::ts_member_type`

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §7.3.1 Bounded Strings

**Spec:** Ch7 §Bounded Strings — `string<N>`/`wstring<N>` SHALL
zusätzlich eine exportierte Konstante
`<Container>_<Field>_BOUND` mit `N` emittieren; ein eigenständiger
bounded-string-Typedef `<Alias>_BOUND`.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{bound_constants, side_constant}`,
gerufen in `declare` (Struktur-, Union- und Alias-Arm) und in
`ts_pending` (Exception-Glieder). Aliase werden nicht verfolgt: ein
Glied, das einen benannten Typedef nennt, nimmt dessen Konstante.

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_3_1_a_bounded_string_states_its_bound`,
`::the_side_tables_and_their_codecs_type_check` (Fall `side-strings`)

**Status:** `done`

### §7.3.2 Branded Character Types

**Spec:** Ch7 §Branded Character Types — `Char`/`WChar` als
branded `string` mit literal-tag `__dds_brand: "char"|"wchar"`;
`makeChar` (ISO 8859-1 range), `makeWChar` (code-point counting,
surrogate-reject).

**Repo:** `crates/lang-rules/src/emit/typescript.rs::code_point` (Zeichen als Codepunkt, Breite und Rahmung aus der Politik der Laufzeit)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §7.3.3 Long Double Handling

**Spec:** Ch7 §long double Handling — `long double` SHALL →
`LongDouble { __dds_brand: "long_double"; bytes: Uint8Array(16) }`.
Default-emit, `makeLongDouble`-Factory, byte-order
implementation-defined. Diagnostic `DDS-TS-I001`.

**Repo:** `crates/lang-rules/src/emit/typescript.rs` (Zeile 145) und `runtime/typescript/src/float128.ts` (Träger für binary128)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §7.3.4 any Mapping

**Spec:** Ch7 §any Mapping — `any` SHALL → `DdsAny { typeId; value }`.
`boxAny`/`unboxAny`-Helpers. Static-unknowability rule.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs` (`ANY_BOX`,
`prelude` deklariert `DdsAny` im erzeugten File, `type_at` bildet
`TypeKind::Any` darauf ab, `type_ref_expr` schreibt
`{ kind: "any" }`), `runtime/typescript/src/types.ts` (`DdsAny`),
`runtime/typescript/src/dds_any.ts` (`boxAny`, `unboxAny`).

`prelude` deklariert `DdsAny` statt es zu importieren — TypeScript ist
strukturell, die Deklaration und die der Laufzeit sind ein Typ, und ein
erzeugtes File ohne Deskriptoren bleibt so ohne Import übersetzbar.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_10_an_any_member_is_a_box_and_never_any_or_unknown`,
`crates/lang-rules/src/adapter/typescript.rs::tests::an_any_member_takes_the_box_and_not_a_typescript_keyword`;
`runtime/typescript/tests/descriptor-runtime.ts` (B.2.4, B.2.5).

**Status:** `done`

### §7.4 Struct Mapping — Uniform Interface

**Spec:** Ch7 §Struct Mapping §Uniform Interface — IDL `struct`
SHALL → TS `interface`, **kein** class. Class-Promotion explizit
verboten.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{ts_items, declare, pair, FACTORY_PREFIX, factory_path}` (Struct als `interface` neben `function new<Name>(): <Name>`; Annotationen wirken auf die Seitentabelle, nie auf die Form des Interface)

Beide Klauseln stehen. Die Form des Interface hängt an keiner
Annotation, und für keinen IDL-Datentyp entsteht eine TypeScript-Klasse:
Was ein Wert zum Anfangen braucht, liefert eine Fabrik-Funktion, die ein
Objektliteral mit jedem Pflicht-Glied auf seinem Tabelle-9-Wert
zurückgibt. Der Defaulter des Lesepfads
(`crates/lang-rules/src/default_typescript.rs::new_local`) ruft sie über
`factory_path`. Ein `@optional`-Glied bekommt keinen Eintrag — §7.2.2.4.4.4.8.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_2_no_class_reaches_the_output_for_any_idl_data_type`, `::b_1_11_annotations_change_the_side_table_and_not_the_interface`, `crates/lang-rules/src/adapter/typescript.rs::tests::a_struct_is_an_interface_and_a_factory_and_never_a_class`, `crates/lang-rules/tests/typescript_reader_generates.rs::the_generated_reader_type_checks`

**Status:** `done`

### §7.4.1 Type Descriptor Side-Table

**Spec:** Ch7 §Type Descriptor Side-Table — pro Struct SHALL
emittieren: `<Type>Type: DdsTypeDescriptor<T>`-Konstante,
`is<Type>`-Type-Guard, `registerType(<Type>Type)`.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{ts_descriptors, ts_items_with_descriptors, push_descriptor, descriptor_object, field_object, guard_expr}`; `crates/lang-rules/src/generate.rs` (Treiber-Pfad, schreibt beide Hälften plus `descriptor_import`)

Der Guard prüft Vorhandensein und Typ jedes Pflicht-Glieds, fordert kein
`@optional`-Glied und prüft `@unit`, `@min`, `@max` nicht. Die
Registrierung steht als Anweisung im Modul-Scope
(`zerodds_lang_cst::typescript::Decl::Statement`), hinter der Konstante,
die sie nennt.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_2_a_struct_gets_an_interface_a_descriptor_and_a_guard`, `::b_1_11_annotations_change_the_side_table_and_not_the_interface`, `::the_generated_types_type_check`

**Status:** `done`

### §7.4.2 Exception Mapping

**Spec:** Ch7 §Exception Mapping — IDL `exception` SHALL → TS
`interface extends DdsException`, descriptor mit
`kind: "exception"`.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::declare` (Exception als eigener Typ), `runtime/typescript/src/error.ts` (Fehlerträger der Laufzeit)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §7.5 Union Mapping

**Spec:** Ch7 §Union Mapping — discriminated-union mit
literal-narrowed `discriminator` (enum/boolean) bzw. number
(integer-disc); default-case via `Exclude<…>` für enum/boolean,
runtime-only für integer; multiple-labels-per-case.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{declare, discriminant_type, discriminant_type_of, discriminant_expr, excluding, explicit_labels}`
und die Konstante `DISCRIMINANT_PROPERTY` (= `discriminator`);
`crates/lang-rules/src/emit/typescript.rs::{discriminant, case_label}`
über die Nähte `crates/lang-rules/src/emit/mod.rs::CodecTarget::{discriminant, case_label}`.

Die Eigenschaft heißt `discriminator`. Der `default:`-Arm trägt
`Exclude<DiscType, ExplicitLabels>`; bei einem Enum- oder
`boolean`-Diskriminanten nimmt `Exclude` die Labels wirklich heraus,
bei einem Ganzzahl-Diskriminanten bleibt der Typ die Zahl — was der
Absatz „Compile-time exclusivity is partial" sagt. Ein
Enum-Diskriminant narrowt auf `typeof Tag.TAG_A`.

Der kanonische JSON-Dialekt der Interop-Suite schreibt einen
Diskriminanten weiter `_d`; das ist `crates/wire-probe/src/canonical.rs`
und nicht dieses Mapping. Die geschriebenen Oktette sind unverändert
(gemessen, siehe Abschluss-Bemerkung).

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_5_a_union_arm_names_its_discriminator`,
`::s_7_5_an_enumeration_discriminator_narrows_on_the_enumerator`,
`::the_side_tables_and_their_codecs_type_check` (Fälle `side-union-integer`, `side-enum-union`),
`crates/lang-rules/src/adapter/typescript.rs::tests::{a_union_is_a_discriminated_union_narrowed_on_a_literal, a_union_discriminant_takes_the_discriminators_own_literal_form}`

**Status:** `done`

### §7.5.1 Union Descriptor Side-Table

**Spec:** Ch7 §Union Descriptor — `kind: "union"` Descriptor mit
synthetic discriminator-field (`id: 0xFFFFFFFF`), `labels`-Property
pro case-member, `hasDefault`/`hasImplicitDefault`-Flags.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{type_spec, DISCRIMINATOR_ID, DISCRIMINANT_PROPERTY, exhausts_discriminator}`

Das synthetische Diskriminanten-Glied steht als erstes und trägt
`id: 4294967295`. Sein `name` ist die Eigenschaft, die die Arme
tatsächlich tragen (`discriminator`, §7.5), damit ein Konsument die
Seitentabelle liest statt den Namen zweimal zu wissen. `hasImplicitDefault` wird für
`boolean`- und Enum-Diskriminanten aus der Abdeckung berechnet; ein
Ganzzahl-Diskriminant gilt nie als abgedeckt.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_3_a_union_carries_its_labels_and_its_default_flags`, `::b_1_3_a_union_without_a_default_and_without_coverage_says_so`, `::b_1_3_an_enum_discriminator_the_cases_exhaust_has_no_implicit_default`

**Status:** `done`

### §7.6 Enum Mapping

**Spec:** Ch7 §Enum Mapping — IDL `enum` SHALL → `as const`-Object
+ string-literal-union type alias. **Kein** TS-`enum` keyword.
`@value(N)`-Companion-Object `<Enum>Ordinal`.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{declare, enum_alias_type, enum_ordinals, ordinal_path, enum_default_literal, enumerator_path}`;
`crates/lang-rules/src/emit/typescript.rs::enumerator_ordinal` über die
Naht `crates/lang-rules/src/emit/mod.rs::CodecTarget::enumerator_ordinal`.

Erzeugt werden `export const Colour = { RED: "RED", … } as const;`,
`export type Colour = (typeof Colour)[keyof typeof Colour];`,
`export const ColourOrdinal: Readonly<Record<Colour, number>>` und
`export const ColourFromOrdinal: ReadonlyMap<number, Colour>`. Die
Ordinal-Tabelle wird auch ohne `@value(N)` emittiert, mit den
impliziten Werten. `@bit_bound(N)` bleibt Descriptor-Feld und berührt
die Oberfläche nicht.

Der Codec liest den Wert durch `<Enum>Ordinal`, weil in der Variablen
keine Zahl mehr steht. Ohne diese Naht schrieb `enum-sparse-values`
die Oktette `70 79 87` — die erste Codeeinheit von `FAIL`, `OK`,
`WARN` — wo das Ledger `65535 1 100` hält.

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_6_an_enumeration_is_a_string_literal_union_with_its_ordinals`,
`::s_7_6_implicit_ordinals_are_emitted_too`,
`::the_side_tables_and_their_codecs_type_check` (Fall `side-enum`),
`crates/lang-rules/tests/typescript_descriptors.rs::b_1_4_an_enum_gets_a_const_object_and_a_descriptor`,
`crates/lang-rules/src/adapter/typescript.rs::tests::an_enumeration_is_an_as_const_object_beside_a_string_literal_union`

**Status:** `done`

### §7.6.1 Enum Descriptor Side-Table

**Spec:** Ch7 §Enum Descriptor — `kind: "enum"` Descriptor mit
`bitBound`-Field, Member als `DdsMemberDescriptor` mit
ordinal-Default.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::type_spec` (Arm `TypeKind::Enumeration`)

Jeder Enumerator wird ein `DdsMemberDescriptor` mit
`type: { kind: "primitive", name: "int32" }` — die 32 Bit aus XTypes 1.3
§7.4.3.5.3 — und seinem Ordinalwert als `default`. `bitBound` trägt den
effektiven Bit-Bound, Vorgabe 32.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_4_an_enum_gets_a_const_object_and_a_descriptor`, `::b_1_4_an_explicit_value_reaches_the_descriptor`

**Status:** `done`

### §7.7 Bitset Mapping

**Spec:** Ch7 §Bitset Mapping — IDL `bitset` SHALL → TS interface
+ `<Bitset>_<Field>_BITS`-Constants. Width ≤ 32 → `number`,
33–64 → `bigint`. Total > 64 fatal-error.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::declare`
(Arm `TypeKind::Bitset`, ein `side_constant` je benanntem Bitfeld) und
`unsigned_holder` für den Anker; die 64-Bit-Grenze weist der Parser ab.
Ein anonymes Bitfeld ist Padding und bekommt keine Konstante.

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_7_a_bitset_states_the_width_of_every_bitfield`,
`::the_side_tables_and_their_codecs_type_check` (Fall `side-bits`)

**Status:** `done`

### §7.7.1 Bitset Descriptor Side-Table

**Spec:** Ch7 §Bitset Descriptor — `kind: "bitset"` Descriptor mit
`type: { kind: "bitfield"; width: W }` pro Member.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{type_spec, bitfield_ref}` (Arm `TypeKind::Bitset`)

Jedes Bitfeld wird ein `DdsMemberDescriptor` mit
`type: { kind: "bitfield", width: W }`; `bitBound` trägt die Summe aller
Breiten, anonyme Padding-Felder eingerechnet. Die 64-Bit-Grenze weist
der Parser ab.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_5a_a_bitset_widths_pick_their_holders_and_the_descriptor_totals_them`

**Status:** `done`

### §7.8 Bitmask Mapping

**Spec:** Ch7 §Bitmask Mapping — IDL `bitmask` SHALL → `as const`-
Object mit shift-Expressions. `bit_bound ≤ 32` → `number` +
`(1 << K) >>> 0`; 33–64 → `bigint` + `1n << Kn`.
`<Bitmask>_BIT_BOUND`-Constant.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::declare`
(Arm `TypeKind::Bitmask`; `side_constant` mit `ast.effective_bit_bound`),
`bit_literal` für den Wert des Ankertyps,
`crates/lang-rules/src/emit/typescript.rs::bitmask_value` für den Codec.

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_8_a_bitmask_states_its_bit_bound`,
`::the_side_tables_and_their_codecs_type_check` (Fall `side-bits`),
`crates/lang-rules/src/adapter/typescript.rs::tests::a_bitmask_writes_its_bit_as_the_number_and_not_as_a_shift`

**Status:** `done`

### §7.8.1 Bitmask Descriptor Side-Table

**Spec:** Ch7 §Bitmask Descriptor — `kind: "bitmask"` Descriptor.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::type_spec` (Arm `TypeKind::Bitmask`)

Jeder Bit-Wert wird ein `DdsMemberDescriptor` mit
`type: { kind: "bitfield", width: 1 }` und der Bit-Position als
`default`; `@position(P)` schlägt dabei durch, weil
`effective_bit_position` sie bereits aufgelöst hat. `bitBound` trägt den
effektiven Bound.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_5b_a_bitmask_carries_its_positions_and_its_bound`, `::b_1_5b_a_wide_bitmask_and_an_explicit_position`

**Status:** `done`

### §7.8.2 Position Annotation

**Spec:** Ch7 §Position Annotation — `@position(P)` SHALL bit
position überschreiben; conflicting positions parser-reject.

**Repo:** `crates/lang-rules/src/emit/typescript.rs::bitmask_value` (`@position` bestimmt das gesetzte Bit)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_wide_integer_is_a_bigint_literal`

**Status:** `done`

### §7.9 Sequence and Array Mapping

**Spec:** Ch7 §Sequence and Array Mapping — bounded/unbounded →
`Array<T'>`; Bounded mit `<Container>_<Field>_BOUND`. Fixed-size
arrays mit `_LENGTH`. Multi-Dim mit `_LENGTH_DIM<N>`.
Element-Mutability (`ReadonlyArray` opt-in). Empty-Sequences
round-trip als `[]`.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{array_of, type_at}`
für `T[]`, `bound_constants` für `_BOUND`, `_LENGTH` und
`_LENGTH_DIM1`…`_LENGTH_DIM<k>` in Deklarationsreihenfolge;
`default_expr` gibt einer Folge `[]` und einem Feld fester Länge
`Array.from({ length: n }, …)`.

`ReadonlyArray` ist ein MAY der Klausel („An implementation MAY emit
`ReadonlyArray` … when configured with `--readonly-collections`") und
zugleich ein SHALL NOT ohne diese Konfiguration; emittiert wird
`Array`.

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_9_a_bounded_sequence_and_an_array_state_their_sizes`,
`::the_side_tables_and_their_codecs_type_check` (Fall `side-collections`),
`crates/lang-rules/src/adapter/typescript.rs::tests::a_sequence_and_an_array_are_both_typescript_arrays`,
`crates/lang-rules/tests/typescript_codec.rs::a_sequence_counts_and_loops`

**Status:** `done`

### §7.9.1 Maps

**Spec:** Ch7 §Maps — IDL `map<K,V>` SHALL → `ReadonlyMap<K',V'>`.
`<Container>_<Field>_BOUND` für bounded; `keyEqualityHazard`-Flag
für non-value-equality keys; `--mutable-collections`-Flag.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::type_at`
(Arm `TypeKind::Map` → `ReadonlyMap<K', V'>`), `default_expr`
(`new Map()`), `guard_check_at` (`instanceof Map`), `bound_constants`
für `_BOUND`, `map_key_hazard` für `keyEqualityHazard`;
`crates/lang-rules/src/emit/typescript.rs::{value_type_of, routine, reading_map_body, entries_of_map, map_of_entries}`.

Nie ein `Record<…>`: das verliert die Schlüssel-Eindeutigkeit und legt
eine Aufzählungsreihenfolge fest. Der Codec arbeitet weiter auf der
Eintragsfolge, die XTypes 1.3 §7.4.1.1.4 auf die Leitung legt — ein
`Map` hat kein halb gefülltes Eintragsobjekt, in das die Leserichtung
Schlüssel und Wert nacheinander schreiben könnte —; die Umwandlung
steht an den beiden Enden der erzeugten Funktion. Die geschriebenen
Oktette sind unverändert (gemessen, siehe Abschluss-Bemerkung).

`--mutable-collections` ist ein MAY der Klausel und ausdrücklich
non-default; emittiert wird `ReadonlyMap`.

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_9_1_a_map_is_a_readonly_map_and_states_its_bound`,
`::the_side_tables_and_their_codecs_type_check` (Fälle `side-map`, `side-map-struct-key`, `side-map-in-sequence`),
`crates/lang-rules/src/adapter/typescript.rs::tests::a_map_member_is_a_readonly_map_over_a_staged_entry_type`,
`crates/lang-rules/tests/typescript_descriptors.rs::b_1_16_a_bounded_map_carries_its_bound_and_a_struct_key_its_hazard`

**Status:** `done`

### §7.10 Typedef Mapping

**Spec:** Ch7 §Typedef Mapping — Simple/Sequence/Array-Aliases,
Recursive Resolution, Forward References.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{module_chain, module_handle, scope_decls}` (Modul als `export namespace`, verschachtelt über die Kette), `crates/lang-rules/src/emit/typescript.rs::module_prefix` (qualifizierte Namen als Punktpfad)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done` — TS-`export type`-Aliases sind module-scope-
order-frei (verifiziert via Forward-Ref-Test). Cycle-Reject ist
Sache des IDL-Frontends (`zerodds-idl-lower` / `zerodds-idl-compose`, nicht hier).

### §7.10.1 Typedef Descriptor Side-Table

**Spec:** Ch7 §Typedef Descriptor — `kind: "alias"` Descriptor mit
einem Glied namens `value`; Type-Guard delegiert an den Guard des
referenzierten Typs.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{type_spec, GuardShape}` (Arm `TypeKind::Alias`; `GuardShape::Delegate` trägt den Guard des Ziels)

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_13_a_typedef_is_an_alias_with_one_member_named_value`, `::b_1_13_a_wide_bit_bound_switches_the_alias_to_bigint`

**Status:** `done`

### §7.10.2 @bit_bound on Integer Typedefs

**Spec:** Ch7 §@bit_bound on Integer Typedefs — N ≤ 32 → `number`,
33–64 → `bigint`-Switch.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{module_chain, module_handle, scope_decls}` (Modul als `export namespace`, verschachtelt über die Kette), `crates/lang-rules/src/emit/typescript.rs::module_prefix` (qualifizierte Namen als Punktpfad)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §7.11 Optional and Default Values

**Spec:** Ch7 §Optional and Default Values — `@optional` SHALL →
`?: T | undefined`; `@default(V)` SHALL → side-table
`<Type>_<Field>_DEFAULT`-Constant + descriptor `default`-Field.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{pair, default_constant, member_field}`;
`crates/lang-rules/src/default_typescript.rs::{explicit_default, member_default}`.

Ein `@optional`-Glied trägt beide Hälften: den `?:`-Suffix und den
`| undefined`-Arm — unter `exactOptionalPropertyTypes` sind
`retry?: number` und `retry?: number | undefined` verschiedene
Deklarationen. Ein `@default(V)` erzeugt
`export const <Type>_<Field>_DEFAULT: T' = V;` aus derselben Funktion,
aus der der Defaulter den Wert nimmt, und dazu das
`default`-Feld des `DdsMemberDescriptor`.

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_11_a_default_reaches_a_side_table_constant`,
`::s_7_11_an_optional_member_is_a_question_mark_and_undefined`,
`::the_side_tables_and_their_codecs_type_check` (Fall `side-defaults`),
`crates/lang-rules/src/adapter/typescript.rs::tests::an_optional_member_is_a_question_mark_and_not_a_null`

**Status:** `done`

### §7.12 Annotation Mapping

**Spec:** Ch7 §Annotation Mapping — Tabelle mit ~15 Annotationen
(`@id`, `@key`, `@optional`, `@default`, `@final`, `@appendable`,
`@mutable`, `@nested`, `@topic`, `@must_understand`, `@unit`,
`@min`, `@max`, `@hashid`, `@autoid`, `@verbatim`, `@bit_bound`)
→ Descriptor-Field oder TSDoc-Tag. Resolution-Order, TSDoc-
Rendering. Strict-Annotations-Mode (`--strict-annotations` →
E004).

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{module_chain, module_handle, scope_decls}` (Modul als `export namespace`, verschachtelt über die Kette), `crates/lang-rules/src/emit/typescript.rs::module_prefix` (qualifizierte Namen als Punktpfad)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

---

## Ch8 Descriptor Runtime (Normative)

### §8.1 Extensibility Kind

**Spec:** Ch8 §Extensibility Kind — `ExtensibilityKind = "final"
| "appendable" | "mutable"`-Type.

**Repo:** `runtime/typescript/src/types.ts` (`ExtensibilityKind`)

Die drei Werte in Kleinschreibung, weil der Deskriptor JSON-fähig ist
und dies seine Werte sind; sie entsprechen `FINAL`, `APPENDABLE` und
`MUTABLE` aus X-Types 1.3 §7.2.2.4.4 eins zu eins.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` (B.2.1 — die
Typ-Liste ist ein Tupel, das `tsc` prüft), `crates/lang-rules/tests/typescript_descriptors.rs::b_1_11_an_unannotated_struct_is_appendable_and_carries_no_topic`

**Status:** `done`

### §8.2 Type References (DdsTypeRef)

**Spec:** Ch8 §Type References — `DdsTypeRef` discriminated-union
mit Kinds: `primitive`, `bitfield`, `string`, `sequence`, `array`,
`map`, `ref` (by name), `any`. Plus `PrimitiveName`-Type.

**Repo:** `runtime/typescript/src/types.ts` (`DdsTypeRef`, `PrimitiveName`); `crates/lang-rules/src/adapter/typescript.rs::{type_ref_expr, primitive_ref, bitfield_ref, named_ref, primitive_ref_name}` (die Erzeugerseite)

Alle acht Kinds. `ref` trägt den qualifizierten IDL-Namen und keine
Deskriptor-Referenz — das hält den Wertgraphen zyklenfrei und
JSON-fähig und ist Voraussetzung für wechselseitig rekursive Typen.

`PrimitiveName` trägt einen Namen mehr als die Liste in §8.2: `int8`.
IDL 4.2 §7.4.1.4.4.2 kennt `int8` neben `uint8`, die Liste nennt
`octet` — das ist `uint8` — und für den vorzeichenbehafteten Fall gar
nichts. Additiv: jeder von §8.2 genannte Wert bedeutet unverändert das,
was §8.2 sagt.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_1_every_primitive_round_trips`, `::b_1_16_a_map_member_is_described_as_a_map`, `::b_1_8_char_and_wchar_reach_the_descriptor_as_themselves`, `::the_generated_types_type_check`; `runtime/typescript/tests/descriptor-runtime.ts` (B.2.8)

**Status:** `done`

### §8.3 Type and Member Descriptors

**Spec:** Ch8 §Type and Member Descriptors —
`DdsMemberDescriptor`, `DescriptorKind`, `DdsTypeDescriptor<T>`.

**Repo:** `runtime/typescript/src/types.ts` (`DdsMemberDescriptor`, `DescriptorKind`, `DdsTypeDescriptor`)

`labels` nimmt zusätzlich `bigint` auf: ein `union U switch (long long)`
ist gültiges IDL 4.2, seine Case-Labels reichen über 2^53 hinaus, und
eine Label-Liste, die keinen tragen kann, widerspräche dem Typ, den sie
beschreibt. Additiv wie `int8` in §8.2.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` (B.2.1 bis B.2.6), `crates/lang-rules/tests/typescript_descriptors.rs::the_generated_types_type_check`

**Status:** `done`

### §8.3.1 Exception Marker

**Spec:** Ch8 §Exception Marker — `DdsException` Interface mit
`__dds_exception: true`.

**Repo:** `runtime/typescript/src/types.ts` (`DdsException`)

Eine Eigenschaft und keine Klasse: TypeScripts strukturelle Typisierung
hat nichts anderes, was eine Exception-Nutzlast zur Übersetzungszeit von
einem gewöhnlichen Struct trennt.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` — `"B.2.1"` (der Export steht in der Liste) und `"B.2.9"`
(`NotFound extends DdsException`).

**Status:** `done`

### §8.4 Boxed any (DdsAny)

**Spec:** Ch8 §Boxed any — `DdsAny { typeId; value }`.

**Repo:** `runtime/typescript/src/types.ts` (`DdsAny`), `runtime/typescript/src/dds_any.ts` (`boxAny`, `unboxAny`)

Beide Funktionen prüfen den Type-Guard des Deskriptors — beim Einpacken
und beim Auspacken. Eine Kiste, deren `typeId` ihren `value` nicht
beschreibt, ist schlimmer als keine, weil jeder Konsument dahinter ihr
glaubt.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` — `"B.2.4"` (`boxAny`) und `"B.2.5"` (`unboxAny`).

**Status:** `done`

### §8.5 Branded Character Helpers

**Spec:** Ch8 §Branded Character Helpers — `Char`/`WChar`-Aliases
+ `makeChar`/`makeWChar`-Factories per §7.3.2.

**Repo:** `runtime/typescript/src/branded.ts` (`Char`, `WChar`, `makeChar`, `makeWChar`)

`makeChar` nimmt genau eine Code-Einheit im Bereich U+0000..U+00FF;
`makeWChar` zählt Code-Punkte statt Code-Einheiten, nimmt also die
astrale Ebene an und weist ungepaarte Surrogate ab.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` — `"B.2.7"`.

**Status:** `done`

### §8.6 LongDouble

**Spec:** Ch8 §LongDouble — `LongDouble`-Type + `makeLongDouble`
per §7.3.3.

**Repo:** `runtime/typescript/src/branded.ts` (`LongDouble`, `makeLongDouble`)

Die Längenprüfung ist die ganze Prüfung: ein binary128 hat kein
ungültiges Bitmuster, und eine Fabrik, die den Exponenten ansähe, würde
Werte abweisen, die IEEE 754 definiert. Die Umrechnung einer Dezimalzahl
in die sechzehn Oktette steht in `runtime/typescript/src/float128.ts`.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` (B.2.7), `crates/lang-rules/tests/typescript_descriptors.rs::b_1_9_a_long_double_is_never_a_number`

**Status:** `done`

### §8.7 Reflection API

**Spec:** Ch8 §Reflection API — exports: `registerType`,
`lookupType`, `getKey`, `getTopic`, `withDefaults`, `boxAny`,
`unboxAny`, `equalKey`, `isOneOf`. Plus `GuardedTypeOf`-Helper.
Strukturelle Äquivalenz für `registerType` definiert
(deep-equal aller Felder außer `typeGuard`).

**Repo:** `runtime/typescript/src/registry.ts` (`registerType`, `lookupType`, `getKey`, `getTopic`, `withDefaults`, `isOneOf`), `runtime/typescript/src/dds_any.ts` (`boxAny`, `unboxAny`), `runtime/typescript/src/equal.ts` (`equalKey`, `descriptorsEqual`), `runtime/typescript/src/types.ts` (`GuardedTypeOf`)

`registerType` schweigt bei strukturgleicher Wiederanmeldung und wirft
bei einer strukturverschiedenen; `descriptorsEqual` vergleicht jedes
Feld tief und `typeGuard` über Referenzidentität. `equalKey` folgt einem
`ref` über `lookupType` und vergleicht Glied für Glied, `NaN` gleich
sich selbst und `+0` gleich `-0`; Map, Sequence und Array sind keine
IDL-Schlüsseltypen und werfen statt zu antworten. `isOneOf` wirft nie.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` — `"B.2.1"` bis `"B.2.9"`; die Reflexion selbst in `"B.2.2"`
(`lookupType`) und `"B.2.3"` (`getKey`).

**Status:** `done`

---

## Ch9 Verbatim Code-Generation Hook (Normative)

### §9.1 Annotation Form

**Spec:** Ch9 §Annotation Form — `@verbatim(language, placement,
text)`-Annotation; `language="ts"|"*"`-Recognition.

**Repo:** `crates/lang-rules/src/verbatim.rs::{language_matches, declaration_slots, file_slots}`

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::encoder`

**Status:** `done`

### §9.2 Placement Kinds

**Spec:** Ch9 §Placement Kinds — alle 6 PlacementKinds
(`BEGIN_FILE`, `BEFORE_DECLARATION`, `BEGIN_DECLARATION`,
`END_DECLARATION`, `AFTER_DECLARATION`, `END_FILE`).

**Repo:** `crates/lang-rules/src/verbatim.rs::{declaration_slots, file_slots}` (Platzierung an Deklaration oder Datei)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::encoder`

**Status:** `done`

### §9.3 Quoting and Escaping

**Spec:** Ch9 §Quoting and Escaping — IDL-string-escape-Resolution
vor Emission.

**Repo:** `crates/lang-rules/src/verbatim.rs::gather` (Auflösung der IDL-Maskierung)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::encoder`

**Status:** `done`

### §9.4 Multi-Line Text + Multiple Annotations

**Spec:** Ch9 §Multi-Line + Multiple — Newlines, source-order,
no-coalescing.

**Repo:** `crates/lang-rules/src/verbatim.rs::gather` (mehrzeiliger Text, mehrere Annotationen je Platz)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::encoder`

**Status:** `done`

---

## Ch10 Code Style (Informative)

**Spec:** Ch10 — Identifier-Casing, Output-Format-Empfehlungen,
File-Layout-Optionen.

**Repo:** `crates/lang-rules/src/verbatim.rs::gather` (mehrzeiliger Text, mehrere Annotationen je Platz)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::encoder`

**Status:** `n/a (informative)`

---

## Ch11 Interface and Operation Mapping (Normative)

### §11.1 Mapping Surface

**Spec:** Ch11 §Mapping Surface — IDL `interface I` SHALL →
`IClient` + `IHandler` + `IService: ServiceDescriptor`. **Kein**
`async`-Keyword auf interface-method-signatures.

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs::{ts_interface_items, decls_of}` — je Interface ein `<I>Client`, ein `<I>Handler` und der Deskriptor

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_maps_an_interface_to_a_client_a_handler_and_a_descriptor`

**Status:** done

### §11.2 Operation Mapping

**Spec:** Ch11 §Operation Mapping — return → `Promise<R>`;
in/inout → method-params; out/inout → result-object-properties;
oneway → `Promise<void>`.

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs::{signatures, operation_signature}` — `out`- und `inout`-Parameter wandern in den aufgelösten Wert des `Promise`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_aggregates_out_parameters_into_the_resolved_value`

**Status:** done

### §11.3 Attribute Mapping

**Spec:** Ch11 §Attribute Mapping — `attribute T name` SHALL →
`get_name(): Promise<T>` + `set_name(v: T): Promise<void>`.
`readonly attribute` → kein Setter. `getraises`/`setraises`.

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs::attribute_signatures` — `get_`/`set_`; ein `readonly attribute` bekommt nur den Leser

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_maps_an_attribute_to_a_getter_and_a_setter`

**Status:** done

### §11.4 Exception Mapping

**Spec:** Ch11 §Exception Mapping — `raises (E)` → reject mit
`Error` + `cause: E`. `isOneOf`-Helper.

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs` (Zeile 395 und 420–424: die `raises`-Liste je Operation und je Attributrichtung im Deskriptor), `runtime/typescript/src/operations.ts` (`isOneOf` prüft die abgefangene Ablehnung)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_writes_the_service_descriptor_of_eleven_five`

**Status:** done

### §11.5 Service Descriptor

**Spec:** Ch11 §Service Descriptor — `ServiceDescriptor<TC,TH>`,
`OperationDescriptor`, `OperationParameterDescriptor`,
`AttributeDescriptor`, `ParameterMode`-Types.

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs::{descriptor, operation_descriptor}`, `runtime/typescript/src/operations.ts::ServiceDescriptor`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_writes_the_service_descriptor_of_eleven_five`

**Status:** done

### §11.6 Inheritance

**Spec:** Ch11 §Inheritance — `interface B : A` SHALL →
`BClient extends AClient` + `BHandler extends AHandler` +
`BService.inherits = [AService]`. Multi-Inheritance erlaubt.

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs` (Zeile 141 und 156: `extends` am Client und am Handler; Zeile 313: das `inherits`-Feld des Deskriptors trägt jede Elternschaft in Deklarationsreihenfolge)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_maps_an_interface_to_a_client_a_handler_and_a_descriptor`

**Status:** done

### §11.7 Forward Declarations

**Spec:** Ch11 §Forward Declarations — `<interface_forward_dcl>`
SHALL NOT separate Declaration produzieren. Orphan → DDS-TS-E002.

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs` — eine Vorwärtsdeklaration ohne Definition ist ein fataler Befund und trägt den Code `DDS-TS-E002`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::a_forward_declaration_without_a_definition_is_e002`

**Status:** done

---

## Annex A — IDL Examples (Informative)

**Spec:** Annex A — drei Beispiele (Bounded Sequence, Mutable Type,
Bitset).

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs` — eine Vorwärtsdeklaration ohne Definition ist ein fataler Befund und trägt den Code `DDS-TS-E002`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::a_forward_declaration_without_a_definition_is_e002`

**Status:** `n/a (informative)`

---

## Annex B — Compliance Test Suite (Normative)

### §B.1 Codegen-Profile Tests

**Spec:** Annex B §B.1 — 16 Pflicht-Tests (B.1.1–B.1.16) gegen
Codegen-Output.

**Repo:** `crates/lang-rules/tests/typescript_descriptors.rs` — 25 Tests,
nach den Marken benannt (`b_1_1_…` bis `b_1_16_…`), plus
`the_generated_types_type_check`, das neun Formen durch
`tsc --strict --noEmit` gegen die echte Laufzeit schickt.

Alle sechzehn Marken stehen. B.1.2 hat zwei Hälften und beide werden
gemessen: die Seitentabelle (`<Type>Type`, `is<Type>`, drei typisierte
Glieder) und die Klausel „No `class` keyword MUST appear in the
generator output for any IDL data type" — geprüft über elf Formen, mit
entfernten Kommentaren, weil ein Kommentar über das Wort keine Klasse
ist.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_1_every_primitive_round_trips`,
`::b_1_2_a_struct_gets_an_interface_a_descriptor_and_a_guard`,
`::b_1_2_no_class_reaches_the_output_for_any_idl_data_type`,
`::b_1_3_a_union_carries_its_labels_and_its_default_flags`,
`::b_1_4_an_enum_gets_a_const_object_and_a_descriptor`,
`::b_1_5a_a_bitset_widths_pick_their_holders_and_the_descriptor_totals_them`,
`::b_1_5b_a_bitmask_carries_its_positions_and_its_bound`,
`::b_1_6_a_module_nests_its_declarations_and_its_descriptors`,
`::b_1_7_free_standing_constants_keep_their_types`,
`::b_1_8_char_and_wchar_reach_the_descriptor_as_themselves`,
`::b_1_9_a_long_double_is_never_a_number`,
`::b_1_10_an_any_member_is_a_box_and_never_any_or_unknown`,
`::b_1_11_annotations_change_the_side_table_and_not_the_interface`,
`::b_1_12_an_exception_is_described_as_one`,
`::b_1_13_a_typedef_is_an_alias_with_one_member_named_value`,
`::b_1_14_a_mixed_module_produces_every_side_table`,
`::b_1_15_an_unrecognised_annotation_does_not_stop_the_generator`,
`::b_1_16_a_map_member_is_described_as_a_map`,
`::the_generated_types_type_check`

**Status:** `done`

### §B.2 Descriptor-Runtime Tests

**Spec:** Annex B §B.2 — 9 Pflicht-Tests gegen `@zerodds/types`-
Library-Surface.

**Repo:** `runtime/typescript/tests/descriptor-runtime.ts` — B.2.1 bis
B.2.9, je Marke ein Block. B.2.1 hat zwei Hälften: die Typ-Liste steht
als Tupel, das `tsc` prüft, und die Funktions-Liste wird zur Laufzeit
geprüft, weil ein Name, der exportiert und `undefined` ist, anstandslos
übersetzt.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` — 9 von 9 Marken: `"B.2.1"`, `"B.2.2"`, `"B.2.3"`, `"B.2.4"`, `"B.2.5"`, `"B.2.6"`, `"B.2.7"`, `"B.2.8"`, `"B.2.9"`; Lauf über `runtime/typescript/run-tests.sh`.

**Status:** `done`

### §B.3 Operations-Profile Tests

**Spec:** Annex B §B.3 — 9 Pflicht-Tests gegen Operations-Profile-
Output.

**Repo:** `crates/lang-rules/tests/interface_bindings.rs` (die sechs `typescript_*`-Marken) und `crates/lang-rules/tests/typescript_operations_profile.rs` (`tsc --strict` über den erzeugten Profil-Code)

**Tests:** `crates/lang-rules/tests/typescript_operations_profile.rs::the_generated_operations_profile_type_checks`

**Status:** done

### §B.4 Reference Test-Harness

**Spec:** Annex B §B.4 — Test-Harness-Definition (tsc-Verify auf
generated Output, Node.js-Runner für Descriptor-Helpers).

**Repo:** `crates/lang-rules/tests/typescript_descriptors.rs` (der
tsc-Teil: `--noEmit --strict` über den erzeugten Output),
`runtime/typescript/run-tests.sh` (der Node.js-Teil: die
Descriptor-Helfer und die WASM-Bindungen unter `node`).

Die Spec nennt das frühere Codegen-Crate als Ort; es gibt es nicht mehr,
die beiden Hälften des Harness schon. Was §B.4 verlangt — Rust-Tests
gegen den Codegen-Output, `tsc --noEmit --strict` darüber, und die
Laufzeit-Helfer in einem Node.js-Runner — steht vollständig.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::the_generated_types_type_check`,
`runtime/typescript/tests/descriptor-runtime.ts`,
`runtime/typescript/tests/wasm-bindings.ts`

**Status:** `done`

---

## Annex C — WASM-Bindings Profile (Normative)

### §C.1 Handle and Sample Types

**Spec:** Annex C §C.1 — Branded Handle-Types (Participant/Topic/
Publisher/Subscriber/Writer/Reader mit string-literal brands),
`SampleInfo`, `Sample`, `DdsGuid` (12+4 byte) mit
`makeDdsGuid`-Factory, `DataAvailableCallback`.

**Repo:** `runtime/typescript/src/wasm.ts` (`ParticipantHandle`,
`TopicHandle`, `PublisherHandle`, `SubscriberHandle`,
`DataWriterHandle`, `DataReaderHandle`, `DdsGuid`, `makeDdsGuid`,
`SampleInfo`, `Sample`, `DataAvailableCallback`)

Die Marken sind String-Literale und keine `unique symbol`: ein
Symbol-Brand ist über Modulgrenzen nominal, und ein Handle aus der
Bindung eines Anbieters passte dann nicht auf den Parameter eines
anderen. `publicationHandle` trägt die vollen sechzehn Oktette des
RTPS-GUID; eine Kürzung auf 64 Bit verlöre Vendor- und Host-Anteil.

**Tests:** `runtime/typescript/tests/wasm-bindings.ts` — `"C.5.1"` prüft `prefix.length === 12` und `sequenceNumber`.

**Status:** `done`

### §C.2 Required Operations

**Spec:** Annex C §C.2 — DCPS-Data-Path-Operations
(createParticipant/Topic/Publisher/Subscriber/DataWriter/
DataReader/writeSample/takeSamples/setDataAvailableListener).

**Repo:** `runtime/typescript/src/wasm.ts` (`DdsBindings` nennt alle
vierzehn Operationen aus §C.2.1 bis §C.2.4; `WasmBackend` ist dieselbe
Fläche mit blanken Zahlen, wie sie eine Wasm-Exporttabelle hat;
`bindWasmBackend` verbindet beide)

Was `bindWasmBackend` zusätzlich durchsetzt, steht nicht im Typsystem:
Handle 0 als Sentinel, Lebendigkeit, die Kaskade beim Löschen, geliehene
Puffer und die Zustellung des Listeners über die Ereignisschleife.

**Tests:** `runtime/typescript/tests/wasm-bindings.ts` — `"C.5.1"`, `"C.5.2"`, `"C.5.3"`, `"C.5.4"`.

**Status:** `done`

### §C.3 Wire-Format Boundary

**Spec:** Annex C §C.3 — XCDR2-Bytes, Memory-Ownership,
DdsAny-Boundary-Rule. `--xcdr-endianness`-Flag.

**Repo:** `runtime/typescript/src/wasm.ts` (`writeSample` und
`takeSamples` nehmen und geben `Uint8Array`; `borrowed` kopiert, was
hinübergereicht wird, und `takeSamples` kopiert, was zurückkommt),
`tools/idlc/src/main.rs` (`--xcdr-endianness <BE|LE>`, Vorgabe LE)

Kein JavaScript-Objektgraph überquert die Grenze: die Nutzlast ist der
XCDR2-Oktettstrom und sonst nichts. Eine `DdsAny` ist davon keine
Ausnahme — `boxAny` auf der Erzeugerseite, der erzeugte Serialisierer,
die Oktette, der erzeugte Deserialisierer, `unboxAny` auf der
Konsumentenseite.

**Tests:** `runtime/typescript/tests/wasm-bindings.ts` (C.5.1 prüft, dass
ein nach dem Schreiben verändertes Array den Empfänger nicht erreicht;
C.5.4 den `DdsAny`-Weg), `tools/idlc/src/main.rs::tests::generate_takes_the_dds_ts_spelling_of_the_endianness_flag`

**Status:** `done`

### §C.4 Browser vs Node.js Bindings

**Spec:** Annex C §C.4 — Browser via wasm-bindgen,
Node.js via N-API (optional).

**Repo:** `runtime/typescript/src/wasm.ts` (`WasmBackend`,
`bindWasmBackend`)

Eine Fläche für beide Profile. Der Browser liefert ein `.wasm`-Modul,
Node.js darf zusätzlich ein `.node`-Modul über N-API liefern; beide
erfüllen `WasmBackend`, und `bindWasmBackend` gibt beidemal dasselbe
`DdsBindings` zurück. Eine Anwendung wechselt damit die Import-Zeile und
sonst nichts. Die Laufzeit selbst hängt an nichts aus Node: sie muss im
Browser laufen, und ein `@types/node` unter ihren Abhängigkeiten sagte
etwas anderes.

**Tests:** `runtime/typescript/tests/wasm-bindings.ts` — `"C.5.1"` bindet gegen ein Backend, das weder das Browser- noch
das Node-Modul ist; gemessen wird damit die Bindungsschicht, nicht das
jeweilige Modul.

**Status:** `done`

### §C.5 Conformance Tests

**Spec:** Annex C §C.5 — 4 Pflicht-Round-Trip-Tests.

**Repo:** `runtime/typescript/tests/wasm-bindings.ts` (C.5.1
Publish/Subscribe-Rundlauf, C.5.2 Listener, C.5.3 Ressourcen-Freigabe,
C.5.4 `DdsAny`-Rundlauf), `runtime/typescript/src/test_backend.ts` (der
In-Memory-Datenpfad, gegen den sie laufen)

Der Referenz-Backend ist absichtlich synchron: er ruft den Listener
innerhalb von `writeSample` auf, was §C.2.4 einer *Bindung* verbietet —
und genau das macht die Verzögerung in `bindWasmBackend` messbar.

**Tests:** `runtime/typescript/tests/wasm-bindings.ts` — 4 von 4: `"C.5.1"`, `"C.5.2"`, `"C.5.3"`, `"C.5.4"`, über `runtime/typescript/run-tests.sh`.

**Status:** `done`

### §C.6 Reservation of Future Extensions

**Spec:** Annex C §C.6 — Reserved-Identifier-Liste.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Annex D — Diagnostic Codes (Normative)

### §D.1 Diagnostic Code Registry

**Spec:** Annex D — Codes E001 (reserved), E002 (forward-decl
orphan), E003 (annotation conflict), E004 (strict-annotations),
W001 (reserved), W002 (unknown annotation), W003 (map key
hazard), W004 (implicit default), I001 (long double info).

**Repo:** —

**Tests:** —

**Status:** open — der Baum führt eine stabile Code-Menge, aber eine andere:
`crates/idl-parse/src/diagnostic.rs::DiagnosticCode` vergibt `IDL0001` bis
`IDL0018`. Die Annex-D-Codes `E001`-`E004`, `W001`-`W004` und `I001` kommen
im Baum nicht vor, und kein Test prüft sie.

---

## Audit-Status

58 done / 0 partial / 1 open / 13 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-lang-rules` — 50 Testbinaries,
1024 Tests grün, 0 failed; darunter
`--test typescript_side_tables` (12), `--test typescript_descriptors` (26),
`--test typescript_reader_generates` (16), `--test typescript_codec` (3),
`--test typescript_names_its_runtime` (2) und
`--test typescript_operations_profile` (1).
`runtime/typescript/run-tests.sh` — 61 Marken grün (16 Oktett-Fälle,
32 Wert-Fälle, 9 Descriptor-Runtime-Marken, 4 WASM-Konformanztests),
0 failed.

Wire-Messung zur Oberfläche von §7.5, §7.6 und §7.9.1: 119 Fälle der
Zelle `matrix/zerodds-typescript/cyclone-xcdr2-le` neu durch
`tools/wireprobe/wireprobe-zerodds-typescript` gefahren und Oktette wie
zurückgelesene Werte gegen die abgelegten Artefakte gehalten — 244 von
244 Vergleichen gleich, 0 verschieden.

Offene Punkte: siehe `dds-ts-1.0.open.md`. Decision-Records: keine.
