# Standards-Registry

Dieses Verzeichnis ist die **kuratierte Registry aller externen Standards**, auf denen ZeroDDS aufbaut. Es enthaelt bewusst **keine PDFs der Specs selbst** — stattdessen pro Standard ein Metadaten-Eintrag mit Version, Bezugsquelle, Relevanz fuer ZeroDDS und eine Abbildung auf die betroffenen Crates.

## Warum keine PDFs im Repo?

Die meisten einschlaegigen Specs (OMG, OASIS, ISO/IEC, W3C) stehen unter eigenen Copyrights. OMG stellt seine Specs oeffentlich zum Download bereit, die Redistribution in einem Drittprojekt-Repo ist aber rechtlich nicht eindeutig. Um die Apache-2.0-Linie von ZeroDDS nicht durch eingebettete Fremd-IP zu trueben, halten wir es so:

1. **Registry** (dieses Verzeichnis, im Git) — was es gibt, wo man es herbekommt, welche Version bindend ist, welche Sections relevant sind.
2. **Cache** (`cache/`, **git-ignored**) — lokale Kopien der Spec-PDFs, nur fuer lokalen Gebrauch. Jeder Entwickler laedt sich die Dateien via `fetch.sh` einmal herunter und arbeitet offline.
3. **Spec-Annotationen im Code** (`#[spec(rtps = "2.5", section = "8.3.7.3")]` per `docs/architecture/01_scope_and_specs.md §7`) verlinken per Section-Nummer auf die Spec, ohne Text der Spec zu zitieren. Damit bleibt der Code immer Apache-2.0-rein.

## Benutzung

```bash
# Einmalig pro Checkout — laedt alle frei verfuegbaren Specs nach cache/
./docs/standards/fetch.sh

# Spec finden
open docs/standards/cache/omg/ddsi-rtps-2.5.pdf
```

Das Script ist idempotent und ueberspringt bereits vorhandene Dateien. Es laesst paywalled Specs (ISO/IEC) bewusst aus; dort steht in der Ausgabe ein Hinweis mit Bezugsquelle.

## Struktur

| Datei | Zweck |
|---|---|
| [`INDEX.md`](./INDEX.md) | Kompakte Tabelle aller Standards mit Version, Stichdatum, Lizenz, Verpflichtungs-Grad |
| [`omg.md`](./omg.md) | Detail-Eintraege fuer OMG-Specs (DDS, RTPS, XTypes, Security, IDL, RPC, XML, XRCE, Language-Mappings) |
| [`non-omg.md`](./non-omg.md) | W3C, IETF, CNCF (OpenTelemetry, Prometheus, OpenMetrics), OASIS (PKCS#11), CycloneDX, sonstige |
| [`fetch.sh`](./fetch.sh) | Download-Script, idempotent, ueberspringt bereits vorhandene Dateien |
| `cache/` | Lokaler PDF-Cache, git-ignored |

## Verpflichtungs-Grade

Jeder Standard-Eintrag traegt einen dieser Verpflichtungs-Grade:

- **normative** — wir erfuellen die Spec vollstaendig. Abweichungen sind Bugs.
- **conformance** — wir zielen auf Spec-Conformance fuer ein definiertes Profil oder Subset (siehe `docs/architecture/01_scope_and_specs.md §3`).
- **integration** — wir konsumieren oder produzieren das Format an Schnittstellen, ohne die Spec selbst zu implementieren (z.B. Prometheus-Exporter).
- **reference** — informativ, orientiert unser Design, aber nicht bindend (z.B. Cyclone-DDS- und Fast-DDS-Verhalten an Spec-Graustellen).
- **future** — fuer spaetere Phasen oder bedingte Expansion-Era-Tracks vorgesehen, siehe `docs/architecture/06_roadmap.md §8`.

## Cross-References

- `docs/architecture/01_scope_and_specs.md` — kanonische Scope-Definition welche Standards in welcher Phase kommen
- `docs/architecture/04_safety_by_architecture.md §4` — wie Spec-Annotationen in Code aussehen (`#[spec(...)]`)
- `tools/traceability/` — erzeugt aus diesen Annotationen eine Coverage-Matrix Spec-Section → Code-Modul

## Updates

Kommt eine neue Revision einer Spec heraus, geht das so:

1. Release-Notes der Spec lesen, Delta gegen unseren gepinnten Stand verstehen.
2. Registry-Eintrag in `omg.md` / `non-omg.md` aktualisieren, **alte Version als "superseded" markieren**, nicht loeschen (Historie bleibt).
3. ADR unter `docs/adr/` anlegen: warum Upgrade, welche Auswirkungen auf Crates, Interop-Risiken.
4. `fetch.sh` auf die neue Version umstellen.
5. Spec-Annotationen im Code ggf. anpassen, wo Sections-Nummern sich verschoben haben.
6. Wenn es ein Safe-Crate betrifft: Safety-Engineering-Lead-Review gemaess `docs/architecture/04_safety_by_architecture.md §8`.
