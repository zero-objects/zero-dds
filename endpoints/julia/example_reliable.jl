# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Runnable reliable-stream demo (in-process, no sockets): an aggregator submits
# N samples over a lossy link (every 3rd dropped on first pass); the receiver
# recovers the gaps via ACKNACK + retransmit and prints the contiguous sequence
# it actually delivered. Run: julia example_reliable.jl

include("reliable.jl")
using .Reliable

function main()
    N = 12
    sender = Reliable.Sender()
    receiver = Reliable.Receiver()
    for i in 0:(N - 1)
        Reliable.submit!(sender, UInt8[i])
    end

    delivered = Int[]
    pass = 0
    dropped_once = UInt16[]
    while length(delivered) < N
        frames = Reliable.in_flight_pairs(sender)
        idx = 0
        for (seq, payload) in frames
            idx += 1
            if pass == 0 && idx % 3 == 0 && !(seq in dropped_once)
                push!(dropped_once, seq)  # simulate loss on the first pass
                continue
            end
            Reliable.recv_data!(receiver, seq, payload)
        end
        for (_, payload) in Reliable.drain_in_order!(receiver)
            push!(delivered, Int(payload[1]))
        end
        # receiver → ACKNACK; sender purges acked, keeps missing for retransmit
        ack = Reliable.pending_acknack(receiver, nothing)
        Reliable.recv_acknack!(sender, ack.first_unacked, ack.nack_lo, ack.nack_hi)
        pass += 1
    end

    println("delivered: ", join(delivered, " "))
    println("dropped on first pass: ", length(dropped_once), " — recovered after ", pass, " passes")
    ok = length(delivered) == N && all(delivered[i + 1] == i for i in 0:(N - 1))
    println(ok ? "RELIABLE OK" : "RELIABLE FAIL")
end

main()
