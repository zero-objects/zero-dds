<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — Go (native endpoint)

A native **pure-Go** endpoint SDK (ADR 0013): a from-scratch XCDR wire-core,
**no cgo**, byte-identical to the Rust core and the other SDKs. Both a
**synchronous** and an **asynchronous** interface — pick whichever fits.

Package: [`endpoints/go`](../../endpoints/go) · runnable example:
[`endpoints/go/example`](../../endpoints/go/example) (`go run ./example`).

## Sync

```go
c := zerodds.NewClient(transport)          // transport: you implement Deliver/Receive
c.Write(sampleXCDR)                          // frame as XRCE WRITE_DATA + deliver
body, ok, _ := c.Receive(time.Second)        // block up to timeout for one sample
```

## Async (goroutine + channel — the idiomatic Go model)

```go
w := zerodds.NewAsyncWriter(transport)
w.Write(sampleXCDR)

r := zerodds.NewAsyncReader(transport)       // spawns a receive goroutine
defer r.Close()
for body := range r.Samples {                // decoded bodies arrive on a channel
    id := zerodds.NewReader(body, zerodds.Little).GetU32()
}
```

## Wire-core

`Writer` / `Reader` cover the XCDR primitives (u8/u16/u32/u64/f32/string/
sequence<octet>) with alignment and LE/BE. `math.Float32bits` gives the IEEE bit
pattern, so `f32` matches the Rust core.

## Tests (CI job `endpoints-go`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback + **live non-blocking UDP** E2E
- the runnable example (`go run ./example`)
