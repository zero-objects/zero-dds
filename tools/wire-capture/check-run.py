#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The four checks a wire run has to pass, and the row it produces.
#
# Reads `extract-rtps.py`'s rows for one run and writes one line:
#
#   impl  case  xcdrN  status  encapsulation  options  keyhash
#         payload  poison  control
#
# `status` is `ok`, or `!` and the reason. A run that fails a check
# produces a reason and no golden — never a blank line, because a
# blank line and an agreement look the same in a table.
#
# The publisher alternates: odd sequence numbers are the **poison**
# sample, even ones are the **corpus** sample. So
#
#   * every sequence number 1..count must be present,
#   * the corpus payloads must be identical to each other,
#   * so must the poison ones,
#   * and the two must differ.
#
# The last one is the check that catches an extractor reading the wrong
# octets: if it were reading a fixed part of the frame, or the same
# cached packet twice, the two would be equal.
import argparse
import collections
import sys


def varying_offsets(hex_strings):
    """Which octet positions are not the same in all of them.

    `None` when the lengths differ, which is a different fault: a
    serialiser whose output *length* is not a function of its input is
    not something an offset list describes.
    """
    blobs = [bytes.fromhex(h) for h in hex_strings]
    if len({len(b) for b in blobs}) != 1:
        return None
    first = blobs[0]
    return [i for i in range(len(first))
            if any(b[i] != first[i] for b in blobs[1:])]


def note(drift, warmup):
    """What the run has to say about the fill, in one field."""
    parts = []
    if warmup:
        parts.append("first-call-differs " + offsets(warmup))
    if drift:
        parts.append("poison-varies " + offsets(drift))
    return "; ".join(parts) if parts else "stable"


def offsets(positions):
    """A run-length spelling, so a fill span reads as a span."""
    spans, start = [], None
    for index, position in enumerate(positions):
        if start is None:
            start = position
        if index + 1 == len(positions) or positions[index + 1] != position + 1:
            spans.append(str(start) if start == position else f"{start}-{position}")
            start = None
    return ",".join(spans)


def steady(payloads):
    """(unstable offsets in steady state, offsets the first call differs at).

    The **first** call in a run has no predecessor, so on a peer that
    does not write its alignment fill the first call's fill spans hold
    whatever the buffer held before the run — and every later call's
    hold the previous call's sample. Holding all of them to one octet
    string would refuse a golden for a peer whose steady state is
    perfectly reproducible, which is the measurement.
    """
    everything = varying_offsets(payloads)
    if everything is None:
        return None, None
    return varying_offsets(payloads[1:]) or [], everything


