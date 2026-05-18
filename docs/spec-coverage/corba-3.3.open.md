# OMG CORBA 3.3 — Open + Partial Items

Aggregat aus `corba-3.3.md`. Nicht von Hand pflegen — vor jedem
Audit-Lauf löschen und aus dem Hauptfile neu generieren.

## Open

— keine.

## Partial

— keine.

## Decision-Records (`n/a (rejected)`)

— keine.

Im Layer-8 Wire-up-Cleanup 2026-05-06 wurden alle ehemals-rejected
Items (§16 Portable Interceptors, §17 CORBA Messaging, Part 2 §11
MIOP) auf `done` reklassifiziert via Wire-up gegen vorhandene
Infrastruktur:

* §16 — `corba-iiop::Connection::with_interceptors`
  + `InterceptorRegistry::walk_client/walk_server/walk_ior` werden in
  `read_message`/`write_message` automatisch aufgerufen.
* §17 — `dispatch_async_reply(&AmiReplySink, &corba_giop::Reply)`
  brueckt GIOP-Reply-Status auf die drei Spec-Callbacks; TII via
  `PersistentRequestStore { add, poll, timeout_expired }`; alle 10
  Messaging-Policies haben `policy_type() -> u32` nach OMG
  `Messaging.idl` §B.5.1.
* Part 2 §11 — `MiopFrameHeader::{encode, decode}` (16-Byte-Header)
  + `MiopSender::send_giop` mit Single-/Multi-Packet-Fragmentierung;
  Multicast-Sink wird via `MulticastSink`-Adapter-Trait injiziert,
  damit `corba-ccm` keinen `transport-udp`-Layer-Zyklus zieht.

Siehe Audit-Status-Footer in `corba-3.3.md` fuer Test-Counts.
