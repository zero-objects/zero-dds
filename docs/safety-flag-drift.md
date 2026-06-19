# Drift-Doc: `safety`-Feature-Flag

**Status:** completed (2026-06-13) — Option 1 (Feature voll ausgerollt) umgesetzt.

## Resolution (2026-06-13)

Option 1 (kohärente Endform) implementiert:

1. **`safety = []`-Marker** zu allen 19 SAFE-klassifizierten Crates ergänzt, die
   ihn nicht hatten (cdr, types, qos, rtps, discovery, transport, transport-udp,
   idl, idl-cpp/-csharp/-java, sql-filter, security-crypto/-keyexchange/-logging/
   -permissions/-pki/-rtps/-runtime). Damit ist der per-Crate-Clippy-Safe-Lint-
   Gate (`cargo clippy -p <crate> --features safety -- -D clippy::unwrap_used …`,
   Doc Z.103) für jede Safe-Crate real lauffähig.
2. **Meta-Crate `crates/safe-crates-only`** angelegt: aggregiert das Safe-Subset
   (foundation, cdr, types, qos, rtps, discovery, transport, transport-udp,
   security, xrce-client, sys) und reicht `safety` + `alloc` (kein std) durch.
   `cargo build -p safe-crates-only --no-default-features --features safety`
   (Doc Z.93) baut jetzt **real no_std grün** — das ✓ ist eingelöst.
3. **Latenter no_std-Bug gefixt:** `crates/discovery/src/security/stack.rs:179`
   initialisierte `remote_vendors` ohne das `#[cfg(feature = "std")]`-Gate, das
   Feld + Import + Nachbar-Initializer tragen → der Safe-Profil-Build deckte es
   auf. (std-Build war nie betroffen.)
4. **Klassifizierungs-Drift:** `zerodds-transport-shm` ist in der Architektur-
   Doc-Liste, aber lib.rs-klassifiziert **STANDARD** → NICHT im Meta-Crate;
   ebenso `idl*` (std-only Build-Tools).

---

## (Historie) Status: offen / zu bewerten
**Aufgenommen:** während Website-Spec-Coverage-Review (Auslöser: `types.html`
listete ein Phantom-`safety`-Feature, das in `crates/types/Cargo.toml` nicht
existiert).
**Verantwortlich:** Implementer (Bewertung + Entscheidung über Vorgehen).
**Scope:** Crate-`Cargo.toml`s + `docs/architecture/04_safety_by_architecture.md`
(nicht Website).

---

## Kurzfassung

Das `safety`-Feature-Flag ist ein **halb-verdrahtetes Architektur-Konzept**:
es existiert in einigen Crates als leerer Marker, **fehlt** aber in mehreren
SAFE-klassifizierten Crates, und das im Architektur-Doc dokumentierte
Safe-Build-Target (`safe-crates-only`) **existiert gar nicht** — obwohl das Doc
den Build als bestehendes CI-Gate (✓) führt.

Der eigentliche Safety-Hardening (Klassifizierung, `forbid(unsafe_code)`,
Speicher-Regeln) ist davon unabhängig **real** vorhanden.

---

## Was `safety` sein soll

Laut `docs/architecture/04_safety_by_architecture.md`:

* Ein **leerer Marker-Feature** (`safety = []`, gatet keinen `#[cfg(...)]`-Code),
  den jede Safe-Crate exponiert.
* Zweck 1 — **Safe-Profil-Build (no_std):**
  `cargo build -p safe-crates-only --no-default-features --features safety`
  (Doc Z.93, als CI-Gate mit ✓ markiert).
* Zweck 2 — **Safe-spezifische Lints:**
  `cargo clippy -p <safe-crate> --features safety -- -D clippy::unwrap_used -D clippy::panic -D clippy::unreachable`
  (Doc Z.103, ✓).

---

## Findings (Ist-Zustand)

### 1. `safety`-Feature ist inkonsistent gesetzt

`safety = []` **vorhanden** in (Cargo.toml `[features]`):
`foundation`, `dcps`, `security`, `cs`, `rs`, `sys`, `cpp`, `py`, `xrce`,
`xrce-agent`.

