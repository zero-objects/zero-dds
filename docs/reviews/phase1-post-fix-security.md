# Phase-1 Post-Fix Security-Audit — ZeroDDS

**Datum:** 2026-04-20
**Scope:** Re-Review nach P1-Fix-Session. Verifikation der beiden
Pflicht-Fixes + Neubewertung von Arc-Payloads (P1-6b), `#[non_exhaustive]`
(P1-5) und Resuche nach ungekappten Allokationen.

## Status-Tabelle (15 Orig-Findings + 2 Fix-Verifikationen)

| #      | Severity | Titel | Status | Evidenz |
|--------|----------|-------|--------|---------|
| 1      | Medium   | `cargo-audit` fehlt lokal | offen | lokal nicht installiert, CI-gedeckt |
| 2      | Medium   | `cargo-deny` fehlt lokal | offen | lokal nicht installiert, CI-gedeckt |
| 3      | Low      | `windows-sys` Duplicate-Skip | offen | `deny.toml:45`, doc-ok bis Windows-Build |
| 4      | Low      | 55 External Deps | offen (info) | moderat |
| **5**  | **High** | `type_lookup::decode_from` unbounded | **FIXED** | `crates/types/src/type_lookup.rs:107` — `safe_capacity(n, 1, r.remaining())` |
| 6      | Medium   | `Vec<T>::decode` ohne `DECODE_PREALLOC_CAP` | offen | `crates/cdr/src/composite.rs:113` unverändert |
| **7**  | Low→Crit→Fixed | ParameterList unbegrenzt | **FIXED (als P1-1)** | `crates/rtps/src/parameter_list.rs:166-170, 191` — `MAX_PARAMETERS=4096` |
| 8      | Info     | `serialized_payload` Rest-Kopie | offen | by-design, <=64 kB Submessage-Cap |
| 9      | Info     | 0 `unsafe` im produktiven Code | nachgeprueft OK | `forbid(unsafe_code)` workspace-weit |
| 10     | Info     | `deny(unsafe_code)` konsistent | OK | alle 26 `lib.rs` |
| 11     | Info     | MD5 nur spec-konform | OK | kein security-use |
| 12     | Info     | Keine self-rolled Crypto | OK | RustCrypto-only |
| 13     | Info     | Kein Token-Handling in P1 | OK | `security/src/lib.rs` = Stub |
| 14     | Low      | `SECURITY.md` Platzhalter | offen | public-release Blocker |
| 15     | Low      | CDR/TypeObject ungefuzzt | offen | WP 2-Backlog |
| **P1-1** | Crit   | ParameterList `MAX_PARAMETERS` | **verifiziert** | commit 39cea45 |
| **P1-2** | High   | type_lookup `safe_capacity` | **verifiziert** | commit 39cea45 |

Beide Pflicht-Fixes sind korrekt platziert und decken die im Orig-Audit
beschriebenen Angriffsszenarien (u32::MAX-Preallocate, zero-length-PID-
Flood) ab.

## Neue Findings aus Folge-Commits

### N1 — Arc<[u8]>-Payloads: **kein Data-Race-Risiko** (Info)

Nach `78edb92` (P1-6b) ist `CacheChange::payload: Arc<[u8]>`. Audit:
- `Arc<[u8]>` ist `Send + Sync`, **immutable slice dereference** —
  `Arc::deref` liefert `&[u8]`, kein `&mut`-Pfad im API-Exposure.
- Keine Verwendung von `Arc::get_mut` / `Arc::make_mut` im Workspace
  (grep: 0 Treffer).
- Reader (`reader.rs:79`, `reliable_reader.rs:132`) exponieren die
  Payload als `pub payload: Arc<[u8]>` — der Konsument bekommt einen
  ref-counted immutablen Slice. `Arc::from(Vec<u8>)` consumiert den
  Vec, danach ist die Allokation eingefroren.
- Fazit: Rust-Typsystem + API-Design schliessen Aliasing-Bugs aus.
  Keine Aktion noetig.

### N2 — `#[non_exhaustive]` auf 8 Error-Enums: workspace-intern safe (Info)

`cargo check --workspace --all-targets` gruen (s. Appendix). Innerhalb
des Workspaces hat kein `match`-Arm auf diese Enums `#[non_exhaustive]`
vergessen (der Compiler haette sonst geknickt). SemVer-Firewall ist
aktiv; externe Konsumenten muessen ab jetzt `_ =>`-Catch-All nutzen.

### N3 — `CompletedSample::payload: Vec<u8>` inkonsistent mit Arc-Story (Low)

`crates/rtps/src/fragment_assembler.rs:46` liefert fragmentierte
Samples als `Vec<u8>` statt `Arc<[u8]>`. Sicherheitsrelevant: nein
(DoS-Caps in `AssemblerCaps` gewaehren die Obergrenze). Architektur-
relevant: ein ungeplanter Copy in den Cache-Insert-Pfad.
**Empfehlung:** in WP 2.0a-Folge vereinheitlichen.

## Weitere Scans (negativ)

- `Vec::with_capacity(n)` mit attacker-controlled `n` ohne Cap:
  alle Treffer ueber `safe_capacity`, `MAX_PARTITIONS`,
  `RTPS_BITMAP_MAX_BITS` oder `remaining_bytes/2` gekappt.
  Ausnahme #6 (composite.rs:113) weiterhin offen, Severity Medium.
- `Arc::from`-Call-Sites: `reader.rs:58`, `history_cache.rs:64`,
  `reliable_writer.rs:232` — alle konsumieren `Vec<u8>` aus bereits
  validierten Submessage-Bodies (Submessage-Header-Laenge `u16`,
  <=64 kB). Kein unbounded-Pfad.

## cargo audit / cargo deny

Lokal weiterhin nicht installiert:
```
$ cargo audit
error: no such command: `audit`
$ cargo deny --version
error: no such command: `deny`
```
CI-Pipeline (GitLab Runner `glr1`) fuehrt beide Tools pro Push aus.
Unveraendert gegenueber Orig-Audit.

## Pre-Phase-2-Release-Checkliste

1. **#6 fixen:** `Vec<T>::decode` in `zerodds-cdr` nutzt `safe_capacity`
   (Helper nach `zerodds-foundation` heben).
2. **#14 fixen:** `SECURITY.md` Platzhalter durch Kontaktadresse ersetzen.
3. **#1/#2:** `cargo install cargo-audit cargo-deny` in
   `scripts/pre-commit` + `CONTRIBUTING.md` verdrahten.
4. **#15 Start:** CDR- und TypeObject-Fuzz-Targets anlegen (WP 2.0-
   Hygiene-Ticket).
5. **N3:** `CompletedSample::payload` auf `Arc<[u8]>` ziehen, in
   gleichem PR wie Fragment-Reassembler-Perf-Benchmarks.
6. **WP 2 Kickoff:** DDS-Security-Plugin-Scaffold inkl. `subtle::
   ConstantTimeEq` fuer alle kuenftigen Token-Vergleiche.

## Verdict

Beide P1-Crit/High-Fixes korrekt. Arc-Payloads zeigen kein
Data-Race-Risiko (Rust-Garantien + API-Disziplin). `#[non_exhaustive]`
workspace-intern konsistent (Compiler-verifiziert). Verbleibende offene
Findings sind Medium/Low und blockieren keinen Phase-1-Close.
Phase-2 kann starten — #6 und #14 vor dem public Release fixen.

**Wortanzahl (Fliesstext ohne Tabelle/Code):** ~395.
