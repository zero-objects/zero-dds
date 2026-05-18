# WP 1.5 XTypes Full Stack — Code Review

**Scope:** Commits `921c854..HEAD` (T1–T20 + T-IDL1/2 + Builder-Teil-2).
**Reviewer-Hinweis:** Spike-Phase. Bewusste Scope-Cuts (Complete-only-Fields,
Representation-QoS Wire-Format, Live-TypeLookup-Transport) sind als solche
dokumentiert und nicht als Findings gezählt. Konzentration auf unbewusste
Schulden, Spec-Abweichungen und Hygiene-Lücken.

## Executive Summary

- **Critical Spec-Abweichung:** `EquivalenceHash` verwendet SHA-256 statt
  MD5 (XTypes 1.3 §7.3.1.2.1). Cyclone/Fast-DDS nutzen MD5. Live-Interop
  mit echten DDS-Stacks wird scheitern, sobald Hash-Matching greift (z.B.
  TypeLookup-Matching).
- **Critical Spec-Abweichung:** Die TypeLookup-Service EntityIds nutzen
  `BuiltinWriterWithKey/ReaderWithKey` (0xC2/0xC7) statt spec-konformer
  `BuiltinWriterNoKey/ReaderNoKey` (0xC3/0xC4) per XTypes §7.6.3.3.4.
- **High DoS-Schicht schwach:** Jede `decode_seq`-Stelle ruft
  `Vec::with_capacity(u32_from_wire as usize)` ohne Cap. Ein Angreifer
  kann mit einem 30-Byte-TYPE_INFORMATION-PID Gigabytes pre-allokieren.
  Gilt für ~11 Decoder-Pfade (struct/union/enum/alias/… + PL_CDR).
- **High Error-Masking:** `type_information.rs` und `type_lookup.rs`
  mappen Decode-Fehler auf falsche Variants (`UnexpectedEof{needed:0,offset:0}`
  oder `InvalidString`), was Debugging unmöglich macht und Testfehler
  verschleiert.
- **Medium API-Inkonsistenz:** `build_minimal_struct`/`build_complete_struct`
  (Struct) vs `build_minimal`/`build_complete` (alle anderen). Kein
  Muster, keine Begründung.

## Findings

