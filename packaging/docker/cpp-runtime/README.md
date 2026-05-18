# `zerodds/cpp-runtime`

Sandbox-Runtime-Image fuer **C- und C++-Bindings** plus Build-Toolchain.
Zielgruppe: Coding-Challenges in Zero-Learn-Sandboxes und allgemein
Quickstart-Demos fuer C/C++-DDS-Entwicklung.

Teil von [**ZeroDDS**](../../../README.md). Anders als die Daemon-Images
liefert dieses Image **keinen ENTRYPOINT-Service**, sondern Tools +
Library + Header. Lerner kompiliert eigenen Code im Container.

Ein **gemeinsames Image** fuer reines C **und** C++, weil die Toolchain
identisch ist (clang, libzerodds.so, gemeinsame Header). Der Unterschied
ist allein, welchen Compiler/Header der Lerner anspricht.

---

## Build

Vom Repo-Root:

```bash
docker build \
  -f packaging/docker/cpp-runtime/Dockerfile \
  -t zerodds/cpp-runtime:rc3 \
  .
```

Erst-Build dauert 5-15 Min (cargo-chef-Cold-Cache, Rust-Compile von
`zerodds-c-api`). Folge-Builds sind durch Layer-Caching deutlich
schneller.

## Run

Interaktive Shell (default CMD):

```bash
docker run --rm -it zerodds/cpp-runtime:rc3
```

Mit gemountetem Lerner-Code:

```bash
docker run --rm -it \
  -v "$PWD/workspace:/workspace" \
  zerodds/cpp-runtime:rc3 \
  bash -c 'cd /workspace && make && ./app'
```

Sandbox-Style (read-only-root, kein Netz nach aussen):

```bash
docker run --rm \
  --read-only \
  --tmpfs /tmp:rw,size=64m \
  --network none \
  -v "$PWD/workspace:/workspace" \
  zerodds/cpp-runtime:rc3 \
  bash /workspace/run.sh
```

## Was drin ist

| Komponente | Pfad |
| --- | --- |
| `libzerodds.so` (cdylib) | `/usr/local/lib/libzerodds.so` |
| C-Header (FFI) | `/usr/local/include/zerodds.h`, `/usr/local/include/zerodds_xcdr2.h` |
| C++-RAII-Header (ZeroDDS-Style) | `/usr/local/include/zerodds/dds.hpp` |
| C++-Header (OMG DDS-PSM-Cxx) | `/usr/local/include/dds/dds.hpp` |
| `zerodds-idlc` | `/usr/local/bin/zerodds-idlc` (mit `--c` und `--cpp` Flag) |
| Compiler | `clang`, `clang++` (`CC`/`CXX` voreingestellt) |
| Build-Tools | `cmake`, `make`, `lld`, `pkg-config` |
| Init | `tini` (PID 1) |

`LD_LIBRARY_PATH` ist auf `/usr/local/lib` gesetzt, `ldconfig` hat
`libzerodds.so` registriert — `clang -lzerodds` und `dlopen()` finden
die Bibliothek ohne weitere Angaben.

## Lerner-Workflow — reines C

```bash
# IDL -> C-Header
zerodds-idlc --c -o gen chat.idl
# erzeugt gen/chat.h mit C-Typen und Codec-Stubs.

# Lerner schreibt main.c:
cat > main.c <<'EOF'
#include <stdio.h>
#include <zerodds.h>
#include "gen/chat.h"

int main(void) {
    zerodds_domain_participant_t* p =
        zerodds_create_participant(0);
    /* Topic, Writer, Reader, write, take ... */
    zerodds_delete_participant(p);
    return 0;
}
EOF

clang -std=c11 -Wall -Igen -L/usr/local/lib -lzerodds \
      main.c -o chat-c
./chat-c
```

## Lerner-Workflow — C++17

```bash
# IDL -> C++-Header
zerodds-idlc --cpp -o gen chat.idl
# erzeugt gen/chat.hpp mit RAII-Wrapper-Klassen + OMG-DDS-PSM-Cxx-Mapping.

# Lerner schreibt main.cpp:
cat > main.cpp <<'EOF'
#include <iostream>
#include <zerodds/dds.hpp>
#include "gen/chat.hpp"

int main() {
    zerodds::DomainParticipant p{0};
    auto topic = p.create_topic<Greeting>("greetings");
    auto writer = p.create_writer(topic);
    writer.write(Greeting{42, "hallo welt"});
    return 0;
}
EOF

clang++ -std=c++17 -Wall -Igen -L/usr/local/lib -lzerodds \
        main.cpp -o chat-cpp
./chat-cpp
```

## Limits

- **Discovery nur via Loopback-Multicast** im selben Container. Multi-
  Host-Discovery braucht den `unicast static peer-list`
  (`documentation/06-operations/deployment.md`), in RC3 noch `planned`.
- **Keine system-Pakete-Installation zur Laufzeit** bei
  `--read-only`-Mounts — alle Build-Tools muessen im Image sein
  (sind sie: clang/cmake/make/lld/pkg-config).
- Container-Start ~400 ms (kein Interpreter-Cold-Start).

## See also

- [`crates/zerodds-c-api/README.md`](../../../crates/zerodds-c-api/README.md) — C-FFI-Crate.
- [`crates/cpp/README.md`](../../../crates/cpp/README.md) — C++-RAII-Wrapper-Crate.
- [`crates/idl-cpp/README.md`](../../../crates/idl-cpp/README.md) — IDL→C++/C-Codegen (idl-cpp deckt beide Modi ueber `--c` / `--cpp` ab).
- [`packaging/docker/py-runtime/`](../py-runtime/) — Python-Schwester-Image.
- [`packaging/docker/ts-node-runtime/`](../ts-node-runtime/) — TS-Node-Schwester-Image.
