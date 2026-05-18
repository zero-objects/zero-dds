# RC1 Guardrails — Referenz für jede Crate-Review

> **Zweck:** zentrale Referenz aller Checks, Forbidden-Patterns und DoD-Kriterien für die `1.0.0-rc.1`-Walking-the-DAG-Phase. Jedes Crate-Review verlinkt hierher; Updates an dieser Datei wirken auf alle nachfolgenden Reviews.
>
> **Versionsstand:** dieser File ist sein eigener Versionsstand — git commit ist die Track-Materialisierung. Keine Datum-/Versions-Marker im Body (Zero-Out-of-Band, siehe `docs/architecture/10_zero_principle_mapping.md` Pillar 7).

---

## 1 Definition of Done pro Crate

Eine Crate ist **`1.0.0-rc.1`-ready**, wenn jede Zeile dieser Liste abgehakt ist. Keine Verkürzungen, keine „später"-Markierungen — entweder alle Punkte erledigt oder Crate bleibt auf 🔄 in-review.

### 1.1 Cargo.toml-Metadata

```toml
[package]
name = "<dds-...>"               # MUSS mit dds- prefix bei Public-API-Crates; intern: zerodds-*
version = "1.0.0-rc.1"           # nicht workspace-version, sondern explizit
edition = "2024"
rust-version = "1.88"
license = "Apache-2.0"
description = "<eine Zeile, was die Crate tut>"
repository = "https://github.com/zero-objects/zero-dds"
homepage = "https://zerodds.org"
documentation = "https://docs.rs/<crate-name>"
readme = "README.md"
keywords = ["dds", "<spec-area>", "<technology>"]   # max 5
categories = ["network-programming", "<area>"]      # max 5, valid crates.io categories
authors = ["ZeroDDS Contributors"]
publish = true                   # oder false fuer internal-only Crates
```

- [ ] alle Felder gesetzt, keine Placeholder (`example.invalid`, `0.0.0`, generic)
- [ ] keywords/categories sind valid (`https://crates.io/category_slugs`)
- [ ] `publish = false` nur wenn explizit dokumentiert warum

### 1.2 lib.rs / main.rs Crate-Header

```rust
//! Crate `<crate-name>`. Safety classification: **<KLASSE>**.
//!
//! <Eine Zeile: was die Crate tut.>
//!
//! Spec: <OMG-Spec / RFC / interne ZeroDDS-Spec> §<n.m>.
//!
//! ## Schichten-Position
//!
//! <Foundation / Primitives / Wire / Schema / Core Services / Bridges / PSM / Profile / CORBA>
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`<TopType>`] — <Zweck>
//! - [`<funktion>`] — <Zweck>
//!
//! ## Beispiel
//!
//! ```rust,no_run
//! use <crate>::<TopType>;
//! // ...
//! ```
```

- [ ] Safety-Klassifikation gesetzt (`SAFE` / `STANDARD` / `QUALIFIED`)
- [ ] Spec-Referenz mit Sektion
- [ ] Schichten-Position (Layer 0–8 oder Tools/Examples)
- [ ] Public-API-Aufzählung mit Zweck
- [ ] mindestens ein doc-tested Code-Example (oder Begründung warum nicht)

### 1.3 README.md (pro Crate)

Aus `docs/release/crate-readme-template.md` ableiten. Pflicht-Sections:
- Title + Status-Badge-Row (CI, docs.rs, license, version)
- Was die Crate tut (1 Absatz)
- Spec + Layer
- Quickstart (5 Zeilen, copy-pasteable)
- Feature-Flags-Tabelle
- Stability-Statement (was ist `pub`-stable, was ist `unstable-`)
- Links: Spec, Coverage-Doc, Examples, CHANGELOG

### 1.4 CHANGELOG.md (pro Crate)

`1.0.0-rc.1` ist die **initiale Release-Materialisierung** der Crate — kein Diff zu einem Vorgaengerstand. Der Eintrag ist eine Vollbeschreibung dessen was im Tx enthalten ist (Foundation §5 Track-Closure: Tx enthaelt alles unter Scope).

Pflicht-Sektionen:

```markdown
# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).
Datums-Marker pro Eintrag sind Keep-a-Changelog-Konvention; alles
weitere git-getrackt.

