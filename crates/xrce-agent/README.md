# `zerodds-xrce-agent`

XRCE agent running in Full/Standard profile

Part of [**ZeroDDS**](../../README.md). Safety classification: **STANDARD**.

## Status

Implements the OMG DDS-XRCE 1.0 Agent side: the wire protocol lives in
`zerodds-xrce` (`crates/xrce`), the session engine and DDS bridge
(`daemon::session::SessionEngine`, `daemon::dcps_bridge`) and the
`zerodds-xrce-agentd` daemon binary live here. See
`man/man1/zerodds-xrce-agentd.1`, `man/man5/zerodds-xrce-agentd.yaml.5`
and the sections below for the daemon; the rustdoc on the crate's public
items covers the library API, and
[Documentation Trail](../../documentation/README.md) has the wider map.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
zerodds-xrce-agent = { path = "../path/to/xrce-agent" }
# or, when published:
# zerodds-xrce-agent = "0.x"
```

## Tests

```bash
cargo test -p zerodds-xrce-agent
```

Two tests replay/exercise the daemon against real eProsima traffic and a
real DDS peer rather than only ZeroDDS-internal fixtures:

* `tests/replay_foreign_session.rs` — replays the 886-datagram capture in
  `tools/wire-capture/xrce-sessions/` (real eProsima Micro-XRCE-DDS
  Agent/Client 3.0.1 traffic, byte-for-byte) against `SessionEngine`
  with a `NullDdsPort`, asserting the handshake, `CREATE`/`DELETE` and
  `STATUS_AGENT`/`STATUS` replies match what the real agent sent.
* `tests/e2e_udp_loopback.rs` — a real UDP loopback run of
  `zerodds-xrce-client` → this daemon → a `DcpsRuntime` DataReader in a
  second process, keyed reliable and best-effort writes with dispose,
  and a payload above the MTU in both directions (client-side and
  daemon-side `FRAGMENT` split/reassembly).

## Running against real eProsima Micro-XRCE-DDS Clients

The commands below run the reference implementation's own example
Clients (the same binaries `tools/wire-capture/xrce-sessions/capture.sh`
uses to build the replay fixture) against `zerodds-xrce-agentd` instead
of `MicroXRCEAgent`, with a Cyclone DDS or Fast DDS `HelloWorld` example
as the peer on the DDS side. Building the Clients is covered by
`tools/wire-capture/xrce-sessions/build-foreign.sh`; nothing below is
ZeroDDS code except the daemon itself.

**Measured** on the Linux measurement host, the eProsima Micro-XRCE-DDS
Client 3.0.1 examples against this daemon and, for comparison, against the
eProsima Micro-XRCE-DDS Agent 3.0.1, with a Cyclone DDS 11.0.1 `HelloWorld`
peer on domain 0, three runs each:

| pair | this daemon | eProsima agent |
|---|---|---|
| `PublishHelloWorld` → Cyclone reader | 4 of 5 | 4 of 5 |
| Cyclone writer → `SubscribeHelloWorld` | 5 of 5 | 5 of 5 |
| `PublishHelloWorld` → `SubscribeHelloWorld` | 5 of 5 | 4–5 of 5 |
| `…BestEffort` pair | 3 of 3 | 2 of 3 |
| `…BigHelloWorld` pair (2 × 50 kB) | 2 of 2 | 1 of 2 |
| `BinaryEntityCreation` | 3 of 3 | 3 of 3 |
| `TimeSync` | runs | runs |

The sample missing from the first row, with both agents, is the first:
the example writes it right after creating its DataWriter, before the
Cyclone reader has matched the agent's new (VOLATILE) DDS writer.
`tests/replay_foreign_session.rs` checks the handshake/`CREATE`/`DELETE`/
`STATUS` replies against the capture's bytes and the DDS entities the
captured `CREATE`s describe; `tests/e2e_udp_loopback.rs` runs
`zerodds-xrce-client` → this daemon → a `DcpsRuntime` reader in a second
process, fragmentation both ways included.

### 1. Build the Clients

```sh
git clone -b v3.0.1 https://github.com/eProsima/Micro-XRCE-DDS-Client ~/xrce-foreign/client
XRCE_ROOT=~/xrce-foreign UCLIENT_BUILD_EXAMPLES=ON \
    cmake -S ~/xrce-foreign/client -B ~/xrce-foreign/client/build \
    -DCMAKE_BUILD_TYPE=Release -DUCLIENT_BUILD_EXAMPLES=ON -DUCLIENT_SUPERBUILD=ON
cmake --build ~/xrce-foreign/client/build -j"$(nproc)"
EX=~/xrce-foreign/client/build/examples
```

### 2. Start the daemon

```sh
zerodds-xrce-agentd --listen udp:0.0.0.0:2018
```

(`2018` is both the daemon's default listener port and the port every
example Client below defaults to when none is given, so this and the
`capture.sh` invocations line up.)

### 3. Start a DDS-side `HelloWorld` peer

The reference examples publish/subscribe topic `HelloWorldTopic`, type
`HelloWorld` (`{ unsigned long index; string message; }`) — the same
type+topic Fast DDS's own `HelloWorldExample` uses, since eProsima built
the two example sets to interoperate.

Fast DDS (path as used in `tests/interop/live_spdp.sh`):

```sh
/opt/fastdds/examples/cpp/dds/HelloWorldExample/bin/DDSHelloWorldExample subscriber
```

Cyclone DDS — `/opt/cyclone` as installed for this repo's interop hosts
(`tools/interop-env.sh`) ships `idlc` and `libddsc` only, not the
prebuilt examples; build the `examples` target of the same checkout to
get `HelloworldSubscriber`/`HelloworldPublisher` under its `examples/helloworld/`
build output:

```sh
cmake --build /opt/cyclone/build --target HelloworldSubscriber HelloworldPublisher
/opt/cyclone/build/bin/HelloworldSubscriber
```

### 4. Run the Micro Clients against the daemon

Reliable, `PublishHelloWorld`/`SubscribeHelloWorld` (5 samples):

```sh
"$EX/SubscribeHelloWorld/SubscribeHelloWorldClient" 127.0.0.1 2018 5
"$EX/PublishHelloWorld/PublishHelloWorldClient" 127.0.0.1 2018 5
```

Best-effort (3 samples):

```sh
"$EX/SubscribeHelloWorldBestEffort/SubscribeHelloWorldClientBestEffort" 127.0.0.1 2018 3
"$EX/PublishHelloWorldBestEffort/PublishHelloWorldClientBestEffort" 127.0.0.1 2018 3
```

`BigHelloWorld` — payload above the MTU, exercises `FRAGMENT` (2
samples; binary name is the upstream one, `SubscribBigBigClient`):

```sh
"$EX/SubscribeBigHelloWorld/SubscribBigBigClient" 127.0.0.1 2018 2
"$EX/PublishBigHelloWorld/PublishBigHelloWorldClient" 127.0.0.1 2018 2
```

`BinaryEntityCreation` — `CREATE` with a binary object representation
(`IN_BINARY`) instead of DDS-XML (3 samples):

```sh
"$EX/BinaryEntityCreation/BinaryEntityCreation" 127.0.0.1 2018 3
```

`TimeSync` — `TIMESTAMP`/`TIMESTAMP_REPLY` exchange (3 rounds):

```sh
"$EX/TimeSync/TimeSync" 127.0.0.1 2018 3
```

## See also

* [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) —
  layered crate architecture.
* [`documentation/02-architecture/components.md`](../../documentation/02-architecture/components.md) —
  per-crate map (English).
