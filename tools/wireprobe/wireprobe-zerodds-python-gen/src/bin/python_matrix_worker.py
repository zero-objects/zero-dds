#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""``wireprobe-python-matrix``'s worker: run many generated ``probe.py``
sources in one interpreter, instead of one process launch per unit.

Read as ``worker.py <manifest> <results>``. ``manifest`` is one JSON
object per line, one per job:

    {"id": <int>, "kind": "write"|"read", "source": <the generated
     module's full text>, "values_out": <path the read direction writes
     to, or null>}

The source is inline and not a path: the write direction alone is
fifty thousand-odd units in a full run, and that many small files on
disk buys nothing a JSON string does not already give a `compile()`
call for a traceback's sake -- `compile`'s second argument only ever
needs to be *a* label, not a real path.

``results`` is written the same way, one line per job, in the order the
manifest gave them:

    {"id": <int>, "exit": <int>, "hex": <stdout bytes, write jobs>}
    {"id": <int>, "exit": <int>, "json": <values_out's text, read jobs>}
    {"id": <int>, "exit": <int>, "stderr": <first non-blank line of
     standard error, present only when the exit code was not zero>}

## Why one process answers a whole shard

Interpreter start-up is the cost a process launch per job would pay
again and again; `wireprobe-zerodds-python-walk` pays it once per case
per configuration per encoding per byte order, which is the whole
matrix's cell count. Here it is paid once per worker, and every job
after the first pays only for `exec()` and its own generated code --
`dataclasses`, `zerodds_wire` and the rest are already in `sys.modules`
after the first job that names them.

## Why each job gets a fresh, registered module

`exec(source, {})` very nearly works: a bare dict runs the module's
top-level code, including every `def` and `@dataclasses.dataclass`
class. It stops working the moment a *frozen* dataclass under `from
__future__ import annotations` is decorated, because `dataclasses`
resolves a `ClassVar`/`InitVar` marker by looking the class's own module
up in `sys.modules` and reading its `__dict__` -- `_is_type` in the
standard library's own `dataclasses.py`. A plain dict is not there, so
`cls.__module__` names nothing and the lookup raises `AttributeError:
'NoneType' object has no attribute '__dict__'` before the sample this
job was measuring ever runs. `types.ModuleType`, registered under a
throw-away name and discarded straight after `exec` returns, is what
every generated module already gets when it runs as a script -- Python
itself registers `__main__` in `sys.modules` for it -- so this gives
`exec` the same thing standing in for a name nothing else uses.

## Why standard output and standard error are both captured

The write direction's octets go to `sys.stdout.buffer`, unchanged from
what the same source writes running as a standalone script; `sys.stdout`
is swapped for an in-memory buffer around the one `exec` call so this
worker's *real* standard output stays free for jobs after it, and for
nothing else -- the results protocol is written straight to the
`results` file, never to standard output. `sys.stderr` is swapped the
same way: a refusal's whole reason is what the generated harness prints
there on its way to a non-zero `SystemExit` -- "wireprobe-zerodds-python:
{error}" -- and a result line that carried only the exit code would turn
every refusal in a matrix run back into a number with no sentence next
to it, which is the finding a caller needs and not only the fact that
one exists. `result["stderr"]` is that first line, present only when the
exit code was not zero.
"""

from __future__ import annotations

import io
import json
import sys
import types


class _FakeStdout:
    """Stands in for ``sys.stdout`` for the one ``exec`` call of a write
    job.

    The generated harness writes through ``.buffer`` -- `sys.stdout` is
    a text stream and the payload is not text -- and nothing else of the
    real ``sys.stdout`` interface, so nothing else is stood in for.
    """

    def __init__(self, buffer: io.BytesIO) -> None:
        self.buffer = buffer


def run_one(job: dict) -> dict:
    """Execute one job's generated module and report what it produced."""
    source = job["source"]
    label = f"<wireprobe-python-matrix job {job['id']}>"

    name = f"zd_matrix_job_{job['id']}"
    module = types.ModuleType(name)
    module.__file__ = label
    sys.modules[name] = module

    real_stdout, real_stderr = sys.stdout, sys.stderr
    out_buffer = io.BytesIO()
    err_buffer = io.StringIO()
    sys.stdout = _FakeStdout(out_buffer)
    sys.stderr = err_buffer
    exit_code = 0
    unexpected = ""
    try:
        exec(compile(source, label, "exec"), module.__dict__)  # noqa: S102
    except SystemExit as error:
        if isinstance(error.code, int):
            exit_code = error.code
        elif error.code is None:
            exit_code = 0
        else:
            exit_code = 1
    except BaseException as error:  # noqa: BLE001 -- one bad job must not sink the shard
        exit_code = 1
        unexpected = f"{type(error).__name__}: {error}"
    finally:
        sys.stdout, sys.stderr = real_stdout, real_stderr
        del sys.modules[name]

    if unexpected:
        # Not a verdict the generated harness reached on purpose: this
        # worker's own real standard error, with the job id, so a caller
        # reading the worker's log can attribute it. `err_buffer` holds
        # only what the *generated code* wrote before it raised, which is
        # a different thing.
        print(f"job {job['id']} ({label}): {unexpected}", file=sys.stderr)

    out: dict = {"id": job["id"], "exit": exit_code}
    if exit_code != 0:
        first_line = next(
            (line for line in err_buffer.getvalue().splitlines() if line.strip()),
            "",
        )
        if first_line:
            out["stderr"] = first_line
    if job["kind"] == "write":
        out["hex"] = out_buffer.getvalue().hex()
    else:
        values_out = job.get("values_out")
        if exit_code == 0 and values_out:
            try:
                with open(values_out, encoding="utf-8") as handle:
                    out["json"] = handle.read()
            except OSError as error:
                out["exit"] = 1
                out["stderr"] = str(error)
                print(f"job {job['id']} ({values_out}): {error}", file=sys.stderr)
    return out


def main() -> int:
    manifest_path, results_path = sys.argv[1], sys.argv[2]
    with open(manifest_path, encoding="utf-8") as handle:
        jobs = [json.loads(line) for line in handle if line.strip()]

    with open(results_path, "w", encoding="utf-8") as results:
        for job in jobs:
            results.write(json.dumps(run_one(job)))
            results.write("\n")
            results.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
