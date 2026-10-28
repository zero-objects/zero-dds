# `zerodds/cpp-runtime`

Sandbox runtime image for **C and C++ bindings** plus a build toolchain.
Target audience: coding challenges in Zero-Learn sandboxes and, in general,
quickstart demos for C/C++ DDS development.

Part of [**ZeroDDS**](../../../README.md). Unlike the daemon images,
this image provides **no ENTRYPOINT service**, but tools +
library + headers. The learner compiles their own code in the container.

One **shared image** for plain C **and** C++, because the toolchain
is identical (clang, libzerodds.so, shared headers). The only difference
is which compiler/headers the learner uses.

---

## Build

From the repo root:

```bash
docker build \
  -f packaging/docker/cpp-runtime/Dockerfile \
  -t zerodds/cpp-runtime:rc7 \
  .
```

The first build takes 5-15 min (cargo-chef cold cache, Rust compile of
`zerodds-c-api`). Subsequent builds are considerably faster thanks to layer
caching.

## Run

Interactive shell (default CMD):

```bash
docker run --rm -it zerodds/cpp-runtime:rc7
```

With mounted learner code:

```bash
docker run --rm -it \
  -v "$PWD/workspace:/workspace" \
  zerodds/cpp-runtime:rc7 \
  bash -c 'cd /workspace && make && ./app'
```

Sandbox style (read-only root, no network to the outside):

```bash
docker run --rm \
  --read-only \
  --tmpfs /tmp:rw,size=64m \
  --network none \
  -v "$PWD/workspace:/workspace" \
  zerodds/cpp-runtime:rc7 \
  bash /workspace/run.sh
```

## What is inside

| Component | Path |
| --- | --- |
| `libzerodds.so` (cdylib) | `/usr/local/lib/libzerodds.so` |
| `libzerodds_wire.a` (the one C++ wire codec, `runtime/cpp`) | `/usr/local/lib/libzerodds_wire.a` |
| C headers (FFI) | `/usr/local/include/zerodds.h`, `/usr/local/include/zerodds_xcdr2.h` |
| C++ RAII headers (ZeroDDS style) + wire codec headers | `/usr/local/include/zerodds/dds.hpp`, `/usr/local/include/zerodds/wire.hpp` |
| C++ headers (OMG DDS-PSM-Cxx) | `/usr/local/include/dds/dds.hpp` |
| `zerodds-idlc` | `/usr/local/bin/zerodds-idlc` (`generate --lang c`, `--lang c++`) |
| Compiler | `clang`, `clang++` (`CC`/`CXX` preset) |
| Build tools | `cmake`, `make`, `lld`, `pkg-config` |
| Init | `tini` (PID 1) |

`LD_LIBRARY_PATH` is set to `/usr/local/lib`, `ldconfig` has
registered `libzerodds.so` — `clang -lzerodds` and `dlopen()` find
the library without further settings.

## Learner workflow — plain C

```bash
# IDL -> C header
zerodds-idlc generate --lang c --out gen chat.idl
# generates gen/chat.h with C types and codec stubs.

# the learner writes main.c:
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

## Learner workflow — C++17

```bash
# IDL -> C++ header
zerodds-idlc generate --lang c++ --cpp-mapping native --out gen chat.idl
# generates gen/chat.hpp with the codec's structs (public fields) for
# zerodds::TypedWriter<T>. Without `--cpp-mapping native` the default is the mapping
# of DDS-PSM-Cxx 1.0 (classes with accessor functions, for dds::pub::DataWriter<T>).

# the learner writes main.cpp:
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

clang++ -std=c++17 -Wall -Igen -L/usr/local/lib -lzerodds -lzerodds_wire \
        main.cpp -o chat-cpp
./chat-cpp
```

## Limits

- **Discovery only via loopback multicast** within the same container.
  Multi-host discovery uses unicast peers: `discovery.peers` / `ZERODDS_PEERS` (see `docs/doc-map/config.en.md`).
- **No installation of system packages at runtime** with
  `--read-only` mounts — all build tools must be in the image
  (they are: clang/cmake/make/lld/pkg-config).
- Container start ~400 ms (no interpreter cold start).

## See also

- [`crates/zerodds-c-api/README.md`](../../../crates/zerodds-c-api/README.md) — C-FFI crate.
- [`crates/cpp/README.md`](../../../crates/cpp/README.md) — C++ RAII wrapper crate.
- [`crates/idl-cpp/README.md`](../../../crates/idl-cpp/README.md) — IDL→C++/C codegen (idl-cpp covers both modes via `--c` / `--cpp`).
- [`packaging/docker/py-runtime/`](../py-runtime/) — Python sibling image.
- [`packaging/docker/ts-node-runtime/`](../ts-node-runtime/) — TS-Node sibling image.