## [1.0.0-rc.1] — YYYY-MM-DD

Initiale Release-Materialisierung.

### Spec-Referenzen
- <OMG-Spec / RFC / interne ZeroDDS-Spec> §<n.m>: <abgedeckter Scope>
- (ggf. weitere Specs)

### Public-API
- [`<TopType>`](src) — <Zweck>
- [`<funktion>`](src) — <Zweck>
- (alle pub-Items aus lib.rs aufzaehlen)

### Implementierung
- <ein Absatz: zentrale Algorithmen / Datenstrukturen / Designentscheidungen>
- <ein Absatz: Performance-/Sicherheits-/no_std-Eigenschaften>

### Architektur
- Layer: <0–8 / Tools / Examples>
- Dependencies (in): <Liste der Dev/Crate-Deps>
- Dependents (out): <wer nutzt diese Crate>
- Feature-Flags: <Tabelle mit Default und Zweck>

### Stabilitaet
- Alle `pub`-Items sind RC1-stabil; Breaking-Changes erfordern Major-Bump.
- <ggf. explizite "unstable-"-Module nennen>
```

- [ ] Version-Header `[1.0.0-rc.1]` mit Datum (Keep-a-Changelog-Konvention erlaubt — siehe §2.1c)
- [ ] **Spec-Referenzen** mit konkretem §-Scope
- [ ] **Public-API** vollstaendig aufgezaehlt aus `lib.rs`
- [ ] **Implementierung** zwei Absaetze (Algorithmen + Eigenschaften)
- [ ] **Architektur** mit Layer + Deps + Feature-Flags-Tabelle
- [ ] **Stabilitaet** Statement

### 1.5 Public-API-Audit

- [ ] `cargo public-api -p <crate>` (oder manuelles Review von `pub`-Sichtbarkeit) — kein versehentliches Re-Export von internen Helpers
- [ ] keine `pub use crate::internal::*;`-Patterns
- [ ] alle `pub`-Items haben Doc-Comments
- [ ] Sealed-Trait-Patterns wo Trait nicht extern impl-bar sein soll

### 1.5b Coherence-Audit (Cross-Crate + Spec)

Identifiziert „definiert-aber-nicht-gewired"-Items. Ein Item ist nur dann RC1-fertig, wenn es entweder genutzt wird oder explizit als Hook/Extension dokumentiert ist.

**Methodik pro Public-Item:**

```bash
# External Production-Refs (ohne tests/ und ohne die Crate selbst)
rg -l '<item>' --type rust crates/ -g '!crates/<self>/**' -g '!**/tests/**'

# Test-only Refs
rg -l '<item>' --type rust crates/ -g '**/tests/**'

