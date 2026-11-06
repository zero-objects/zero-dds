# cos-event-service-1.4 — offene Punkte

Aggregat aus `cos-event-service-1.4.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §2.4 Typed Event Communication

**Status:** `partial` — Typed Push (Operationsname und Argumente erreichen den Consumer) und Typed Pull in generischer Form (`TypedInvocation`) sind umgesetzt; es fehlt die abgeleitete Schnittstelle `Pull<I>`, eigenes Item.

## §2.4.2 Abgeleitete Schnittstelle Pull<I>

**Status:** `open` — kein Code erzeugt oder bildet `Pull<I>` ab; `TypedPullSupplier` liefert jede Operation als generisches `TypedInvocation`.

## §2.6 Typed Event Channels

**Status:** `open` — `TypedEventChannel` (`crates/corba-cos-event/src/typed.rs`) stellt nur an typisierte Consumer zu; eine Umsetzung zwischen typisierter und generischer Form fehlt.

## §2.8 Composing Event Channels and Filtering

**Status:** `partial` — der Typfilter nach Repository-ID ist getestet; kein Test verkettet zwei Kanäle.
