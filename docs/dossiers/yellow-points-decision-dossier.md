# Decision-Dossier — gelbe Punkte (F2c Listener-Callbacks, F1 ROS-2-Rejects)

- **Status:** Entscheidungs-Dossier (braucht Owner-Entscheid)
- **Datum:** 2026-06-11
- **Leitprinzip:** [[feedback_spec_completeness_over_competition]] — „kein
  Customer-Pull" ist KEIN gültiger Reject-Grund für ein optionales Spec-Profil
  (vgl. OSCORE: von 0007-rejected zu 0010-accepted re-bewertet). Reject ist nur
  gültig, wenn ein Item **architektonisch falsch verortet** oder durch eine
  **spec-konforme Alternative bereits abgedeckt** ist.

---

## Teil A — F2c: Listener-Callbacks (`zerodds-listener-callbacks-1.0`)

**Ist-Stand:** 19/24 done. Aktive Listener wired für **C++ (§7.1)** und
**C# (§7.2)**. Threading-Vertrag §2.2.4 erfüllt. 5 Items `n/a`:

| # | Item | aktueller Status | Natur |
|---|---|---|---|
| 1 | §5 **Bubble-Up** (Listener-Propagation Reader→Subscriber→DomainParticipant) | rejected | echtes DDS-Feature, fehlt |
| 2 | **Sub-Aggregator-Set-Semantik** | out-of-scope | echtes DDS-Feature, fehlt |
| 3 | §7.3 **Java**-Bridge | alternative | Polling statt Callback |
| 4 | §7.4 **Python**-Bridge | alternative | Polling statt Callback |
| 5 | §7.5 **TS/Node**-Bridge | alternative | Polling statt Callback |

### Analyse