# Doc-Refs (Coverage-Docs, Architecture, Tutorials)
rg -l '<item>' docs/ examples/
```

**Klassifikation pro Item:**

| Klasse | Bedeutung | Akzeptanz |
|---|---|---|
| `CONNECTED` | ≥1 externe Production-Ref | ✅ |
| `TEST-ONLY` | nur in `tests/` referenziert | ❌ Wiring-Bug oder Item ist Test-Helper-only (dann `pub(crate)` reduzieren) |
| `SPEC-MANDATED-OPEN` | Spec MUST, aber 0 externe Refs | ❌ MUSS gefixt werden (`wire-up` oder `drop`) |
| `OPTIONAL-HOOK` | Spec MAY oder Plugin-Hook, 0 externe Refs OK | ✅ wenn explizit als Hook dokumentiert |
| `VENDOR-EXTENSION` | keine Spec, ZeroDDS-eigen, ≥1 externe Ref | ✅ wenn dokumentiert |
| `DEAD` | 0 Refs (auch keine Tests) | ❌ `drop` oder `wire-up` |

**Decision pro Item bei Klasse ❌:**

- `wire-up` — verbinden im selben Review (Beleg-Commit-Hash)
- `defer-with-issue` — in `docs/release/RC1_FINDINGS.md` aufnehmen mit konkretem Plan
- `drop` — entfernen
- `document-as-hook` — bleibt mit expliziter Plugin-API-Doku

**Akzeptanz-Kriterium:** Crate ist nur RC1-fertig wenn alle Public-Items klassifiziert sind UND keine `❌`-Klasse mehr offen ist (entweder Decision durchgeführt oder im Findings-Tracker).

- [ ] Coherence-Audit-Tabelle im Review-Doc ausgefüllt
- [ ] alle ❌-Klassen haben eine Decision (durchgeführt oder im Findings-Tracker)
- [ ] Findings-Tracker `docs/release/RC1_FINDINGS.md` aktualisiert (falls deferred Items)

### 1.6 Spec-Coverage-Doc-Update

- [ ] Crate-relevante Sektionen in `docs/spec-coverage/<spec>.md` auf `done`
- [ ] **Repo:** Zeile zeigt auf konkrete Module/Funktionen in der Crate
- [ ] **Tests:** Zeile zeigt auf konkrete Test-Funktionen
- [ ] keine `partial`/`open` für RC1-relevante Sektionen ohne dokumentierte Begründung

### 1.7 Forbidden-Token-Sweep

- [ ] **manueller `rg`-Lauf** über die Crate (siehe §2 unten)
- [ ] alle Treffer entfernt oder dokumentiert (`tests/cyclone_live_*` darf interne Refs haben, gehört nicht in Public-Mirror)

### 1.8 License-Header pro File

```rust
// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
```

- [ ] jede `*.rs`-Datei hat den Header in den ersten 3 Zeilen
- [ ] Test-Files dürfen Header weglassen wenn `tests/`-Subdir reicht

### 1.9 Tests + Lints + Doc-Build

- [ ] `cargo test -p <crate>` grün lokal
- [ ] `cargo clippy -p <crate> --tests -- -D warnings` grün
- [ ] `cargo fmt -p <crate> -- --check` grün
- [ ] `cargo doc -p <crate> --no-deps` baut ohne Warnungen
- [ ] `cargo run --bin zerodds-lint -- check` grün workspace-weit (kein Reibach mit anderen Crates)

### 1.10 Review-Doc unter `docs/release/rc1-reviews/<crate>.md`

- [ ] Template-Instanz aus `docs/release/RC1_CRATE_REVIEW_TEMPLATE.md` ausgefüllt
- [ ] Sign-off-Line am Ende mit Datum

### 1.11 Tracker-Update

- [ ] Eintrag in `docs/release/RC1_TRACKER.md` auf `✅ rc1-ready`

### 1.12 Public-Mirror-Artifacts (`github/` + `website/`)

Sobald eine Public-Crate (🌐) auf `✅ rc1-ready` geht, MUSS parallel der Public-Mirror in den beiden Top-Level-Ordnern materialisiert werden. Forbidden-Token-Sweep (§2.1) gilt für beide Ordner-Inhalte hard.

- [ ] **`github/crates/<crate>/`** — synchronisierte Public-Kopie der Crate (Cargo.toml + src + README + CHANGELOG + LICENSE; KEINE internen Test-Harness-Files, KEINE `tests/cyclone_*.rs`-E2E-Tests die auf interne Lab-Hosts zeigen).
- [ ] **`github/Cargo.toml`** — Workspace-Manifest enthält die neue Crate als Member.
- [ ] **`github/CHANGELOG.md`** — Workspace-Level-Eintrag verlinkt die Crate.
- [ ] **`website/docs/<crate>.md`** — Public-Doc-Page mit Quickstart + Spec-Anker + Layer-Position.
- [ ] **`website/spec-coverage/<spec>.md`** — Public-Mirror der Coverage-Doc (gleiche Source, kein Branch-Drift).

Zwei Ordner werden im Dev-Repo getrackt; zum Public-Release wird `github/` als eigener Working-Tree nach `github.com/zero-objects/zero-dds` gepusht und `website/` nach `zerodds.org` deployed.

### 1.13 Spec-Conformance-Audit (HARD-BLOCKER)

Zentrale RC1-Regel: **ein Feature ist entweder vollständig oder es bleibt
auf 🔄 in-review**. Versteckte TODOs, "Phase-X"-Deferrals, "out-of-scope"-
Marker und "intra-ZeroDDS-only"-Kompromisse sind keine zulässigen RC1-
Endzustände.

Jede Crate muss vor Sign-off folgende Sweeps bestehen:

```bash
# Inline-Deferral-Marker
rg -in 'TODO|FIXME|XXX|HACK|Phase-?[0-9]|deferred|out.of.scope|scheduled.for' crates/<crate>/src/

