# zerodds-test-domains

Leases a DDS domain to one test process at a time.

A DDS domain number is a UDP port number (RTPS 9.6.1.4.1:
`7400 + 250*domain`), so it is a resource of the whole host. This crate
hands out domains that

* no other process on the host holds, across test binaries and across
  checkouts,
* have their whole RTPS port window outside the range the kernel gives to
  sockets that asked for any port, and
* no DDS vendor and no ROS deployment defaults to, and no process outside
  this workspace uses on the LAN (`FOREIGN_DOMAINS`: 226 and 230), and
* lie in this host's share of the band. SPDP multicast reaches every host
  of the LAN (measured between the CI runner host and the Linux test
  host, 2026-11-01), and a lease arbitrates one host. The CI jobs set
  `ZERODDS_TEST_DOMAIN_BAND=ci` and lease from 219 to 231; every other host
  leases from the rest of the band (on Linux 215 to 218, 90 and 92); a value
  `<low>-<high>` names another share. On Windows and macOS, whose band does
  not reach 219, `ci` is the whole band.

The leases are files in `$TMPDIR/zerodds-dcps-domains`, or in the
directory `ZERODDS_TEST_DOMAIN_DIR` names. The CI jobs of a runner host run
in containers of their own on the host's network; they set the variable to a
directory every job of the host mounts, so the jobs arbitrate the host's
domains among themselves. A lease line names its owner,
`pid <N> since <T> ns <pid namespace>`. A reader in the owner's PID
namespace asks the kernel whether that PID lives; a reader in another one
reads the owner's heartbeat instead: the owner refreshes the file's
modification time every 2 s, and a lease not refreshed for 20 s counts as
given up. The Java and TypeScript copies
(`crates/java-omgdds/java/src/test/java/org/zerodds/testing/TestDomains.java`,
`crates/ts-node/test/support/test-domains.ts`) follow the same protocol;
`tests/java_lease_agrees.rs` and `tests/ts_lease_agrees.rs` hold them to it.

Not published: it is scaffolding for this workspace's tests.
