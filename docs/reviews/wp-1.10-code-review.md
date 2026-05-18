# WP 1.10 — Compliance-Test-Suite Code Review

**Overall Assessment**: Good (Phase-1-Start-adäquat)
**Scope**: Golden-Vector-Infrastruktur + 4 Beispielvektoren
**Seit**: `4a7521d`

## Findings

### Critical

**C1 — Self-referenzielle Vectors (kein Spec-Drift-Detektor)**
`heartbeat_minimal_le.hex`, `int32_le.hex`, `md5_empty_string.hex`, `durability_transient_local_le.hex` sind allesamt Werte, die unser eigener Encoder erzeugt — plausibel per Hand aus der Spec gerechnet, aber ohne externe Quelle. Die Tests erkennen Encoder-Regression, nicht Spec-Drift. Der `README.md`-Claim "Wireshark trace samples / Fast-DDS-Logs / Cyclone `ddsi_type*`-Fixtures" ist für T2-T5 aktuell unbelegt. Zumindest 1 Fixture pro Domain sollte aus externer Quelle stammen (Annex D Hexdump abgetippt, Cyclone-Capture). Sonst ist der Test "encode==encode".

### Important

**I1 — 4-fach duplizierter Loader (`load_hex`/`compliance_root`)**
`crates/{cdr,types,qos}/tests/compliance_*.rs` kopieren jeweils die zwei Funktionen aus `fixture_loader.rs`. Nur `rtps` verwendet `#[path = "fixture_loader.rs"] mod`. Die Duplikate sind laxer (kein Odd-Length-Check, kein 16-MiB-Cap, alles `unwrap`). Lösung: `crates/test-support`-Helper-Crate (dev-dependency) oder einheitlich `#[path]`-Include auf einen gemeinsamen Pfad (z.B. `tests/common/fixture_loader.rs`).

**I2 — README beschreibt nicht-existente Infrastruktur**
`tests/compliance/README.md` verspricht `.expect.json`-Manifeste und `tests/compliance/mod.rs`. Beides fehlt. Entweder umsetzen oder README zurückziehen — Doku, die lügt, ist schlimmer als keine.

**I3 — Fixture-Inventar-Diskrepanz**
`tests/compliance/rtps/README.md` listet 6 Vectors (data_noinline_qos, acknack_basic, gap_single, data_frag_first, nack_frag_basic). Vorhanden: 1. Liste als "geplant" markieren oder TODO-Tabelle.

### Suggestions

**S1 — Fixture-Format skaliert, aber Header fehlt**
`.hex` mit `#`-Kommentaren ist ok bis ~1 kB. Für DATA_FRAG/SPDP-Payloads (mehrere kB) empfiehlt sich ein Header-Block mit Metadaten (`# source: cyclone-0.10.5`, `# captured: 2026-04-18`, `# endianness: LE`, `# submessage: HEARTBEAT`). Hilft bei Debugging und erlaubt später maschinell durchsuchbar.

**S2 — Naming zukunftsfest machen**
`heartbeat_minimal_le.hex` wird eng, sobald du 5 HEARTBEAT-Varianten hast. Migration auf `rtps/heartbeat/01_minimal_le.hex` wird erzwungen und bricht Pfade. Empfehlung: jetzt umbenennen zu `rtps/heartbeat/minimal_le.hex` (Unterordner pro Submessage). Gleiches für `xcdr2/primitives/int32_le.hex`, `qos_pid/durability/transient_local_le.hex`.

**S3 — Odd-Length-Check nur in `fixture_loader.rs`**
Die 3 Duplikate lassen ein ungerades Hex-Token still durchfallen (letztes Nibble ignoriert via `chunks(2)`). Zusammen mit I1 lösbar.

**S4 — `env!("CARGO_MANIFEST_DIR")/../..` ist brüchig**
Verlässt sich auf 2-Level-Tiefe `crates/<x>`. Bei Umzug (`crates/net/rtps/…`) bricht es still. `CARGO_WORKSPACE_DIR` gibt's nicht stable — Alternative: Walk bis `Cargo.toml` mit `[workspace]` gefunden, oder `$CARGO_WORKSPACE_DIR` per `.cargo/config.toml` setzen.

**S5 — Tests prüfen nur Happy-Path**
Kein Negativ-Vector (korrumpierte Bytes → Error). Gerade Compliance-Suite sollte "spec says reject" belegen. Ein `heartbeat_bad_flags_le.hex` mit zugehörigem `#[should_panic]`/`is_err()`-Test belegt Robustheit.

### Positive

- `fixture_loader.rs` selbst ist sauber: `#`-Kommentar-Strip, `0x`-Prefix optional, 16-MiB-Cap, odd-length-Detection, Smoke-Test + `compliance_root_exists`-Guard.
- RTPS-Test prüft Roundtrip _und_ E-Flag — das ist der richtige Pattern.
- Klare Trennung WP 1.10 (offline Golden) vs. WP 1.11 (Live-Interop) im README.
- Kommentare in `heartbeat_minimal_le.hex` sind vorbildlich (Annotation pro Feld).

## Empfehlung

Vor WP 1.11-Start: I1 + I2 fixen (1-2 h), C1 mit mindestens einem extern-stammenden Vector adressieren (Cyclone-Capture aus WP 0.6 reaktivieren?). S2 jetzt billig, später teuer.