| #  | Severity | File:Line | Kategorie | Beschreibung | Empfehlung |
|----|----------|-----------|-----------|--------------|------------|
| 1  | Critical | `crates/types/src/hash.rs:61` | Spec-Abweichung | `hash_bytes` nutzt SHA-256. XTypes 1.3 §7.3.1.2.1 verlangt **MD5** der serialisierten TypeObject (erste 14 Oktette). `md-5` ist bereits Dependency (NameHash), wird aber nur dort benutzt. Cyclone/Fast-DDS verwenden MD5. Hashes werden beim Live-Interop nicht matchen. | `use md5::{Digest, Md5}; let digest = Md5::digest(data); out.copy_from_slice(&digest[..14])`. `sha2`-Dep aus `Cargo.toml` entfernen, wenn nirgends sonst gebraucht. |
| 2  | Critical | `crates/rtps/src/wire_types.rs:243-262` | Spec-Abweichung | TL_SVC_* EntityIds nutzen `BuiltinWriterWithKey=0xC2` / `BuiltinReaderWithKey=0xC7`. XTypes §7.6.3.3.4 schreibt `ENTITYKIND_BUILTIN_WRITER_NO_KEY=0xC3` / `READER_NO_KEY=0xC4` vor (Service-RPC, kein Key). Cyclone wird TL-Endpoints nicht erkennen. | Auf `BuiltinWriterNoKey`/`BuiltinReaderNoKey` umstellen und Test hinzufügen, der die 4 EntityId-Bytes direkt gegen die Spec-Konstanten matched (`0x000300c3`, `0x000300c4`, `0x000301c3`, `0x000301c4`). |
| 3  | High | `crates/types/src/type_object/common.rs:559-572` (und ~11 weitere `decode_seq`-/`read_u32 as usize`-Stellen) | Security/DoS | `decode_seq`: `let len = r.read_u32()? as usize; let mut out = Vec::with_capacity(len);`. u32::MAX → ~4 GB Pre-Allocation. Selbst 10 MB × 10 Requests = OOM. Gleiches Pattern in `type_identifier/mod.rs:480`, `common.rs:140`, `annotation_type.rs:34`, `complete/mod.rs:585`, `type_lookup.rs:101`, `publication_data.rs:314`. | Gemeinsame Hilfsfunktion `fn safe_with_capacity<T>(len: usize, elem_min_size: usize, remaining: usize) -> Vec<T>`, die `len * elem_min_size <= remaining` prüft und sonst `Vec::new()` oder Fehler liefert. Alternative: `len.min(remaining / elem_min_size)` als Cap. |
| 4  | High | `crates/types/src/type_information.rs:121-132` | Error-Handling | Decode-Error-Wrapper lügt: `TypeCodecError::UnknownTypeKind {..}` wird auf `DecodeError::UnexpectedEof { needed: 0, offset: 0 }` gemappt. Debugging-unfähig — Tests, die fehlschlagen, zeigen "unexpected EOF at offset 0" statt der echten Ursache. | `decode_seq` so generalisieren, dass es `TypeCodecError` statt `DecodeError` zurückgibt (via zweite Variante der Helpers), oder `TypeCodecError` ins Innere propagieren. |
| 5  | High | `crates/types/src/type_lookup.rs:47-57, 177-181, 214-221` | Error-Handling | Drei Stellen: `TypeIdentifier::decode_from(rr).map_err(|e| zerodds_cdr::DecodeError::InvalidString {offset: 0, reason: "eof"})`. Der echte Fehler wird komplett verworfen — auch die Offsets, auch andere `DecodeError`-Variants. | Analog zu #4 die Sequence-Helpers auf `TypeCodecError`-Rückgabe umstellen. |
| 6  | High | `crates/types/src/type_lookup.rs:143-149` | Semantic/Error-Handling | `ContinuationPoint::decode_from` liefert `TypeCodecError::UnknownTypeKind { kind: 0 }` wenn `len > MAX_LEN`. Das ist semantisch falsch (kein TypeKind-Fehler) und verschleiert den wirklichen Grund. | Eigene Variante `TypeCodecError::InvalidContinuationPoint { len }` oder zumindest `DecodeError::InvalidString { offset, reason: "continuation_point too long" }`. |
| 7  | High | `crates/types/src/type_object/common.rs:226-265` + analog 336-367, 432-470 | Spec-Abweichung/Unsauber | `AppliedBuiltinTypeAnnotations`, `OptionalAppliedAnnotationSeq`, `AppliedBuiltinMemberAnnotations` kodieren "Optional" als `sequence<T, 1>` mit Länge 0 oder 1. Die `for _ in 1..len { skip }`-Schleife liest aber *weitere* ganze Sequenzen, was spec-fremd ist — XTypes §7.3.4.5.4 verlangt echtes `@optional`-Encoding (IDL `@optional T x`, Wire = 1-byte present-Flag + T). Das funktioniert nur solange kein Peer den "höchstens eins"-Constraint verletzt. | Entweder das IDL `@optional`-Encoding sauber implementieren (XCDR2 §7.4.3.1) oder die seltsame Skip-Schleife dokumentieren und einen Spec-Check-Eintrag in die Notes-Datei. Aktuell ist es eine stille Nicht-Konformität. |
| 8  | High | `crates/types/src/assignability.rs:82-134` | Halbfertige Implementierung | `check_direct` deckt Primitive, String, EK_HashMinimal↔EK_HashMinimal und `PlainSequenceSmall↔PlainSequenceSmall` ab. Fehlend: `PlainSequenceLarge`, alle `PlainArray`/`PlainMap`, `String8↔String16` (sollte "No"), asymmetrische Small↔Large-Sequences, EK_COMPLETE↔EK_COMPLETE, EK_MINIMAL↔EK_COMPLETE (§7.2.4.4 fordert Cross-Kind-Fallback). Der `_ => Assignable::No("kinds do not match")`-Catch-All maskiert das. | Jeden `TypeIdentifier`-Variant explizit behandeln, Catch-All nur als `unreachable!()` nach vollständiger Abdeckung. Mindestens `PlainSequenceLarge` + `PlainArray*` + Cross-Hash-Kind in T17-Anschluss liefern. |
| 9  | Medium | `crates/types/src/assignability.rs:246-262` | Subtle Bug | Enum-Assignability: "reader muss alle writer-values kennen". Spec §7.2.4.4.4.3 sagt das Gegenteil: **writer** (producer) muss alle values schreiben können, die **reader** (consumer) erwartet — oder beide müssen das Default-Literal haben. Die Richtung ist invertiert. | Richtung prüfen (wahrscheinlich `rl.value` gegen `we.literal_seq`) und Test mit asymmetrischem Enum hinzufügen. |
| 10 | Medium | `crates/types/src/assignability.rs:173-179` | Subtle Bug | `if w_final != r_final \|\| w_mut != r_mut`: "extensibility mismatch". Damit wird `Appendable↔Appendable` akzeptiert, aber `Appendable↔Final` und `Final↔Appendable` beides abgelehnt. Per §7.2.4.4.4.2 ist **mutual assignability** erfordert, dass **beide** dieselbe Extensibility haben — OK — aber die Condition erfasst nicht den Fall "beide haben weder FINAL noch MUTABLE" korrekt, wenn jemand `IS_APPENDABLE` nicht gesetzt hat (default empty flags). | Explizit `extensibility_of(flags)`-Helper bauen, der `Final`/`Appendable`/`Mutable` liefert, und dann `w_ext == r_ext` prüfen. |
| 11 | Medium | `crates/types/src/type_identifier/mod.rs:410-411` | Subtle Bug | `w.write_u32(scc.scc_length as u32)`. `scc_length: i32` ist signed per Spec (§7.3.4.9). Bei negativen Werten — die Spec verbietet sie nicht explizit — geht der Most-significant-Bit in den u32 über und wird beim Decoder mit `as i32` zurückgewandelt. Funktional korrekt für Two's Complement, aber ohne Validation, dass `scc_length >= 0` und `scc_index < scc_length`. | Validate auf Encode-Seite: `if scc.scc_length < 0 || scc.scc_index < 0 || scc.scc_index >= scc.scc_length { return Err(...) }`. |
| 12 | Medium | `crates/types/src/type_identifier/mod.rs:432` | Dead-Code/Defensive | `Self::Primitive(PrimitiveKind::from_u8(d).unwrap_or(PrimitiveKind::Boolean))`. Der match-Arm oben deckt nur Werte ab, die `from_u8` kennt. `unwrap_or(Boolean)` ist unerreichbar, aber wenn jemand TK_* um Werte erweitert ohne `from_u8` zu updaten, schlucken wir stumm einen falschen Typ. | `.expect("match arm guarantees Some")` oder `match d { TK_BOOLEAN => Boolean, ... }` inline. |
| 13 | Medium | `crates/types/src/builder.rs:288-294` | Halbfertige/Stille Abweichung | `autoid(HASH)` nutzt MD5[0..4] mod 2^28. Kommentar sagt selbst "vereinfacht". Spec §7.3.1.2.1.1 fordert eine bestimmte CRC-basierte Formel. Peers mit unterschiedlicher ID-Berechnung matchen Member-IDs nicht. | Formel aus XTypes 1.3 §7.3.1.2.1.1 implementieren (CRC-64 Teile), plus Test-Fixture mit Cyclone-Referenzwerten. |
| 14 | Medium | `crates/types/src/resolve.rs:108` | Performance/Algorithm | `visited.contains(&hash)` ist O(n) pro Iteration → Alias-Resolution O(n²). Bei `max_depth=64` noch OK, aber `collect_from_ti` (Zeile 251) hat dasselbe Pattern über potenziell grosse Graphen. | `BTreeSet<EquivalenceHash>` statt `Vec` für `visited`/`seen`. |
| 15 | Medium | `crates/types/src/resolve.rs:140-148` | DoS/Depth-Cap Semantik | `collect_referenced_hashes` hat `max_depth`-Parameter, aber das Cap ist **call-stack-depth**, nicht "Anzahl besuchter Knoten". Ein flacher Graph mit 10000 Children (sequence<sequence<...>>) wird nicht gecapt. | Zusätzlich `max_nodes`-Cap (z.B. `DEFAULT_MAX_NODES=4096`) und `seen.len() >= max_nodes → DepthExceeded`. |
| 16 | Medium | `crates/rtps/src/publication_data.rs:312-329` | DoS/Allocation | `let n = u32 as usize; let mut reps = Vec::with_capacity(n)`. Der Loop bricht zwar via `off + 2 > v.len()` ab, aber `with_capacity(n)` hat schon mit `n=u32::MAX/2` 2 GB reserviert. | `Vec::with_capacity(n.min(v.len() / 2))` oder `n.min((v.len() - 4) / 2)`. |
| 17 | Medium | `crates/types/src/builder.rs` | API-Design Inkonsistenz | StructBuilder liefert `build_minimal_struct` / `build_complete_struct`. EnumBuilder/AliasBuilder/UnionBuilder/SequenceBuilder/ArrayBuilder/MapBuilder/BitmaskBuilder/BitsetBuilder liefern `build_minimal` / `build_complete`. Kein Pattern, kein Grund. | Einheitlich `build_minimal` / `build_complete` in allen Buildern (Struct anpassen). Bricht API, aber Spike-Phase → akzeptabel. |
| 18 | Medium | `crates/types/src/type_object/complete/mod.rs` vs `minimal/mod.rs` | Konsistenz/Struktur | `complete` ist **eine** 755-Zeilen-Datei, `minimal` ist pro-Kind (7 Dateien). Encode/Decode-Parallele pro Kind ist nicht einfach verifizierbar. | `complete/` analog zu `minimal/` pro Kind aufsplitten (struct_type.rs, union_type.rs …). |
| 19 | Medium | `crates/types/src/type_information.rs:171-172` | Silent-Truncation | `u32::try_from(bytes_len).unwrap_or(u32::MAX)`. Wenn das TypeObject tatsächlich > 4 GB wäre (unrealistisch, aber ungekappt), geht die Grösse still auf `u32::MAX` und die Hash-Validation beim Peer scheitert mit kryptischem Fehler. | `.map_err(|_| EncodeError::ValueOutOfRange { message: "typeobject serialized size exceeds u32::MAX" })?`. |
| 20 | Medium | `crates/types/src/qos.rs:77-115` | Spec-Abweichung | `TypeConsistencyEnforcement::to_bytes_le` ist explicitly "vereinfacht" und nicht Wire-kompatibel mit Cyclone. Kommentar markiert es, aber es gibt keinen Guard (z.B. Feature-Flag), der zufällig-korrektes Encoding verhindert. | Alternativ eine `impl From<&[u8]>`-Variante einbauen, die spec-konform **und** legacy-lenient ist — oder die aktuelle Version ins QoS-WP verschieben. |
| 21 | Low | `crates/types/src/hash.rs:45-55` | Performance/Allocation | `compute_minimal_hash` klont das ganze `MinimalTypeObject` (`t.clone()`) nur um es in `TypeObject::Minimal(_)` zu wrappen. Großes TypeObject → großes Clone vor jedem Hash. | `compute_hash` so verallgemeinern, dass es direkt `encode_into(&mut BufferWriter)` nimmt + Discriminator separat schreibt, ohne Clone-Wrap. |
| 22 | Low | `crates/types/src/type_lookup.rs:86-93` | Performance/Allocation | `build_get_types_reply` klont jedes `MinimalTypeObject` um es in `ReplyTypeObject::Minimal(m.clone())` zu stecken und anschließend via `encode_into` zu serialisieren. Drain-and-Re-wrap. | Direkt `m.encode_into(w)` pro Registry-Eintrag, ohne Zwischen-`Vec<ReplyTypeObject>`. |
| 23 | Low | `crates/types/src/type_identifier/mod.rs:428-433` | Wartbarkeit | Der match-Arm listet 15 Primitive TK_-Konstanten explizit. Wenn jemand eine weitere hinzufügt (z.B. TK_INT128), wird sie zu `Unknown(d)` und silent als Primitive-`Boolean` dekodiert wegen der `unwrap_or`-Dead-Code-Falle (#12). | Per `PrimitiveKind::from_u8(d)` zuerst testen, dann `match` auf `Some(p) => Primitive(p)` bzw. spezielle Kinds. |
| 24 | Low | `crates/types/src/type_object/common.rs:191-196` | API-Smell | `VerbatimPlacement::Other(String)` serialisiert Placement-Kind als String statt als EnumId. Robust für Forward-Compat, aber spec-lax. Auch: Decode erkennt `"BEFORE_DECLARATION"` — falls Cyclone den String-Identifier anders schreibt, schlägt match fehl und wir landen in `Other` — silent semantic loss. | Test mit Cyclone-Fixture-Bytes hinzufügen (wenn Cyclone `@verbatim` unterstützt). |
| 25 | Low | `crates/types/src/type_object/minimal/union_type.rs:12-31` | Unnötig | `MinimalUnionHeader` ist ein leerer Typ mit no-op encode/decode. `Result<Self, ...>` auch no-op. 20 Zeilen für eine Zero-Size-Type-Abstraktion. | Direkt `()` oder weglassen; decode_from implicit. |
| 26 | Low | `crates/types/src/builder.rs:848-854, 912-916` | Dead-Code-Smell | `BitmaskBuilder::name()` und `BitsetBuilder::name()` als Getter, die selbst vom Builder nicht benutzt werden ("Phase-1-Only Feld"). Sie werden erst gebraucht wenn `build_complete` für diese Typen existiert (Scope-Cut). Kommentar als Entschuldigung. | Entweder `build_complete_bitmask`/`build_complete_bitset` mitliefern oder `#[allow(dead_code)]` bzw. Getter entfernen. |
| 27 | Low | `crates/types/src/builder.rs:307-329, 333-369` | Copy-Paste | `build_minimal_struct` und `build_complete_struct` sind strukturell identisch, ~60 Zeilen Code dupliziert, nur der Member-Konstruktor unterscheidet sich. Analog bei Union und Enum. | Gemeinsame Helper-Funktion, die ein `resolved_member`-Tuple `(member_id, member_name, spec)` produziert, und die beiden build_*-Pfade konsumieren nur das Closure-Mapping. |
| 28 | Low | `crates/idl/src/semantics/to_typeobject.rs:94-96` | Halbfertig/Scope-Cut | `TypeSpec::Scoped(_) => EquivalenceHashMinimal(ZERO)`. Platzhalter-Null-Hash bedeutet im Assignability-Check "unknown type" — alle scoped refs werden unassignable. Für `struct Foo { Bar b; }` wo Bar ein anderer lokaler Struct ist, liefert die aktuelle Implementation keinen nutzbaren Type-Graph. | Either: (a) zweistufiges Mapping (erst alle Structs sammeln + Hash vorberechnen, dann Refs auflösen), oder (b) eindeutig dokumentieren, dass `lower_struct_to_minimal` nur für Flat-Structs gedacht ist. |
| 29 | Low | `crates/idl/src/semantics/annotations.rs:174-176` | Subtle Bug | `const_to_string`: `s.trim_matches('"').to_string()`. Wenn ein String-Literal ein `"` am Anfang *und* am Ende hat → entfernt beide. Wenn's nur eine Seite hat (ungültige IDL) oder "mehrfache" Quotes ("""x"""), trimmt `trim_matches` alle. | Stattdessen `s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(s)`. |
| 30 | Low | `crates/idl/src/semantics/annotations.rs:277` | Silent-Default | `BuiltinAnnotation::BitBound(const_to_u32(e).unwrap_or(32) as u16)`. Bei invalidem Argument wird stillschweigend 32 eingesetzt, statt `LowerError::InvalidIdArgument`. Für `@bit_bound("xyz")` kriegt der User keine Fehlermeldung. | `const_to_u32(e).ok_or(LowerError::WrongArgumentCount { annotation: "bit_bound", expected: 1, got: 0 })?`. |
| 31 | Low | `crates/rtps/src/subscription_data.rs:51-82` | Structural | SubscriptionBuiltinTopicData delegiert encode/decode vollständig an Publication. Semantisch unterschiedliche Typen teilen Wire-Code — wenn Subscription Phase-2 um Reader-spezifische PIDs erweitert wird (`TIME_BASED_FILTER`), sind Merge-Konflikte garantiert. | Eigene `to_pl_cdr_le`/`from_pl_cdr_le` mit gemeinsamem `ParameterList`-Helper; Delegation vermeidbar. |
| 32 | Low | `crates/types/src/type_object/common.rs:54-57` | Unnötig `copy_from_slice` | Drei Stellen (NameHash::decode_from, EquivalenceHash, SCC-Hash) bauen Array-from-Slice via `copy_from_slice`, was bei `read_bytes` schon geprüft wurde. `bytes.try_into()` wäre idiomatischer. | `let arr: [u8; 4] = r.read_bytes(4)?.try_into().map_err(|_| DecodeError::UnexpectedEof {..})?`. |

## Nicht gefunden (explizit geprüft, OK)

- **Wire-Discriminator-Werte** (TK_NONE=0x00, TK_BOOLEAN=0x01, …, EK_MINIMAL=0xF1,
  EK_COMPLETE=0xF2) stimmen mit Cyclone-Fixtures und XTypes 1.3 §7.3.4.2 überein.
- **`#[non_exhaustive]`** auf `TypeIdentifier`, `TypeObject`, `MinimalTypeObject`,
  `CompleteTypeObject` korrekt gesetzt (Forward-Compat für neue Variants).
- **`TypeCodecError`** implementiert `From<EncodeError>` + `From<DecodeError>`
  sauber, `fmt::Display` ist informativ.
- **Hash-Determinismus:** `compute_hash` ist stabil (zweimalige Berechnung liefert
  denselben Hash — bis auf den MD5-vs-SHA-256-Bug #1 ist die Roundtrip-Stabilität OK).
- **Negative `dependent_typeid_count` = -1** wird korrekt durch `i32`-Cast durch
  `u32` getragen (spec-relevant für "unknown dependencies").
- **Encapsulation-Header** in PublicationBuiltinTopicData (`ENCAPSULATION_PL_CDR_LE`
  vs. `[0x00, 0x02]` BE) richtig erkannt, Fehlerbehandlung mit
  `UnsupportedEncapsulation` sauber.
- **PID_TYPE_INFORMATION (0x0075)**, **PID_DATA_REPRESENTATION (0x0073)**,
  **PID_TYPE_CONSISTENCY_ENFORCEMENT (0x0074)** Werte stimmen mit
  RTI/Cyclone-Namespaces überein.
- **Alias-Resolution Cycle-Detection** (resolve.rs:108) wird vom Testfall
  `resolve_cycle_detected` gedeckt.
- **Builder-Fluency** (StructMemberBuilder, Closure-Pattern) idiomatisch.
- **Tests pro Kind:** Jede Minimal- und Complete-Kind-Variante hat mindestens
  einen Roundtrip-Test in `type_object/mod.rs:tests`. Lückenhafte Tests bei
  `Annotation`/`Bitset` weisen auf weniger Deckung hin, aber Happy-Path ist
  abgedeckt.
- **`i32 as u32 as i32`-Roundtrip** (enum value, scc indices, union labels,
  dependent_typeid_count) ist Two's-Complement-safe.
- **Multi-Level-Alias-Resolution** (2-tief) hat expliziten Test.
- **Extensibility-Bits** kollidieren nicht (IS_FINAL=0x0001, IS_APPENDABLE=0x0002,
  IS_MUTABLE=0x0004, IS_NESTED=0x0008, IS_AUTOID_HASH=0x0010) — matching XTypes
  §7.3.4.5.1.
- **EntityId für SEDP/SPDP** (SEDP_BUILTIN_PUBLICATIONS_WRITER etc.) folgen
  DDSI-RTPS 2.5 Tabelle 9.1 — keine Regression durch den TL_SVC-Diff.

## Priorisierung

**Muss vor WP 1.6 Live-TypeLookup gefixt werden:**
- #1 (MD5), #2 (TL_SVC EntityKind), #4/#5 (Error-Mapping — sonst werden Live-Fehler unsichtbar).

**Parallel zu WP 1.6:**
- #3 (DoS-Cap über alle Decoder), #8 (Assignability-Vollständigkeit für reale Matching-Scenarios), #7 (Optional-Encoding klären).

**WP 1.7 oder später:**
- #9/#10 (Assignability-Regeln), #13 (Autoid-HASH), #17–#27 (Hygiene, API-Konsistenz, Dead-Code).

