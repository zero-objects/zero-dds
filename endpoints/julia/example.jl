# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Runnable example for the native Julia endpoint: sync (poll) and async
# (Task + Channel). Run with `julia example.jl`.

include("zerodds.jl")
using .ZeroDDS

function sample(id, label)
    w = ZeroDDS.Writer(ZeroDDS.LE)
    ZeroDDS.put_u32!(w, id)
    ZeroDDS.put_string!(w, label)
    ZeroDDS.bytes(w)
end

function main()
    # --- sync ---
    t = ZeroDDS.mem_transport()
    c = ZeroDDS.Client(t)
    ZeroDDS.write!(c, sample(0x42, "sync-hello"))
    body = ZeroDDS.poll(c)
    if body !== nothing
        id = ZeroDDS.get_u32(ZeroDDS.Reader(body, ZeroDDS.LE))
        println("sync: received id=0x", string(id, base=16))
    end

    # --- async ---
    t2 = ZeroDDS.mem_transport()
    w = ZeroDDS.Client(t2)
    for i in 0:2
        ZeroDDS.write!(w, sample(0x100 + i, "async"))
    end
    r = ZeroDDS.start_reader(t2)
    for _ in 0:2
        b = ZeroDDS.recv(r)
        id = ZeroDDS.get_u32(ZeroDDS.Reader(b, ZeroDDS.LE))
        println("async: received id=0x", string(id, base=16))
    end
    ZeroDDS.stop!(r)
    println("ALL OK")
end

main()
