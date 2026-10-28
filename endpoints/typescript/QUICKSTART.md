<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS TypeScript XRCE client — Quickstart

A typed port of `crates/xrce-client` (the DDS-XRCE 1.0 session state machine)
combined with `crates/xrce`'s submessage codecs (CREATE_CLIENT / CREATE /
DELETE / WRITE_DATA / READ_DATA) — so a program in TypeScript on Node can
talk to a ZeroDDS instance without going through the C API.

```sh
cd endpoints/typescript
npm install    # fetches @types/node + typescript, no runtime dependency
npm test       # tsc && node --test dist/xrce_client.test.js
```

Prints 12 passing checks.

## What this is, and isn't

`xrce_client.ts` is the session state machine (`Disconnected -> Connecting ->
Connected`, the same request-id counter crates/xrce-client uses) plus the
wire framing — a program still owns its own transport (`deliver`/`receive`)
and hands this module's `frame*` output to it.

It is a standalone port: there is no pre-existing TypeScript endpoint SDK in
this repository to extend (`endpoints/node` is the sibling JavaScript SDK,
with its own, older WRITE_DATA framing for the simplified "ZeroDDS Endpoint
Profile" — a different, non-conformant wire shape from the one here).

## Representation timestamps

Option `representationTimestamps` with the values `'absent'` (default) and
`'present'`. DDS-XRCE 1.0 contradicts itself on `CLIENT_Representation`: the
prose of section 7.8.2.1 lists `Time_t client_timestamp` after
`xrce_vendor_id`; the IDL of Annex A and the bytes of Annex B.1 do not have it.
`'absent'` writes the Annex A form (what the SDK wrote before). `'present'`
writes the prose form: seconds `i32` and nanoseconds `u32`, little endian.
`Time_t` has no `@extensibility`, so it is APPENDABLE (DDS-XTypes 1.3 section
7.3.1.2.1.8) with `unannotatedTypes: 'appendable'` (default): DHEADER = 8, 12
octets. With `unannotatedTypes: 'final'` there is no DHEADER, 8 octets. This SDK
has no reliable module, so `unannotatedTypes` exists on the XRCE client and the
encoder only.

The agent has to be set the same way (`representation_timestamps: absent |
present` in its configuration).

```ts
const client = new XrceClient(key, { representationTimestamps: 'present' });
client.connect(1);
const rep = client.clientRepresentation();      // Time_t = wall clock at the call
frameCreateClient(1, key, rep);
encodeClientRepresentation([1, 0], [0, 0], key, 1, 508,
  { representationTimestamps: 'present', seconds: 1518905996, nanoseconds: 500000000 });
```

Bytes (client_key 01 02 03 04, session_id 01, version 01 00, vendor 00 00,
requested_mtu 508, seconds 1518905996, nanoseconds 500000000):

    absent (16):              58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
    present, appendable (28): 58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
    present, final (24):      58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01

The SDK does not read `AGENT_Representation`; `STATUS_AGENT` is a state change
only.

## Wire

Byte-identical to `endpoints/c`'s `zdwc_frame_*` and checked against the
same fixtures, `endpoints/testdata/xrce-client/*.bin`: every frame carries
the 4-octet ClientKey, and `request_id` is encoded big-endian (2 octets),
matching `crates/xrce-client::request_id_octets` and `ObjectId`'s own wire
order.
