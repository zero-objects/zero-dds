# What four DDS stacks put on a socket, whole sessions

`tools/wire-capture/frames/` keeps the packets that carried a **user
sample**, because its question is what a serialiser produced. This
directory keeps **everything an implementation sent** for the length of
a session, because its question is what the protocol stack produced:
the SPDP participant announcement it repeats on a timer, the SEDP
publication and subscription announcements that describe its endpoints,
the `HEARTBEAT` a reliable writer sends, the `ACKNACK` its reader
answers with, the `INFO_*` submessages that scope them, and the user
data among them.

One file per implementation, one hex string per line, one line per
distinct RTPS datagram — the UDP payload, from the `RTPS` magic to the
last octet.

```sh
. tools/interop-env.sh
tools/wire-capture/capture-rtps-session.sh --impl cyclone \
    --out tools/wire-capture/rtps-sessions
```

## The four

| implementation | version | protocol | vendor id | domain | publisher and subscriber |
|---|---|---|---|--:|---|
| Eclipse Cyclone DDS | 11.0.1 | 2.5 | `01.10` | 61 | `ddsperf pub 10Hz` / `ddsperf sub` |
| RTI Connext | 7.7.0 | 2.5 | `01.01` | 62 | `rtiddsping -publisher` / `-subscriber` |
| eProsima Fast DDS | 2.6.10 via `rmw_fastrtps_cpp` | 2.2 | `01.0f` | 63 | `ros2 topic pub` / `ros2 topic echo` |
| OpenDDS | 3.34.0 | 2.4 | `01.03` | 42 | the `DCPS/Messenger` example over RTPS discovery |

The protocol version is the one each stack writes into its own RTPS
header and not the one it advertises elsewhere. OpenDDS is on domain 42
because the DevGuide `Messenger` writes `create_participant(42, …)` into
`Publisher.cpp` and takes no domain argument; the other three take one.
The Fast DDS column is the ROS 2 install's 2.6.10 and not the 3.6.1
under `/opt/fastdds`, because Fast DDS ships no publisher of its own
outside its examples and the RMW is a real Fast DDS participant.

Every publisher and subscriber is that implementation's own shipped
tooling. Nothing in these captures was written by this tree, which is
the property `crates/rtps/tests/vendor_frames.rs` rests on: a round trip
against our own encoder proves only that the encoder and the decoder
agree with each other.

## Four things the method had to be corrected on, and all four are findings

**A capture filtered by port misses most of a Cyclone session.**
DDSI-RTPS 2.5 §9.6.1.1 maps a domain onto a 250-wide window of UDP
ports, and a first run filtered `udp portrange 17650-17899` for domain
41. It recorded four SPDP packets of a Cyclone pair that had exchanged
2100 samples: Cyclone 11.0.1 sends discovery to the mapped port `17650`
and its user traffic from an ephemeral source port to an ephemeral
destination port — `60164 → 51507` in that run — because the port
mapping binds the discovery sockets and not the data ones. The capture
is therefore unfiltered UDP, and a datagram is attributed to an
implementation by the **vendor id in its own RTPS header** rather than
by the port a filter guessed.

**A capture on `lo` misses all of it.** SPDP is multicast and leaves
through whichever interface the stack selected, so the same first run
saw nothing at all on the loopback. `tcpdump -i any` sees both, at the
price of seeing a packet once per interface it crosses;
`extract-rtps-datagrams.py` collapses identical datagrams and reports
how many it collapsed.

**An unfiltered capture of two participants can hold a third, and the
vendor id does not separate them.** A second harness ran `rtiddsping` on
the same host while this one did, and 512 of its datagrams carried the
same `01.01`. Two gates answer it, and both read the message rather
than the capture filter: a participant belongs to this run's domain if
its SPDP went to that domain's §9.6.1.1 discovery port, and after that
every datagram carrying its GUID prefix is kept whatever port it used;
and the run refuses to write a frames file unless the number of distinct
sender GUID prefixes is the number of participants it started — two,
or three for the ROS 2 pair because `ros2` starts a `_ros2daemon`
participant of its own.

**A datagram shorter than a header has no sender.** `rtiddsping` sends
16-octet probes that begin with `RTPS` and end after eight octets of
`NDDSPING` where §8.3.3.1's 12-octet `guidPrefix` belongs. Counted as
participants they made a two-participant run report three; they are
dropped with a named count.

## What is not here

The `.pcap` captures. A session capture is mostly repeats — the same
SPDP announcement every few seconds, the same `DATA`/`HEARTBEAT` pair
per sample — and what a round-trip test needs is the variety and not the
volume. `--keep-pcap` keeps the capture beside the frames for a run that
wants to re-derive them.
