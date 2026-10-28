# DDS-XRCE session captures

Every UDP payload of a DDS-XRCE session between **eProsima Micro-XRCE-DDS
Agent 3.0.1** and **eProsima Micro-XRCE-DDS Client 3.0.1**. Nothing in this
directory was written by this tree.

| file | datagrams |
|---|--:|
| `client-to-agent.hex` | 886 |
| `agent-to-client.hex` | 886 |

One datagram per line, lower-case hex, no separators.

## How to make them again

```sh
XRCE_ROOT=~/xrce-foreign ./build-foreign.sh     # the counterparty
XRCE_ROOT=~/xrce-foreign XRCE_OUT=~/xrce-capture ./capture.sh
./split.py --capture ~/xrce-capture             # the two .hex files
```

`capture.sh` needs to be able to open the loopback interface with
`tcpdump`, so it runs as root on the measurement host. The three scripts
sat in `/root` and nowhere else until this directory had them.

## Provenance

Built from source on the measurement host and run there; no ZeroDDS code
was loaded into either process.

* Agent — `github.com/eProsima/Micro-XRCE-DDS-Agent`, tag `v3.0.1`,
  commit `155cfaaf8b7abac2e85d4a62d3649b09ace0be55`, CMake superbuild
  (Fast DDS + Fast CDR fetched by that build).
* Client — `github.com/eProsima/Micro-XRCE-DDS-Client`, tag `v3.0.1`,
  commit `bdfa28090efc8c0e89aa5c95cdf0878c8289a18b`, CMake superbuild
  (Micro CDR 2.0.2 fetched by that build).
* Transport `udp4`, agent port 2018, capture with `tcpdump -i lo`.

## What the session contains

The clients are the reference implementation's own examples, run in this
order:

1. `SubscribeHelloWorld` + `PublishHelloWorld` — reliable stream.
2. `SubscribeHelloWorldBestEffort` + `PublishHelloWorldBestEffort` —
   best-effort stream.
3. `SubscribeBigHelloWorld` + `PublishBigHelloWorld` — payloads above the
   MTU, so the session carries `FRAGMENT`.
4. `BinaryEntityCreation` — `CREATE` with a binary object representation
   rather than the XML one.
5. `TimeSync` — `TIMESTAMP` / `TIMESTAMP_REPLY`.

Twelve of the seventeen submessage kinds of §8.3.5 appear:

| id | kind | count |
|---:|---|--:|
| 0x00 | `CREATE_CLIENT` | 8 |
| 0x01 | `CREATE` | 30 |
| 0x03 | `DELETE` | 5 |
| 0x04 | `STATUS_AGENT` | 8 |
| 0x05 | `STATUS` | 35 |
| 0x07 | `WRITE_DATA` | 11 |
| 0x08 | `READ_DATA` | 4 |
| 0x09 | `DATA` | 10 |
| 0x0a | `ACKNACK` | 358 |
| 0x0d | `FRAGMENT` | 303 |
| 0x0e | `TIMESTAMP` | 512 |
| 0x0f | `TIMESTAMP_REPLY` | 512 |

`HEARTBEAT`, `RESET` and `PERFORMANCE` are not in this session; no
example of the reference implementation emits them over this transport.

`crates/xrce/tests/foreign_frames.rs` is what reads these files.

## `GET_INFO` and `INFO` — recorded, and deliberately not here

Section 6 of `capture.sh` runs the agent with `-d` and the reference
implementation's `Discovery` example against it, and the pair does emit
both kinds. Two distinct datagrams, ten times each, from the agent's own
`-v 6` log on the measurement host:

| direction | length | submessage |
|---|--:|---|
| client → agent | 16 | `GET_INFO`, id `0x02`, flags 1, body 8 octets |
| agent → client | 60 | `INFO`, id `0x06`, flags 1, body 52 octets |

Both decode with `zerodds_xrce::Message::decode` and both re-encode to
the same octets.

**The octets are not in this directory and should not be put here.** An
`INFO` body is the list of transport locators the agent answers on
(§8.3.5, `TransportAddress`), and the recorded one carries all three
IPv4 addresses of the measurement host — the loopback, the LAN address
and a Docker bridge. Editing them out would leave a file that is no
longer what a foreign implementation sent, which is the only property
that makes anything in this directory worth reading.

So the count above stands at twelve of seventeen submessage kinds in the
committed files, and fourteen observed. Closing the gap means a capture
taken on a host whose addresses may be published, or an `INFO` from an
agent bound to loopback alone — `MicroXRCEAgent udp4 -p <port> -d` binds
every interface it finds.
