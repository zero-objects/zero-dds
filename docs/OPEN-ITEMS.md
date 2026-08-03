# Open Items

Tracked follow-up work. Each entry links a `*-followup.md` with the detail.

## Discovery

- **Multi-interface multicast join/TX** — see
  [discovery-multi-interface-multicast-followup.md](discovery-multi-interface-multicast-followup.md).
  Deferred out of the #27 multi-locator fix. On a multi-homed host the SPDP
  multicast socket joins/transmits on a single OS-selected interface. #27 is
  resolved without changing this (the announced unicast locator, not multicast
  membership, was the failing operation — proven by packet capture). Implement
  only after an independent reproduction demonstrates a case where multicast
  RX/TX itself selects the wrong interface.
