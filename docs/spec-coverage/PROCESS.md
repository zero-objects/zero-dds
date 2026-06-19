# Spec-Coverage-Vorgehen

Wie wir Spec-Coverage-Dokumente führen. Verbindlich für alle
`docs/spec-coverage/<spec>.md`-Files. Basis für jede neue Spec-Coverage-
Pflege.

## Grundsatz

Pro Spec gibt es **zwei** Dateien:

1. `<spec-name>.md` — die einzige Wahrheits-Quelle:
   - vollständig (alle normativen Items der PDF-Spec)
   - faktisch (kein Storytelling, keine Phasen-Erklärungen)
   - prüfbar (jeder Eintrag verlinkt auf konkretes File + Tests)
   - alle Items inkl. derer mit Status `open` und `partial`
2. `<spec-name>.open.md` — Aggregat-Liste der `open`-Items:
   - automatisch aus dem Hauptfile abgeleitet, kein neuer Inhalt
   - jedes `open`-Item mit Spec-Ref + Kurzbeschreibung + Plan-Hinweis
   - dient als Arbeits-Backlog
   - Vor jedem Audit-Lauf gelöscht und neu generiert (kein Drift)
   - **Ausnahme:** ist nach dem Audit nichts mehr offen/partial/rejected,
     entfällt die `.open.md` ganz — siehe „Leere `.open.md` entfernen" am Ende.

## File-Struktur

```
# <Spec-Titel> — Spec-Coverage

**Quelle:** `docs/standards/cache/<vendor>/<spec>.pdf` (<Seiten>, <Datum>)
        — oder bei ZeroDDS-Vendor-Specs: `documentation/specs/<spec>/main.tex`

## <§-Nummer> <Section-Titel>

### <Item-Titel>

**Spec:** §<§-Ref>, S. <Seite> (PDF) — wörtliches Zitat oder
exakte Paraphrase der normativen Anforderung.

**Repo:** <File-Pfad> (`<Symbol-Name>` / Funktion / Production-ID)

**Tests:** `<test-file>::<test-name>` (mehrere durch Komma getrennt)

**Status:** `done` | `partial` | `open` | `n/a (informative)` | `n/a (rejected)`
```

### Status-Werte

| Wert | Bedeutung |
|---|---|
| `done` | 100% spec-konform implementiert; Repo + Tests vorhanden und decken die Anforderung ab. |
| `partial` | Teilweise umgesetzt. Beschreibe **exakt** was fehlt. Verbleibendes muss als eigenes Item mit Status `open` geführt werden, sonst nicht zulässig. |
| `open` | Nicht umgesetzt. Repo-Spalte = `—`. Tests-Spalte = `—`. |
| `n/a` | Nicht implementier-pflichtig. **Zwei Klassen, siehe unten.** |

#### `n/a`-Klassifikation

Jeder `n/a`-Status **MUSS** mit Klasse-Suffix versehen werden:

| Klasse | Bedeutung | Pflicht in `.open.md`? |
|---|---|---|
| `n/a (informative)` | Spec-Section ist explizit informativ (Vorwort, Acknowledgments, Glossar, Editorial-History, Cross-Spec-References). Implementierung naturgemäß nicht nötig. | nein |
| `n/a (rejected)` | Spec-Section ist normativ und implementierbar, aber wir haben uns aus einer bewussten Design-Entscheidung **gegen** Implementierung entschieden. | **ja, mit Decision-Record (siehe unten).** |

Die Item-Notiz **MUSS** die Klasse begründen. Ein nacktes `n/a`
ohne Klasse ist nicht zulässig.

## Workflow pro Audit

1. Vorhandene `.open.md` löschen (`rm docs/spec-coverage/<spec>.open.md`).
2. Spec-Quelle öffnen — bei OMG/IETF/OASIS-Specs die PDF (Read-Tool mit
   `pages`-Parameter, max 20 Seiten/Aufruf) in `docs/standards/cache/`;
   bei ZeroDDS-Vendor-Specs die TeX-Quelle in `documentation/specs/`.
3. Sequentiell §1 → §N durchgehen. Pro normativer Anforderung:
   - Item nach obigem Schema schreiben
   - Spec-Zitat oder exakte Paraphrase, mit Seitenzahl
   - Repo-Pfad: per `grep` / Symbol-Suche im Code finden
   - Test-Pfad: per `grep` im Test-Tree finden
   - Status setzen
4. Workspace-Test laufen lassen, um sicher zu sein dass die referenzierten
   Tests existieren und grün sind.
