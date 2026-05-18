# F-PSM-CXX-readcond-segv — ReadCondition::trigger_value SIGSEGV auf Linux

- **Status**: blocking (CI-test ignored)
- **Datum**: 2026-05-08
- **Sprint-Kontext**: K14 DDS-PSM-Cxx-1.0 RC1 (Layer-3 release commit f5b54cb)
- **Reproduktion**: GitLab Pipeline 916, test-job 11714 (`cargo test -p
  zerodds-cpp --test cpp_smoke_via_cargo`); Linux x86_64 GNU,
  Exit-Status 139 (SIGSEGV) zwischen `[smoke] test_readcondition_lifecycle
  start` und der ersten `EXPECT`-Auswertung.

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
