# RC1 Review — `zerodds-rt-linux`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 4 (Core Services)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public.

---

## 1 Purpose

Linux Real-Time-Scheduling Adapter: `SCHED_FIFO`/`SCHED_RR`/`SCHED_DEADLINE`-Profile + CPU-Pinning. Die einzige Crate im ZeroDDS-Workspace mit `unsafe { libc::syscall(...) }`-Boundary (COMFORT-Klasse).

## 2 Public-Strategy

🌐 public — keine ZeroDDS-Crate-Deps, klare FFI-Boundary, isolierter Use-Case.

## 3 Content-Inventur

```
src/
├── lib.rs       # Crate-Entry + Re-Exports (Threat-Model + Invarianten dokumentiert)
├── affinity.rs  # pin_current_thread_to_cpus
├── scheduler.rs # SchedulerProfile + SchedulerKind + RunningSchedulerInfo + apply/current
└── syscalls.rs  # private — alle unsafe { libc::syscall(...) }-Bloecke mit per-Block SAFETY-Kommentaren
```

4 src-Files, 814 LOC, **7 Tests** (6 lib + 1 integration) gruen.

### Public-API

```rust
pub use affinity::pin_current_thread_to_cpus;
pub use scheduler::{RunningSchedulerInfo, SchedulerKind, SchedulerProfile};
```

`syscalls` ist `mod syscalls` (private) — keine Public-Surface, alle FFI-Calls leben dort.

### 3.4 Coherence-Audit

| Public-Item | Spec-Anker | External Production-Refs | Klassifikation | Decision |
|---|---|---|---|---|
| `SchedulerProfile::{other, fifo, rr, deadline}` | sched(7), sched_setattr(2) | end-user-Builds; DCPS-Hot-Path-Threads die RT-Profile brauchen | OPTIONAL-HOOK (kein DCPS-Default-Wire-up) | document-as-hook |
| `SchedulerProfile::apply_to_current_thread` | sched_setattr(2) | end-user-Builds | OPTIONAL-HOOK | document-as-hook |
| `SchedulerKind` | sched(7) Diskriminanten | `SchedulerProfile` (intern), `RunningSchedulerInfo` | CONNECTED | — |
| `RunningSchedulerInfo` + `current_scheduler()` | sched_getattr(2) | Round-Trip-Test + end-user-Inspect | CONNECTED | — |
| `pin_current_thread_to_cpus` | sched_setaffinity(2) | end-user-Builds | OPTIONAL-HOOK | document-as-hook |

Ergebnis: **0 ❌-Klassen offen.** Die OPTIONAL-HOOK-Items sind End-User-API ohne intra-ZeroDDS-Production-Refs — das ist Design-konform, weil `zerodds-rt-linux` ein Caller-Side-Adapter ist, kein Wire-Path-Element.

## 4 Wiring

### 4.1 Dependencies

```toml
[target.'cfg(target_os = "linux")'.dependencies]
libc = "0.2"
```

Keine ZeroDDS-Crate-Deps. Auf Nicht-Linux-Targets baut die Crate ohne `libc`.

### 4.2 Dependents

End-User-Builds + ZeroDDS-DCPS-Hot-Path-Threads (per Caller-Wahl).

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | `std::io::Error` + Stack-Strukturen |

## 5 Spec-Relevanz

- **Linux-Kernel-API** (keine OMG-Spec):
  - `sched(7)` — SCHED_OTHER/SCHED_FIFO/SCHED_RR/SCHED_DEADLINE.
  - `sched_setattr(2)` + `sched_getattr(2)` — `sched_attr`-Struktur.
  - `sched_setaffinity(2)` + `sched_getaffinity(2)` — `cpu_set_t`.

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

```bash
rg -i -e 'llvm@llvm' -e 'sandra-kessler' -e 'fishermen21' \
  -e '/Users/sandrakessler' -e 'PDE-Spec' -e 'zero-principle' \
  crates/rt-linux/
```

