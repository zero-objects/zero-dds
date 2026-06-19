# zerodds-transport-udp

[![docs.rs](https://img.shields.io/docsrs/zerodds-transport-udp)](https://docs.rs/zerodds-transport-udp)
[![crates.io](https://img.shields.io/crates/v/zerodds-transport-udp)](https://crates.io/crates/zerodds-transport-udp)

UDP/IP PSM implementation for ZeroDDS. Layer 2 (wire implementation).

`std`-only, `forbid(unsafe_code)`, safety class **SAFE**.

## What this crate provides

- `UdpTransport` — `Transport` trait implementation over `std::net::UdpSocket`
- `UdpTransport::bind_v4` — UDPv4 unicast bind
- `UdpTransport::bind_multicast_v4` — multicast group join with
  `SO_REUSEADDR`/`SO_REUSEPORT` (discovery path SPDP/SEDP)
- `UdpTransport::set_multicast_ttl` — multicast TTL for outgoing packets
- `UdpTransport::with_timeout` — configurable read timeout
- `MAX_DATAGRAM_SIZE` — datagram cap for safe sends
- `UdpTransportError` — typed errors

## Spec

- **DDSI-RTPS 2.5 §9.6.1** — UDP/IP PSM wire mapping
- **DDSI-RTPS 2.5 §9.6.1.4** — discovery multicast (SPDP)

## Implemented (RC1)

| Feature | Status |
|---|---|
| UDPv4 unicast | ✅ |
| UDPv4 multicast (group join + TTL) | ✅ |
| `SO_REUSEADDR` + `SO_REUSEPORT` (coexistence with Cyclone) | ✅ |
| Configurable read timeout | ✅ |
| Bind retry loop (CI EADDRINUSE race) | ✅ |
| Application-layer fragmentation | ✅ via `zerodds-rtps` (DATA_FRAG) |

## Deliberately not in the crate

- **UDPv6** — extension point for future releases. The locator wire
  already supports v6, the bind API is v4-specific.
- **Async/non-blocking** — the sync architecture is a chosen style; DCPS
  uses its own tick scheduler.
- **Path-MTU discovery** — fragmentation runs at the RTPS layer.

## Tests

```bash
cargo test -p zerodds-transport-udp
```

11 tests green (lib + integration). Multicast tests skip automatically in
environments without multicast routing.

## License

Apache-2.0 OR MIT — see the workspace root.