# "Layering-violation"-Begründungen (rote Flagge)
rg -in 'layering.violation|layer.break|bewusst.designen' crates/<crate>/src/

# Intra-Vendor-only-Kompromisse
rg -in 'intra-zerodds|cross.vendor.*nicht|interop.bleibt' crates/<crate>/src/
```

- [ ] **Inline-Deferral-Marker**: 0 Treffer, oder pro Treffer ein Sub-
      Finding F-X-N im Review-Doc mit Status "✅ resolved" (nicht "documented").
- [ ] **Spec-Section-Coverage**: jede für die Crate relevante Spec-§
      (siehe `docs/spec-coverage/<spec>.md`) ist auf `done`. `partial`/
      `open` sind RC1-Blocker.
- [ ] **Wire-Konformität**: bei Wire-Specs (RTPS, XCDR, AMQP, GIOP, …)
      muss die Wire-Bytes-Form der Spec entsprechen, nicht einer
      "ZeroDDS-internen Variante". Cross-Vendor-Interop-Path muss
      mindestens auf Wire-Bytes-Ebene verifiziert sein, auch wenn
      live-Test-Setup fehlt.
- [ ] **Kohärenz**: jedes Feature ist (a) in sich kohärent (Wire +
      Semantik), (b) mit allen Modulen wired (kein "wir senden's, aber
      Empfänger wirft's weg"), (c) getestet. Drei-Punkte-Liste explizit
      in Review-Doc.

**Wenn ein Author-Kommentar im Code ein Deferral oder Architektur-Trade-
off begründet** (z.B. "layering violation, die man bewusst designen
muss"): **das ist ein Finding, kein Beleg**. Die Architektur ist so zu
gestalten, dass die Spec einhaltbar ist, nicht umgekehrt.

Memory-Anker: `feedback_no_hidden_todos_full_spec.md`,
`feedback_no_phase_deferral_in_idl.md`,
`feedback_spec_completeness_over_competition.md`,
`feedback_no_mvp_build_product.md`.

---

## 2 Forbidden-Token-Sweep

Pro Crate vor RC1-Sign-off mit `ripgrep` durchsuchen. Treffer sind entweder zu entfernen oder explizit als „internal-only File" zu markieren (siehe §3 Public-Strategy).

### 2.1 Hard-Forbidden (nie in Public-Mirror)

```bash
rg -g '!target/' -g '!.git/' -i \
  -e 'llvm@llvm' -e 'sandra-kessler' -e 'gitlab\.sandra-kessler' \
  -e 'fishermen21' -e '/Users/sandrakessler' -e 'admin@ifyna' \
  -e 'PDE-Spec' -e 'zero_concept' -e 'zero-principle' -e 'Zero-Principle' \
  -e 'Ghost-Inject' -e 'R-09[7-9]' -e 'R-10[0-4]' -e 'R-110' \
  -e '\bseesaw\b' -e 'IfynaNeu' -e 'paperless' \
  -e '\bglr1\b' -e '\bglr2\b' -e '/tmp/cyc\.xml' \
  crates/<crate-name>
```

Erwartung: **keine Treffer ausserhalb von `tests/cyclone_live_*` und `tests/common/cross_vendor.rs`** (Live-Test-Files sind ohnehin im Public-Mirror-Exclude).

### 2.1c Datums-Marker in Doc-Headers / Spec-Audits

Files sind selbst ihre eigene Stand-Materialisierung; git-commits tragen die zeitliche Reihenfolge. Datums-Marker (`Stand 2026-05-05`, `Letzte Aktualisierung: ...`, `done — RC1-Audit YYYY-MM-DD`) im File-Body verdoppeln diese Information out-of-band und divergieren mit der Zeit (Pillar 7 Zero-Out-of-Band, Foundation §5 Track).

```bash
rg -g '!target/' -g '!.git/' \
  -e 'Stand:?\s*\d{4}' \
  -e 'Letzte Aktualisierung' \
  -e 'Last [Uu]pdated' \
  -e 'RC1-Audit\s*\d{4}' \
  -e '\(\d{4}-\d{2}-\d{2}\)' \
  -e '^>\s*\*\*Stand:\*\*' \
  crates/<crate-name> docs/spec-coverage/
