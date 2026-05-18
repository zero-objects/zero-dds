# RC1 Review — `<crate-name>`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md` (DoD + Forbidden-Tokens + Public-Strategy).
> **Layer:** <0–8 / Tool / Example>
> **Reviewer:** <Name>
> **Public-Strategy:** <🌐 public / 🔒 public-feature-gated / 🚫 embargo / 🏠 internal-only / 🗑 drop>
>
> Track-Materialisierung via git: `git log docs/release/rc1-reviews/<crate>.md`.

---

## 1 Purpose

Eine Zeile, was die Crate tut.

## 2 Public-Strategy

- **Marker:** <public / public-feature-gated / embargo / internal-only / drop>
- **Begründung:** <warum dieser Marker — z.B. „bis PDE-Release intern", „Lab-Tooling, kein Public-Use", „Default-Crate für DDS-User">
- **Trigger zur Lift-Up** (nur bei embargo): <z.B. „PDE 1.0 published">

## 3 Content-Inventur

### 3.1 Module

```
src/
├── lib.rs           # <Zweck>
├── <module>.rs      # <Zweck>
└── ...
```

### 3.2 Public-API-Surface

```rust
// Top-Level pub-Items:
pub struct <X>;
pub fn <y>();
pub trait <Z>;
// ...
```

Aufgezählt aus `cargo public-api -p <crate>` oder manuell aus `lib.rs`.

### 3.3 Tests

- `cargo test -p <crate>` lokal: ✅ / ❌  (`<n> passed, <m> failed`)
- E2E-Tests (Live/Cyclone/etc.): aufzählen und Status

### 3.4 Coherence-Audit (Public-API × Cross-Crate × Spec)

Eine Zeile pro Public-Item-Familie. „External Refs" zählt nur Production-Code in **anderen** Crates; eigene Tests + intra-crate-Calls sind nicht als Beleg gewertet.

| Public-Item | Spec-Anker | External Production-Refs | Test-Refs | Klassifikation | Decision |
|---|---|---|---|---|---|
| `<TopType>` | <Spec §X.Y / Vendor-Extension / Hook> | <count + Crate-Liste> | <count> | <CONNECTED / TEST-ONLY / SPEC-MANDATED-OPEN / OPTIONAL-HOOK / VENDOR-EXTENSION / DEAD> | <—  / wire-up / defer / drop / doc-as-hook> |
| `<funktion>` | ... | ... | ... | ... | ... |

Hinweis: bei aggregierten Items (z.B. eine ganze Trait-Familie) eine Zeile pro Familie reicht; einzeln auflisten nur wenn die Familie heterogen ist (Teile connected, andere dead).

## 4 Wiring

### 4.1 Dependencies (uses)

```toml
[dependencies]
zerodds-foundation = { path = "../foundation" }
# ...
```

### 4.2 Dependents (used-by)

```bash
$ rg -l '<crate-name>' --type-add 'cargo:*Cargo.toml' -t cargo crates/
```

Liste: <crate-A>, <crate-B>, ...

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | std-Aufruf |
| `<x>` | ❌ | <Zweck> |

## 5 Spec-Relevanz

- **Spec(s):** <OMG-Spec-Name §X.Y / RFC nnnn / interne ZeroDDS-Spec>
- **Coverage-Doc(s):** `docs/spec-coverage/<spec>.md`
- **abgedeckte §-Sektionen:** <Liste>

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

Befehl (siehe `RC1_GUARDRAILS.md` §2.1):

```bash
rg -g '!target/' -i \
  -e 'llvm@llvm' -e 'sandra-kessler' -e 'fishermen21' \
  -e '/Users/sandrakessler' -e 'PDE-Spec' -e 'zero_concept' \
  -e 'zero-principle' -e 'Ghost-Inject' -e 'R-09[7-9]' \
  -e 'R-10[0-4]' -e 'R-110' -e '\bseesaw\b' \
  crates/<crate>
```

Treffer:
- <File:Line — Treffer — Aktion (entfernt / behalten weil tests/cyclone_*)>

### 6.2 Soft-Review-Treffer (TODO/FIXME/HACK)

```bash
rg -i -e 'TODO\b' -e 'FIXME\b' -e 'XXX\b' -e '\bhack\b' crates/<crate>
```

Treffer:
- <File:Line — Treffer — Entscheidung>

### 6.3 Tech-Debt + Dead Code

Gefunden:
- <Modul / Funktion / Konstante — Begründung warum dead-code>

### 6.4 Public-API-Leaks

Gefunden:
- `pub use crate::<...>::*;` — wird zu `pub use crate::<...>::{X, Y};`
- ungewollt `pub` markierte Helper — auf `pub(crate)` reduziert

## 7 Cleanup-Actions

Was wir tatsächlich getan haben (mit Commit-Hash wenn möglich):

1. <Aktion 1>
2. <Aktion 2>
3. ...

## 8 Spec-Doc-Updates

Anpassungen in `docs/spec-coverage/<spec>.md`:

- §X.Y: `partial` → `done` (Belege: Repo + Tests)
- §X.Z: neu hinzugefügt
- ...

## 9 Doc-Artefacts

- [ ] `Cargo.toml`-Metadata vollständig (siehe Guardrails §1.1)
- [ ] `lib.rs`-Crate-Header mit Safety-Class + Spec-Ref + Layer + API-Aufzählung (Guardrails §1.2)
- [ ] `README.md` aus Template (Guardrails §1.3)
- [ ] `CHANGELOG.md` mit `[1.0.0-rc.1]`-Entry (Guardrails §1.4)
- [ ] doc-tested Code-Example oder Begründung warum nicht

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p <crate>           # ✅ <n> passed
cargo clippy -p <crate> --tests -- -D warnings   # ✅
cargo fmt -p <crate> -- --check                  # ✅
cargo doc -p <crate> --no-deps                   # ✅
cargo run --bin zerodds-lint -- check                # ✅ workspace-weit
```

## 11 RC1-DoD-Checkliste

(Alle aus `RC1_GUARDRAILS.md` §1)

- [ ] §1.1 Cargo.toml-Metadata
- [ ] §1.2 lib.rs Crate-Header
- [ ] §1.3 README.md aus Template
- [ ] §1.4 CHANGELOG.md mit RC1-Entry (initial-Materialisierung-Format)
- [ ] §1.5 Public-API-Audit
- [ ] §1.5b Coherence-Audit (Tabelle in §3.4 ausgefüllt, alle ❌ haben Decision)
- [ ] §1.6 Spec-Coverage-Update
- [ ] §1.7 Forbidden-Token-Sweep
- [ ] §1.8 License-Header pro File
- [ ] §1.9 Tests + Lints + Doc-Build grün
- [ ] §1.10 Review-Doc ausgefüllt (= dieses Dokument)
- [ ] §1.11 Tracker auf ✅
- [ ] Findings-Tracker `RC1_FINDINGS.md` aktualisiert (falls deferred)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer-Sign-off:** <Name>
- **Tracker-Eintrag aktualisiert:** ✅

(Sign-off-Zeitpunkt = git-commit-Zeitpunkt dieser Datei.)
