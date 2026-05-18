# Track RC2-D — Tutorial-Audit

**Goal:** alle 15 Tutorial-Chapters in `examples/tutorials/dds-chat/` und
das `documentation/`-Trail werden hands-on durchgespielt. Pub/Sub-Splits
in 9 Sprachen verifiziert, README-Schritte exakt reproducierbar.

**Status:** 📋 todo

**Estimate:** 2 Personenwochen.

## Audit-Scope

### Documentation Trail (`documentation/`)

| Station | Audit-Pflicht |
|---|---|
| 01-getting-started | "5 Minuten zum hello-world" — exakte Schritte zeitnehmen, hat ein Newcomer das in 5 min? |
| 02-architecture | Diagramme aktuell, Layer-Beschreibungen passen zum Stand |
| 03-configuration | jede QoS-Policy wenigstens einmal als Snippet sichtbar |
| 04-idl | Codegen-Befehle mit echtem `idlc` ausführen, Outputs prüfen |
| 05-integration | für jede Sprache: Build-Befehl, Hello-World, Output |
| 06-operations | Deployment-Schritte gegen den live LXC-220-Setup verifizieren |
| 07-migration | je eine Quelle (Cyclone, Fast, OpenDDS, RTI) durchspielen |

### dds-chat Tutorial (`examples/tutorials/dds-chat/`)

| Chapter | Inhalt | Audit |
|---|---|---|
| 01 | Hello DDS — minimal pub/sub | run on linux + macOS |
| 02 | Topics & types — IDL + codegen | regen mit `idlc`, verify |
| 03 | Reliability | 0%/10%/30% loss reproducible? |
| 04 | Durability | TransientLocal-Replay funktioniert |
| 05 | History & resource limits | KeepLast(N) verhalten |
| 06 | Deadlines & liveliness | Trigger-Status correct |
| 07 | Content-filtered topics | SQL-Filter live |
| 08 | Ownership | Exclusive-with-strength |
| 09 | Type evolution | append+mutate, alt-readers OK |
| 10 | Recording & replay | recorder-bridge + replay tool live |
| 11 | Bridges | je eine Bridge durchspielen |
| 12 | Security | TLS-cert-bestücken, Auth+ACL |
| 13 | Performance tuning | shm + AES-NI demonstrieren |
| 14 | Cross-language interop | Rust↔C++↔Java↔Python↔TS roundtrip |
| 15 | Production deployment | systemd + monitoring + packaging |

### Sprach-Ports (9× pro Chapter, wo Pub/Sub-Splits existieren)

- Rust, C++, C++/Qt6, C#, Java, Python, TS-Node, TS-Browser, Flutter

Für jede Sprache pro Chapter:
- Build-Schritt funktioniert
- Hello-World gegen den Rust-Reference-Publisher empfängt
- IDL-Codegen erzeugt validen Code
- README beschreibt was an dieser Sprache anders ist

## Audit-Doku

Pro Chapter: `examples/tutorials/dds-chat/<NN>-<chapter>/AUDIT.md` mit
Sign-off-Block:

```markdown
# Chapter NN Audit

Reviewer: <name>
Date: 2026-MM-DD
Platforms: linux-x86_64, macos-aarch64
Languages: rust ✓, cpp ✓, java ✓, python ✓, ts-node ✓, ...

## Findings

- [Issue 1] description + fix-commit
- [Issue 2] ...

## Sign-off

Chapter is reproducible end-to-end on the listed platforms with the
languages marked ✓ as of commit <sha>.
```

## Spezifische Risiken zu erwarten

- **Cross-language IDL-Roundtrip** in Chapter 14: 9 Sprachen, jede mit
  eigenem Codegen-Backend, Schwachstelle ist meist Encoder/Decoder-
  Inkompatibilität (XCDR2-edge-cases). Erwartung: 2-3 Bugs zu finden.
- **Flutter-Chapter-Step**: Flutter-Tooling-Drift (dart_ffi-API ändert
  sich häufig). Erwartung: 1-2 Build-Fixes.
- **Recorder-Replay-Chapter**: braucht laufende recorder-bridge +
  replay-tool. Bei jedem Schema-Bump im `.zddsrec`-Format gibt's potentiell
  Breakage.

## Acceptance

- 7 Trail-Stations audited, jede mit AUDIT.md
- 15 dds-chat Chapters audited
- Cross-language roundtrip in Chapter 14 grün für mind. 6 von 9 Sprachen
- Alle README-Schritte sind copy-paste-able auf Linux + macOS
- Bugs als Findings im RC1_FINDINGS.md (oder neuem RC2_FINDINGS.md)
  tracked

## Dependencies

- Demo-Audit (RC2-C) abgeschlossen, weil mehrere dds-chat-Chapters auf
  warehouse-Demo-Komponenten zugreifen
- Linux + macOS Test-Hosts für Multi-Platform-Verifikation
