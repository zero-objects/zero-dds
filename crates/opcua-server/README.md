# zerodds-opcua-server

Native OPC-UA Client/Server (OPC Foundation **Part 4 — Services**) over the
SecureChannel/UACP transport ([`zerodds-opcua-uacp`](../opcua-uacp)).

The request/response counterpart to the native UADP PubSub stack
([`zerodds-opcua-pubsub`](../opcua-pubsub)). Pure-Rust `no_std + alloc`,
`forbid(unsafe_code)`; TCP endpoints behind the `std` feature.

## Contents

- `services` — the full service set: Session lifecycle (CreateSession /
  ActivateSession / CloseSession), Attribute (Read / Write), Method (Call),
  View (Browse), Discovery (GetEndpoints / FindServers) and Subscription /
  MonitoredItem (CreateSubscription / CreateMonitoredItems / Publish /
  SetPublishingMode / DeleteSubscriptions), with the `ServiceRequest` /
  `ServiceResponse` dispatch enums.
- `address_space` — an in-memory AddressSpace: node `Value` attributes, method
  handlers, and a browsable node/reference graph (`NodeMeta` / `ReferenceRecord`
  / `NodeClass` + the ns0 `reference_types`).
- `server` — a request-driven `Server` (Hello → OpenSecureChannel → Session →
  service dispatch). Subscriptions use sample-on-publish. Set a `ServerSecurity`
  (feature `crypto`) for the secured handshake.
- `client` — a `Client` plus `LoopbackTransport` (in-process) and
  `TcpTransport` (`opc.tcp`); `open_channel` for session-less Discovery, or
  `connect` for the full session. Set a `ClientSecurity` for a secured channel.

## Security

SecurityMode `None` works out of the box (in-process and over `opc.tcp`). The
secured SecurityPolicies (`crypto` feature → `zerodds-opcua-uacp/crypto`:
`Basic256Sha256`, `Aes128_Sha256_RsaOaep`, `Aes256_Sha256_RsaPss`) are wired end
to end through the OpenSecureChannel handshake and protect `MSG`/`CLO`; the peer
trust (certificates + public keys) and the CSPRNG are caller-supplied.

See `docs/spec-coverage/opcua-client-server-1.05.md` for the full spec coverage.
