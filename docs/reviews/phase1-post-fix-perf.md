# Phase 1 Post-Fix Performance-Audit

Stand 2026-04-20. Folge-Review zu `phase1-perf-audit.md` nach
Hotfixes F17 und F7/F8/F10.

## 1. Fix-Verifikation

- **F17 adressiert.** `rtps/src/parameter_list.rs:144,166,191` —
  `MAX_PARAMETERS = 4_096`, Early-Abort vor `push()`.
- **F7/F8 adressiert (Cache-Seite).** `history_cache.rs:53`
  `payload: Arc<[u8]>`; `reliable_writer.rs:232` konvertiert einmal
  via `Arc::from(payload)`; `cache.insert` und
  `change.payload.clone()` in `tick` (`:286,:304`) sind
  Refcount-Bumps.
- **F10 teilweise.** `reader.rs:58` und
  `reliable_reader.rs:132,287,427` halten `Arc<[u8]>`;
  `DeliveredSample` ist Zero-Copy. Submessage-Build bleibt offen,
  siehe N1.

## 2. Neue / verbleibende Regressionen

### N1 [M] Submessage-Struct bleibt `Vec<u8>`-owned
`submessages.rs:416,868`: `DataSubmessage::serialized_payload` und
`DataFragSubmessage::serialized_payload` sind weiterhin `Vec<u8>`.
`reliable_writer.rs:428,599,671` macht pro DATA/DATA_FRAG einen
`payload.to_vec()` bzw. `chunk.to_vec()`. Der Arc-Fix deckt nur die
Cache-Seite — realer Speedup heute ~10-15 %, nicht die
prognostizierten 30-50 %.
**Fix:** `write_body_into(&mut Vec<u8>, &[u8])` oder
`Cow<'a, [u8]>`; gehoert in `phase2-spike-arc-payloads.md`.

### N2 [L] `Arc::from(Vec<u8>)` im Writer-Hot-Path
`reliable_writer.rs:232` allokiert pro `write()` den Arc-Header
zusaetzlich. Fuer kleine Payloads (<50 Byte) ~5-8 % Overhead
vs. `Vec::clone`. Design-relevant fuer WP 2.1 DCPS-API — sollte
zusaetzlich einen `write_arc(Arc<[u8]>)`-Entrypoint bieten, damit
Caller die Alloc sparen. Heute kein Blocker.

### N3 [L] Arc-Atomics auf Multi-Core-Tick
`Arc::clone` ist fetch-add. Solange Writer-Tick single-threaded
bleibt, irrelevant. Falls WP 2.x Multi-Writer-Pool einfuehrt:
Per-Shard-`Rc` oder `triomphe::Arc` evaluieren.

## 3. Status-Tabelle

| ID | Status |
|----|--------|
| F7 F8 F9 F17 | Fixed |
| F10 | Teil-Fix (Submessage offen → N1) |
| F1 F2 F3 F4 F5 F6 F11 F12 F13 F14 F15 F16 F18 F19 F20 | Open |
| N1 N2 N3 | **Neu** (siehe §2) |

## 4. Empfehlung: WP 2.0a Perf-Spike-Bundle

Drei thematisch gekoppelte Items mit messbarem Nutzen:

1. **N1 + Rest F7/F10** — Submessage-Build auf `&[u8]`/Cow,
   `write_body_into` einfuehren. Erst damit schlaegt die
   Arc-Umstellung auf die 30-50 %-Prognose durch. Reuse
   `phase2-spike-arc-payloads.md`.
2. **F3 SEDP-Cache-Index** — `BTreeMap<GuidPrefix, BTreeMap<SN,
   Guid>>`. Blockiert die "10 k Topics"-Sales-Demo; ohne Fix
   quadratisches Fuellen.
3. **F18 TypeLookup-Hygiene** — Registry-Cap,
   `compute_hash(&m)` ohne Clone, Pending-Requests-Match.
   Security + Perf kombiniert.

**Separater Bundle WP 2.0b:** F1/F2/F4/F5 (Vec → BTreeMap). Kleines
Risk-Surface, Junior-Session tauglich.

F11/F20 bleiben Phase 2 wie geplant; F13-F16 Cosmetic oder an
Async-Port gekoppelt.