5. Commit: `docs(spec-coverage): <spec> Spec-Check N.0 verify`.

## Quellen

| Spalte | Quelle (OMG / IETF / OASIS) | Quelle (ZeroDDS Vendor-Spec) | Verifikations-Befehl |
|---|---|---|---|
| Spec-Zitat | PDF-Spec direkt (Read-Tool, `pages`-Param) in `docs/standards/cache/<vendor>/<spec>.pdf` | TeX-Quelle in `documentation/specs/<spec>/main.tex` | — |
| Repo-Pfad | `grep -nE "<symbol>" crates/` | dito | `cargo build` muss compile-en |
| Tests | `grep -nE "fn <test>" crates/**/tests/` | dito | `cargo test --workspace` muss grün sein |
| Status | Logischer Schluss aus den drei oben | dito | bei `done` muss Test grün sein |

## Anti-Patterns (verboten)

1. **Status-Drift**: Item als `done` markieren ohne Tests im Repo.
2. **Phasen-Marker**: "Phase-2", "deferred", "out-of-scope", "später"
   sind keine Status-Werte. Wenn unklar → `open` mit konkreter
   Beschreibung.
3. **Sammel-Status**: "alle Rules 133-183 done" ohne Item-für-Item-
   Auflistung. Jede Regel = ein Item.
4. **Vendor-Annotations als Hülle**: Spec-Section nicht überspringen
   nur weil sie informativ wirkt — auch Tabellen-Inhalte (Keywords,
   reservierte Wörter) müssen aufgeführt werden.
5. **Code-Pfad ohne Verifizierung**: jede Repo-Spalte muss tatsächlich
   existieren. Vor Commit `cargo check` laufen lassen.
6. **Test-Pfad ohne Verifizierung**: Test-Funktion muss tatsächlich
   existieren und grün sein. `cargo test --workspace` vor Commit.
7. **`n/a` ohne Klasse**: jeder `n/a`-Status **MUSS** als
   `n/a (informative)` oder `n/a (rejected)` markiert sein.
8. **`n/a (rejected)` ohne Decision-Record**: jedes als
   `n/a (rejected)` markierte Item **MUSS** in der `.open.md`
   einen Decision-Record-Eintrag mit allen drei Pflicht-Feldern
   (Begründung, Impl-Auswirkung, Impl-Vorteil) tragen — siehe
   nächste Sektion.

## `.open.md`-Format

Die `.open.md` ist ein Aggregat. Ihr Inhalt **MUSS** automatisch
aus dem Hauptfile abgeleitet werden, vor jedem Audit-Lauf
gelöscht und neu generiert. Sie enthält zwei Abschnitte:

### Abschnitt 1: `open`- und `partial`-Items

Jedes Item mit Status `open` oder `partial` aus dem Hauptfile
**MUSS** hier auftauchen. Format pro Eintrag:

```markdown
## §<§-Ref> <Section-Titel>

**Status:** `open` | `partial` — <Kurzbeschreibung was fehlt + Plan-Hinweis>
```

### Abschnitt 2: Decision-Records (`n/a (rejected)`)

Jedes Item, das im Hauptfile als `n/a (rejected)` geführt wird,
**MUSS** hier mit einem Decision-Record erscheinen. Format:

```markdown
## §<§-Ref> <Section-Titel> — `n/a (rejected)`

**Begründung:** <warum wir uns gegen die Implementierung
entschieden haben — sachlich, mit Verweis auf Architektur-
Entscheidung, Plattform-Constraint, oder Spec-Konflikt>.

**Impl-Auswirkung:** <was eine `done`-Implementierung am Code
verändern würde — konkret welche Crates, welcher Aufwand, welche
neuen Abhängigkeiten>.

**Impl-Vorteil:** <welcher konkrete Vorteil entstünde — Use-Case,
Cross-Vendor-Interop, Coverage-Gewinn — falls man die Decision
revidiert>.
```

`n/a (informative)`-Items tauchen in der `.open.md` **nicht** auf;
sie erscheinen ausschließlich in der Schluss-Aggregation des
Hauptfiles.

## Abschluss-Bemerkung im Hauptfile

Jedes `<spec>.md`-Hauptfile **MUSS** mit einer Abschluss-
Bemerkung enden. Form ist verbindlich nüchtern (keine Storytelling-
Absätze, keine Datums-Erzählung, keine Phasen-Verweise):

```markdown
---

## Audit-Status

<N> done / <M> partial / <O> open / <P> n/a (informative) / <Q> n/a (rejected).

Test-Lauf: `<test-command>` — <X> Tests grün, 0 failed.
```

