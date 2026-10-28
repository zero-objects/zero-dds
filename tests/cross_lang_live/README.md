# cross_lang_live — end-to-end live pub/sub tests per language binding

Spec: `docs/specs/zerodds-ffi-loader-1.0.md` §5 + §8.3.

One script per language that starts a **Rust publisher as a subprocess**
and runs a **language-X subscriber** in parallel, then
verifies the sample on the subscriber side. All six languages:
**C, C++, Python, Java, C#, TypeScript** (via ts-node), plus optionally
**Flutter (dart)** via `dds-chat`.

## Prerequisites

* `cargo build --release -p zerodds-c-api` (provides `libzerodds.so`).
* Per language the respective toolchain binary in `PATH`:
  * C/C++: `cc` / `c++`.
  * Python: `python3` with the `zerodds` wheel installed
    (`pip install -e bindings/python/`).
  * Java: `javac` + `java` (JDK 17+); `bindings/java/zerodds.jar` on the
    classpath.
  * C#: `dotnet` (8.0+); build `bindings/csharp/ZeroDDS/` via
    `dotnet build`.
  * TS: `node` + `ts-node` (or `tsx`); `bindings/typescript/dist/`
    via `npm run build`.

## Running

```bash
./run_all.sh
```

Per language:

```bash
./live_pubsub_c.sh
./live_pubsub_cpp.sh
./live_pubsub_python.sh
./live_pubsub_java.sh
./live_pubsub_csharp.sh
./live_pubsub_typescript.sh
```

## Expected result

Each script exits with code 0 and the output:
```
[lang=X] sub received: AAPL@200
[lang=X] PASS
```

`run_all.sh` aggregates the exit codes and prints a report.
