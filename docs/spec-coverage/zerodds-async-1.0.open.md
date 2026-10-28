# zerodds-async-1.0 — offene Punkte

Aggregat aus `zerodds-async-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §8 Ein async-Pendant pro Sync-Test

**Status:** `partial` — die Async-Oberfläche ist item-für-item belegt, aber `crates/dcps/tests/` hat rund sechzig Test-Files (QoS-Matrix, Lifecycle-Wire, Discovery-Server, Security) und `crates/dcps-async/tests/` neun. Ein 1:1-Pendant pro Sync-Test gibt es nicht. Der Rest steht als eigenes `open`-Item „§8 1:1-Async-Pendant pro Sync-Test".

## §8 tokio::test als Default-Runner, smol als Feature-Probe

**Status:** `partial` — der Default-Runner ist tokio, und die Runtime-Unabhängigkeit ist über einen im Test gebauten Park-Executor belegt. Eine `smol`-Variante gibt es nicht; `smol` ist keine dev-dependency. Der Rest steht als eigenes `open`-Item „§8 smol-Test-Variante".

## §8 E2E gegen Cyclone-Live plus Latenz-Vergleich

**Status:** `partial` — der Latenz-Vergleich sync gegen async läuft als Assertion im Release-Build, im CI-Job `perf-gate`. Der Cyclone-Live-Teil ist `#[ignore]` und lief in diesem Audit nicht. Der Rest steht als eigenes `open`-Item „§8 Cyclone-Live-E2E ohne Lab-Gate".

## §8 1:1-Async-Pendant pro Sync-Test

**Status:** `open` — ohne Beschreibung im Hauptfile.

## §8 smol-Test-Variante

**Status:** `open` — ohne Beschreibung im Hauptfile.

## §8 Cyclone-Live-E2E ohne Lab-Gate

**Status:** `open` — ohne Beschreibung im Hauptfile.
