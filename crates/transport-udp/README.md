# zerodds-transport-udp

[![docs.rs](https://img.shields.io/docsrs/zerodds-transport-udp)](https://docs.rs/zerodds-transport-udp)
[![crates.io](https://img.shields.io/crates/v/zerodds-transport-udp)](https://crates.io/crates/zerodds-transport-udp)

UDP/IP-PSM-Implementation für ZeroDDS. Layer 2 (Wire-Implementation).

`std`-only, `forbid(unsafe_code)`, Safety-Klasse **SAFE**.

## Was liefert dieses Crate

- `UdpTransport` — `Transport`-Trait-Implementation über `std::net::UdpSocket`
- `UdpTransport::bind_v4` — UDPv4 Unicast Bind
- `UdpTransport::bind_multicast_v4` — Multicast-Group-Join mit
  `SO_REUSEADDR`/`SO_REUSEPORT` (Discovery-Pfad SPDP/SEDP)
- `UdpTransport::set_multicast_ttl` — Multicast-TTL für ausgehende Pakete
- `UdpTransport::with_timeout` — konfigurierbarer Read-Timeout
- `MAX_DATAGRAM_SIZE` — Datagramm-Cap für sichere Sends
- `UdpTransportError` — typisierte Fehler

## Spec

- **DDSI-RTPS 2.5 §9.6.1** — UDP/IP PSM Wire-Mapping
- **DDSI-RTPS 2.5 §9.6.1.4** — Discovery-Multicast (SPDP)

## Implementiert (RC1)

| Feature | Status |
|---|---|
| UDPv4 Unicast | ✅ |
| UDPv4 Multicast (Group-Join + TTL) | ✅ |
| `SO_REUSEADDR` + `SO_REUSEPORT` (Coexistenz mit Cyclone) | ✅ |
| Read-Timeout konfigurierbar | ✅ |
| Bind-Retry-Loop (CI-EADDRINUSE-Race) | ✅ |
| Anwendungs-Layer-Fragmentation | ✅ via `zerodds-rtps` (DATA_FRAG) |

## Bewusst nicht im Crate

- **UDPv6** — Erweiterungspunkt für künftige Releases. Locator-Wire
  unterstützt v6 bereits, Bind-API ist v4-spezifisch.
- **Async/Non-blocking** — Sync-Architektur ist gewählter Stil; DCPS
  nutzt eigene Tick-Scheduler.
- **Pfad-MTU-Discovery** — Fragmentation läuft auf RTPS-Layer.

## Tests

```bash
cargo test -p zerodds-transport-udp
```

11 Tests grün (lib + integration). Multicast-Tests skippen automatisch in
Umgebungen ohne Multicast-Routing.

## Lizenz

Apache-2.0 OR MIT — siehe Workspace-Root.
