# ZeroDDS Delivery Modes 1.0

> Scope: user-selectable sample delivery format/transport per writer.
> Vendor-specific (no OMG normative equivalent).
> Umbrella: see `zerodds-zero-copy-1.0.md` for the copy inventory and
> `zerodds-flatdata-1.0.md` for the slot/`FlatStruct` layout this builds on.

## 1 Purpose

A sample can travel from a writer to a matched reader in more than one
physical form. The portable form is interoperable but always pays a
serialization step; the raw same-host form is fastest but only an identical
peer can read it. ZeroDDS does **not** hard-code one choice. This spec defines
the set of delivery modes, how the application selects one, how the runtime
negotiates the actual per-reader delivery, and the interoperability guarantees
of each mode.

The decision is the **application's**, exposed as configuration. The runtime's
job is to honour the configured mode where possible, negotiate per matched
reader, and never silently break interoperability or lose a sample.

## 2 Three orthogonal properties

The confusion this spec removes: "format", "reach" and "interop" are three
different things, not one.

| Property | Question it answers |
|---|---|
| **Format** | What bytes carry the sample — portable serialized form, or the in-memory layout? |
| **Reach** | Can it cross machine boundaries, or same-host only? |
| **Interop** | Who can read it — any DDS vendor, only ZeroDDS, or iceoryx-based peers? |

A mode is a fixed combination of these three. Choosing "raw" does **not** by
itself imply iceoryx compatibility: raw bytes in ZeroDDS's own shared-memory
segment are readable only by another ZeroDDS process with the same type layout.
iceoryx is a separate shared-memory system with its own conventions; talking to
it is its own mode.

## 3 The delivery modes

### 3.1 Mode `Portable` (default)

- **Format:** serialized portable payload (the standard wire encoding +
  encapsulation header), identical to `zerodds_dw_write`.
- **Reach:** cross-host and same-host.
- **Interop:** any DDS vendor, any host.
- **Notes:** the default and the only interop-safe-everywhere mode. Same-host
  delivery MAY be accelerated transparently (§5) without changing the format,
  so a portable writer still avoids the local network round-trip when both ends
  are on one machine — the bytes on both paths are byte-for-byte the portable
  form.
- **Requirements:** none. Always available; the universal fallback.

### 3.2 Mode `RawSameHost` (ZeroDDS-native zero-serialize)

- **Format:** the in-memory typed payload (`T: FlatStruct`, `#[repr(C)]` POD),
  written directly into a ZeroDDS POSIX shared-memory slot — no serialization.
- **Reach:** same-host only.
- **Interop:** ZeroDDS ↔ ZeroDDS only, and only when both ends agree on the
  exact type layout (gated by the `FlatStruct` `TYPE_HASH`).
- **Notes:** the only true zero-serialize path. A type-layout or host mismatch
  is **not** transparently bridged to portable — the producer wrote a struct,
  not a serialized payload, so it cannot be reframed for the wire. A
  cross-host or mismatched reader therefore is simply **not served** by a
  `RawSameHost` writer (it does not match, or matches only its portable
  companion endpoint if one is also offered — see §6).
- **Requirements:** same host, matching `TYPE_HASH`, `FlatStruct`-typed sample.

### 3.3 Mode `Iceoryx` (cross-stack same-host)

