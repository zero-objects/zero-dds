# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Reliable sender app for the live E2E test. The producer enqueues samples into
# a `Channel` (the decoupled path — it never touches the socket); a drain `Task`
# (`@async`, Julia's idiomatic green-thread concurrency — no `--threads` flag
# needed) owns the reliable `Sender` state + the `UDPSocket`, sends WRITE_DATA,
# emits HEARTBEAT, and retransmits on ACKNACK until the send window drains.
#
# argv: <peer-port> <N> [run|bench]
#
# Julia's `Channel` (default unbuffered/bounded) uses a lock+condvar, not a
# wait-free ring; the enqueue still pays a lock+copy, it does keep the producer
# off the send syscall, which is what the latency bench measures. A true
# wait-free SPSC ring would improve the enqueue figure further — reported
# honestly.
#
# Gotcha: libuv's `UDPSocket` only starts delivering datagrams to a Julia task
# once *something* has called into its read path (`recv`/`recvfrom`). If the
# drain task sent WRITE_DATA/HEARTBEAT before the receive task ever armed
# (called `recv`), a fast loopback ACKNACK reply could race the socket's libuv
# read-start and never reach us. So the receive task is spawned and explicitly
# yielded to (arming its `recv` call) *before* the first send.

using Sockets

include("reliable.jl")
using .Reliable

now_ms() = round(Int, time() * 1000)

"Blocks up to `timeout_s` for the next item, `nothing` on timeout."
function try_take!(ch::Channel{Vector{UInt8}}, timeout_s::Float64)
    t0 = time()
    while !isready(ch)
        (time() - t0) > timeout_s && return nothing
        sleep(0.0005)
    end
    take!(ch)
end

"Drains the producer `chan`, sends WRITE_DATA/HEARTBEAT, retransmits on
ACKNACK, until the sentinel (empty sample) has been seen and the send window
has drained."
function drain(port::Int, chan::Channel{Vector{UInt8}})
    sock = UDPSocket()
    bind(sock, ip"127.0.0.1", 0)  # concrete local port before anything else

    # Arm the receive path *before* any send — see module docstring.
    recv_chan = Channel{Vector{UInt8}}(1024)
    recv_task = @async begin
        try
            while true
                data = recv(sock)
                put!(recv_chan, data)
            end
        catch
            # socket closed under us at shutdown → exit quietly
        end
    end
    yield()  # let the recv task actually reach its first blocking `recv`

    peer = ip"127.0.0.1"
    sender = Reliable.Sender()
    msg_seq = 1
    closed = false
    start_ms = now_ms()

    while true
        # 1. pull enqueued samples: submit + send WRITE_DATA
        while isready(chan)
            payload = take!(chan)
            if isempty(payload)
                closed = true
            else
                status, seq = Reliable.submit!(sender, payload)
                if status == Reliable.SR_OK
                    send(sock, peer, port, Reliable.reliable_write_frame(seq, payload))
                end
            end
        end
        # 2. done when closed and the window has drained
        if closed && Reliable.in_flight_count(sender) == 0
            break
        end
        # 3. safety valve (peer's own deadline is 30s)
        if now_ms() - start_ms > 20000
            break
        end
        # 4. period-gated HEARTBEAT
        hb = Reliable.pending_heartbeat!(sender, now_ms())
        if hb !== nothing
            send(sock, peer, port,
                 Reliable.heartbeat_frame(Reliable.SESSION_NOKEY, Reliable.STREAM_NONE, msg_seq,
                                           hb.first, hb.last, hb.stream_id))
            msg_seq += 1
        end
        # 5. drain incoming ACKNACKs (bounded, short timeout)
        for _ in 1:64
            data = try_take!(recv_chan, 0.02)
            data === nothing && break
            ack = Reliable.parse_acknack(data)
            if ack !== nothing
                Reliable.recv_acknack!(sender, ack.first_unacked, ack.nack_lo, ack.nack_hi)
            end
        end
        # 6. retransmit whatever is still in-flight (peer marked it missing)
        if Reliable.in_flight_count(sender) > 0
            for (seq, payload) in Reliable.in_flight_pairs(sender)
                send(sock, peer, port, Reliable.reliable_write_frame(seq, payload))
            end
        end
    end
    close(sock)
end

function run_reliable(port::Int, n::Int)
    chan = Channel{Vector{UInt8}}(1024)
    drain_task = @async drain(port, chan)
    for i in 0:(n - 1)
        v = UInt32(i)
        put!(chan, UInt8[v & 0xff, (v >> 8) & 0xff, (v >> 16) & 0xff, (v >> 24) & 0xff])
    end
    put!(chan, UInt8[])  # sentinel: empty payload = "no more samples"
    wait(drain_task)
    println("SENT $n")
end

function run_bench(port::Int)
    iters = 20000
    sample = UInt8[0, 0, 0, 0]
    peer = ip"127.0.0.1"

    # inline: producer does the send syscall itself
    sock = UDPSocket()
    bind(sock, ip"127.0.0.1", 0)
    t0 = time_ns()
    for i in 0:(iters - 1)
        send(sock, peer, port, Reliable.reliable_write_frame(UInt16(i & 0xffff), sample))
    end
    inline_ns = (time_ns() - t0) / iters
    close(sock)

    # decoupled: producer only enqueues (a drain task would do the I/O)
    chan = Channel{Vector{UInt8}}(1024)
    sink_task = @async begin
        while true
            data = take!(chan)
            isempty(data) && break
        end
    end
    yield()
    t0 = time_ns()
    for i in 0:(iters - 1)
        put!(chan, sample)
    end
    enqueue_ns = (time_ns() - t0) / iters
    put!(chan, UInt8[])
    wait(sink_task)

    println("BENCH enqueue_ns=$(round(Int, enqueue_ns)) inline_send_ns=$(round(Int, inline_ns))")
end

function main()
    if length(ARGS) < 2
        println(stderr, "usage: julia reliable_app.jl <port> <N> [run|bench]")
        exit(2)
    end
    port = parse(Int, ARGS[1])
    n = parse(Int, ARGS[2])
    mode = length(ARGS) >= 3 ? ARGS[3] : "run"
    if mode == "bench"
        run_bench(port)
    else
        run_reliable(port, n)
    end
end

main()
