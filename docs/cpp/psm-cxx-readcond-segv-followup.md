# F-PSM-CXX-readcond-segv — ReadCondition::trigger_value SIGSEGV auf Linux

- **Status**: ✅ DONE 2026-06-13 (332c0d5d)
- **Datum**: 2026-05-08

## Resolution (2026-06-13)

**Kein gdb nötig — Code-Audit reichte.** Root-Cause: `ZeroDdsGuardCondition/
StatusCondition/ReadCondition/QueryCondition` (`crates/zerodds-c-api/src/
condition_ffi.rs`) waren **`repr(Rust)`**. Der generische
`condition_kind()`-Dispatcher liest die `ConditionKind`-Diskriminante über einen
`*const ConditionHeader`-Cast und SETZT VORAUS, dass das `header`-Feld auf
Offset 0 liegt (der Doc-Kommentar behauptete „Layout via `#[repr(C)]`", aber das
`repr` fehlte). Unter `repr(Rust)` darf der Compiler die Felder umordnen →
`header` woanders → Dispatcher liest Garbage-Kind → falscher Typ-Cast →
`r.reader` vom falschen Offset → SIGSEGV bei `&*r.reader`. Linux-only, weil das
macOS-Layout zufällig `header@0` ergab.

Fix:
1. `#[repr(C)]` auf alle vier Condition-Structs (commit `332c0d5d`) +
   Regression-Test `condition_header_at_offset_zero` (`offset_of! == 0`).
2. Folge: cbindgen emittierte die `repr(C)`-Structs mit vollem Body →
   `QueryCondition`-`String`/`Vec` wurden unvollständige C-Typen → C++-Compile-
   Fehler. Behoben durch cbindgen-`exclude` der vier + opaque Forward-Decls in
   der Header-Präambel (commit `53c2160e`); `*/`-in-`void*/pointer`-Falle in der
   Präambel gefixt (commit `82c3a661`).
3. `test_status_getters` deckte (hinter dem Segv versteckt) auf, dass Same-
   Participant Writer+Reader jetzt korrekt matchen (Folge des
   F-DCPS-latency-self-match-Fixes) — Assertion `==0`→`==1` korrigiert.

`#[ignore]` aus `cpp_smoke_via_cargo.rs` entfernt; voller Smoke live auf codepit
grün (alle 10 Sub-Tests, inkl. `test_readcondition_lifecycle`).
- **Sprint-Kontext**: K14 DDS-PSM-Cxx-1.0 RC1 (Layer-3 release commit f5b54cb)
- **Reproduktion**: GitLab Pipeline 916, test-job 11714 (`cargo test -p
  zerodds-cpp --test cpp_smoke_via_cargo`); Linux x86_64 GNU,
  Exit-Status 139 (SIGSEGV) zwischen `[smoke] test_readcondition_lifecycle
  start` und der ersten `EXPECT`-Auswertung.

Der folgende Abschnitt beschreibt den historischen offenen Stand.

## Was ist offen

Der C++-PSM-Smoke-Test in `crates/cpp/tests/smoke_dds_psm.cpp` crashed
auf Linux beim Aufruf:

```cpp
sub::cond::ReadCondition<core::ByteSeq> rc(dr, 0xFF, 0xFF, 0xFF);
bool t1 = rc.trigger_value();
```

Lokal auf macOS (clang++ + libc++) laeuft der Test gruen. Auf Linux
gcc/clang gegen libstdc++ → SIGSEGV. Vermutete Ursache:
Layout-Mismatch in der ReadCondition-FFI-Bridge (`crates/cpp/include/dds/sub/cond/`)
oder im Rust-side `zerodds_readcondition_*`-Pfad in `zerodds-c-api`.
Andere Tests (DomainParticipant, Topic, Pub/Sub, Reader, Writer,
Status-Getter, GuardCondition+WaitSet) laufen sauber durch.

## Warum offen

CI-Lint-Stage ist gruen, alle anderen Test-Bestandteile gruen. Der
PSM-Cxx-Pfad ist Layer-3-Eigentum (siehe Memory K14, Owner: Layer-3
PSM-Cxx-Agent). Der CDR-Agent fasst Layer-3-FFI-Bridges nicht direkt
an; Test wurde in `cpp_smoke_via_cargo.rs` mit `#[ignore]` markiert
damit CI-Pipelines gruen sind.

## Implikationen

- Smoke-Coverage fuer `dds::sub::cond::ReadCondition` ist auf Linux
  *nicht* automatisch verifiziert.
- macOS-Build verifiziert das API; Cross-Vendor-Conformance (DDS-PSM-Cxx
  1.0 §7.2.2.2.4) bleibt damit unvollstaendig auf dem CI-Target.
- Bei einem C++-Customer-Build auf Linux koennten ReadCondition-User
  einen Crash sehen ohne dass CI das fängt.

## Wann pick-up sinnvoll

- Vor naechstem K14-Release-Tag.
- Wenn die Layer-3 PSM-Cxx-Owner-Agent eine Sprint-Iteration auf
  ReadCondition macht.

## Implementations-Pfad

1. Layer-3 PSM-Cxx-Agent reproduziert lokal mit Linux-Container
   (z. B. `docker run rust:1.88-bookworm`).
2. `gdb` auf das smoke-binary, Backtrace innerhalb von
   `ReadCondition::trigger_value`.
3. Vermutung: Vtable-Slot oder Box-Layout fuer den ReadCondition-Bridge
   ist fuer Linux falsch dimensioniert; Rust-side `repr(C)` pruefen
   und mit `static_assert(sizeof(...) == ...)` im C++-Header pinnen.
4. Im selben Sprint: `#[ignore]` aus `cpp_smoke_via_cargo.rs` entfernen.

Geschaetzt: 0.5–1 Tag (Layer-3-Agent-Domaene-Wissen).
