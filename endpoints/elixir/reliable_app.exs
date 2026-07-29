# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Reliable sender app for the live E2E test
# (crates/endpoint-e2e/tests/elixir_reliable.rs). The producer just
# `ZeroDDS.Reliable.Drain.submit/2`s — a `GenServer.cast`, returns
# immediately — and the drain process (the idiomatic BEAM decoupling:
# message-passing, not a wait-free ring) owns the reliable `Sender` state and
# the `:gen_udp` socket: submit -> history -> WRITE_DATA, periodic HEARTBEAT,
# retransmit on ACKNACK, until the send window drains.
#
# argv: <peer-port> <N> [run|bench]
# Run:  elixir -r lib/reliable.ex reliable_app.exs <port> <N>

alias ZeroDDS.Reliable
alias ZeroDDS.Reliable.Drain

defmodule ReliableApp do
  # Opens a UDP socket connected to the peer, starts a Drain, and hands it the
  # socket. Mind the ownership order: start_link (no socket I/O in init) ->
  # controlling_process (still called by us, the current owner) -> activate
  # (tells the drain process to flip `active: true` and start ticking).
  def open_drain(port) do
    {:ok, socket} = :gen_udp.open(0, [:binary, active: false])
    :ok = :gen_udp.connect(socket, {127, 0, 0, 1}, port)
    {:ok, drain} = Drain.start_link(socket)
    :ok = :gen_udp.controlling_process(socket, drain)
    :ok = Drain.activate(drain)
    drain
  end

  def run(port, n) do
    drain = open_drain(port)

    for i <- 0..(n - 1) do
      Drain.submit(drain, <<i::little-32>>)
    end

    :ok = Drain.finish(drain)
    GenServer.stop(drain)
    IO.puts("SENT #{n}")
  end

  # Producer-latency bench: measures submit-and-return (decoupled — the
  # syscall happens in the Drain process) against an inline send (the
  # producer itself pays the `:gen_udp.send` syscall). `port` must be a bound
  # (real or scratch) UDP peer; replies are not required for this bench.
  def bench(port) do
    iters = 20_000
    sample = <<0::little-32>>

    {:ok, inline_sock} = :gen_udp.open(0, [:binary, active: false])
    :ok = :gen_udp.connect(inline_sock, {127, 0, 0, 1}, port)
    frame = Reliable.write_frame(0, sample)
    t0 = System.monotonic_time(:nanosecond)
    for _ <- 1..iters, do: :gen_udp.send(inline_sock, frame)
    inline_ns = (System.monotonic_time(:nanosecond) - t0) / iters
    :gen_udp.close(inline_sock)

    drain = open_drain(port)
    t0 = System.monotonic_time(:nanosecond)
    for i <- 1..iters, do: Drain.submit(drain, <<rem(i, 0x1_0000)::little-32>>)
    submit_ns = (System.monotonic_time(:nanosecond) - t0) / iters
    GenServer.stop(drain, :normal, 200)

    IO.puts("BENCH submit_ns=#{trunc(submit_ns)} inline_send_ns=#{trunc(inline_ns)}")
  end
end

[port_s, n_s | rest] = System.argv()
port = String.to_integer(port_s)
n = String.to_integer(n_s)
mode = List.first(rest) || "run"

case mode do
  "bench" -> ReliableApp.bench(port)
  _ -> ReliableApp.run(port, n)
end
