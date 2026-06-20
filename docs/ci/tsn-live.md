# CI: `tsn-live` job — live AF_PACKET transport

The `tsn-live` job (stage `test` in `.gitlab-ci.yml`) covers the DDS-TSN
1.0 Annex A live transport: RTPS directly in Ethernet frames
(EtherType `0x88B5`) over an `AF_PACKET`/`SOCK_RAW` socket
(`crates/transport-tsn/src/socket.rs`, feature `live`, Linux-only).

The default `test` job builds the workspace **without** the
`transport-tsn` `live` feature, so `socket.rs` and its frame logic were
never compiled or run in CI before this job existed.

## What runs unconditionally (no privileges)

```
cargo test -p zerodds-transport-tsn --features live \
  --target x86_64-unknown-linux-gnu
```

This compiles the Linux-only `socket.rs` (catching any rot in the
syscall layer) and runs the 12 platform-neutral `live_frame` unit tests
(VLAN selection, Ethernet frame construction + minimum-frame padding,
sysfs MAC parsing). No `CAP_*` needed.

## What runs only with net capabilities

The `veth_loopback` integration test
(`crates/transport-tsn/tests/veth_loopback.rs`) performs a real RTPS
round trip between two `TsnTransport` instances bound to the two ends of
a `veth` pair in the root network namespace. It is marked `#[ignore]`
and is run only via `--ignored`.

It needs two Linux capabilities:

- **`CAP_NET_ADMIN`** — to create the `veth` pair (`ip link add … type
  veth`).
- **`CAP_NET_RAW`** — to open the `AF_PACKET`/`SOCK_RAW` socket.

The job probes for `CAP_NET_ADMIN` by trying to create a throwaway
`veth` pair. Only on success does it run the round trip; otherwise it
logs a clear `SKIP` line and leaves the job green. The skip is never
silent.

### Making part 2 mandatory

The runner is a Docker executor with `network_mode=host`, which does
**not** grant `NET_ADMIN`/`NET_RAW` by default. To make the veth round
trip run in CI, grant the two capabilities to the runner's job
containers in the runner config (`/etc/gitlab-runner/config.toml`):

```toml
[runners.docker]
  # host networking is already in use; add the two net caps so the
  # tsn-live veth round trip can create the link and bind AF_PACKET.
  cap_add = ["NET_ADMIN", "NET_RAW"]
```

(Equivalent to `--cap-add=NET_ADMIN --cap-add=NET_RAW` on `docker run`.)
Avoid `privileged = true` — only the two specific caps are required.

After granting them, the probe passes and the round trip asserts a real
byte-identical RTPS exchange (forward + reverse) with the VLAN VID
preserved.

## Local run (on a Linux box with root)

```
sudo -E cargo test -p zerodds-transport-tsn --features live \
  --test veth_loopback -- --ignored --test-threads=1
```

In an LXC container (e.g. codepit) the root network namespace is used
because `ip netns add` is blocked there (EPERM); the `veth` pair in the
root NS proves the AF_PACKET path over a real L2 link regardless.
