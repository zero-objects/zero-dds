# Isolation matrix — smoke test setups (WP 2.0b T4)

Every transport (UDP, TCP, SHM, UDS filesystem, UDS abstract) must
work in the five isolation levels that phase 2 provides for:

| Level | Meaning | What is tested |
|-------|-----------|-------------------|
| **L0** | Same-Process | unit tests in every `crates/transport-*/src/` |
| **L1** | Same-User, Different-Process | `l1_cross_process` integration tests + `host/l1.sh` |
| **L2** | Different-User, Same-Host | `host/l2_different_user.sh` (sudo-based) |
| **L3** | Different-Container, Same-Host | `docker/docker-compose.*.yml` |
| **L4** | Different-Host, LAN | `l4/cross_host.sh` (two hosts over SSH) |

## Smoke test runner

`tools/isolation-smoke` builds a binary `isolation-smoke`, which runs as
sender or receiver on a selectable transport. All scripts
orchestrate this one binary.

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

Return codes: `0` = ok, `1` = config error, `2` = bind error,
`3` = send/recv error, `4` = payload mismatch.

## Matrix table (where each transport is valid)

|                     | L0 | L1 | L2 | L3 (Docker) | L4 (SSH) |
|---------------------|:--:|:--:|:--:|:-----------:|:--------:|
| UDP-unicast         | ✓  | ✓  | ✓  | ✓ (net-shared) | ✓     |
| UDP-multicast       | ✓  | ✓  | ✓  | ✓ (net-shared) | ✓     |
| TCP (with handshake) | ✓  | ✓  | ✓  | ✓           | ✓     |
| SHM POSIX           | ✓  | ✓  | ✓  | ✓ (`--ipc host`) | ✗ (intrinsic) |
| UDS (FS)            | ✓  | ✓  | ✓  | ✓ (volume)  | ✗ (intrinsic) |
| UDS (Abstract)      | —  | ✓ Linux | ✓ Linux | ✓ (net-ns) | ✗ (intrinsic) |

`✗ intrinsic` means: the isolation boundary violates the semantics of the
transport (e.g. SHM by its nature cannot do a different-host separation).

## Scripts

- `host/l1.sh <transport>` — two processes as the same user
- `host/l2_different_user.sh <transport>` — sender and receiver as
  different Unix users (needs `sudo`)
- `docker/` — Compose files per transport for L3
- `l4/cross_host.sh <transport>` — one host as sender, another as
  receiver (both in the same `/24` LAN), named by `ZERODDS_TX_HOST`
  and `ZERODDS_RX_HOST`

## Status

This directory is v1.2 infrastructure, currently the **framework**.
The scripts implement an L1-L4 smoke test per transport. The
full scaling matrix (`h_many_topics`, `h_isolation_sweep`) belongs in
the WP 2.3 harness.
