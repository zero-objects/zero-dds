# The discovery announcements of four DDS stacks

One file per implementation, one hex string per line, one line per RTPS
datagram — the UDP payload, from the `RTPS` magic to the last octet.
Every datagram carries at least one announcement this tree had not seen
before: a DATA from a builtin discovery writer whose serialized payload
is distinct from every payload kept before it.

`tests/vendor_discovery_frames.rs` reads all four and writes every
announcement back.

## Where they came from

Each capture is a whole session of one implementation on its own domain,
recorded with `tcpdump -i any` on the Linux test host on 2026-08-14 and attributed
by the vendor id in the RTPS header rather than by a port filter. The
publisher and the subscriber of every run are that implementation's own
shipped tooling — nothing in these files was written by this tree, which
is the property the round-trip test rests on.

| implementation | version | domain | publisher and subscriber |
|---|---|---|---|
| Eclipse Cyclone DDS | 11.0.1 | 41 | `ddsperf pub 100Hz` / `ddsperf sub` |
| RTI Connext | 7.7.0 | 42 | `rtiddsping -publisher` / `-subscriber` |
| eProsima Fast DDS | via `rmw_fastrtps_cpp` | 53 | `ros2 topic pub -r 10` / `ros2 topic echo` |
| OpenDDS | 3.34.0 | 44 | `DevGuideExamples/DCPS/Messenger` over RTPS discovery |

```sh
. tools/interop-env.sh
tools/wire-capture/capture-rtps-session.sh --impl cyclone --out /root/rtps-sessions
tools/wire-capture/extract-discovery-frames.py /root/rtps-sessions/cyclone/session.frames \
    --out crates/discovery/tests/fixtures/vendor-discovery/cyclone.frames
```

The session captures themselves are not here. A session is mostly
repeats — the same SPDP announcement every few seconds, the same
`DATA`/`HEARTBEAT` pair per sample — and 2 162 Cyclone datagrams reduce
to 19 that carry an announcement not already seen. What a round-trip
test needs is the variety and not the volume.

## What the fixtures do not cover

The Fast DDS run is a ROS 2 pair, so its endpoints are `rmw_fastrtps_cpp`'s
and not a bare Fast DDS application's. The RTI run is short: `rtiddsping`
repeats one publication and one subscription, so four datagrams hold
everything it announces. Neither absence is a property of the vendor —
both are properties of this measurement, and a wider run would widen the
fixture rather than change what it asserts.
