# ZeroDDS native endpoint examples (ADR 0013)

Runnable, tested end-to-end examples: a **SensorReading** published from a
native **C / Python / Java** endpoint to a ZeroDDS/XRCE hub over UDP, and
accepted + decoded by a real `zerodds-xrce` agent. No Rust on the endpoint
side; the same frame works on a big-endian legacy host.

## Run it

```sh
endpoints/examples/run.sh          # default udp/17490
endpoints/examples/run.sh 12345    # custom port
```

The script builds the agent (`endpoints/xrce-agent-demo`) and the C + Java
publishers, starts the agent expecting three samples, publishes from all three
languages, and asserts all six exchanges: three publishes (endpoint->hub) and three receives (hub->endpoint), `EXAMPLES OK: publish 3/3 + receive 3/3`.

## The pieces

| File | Role |
|---|---|
| `publish_udp.py` | pure-Python endpoint: encode SensorReading -> XRCE WRITE_DATA -> UDP |
| `PublishUdp.java` | pure-Java endpoint (uses `Zdw` + `ZdwEndpoint`) |
| `../c/examples/udp_endpoint.c` | C89 endpoint: POSIX-UDP frame-hook fill |
| `receive_udp.py` / `ReceiveUdp.java` / `../c/examples/udp_receiver.c` | receivers: bind UDP, unwrap a DATA message the hub pushes, decode the sample |
| `../xrce-agent-demo` | Rust receiver: decodes the XRCE message + reads the sample | (+ `send` mode: pushes a DATA sample to a receiver) |

## What each endpoint does

1. Encode a sample with the wire-core (`wire-fixed` or reflective) — any DDS
   type, byte-identical to the Rust core.
2. Frame it as an XRCE `WRITE_DATA` message (`zdw_xrce_write_frame` / the
   language equivalent).
3. Hand the frame to a transport via the frame-hook — here a UDP socket;
   another target fills the same hook with its own transport (see
   `../c/src/zerodds_endpoint.c` `zdw_serial_frame` for the Annex-C HDLC serial
   framing).

The hub stays Rust; the endpoint is conservative C89/pure-Python/pure-Java.
