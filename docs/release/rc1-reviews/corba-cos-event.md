# RC1 Review — `zerodds-corba-cos-event`

> **Layer:** 8 (CORBA-Stack, Tier-A) | **Reviewer:** claude | **Public-Strategy:** 🌐 public
> **Status:** ✅ rc1-ready (`EventChannel` + `PushConsumer` jetzt CONNECTED via `corba-ccm::cos_event_bridge` unter Feature `cos-event`; F-CORBA-COS-EVENT-NOT-WIRED ✅ resolved)

## 1 Purpose

OMG CosEventService v1.2 (`formal/04-10-02`) voller Stack: Push/Pull-Modell (§1.5) + EventChannelAdmin (§1.6) + TypedEvent (§2). `no_std + alloc`.

## 2-3 Inhalt

- 4 src-Files (channel, comm, lib, typed).
- 0 tests-Files (Tests inline).
- **24 Tests grün** (23 unit + 1 doc).

## 3.4 Coherence-Audit (Cross-Crate × Spec)

**Verifizierung:** `rg -l 'zerodds_corba_cos_event' --type rust crates/ -g '!crates/corba-cos-event/**'` → **`crates/corba-ccm/src/cos_event_bridge.rs`** (4 Production-Refs auf `AnyEvent` / `PushConsumer` / `EventChannel` unter `cos-event`-Feature).

| Item-Familie | Spec-Anker | External Production-Refs | Klassifikation |
|---|---|---|---|
| `AnyEvent` / `Disconnected` / `ConnectError` | §1.5 CosEventComm | **2** in corba-ccm (cos_event_bridge.rs) | **CONNECTED** ✅ |
| `PushConsumer` (Trait) | §1.5.1 | **2** in corba-ccm (Adapter-Field + Test-Mock-impl) | **CONNECTED** ✅ |
| `PushSupplier` / `PullConsumer` / `PullSupplier` (Trait-Surfaces) | §1.5 | 0 | OPTIONAL-HOOK (Spec-MAY-Endpoints; Pull-Modus Plugin-API) |
| `EventChannel` / `ConsumerAdmin` / `SupplierAdmin` / Proxies | §1.6 CosEventChannelAdmin | **1** in corba-ccm (Test-Setup) + Self-Tests | **CONNECTED** ✅ |
| `TypedEventChannel` / `TypedPushConsumer` / `TypedPushSupplier` | §2 CosTypedEventComm | 0 | OPTIONAL-HOOK (typed-event ist Spec-MAY; Untyped-Pfad ist Spec-MUST und voll connected) |

**Wire-up:** `corba-ccm::cos_event_bridge::EventChannelTimerCallback` adaptiert `TimerEventService::TimerCallback` an `CosEventComm::PushConsumer` — direkt Spec-§2.2.4 OMG Time Service 1.1: "TimerEventHandler is implemented as a CosEventComm::PushConsumer". Feature-gated unter `corba-ccm/cos-event`, damit Caller die Dep nur bei Bedarf ziehen.

3 Test-Mock-Stubs in `comm.rs:100`, `channel.rs:343`, `typed.rs:296` sind alle in `#[cfg(test)] mod tests`-Bloecken — keine Production-Stubs, sondern Test-Mock-Pattern.

**Finding:** `F-CORBA-COS-EVENT-NOT-WIRED` ✅ resolved.

## 6 Cleanup

- **Forbidden:** 0.
- **Sprint-Marker:** 0 (Crate war pre-Review pristine).
- **TODO/FIXME/Stub:** 0 (3 unused-args sind Test-Mocks im `#[cfg(test)]`).

## 7 Cleanup-Actions

1. Cargo.toml: publish=true + Metadata komplett.
2. lib.rs: SPDX + RC1-Header mit Public-API-Liste + Doc-Test.
3. SPDX auf alle 4 src-Files.
4. README + CHANGELOG.
5. Mirror unter `github/crates/corba-cos-event/`.
6. `website/docs/corba-cos-event.md`.
7. `github/Cargo.toml` + CHANGELOG.md ergaenzt.

## §1.13 Drei-Punkte-Kohärenz

- **(a) Wire + Semantik kohärent:** Spec §1.5 + §1.6 + §2 voll abgebildet (Push/Pull-Modell, EventChannelAdmin, Typed-Variant). Disconnect-Lifecycle pro Spec.
- **(b) Wire-up mit allen Modulen:** ✅ — `corba-ccm::cos_event_bridge::EventChannelTimerCallback` (Feature `cos-event`) bindet `TimerEventService` an `EventChannel` per OMG Time-Service §2.2.4. Pull-Modus + Typed-Variant sind Spec-MAY (OPTIONAL-HOOK explizit dokumentiert).
- **(c) Getestet:** 23 Unit-Tests + 1 Doc-Test. Test-Mocks fuer alle vier `Push/Pull*`-Trait-Surfaces (Spec §1.5) verifizieren Trait-Aufrufbarkeit. Plus 1 Cross-Crate-Test in corba-ccm: `cos_event_bridge::tests::one_shot_timer_pushes_event_to_channel` verifiziert end-to-end Timer→Channel→Consumer-Push.

## 10-12 Gates

- `cargo test`: ✅ 24 (23 unit + 1 doc).
- `cargo clippy --tests -- -D warnings`: ✅.
- `cargo fmt --check`: ✅.
- `cargo doc --no-deps`: ✅.
- `cargo run --bin zerodds-lint -- check` (workspace-weit): ⚠️ 21 errors in `zerodds-c-api/src/factory_ffi.rs` (nicht von dieser Crate verursacht).

## RC1-DoD-Status

- §1.1 Cargo.toml ✅
- §1.2 lib.rs Crate-Header ✅ (mit Doc-Test)
- §1.3 README ✅
- §1.4 CHANGELOG ✅
- §1.5b Coherence-Audit: ✅ (PushConsumer + EventChannel CONNECTED via corba-ccm/cos-event-Feature; Pull/Typed sind Spec-MAY-OPTIONAL-HOOKS; Finding F-CORBA-COS-EVENT-NOT-WIRED ✅ resolved)
- §1.6 Spec-Coverage: ✅ (`cos-event-service-1.4.md` alle Sektionen done)
- §1.7 Forbidden-Sweep ✅
- §1.8 License-Header ✅
- §1.9 Tests/Lints/Doc ✅; zerodds-lint workspace ⚠️ (Reibach aus `zerodds-c-api`)
- §1.10 Review-Doc ✅
- §1.12 Public-Mirror ✅ (spec-coverage-Mirror nach `website/spec-coverage/` durch Parallel-Agent)
- §1.13 Inline-Deferral-Sweep ✅; Drei-Punkte-Liste ✅; Wire-up-Status: ✅ resolved.

**Crate-Version:** `1.0.0-rc.1` | **Status:** ✅ rc1-ready | **Sign-off:** claude
