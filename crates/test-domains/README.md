# zerodds-test-domains

Leases a DDS domain to one test process at a time.

A DDS domain number is a UDP port number (RTPS 9.6.1.4.1:
`7400 + 250*domain`), so it is a resource of the whole host. This crate
hands out domains that

* no other process on the host holds, across test binaries and across
  checkouts,
* have their whole RTPS port window outside the range the kernel gives to
  sockets that asked for any port, and
* no DDS vendor and no ROS deployment defaults to.

Not published: it is scaffolding for this workspace's tests.
