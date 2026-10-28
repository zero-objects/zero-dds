# IDL-Werkzeugkette, Codegen und Sprachanbindungen

Diese Seite beschreibt den Weg von IDL-Text zu Quellcode in 21 Zielsprachen, die Werkzeuge `zerodds-idlc` und `zerodds-xmlc`, die DDS-Sprachanbindungen unter `crates/` (C-ABI, C++, Python, Java, C#, TypeScript, Rust) und die nativen Endpoint-SDKs unter `endpoints/`. Grundlage ist der Code auf `main`. Aussagen, die nur aus Kommentaren oder READMEs stammen, sind mit „(laut Doku-Kommentar)“ markiert. Die Flags stammen aus `tools/idlc/src/main.rs` und aus `zerodds-idlc --help`; was ausgeführt wurde, ist als gemessen gekennzeichnet.

## Verwandte Seiten

- [README.md](README.md)
- [architecture.md](architecture.md)
- [crates-core.md](crates-core.md)
- [crates-bridges-services.md](crates-bridges-services.md)
- [crates-languages-codegen.md](crates-languages-codegen.md)
- [config.md](config.md)
- [cli-options.md](cli-options.md)
- [tools-and-man.md](tools-and-man.md)
- [system-setup.md](system-setup.md)
- [security.md](security.md)
- [migration.md](migration.md)
- [interop-and-dialects.md](interop-and-dialects.md)
- [ros2.md](ros2.md)

---

## 1. Ergebnis in Kürze

- Ein Codegen-Pfad für alle Sprachen: `idl-parse` → `idl-cst` → `idl-lower` → `ast` → `ast-layout` → `lang-rules` → `lang-cst` → `lang-render`. Der Einstieg für Anwender ist `zerodds-idlc generate --lang <NAME>`, für Rust-Buildskripte `zerodds-build`.
- `--lang` akzeptiert genau 21 Namen: `rust`, `c++`, `c`, `java`, `c#`, `python`, `typescript`, `go`, `ocaml`, `ada`, `d`, `elixir`, `julia`, `lua`, `nim`, `swift`, `zig`, `node`, `ada83`, `f#`, `kotlin` (`Language::ALL`, `crates/lang-cst/src/common.rs:76`).
- Erzeugter Code braucht die dünne Wire-Laufzeit der Sprache: `runtime/<lang>/`, für Rust `crates/wire`. Eine DDS-Anbindung (Participant, Writer, Reader) gibt es für C, C++, Python, Java, C#, TypeScript und Rust. Die übrigen Sprachen erreichen DDS über die Endpoint-SDKs in `endpoints/<lang>/` (XRCE-Framing zu einem Rust-Hub, ADR 0013).
- Die Crates `crates/idl`, `crates/idl-java` und `crates/idl-csharp` sind keine Cargo-Pakete. `crates/idl` enthält nur leere Verzeichnisse, die anderen beiden nur Laufzeit-Quelldateien (`runtime/`).
- Die Dokumente verwenden die aktuelle CLI-Syntax `generate --lang … --out …`. Die CMake-Funktion `zerodds_idlc_generate` ruft `generate --lang <backend> --out … -I … --stamp … --depfile … --output-manifest …` (`cmake/tests/run-idlc-generate.sh`: Neubau bei geändertem `#include`).

---

## 2. Pipeline: IDL-Text → Quellcode

```text
.idl
  │  zerodds-idl-parse   preprocess (#include/#define) + rekursiver Abstiegsparser, Dialect
  ▼
CST (zerodds-idl-cst)    verlustfrei, Trivia erhalten, byte-genau reproduzierbar
  │  zerodds-idl-lower   Scopes, Namensbindung, Auflösung, Completion (@autoid/@id), Validierung
  ▼
AST (zerodds-ast über zerodds-ast-core)   quellneutrale Knoten + Facetten (XTypes, CORBA, ROS 2, XML …)
  │  zerodds-idl-compose bündelt die drei Schritte + resolve + Zyklenprüfung + TypeObject-Registrierung
  │
  ├─► zerodds-types::from_ast        Minimal/Complete TypeObject, TypeIdentifier, TypeInformation
  ├─► zerodds-ast-layout             XCDR1/XCDR2-Layoutplan (Member-Pläne, Length Codes, Key Holder)
  ▼
zerodds-lang-rules       Ast → ModelGraph → apply(rule::catalogue(lang)) → Materialiser (adapter/)
  │                      + Codec-Walk (emit/encode.rs, emit/decode.rs) über den Layoutplan
  ▼
zerodds-lang-cst         typisierte CSTs der Zielsprachen (je Sprache ein Modul)
  │  zerodds-lang-render rendert CST zu Text, entscheidet nichts selbst
  ▼
Quelltext  +  Präambel/Companion aus tools/idlc (Import der Laufzeit, Endianness-Konstante, TypeSupport)
```

Belege:

- `tools/idlc/src/main.rs:744-809` (`analyse`): ruft `zerodds_idl_compose::compose_source_with` mit `PathLoader::new` über die `-I`-Verzeichnisse und `ComposeOptions { dialect, default_extensibility, … }`; ohne `--default-extensibility` kommt `default_extensibility` aus `--dialect`, die Lowering-Optionen aus `DialectLowering::for_dialect`.
- `crates/idl-compose/src/lib.rs:290-346`: `preprocess_with` → `parse_preprocessed_with` → `lower_with` → `resolve` → `inadmissible_cycles` → `from_ast::register_all`.
- `crates/lang-rules/src/generate.rs:7-12` (Doku-Kommentar) und `generate::source` (Zeile 679): `Ast → ModelGraph → apply(model, rule::catalogue(language)) → Materialiser → Renderer → Text`.
- `tools/idlc/src/main.rs:1027-1041`: `run_generate` bildet den `LayoutKey` aus `--dialect`, `--encoding`, `--endian` und den Achsen-Flags (`dialect_settings_from_overrides`) und übergibt ihn an `generate::files_with_mapping` (`main.rs:1078`). `tools/idlc/src/generate.rs:353`: `source_with_preamble_mapping` ruft `zerodds_lang_rules::generate::source_with_mapping`, für C# `csharp_source_with_runtime_and_options`, für C++ `cpp_source_with_mapping`; `source` (`generate.rs:324`) ist nur ein Wrapper für Tests.
- Die Regel-Engine ist `seesaw-tgg` 2.0 als Versionsabhängigkeit (laut Doku-Kommentar, `crates/lang-rules/src/lib.rs:15-19`).

### Crates der Werkzeugkette

| Crate | Paketname | Aufgabe (aus `Cargo.toml` `description`, geprüft gegen `src/`) |
|---|---|---|
| `crates/idl-parse` | `zerodds-idl-parse` | Handgeschriebener Parser für OMG IDL 4.2, Fehlererholung, Präprozessor (`src/preprocess/`), `Dialect::STRICT` und `Dialect::ALL_VENDORS` (`src/dialect.rs:31,40`) |
| `crates/idl-cst` | `zerodds-idl-cst` | Verlustfreier CST (Green/Red-Tree) |
| `crates/idl-lower` | `zerodds-idl-lower` | CST → AST: Scopes, Auflösung, Completion (`complete.rs`), Annotationen (`annotation.rs`), Validierung (`validate.rs`) |
| `crates/idl-compose` | `zerodds-idl-compose` | Ein Aufruf für alle Build-Zeit-Nutzer: Include-Suchpfade, Präprozessor, Parse, Lower, TypeObjects |
| `crates/ast-core` | `zerodds-ast-core` | Arenas, stabile IDs, Transaktionen, Snapshots; kein DDS-Wissen |
| `crates/ast` | `zerodds-ast` | Gemeinsames AST mit Facetten (`src/facet/`: `xtypes`, `corba`, `ros2`, `xml`, `flatdata`, `vendor`) |
| `crates/ast-layout` | `zerodds-ast-layout` | Layoutplan XCDR1/XCDR2, `Encoding`, `Endianness`, `LayoutKey`, KeyHash |
| `crates/types` | `zerodds-types` | XTypes 1.3: TypeIdentifier, TypeObject, Assignability, DynamicType, TypeLookup; `from_ast.rs` erzeugt TypeObjects aus dem AST |
| `crates/lang-cst` | `zerodds-lang-cst` | CSTs der 21 Zielsprachen (`src/<lang>/`). Die Paketbeschreibung nennt nur „Rust, C++, C and Java“ und ist veraltet |
| `crates/lang-render` | `zerodds-lang-render` | CST → Text, je Sprache `src/<lang>.rs` |
| `crates/lang-rules` | `zerodds-lang-rules` | Regeln als Daten (`rule/`), Materialiser (`adapter/`), Codec-Emitter (`emit/`), Table-9-Defaults (`default_<lang>.rs`), `generate.rs` |
| `crates/zerodds-build` | `zerodds-build` | `build.rs`-Helfer: ein `.rs` je `.idl` in `OUT_DIR`, `rerun-if-changed` auch für `#include`-Dateien, optional `DdsType`-Impls |
| `crates/proto` | `zerodds-proto` | Protobuf `FileDescriptorSet` → XTypes `DynamicType`. Nur Bibliothek, kein CLI, kein Codegen-Pfad durch `lang-rules` |
| `crates/xml-compose` | `zerodds-xml-compose` | DDS-XML-Typdefinitionen → gemeinsames AST (Gegenstück zu `idl-compose`) |
| `crates/corba-codegen` | `zerodds-corba-codegen` | CORBA 3.3 Annex A.1 Sondertypen, Stub-/Skeleton-Mappings über dem AST |
| `crates/corba-rust` | `zerodds-corba-rust` | IDL → Rust für CORBA-Konstrukte (Interfaces, Valuetypes, Components) |
| `crates/dlrl-codegen` | `zerodds-dlrl-codegen` | DLRL-Objekt-/Home-Klassen (DDS 1.4 Annex B.4) für C++, C#, Java, TypeScript |
| `crates/flatdata-derive` | `zerodds-flatdata-derive` | `#[derive(FlatStruct)]` für Zero-Copy-SHM, kein IDL-Pfad |
| `crates/wire` | `zerodds-wire` | Laufzeit, die erzeugter Rust-Code aufruft (`Writer`/`Reader`, XCDR1/XCDR2) |
| `crates/wire-codec` | `zerodds-wire-codec` | Führt einen `ast-layout`-Plan gegen einen Wert aus, reflektiv |
| `crates/idl` | – | Kein Paket. Nur leere Verzeichnisse `src/grammar/` und `.DS_Store`, nicht versioniert (`git ls-files crates/idl` leer) |
| `crates/idl-java` | – | Kein Paket. `runtime/`: Java-Annotationen (`Key`, `Id`, `Optional`, `External`, `Nested`, `Extensibility`, `MustUnderstand`, `Shared`, `TopicType`) und `rpc/`. Wird von `crates/java-omgdds/java/pom.xml` als Quellverzeichnis eingebunden |
| `crates/idl-csharp` | – | Kein Paket. `crates/idl-csharp/runtime/Omg.Types.cs` (Sequenzverträge, Annotation-Attribute, `ITopicType<T>`). Wird von `crates/cs/csharp/ZeroDDS.Cdr/ZeroDDS.Cdr.csproj` eingebunden |

Die früheren Crates `zerodds-idl`, `idl-cpp`, `idl-rust`, `idl-python` und die übrigen `crates/idl-<lang>` sind entfernt. Ersatz ist `idl-parse` → `idl-lower` → `lang-rules` (Kommentar in `Cargo.toml` Zeilen 332-335 und `crates/zerodds-c-api/Cargo.toml` `[dev-dependencies]`).

---

## 3. `zerodds-idlc`

Paket `tools/idlc` (`zerodds-idlc`), Binary `zerodds-idlc`, `publish = true`. Man-Page `man/man1/zerodds-idlc.1`.

Bauen:

```sh
cargo build --release -p zerodds-idlc
# Binary: target/release/zerodds-idlc
```

### Subcommands (`tools/idlc/src/main.rs:117-128`)

| Subcommand | Wirkung |
|---|---|
| `check` | Parsen und Lowering, alle Diagnosen melden |
| `lint` | Sechs Regeln für gültige Dateien (`tools/idlc/src/lint.rs`). Läuft nur, wenn `check` fehlerfrei ist |
| `format` | Kanonische Form nach stdout; `--check` meldet, `--write` schreibt zurück |
| `dump-ast` | Modell als Baum |
| `print-deps` | Die Typen, die jede Deklaration nennt |
| `generate` | Quellcode einer Sprache |

### Optionen (`parse_args`, `main.rs:381-735`)

| Option | Gilt für | Werte / Default |
|---|---|---|
| `--strict` | alle | nur OMG IDL 4.2 (`Dialect::STRICT`); ohne Flag `Dialect::ALL_VENDORS` |
| `--json` | alle | maschinenlesbare Diagnosen |
| `--deny-warnings` | alle | Exit 1 bei Warnungen |
| `--default-extensibility <final\|appendable\|mutable>` | alle | ohne Flag gilt die Spec-Vorgabe `appendable`; explizite Annotation im Quelltext hat Vorrang |
| `--check`, `--write` | nur `format` | schließen sich gegenseitig aus |
| `--lang <NAME>` | nur `generate`, Pflicht | einer der 21 Namen aus Abschnitt 4 |
| `--out <DIR>` | nur `generate` | Default `.` |
| `--encoding <ENC>` | nur `generate` | `xcdr1` = `plain-cdr1`, `xcdr2` = `plain-cdr2` (Default), `pl-cdr1`, `pl-cdr2`, `delimited-cdr` |
| `--endian <le\|be>` | nur `generate` | Default `le`; groß/klein egal |
| `--xcdr-endianness <BE\|LE>` | nur `generate` | Zweitschreibweise von `--endian` (DDS-TS 1.0 §C.3.1) |
| `--dialect <NAME>` | nur `generate` | einer der elf CDR-Dialekte; Default `specification` ([interop-and-dialects.md](interop-and-dialects.md), 6.3) |
| `--<achse> <WERT>` | nur `generate` | überschreibt eine Antwort des Dialekts; Namen und Werte in `docs/wire-axis-flags.md` |
| `-I`, `--include <DIR>` | alle | Suchverzeichnis für quoted `#include` nach dem Verzeichnis der einbindenden Datei; wiederholbar |
| `--stamp <FILE>` | nur `generate` | zuletzt geschrieben, nur bei erfolgreichem Lauf |
| `--depfile <FILE>` | nur `generate` | Make-Abhängigkeitsdatei (Ziel: Stamp, sonst Manifest) mit jeder gelesenen IDL-Datei |
| `--output-manifest <FILE>` | nur `generate` | JSON der gelesenen und geschriebenen Dateien |
| `-h`, `--help` / `-V`, `--version` | – | – |

Jede andere Option mit führendem `-` wird mit Exit 2 abgelehnt. Quoted Includes werden neben der einbindenden Datei, dann in den `-I`-Verzeichnissen und zuletzt relativ zum Arbeitsverzeichnis gesucht (`crates/idl-compose/src/lib.rs:94-105`; `-I`, `--stamp`, `--depfile`, `--output-manifest` in `main.rs:450-490`).

Exit-Codes: `0` nichts zu melden, `1` Diagnose oder `format --check` fand eine Datei, `2` Kommandozeile falsch, `3` Datei nicht lesbar.

Wirkung von `--encoding` und `--endian` (laut Doku-Kommentar `main.rs:50-65`, durch Test `the_encoding_flag_changes_the_emitted_codec` gestützt):

- `--encoding` ändert den erzeugten Codec. Es wählt die XCDR-Version. `xcdr2`, `delimited-cdr` und `pl-cdr2` erzeugen denselben Text, ebenso `xcdr1` und `pl-cdr1`. Die Kodierung je Typ folgt aus seiner Extensibility (XTypes 1.3 §7.4.2).
- `--endian` ändert den Codec nicht. Es setzt nur die Endianness-Konstante in der Präambel. Die Laufzeiten tragen die Byte-Reihenfolge in `Writer`/`Reader`.

### Ausgabedateien (`tools/idlc/src/generate.rs:93-221`)

Ohne Sonderfall: `<out>/<stem>.<ext>`, `<stem>` = Dateiname der IDL ohne Endung. Endungen aus `Language::extension()`.

| Sprache | Dateien |
|---|---|
| Java | `ZdTypes.java`, `ZdCodec.java`, dazu `ZdTypeSupports.java`, wenn topic-fähige Aggregate vorhanden sind |
| C# | `<stem>.cs`, dazu `<stem>.TypeSupport.cs` bei topic-fähigen Aggregaten |
| Ada | `zd_types.ads`, `zd_types.adb` (Body entfällt ohne Codec) |
| Ada 83 | `zd83_types.ads`, `zd83_types.adb` |
| Python | `<stem>.py`, am Ende `ZD_TOPIC_DESCRIPTORS` (`python_support.rs`) |
| C++ | `<stem>.hpp`, am Ende `dds::topic::topic_type_support<T>`-Spezialisierungen für jedes nicht-`@nested` Aggregat (`topic_traits`, `generate.rs:927`) |
| alle anderen | eine Datei `<stem>.<ext>` |

Die Java- und Ada-Dateinamen hängen nicht vom Stem ab. Eine zweite IDL-Datei in dasselbe `--out` wird dort mit `error: … was already written for an earlier input in this run` abgewiesen, Exit 1 (gemessen mit `--lang java` und `--lang ada`; Test `two_inputs_writing_one_output_file_are_refused` in `tools/idlc/tests/build_contract.rs`).

Tests: `tools/idlc/src/main.rs` (`mod tests`, unter anderem `every_language_emits_a_codec_beside_its_declarations`, `a_model_with_a_codec_gets_its_runtime_preamble`), `tools/idlc/tests/{annotation_arguments,corpus,default_extensibility,strict}.rs`, Konsumenten-Fixtures `tools/idlc/tests/{csharp-support,java-support,java-typelookup,python-support}/`.

---

## 4. Zielsprachen von `generate`

Quelle: `Language::ALL`, `Language::as_str`, `Language::extension` (`crates/lang-cst/src/common.rs:76-150`); Präambeln aus `tools/idlc/src/generate.rs:605-884`.

| `--lang` | Endung | Import der Laufzeit in der erzeugten Datei | Laufzeit im Repo |
|---|---|---|---|
| `rust` | `.rs` | `use zerodds_wire::{…};` | `crates/wire` (`zerodds-wire`) |
| `c++` | `.hpp` | `#include "zerodds/wire.hpp"`, bei Topic-Typen zusätzlich `dds/topic/TopicTraits.hpp`, `dds/topic/xcdr2.hpp` | `runtime/cpp/include/zerodds/wire.hpp`; Traits in `crates/cpp/include/dds/topic/` |
| `c` | `.h` | `#include "zerodds_wire.h"` | `runtime/c/include/zerodds_wire.h` |
| `java` | `.java` | `import org.zerodds.wire.*;` | `runtime/java` |
| `c#` | `.cs` | `global::ZeroDds.Wire` (voll qualifiziert) | `runtime/csharp/ZeroDds.Wire.csproj` |
| `python` | `.py` | `from zerodds_wire import …` | `runtime/python` (Paket `zerodds-wire`) |
| `typescript` | `.ts` | `from "@zerodds/wire"` | `runtime/typescript` (Paket `@zerodds/wire`) |
| `go` | `.go` | `import "zdwire"`, Paket `zdgen` | `runtime/go` |
| `ocaml` | `.ml` | Modul `Zdwire` ohne Import | `runtime/ocaml/zdwire.ml` |
| `ada` | `.ads`/`.adb` | `with Zd_Wire;` | `runtime/ada` (`zd_wire.gpr`) |
| `d` | `.d` | `import zdwire;` | `runtime/d/zdwire.d` |
| `elixir` | `.ex` | `ZeroddsWire` voll qualifiziert | `runtime/elixir` |
| `julia` | `.jl` | `using .ZeroddsWire` | `runtime/julia/zerodds_wire.jl` |
| `lua` | `.lua` | `require("zerodds_wire")`, Datei endet mit `return ZD` | `runtime/lua/zerodds_wire` |
| `nim` | `.nim` | `import zdwire` | `runtime/nim/zdwire.nim` |
| `swift` | `.swift` | `import ZdWire` | `runtime/swift` |
| `zig` | `.zig` | `@import("zdwire.zig")` | `runtime/zig/zdwire.zig` |
| `node` | `.js` | `require("./zdwire.js")` | `runtime/node/zdwire.js` |
| `ada83` | `.ads`/`.adb` | `with Zd_Wire83;` | `runtime/ada-83` (`zd_wire83.gpr`) |
| `f#` | `.fs` | `ZdWire` voll qualifiziert, `module rec ZdGen` | `runtime/fsharp/ZdWire.fs` |
| `kotlin` | `.kt` | `import zerodds.wire.*` | `runtime/kotlin/ZdWire.kt` |

Hinweise:

- `c++`, `c#` und `f#` enthalten Sonderzeichen. In der Shell quoten: `--lang 'c++'`, `--lang 'c#'`, `--lang 'f#'`.
- Ein Modell ohne Codec (zum Beispiel ein leeres Modul) bekommt keinen Laufzeit-Import (Test `a_model_without_a_codec_gets_no_runtime_preamble`).
- Gemessener Stand je Sprache (Oktettvergleiche, bekannte Lücken bei C, C++, F#, Kotlin; Ada, Ada 83, D auf macOS ungemessen): `docs/thin-frontends.md`. Historie der Sprachspalten: `docs/language-columns-status.md`.
- Codegen-Tests je Sprache: `crates/lang-rules/tests/<lang>_end_to_end.rs`, `<lang>_codec.rs`, `<lang>_corpus_compiles.rs` (für `rust`, `cpp`, `java`, `csharp`, `python`, `typescript`).

---

## 5. Unterstützte IDL-Konstrukte und Annotationen

Im Modell geprüft. Ob jede Sprache jeden Fall abbildet, entscheidet ihr Materialiser. Ein nicht abbildbarer Fall ergibt `GenerateError::NoForm` oder `GenerateError::Codec` und damit einen Fehler, keine stille Auslassung (`crates/lang-rules/src/generate.rs:42-55`; Test `crates/lang-rules/tests/kotlin_and_node_refuse_rather_than_substitute.rs`).

Typarten (`TypeKind`, `crates/ast/src/node/type_.rs:203-360`):

- `Primitive`, `Fixed { digits, scale }` (`fixed<P,S>`), `String` (`string`/`wstring`, optional gebunden), `Sequence` (optional gebunden), `Array` (mehrdimensional), `Map` (`map<K,V>`/`map<K,V,N>`)
- `Structure` (Einfachvererbung), `Union` (mit `default:`), `Enumeration`, `Bitmask`, `Bitset` (mit Vererbung), `Alias` (`typedef`)
- `Valuetype`, `ValueBox`, Interfaces, Exceptions, `AnnotationType`
- `long double` wird in 16 Byte über `crates/float128` getragen (Paketbeschreibung).

Annotationen im Modell (`XTypesFacet`, `crates/ast/src/facet/xtypes.rs:243-431`):

- Extensibility: `@final`, `@appendable`, `@mutable` (Feld `extensibility`), Default steuerbar mit `--default-extensibility`
- `@key` (auch am Union-Diskriminator), `@optional`, `@external`, `@must_understand`, `@non_serialized`, `@nested`, `@default_nested`
- `@id`, `@autoid`, `@hashid` (über `MemberIdAssignment`, `AutoId`)
- `@bit_bound`, `@default`, `@default_literal`, `@range`/`@min`/`@max`, `@unit`
- `@try_construct` (am Member, am Collection-Element, am Map-Key, am Diskriminator)
- `@verbatim`, `@topic`, `@data_representation`, `@ignore_literal_names`
- Vendor-Key-Pragmas werden im Lowering zu `@key` (laut Doku-Kommentar, `crates/idl-compose/src/lib.rs:23`)

Validierung im Front-End:

- Rekursion ist nur über `sequence`, `map` oder `@optional` zulässig (IDL 4.2 §7.4.1.4.4.4.4, `inadmissible_cycles` in `crates/idl-compose/src/lib.rs`).
- `generate` bricht bei jedem Parse- oder Lowering-Fehler ab, bevor etwas geschrieben wird (`run_generate`, `main.rs:969-979`).

---

## 6. `zerodds-xmlc`

Paket `tools/xmlc` (`zerodds-xmlc`), Abhängigkeit nur `crates/xml`. Man-Page `man/man1/zerodds-xmlc.1`. Das Werkzeug erzeugt keinen Code und öffnet keinen Socket.

| Subcommand | Wirkung |
|---|---|
| `validate FILE...` | Parst alle fünf DDS-XML-Bibliotheken (Typen, QoS, Domain, Participant, Application), meldet jeden Fehler |
| `types FILE...` | Listet die `<types>`-Definitionen je Bibliothek |
| `render FILE...` | Gibt das Deployment aus: Domains, Participants, Endpoints |

Optionen: `-h`/`--help`, `-V`/`--version` (`tools/xmlc/src/main.rs:60-67`); `--skip-unknown` lässt unbekannte Elemente mit Pfadmeldung aus, `--vendor-forms` liest Herstellerformen (`main.rs:109-112`). Jede andere Option ergibt Exit 2 (`tools/xmlc/src/main.rs:101-118`). Exit-Codes wie `zerodds-idlc`.

Für DDS-XML-Typen als Codegen-Quelle gibt es `crates/xml-compose` (DDS-XML → AST). Ein CLI-Pfad von XML zu Quellcode wurde nicht gefunden.

---

## 7. DDS-Sprachanbindungen unter `crates/`

### 7.1 `crates/zerodds-c-api` (C-ABI)

- Paket `zerodds-c-api`, Bibliotheksname `zerodds`, `crate-type = ["cdylib", "staticlib", "rlib"]`. Artefakt `target/<profil>/libzerodds.{so,dylib,a}`, unter Windows `zerodds.dll` (CMake).
- Header: `crates/zerodds-c-api/include/zerodds.h`, erzeugt von `build.rs` über cbindgen (`cbindgen.toml`). Zweiter Header: `include/zerodds_xcdr2.h`.
- 259 exportierte `extern "C"`-Funktionen (gezählt in `src/`). ABI-Snapshot: `crates/zerodds-c-api/tests/abi.snapshot.json`, geprüft von `crates/zerodds-c-api/tests/abi_compat.rs`.
- Samples sind bytebasiert: `write` nimmt bereits CDR-kodierte Bytes, `take` liefert Wire-Bytes, Freigabe mit `zerodds_buffer_free` (laut Doku-Kommentar `src/lib.rs:15-45`, geprüft an den Signaturen). Typ-Identität über die `*_typed`-Varianten mit serialisiertem COMPLETE `TypeObject`.
- Features (`Cargo.toml`): `std`, `same-host-shm`, `flatdata-loan` (Default), `delivery-iceoryx`, `security`, `fips`, `wolfcrypt`.

Einstiegsfunktionen (geprüft in `src/lib.rs`, `src/security_ffi.rs`):

| Funktion | Zweck |
|---|---|
| `zerodds_runtime_create(domain_id)` | Runtime mit Default-Konfiguration |
| `zerodds_runtime_create_from_config(domain_id, config_path, sets, n_sets)` | Konfiguration wie `--config PATH` und `--set PATH=VALUE`; `config_path = NULL` = Standard-Systemlayout; `"+path=value"` hängt an Listen an (`lib.rs:1221`) |
| `zerodds_runtime_create_ros_defaults(domain_id)` | ROS-2-Profil, bietet XCDR1 und XCDR2 an |
| `zerodds_runtime_create_secure(…)`, `zerodds_runtime_create_secure_from_env(…)` | DDS-Security, nur mit Feature `security` |
| `zerodds_runtime_destroy` | Freigabe |

Weitere Gruppen: DomainParticipantFactory (`zerodds_dpf_*`), Participant (`zerodds_dp_*`), Publisher/Subscriber (`zerodds_pub_*`, `zerodds_sub_*`), DataWriter/DataReader (`zerodds_dw_*`, `zerodds_dr_*`, vereinfacht `zerodds_writer_*`, `zerodds_reader_*`), QoS-Getter/Setter, Listener, WaitSet/Conditions, ContentFilteredTopic (`zerodds_dp_create_contentfilteredtopic`), Builtin-Topics, Loans und SHM (`zerodds_dw_loan_message`, `zerodds_dr_take_shm`), iceoryx, XCDR2-Helfer (`zerodds_xcdr2_encode`/`_decode`).

Beispiel: `crates/zerodds-c-api/examples/c_smoke.c`, Build und Lauf mit `crates/zerodds-c-api/examples/build_c_smoke.sh`:

```sh
cargo build --release -p zerodds-c-api
gcc -O2 -I crates/zerodds-c-api/include -o /tmp/zerodds_c_smoke \
    crates/zerodds-c-api/examples/c_smoke.c \
    -L target/release -lzerodds -lpthread -ldl -lm
# macOS zusätzlich: -framework CoreFoundation -framework Security
```

Tests: `crates/zerodds-c-api/tests/` (`smoke_ffi.rs`, `abi_compat.rs`, `security_ffi_e2e.rs`, `shm_loan_e2e.rs`, `waitset_event_driven.rs`, `xcdr2_c_codegen.rs`, `xcdr2_c_compile.rs`, `xcdr2_wire_vectors.rs` u. a.).

CMake-Paket: Wurzel-`CMakeLists.txt` baut `zerodds-c-api` über cargo und installiert `zerodds::zerodds-cpp`, pkg-config `cmake/zerodds.pc.in`, `cmake/zeroddsConfig.cmake.in`. Optionen `ZERODDS_BUILD_CPP`, `ZERODDS_BUILD_IDLC`, `ZERODDS_BUILD_EXAMPLES`, `ZERODDS_BUILD_TESTS`, `ZERODDS_SANITIZER`, `ZERODDS_CARGO_FEATURES`, `BUILD_SHARED_LIBS`.

`crates/sys` (`zerodds-sys`, „Stable C-ABI, basis for non-Rust bindings“) enthält 50 Zeilen ohne `extern "C"`-Export. Die C-ABI liegt vollständig in `zerodds-c-api`.

### 7.2 `crates/cpp` (C++17)

- Paket `zerodds-cpp`, `publish = false`, Header-only. CMake-Target `zerodds::zerodds-cpp` (`crates/cpp/CMakeLists.txt`, INTERFACE-Bibliothek, `CMAKE_CXX_STANDARD 17`).
- Zwei API-Ebenen:
  - `include/zerodds/dds.hpp`: RAII über `zerodds.h`. `zerodds::Runtime(domain_id)`, `zerodds::Writer`, `zerodds::Reader` (Bytes), `zerodds::TypedWriter<T>(rt, topic_name, reliable)`, `zerodds::TypedReader<T>(rt, topic_name, reliable)`. `TypedWriter<T>` benötigt `dds::topic::topic_type_support<T>`, die `zerodds-idlc generate --lang 'c++'` erzeugt.
  - `include/dds/…`: DDS-PSM-Cxx-Form (`dds/domain/DomainParticipant.hpp`, `dds/pub/Publisher.hpp`, `dds/sub/Subscriber.hpp`, `dds/topic/Topic.hpp`, `ContentFilteredTopic.hpp`, `BuiltinTopic.hpp`, `dds/core/policy/CorePolicy.hpp`, Conditions, Listener).
- Linken gegen `libzerodds` aus `zerodds-c-api`.
- Beispiel: `crates/cpp/examples/cpp_smoke.cpp`, Build mit `crates/cpp/examples/build_cpp_smoke.sh` (`g++ -std=c++17 -I crates/zerodds-c-api/include -I crates/cpp/include … -lzerodds`).
- Tests: `crates/cpp/tests/cpp_smoke_via_cargo.rs`, `crates/cpp/tests/smoke_dds_psm.cpp`; Codegen-Seite `crates/lang-rules/tests/cpp_corpus_compiles.rs`, `cpp_reader_generates.rs`; CMake-Konsumenten `cmake/tests/`.

### 7.3 `crates/py` (Python, PyO3)

- Cargo-Paket `zerodds-py`, PyPI-Name `zerodds`, Build-Backend maturin (`crates/py/pyproject.toml`). Extension-Modul `zerodds._core`, Python-Quellen in `crates/py/python/zerodds/`. `requires-python >= 3.9`. Laufzeitabhängigkeit `zerodds-wire==1.0.0rc7` (`runtime/python`).
- Ohne Feature `extension-module` ist das Crate eine Platzhalter-Bibliothek, damit `cargo build --workspace` ohne Python-Header läuft (Kommentar in `Cargo.toml`, Feature-Definition geprüft).
- Build:

  ```sh
  pip install maturin pytest
  pip install ./runtime/python
  cd crates/py && maturin develop --features extension-module
  ```

- API: `DomainParticipantFactory.instance()`, `create_participant`, Bytes-Topics (`create_bytes_topic`, `create_bytes_writer`, `create_bytes_reader`), QoS-Klassen, Listener, Conditions, `aio` (asyncio). Typisierte Topics über `IdlTopic(participant, name, descriptor)` mit einem Deskriptor aus `ZD_TOPIC_DESCRIPTORS` der mit `--lang python` erzeugten Datei (`python/zerodds/topic.py`, `IdlTopic.__init__` ruft `participant.create_descriptor_topic`). Kodiert wird nur mit dem generierten Code über `runtime/python` (`zerodds_wire`).
- Zusätzlich `python/zerodds/loader.py`: ctypes-Lader direkt gegen `libzerodds`, ohne PyO3.
- Beispiele: `crates/py/examples/01_bytes_pubsub.py`, `02_shape_pubsub.py`, `03_idl_topic.py` (mit `03_idl_topic.idl` und dem generierten Modul).
- Tests: `pytest crates/py/python/tests/`; `crates/py/tests/generated_descriptor_live.py` (zwei Prozesse, erzeugte Typen), `crates/py/tests/installed_wheel_consumer.sh`.

### 7.4 `crates/java-omgdds` (Java, reines Java)

- Kein Cargo-Paket. Maven-Projekt `crates/java-omgdds/java/pom.xml`, `groupId org.zerodds`, `artifactId omgdds`, Version `1.0.0-rc.7`, `maven.compiler.source 8`.
- Implementiert die OMG DDS Java PSM (`org.omg.dds.*`, 70 Quelldateien unter `src/main/java`) mit eigenem Java-RTPS-Stack (`org.zerodds.rtps`), ohne JNI und ohne Rust-Bibliothek (laut README; kein JNI-Code gefunden).
- Bindet als Quellverzeichnisse `crates/idl-java/runtime` und `runtime/java/src/main/java` ein (`pom.xml`, build-helper-maven-plugin).
- Build: `mvn -f crates/java-omgdds/java/pom.xml package`.
- Erzeugte Typen: `zerodds-idlc generate --lang java` → `ZdTypes.java`, `ZdCodec.java`, `ZdTypeSupports.java`. `ZdTypeSupports` liefert `TopicTypeSupport<T>`-Instanzen für die Writer/Reader dieser Anbindung (laut `tools/idlc/README.md`).
- Beispiel: `crates/java-omgdds/java/examples/TopicTypedSmoke.java` (handgeschriebener `TopicTypeSupport`, nur Wire-Ebene).
- Tests: `crates/java-omgdds/java/src/test/java` (15 Dateien), `tools/idlc/tests/java-support/`, `tools/idlc/tests/java-typelookup/`, `crates/java-omgdds/run_grpc_bridge_e2e.sh`.

### 7.5 `crates/cs` (C#, .NET 8)

- Kein Cargo-Paket. .NET-Projekte unter `crates/cs/csharp/`:
  - `ZeroDDS` (PackageId `ZeroDDS`, `net8.0`): DDS-API über P/Invoke gegen `zerodds` (`src/Native.cs`, `LibName = "zerodds"`), Dateien `Domain.cs`, `Pub.cs`, `Sub.cs`, `Topic.cs`, `Qos.cs`, `Listener.cs`, `Conditions.cs`, `Cft.cs`, `Async.cs`, `Typed.cs`.
  - `ZeroDDS.Cdr` (PackageId `ZeroDDS.Cdr`): `Xcdr2Writer`, `Xcdr2Reader`, `IDdsTopicType<T>`, `IGeneratedWireTypeSupport`; bindet `crates/idl-csharp/runtime/Omg.Types.cs` ein.
  - `ZeroDDS.Cdr.SourceGenerators` (`netstandard2.0`, Analyzer).
- Erzeugter Code: `<stem>.cs` braucht nur `ZeroDds.Wire` (`runtime/csharp`); `<stem>.TypeSupport.cs` braucht zusätzlich `ZeroDDS.Cdr` und für DDS-Entities `ZeroDDS` (laut `tools/idlc/README.md`).
- Build/Lauf: `crates/cs/csharp/build_cs_smoke.sh` (baut `zerodds-c-api`, dann `dotnet build ZeroDDS.Tests`, legt `libzerodds` neben die DLL).
- Beispiel: `crates/cs/Examples/TopicTypedSmoke.cs` (`dotnet run --project TopicTypedSmoke.csproj`).
- Tests: `crates/cs/csharp/ZeroDDS.Tests/`, `crates/cs/csharp/ZeroDDS.Cdr.Tests/`, `tools/idlc/tests/csharp-support/`, `crates/lang-rules/tests/csharp_corpus_compiles.rs`, `csharp_consumer_assembly.rs`.
- `crates/dotnet-build-lock` (`zerodds-dotnet-build-lock`, `publish = false`): nur für Tests. Prozessübergreifende Sperre, die parallele `dotnet build` auf `ZeroDDS.Cdr` serialisiert (Fehler CS2012). Verwendet von `crates/endpoint-e2e` und `crates/conformance`.

### 7.6 `crates/ts-node` (TypeScript, Node.js)

- Kein Cargo-Paket. npm-Paket `@zerodds/node` 1.0.0-rc.7, FFI über `koffi` gegen `libzerodds`.
- Bibliothekssuche (`src/native.ts:37-76`): `ZERODDS_LIB`, dann `target/{release,debug}/`, `target/<triple>/{release,debug}/`, `dist/runtimes/<platform>-<arch>/`, dann OS-Suchpfad.
- Build/Test: `npm run build` (`tsc`), `npm test` (`test/smoke.test.ts`, `xcdr2-wire-vectors.test.ts`, `qos.test.ts`, `extensibility.test.ts`).
- API: `src/dds.ts`, `src/qos.ts`, `src/cdr/` (`Xcdr2Writer`, `Xcdr2Reader`, `DdsTopicType`).
- Beispiel: `crates/ts-node/examples/topic-typed-smoke.ts` (`npx tsx …`, nur Wire-Ebene).

### 7.7 `crates/ts-wasm` (TypeScript, Browser)

- Cargo-Paket `zerodds-ts-wasm`, `crate-type = ["cdylib", "rlib"]`; npm-Paket `@zerodds/wasm`.
- Inhalt: XCDR1/XCDR2-Codec aus `zerodds-wire` für Primitive, Strings, Bytes (`src/lib.rs`), dazu eine DCPS-over-WebSocket-Laufzeit nach DDS-TS Annex C (`src/dcps/`). Live-DDS im Browser läuft über `crates/websocket-bridge` (laut Doku-Kommentar).
- Build: `npm run build:wasm` (`wasm-pack build --target web --out-dir pkg-web` und `--target nodejs --out-dir pkg-node`), `npm run build:ts`.
- Tests: `test/smoke.test.mjs`, `test/dcps.test.ts`.

### 7.8 `crates/rs` (Rust)

- Paket `zerodds-rs`, Fassade mit Re-Exports aus `zerodds-dcps` (`dcps`), `zerodds-dcps-async` (`aio`), `zerodds-qos` (`qos`), `zerodds-types` (`types`), `zerodds-monitor`, `zerodds-recorder`.
- Typisierte Topics: `create_topic::<T>` mit `T: zerodds_dcps::DdsType`. Die Impls erzeugt `zerodds-build` mit `Config::dds_type_impls(true)`. `zerodds-idlc generate --lang rust` erzeugt Typ und Codec, aber keine `DdsType`-Impls (laut `tools/idlc/README.md`; im Präambel-Code von `generate.rs` kein `DdsType` gefunden).
- Kein eigenes `examples/`-Verzeichnis; Beispiel im Doku-Kommentar von `crates/rs/src/lib.rs`.

### 7.9 Weitere Anbindungen

- ROS 2: `crates/ros2-rmw`, `crates/rmw-zerodds-shim` (C-FFI über `zerodds-ros2-rmw` und `zerodds-c-api`). Siehe [ros2.md](ros2.md).
- CORBA-Sprachabbildungen: `crates/corba-rust`, `crates/corba-codegen`. Siehe [crates-bridges-services.md](crates-bridges-services.md).
- DDS-WEB (REST-PSM): `crates/web`.

---

## 8. Native Endpoint-SDKs (ADR 0013)

`docs/adr/0013-native-endpoint-sdks.md`, Status `accepted`. Zwei Rollen: ein Rust-Hub mit vollem DDS (Discovery, Security, RTPS, XRCE-Agent `crates/xrce`) und native Endpoints ohne Rust, die über XRCE-förmiges Framing mit dem Hub sprechen. Der Transport ist ein Frame-Hook (`deliver`/`receive`), den die Anwendung füllt.

Verzeichnisse in `endpoints/` (geprüft): `ada`, `ada-83`, `ada-native`, `c`, `cpp`, `csharp`, `d`, `elixir`, `fsharp`, `go`, `java`, `julia`, `kotlin`, `lua`, `nim`, `node`, `ocaml`, `python`, `rust`, `swift`, `typescript`, `zig`. Dazu `codegen`, `golden-gen`, `xrce-agent-demo`, `examples`, `testdata`. Die ADR nennt nur C, C++, Python und Java.

- Jede Sprache hat `QUICKSTART.md`. Die meisten haben `example_sync.*`, `example_async.*`, `example_reliable.*` sowie `xrce_client.*`. Beispiel C: `cd endpoints/c && make examples` (laut `endpoints/c/QUICKSTART.md`).
- `endpoints/codegen` (`zerodds-endpoint-codegen`, `publish = false`): eigenes Binary, `zerodds-endpoint-codegen <input.idl> <out-dir> [c|python|java]`. Nutzt `idl-parse` und `idl-lower` direkt, nicht `lang-rules`. Deckt Primitive, `string`, `sequence<octet>`, geschachtelte Structs, `sequence<struct>` und `@final`/`@appendable`/`@mutable` ab (laut Doku-Kommentar `endpoints/codegen/src/main.rs:1-22`). Test: `endpoints/codegen/tests/emitted_codec.rs`, Fixtures `endpoints/codegen/idl/`.
- `endpoints/rust` (`zerodds-endpoint-rust`, `publish = false`): XRCE- und XCDR2-Client über `zerodds-wire`, `zerodds-xrce-client`; Beispiele `endpoints/rust/examples/example_{sync,async,reliable}.rs`.
- `endpoints/xrce-agent-demo`: Rust-Gegenstelle für Live-UDP.
- Tests: `crates/endpoint-e2e/tests/<lang>.rs` und `<lang>_reliable.rs`. Paketbeschreibung: „native endpoint apps (idlc-generated types + endpoint SDK) exchange typed samples with a Rust XRCE peer“. Für `fsharp`, `rust` und `ada83` gibt es nur `_reliable.rs`, für `typescript` keine Datei.

Das Verhältnis von `runtime/<lang>/` (Wire-Laufzeit für `idlc`-Ausgabe) zu den Wire-Dateien in `endpoints/<lang>/` (zum Beispiel `endpoints/python/zerodds_wire.py`) wurde nicht geprüft.

---

## 9. Übersicht je Sprache

„DDS“ = Anbindung mit Participant/Writer/Reader im Prozess. „Endpoint“ = natives SDK zum Rust-Hub (Abschnitt 8).

| Sprache | Crate/Pfad | Codegen-Flag | Laufzeit | Beispiel | Tests |
|---|---|---|---|---|---|
| Rust | DDS: `crates/rs`, `crates/dcps`; Build: `crates/zerodds-build`; Endpoint: `endpoints/rust` | `--lang rust` | `crates/wire` | `endpoints/rust/examples/example_sync.rs`; Doku-Beispiel in `crates/rs/src/lib.rs` | `crates/lang-rules/tests/rust_corpus_compiles.rs`, `crates/zerodds-build/tests/`, `crates/endpoint-e2e/tests/rust_reliable.rs` |
| C | DDS: `crates/zerodds-c-api`; Endpoint: `endpoints/c` | `--lang c` | `runtime/c` | `crates/zerodds-c-api/examples/c_smoke.c` | `crates/zerodds-c-api/tests/`, `crates/lang-rules/tests/c_end_to_end.rs`, `crates/endpoint-e2e/tests/c.rs` |
| C++ | DDS: `crates/cpp`; Endpoint: `endpoints/cpp` | `--lang 'c++'` | `runtime/cpp` + `crates/cpp/include` | `crates/cpp/examples/cpp_smoke.cpp`, `endpoints/cpp/example_sync.cpp` | `crates/cpp/tests/`, `crates/lang-rules/tests/cpp_corpus_compiles.rs`, `crates/endpoint-e2e/tests/cpp.rs` |
| Python | DDS: `crates/py`; Endpoint: `endpoints/python` | `--lang python` | `runtime/python` (`zerodds-wire`) | `crates/py/examples/01_bytes_pubsub.py` | `crates/py/python/tests/`, `tools/idlc/tests/python-support/`, `crates/endpoint-e2e/tests/python.rs` |
| Java | DDS: `crates/java-omgdds`; Endpoint: `endpoints/java` | `--lang java` | `runtime/java`, `crates/idl-java/runtime` | `crates/java-omgdds/java/examples/TopicTypedSmoke.java` | `crates/java-omgdds/java/src/test/java`, `tools/idlc/tests/java-support/`, `crates/endpoint-e2e/tests/java.rs` |
| C# | DDS: `crates/cs`; Endpoint: `endpoints/csharp` | `--lang 'c#'` | `runtime/csharp` (`ZeroDds.Wire`), `crates/idl-csharp/runtime` | `crates/cs/Examples/TopicTypedSmoke.cs` | `crates/cs/csharp/ZeroDDS.Tests/`, `ZeroDDS.Cdr.Tests/`, `tools/idlc/tests/csharp-support/`, `crates/endpoint-e2e/tests/csharp.rs` |
| TypeScript (Node) | DDS: `crates/ts-node` (`@zerodds/node`); Endpoint: `endpoints/typescript` | `--lang typescript` | `runtime/typescript` (`@zerodds/wire`) | `crates/ts-node/examples/topic-typed-smoke.ts` | `crates/ts-node/test/`, `crates/lang-rules/tests/typescript_codec.rs` |
| TypeScript (Browser) | `crates/ts-wasm` (`@zerodds/wasm`) | `--lang typescript` | `runtime/typescript`; Codec in WASM | – | `crates/ts-wasm/test/` |
| JavaScript (Node) | Endpoint: `endpoints/node` | `--lang node` | `runtime/node/zdwire.js` | `endpoints/node/example_sync.js` | `crates/lang-rules/tests/node_end_to_end.rs`, `crates/endpoint-e2e/tests/node.rs` |
| Go | Endpoint: `endpoints/go` | `--lang go` | `runtime/go` | `endpoints/go/example_sync/` | `crates/lang-rules/tests/go_end_to_end.rs`, `crates/endpoint-e2e/tests/go.rs` |
| Ada 2012 | Endpoint: `endpoints/ada`, `endpoints/ada-native` | `--lang ada` | `runtime/ada` | `endpoints/ada/QUICKSTART.md` | `crates/lang-rules/tests/ada_end_to_end.rs`, `crates/endpoint-e2e/tests/ada.rs` |
| Ada 83 | Endpoint: `endpoints/ada-83` | `--lang ada83` | `runtime/ada-83` | – | `crates/lang-rules/tests/ada83_end_to_end.rs`, `crates/endpoint-e2e/tests/ada83_reliable.rs` |
| D | Endpoint: `endpoints/d` | `--lang d` | `runtime/d/zdwire.d` | `endpoints/d/example_sync.d` | `crates/lang-rules/tests/d_end_to_end.rs`, `crates/endpoint-e2e/tests/d.rs` |
| Elixir | Endpoint: `endpoints/elixir` | `--lang elixir` | `runtime/elixir` | `endpoints/elixir/example_sync.exs` | `crates/lang-rules/tests/elixir_end_to_end.rs`, `crates/endpoint-e2e/tests/elixir.rs` |
| F# | Endpoint: `endpoints/fsharp` | `--lang 'f#'` | `runtime/fsharp/ZdWire.fs` | `endpoints/fsharp/example_sync.fsx` | `crates/lang-rules/tests/fsharp_end_to_end.rs`, `crates/endpoint-e2e/tests/fsharp_reliable.rs` |
| Julia | Endpoint: `endpoints/julia` | `--lang julia` | `runtime/julia/zerodds_wire.jl` | `endpoints/julia/example_sync.jl` | `crates/lang-rules/tests/julia_end_to_end.rs`, `crates/endpoint-e2e/tests/julia.rs` |
| Kotlin | Endpoint: `endpoints/kotlin` | `--lang kotlin` | `runtime/kotlin/ZdWire.kt` | `endpoints/kotlin/example_sync/` | `crates/lang-rules/tests/kotlin_end_to_end.rs`, `crates/endpoint-e2e/tests/kotlin.rs` |
| Lua | Endpoint: `endpoints/lua` | `--lang lua` | `runtime/lua/zerodds_wire` | `endpoints/lua/example_sync.lua` | `crates/lang-rules/tests/lua_end_to_end.rs`, `crates/endpoint-e2e/tests/lua.rs` |
| Nim | Endpoint: `endpoints/nim` | `--lang nim` | `runtime/nim/zdwire.nim` | `endpoints/nim/example_sync.nim` | `crates/lang-rules/tests/nim_end_to_end.rs`, `crates/endpoint-e2e/tests/nim.rs` |
| OCaml | Endpoint: `endpoints/ocaml` | `--lang ocaml` | `runtime/ocaml` | `endpoints/ocaml/example_sync.ml` | `crates/lang-rules/tests/ocaml_end_to_end.rs`, `crates/endpoint-e2e/tests/ocaml.rs` |
| Swift | Endpoint: `endpoints/swift` | `--lang swift` | `runtime/swift` | `endpoints/swift/example_reliable.swift` | `crates/lang-rules/tests/swift_end_to_end.rs`, `crates/endpoint-e2e/tests/swift.rs` |
| Zig | Endpoint: `endpoints/zig` | `--lang zig` | `runtime/zig/zdwire.zig` | `endpoints/zig/example_sync.zig` | `crates/lang-rules/tests/zig_end_to_end.rs`, `crates/endpoint-e2e/tests/zig.rs` |

---

## 10. So erzeugt man Typen für eine Anwendung

Die Befehle folgen aus `parse_args`. Ausgeführt wurden sie nicht.

### 10.1 Werkzeug bauen, IDL prüfen

```sh
cargo build --release -p zerodds-idlc
IDLC=target/release/zerodds-idlc

$IDLC check  idl/Robot.idl
$IDLC lint   --deny-warnings idl/Robot.idl
$IDLC format --check idl/Robot.idl
```

### 10.2 Erzeugen

```sh
# Default: XCDR2, Little Endian, Ausgabe ins aktuelle Verzeichnis
$IDLC generate --lang rust   --out gen/rust   idl/Robot.idl   # gen/rust/Robot.rs
$IDLC generate --lang 'c++'  --out gen/cpp    idl/Robot.idl   # gen/cpp/Robot.hpp
$IDLC generate --lang python --out gen/py     idl/Robot.idl   # gen/py/Robot.py
$IDLC generate --lang java   --out gen/java   idl/Robot.idl   # ZdTypes.java, ZdCodec.java, ZdTypeSupports.java

# XCDR1 für einen Peer, der XCDR1 spricht; unannotierte Aggregate als @final
$IDLC generate --lang 'c#' --encoding xcdr1 --default-extensibility final --out gen/cs idl/Robot.idl
```

Jede geschriebene Datei wird als `<idl>: wrote <pfad>` gemeldet, am Ende `generate: N file(s), E error(s), W warning(s)`.

### 10.3 In der Anwendung verwenden

Rust (Cargo): statt `idlc` den `build.rs`-Helfer verwenden. Er erzeugt zusätzlich `DdsType`-Impls und meldet Includes an cargo.

```rust
// build.rs  ([build-dependencies] zerodds-build)
fn main() -> Result<(), zerodds_build::Error> {
    zerodds_build::Config::new()
        .include_dir("idl")
        .dds_type_impls(true)
        .compile(&["idl/Robot.idl"])
}
```

```rust
// Crate-Wurzel
include!(concat!(env!("OUT_DIR"), "/Robot.rs"));
// participant.create_topic::<Robot>("entity.robot")?;
```

Quelle: Doku-Kommentar `crates/zerodds-build/src/lib.rs:7-40`, Methoden geprüft (`include_dir` Zeile 243, `dds_type_impls` Zeile 470, `compile` Zeile 622).

C++: die erzeugte `Robot.hpp` enthält `topic_type_support<T>`; sie wird mit `crates/cpp/include`, `runtime/cpp/include` und `crates/zerodds-c-api/include` übersetzt und gegen `libzerodds` gelinkt.

```cpp
#include "zerodds/dds.hpp"
#include "Robot.hpp"
zerodds::Runtime rt(0);
zerodds::TypedWriter<T> w(rt, "Robot", /*reliable=*/true);   // T = erzeugter Typ
zerodds::TypedReader<T> r(rt, "Robot", true);
```

Python: `runtime/python` und `crates/py` installieren (Abschnitt 7.3), dann:

```python
import zerodds, Robot as generated
descriptor = generated.ZD_TOPIC_DESCRIPTORS["<Modul>::<Typ>"]
topic = zerodds.IdlTopic(participant, "Robot", descriptor)
writer = topic.create_writer(participant.create_publisher())
```

Quelle: `crates/py/README.md` Abschnitt „Generated Python and native topics“, geprüft gegen `crates/py/python/zerodds/topic.py:36-87`.

Java und C#: die Companion-Dateien (`ZdTypeSupports.java`, `<stem>.TypeSupport.cs`) liefern die TypeSupport-Objekte für `crates/java-omgdds` bzw. `crates/cs`. Konsumenten-Beispiele: `tools/idlc/tests/java-support/Consumer.java`, `tools/idlc/tests/csharp-support/Program.cs`.

Übrige Sprachen: erzeugte Datei plus `runtime/<lang>/` ergibt Typ und Codec. Versand über ein Endpoint-SDK aus `endpoints/<lang>/` an einen Hub.

### 10.4 Build-Systeme

`build-integration/<ökosystem>/` ruft `zerodds-idlc generate --lang … --out …` aus dem jeweiligen Build-Werkzeug auf; jedes `sample-consumer/` erzeugt `Robot::Pose`, übersetzt es gegen `runtime/<lang>/` und schickt ein Sample hin und zurück. Gemessen (Round-Trip jeweils 40 Byte):

| Ökosystem | Host, Werkzeug |
|---|---|
| Maven, MSBuild, Mix, SwiftPM, Nimble, Julia `Pkg.build`, Lua/LuaRocks | macOS |
| dub (LDC 1.40.0), gprbuild (Ada) | Linux-Testhost |
| Gradle | nicht gebaut: kein Gradle auf beiden Hosts |

Keine der Integrationen verfolgt `#include`-Dateien; die CMake-Funktion tut es über `--depfile`. Übersicht mit Versionen: `build-integration/README.md`.

---

## 11. Unklare oder widersprüchliche Stellen

| Stelle | Befund |
|---|---|
| `crates/idl` | Unversioniertes, per `.gitignore` ignoriertes Verzeichnis; enthält nur leere Unterverzeichnisse und `.DS_Store` |
| `endpoints/<lang>/zerodds_wire.*` vs. `runtime/<lang>/` | Ob beides dieselbe Laufzeit ist, nicht geprüft |
