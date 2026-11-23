# Isolation matrix — smoke test setups (WP 2.0b T4)

Every transport (UDP, TCP, SHM, UDS filesystem, UDS abstract) must
work in the five isolation levels that phase 2 provides for:

| Level | Meaning | What is tested |
|-------|-----------|-------------------|
| **L0** | Same-Process | unit tests in every `crates/transport-*/src/` |
| **L1** | Same-User, Different-Process | `l1_cross_process` integration tests + `host/l1.sh` |
| **L2** | Different-User, Same-Host | `host/l2_different_user.sh` (sudo-based) |
| **L3** | Different-Container, Same-Host | `docker/docker-compose.*.yml`; UDS filesystem: `tools/isolation-smoke/tests/container_volume.rs` |
| **L4** | Different-Host, LAN | `l4/cross_host.sh` (two hosts over SSH) |

## Smoke test runner

`tools/isolation-smoke` builds a binary `isolation-smoke`, which runs as
sender or receiver on a selectable transport. All scripts
orchestrate this one binary.

```
isolation-smoke --transport=<udp|shm|uds|uds-abstract>
                --role=<sender|receiver|echo|ping>
                --count=N
                [--local=ip:port]
                [--peer=ip:port]
                [--local-id=HEX]
                [--peer-id=HEX]
                [--base-dir=PATH]
                [--abstract-prefix=NAME]
                [--payload-size=N]
                [--socket-mode=OCTAL]
                [--hold-ms=N]
```

`echo` sends every datagram back to the locator it came from; `ping`
sends `--count` datagrams one at a time and checks that each comes back
unchanged from its peer (not for `shm`). `--socket-mode` (UDS only) sets
the mode of the socket file; `--hold-ms` keeps the transport bound that
long after a successful run.

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
- `tools/isolation-smoke/tests/container_volume.rs` — L3 for UDS
  filesystem sockets (zerodds-uds-transport-1.0 §5.2): two containers
  under different UIDs on one volume with Docker, and with a user
  namespace each under Podman; a manual test on the Linux test host
  (`ci/manual-tests.tsv`)
- `l4/cross_host.sh <transport>` — one host as sender, another as
  receiver (both in the same `/24` LAN), named by `ZERODDS_TX_HOST`
  and `ZERODDS_RX_HOST`

## Status

This directory is v1.2 infrastructure, currently the **framework**.
The scripts implement an L1-L4 smoke test per transport. The
full scaling matrix (`h_many_topics`, `h_isolation_sweep`) belongs in
the WP 2.3 harness.