Treffer: **0**.

### 6.2 Sprint-/Phase-Marker

Pre-Cleanup:
- `lib.rs:1` `Linux Real-Time-Scheduling Adapter (Phase-5 WP 5.D.3).`
- `Cargo.toml:9` `description = "Linux Real-Time-Scheduling adapter (sched_setattr / sched_setaffinity) — Phase-5 Cluster-D"`
- README.md auto-generiert mit `Phase-5 Cluster-D`-Suffix.

Post-Cleanup: **0**. lib.rs neu in Guardrails §1.2-Form (Safety-Class + Spec-Ref + Layer + Public-API). Cargo.toml `description` neu (fachlich, ohne Sprint-Marker).

### 6.3 Datums-Marker

CHANGELOG-Eintrag traegt Keep-a-Changelog-Datum.

### 6.4 Soft-Review

Keine TODO/FIXME/HACK in src/.

### 6.5 Public-API-Leaks

Keine — `syscalls` ist privat (`mod syscalls`).

### 6.6 Tech-Debt + Dead-Code

Keine.

## 7 Cleanup-Actions

1. **F-RT-LINUX-1** (resolved): Sprint-Marker `Phase-5 WP 5.D.3` + `Phase-5 Cluster-D` aus lib.rs/Cargo.toml/README entfernt. lib.rs in Guardrails §1.2-Form mit expliziter Threat-Model + Invarianten-Sektion (5 Punkte).
2. **SPDX-Header** in 4 src-Files (lib + affinity + scheduler + syscalls).
3. **Cargo.toml-Metadata**: `description` praezisiert (fachlich, kein Sprint); `homepage`/`documentation`/`readme`/`keywords`/`categories` ergaenzt; `publish = false → true`.
4. **README.md** im RC1-Format (Spec-Mapping + Quickstart + Privilegien-Hinweis + Threat-Model + Feature-Flags + Stabilitaet).
5. **CHANGELOG.md** `[1.0.0-rc.1]` Initial-Materialisierung.
6. **rustdoc-Links**: 4 unresolved-link-Warnings repariert (`syscalls` ist privat — alle Verweise auf prosa-Form `\`syscalls\`` reduziert).

## 8 Spec-Doc-Updates

Keine — Linux-Kernel-API ist die Quelle, ZeroDDS hat keinen eigenen Spec-Doc.

## 9 Doc-Artefacts

- [x] Cargo.toml-Metadata
- [x] lib.rs-Crate-Header (Safety + Spec + Layer + Public-API + Threat-Model)
- [x] README.md
- [x] CHANGELOG.md
- [x] doc-tested Code-Example (Quickstart `rust,no_run`)

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-rt-linux                     # ✅ 6 + 1 = 7 passed
cargo clippy -p zerodds-rt-linux --tests -- -D warnings  # ✅
cargo fmt -p zerodds-rt-linux -- --check           # ✅
cargo doc -p zerodds-rt-linux --no-deps            # ✅ (post-Fix: 0 Warnings)
cargo run --bin zerodds-lint -- check              # ✅ workspace clean
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README.md
- [x] §1.4 CHANGELOG.md
- [x] §1.5 Public-API-Audit
- [x] §1.5b Coherence-Audit
- [x] §1.6 Spec-Coverage-Update (kein Update — Linux-Kernel-API)
- [x] §1.7 Forbidden-Token-Sweep
- [x] §1.8 License-Header (4 src-Files)
- [x] §1.9 Tests + Lints + Doc-Build gruen
- [x] §1.10 Review-Doc
- [x] §1.11 Tracker auf ✅
- [x] §1.12 Public-Mirror-Artifacts
- [x] §1.13 Spec-Conformance-Audit (F-RT-LINUX-1 ✅ resolved)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer-Sign-off:** Claude
- **Tracker-Eintrag aktualisiert:** ✅
