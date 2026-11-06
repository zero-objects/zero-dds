# zerodds-async-1.0 — offene Punkte

Aggregat aus `zerodds-async-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## Nicht-Ziel — async-Gegenstück zu WaitSet

**Status:** `open` — die Spec schließt die async-Fassung von `WaitSet` aus und beschreibt sie zugleich als `Stream<Item = ConditionEvent>`; dieser Stream fehlt in `crates/dcps-async/`. `WaitSet` selbst bleibt synchron in `crates/dcps/`.

## §8 Ein async-Pendant pro Sync-Test

**Status:** `partial` — die Async-Oberfläche ist item-für-item belegt, aber `crates/dcps/tests/` hat rund sechzig Test-Files (QoS-Matrix, Lifecycle-Wire, Discovery-Server, Security) und `crates/dcps-async/tests/` neun. Ein 1:1-Pendant pro Sync-Test gibt es nicht. Der Rest steht als eigenes `open`-Item „§8 1:1-Async-Pendant pro Sync-Test".

## §8 tokio::test als Default-Runner, smol als Feature-Probe

**Status:** `partial` — der Default-Runner ist tokio, und die Runtime-Unabhängigkeit ist über einen im Test gebauten Park-Executor belegt. Eine `smol`-Variante gibt es nicht; `smol` ist keine dev-dependency. Der Rest steht als eigenes `open`-Item „§8 smol-Test-Variante".

## §8 1:1-Async-Pendant pro Sync-Test

**Status:** `open` — `crates/dcps-async/tests/` hat 9 Testdateien, `crates/dcps/tests/` 150; eine Zuordnung Sync-Test → async-Pendant fehlt.

## §8 smol-Test-Variante

**Status:** `open` — kein Test läuft unter `smol::test`; `smol` kommt nur in der Liste der Runtimes vor, die `crates/dcps-async/tests/default_build.rs` als Abhängigkeit ausschließt.
