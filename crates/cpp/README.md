# `zerodds-cpp`

C++ headers over the ZeroDDS C API (`crates/zerodds-c-api`, `libzerodds`):

- `#include <dds/dds.hpp>` — DDS-PSM-Cxx 1.0 (`dds::domain`, `dds::topic`,
  `dds::pub`, `dds::sub`, `dds::core`);
- `#include <zerodds/dds.hpp>` — the short form (`zerodds::Runtime`,
  `zerodds::TypedWriter<T>`).

The headers under `dds/` are ISO/IEC C++ 2003, as DDS-PSM-Cxx 1.0 §3 and §7.1 ask: they compile with
`-std=c++03 -pedantic` (`tools/psm-cxx03-check.py`, the test
`every_public_header_compiles_on_its_own_as_cxx03`). `zerodds/psm/cxx.hpp` gives them the standard
library names of C++11 where there is a standard library, and small own classes (`shared_ptr`,
`mutex`, `function`, `array`) where there is none; the C++11 additions of the specification
(§7.12) are behind `#if ZERODDS_CXX11`. `<zerodds/dds.hpp>`, `zerodds/psm/convert.hpp`, the codec
below and the code of `zerodds-idlc generate --lang c++` need C++17.

`zerodds-idlc generate --lang c++` writes the classes of DDS-PSM-Cxx 1.0 §7.4 and §8.1 by default;
`--cpp-mapping native` writes the structures with public fields that the codec reads and writes
(what `zerodds::TypedWriter<T>` takes).

One CDR codec: `runtime/cpp` (`<zerodds/wire.hpp>`, `zerodds::wire::Writer` /
`Reader`). `topic_type_support<T>::encode`/`decode`, whether hand-written or
emitted by `zerodds-idlc generate --lang c++`, builds a `Writer`/`Reader` and
calls into it; there is no second, hand-rolled XCDR2 implementation under
`crates/cpp/include` any more.

Header-only, but not dependency-free: `zerodds-cpp` links
`zerodds::zerodds-wire-cpp` (`runtime/cpp/CMakeLists.txt`), a small static
library of four translation units. The Cargo package itself is a container
for the C++ artifacts; it has no Rust runtime. Part of
[**ZeroDDS**](../../README.md).

## Build

As a CMake subproject or standalone (`CMakeLists.txt`; options
`ZERODDS_BUILD_EXAMPLES`, `ZERODDS_BUILD_TESTS`, both on when the directory is
the top-level project) — configure `../../runtime/cpp` first, or via the root
`CMakeLists.txt`, which already does. Types come from
`zerodds-idlc generate --lang c++`.

## Tests

```bash
cargo test -p zerodds-cpp          # compiles tests/smoke_dds_psm.cpp against libzerodds; needs clang++ or c++
crates/cpp/examples/build_cpp_smoke.sh
```

`cargo test -p zerodds-cpp` also runs `generated_type_via_cargo`, which needs `make` and `runtime/cpp`
besides the compiler, and the smoke program under the thread and the address sanitizer. A smoke run
started by hand (`./smoke_dds_psm`) uses the domains 0 to 49; `cargo test` leases them
(`ZERODDS_TEST_DOMAINS`, one domain per position `D(n)` of the program) so that no other process of the
test host hears it. A missing C++ compiler is a failure of the tests that build C++ when
`ZERODDS_REQUIRE_CXX=1` is set (`tools/idlc/tests/cpp_headers_together.rs`).
