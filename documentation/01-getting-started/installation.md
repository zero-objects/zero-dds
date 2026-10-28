# Installation from the candidate checkout

This guide follows the source revision you have checked out. It does not
assert that RC7 packages, container images or registry crates have been
published. Release-channel availability must be checked separately when the
candidate is released.

## Requirements

The workspace declares Rust 1.88 or newer in [Cargo.toml](../../Cargo.toml).
Install a compatible Rust toolchain and your platform's native linker.
Run the commands below from the ZeroDDS checkout root. Keep Cargo.lock and
the source revision together.

## Build the introductory examples

```bash
cargo build --locked -j 1 -p zerodds-dcps \
  --example hello_dds_publisher --example hello_dds_subscriber
```

This builds just the two examples needed for the next chapter, not every
binding, bridge and tool in the workspace. Their executables are placed in
target/debug/examples (with .exe on Windows). Cargo may need network access
to fetch dependencies on the first build.

The commands describe the current source targets. They are not a claim
that every platform or external distribution channel has passed the RC7
release gate.

## Configuration before running

The participant factory loads runtime configuration from
/etc/zerodds/zerodds.yaml, then sorted YAML fragments in
/etc/zerodds/zerodds.d and /run/zerodds/zerodds.d. Missing implicit paths
use defaults. Malformed or unreadable configuration fails startup.
The examples have no --config argument.

See the [runtime configuration contract](../../crates/config/README.md)
and [executable layering examples](../../crates/dcps/tests/runtime_config_layering.rs).
Do not copy legacy ZERODDS_PEERS or ZERODDS_CONFIG exports into this workflow.

## Next

[Run the publisher and subscriber](first-publisher.md) ·
[Website companion](../../website/docs/examples/getting-started-first-publisher.html)
