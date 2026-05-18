# Open Items — Persistente Backlog-Liste

Diese Datei trackt **offene Engineering-Items**, die in einem
abgeschlossenen Sprint identifiziert aber bewusst zurückgestellt
wurden. Pro Item: Was, Warum-deferred, Implikationen, Wann-pickup,
und ein Pfad zur Pick-up-Spec.

Live-Issue-Tracker (gitlab.sandra-kessler.eu) ist die Authoritative-
Quelle für aktive Arbeit. Diese Datei ist die **Engineering-
Diary-Version** — was wissen wir, was haben wir bewusst *nicht*
gemacht, und warum.

## Konvention

* Pro Item ein eigenes `*-followup.md` im thematischen `docs/`-Verzeichnis (`docs/perf/`, `docs/interop/`, `docs/architecture/` …)
* Filename-Pattern: `<sprint-id>-<topic>-followup.md` ODER `<topic>-followup.md`
* Inhalt-Template:
  - **Status** (deferred / partial / blocking)
  - **Datum** + **Sprint-Kontext**
  - **Was ist offen** (technisch konkret)
  - **Warum offen** (Trade-off der bewusst ging)
  - **Implikationen** wenn nicht implementiert (funktional / perf / spec / UX)
  - **Wann pick-up sinnvoll** (Trigger-Events)
  - **Implementations-Pfad** (geschätzte Dauer + Phasen)

## Currently Open

### Performance

| Item | File | Geschätzt | Trigger |
|---|---|---|---|
| **D.5e Phase 3 — Deadline-Heap-Worker** | [`docs/perf/d5e-phase3-deadline-heap-followup.md`](perf/d5e-phase3-deadline-heap-followup.md) | 2-3 Wochen | Sub-100µs-p99-Target, >1000 matched-Participants, dcps-async-Pfad |

### Interop

| Item | File | Geschätzt | Trigger |
|---|---|---|---|
| **QoS-Profile XML-Handling** | [`docs/interop/qos-profile-xml-followup.md`](interop/qos-profile-xml-followup.md) | 1-1.5 Sprints | RTI/Cyclone-Migration-Use-Case, DDS-XML 1.0 Conformance-Audit, Hot-Reload-Feature |
| **ShapeExtended-Type Support** | [`docs/interop/shape-extended-followup.md`](interop/shape-extended-followup.md) | 1-2 Tage (Quick-Win) | "30-Sekunden-RTI-Demo ohne CLI-Flags", eProsima ShapesDemo-Default-Update |

### PSM / Bindings

| Item | File | Geschätzt | Trigger |
|---|---|---|---|
| **F-PSM-CXX-readcond-segv** | [`docs/cpp/psm-cxx-readcond-segv-followup.md`](cpp/psm-cxx-readcond-segv-followup.md) | 0.5-1 Tag | naechster K14-Release-Tag, Linux-Customer-Use-Case |

### DCPS / Discovery

| Item | File | Geschätzt | Trigger |
|---|---|---|---|
| **F-DCPS-latency-self-match-timeout** | [`docs/dcps/latency-self-match-timeout-followup.md`](dcps/latency-self-match-timeout-followup.md) | 0.5-1.5 Tage | naechster K3a-Release-Tag, intra-Process-PSM-Bug-Report |

### Packaging / Distribution

| Item | File | Geschätzt | Trigger |
|---|---|---|---|
| **Language-Binding-Registry-Publish (PyPI / npm / Maven / NuGet)** | [`docs/packaging/language-binding-publish-followup.md`](packaging/language-binding-publish-followup.md) | 2-3 Sprints | RC3-Vorbereitung, Anwendungsentwickler-Onboarding (heute "git clone + cargo build" statt `pip install`) |

## Wie ein neues Open-Item hinzugefügt wird

Wenn aus einer Sprint-Retro ein nicht-blocking-deferred Item rausfällt:

1. Pro Item ein neues `<topic>-followup.md` File im thematischen Verzeichnis (`docs/perf/`, `docs/interop/`, `docs/architecture/`)
2. Inhalt nach Template (siehe `docs/perf/d5e-phase3-deadline-heap-followup.md` als Vorlage)
3. Eintrag in dieser Datei (`docs/OPEN-ITEMS.md`) Tabelle hinzufügen
4. Commit-Message: `docs(open-items): add <topic>-followup.md`

## Kompletted-Removed

Wenn ein Item abgeschlossen wurde:
1. `*-followup.md` File **nicht löschen** — wird zu Geschichte / Sprint-Diary
2. Im File-Header `Status: completed` setzen + `Closed-Datum: YYYY-MM-DD` + `Closed-by-commit: <hash>`
3. Aus Tabelle in dieser Datei entfernen (oder in einen "Recently Closed" Abschnitt verschieben falls historisch interessant)

## Nicht hier dokumentiert

Diese Datei trackt **keine**:
* Aktiven In-Progress-Sprints — die leben in `.planning/`
* Bug-Reports — gitlab.sandra-kessler.eu Issues
* Open-Source-Tickets externer Vendoren
* Tutorial- und Onboarding-TODO — `examples/tutorials/dds-chat/ROADMAP.md`
* High-level Strategic Roadmap — `docs/architecture/06_roadmap.md`
