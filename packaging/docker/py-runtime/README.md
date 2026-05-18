# `zerodds/py-runtime`

Sandbox-Runtime-Image fuer **Python-Bindings** (`zerodds-py` via PyO3 +
maturin). Zielgruppe: Coding-Challenges in Zero-Learn-Sandboxes und
Quickstart-Demos fuer Python-DDS-Entwicklung.

Teil von [**ZeroDDS**](../../../README.md). Anders als die Daemon-Images
(`zerodds/cli`, `zerodds/ws-bridged`, …) liefert dieses Image **keinen
ENTRYPOINT-Service**, sondern eine bereite Python-Umgebung mit
installiertem `zerodds`-Wheel.

---

## Build

Vom Repo-Root:

```bash
docker build \
  -f packaging/docker/py-runtime/Dockerfile \
  -t zerodds/py-runtime:rc3 \
  .
```

Erst-Build dauert 5-15 Min (Cargo-Workspace + maturin-Wheel-Build).
Folge-Builds nutzen cargo-chef-Caching aggressiv und sind deutlich
schneller.

## Run

Direkt als Python-REPL (default CMD):

```bash
docker run --rm -it zerodds/py-runtime:rc3
```

Mit gemountetem Lerner-Code:

```bash
docker run --rm -it \
  -v "$PWD/workspace:/workspace" \
  zerodds/py-runtime:rc3 \
  python3 /workspace/main.py
```

Sandbox-Style (read-only-root, kein Netz nach aussen):

```bash
docker run --rm \
  --read-only \
  --tmpfs /tmp/session:rw,size=64m \
  --network none \
  -v "$PWD/workspace:/workspace:ro" \
  zerodds/py-runtime:rc3 \
  python3 /workspace/loopback_pubsub.py
```

## Was drin ist

| Komponente | Version | Pfad |
| --- | --- | --- |
| Python | 3.11 (Debian-bookworm-slim) | `/opt/zerodds/bin/python3` |
| `zerodds` Wheel | RC-Build aus `crates/py` | `/opt/zerodds/lib/python3.11/site-packages/zerodds/` |
| tini (PID 1) | bookworm-package | `/usr/bin/tini` |

`PATH` enthält das venv (`/opt/zerodds/bin`) zuerst — `python3` und
`pip` zeigen ohne weitere Massnahmen auf die ZeroDDS-Installation.

`PYTHONDONTWRITEBYTECODE=1` und `PYTHONUNBUFFERED=1` sind gesetzt; das
Image laeuft sauber mit `--read-only` (kein `.pyc`-Schreiben in
Site-Packages, Logs gehen ungepuffert auf stdout).

## Lerner-Workflow

Einfacher Codec-Roundtrip ohne Discovery:

```python
import zerodds

# IDL-aequivalenter Dataclass-Decorator (kein zerodds-idlc noetig — der
# @idl_struct-Pfad ist die offizielle Python-API laut Vendor-Spec
# zerodds-py-1.0).
@zerodds.idl_struct
class Greeting:
    id: zerodds.Int32
    text: str

g = Greeting(id=42, text="hallo welt")
buf = zerodds.encode(g)
roundtrip = zerodds.decode(Greeting, buf)
assert roundtrip == g
```

Loopback-Pub/Sub im selben Container (Discovery via Multicast-Loopback —
funktioniert ohne Multi-Container-Setup):

```python
import multiprocessing, time, zerodds

@zerodds.idl_struct
class Greeting:
    id: zerodds.Int32
    text: str

def publisher():
    p = zerodds.DomainParticipant(domain_id=0)
    w = p.create_writer(Greeting, topic="greetings")
    for i in range(5):
        w.write(Greeting(id=i, text=f"hello {i}"))
        time.sleep(0.1)

def subscriber():
    p = zerodds.DomainParticipant(domain_id=0)
    r = p.create_reader(Greeting, topic="greetings")
    for _ in range(5):
        sample = r.take_next(timeout=1.0)
        print(sample)

if __name__ == "__main__":
    multiprocessing.Process(target=subscriber).start()
    time.sleep(0.5)
    publisher()
```

## Limits

- **Discovery nur via Loopback-Multicast** im selben Container. Multi-
  Host-Discovery braucht den `unicast static peer-list` aus
  `documentation/06-operations/deployment.md` — der ist in RC3 als
  `planned` markiert.
- **Kein FFI-Loader nach `/etc`/`/usr`** zur Laufzeit — alles geht
  ueber das fertige Wheel.
- Container-Start braucht etwa 800 ms (Python-Interpreter + zerodds-
  Import). Fuer kurze Coding-Challenges einkalkulieren.

## See also

- [`crates/py/README.md`](../../../crates/py/README.md) — Python-Binding-Crate.
- [`docs/specs/zerodds-py-1.0.md`](../../../docs/specs/zerodds-py-1.0.md) — Vendor-Spec.
- [`packaging/docker/cpp-runtime/`](../cpp-runtime/) — C/C++-Schwester-Image.
- [`packaging/docker/ts-node-runtime/`](../ts-node-runtime/) — TS-Node-Schwester-Image.