Optional zusätzlich (eine Zeile, sachlich, nur wenn nicht trivial):
ein Verweis auf `<spec>.open.md` für die offenen Punkte und
Decision-Records.

**Verboten** in der Abschluss-Bemerkung:

- Datumsangaben ("abgeschlossen 2026-MM-TT")
- Phasen-/Cluster-Narrative ("K3 abgeschlossen, K4 kann beginnen")
- Erzählungen über die Audit-Geschichte
- Begründungs-Absätze für einzelne Items (gehören in Item-Notiz oder Decision-Record)
- Marketing-Aussagen ("byte-identisch zu Cyclone DDS", "100% Coverage")

Die Aggregations-Zahlen `<N>/<M>/<O>/<P>/<Q>` **MÜSSEN** mit der
tatsächlichen Item-Zählung im Hauptfile und mit der `.open.md`
übereinstimmen. Drift ist Audit-Fail.

## Beispiel-Eintrag

```markdown
### §7.4.1.3 Rule (61) — Native-Type Declaration

**Spec:** §7.4.1.3 (61), S. 24 (PDF) —
"<native_dcl> ::= 'native' <simple_declarator>"

**Repo:** `crates/idl/src/grammar/idl42.rs` (`PROD_NATIVE_DCL`,
ID 121); Top-Level-Aktivierung in `PROD_TYPE_DCL` Alt 3.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests`:
`parses_native_dcl_top_level`, `parses_native_dcl_in_module`,
`parses_native_dcl_in_interface`. Feature-Gate-Test:
`crates/idl/src/features/gate.rs::tests::dds_basic_rejects_native`.

