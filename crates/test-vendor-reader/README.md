# zerodds-test-vendor-reader

Drives a DDS reader of another vendor from a Rust test and reads back the
instance state it saw per instance. The live cross-vendor dispose tests use
it: the C API (`crates/zerodds-c-api/tests/writer_dispose_sample_vendors.rs`),
the routing service, the Zenoh bridge and the DDS-AMQP endpoint.

The reader is a child process that speaks one line protocol:

```
reader DOMAIN TOPIC SECONDS SNAPSHOT_FILE STOP_FILE
```

It prints `READY` once its reader exists, `MATCHED` once it matched a
writer and `SEEN id=<id>` for the first valid sample of each instance. When `SNAPSHOT_FILE` appears it prints `SNAP id=<id> state=<state>`
per instance and `SNAPDONE`; when `STOP_FILE` appears, or after `SECONDS`,
the same as `FINAL` lines and exits. The type is
`@final struct KeyedPointA { @key long id; long value; }`.

Readers:

- Eclipse Cyclone DDS through its Python binding: [`CYCLONE_READER_PY`],
  run with `python3` (`PYTHON` names another interpreter).
- Fast DDS, OpenDDS and OpenSplice 6.9: `tools/dispose-reader/build.sh`
  builds them and prints the command line to put into
  `ZERODDS_DISPOSE_READER`.

`lease_domain` leases a test domain up to `HIGHEST_DOMAIN` (230):
OpenSplice 6.9 refuses domain 231.

Not published: it is scaffolding for this workspace's tests.
