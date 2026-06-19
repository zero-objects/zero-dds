# `zerodds-corba-rt` v1.0 — Real-Time CORBA in Rust

ZeroDDS Vendor-Spec. In `crates/corba-rt` implementiert. Authored im Stil der
OMG-CORBA-Spec (nummerierte Klauseln, RFC-2119-Keywords, Konformitätsprofil). Das
**Priority-Propagation-Wire** (`RTCorbaPriority`-ServiceContext) ist OMG-normativ
(OMG Real-Time CORBA 1.0); diese Spec normiert das **ZeroDDS-eigene Rust-PSM** der
RT-Policies — die OMG hat kein Rust-Language-Mapping standardisiert.

## Motivation

**Real-Time CORBA** bringt deterministisches Echtzeit-Verhalten ins CORBA-Modell:
end-to-end-Prioritäts-Propagation, prioritäts-bewusste Thread-Pools, prioritäts-
gebänderte Connections. Es ist TAOs Flaggschiff und in Avionik/Verteidigungs-
Bestand verbreitet. `zerodds-corba-rt` liefert das RT-Policy-Modell + die
Prioritäts-Propagation in `no_std + alloc`, `forbid(unsafe_code)` — komplementär
zur DDS-seitigen Echtzeit-QoS (Deadline/Latency-Budget/Transport-Priority).

## Ziele

- **CORBA-Priorität** (`0..=32767`) + **PriorityMapping** ↔ native OS-Priorität.
- **PriorityModel** `SERVER_DECLARED` / `CLIENT_PROPAGATED` + `PriorityModelPolicy`.
- **`RTCorbaPriority`-ServiceContext** (id = 10) — Propagation der Client-Priorität
  über GIOP, byte-exakt CDR.
- **Threadpool/Lane** + **PriorityBand** mit prioritäts-basierter Auswahl.
- **`RTCORBA::Current`** — CORBA-Priorität des aktuellen Kontexts.

## Nicht-Ziele

- **Native Thread-Spawning + OS-Scheduler-Bindung** — v1.0 modelliert die
  Threadpool-/Lane-**Struktur + Auswahl**; das tatsächliche Spawnen und
  `pthread_setschedparam` ist Sache der Laufzeit-Integration.
- **Priority-Inheritance-Mutexe** (OMG RTCORBA §5.6 `Mutex`) — Folge-Erweiterung.
- **Static Scheduling Service** (RT-CORBA-Scheduling-Profil) — Nicht-Ziel v1.0.

## §1 Priorität + Mapping

### §1.1 `Priority`

Eine CORBA-Priorität ist ein `short` im Bereich `0..=32767` (RT-CORBA §5.3);
höher = dringlicher. `Priority::new(v)` MUSS Werte außerhalb ablehnen;
`Priority::clamped(v)` klemmt.

### §1.2 `PriorityMapping`

```rust
pub trait PriorityMapping {
    fn to_native(&self, corba: Priority) -> Option<i32>;
    fn to_corba(&self, native: i32) -> Option<Priority>;
}
```

Die Default-`LinearPriorityMapping::new(native_min, native_max)` skaliert linear
auf ein natives Fenster (z.B. POSIX-`SCHED_FIFO` `1..99`). `to_corba` MUSS native
Werte außerhalb des Fensters mit `None` ablehnen.

## §2 Priority-Model

### §2.1 `PriorityModelPolicy`

```rust
pub enum PriorityModel { ServerDeclared, ClientPropagated }
pub struct PriorityModelPolicy { pub model: PriorityModel, pub server_priority: Priority }
```

`effective_priority(propagated)` MUSS: bei `ClientPropagated` die propagierte
Client-Priorität (Fallback `server_priority`), bei `ServerDeclared` immer
`server_priority` liefern. Die Policy wird im IOR als `TAG_RT_CORBA_PRIORITY_MODEL`
annonciert.

### §2.2 `RTCorbaPriority`-ServiceContext

