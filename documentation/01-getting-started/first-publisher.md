# First publisher and subscriber

Run the maintained source examples directly instead of copying a second
implementation into this chapter:

- [hello_dds_publisher.rs](../../crates/dcps/examples/hello_dds_publisher.rs)
- [hello_dds_subscriber.rs](../../crates/dcps/examples/hello_dds_subscriber.rs)

They use DomainParticipantFactory, a topic, and typed writer/reader handles
over RawBytes. RawBytes is an opaque payload demonstration, not proof of
an IDL-generated application type or cross-vendor interoperability. For
generated types, continue with the [build.rs integration](../../crates/zerodds-build/README.md)
and its [example companion](../../website/docs/examples/zerodds-build-sample.html).

## Build

Follow [Installation](installation.md), from the same checkout root:

```bash
cargo build --locked -j 1 -p zerodds-dcps \
  --example hello_dds_publisher --example hello_dds_subscriber
```

## Run in two terminals

Start the reader first:

```bash
./target/debug/examples/hello_dds_subscriber
```

Then start the publisher in another terminal:

```bash
./target/debug/examples/hello_dds_publisher
```

On Windows, use the corresponding .exe paths. Both processes use domain 0
and topic Chatter. The publisher writes hello # followed by an increasing
counter once per second. The subscriber prints received UTF-8 payloads.
Both continue until Ctrl-C. A one-second reader timeout is an idle tick,
not an application failure.

The publisher's console output means it attempted a write. Confirm the
matching payload on the subscriber before considering the round trip
successful. Initial samples can precede discovery.

## Discovery and configuration

Use two processes on the same host for this introduction. The participant
factory selects the standard runtime files described in
[Installation](installation.md#configuration-before-running).
Default UDP discovery needs multicast connectivity. If the machine's
configuration disables multicast, provide reachable discovery.peers in
the runtime files instead of changing the multicast group to a unicast
address. Do not assume that two console windows alone prove discovery.

The source examples create participants through the factory rather than
embedding fixed GUID prefixes. Errors are propagated by the examples.
These commands are source-aligned instructions, not a replacement for a
recorded release-gate result.

## Related

[Website companion](../../website/docs/examples/getting-started-first-publisher.html) ·
[Integration](../README.md#integration-per-language) ·
[Runtime layering source](../../crates/dcps/tests/runtime_config_layering.rs)