`safety`-Feature **FEHLT** in SAFE-klassifizierten Crates:
`cdr`, `types`, `rtps`, `discovery`, `qos`, `idl`.
(Liste evtl. unvollständig — Implementer sollte alle Crates gegen ihre
`Safety classification`-Annotation auditieren.)

Auffällig:
* `dcps` ist **STANDARD**-klassifiziert, hat aber das Feature.
* `cdr` / `types` / `rtps` sind **SAFE** und stehen in der Safe-Crate-Liste des
  Architektur-Docs (Z.24–28), haben das Feature aber **nicht**.

### 2. `safety` gatet keinen Code

Workspace-weit **0** Vorkommen von `#[cfg(feature = "safety")]`. Das Feature ist
rein ein Build-/Lint-Profil-Marker, kein code-gatender Switch.

### 3. `safe-crates-only`-Meta-Crate existiert nicht

Der im Doc (Z.93) als CI-Gate ✓ geführte Befehl
`cargo build -p safe-crates-only --no-default-features --features safety`
verweist auf ein Package `safe-crates-only`, das im Repo **nicht existiert**
(kein `Cargo.toml` mit diesem Namen). Der Befehl kann nicht laufen → das ✓ ist
ein nicht-eingelöster / aspirativer Claim.

Damit das Feature den dokumentierten Zweck (Safe-Profil-Build über ein
Meta-Crate) erfüllen kann, müsste:
* das Meta-Crate `safe-crates-only` existieren und sein `safety`-Feature an die
  Safe-Crate-Deps durchreichen, **und**
* jede dieser Safe-Crate-Deps das `safety`-Feature exponieren (siehe Finding 1).

### 4. Was real existiert (vom Flag unabhängig)

* **Safety classification** pro Crate im `//!`-Doc (SAFE / STANDARD / TOOLING /
  BINDING / SAFE(std-only)) — durchgängig gepflegt.
* **`forbid(unsafe_code)`** in **70** Crates (`crates/*/src/lib.rs`).
* Safe-Crate-Speicher-/API-Regeln im Architektur-Doc (Z.57 ff.: Static-Allocation,
  kein `std::thread::spawn`, kein `io::Error` in Safe-Public-APIs, …).
* Ferrocene-Certified-Core-Subset als Design-Maßstab (Doc Z.229/248).

---

## Entscheidungs-Optionen (für Implementer)

1. **Konsistenz herstellen (Feature voll ausrollen):**
   `safety = []` zu allen SAFE-Crates (cdr, types, rtps, discovery, qos, idl, …)
   hinzufügen + `safe-crates-only`-Meta-Crate anlegen, das `--features safety`
   an die Safe-Deps propagiert. Dann wird der dokumentierte Safe-Build real und
   das ✓ im Doc eingelöst. `dcps` (STANDARD) prüfen, ob es ins Safe-Set gehört.

2. **Doc ehrlich machen (Minimal):**
   Das ✓ am `safe-crates-only`-Build-Gate und am `safety`-Clippy-Gate auf
   „geplant / aspirativ" zurückstufen, solange Meta-Crate + Feature-Coverage
   fehlen.

3. **Marker abschaffen:**
   Falls der Safe-Profil-Build nicht aktiv verfolgt wird: das leere
   `safety`-Feature aus den 10 Crates entfernen und im Doc auf die real
   wirksamen Mechanismen (Klassifizierung, `forbid(unsafe_code)`, Lint-Config,
   Ferrocene) reduzieren.

Empfehlung der Aufnahme: Option 1 ist die kohärente Endform, Option 2 der
ehrliche Zwischenstand bis dahin. Option 3 nur, wenn das Safe-Profil bewusst
aufgegeben wird.

---

## Begleit-Fix (bereits erledigt)

`website/docs/types.html` listete fälschlich ein
`safety ❌ Reserved für Safety-Class-Hardening (Phase-2)` in der Feature-Flags-
Tabelle, obwohl `crates/types/Cargo.toml` nur `std` + `alloc` kennt. Die Zeile
wurde entfernt (Commit `53c42a6a`), die README-Tabelle war bereits korrekt.