**Items 3-5 (Java/Python/TS „alternative"):** Diese sind **spec-konform**, kein
Gap. Die DDS-Spec (§2.2.4) erlaubt explizit **zwei** Wege zum Status-Zugriff:
Listener-Callbacks **oder** Conditions/WaitSet-Polling. ZeroDDS liefert für
Java/Python/TS die Polling-API (`waitFor*`, `wait_for_data`, GuardCondition +
WaitSet + Status-Getter) — in einer Event-Loop-Sprache (Node) bzw. Debug-/
Toolchain-Kontext (Python) ist Caller-driven-Polling **idiomatischer** als ein
FFI-Callback über die Sprachgrenze. → **Spec-Erfüllung ist gegeben.**

Die **offene** Frage ist reine **Ergonomie**: native aktive Callbacks
(PyO3-`Py<PyAny>` für Python, koffi-V8-weak-ref für TS, gRPC-Bridge für Java)
wären für manche Nutzer bequemer, sind aber **funktional redundant** zum
Polling-Pfad.

**Items 1-2 (Bubble-Up + Sub-Aggregator):** echte optionale DDS-Mechanismen.
Bubble-Up = ein nicht gesetzter Reader-Listener delegiert das Event an den
Subscriber- bzw. DomainParticipant-Listener (Spec §2.2.4.1). Das ist **kein
Format-Zucker**, sondern Verhalten — unter dem Spec-Completeness-Prinzip ein
legitimer Kandidat zum Nachziehen.

### Optionen F2c

| Option | Aufwand | Bewertung |
|---|---|---|
| **A. Status-quo akzeptieren** (Polling = spec-konform; Bubble-Up bleibt rejected) | 0 | Spec-konform, aber Bubble-Up bleibt eine echte Lücke |
| **B. Bubble-Up + Sub-Aggregator nachziehen** (Core-Listener-Propagation), Polling-Bridges bleiben | mittel | schließt die echten DDS-Lücken; sprachfremde Callbacks bleiben Polling |
| **C. Voll: B + aktive FFI-Callbacks Python/TS/Java** | groß | maximale Ergonomie, aber FFI-Callback-über-Sprachgrenze ist fehleranfällig (Re-Entrancy, GIL, V8-Lifetime) und redundant |

### Empfehlung F2c

**Option B.** Items 1-2 (Bubble-Up + Sub-Aggregator) sind echte DDS-Verhaltens-
Features und unter Spec-Completeness nachzuziehen — sie betreffen den **Core**
(sprach-unabhängig), nicht nur die Bindings. Items 3-5 als **spec-konform
akzeptieren** (Polling erfüllt §2.2.4); aktive FFI-Callbacks nur bei konkretem
Pull, da redundant + FFI-riskant. → Bubble-Up ist der einzige echte Code-Punkt;
die „alternative"-Rejects sind korrekt.

---

## Teil B — F1: ROS-2-Rejects

Zwei verschiedene Reject-Klassen werden hier zusammengeworfen — sie brauchen
**unterschiedliche** Entscheidungen:

### B.1 — rclcpp-Layer-Features (REP-2007/2008/2009)

ZeroDDS dockt via **rmw_zerodds** an ROS-2 an (RMW-FFI), ist also die
**DDS-/rmw-Schicht**, NICHT rclcpp ([[project_ros2_architecture_decision]]).
Features wie Node-Graph, Parameter, Lifecycle, Executors (REP-2007/8/9) leben
**oberhalb** der rmw-Grenze in rclcpp.

→ **Entscheidung: bleiben rejected — architektonisch korrekt.** Diese Features
in ZeroDDS zu reimplementieren wäre ein Layer-Verstoß (rclcpp-Konkurrenz statt
rmw-Substrat). Das ist KEIN „Customer-Pull"-Reject, sondern ein **Scope-Reject**
— der einzige spec-completeness-konforme Reject-Grund. **Bestätigt.**

### B.2 — SROS2-Enclaves + Permissions-XML (ADR 0008)

ADR 0008 rejected SROS2-Enclaves (§7.1) + ROS-2-Permissions-XML (§7.2) mit der
Begründung „87% laufen ohne SROS2 / kein Customer-Pull". **Das ist genau der
Reject-Grund, den das Spec-Completeness-Prinzip NICHT akzeptiert** — analog zu
OSCORE (ADR 0007→0010).

ABER: anders als OSCORE (eigener Krypto-Mechanismus) ist SROS2-Enclave-Mapping
laut ADR 0008 selbst nur eine **dünne Übersetzungsschicht** (~800 LOC) auf das
**bereits implementierte** `security-permissions` (DDS-Security 1.2 §9.4). Es
ist also ein **Format-Adapter** (sros2-keystore/`policy.xml` → DDS-Security-
Permissions), kein neues Bedrohungsmodell.

### Optionen F1-B.2

| Option | Aufwand | Bewertung |
|---|---|---|
| **A. Bei ADR 0008 bleiben** (rejected) | 0 | widerspricht dem Spec-Completeness-Prinzip (Customer-Pull-Reject) |
| **B. SROS2-Enclave-Mapping implementieren** (Format-Parser + Mapping auf `security-permissions`) | ~800 LOC | spec-vollständig; echter ROS-2→ZeroDDS-Migrationspfad; baut auf vorhandenem Substrat |

### Empfehlung F1

- **B.1 (rclcpp-Layer): rejected bestätigen** — Scope-korrekt, kein Spec-Verstoß.
- **B.2 (SROS2-Enclaves): ADR 0008 RE-BEWERTEN → implementieren** (Option B),
  analog OSCORE. Es ist ein optionales Spec-Profil (REP-2018) mit existierendem
  Substrat; der „kein Customer-Pull"-Reject ist unter Spec-Completeness ungültig.
  Eine Folge-ADR (0011) sollte 0008 superseden, sobald die Implementierung
  startet.

---

## Zusammenfassung der Entscheidungs-Vorlage

| Punkt | Empfehlung | Code-Aufwand |
|---|---|---|
| F2c Items 3-5 (Polling) | **akzeptieren** (spec-konform §2.2.4) | 0 |
| F2c Items 1-2 (Bubble-Up/Aggregator) | **nachziehen** (Core-Verhalten) | mittel |
| F1 B.1 (rclcpp-Layer) | **rejected bestätigen** (Scope-korrekt) | 0 |
| F1 B.2 (SROS2-Enclaves) | **re-bewerten → implementieren** (ADR 0008 superseden) | ~800 LOC |

Offene Owner-Entscheidung: F2c-Bubble-Up und F1-SROS2 freigeben? Beide sind
unter dem Spec-Completeness-Prinzip die konsequente Wahl.