Bei `CLIENT_PROPAGATED` legt der Client seine CORBA-Priorität als IOP-
`ServiceContext` mit der Id `RT_CORBA_PRIORITY_SC_ID` (= 10, RT-CORBA §5.4.2) bei
— eine CDR-Encapsulation (Byte-Order-Octet + `short`).

**Byte-Konformität (normativ).** Priorität `1337` Big-Endian MUSS zu

```
00 000539
```

encodieren (BO-Octet 0, Pad, `short` 0x0539). `encode_priority_context` /
`decode_priority_context` bilden das ab.

## §3 Threadpools + Bänder

### §3.1 Threadpool/Lane

```rust
pub struct Lane { pub priority: Priority, pub static_threads: u32, pub dynamic_threads: u32 }
pub struct Threadpool { pub lanes: Vec<Lane>, pub stacksize: usize,
                        pub allow_request_buffering: bool, pub max_buffered_requests: u32 }
```

`Threadpool::lane_for(priority)` MUSS die Lane mit der **höchsten Priorität ≤
`priority`** wählen (die anspruchsvollste, die den Request noch abdeckt);
existiert keine, die niedrigste Lane.

### §3.2 PriorityBand

```rust
pub struct PriorityBand { pub low: Priority, pub high: Priority }
pub struct PriorityBandedConnectionPolicy { pub bands: Vec<PriorityBand> }
```

`band_for(priority)` MUSS den Index des Bandes liefern, das `priority` abdeckt —
die Connection-Auswahl (verhindert Priority-Inversion auf gemeinsamer Connection).

## §4 `RTCORBA::Current`

`RtCurrent::get_priority`/`set_priority` (RT-CORBA §5.5) lesen/setzen die
CORBA-Priorität des aktuellen Kontexts. In einer Laufzeit-Integration ändert
`set_priority` die native Thread-Priorität über das aktive `PriorityMapping`.

## §5 Konformität

Ein **RT-CORBA-konformes** ZeroDDS-Modul:

1. validiert/mappt Prioritäten gemäß §1,
2. liefert die `effective_priority`-Semantik gemäß §2.1 für beide Modelle,
3. encodiert den `RTCorbaPriority`-ServiceContext byte-exakt gemäß §2.2 (id = 10),
4. wählt Lane/Band gemäß §3,
5. liefert `RTCORBA::Current` gemäß §4.

## §6 Implementierungs-Mapping

| Spec | Code |
|---|---|
| §1 Priorität + Mapping | `corba-rt/src/priority.rs` — `Priority`, `LinearPriorityMapping` |
| §2 Priority-Model | `corba-rt/src/policy.rs` — `PriorityModel`, `PriorityModelPolicy` |
| §2.2 SC-Propagation | `corba-rt/src/propagation.rs` — `RT_CORBA_PRIORITY_SC_ID`, `encode/decode_priority_context` |
| §3 Threadpool/Bänder | `corba-rt/src/policy.rs` — `Lane`, `Threadpool`, `PriorityBand`, `PriorityBandedConnectionPolicy` |
| §4 Current | `corba-rt/src/current.rs` — `RtCurrent` |

## §7 Tests

- Unit (12): Prioritäts-Range + Linear-Mapping-Endpunkte + Native-Window; beide
  Priority-Modelle (`effective_priority`); Lane-Auswahl + Kapazität; Band-Auswahl;
  `RTCorbaPriority`-SC Roundtrip BE+LE + Byte-Exact-Golden; `RtCurrent` get/set.

## Annex A — Verhältnis zur DDS-Echtzeit-Seite

RT-CORBA und die DDS-QoS-Echtzeit-Seite sind komplementär: RT-CORBA propagiert
CORBA-Prioritäten end-to-end über GIOP, DDS deckt Deadline/Latency-Budget/
Transport-Priority auf der Pub/Sub-Seite. Eine CCM-Komponente mit GIOP-Ports und
DDS-Topic-Ports kann beide Modelle gleichzeitig nutzen.
