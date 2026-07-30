# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Deep example (sync): a realistic sensor-telemetry flow. A publisher frames five
# typed `Reading { id, value, label }` samples and delivers them; the subscriber
# owns the run-loop and polls, decoding EVERY field byte-for-byte.

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
        r = Reading(0x1000 + i, 20.0f0 + i * 0.5f0, @sprintf("bay-%02d", i))
        ZeroDDS.write!(c, marshal(r, ZeroDDS.LE))
    end
    got = 0
    while got < total
        body = ZeroDDS.poll(c)
        body === nothing && break
        r = decode(body)
        @printf("sync reading %d: id=0x%x value=%.1f label=\"%s\"\n", got, r.id, r.value, r.label)
        got += 1
    end
    if got != total
        @printf(stderr, "incomplete: got %d of %d\n", got, total)
        exit(1)
    end
    println("ALL OK")
end

main()
