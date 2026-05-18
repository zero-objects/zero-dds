# cross_lang_live — End-to-end live Pub/Sub-Tests pro Sprachbinding

Spec: `docs/specs/zerodds-ffi-loader-1.0.md` §5 + §8.3.

Pro Sprache ein Skript, das einen **Rust-Publisher als Subprocess**
startet und einen **Lang-X-Subscriber** parallel laufen laesst, dann
das Sample auf der subscriber-Seite verifiziert. Alle sechs Sprachen:
**C, C++, Python, Java, C#, TypeScript** (per ts-node), plus optional
**Flutter (dart)** ueber `dds-chat`.

## Voraussetzungen

* `cargo build --release -p zerodds-c-api` (liefert `libzerodds.so`).
* Pro Sprache das jeweilige Toolchain-Binary in `PATH`:
  * C/C++: `cc` / `c++`.
  * Python: `python3` mit `zerodds`-Wheel installiert
    (`pip install -e bindings/python/`).
  * Java: `javac` + `java` (JDK 17+); `bindings/java/zerodds.jar` im
    Classpath.
  * C#: `dotnet` (8.0+); `bindings/csharp/ZeroDDS/` build via
    `dotnet build`.
  * TS: `node` + `ts-node` (oder `tsx`); `bindings/typescript/dist/`
    via `npm run build`.

## Ausfuehrung

```bash
./run_all.sh
```

Pro Sprache:

```bash
./live_pubsub_c.sh
./live_pubsub_cpp.sh
./live_pubsub_python.sh
./live_pubsub_java.sh
./live_pubsub_csharp.sh
./live_pubsub_typescript.sh
```

## Erwartetes Ergebnis

Jedes Skript exit-code 0 mit Output:
```
[lang=X] sub received: AAPL@200
[lang=X] PASS
```

`run_all.sh` aggregiert die Exit-Codes und gibt einen Bericht aus.
