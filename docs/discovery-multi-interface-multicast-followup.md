# Follow-up: multi-interface multicast join/TX

**Status:** deferred (not required for #27).

## Context

#27 (multi-homed discovery failure) is resolved by announcing every eligible
unicast interface address and fanning metatraffic out to every advertised peer
locator. That fix is entirely on the **unicast** path.

The **multicast** path is unchanged: the SPDP socket binds `0.0.0.0` and the OS
selects one interface for the group join and for multicast transmission
(`IP_MULTICAST_IF` per the routing table).

## Why it is deferred, not done

Packet capture of the #27 reproduction showed SPDP multicast TX leaving on the
correct interface in **both** the failing and the passing run — 0 packets on the
misconfigured interface. The reader discovered the peer participant in the
failing run (`discovered=1`); only the endpoint match failed, because the
announced **unicast** locator was unreachable. Multicast membership/TX was never
the failing operation.

## What full multi-interface multicast would add

Independence from the OS single-interface multicast selection:

- join the SPDP group on **every** eligible interface,
- transmit SPDP on **every** eligible interface (per-interface sockets — never a
  shared socket whose outgoing interface is mutated concurrently),
- tolerate per-interface failure, require at least one usable interface,
- log the selected/joined interfaces at diagnostic level.

## Gate to implement

An independent, reproducible failure in which the OS selects the wrong interface
for multicast **join or transmission** (not merely for the unicast source-address
probe). Until such a reproduction exists, this is speculative hardening.
