<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS async — C++ (native endpoint)

The native C++ endpoint SDK (ADR 0013) gains a **modern (C++17) async facade**
as an **additive** add-on: the conservative C++98 facade (`zerodds_wire.hpp`)
stays untouched. Like the rest of the C++ SDK it is a thin facade over the
audited C reactor — it does **not** re-implement the wire, so it cannot drift
from the byte-identical core.

Header: [`endpoints/cpp/include/zerodds_async.hpp`](../../endpoints/cpp/include/zerodds_async.hpp)

## Model

`std::function` callbacks + RAII over the transport's non-blocking receive:

```cpp
#include "zerodds_async.hpp"

zerodds::AsyncReader reader(&transport, rxbuf, sizeof rxbuf,
    [&](const unsigned char* body, std::size_t len) {
        // decode `body` with your generated codec
    });

for (;;) {
    reader.run();               // dispatch all ready frames, returns count
    // … wait on your event loop …
}

zerodds::AsyncWriter writer(&transport, txbuf, sizeof txbuf,
                            ZDW_XRCE_SESSION_NOKEY, ZDW_XRCE_STREAM_BEST_EFFORT);
writer.write(sample_xcdr, len); // fire-and-forget, framed as XRCE WRITE_DATA
```

`AsyncReader` is non-copyable (it registers `this` as the C callback cookie).

## Tests

`make -C endpoints/cpp test` (in CI via `endpoints-native`) — the existing C++98
byte-identity / endpoint / mutable / nested / reflective tests **plus**:

- `test_async_loopback` (C++17) — N samples through a FIFO transport, dispatched
  + decoded in order.
- `test_async_udp` (C++17) — the same over a real non-blocking UDP socket.

Conservative tests build with `-std=c++98`, the async add-on with `-std=c++17`;
the shared C wire-core stays ISO C89 and compiles clean under both.