def check_giop(arguments, emit):
    """The same four checks over GIOP requests.

    There is no encapsulation header on a GIOP parameter and no key
    hash, so those three columns are empty; the byte order goes in the
    `encapsulation` column's place, because on this transport it is the
    header's flag byte that carries it and nothing in the stream does.
    """
    bodies = []
    orders = set()
    for line in open(arguments.rows, encoding="ascii"):
        parts = line.rstrip("\n").split("\t")
        if len(parts) != 5:
            continue
        _, operation, _, order, body = parts
        if operation != "put":
            continue
        orders.add(order)
        bodies.append(body)

    if len(bodies) != arguments.count:
        emit("!count", f"{len(bodies)} `put` requests in the capture, "
                       f"{arguments.count} were made")
        return
    if len(orders) != 1:
        emit("!orders", f"the requests are not all one byte order: {orders}")
        return

    poisons, samples = bodies[0::2], bodies[1::2]
    unstable, warmup = steady(samples)
    if unstable is None:
        emit("!unstable", "the corpus sample did not marshal to the same "
                          "*length* twice over one connection")
        return
    if unstable:
        emit("!unstable", "the corpus sample marshalled to different octets "
                          "over one connection, in steady state, at "
                          + offsets(unstable))
        return
    if samples[0] == poisons[0]:
        emit("!indistinct", "the corpus and the poison request produced the "
                            "same octets")
        return
    drift = varying_offsets(poisons) or []
    emit("ok", orders.pop(), "", "", samples[-1], poisons[-1], arguments.control,
         note(drift, warmup))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--rows", required=True)
    parser.add_argument("--count", type=int, required=True)
    parser.add_argument("--impl", required=True)
    parser.add_argument("--case", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--control", default="equal")
    parser.add_argument("--sub-log", default=None)
    parser.add_argument("--topic", default="",
                        help="the DDS topic name; an RMW prefixes it with rt/")
    parser.add_argument("--giop", action="store_true",
                        help="the rows are `extract-giop.py`'s, not "
                             "`extract-rtps.py`'s")
    arguments = parser.parse_args()

    def emit(status, *rest):
        print(
            "\t".join(
                [arguments.impl, arguments.case, f"xcdr{arguments.version}", status]
                + list(rest)
            )
        )

    # By sequence number, keeping the first copy: a reliable writer
    # retransmits, and a retransmission is the same sample.
    if arguments.giop:
        return check_giop(arguments, emit)

    # An RMW creates writers of its own beside the one under test —
    # `ros_discovery_info` is a user-defined writer like any other — so
    # "there is one user writer, it must be ours" is not a rule that
    # holds. The capture's own SEDP says which writer serves which
    # topic, and that is what selects.
    wanted = (arguments.topic, "rt/" + arguments.topic)

    by_sequence = {}
    writers = collections.Counter()
    other_kinds = collections.Counter()
    for line in open(arguments.rows, encoding="ascii"):
        parts = line.rstrip("\n").split("\t")
        if len(parts) != 9:
            continue
        (_, kind, topic, writer, sequence, key_hash, encapsulation, options,
         payload) = parts
        if topic not in wanted:
            continue
        writers[writer] += 1
        if kind != "data":
            other_kinds[kind] += 1
            continue
        by_sequence.setdefault(
            int(sequence), (writer, key_hash, encapsulation, options, payload)
        )

    if len(writers) != 1:
        emit("!writers", f"{len(writers)} writers announced topic "
                         f"{arguments.topic}: {','.join(sorted(writers))}"
                         if writers else
                         f"no writer in the capture announced topic "
                         f"{arguments.topic} — SEDP never named it")
        return

    ordered = [by_sequence[n] for n in sorted(by_sequence)]
    if len(ordered) != arguments.count:
        emit("!count", f"{len(ordered)} sample-carrying DATA in the capture, "
                       f"{arguments.count} were written"
                       + (f" (and {dict(other_kinds)} that carry no sample)"
                          if other_kinds else ""))
        return

    if arguments.sub_log:
        read = ""
        try:
            for line in open(arguments.sub_log, encoding="utf-8", errors="replace"):
                if line.startswith("read "):
                    read = line.split()[1]
        except OSError:
            read = ""
        if read != str(arguments.count):
            emit("!unread", f"the subscriber read {read or 'nothing'} of "
                            f"{arguments.count}")
            return

    poisons = ordered[0::2]
    samples = ordered[1::2]

    unstable, warmup = steady([s[4] for s in samples])
    if unstable is None:
        emit("!unstable", "the corpus sample did not serialise to the same "
                          "*length* twice through one writer")
        return
    if unstable:
        emit("!unstable", "the corpus sample serialised to different octets "
                          "through one writer, in steady state, at "
                          + offsets(unstable))
        return
    if samples[0][4] == poisons[0][4]:
        emit("!indistinct", "the corpus and the poison sample produced the "
                            "same octets — the extractor is not reading the "
                            "sample")
        return

    drift = varying_offsets([p[4] for p in poisons]) or []
    _, key_hash, encapsulation, options, payload = samples[-1]
    emit(
        "ok",
        encapsulation,
        options,
        key_hash,
        payload,
        poisons[-1][4],
        arguments.control,
        note(drift, warmup),
    )


if __name__ == "__main__":
    sys.exit(main())
