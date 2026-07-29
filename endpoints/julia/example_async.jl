# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Deep example (async): the same sensor-telemetry flow, but the subscriber does
# not own the run-loop. `start_reader` spawns a Task that fills a Channel; the
# consumer blocks on `recv` (take!) — the idiomatic Julia concurrency model.
# Every field is decoded.

include("zerodds.jl")
using .ZeroDDS
using Printf

struct Reading
    id::UInt32
    value::Float32
    label::String
end

function marshal(r::Reading, endian::ZeroDDS.Endian)
    w = ZeroDDS.Writer(endian)
    ZeroDDS.put_u32!(w, r.id)
    ZeroDDS.put_f32!(w, r.value)
    ZeroDDS.put_string!(w, r.label)
    ZeroDDS.bytes(w)
end

function decode(body::Vector{UInt8})
    r = ZeroDDS.Reader(body, ZeroDDS.LE)
    id = ZeroDDS.get_u32(r)
    value = ZeroDDS.get_f32(r)
    label = ZeroDDS.get_string(r)
    Reading(id, value, label)
end

function main()
    total = 5
    t = ZeroDDS.mem_transport()
    c = ZeroDDS.Client(t)
    for i in 0:(total - 1)
        r = Reading(0x2000 + i, 100.0f0 - i, @sprintf("sensor-%02d", i))
        ZeroDDS.write!(c, marshal(r, ZeroDDS.LE))
    end
    reader = ZeroDDS.start_reader(t)
    for got in 0:(total - 1)
        body = ZeroDDS.recv(reader)
        r = decode(body)
        @printf("async reading %d: id=0x%x value=%.1f label=\"%s\"\n", got, r.id, r.value, r.label)
    end
    ZeroDDS.stop!(reader)
    println("ALL OK")
end

main()
