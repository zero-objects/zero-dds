# zerodds-corba-rust-1.1 — offene Punkte

Aggregat aus `zerodds-corba-rust-1.1.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §13 Cross-ORB-Live + Byte-Konformität

**Status:** `partial` — `crates/corba-interop/tests/ami_cross_orb.rs` braucht einen Bench-Server, den `competitors/run_interop.sh` startet, und `crates/corba-interop/tests/jacorb_live_ots_handshake.rs` einen JacORB-NameServer mit TransactionService; beide laufen in keiner Ebene.