**Status:** done
```

## Wieder-Verwendung für andere Spec-Files

Dieses Schema ist **identisch** anwendbar auf alle Spec-Coverage-
Files. Aktueller Bestand:

### OMG-DDS-Kern (PSM)

- `zerodds-dcps-1.4.md`
- `ddsi-rtps-2.5.md`
- `dds-xtypes-1.3.md`
- `zerodds-rpc-1.0.md`
- `zerodds-xrce-1.0.md`
- `zerodds-security-1.2.md`
- `zerodds-xml-1.0.md`
- `dds-psm-cxx-1.0.md`
- `zerodds-java-psm-1.0.md`

### OMG-IDL

- `idl-4.2.md`
- `idl4-cpp-1.0.md`
- `idl4-csharp-1.0.md`
- `idl4-java-1.0.md`

### OMG-Sekundäre Specs

- `dds-tsn-1.0.md`
- `zerodds-web-1.0.md`
- `dds-opcua-1.0.md`
- `dds4ccm-1.1.md`
- `dlrl-1.2.md` (DLRL aus DDS 1.2 §8 + Annex B; nicht Teil von
  DDS 1.4 oder neuer)
- `omg-time-1.1.md`
- `omg-ami4ccm-1.1.md`
- `omg-ccm-4.0.md`
- `omg-rtc-1.0.md`
- `corba-3.3.md` (WP CORBA-Coexistence; Migrations-Pfad für
  Bestands-Anwendungen, drop-in gegen GIOP/IIOP/POA/CSIv2)
- `cos-event-service-1.4.md` (WP COS-EventService; Voraussetzung
  für TimerEventService aus `omg-time-1.1.md` §2.2-§2.4)

### Non-OMG (IETF / OASIS / Industrie)

- `coap-rfc-7252.md`
- `websocket-rfc-6455.md`
- `mqtt-5.0.md`
- `mqtt-3.1.1.md` (MQTT 3.1.1 — version-aware Codec-Pfad neben 5.0;
  Repo: `crates/mqtt-bridge/` `ProtocolVersion::V311`)
- `amqp-1.0.md`
- `amqp-0-9-1.md` (AMQP 0.9.1 — eigenständiges Protokoll: connection/
  channel/exchange/queue/basic/confirm/tx; Repo: `crates/amqp-0-9-1/`)
- `grpc-protocol.md`
- `ros2-rmw.md`

### OPC Foundation (OPC-UA)

Quelle: die OPC-UA-Core-Parts in `docs/standards/cache/opcfoundation/`
(per OPC-Account beschafft; aus IP-/Copyright-Gründen gitignored — nur zum
Zitieren, nie committen). PDF-Offset Part 14 = +20.

- `opcua-pubsub-1.05.md` (Part 14 — PubSub/UADP; Cross-Spec Part 6 §5 Binär-
  Codec / Part 4 / Part 3; Repo: `crates/opcua-pubsub/`)
- `opcua-client-server-1.05.md` (Part 4 — Services + Part 6 §7.1 UACP /
  §6.7 OPC UA Secure Conversation; Repo: `crates/opcua-uacp/` +
  `crates/opcua-server/`). Nicht zu verwechseln mit `dds-opcua-1.0.md`, das die
  OMG-DDS-OPC-UA-**Gateway**-Spec abdeckt.

### ZeroDDS-Vendor-Specs

Quelle: entweder `documentation/specs/<spec>/main.tex` oder
`docs/specs/<spec>.md` (Markdown-Vendor-Specs ohne TeX). Audit-
File-Konvention identisch zu OMG-Specs; Repo-Pfade zeigen auf
`crates/<spec-impl>/`.

- `dds-amqp-1.0.md`
- `dds-ts-1.0.md` (ersetzt das frühere `idl4-ts-1.0.md`,
  Quelle: `documentation/specs/dds-ts-1.0/main.tex`)
- `zerodds-py-1.0.md` (Python-Binding, Quelle:
  `docs/specs/zerodds-py-1.0.md`; Repo: `crates/py/`)
- `zerodds-monitor-1.1.md` (Observability-Substrat, Quelle:
  `docs/specs/zerodds-monitor-1.1.md`; Repo: `crates/monitor/` + Hook-Points in
  `crates/{transport,rtps,discovery,dcps,security}/`; 10 done / 0 open, löst die
  1.0-Coverage ab)
- `zerodds-listener-callbacks-1.1.md` (C-FFI cross-language Listener-Layer,
  Quelle: `docs/specs/zerodds-listener-callbacks-1.1.md`; Repo:
  `crates/zerodds-c-api/` Listener-FFI + Status-/Inconsistent-Topic-Counter in
  `crates/{dcps,discovery}/`; 26 done / 0 open / 1 n/a (informative))
- `zerodds-observability-otlp-1.0.md` (OTLP/HTTP/JSON-Exporter, Quelle:
  `docs/specs/zerodds-observability-otlp-1.0.md`; Repo: `crates/observability-otlp/`)
- `zddsrec-1.0.md` (Recording-Format `.zddsrec`, Quelle:
  `docs/specs/zddsrec-1.0.md`; Repo: `crates/recorder/`)

### Test-Inventare (kein Spec-Coverage)

`cross-vendor-validation.md` ist ein Test-Inventar, kein
PROCESS-konformes Audit-File. Es folgt dem gleichen Item-Format
zur einfachen Lesbarkeit, ist aber explizit nicht auf eine
einzelne Spec-PDF normalisiert.

---

Pro Spec gilt: zugehörige `.open.md` vor Audit löschen, kompletter
Re-Sync gegen die Quelle (PDF oder TeX), Item-für-Item. Nach
Re-Sync: Abschluss-Bemerkung im Hauptfile aktualisieren,
Decision-Records in `.open.md` neu generieren.

## Leere `.open.md` entfernen (kein Leer-Aggregat)

Wenn nach dem vollständigen, sauberen Item-für-Item-Durchgang **kein
einziges** Item mehr den Status `open`, `partial` oder `n/a (rejected)`
trägt — und auch keine verkappten Aufschiebe-Marker wie „deferred",
„later", „Phase X", „out-of-scope", „TODO" mehr im Hauptfile stehen
(diese sind ohnehin keine zulässigen Status, siehe Anti-Patterns) —
dann gibt es nichts zu aggregieren. In diesem Fall:

1. Die `.open.md` (und ggf. `.open.en.md`) **löschen** statt ein
   inhaltsleeres Aggregat zu pflegen.
2. Im Hauptfile den `.open.md`-Querverweis entfernen (Intro **und**
   Abschluss-Bemerkung) und durch „Offene Punkte: keine. Decision-
   Records: keine." ersetzen.
3. Gerenderte `*.open.html` mitlöschen und Coverage-HTML neu rendern,
   damit die Sidebars die entfernte Seite nicht mehr verlinken
   (keine toten Links).

Die Aggregat-Zahl in der Abschluss-Bemerkung muss dann
`<N> done / 0 partial / 0 open / <P> n/a (informative) / 0 n/a (rejected)`
lauten (nur `done` und optional `n/a (informative)`). Sobald ein späterer
Re-Audit wieder ein `open`/`partial`/`rejected`-Item findet, wird die
`.open.md` nach dem normalen Workflow neu erzeugt.