```

**Erlaubt** sind dagegen Datums-Marker in:
- CHANGELOG-Entries (`## [1.0.0-rc.1] — 2026-MM-DD` ist Konvention von Keep-a-Changelog, dort ist git-Track unzureichend weil ein einzelner Eintrag mehrere Commits zusammenfasst)
- Memory-Files unter `~/.claude/projects/.../memory/` (Persistenz-Layer, eigene Track-Quelle)
- Live-Test-Modul-Docs wo das Datum konkret ein Lab-Setup-Stand referenziert (Lab ist ohnehin nicht im Public-Mirror)

### 2.1b Sprint-/Project-Management-Sprache (nie in Public-Mirror)

Sprint-interne Marker wie `WP 5.D.1`, `Phase-5 Cluster-D`, `Sprint-17 #58` sind Dev-Repo-Sprache. Sie referenzieren interne Sprint-Boards und Roadmaps, die im Public-Mirror nichts zu suchen haben. Pro Crate ersetzen durch fachliche Begründung („Hot-Path-Optimierung", „Lock-Free-Read-Path", „Observability-Sink-Schicht") oder ganz entfernen.

```bash
rg -g '!target/' -g '!.git/' -i \
  -e '\bWP[ -]?[0-9]' -e '\bWP-[0-9A-Z]' \
  -e '\bPhase[- ]?[0-9]' \
  -e '\bCluster[- ]?[A-Z0-9]' \
  -e '\bSprint[- ]?[0-9]' \
  crates/<crate-name>
```

Auch in Doc-Strings, lib.rs-Headers, README-Texten, Coverage-Docs zu entfernen. Im Spec-Coverage-Doc darf stattdessen ein Datum stehen („done — RC1-Audit YYYY-MM-DD"), aber kein Sprint-Marker.

**Ausnahme:** `docs/plans/`, `docs/test-harness/`, `docs/PHASE*.md` etc. sind ohnehin internal-only und im Public-Mirror-Exclude — dort dürfen die Marker stehen bleiben.

### 2.2 Soft-Review (manuell prüfen)

```bash
rg -i -e 'TODO\b' -e 'FIXME\b' -e 'XXX\b' -e '\bhack\b' \
   -e 'workaround' -e 'tmp\b' -e 'temporary\b' \
   crates/<crate-name>
```

Pro Treffer entscheiden: legitime TODO mit Issue-Link → behalten, oder Tech-Debt-Marker ohne Plan → entfernen oder Issue eröffnen.

### 2.3 Lab-Refs in Comments / Doc-Strings

```bash
rg -i -e 'pve\b' -e 'pivot\b' -e 'enp6s18' -e 'Lab-' -e 'sshpass' \
   crates/<crate-name>
```

Live-Test-Files: ok. Production-Source: nicht ok.

### 2.4 Internal-Customer-/Project-Names

```bash
rg -i -e 'kunde:' -e 'customer:' -e 'project:' -e '\bzero_concept\b' \
   crates/<crate-name>
```

Erwartung: keine Treffer.

---

## 3 Public-Strategy-Klassifikation pro Crate

Jede Crate bekommt im Review-Doc eine der folgenden Markierungen:

| Marker | Bedeutung | Konsequenz |
|---|---|---|
| **public** | wandert ins externe GitHub-Repo + crates.io | Forbidden-Sweep streng |
| **public-feature-gated** | wandert raus, aber default-disabled (z.B. `--features inspect`) | Forbidden-Sweep streng + Feature-Doc |
| **embargo** | bleibt intern bis Trigger-Event | Trigger im Review-Doc nennen (z.B. „bis PDE-Release") |
| **internal-only** | wandert nicht raus, nicht released auf crates.io | `publish = false` in Cargo.toml |
| **drop** | wird vor RC1 gelöscht | Begründung im Review-Doc |

Default für alle Crates ist **public**. Ausnahmen brauchen Begründung.

---

## 4 Spec-Doku-Update-Patterns

Wenn ein Crate-Review eine Anpassung der Spec-Coverage-Doc auslöst, gelten diese Regeln:

### 4.1 Status-Übergänge

- `partial` → `done`: erlaubt, wenn alle Sub-Items implementiert sind, mit Repo+Tests-Belegen.
- `open` → `done`: erlaubt, wenn vollständige Implementation + Tests da.
- `done` → `partial`/`open`: nur bei dokumentiertem Regression-Befund (Memory `feedback_no_bulk_done_phrases`).

### 4.2 Belegformat

```markdown
### §X.Y <Anforderung>
- **Anforderung:** <eine Zeile aus der Spec, optional mit §-Referenz>
- **Repo:** `<crate>::<modul>::<symbol>` (+ ggf. weitere)
- **Tests:** `<test-modul>::<test-fn>` (+ ggf. weitere)
- **Status:** done
```

**Keine Datums-Marker.** Die zeitliche Reihenfolge wird durch git-commits getragen (Foundation §5 Track-Materialisierung). Ein File-Body-Stempel `2026-MM-DD` neben `git log <file>` erzeugt out-of-band-State zur Track-Quelle.

### 4.3 Memory `feedback_no_bulk_done_phrases`

> "Cross-Vendor-Korpus pflegt" reicht nicht; pro Item drei Belege (Spec/Repo/Test).

Daran halten wir uns auch im RC1-Audit.

---

## 5 Walking-the-DAG-Reihenfolge

Wir reviewen Crates schichtweise:

| Layer | Crates (Reihenfolge intern alphabetisch) |
|---|---|
| **0 Foundation** | foundation |
| **1 Primitives** | cdr, lint, qos, time-service, types |
| **2 Wire** | discovery, rtps, transport, transport-shm, transport-tcp, transport-tsn, transport-udp, transport-uds |
| **3 Schema** | idl, idl-cpp, idl-csharp, idl-java, idl-ts, xml, xml-wire (`zerodds-xml-wire`) |
| **4 Core Services** | dcps, dcps-async, flatdata, flatdata-derive, monitor, observability-otlp, recorder, rpc, rt-linux, security, security-crypto, security-keyexchange, security-logging, security-permissions, security-pki, security-rtps, security-runtime, sql-filter |
| **5 Bridges** | amqp-bridge, amqp-endpoint, coap-bridge, grpc-bridge, hpack, http2, mqtt-bridge, websocket-bridge, zenoh-bridge |
| **6 PSMs/Bindings** | cpp, cs, zerodds-c-api, java-omgdds, java, java-omgdds, py, rs, sys, ts-node, ts-wasm |
| **7 Profiles** | conformance, zerodds-soap, dlrl, dlrl-codegen, opcua-gateway, rmw-zerodds-shim, ros2-rmw, web, xrce, xrce-agent, xrce-client |
| **8 CORBA-Stack** | ami4ccm, ccm, corba-ccm, corba-ccm-ejb, corba-ccm-lib, corba-codegen, corba-cos-event, corba-cosnaming, corba-csiv2, corba-dds-bridge, corba-dnc, corba-giop, corba-iiop, corba-ior, corba-ir, corba-poa, rtc |
| **🚫 Embargo** | inspect-endpoint (bis PDE-Release) |

Tools + Examples kommen **nach** den Crates.

Jede Crate ist nur RC1-ready, wenn alle ihre Dependencies (lower-Layer) bereits RC1-ready sind. Das verhindert circular „warten-auf-X".

---

## 6 Workspace-Wide Gates

Zwischen jedem Layer-Wechsel:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --tests -- -D warnings
cargo test --workspace --tests
cargo run --bin zerodds-lint -- check
cargo doc --workspace --no-deps
```

Alle fünf MÜSSEN grün sein, bevor wir den nächsten Layer starten. Bei Reibach (z.B. ein RC1-Crate macht ein nicht-RC1-Crate clippy-rot) wird das blockierende Item gefixt.

---

## 7 Wann ist die ganze Walk fertig?

Wenn:

1. Alle 89 Crates auf `✅ rc1-ready` im Tracker (außer Embargo + Drop)
2. Alle Tools auf `✅ rc1-ready`
3. Alle Examples auf `✅ rc1-ready`
4. Mainline-Doku gereviewed (`docs/architecture/`, `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`)
5. Workspace-Gates grün (siehe §6)
6. Public-Mirror-Repo angelegt + initial-Sync gemacht
7. Website-Skelett steht
8. crates.io-Publish-Dry-Run erfolgreich

Dann: Workspace-Tag `r1.0.0` und Release.
