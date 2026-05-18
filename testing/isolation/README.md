# Isolation-Matrix — Smoke-Test-Setups (WP 2.0b T4)

Jeder Transport (UDP, TCP, SHM, UDS-Filesystem, UDS-Abstract) muss
in den fuenf Isolation-Leveln funktionieren, die Phase-2 vorsieht:

| Level | Bedeutung | Was wird getestet |
|-------|-----------|-------------------|
| **L0** | Same-Process | Unit-Tests in jeder `crates/transport-*/src/` |
| **L1** | Same-User, Different-Process | `l1_cross_process`-Integrations-Tests + `host/l1.sh` |
| **L2** | Different-User, Same-Host | `host/l2_different_user.sh` (sudo-basiert) |
| **L3** | Different-Container, Same-Host | `docker/docker-compose.*.yml` |
| **L4** | Different-Host, LAN | `l4/cross_host.sh` (SSH llvm↔pivot) |

## Smoke-Test-Runner

`tools/isolation-smoke` baut ein Binary `isolation-smoke`, das als
Sender oder Receiver auf waehlbarem Transport laeuft. Alle Scripts
orchestrieren diese eine Binary.

```
isolation-smoke --transport=<udp|shm|uds|uds-abstract>
                --role=<sender|receiver>
                --count=N
                [--local=ip:port]
                [--peer=ip:port]
                [--local-id=HEX]
                [--peer-id=HEX]
                [--base-dir=PATH]
                [--abstract-prefix=NAME]
                [--payload-size=N]
```

Return-Codes: `0` = ok, `1` = Config-Fehler, `2` = Bind-Fehler,
`3` = Send/Recv-Fehler, `4` = Payload-Mismatch.

## Matrix-Tabelle (wo jeder Transport gueltig ist)

|                     | L0 | L1 | L2 | L3 (Docker) | L4 (SSH) |
|---------------------|:--:|:--:|:--:|:-----------:|:--------:|
| UDP-unicast         | ✓  | ✓  | ✓  | ✓ (net-shared) | ✓     |
| UDP-multicast       | ✓  | ✓  | ✓  | ✓ (net-shared) | ✓     |
| TCP (mit Handshake) | ✓  | ✓  | ✓  | ✓           | ✓     |
| SHM POSIX           | ✓  | ✓  | ✓  | ✓ (`--ipc host`) | ✗ (intrinsic) |
| UDS (FS)            | ✓  | ✓  | ✓  | ✓ (volume)  | ✗ (intrinsic) |
| UDS (Abstract)      | —  | ✓ Linux | ✓ Linux | ✓ (net-ns) | ✗ (intrinsic) |

`✗ intrinsic` heisst: die Isolation-Grenze verletzt die Semantik des
Transports (z.B. SHM kann naturgemaess keine Different-Host-Trennung).

## Scripts

- `host/l1.sh <transport>` — zwei Prozesse auf dem gleichen User
- `host/l2_different_user.sh <transport>` — Sender und Receiver als
  verschiedene Unix-User (braucht `sudo`)
- `docker/` — Compose-Files pro Transport fuer L3
- `l4/cross_host.sh <transport>` — llvm als Sender, pivot als
  Receiver (beide im selben `/24`-LAN)

## Status

Dieses Verzeichnis ist v1.2-Infrastruktur, aktuell der **Rahmen**.
Die Scripts implementieren einen L1-L4-Smoke-Test pro Transport. Die
volle Scaling-Matrix (`h_many_topics`, `h_isolation_sweep`) gehoert in
WP 2.3 Harness.