- **Format:** the in-memory typed payload, placed into an
  [iceoryx2](https://github.com/eclipse-iceoryx/iceoryx2) sample.
- **Reach:** same-host only.
- **Interop:** with iceoryx2-based peers on the same host (e.g. other ROS-2
  zero-copy stacks) **and** ZeroDDS, provided both agree on the iceoryx2
  service name and the payload layout.
- **Notes:** uses the existing flatdata iceoryx2 bridge
  (`crates/flatdata/src/iceoryx.rs`, feature `iceoryx2-bridge`). Distinct from
  `RawSameHost`: same raw payload form, but delivered through iceoryx2's
  segments/discovery rather than ZeroDDS's own — that is what makes it readable
  by non-ZeroDDS iceoryx peers.
- **Requirements:** same host, iceoryx2 runtime present, agreed service name +
  payload layout.

### 3.4 Mode matrix

| Mode | Format | Reach | Reads it | Serialize? |
|---|---|---|---|---|
| `Portable` | portable serialized | cross- + same-host | any DDS vendor | yes |
| `RawSameHost` | in-memory struct | same-host | ZeroDDS, same layout | no |
| `Iceoryx` | in-memory struct | same-host | iceoryx2 peers + ZeroDDS | no |

## 4 Configuration (application-facing)

Delivery mode is a per-writer property with a participant-level default:

- **Participant default:** environment variable `ZERODDS_DELIVERY_MODE`
  (`portable` | `raw-same-host` | `iceoryx`), default `portable`.
- **Per-writer override (implemented):** the C-API setter
  `zerodds_dw_set_delivery_mode(dw, mode)` and the runtime-path equivalent
  `zerodds_writer_set_delivery_mode(writer, mode)`, with `mode` ∈
  {`0`=Portable, `1`=RawSameHost, `2`=Iceoryx}. `Iceoryx` is accepted when the
  crate is built with the `delivery-iceoryx` feature (else `Unsupported`); the
  iceoryx service is bound separately via `zerodds_dw_enable_iceoryx` /
  `zerodds_reader_enable_iceoryx` (and the runtime-path twins).
- **Reader side (deferred):** a `zerodds_dr_set_delivery_mode` /
  `zerodds_reader_set_delivery_mode` declaring which modes the reader accepts
  belongs to the negotiation step (§5) and is not added until then (it would be
  a no-op today).
- **Granularity:** per topic via the writer created on it. A camera topic may
  run `raw-same-host` while control commands stay `portable`.

The default MUST be `Portable` so no deployment loses interoperability without
an explicit opt-in.

## 5 Per-reader negotiation (runtime)

A writer can be matched with several readers at once — some local, some
remote, some ZeroDDS, some foreign. The runtime selects the delivery per
matched reader; the configured mode is the **preferred** mode, not a global
verdict.

1. **Capability advertisement.** Each reader advertises the set of modes it
   accepts (its configured modes plus `Portable`, which every reader accepts)
   and its host identity, in discovery (a vendor-specific endpoint property).
2. **Selection per matched reader**, writer side:
   - If the reader is the same host AND accepts the writer's preferred raw mode
     (`RawSameHost` / `Iceoryx`) AND the type layout matches → deliver via that
     mode.
   - Else → deliver `Portable` (over the same-host SHM transport if both are
     same-host and that path is available, else over the network).
3. **No double delivery.** When a reader is served via a raw/iceoryx mode, the
   writer MUST suppress that same reader's network (UDP) locator for the
   sample, so the one reader entity receives the sample exactly once. Readers
   served `Portable` are unaffected.
4. **Fallback.** If a raw/iceoryx segment cannot be established (race,
   permissions, missing runtime), the writer falls back to `Portable` for that
   reader so no sample is lost — **except** that a `RawSameHost`/`Iceoryx`
   writer is under no obligation to serve a reader that cannot do raw at all
   (see §6).

## 6 Interop & safety rules

- `Portable` is always interop-safe and is the default. A deployment only loses
  cross-host or cross-vendor reach by **explicitly** selecting a raw mode.
- A `RawSameHost` / `Iceoryx` writer that also wants remote reachability MUST
  additionally offer `Portable` (the runtime then serves local raw readers via
  SHM/iceoryx and remote readers via the portable path — see §5.2). A writer
  configured raw-only is, by the user's explicit choice, same-host-only.
- The raw modes never put in-memory bytes onto the network framed as the
  portable form. The byte-oriented loan buffer of a `Portable` writer carries
  the serialized payload; the loan buffer of a raw writer carries the struct —
  these are not interchangeable and the runtime never reframes one as the
  other.
- Type-layout agreement for raw modes is enforced by the `FlatStruct`
  `TYPE_HASH`; a mismatch does not match (no garbage read).

## 7 C-API / RMW contract per mode

The loan API (`loan_message` / `commit_loan` / `discard_loan`, both the DCPS
`zerodds_dw_*` and runtime `zerodds_writer_*` surfaces) is the entry point. The
mode determines what the caller writes into the loaned buffer:

| Mode | Loan buffer holds | Writer step | Reader take |
|---|---|---|---|
| `Portable` | serialized payload (CDR body) | `commit` finalizes slot + delivers portable (SHM same-host / network remote) | normal take (CDR→struct) |
| `RawSameHost` | in-memory struct (`FlatStruct`) | `commit` publishes the slot; no serialization | `take_shm` (struct, no decode) |
| `Iceoryx` | in-memory struct | `commit` sends the iceoryx2 sample | iceoryx2 receive (struct) |

For ROS-2 (rclcpp owns the loan contract and hands the user a typed
`MessageT*`):

- `Portable`: rmw serializes struct→portable into the loan buffer at publish
  (current behaviour), gains same-host network-loopback elimination via §5.
- `RawSameHost` / `Iceoryx`: rmw places the struct directly; the matched
  same-host reader takes it without a decode.

## 8 Building blocks & work split

Existing components this composes (status to be tracked in the respective
coverage audits, not asserted here):

- `crates/flatdata` — POSIX slot allocator + `FlatStruct` + iceoryx2 bridge;
  the writer/reader zero-copy slot primitives.
- `crates/zerodds-c-api` `shm_loan_ffi` — `(runtime, eid)`-keyed loan registry
  + `enable_shm_loan` / `take_shm` on both FFI surfaces (`Portable`-CDR and
  `RawSameHost` byte transport, today).
- `crates/transport-shm` + the `dcps` same-host path — same-host SHM transport
  for the portable form (the §5.2 `Portable`-over-SHM acceleration).

Responsibilities:

- **Runtime / C-API (this agent):** delivery-mode config surface (§4),
  per-reader negotiation incl. capability advertisement and no-double-delivery
  suppression (§5), wiring `RawSameHost` and `Iceoryx` commit/take onto the
  flatdata + iceoryx2 backends.
- **RMW (ROS-2 agent):** map rclcpp's loan to the selected mode (§7), reader
  take path (`take_shm` / iceoryx2 receive for raw modes), and the rclcpp-side
  type-support that yields the struct layout.
- **flatdata:** already provides the slot + iceoryx2 primitives; extend only if
  a backend gap surfaces.

## 9 Acceptance

- A `Portable` writer interoperates with a foreign-vendor reader cross-host
  (existing cross-vendor suite) and, same-host, delivers without the network
  loopback while remaining byte-identical on the wire.
- A `RawSameHost` writer + reader on one host exchange a `FlatStruct` sample
  with zero serialization; a layout mismatch does not match; a cross-host
  reader is not served (unless the writer also offers `Portable`).
- An `Iceoryx` writer is read by an iceoryx2-based peer on the same host.
- A writer matched by both a local raw reader and a remote portable reader
  serves each correctly and delivers the sample to the local reader exactly
  once (no double delivery).

## 10 Cross-references

- `zerodds-zero-copy-1.0.md` — copy inventory + reduction architecture
  (umbrella).
- `zerodds-flatdata-1.0.md` — `FlatStruct`, slot layout, iceoryx2 bridge.
- `zerodds-shm-transport-1.0.md` — same-host SHM transport (portable form).
- `zerodds-c-api-1.0.md` §2.6 — loan API signatures.
- DDSI-RTPS 2.5 §9.4 — `LOCATOR_KIND_SHM`.

## 11 Status & open items

- [x] `Portable` loan with same-host SHM byte transport + `(runtime, eid)`
      keying on both FFI surfaces — implemented (`shm_loan_ffi`).
- [x] Delivery-mode config: `ZERODDS_DELIVERY_MODE` env default (`portable` |
      `raw-same-host`) + per-writer setters `zerodds_dw_set_delivery_mode` /
      `zerodds_writer_set_delivery_mode` (`Iceoryx` → `Unsupported` until §3.3
      is wired) — implemented.
- [x] `RawSameHost` commit behaviour: commit finalizes the slot and does **not**
      publish over RTPS (same-host only, no wire, no double delivery); the
      same-host reader takes via `take_shm` — implemented and tested
      (`shm_loan_e2e::raw_same_host_mode_writer_to_reader`). `TYPE_HASH` layout
      gating lives at the typed `FlatStruct`/RMW layer, not the byte FFI.
- [ ] Reader-side delivery-mode setter (`zerodds_dr_set_delivery_mode` /
      `zerodds_reader_set_delivery_mode`) — deferred until negotiation exists
      (no effect without it; not added as a no-op).
- [x] Per-reader same-host selection + no-double-delivery for the `Portable`
      form — already provided by the `same-host-shm` path (the
      `SameHostTracker` + `same_host_udp_skip_set` + `same_host_send_pass` in
      `crates/dcps/src/runtime.rs`, with `crates/dcps/src/same_host*.rs`). A
      same-host reader receives the portable form over the SHM transport and
      its UDP locator is suppressed (no duplicate); remote readers get UDP.
      The loan commit path (`write_user_sample_borrowed`) goes through exactly
      this, so a `Portable` loan writer inherits it. E2E:
      `crates/dcps/tests/same_host_e2e.rs`.
- [ ] Mixed-form delivery from **one** writer (a same-host reader getting the
      zero-serialize raw form while a remote reader gets the portable form) is
      **not** achievable at the byte-oriented FFI: one loan buffer holds one
      form and the runtime cannot reframe a struct into the portable form
      without type info. This belongs to the typed `FlatStruct`/RMW layer, not
      this byte-FFI spec.
- [ ] Capability advertisement in discovery (explicit reader opt-out /
      cross-stack signal) — not implemented; same-host ZeroDDS↔ZeroDDS uses
      host-id inference (`GuidPrefix` host-id), which suffices today.
- [x] `Iceoryx` commit/take wired onto the flatdata iceoryx2 bridge on the
      C-API path (feature `delivery-iceoryx`, off by default): byte-oriented
      `RawIceoryx2Publisher`/`RawIceoryx2Subscriber` in `crates/flatdata`
      (thread-safe `ipc_threadsafe` service) + `zerodds_dw_enable_iceoryx` /
      `zerodds_reader_enable_iceoryx` (and DCPS/runtime twins). `commit` sends
      over iceoryx2 (no RTPS); the reader receives via `take_shm`. Tests:
      `flatdata::iceoryx::tests::raw_byte_publisher_subscriber_roundtrip`,
      `shm_loan_e2e::iceoryx_mode_writer_to_reader`. Refinement: the writer
      loan buffer is a heap buffer copied into the iceoryx slot at commit (one
      copy at the boundary); end-to-end zero-copy into the iceoryx slot would
      hold the iceoryx loan across the FFI loan/commit calls.
- [ ] RMW-side use of the `Iceoryx` mode (rclcpp loan → `enable_iceoryx` +
      reader receive) — ROS-2 agent's part (spec §7/§8).
