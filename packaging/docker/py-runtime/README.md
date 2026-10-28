# `zerodds/py-runtime`

Sandbox runtime image for **Python bindings** (`zerodds-py` via PyO3 +
maturin). Target audience: coding challenges in Zero-Learn sandboxes and
quickstart demos for Python DDS development.

Part of [**ZeroDDS**](../../../README.md). Unlike the daemon images
(`zerodds/cli`, `zerodds/ws-bridged`, …) this image provides **no
ENTRYPOINT service**, but a ready Python environment with the
`zerodds` wheel installed.

---

## Build

From the repo root:

```bash
docker build \
  -f packaging/docker/py-runtime/Dockerfile \
  -t zerodds/py-runtime:rc7 \
  .
```

The first build takes 5-15 min (cargo workspace + maturin wheel build).
Subsequent builds use cargo-chef caching aggressively and are considerably
faster.

## Run

Directly as a Python REPL (default CMD):

```bash
docker run --rm -it zerodds/py-runtime:rc7
```

With mounted learner code:

```bash
docker run --rm -it \
  -v "$PWD/workspace:/workspace" \
  zerodds/py-runtime:rc7 \
  python3 /workspace/main.py
```

Sandbox style (read-only root, no network to the outside):

```bash
docker run --rm \
  --read-only \
  --tmpfs /tmp/session:rw,size=64m \
  --network none \
  -v "$PWD/workspace:/workspace:ro" \
  zerodds/py-runtime:rc7 \
  python3 /workspace/loopback_pubsub.py
```

## What is inside

| Component | Version | Path |
| --- | --- | --- |
| Python | 3.11 (Debian bookworm-slim) | `/opt/zerodds/bin/python3` |
| `zerodds` wheel | RC build from `crates/py` | `/opt/zerodds/lib/python3.11/site-packages/zerodds/` |
| tini (PID 1) | bookworm package | `/usr/bin/tini` |

`PATH` contains the venv (`/opt/zerodds/bin`) first — `python3` and
`pip` point to the ZeroDDS installation without further steps.

`PYTHONDONTWRITEBYTECODE=1` and `PYTHONUNBUFFERED=1` are set; the
image runs cleanly with `--read-only` (no `.pyc` writing into
site-packages, logs go unbuffered to stdout).

## Learner workflow

Types come from real IDL, not from a Python decorator (no more
`@idl_struct` — RC7 removes the second, hand-written Python
CDR codec in favour of exactly one, C2(7)). Generate once:

```idl
// greeting.idl
@final
struct Greeting {
  long id;
  string text;
};
```

```bash
zerodds-idlc generate --lang python --out . greeting.idl
mv greeting.py greeting_gen.py
```

Simple codec round trip without discovery:

```python
import greeting_gen as generated  # freshly generated, see above

descriptor = generated.ZD_TOPIC_DESCRIPTORS["Greeting"]
Greeting = descriptor.cls

g = Greeting(id=42, text="hallo welt")
buf = descriptor.encode(g)
roundtrip = descriptor.decode(buf)
assert roundtrip == g
```

Loopback pub/sub in the same container (discovery via multicast loopback —
works without a multi-container setup):

```python
import multiprocessing, time, zerodds
import greeting_gen as generated

descriptor = generated.ZD_TOPIC_DESCRIPTORS["Greeting"]
Greeting = descriptor.cls

def publisher():
    p = zerodds.DomainParticipantFactory.instance().create_participant(0)
    topic = zerodds.IdlTopic(p, "greetings", descriptor)
    w = topic.create_writer(p.create_publisher())
    for i in range(5):
        w.write(Greeting(id=i, text=f"hello {i}"))
        time.sleep(0.1)

def subscriber():
    p = zerodds.DomainParticipantFactory.instance().create_participant(0)
    topic = zerodds.IdlTopic(p, "greetings", descriptor)
    r = topic.create_reader(p.create_subscriber())
    for _ in range(5):
        r.wait_for_data(1.0)
        for sample in r.take():
            print(sample)

if __name__ == "__main__":
    multiprocessing.Process(target=subscriber).start()
    time.sleep(0.5)
    publisher()
```

## Limits

- **Discovery only via loopback multicast** within the same container.
  Multi-host discovery uses unicast peers: `discovery.peers` / `ZERODDS_PEERS` (see `docs/doc-map/config.en.md`).
- **No FFI loader to `/etc`/`/usr`** at runtime — everything goes
  through the prebuilt wheel.
- Container start takes about 800 ms (Python interpreter + zerodds
  import). Factor this in for short coding challenges.

## See also

- [`crates/py/README.md`](../../../crates/py/README.md) — Python binding crate.
- [`docs/specs/zerodds-py-1.0.md`](../../../docs/specs/zerodds-py-1.0.md) — vendor spec.
- [`packaging/docker/cpp-runtime/`](../cpp-runtime/) — C/C++ sibling image.
- [`packaging/docker/ts-node-runtime/`](../ts-node-runtime/) — TS-Node sibling image.
