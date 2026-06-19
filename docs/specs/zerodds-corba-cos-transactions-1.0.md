# `zerodds-corba-cos-transactions` v1.0 — OTS: Object Transaction Service in Rust

ZeroDDS Vendor-Spec. In `crates/corba-cos-transactions` implementiert. Authored im
Stil der OMG-CORBA-Spec (nummerierte Klauseln, RFC-2119-Keywords,
Konformitätsprofil). Die **Wire-Formate** (`otid_t`, `PropagationContext`,
`TransactionService`-ServiceContext) sind OMG-normativ (OMG Transaction Service
1.4); diese Spec normiert das **ZeroDDS-eigene Rust-PSM** dieser Service — die
OMG hat kein Rust-Language-Mapping standardisiert (vgl. `01_scope_and_specs.md`,
„ZeroDDS-eigener, nicht OMG-normierter Anteil").

## Motivation

CORBA-Bestand in der Finanzindustrie hängt am **Object Transaction Service**:
verteilte Transaktionen mit 2-Phase-Commit über mehrere ORBs, propagiert über den
GIOP-`TransactionService`-ServiceContext. Ohne OTS ist eine Finanz-Migration auf
einen neuen ORB nicht „drop-in". `zerodds-corba-cos-transactions` liefert den OTS-
Kern in `no_std + alloc`, `forbid(unsafe_code)`, mit byte-konformer Wire-
Propagation gegen JacORB als Referenz-ORB.

## Ziele

- **`otid_t`-Wire byte-konform** zu OMG OTS App. A — die ORB-übergreifende
  Transaktions-Identität, verifiziert gegen JacORB `otid_tHelper`.
- **`PropagationContext` über GIOP** im `TransactionService`-ServiceContext
  (id = 0), byte-konform zu JacORB `PropagationContextHelper`.
- **2-Phase-Commit-Engine** mit Vote-getriebener State-Machine, Read-Only- und
  One-Phase-Optimierung, Heuristik-Behandlung.
- **Idiomatisches Rust-PSM**: `Otid`, `PropagationContext`, `Resource`, `Vote`,
  `Current`/`Coordinator`/`Terminator`/`Control`.

## Nicht-Ziele

- **Recoverable-2PC mit persistentem Log** (Recovery nach Crash) — v1.0 ist
  in-memory; Durability ist Sache der `Resource`-Implementierung.
- **Verschachtelte Transaktionen mit Sub-Coordinator-Federation** — der
  `PropagationContext` trägt `parents`, aber die Sub-Transaction-Semantik (§OTS
  10.4) ist v1.0 nur strukturell, nicht voll orchestriert.
- **Live Coordinator/Terminator-Object-Ref-Callback** über GIOP (verteiltes 2PC
  über echte IORs) — die `*_ior`-Felder werden getragen (nil bei flacher Tx);
  das typisierte Embedding ist eine Folge-Erweiterung.

## §1 `otid_t` — Transaktions-Identität

### §1.1 Struktur

```text
struct otid_t {
    long             formatID;        // X/Open-XID-Format (-1 = NULL)
    long             bequeath_length; // an Sub-Tx vererbtes tid-Präfix
    sequence<octet>  tid;             // global transaction id + branch qualifier
};
```

Das Rust-PSM ist `Otid { format_id: i32, bequeath_length: i32, tid: Vec<u8> }`.
`Otid::new(format_id, tid)` setzt `bequeath_length` auf die volle `tid`-Länge
(Wurzel-Transaktion); `Otid::null()` liefert `formatID = -1`.

### §1.2 CDR-Encoding

`Otid::encode`/`decode` MÜSSEN exakt `i32 formatID`, `i32 bequeath_length`,
`u32 tid.len()`, `tid`-Bytes schreiben/lesen. Big- und Little-Endian.

**Byte-Konformität (normativ).** `otid_t(formatID=7, bequeath_length=3,
tid=[0xAA,0xBB,0xCC])` MUSS Big-Endian zu

```
00000007 00000003 00000003 aabbcc
```

encodieren — byte-identisch zu JacORB 3.9 `org.omg.CosTransactions.otid_tHelper.write`.

## §2 `PropagationContext` — Transaction-Context-Propagation

### §2.1 ServiceContext

Ein transaktionaler Request trägt im IOP-`ServiceContext` mit der Id
`TRANSACTION_SERVICE_CONTEXT_ID` (= 0, `TransactionService`) eine CDR-
Encapsulation (Byte-Order-Octet + Body) des `PropagationContext`:

```text
struct TransIdentity {
    Coordinator coord;   // Object-Reference (nil-Ref: type_id "" + 0 Profile)
    Terminator  term;    // Object-Reference
    otid_t      otid;
};
struct PropagationContext {
    unsigned long           timeout;
    TransIdentity           current;
    sequence<TransIdentity> parents;
    any                     implementation_specific_data;  // tk_null
};
```

### §2.2 Object-Reference-Encoding

`coord`/`term` MÜSSEN als CORBA-Object-References codiert werden. Eine
**nil-Reference** ist `type_id ""` (CDR-Länge 1 + NUL) + 0 TaggedProfiles. Das
`implementation_specific_data`-`any` ist `tk_null` (1 Wort `00000000`).

**Byte-Konformität (normativ).** `PropagationContext(timeout=30,
current={nil coord, nil term, otid(0,4,[0,0,0,1])}, parents=[], tk_null)` MUSS
Big-Endian zu

```
0000001e 00000001 00000000 00000000 00000001 00000000 00000000
00000000 00000004 00000004 00000001 00000000 00000000
```

(52 Byte) encodieren — byte-identisch zu JacORB 3.9
`org.omg.CosTransactions.PropagationContextHelper.write`.

### §2.3 API

`PropagationContext::flat(timeout, otid)` baut den flachen Kontext;
`to_service_context_data(endianness)` / `from_service_context_data(bytes)`
serialisieren die Encapsulation.

## §3 2-Phase-Commit

### §3.1 `Resource` + `Vote`

Eine `Resource` ist ein transaktionaler Teilnehmer (OMG OTS §10.3.2):

```rust
pub trait Resource {
    fn prepare(&self) -> Vote;                          // Phase 1
    fn commit(&self) -> Result<(), HeuristicOutcome>;   // Phase 2
    fn rollback(&self) -> Result<(), HeuristicOutcome>;
    fn commit_one_phase(&self) -> Result<(), HeuristicOutcome>;
    fn forget(&self);
}
```

`Vote` ist `Commit` | `Rollback` | `ReadOnly`.

### §3.2 Coordinator-Algorithmus

`coordinate_commit(resources)` MUSS:

1. **leere Menge** → `Committed` (nichts zu tun),
2. **eine Resource** → `commit_one_phase` (One-Phase-Optimierung),
3. **sonst** Phase 1 (`prepare` alle): votet *eine* `Rollback` → alle bereits
   mit `Commit` vorbereiteten Resources `rollback`en, Ergebnis `RolledBack`;
   sonst Phase 2 → alle `Commit`-Voter `commit`en (`ReadOnly` entfällt),
   Ergebnis `Committed` (oder `HeuristicMixed` bei Heuristik-Abweichung).

### §3.3 Orchestrierung

`Current` (§OTS 10.3.6) ist kontext-gebunden: `begin` erzeugt eine Transaktion
mit frischer `otid` (zähler-basiert), `commit`/`rollback` beenden sie über den
`Terminator`. `rollback_only` markiert die Transaktion; ein folgendes `commit`
MUSS dann `RolledBack` liefern. `Coordinator::register_resource` ist nur im
`Active`-Status erlaubt.

## §4 Konformität

Ein **OTS-konformes** ZeroDDS-Modul:

1. encodiert `otid_t` und `PropagationContext` byte-konform gemäß §1.2/§2.2,
2. propagiert den Kontext im ServiceContext id = 0 (§2.1),
3. implementiert das 2PC gemäß §3.2 inkl. Read-Only- und One-Phase-Optimierung,
4. liefert die `Current`/`Coordinator`/`Terminator`-Semantik gemäß §3.3.

## §5 Implementierungs-Mapping

| Spec | Code |
|---|---|
| §1 `otid_t` | `corba-cos-transactions/src/otid.rs` — `Otid` |
| §2 PropagationContext | `corba-cos-transactions/src/propagation.rs` — `PropagationContext`, `TransIdentity`, `TRANSACTION_SERVICE_CONTEXT_ID` |
| §3 2-Phase-Commit | `corba-cos-transactions/src/two_phase.rs` — `Vote`, `Resource`, `coordinate_commit` |
| §3.3 Orchestrierung | `corba-cos-transactions/src/transaction.rs` — `Current`, `Coordinator`, `Terminator`, `Control`, `Status` |

## §6 Tests

- Unit (24): `otid` Roundtrip BE+LE + Golden-Vektor; `PropagationContext`
  ServiceContext-Roundtrip + JacORB-Byte-Konformität; 2PC-State-Transitions
  (all-commit/veto/read-only/one-phase/heuristic); `Current` begin/commit/rollback.
- E2E: `tests/ots_distributed.rs` — atomarer Bank-Transfer über zwei Resources,
  Atomic-Rollback bei Veto, Context-Propagation über den Wire.
- Cross-ORB-Byte-Konformität: `otid_t` + `PropagationContext` byte-identisch zu
  JacORB 3.9 (`competitors/jacorb/csiv2-ots/Dump.java`).

## Annex A — Heuristik

Eine `Resource` darf eigenmächtig entscheiden (`HeuristicOutcome::Mixed`/
`Rollback`/`Commit`/`Hazard`). Weicht eine committe Resource von der Coordinator-
Entscheidung ab, MUSS der Coordinator `HeuristicMixed` melden und der Resource
`forget` erlauben. Persistentes Heuristik-Logging ist Nicht-Ziel (§Nicht-Ziele).
