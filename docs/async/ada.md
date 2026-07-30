<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS async — Ada (native endpoint)

Ada has two conservative endpoint variants already (Stage 1 `endpoints/ada`,
bindings over the C wire-core; Stage 2 `endpoints/ada-native`, pure Ada,
procedural). The **modern, additive** async add-on is **Object-Ada**: genuine
object orientation — interfaces, tagged types, dispatching — for teams on an
Ada 2012+ toolchain. Object-Ada DDS endpoints are a defense differentiator
almost nobody offers.

Package: [`Zerodds_Native_Endpoint`](../../endpoints/ada-native/src/zerodds_native_endpoint.ads)

## Model (OOP)

The single integration point is a dispatching `Transport` interface; samples
are delivered to a dispatching `Sample_Handler`:

```ada
type My_Transport is limited new Transport with record ... end record;
overriding procedure Deliver (T : in out My_Transport; Frame : Byte_Array; Ok : out Boolean);
overriding procedure Receive (T : in out My_Transport; Buf : out Byte_Array;
                              Last : out Natural; Status : out Recv_Status);

type My_Handler is limited new Sample_Handler with record ... end record;
overriding procedure On_Sample (H : in out My_Handler; Sample_Body : Byte_Array);

--  event-driven reader dispatches through the tagged-type machinery
Reader : Async_Reader (Transport'Access, Handler'Access);
Run (Reader, Count);        --  drain: Receive -> Xrce body -> On_Sample

--  fire-and-forget writer
Writer : Async_Writer (Transport'Access);
Write (Writer, Sample, Ok); --  frames as XRCE WRITE_DATA
```

No threads, no heap — drive `Poll` / `Run` from any scheduler.

## Tests

`make -C endpoints/ada-native test` (in CI via `endpoints-native`):

- `test_oop_endpoint` — a concrete FIFO `Transport` + `Collector`
  `Sample_Handler`; the writer fires N samples, the reader dispatches + decodes
  them in order through the interfaces.

## Variant status

| Variant | Kind | Status |
|---------|------|--------|
| Stage 1 (`endpoints/ada`) | Interfaces.C bindings | ✅ byte-identity + live UDP |
| Stage 2 (`endpoints/ada-native`) | pure Ada, procedural | ✅ final/appendable/mutable/reflective/framing |
| **Object-Ada async** | pure Ada, OOP (tagged) | ✅ event-driven reactor (this page) |
| pre-Object Ada 83 | pure Ada, procedural, Ada-83 subset | planned |
