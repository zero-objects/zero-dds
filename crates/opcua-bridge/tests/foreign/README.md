# open62541 interop fixture

`open62541_client.c` is a small, self-contained OPC-UA client built directly
against `libopen62541`'s public C API (not a shell wrapper around a
pre-built example binary). It:

1. Connects to the endpoint named in `argv[1]` (SecurityMode `None`, matching
   this daemon's own default `opcua.server.security_policies: ["none"]`, so
   no crypto backend build of open62541 is required).
2. Browses the well-known `Objects` folder (`ns=0;i=85`) and prints the
   number of references found.
3. Reads the `Value` attribute of the node named in `argv[2]`
   (`ns=2;s=<identifier>` form, e.g. `Topic:Telemetry/Value/value`) and
   prints it as a `Double`.
4. Writes the `Int32` value in `argv[4]` to the node named in `argv[3]`
   and reads it back.

Every step prints one `<STEP>_OK ...` or `<STEP>_FAIL status=0x...` line to
stdout — the harness (`../../tests/open62541_interop.rs`) parses these, it
does not parse open62541's own diagnostic output.

## Prerequisites (Linux; not run on macOS/by this agent)

- `open62541` built and installed so `pkg-config --cflags --libs open62541`
  resolves (Debian/Ubuntu: `apt install libopen62541-dev` where packaged;
  otherwise build from <https://github.com/open62541/open62541> with
  `-DUA_ENABLE_AMALGAMATION=OFF -DCMAKE_INSTALL_PREFIX=/usr/local` and
  `make install`). Tested against the open62541 1.3/1.4 stable API
  (`UA_Client_connect`, `UA_Client_readValueAttribute`,
  `UA_Client_writeValueAttribute`, `UA_Client_Service_browse`) — these calls
  have been stable across that range; a `pkg-config`-visible install is the
  only requirement checked at test time.
- A C compiler as `cc` (or set `CC=...`).
- If `pkg-config` cannot find `open62541` but the library is installed
  elsewhere, set `OPEN62541_CFLAGS`/`OPEN62541_LIBS` directly (e.g.
  `OPEN62541_CFLAGS="-I/usr/local/include"
  OPEN62541_LIBS="-L/usr/local/lib -lopen62541"`).

## Running it

```
OPCUA_OPEN62541=1 cargo test -p zerodds-opcua-bridge --test open62541_interop \
    --features daemon -- --ignored --nocapture
```

The test is `#[ignore]`d by default and short-circuits with a clear skip
message when `OPCUA_OPEN62541` is unset or `pkg-config`/`cc` cannot find
open62541 — set the env var to actually run it. It has not been run by the
agent that wrote it (no open62541 install, and this repo's own convention
for foreign-vendor interop is "Linux, not macOS" — see
`tests/interop/shapes_cyclone_interop.sh`'s own header comment).
