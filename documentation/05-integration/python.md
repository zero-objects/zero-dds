# Python

`zerodds-py` is a `pyo3`-based binding. Native, no FFI shim — Python
calls Rust directly.

## Install (when published)

```bash
pip install zerodds
```

Until then, build from source:

```bash
cd crates/py
pip install maturin
maturin develop --release
```

## Hello, world

```python
import zerodds
import time

rt = zerodds.Runtime(domain_id=0)

with rt.writer("Hello", "RawBytes", reliable=True) as w:
    if not w.wait_for_matched(1, timeout=5.0):
        raise RuntimeError("no subscriber")
    w.write(b"hello from Python")
```

Subscriber:

```python
import zerodds

rt = zerodds.Runtime(domain_id=0)

with rt.reader("Hello", "RawBytes", reliable=True) as r:
    while True:
        data = r.take(timeout=1.0)
        if data is not None:
            print("got:", data.decode("utf-8"))
```

## Generated types from IDL

`zerodds-idlc Robot.idl --python -o gen/py` produces:

```python
# gen/py/robot/pose.py
from dataclasses import dataclass

@dataclass
class Pose:
    id: str
    x: float
    y: float
    z: float

    def encode_cdr(self) -> bytes: ...
    @classmethod
    def decode_cdr(cls, data: bytes) -> "Pose": ...
```

Use with the typed API:

```python
from robot import Pose

w = rt.typed_writer("Telemetry", Pose)
w.write(Pose(id="r1", x=1.0, y=2.0, z=3.0))
```

## QoS

```python
import zerodds.qos as q

writer_qos = q.WriterQos(
    reliable=True,
    durability=q.Durability.TRANSIENT_LOCAL,
    deadline_period=0.05,           # seconds
    history=q.History.keep_last(10),
)
w = rt.writer("Telemetry", "Robot::Pose", qos=writer_qos)
```

## Async

`zerodds-py` exposes both blocking and `asyncio`-friendly readers:

```python
import asyncio

async def main():
    rt = zerodds.Runtime(0)
    async with rt.async_reader("Telemetry", "Robot::Pose") as r:
        async for sample in r:
            print(sample)

asyncio.run(main())
```

Internally an mpsc receiver gets fed into a Python `asyncio.Queue`
on a background thread.

## Type hints

The binding ships `.pyi` stubs so `mypy` / Pyright catch
type-mismatches. The PEP-561 `py.typed` marker is included.

## Multiprocessing

Each process runs its own `DcpsRuntime` — no inter-process state
sharing in the binding. To share the same runtime across
processes, use shared-memory transport (planned via
`transport-shm`).

## Reading further

- `crates/py/README.md` — pyo3-binding details, build tooling.
- `crates/py/examples/` — typical usage samples.
- `pyo3` docs at <https://pyo3.rs> for binding internals.
