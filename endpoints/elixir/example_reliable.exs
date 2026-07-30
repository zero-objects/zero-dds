# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Runnable reliable-stream demo (in-process, no sockets): a sender submits N
# samples over a lossy link (every 3rd dropped on the first pass); the
# receiver recovers the gaps via ACKNACK + retransmit and prints the
# contiguous sequence it actually delivered.
# Run: elixir -r lib/reliable.ex example_reliable.exs

alias ZeroDDS.Reliable.{Sender, Receiver}

n = 12

sender =
  Enum.reduce(0..(n - 1), Sender.new(), fn i, s ->
    {:ok, _seq, s} = Sender.submit(s, <<i>>)
    s
  end)

deliver_pass = fn deliver_pass, sender, receiver, delivered, pass, dropped_once ->
  if length(delivered) >= n do
    {delivered, pass, dropped_once}
  else
    {receiver, dropped_once} =
      sender
      |> Sender.in_flight_pairs()
      |> Enum.with_index(1)
      |> Enum.reduce({receiver, dropped_once}, fn {{seq, payload}, idx},
                                                  {receiver, dropped_once} ->
        drop_now? = pass == 0 and rem(idx, 3) == 0 and not MapSet.member?(dropped_once, seq)

        if drop_now? do
          {receiver, MapSet.put(dropped_once, seq)}
        else
          {:ok, receiver} = Receiver.recv_data(receiver, seq, payload)
          {receiver, dropped_once}
        end
      end)

    {new, receiver} = Receiver.drain_in_order(receiver)
    delivered = delivered ++ Enum.map(new, fn {_seq, payload} -> :binary.first(payload) end)

    # receiver -> ACKNACK; sender purges acked, keeps missing for retransmit
    ack = Receiver.pending_acknack(receiver, nil)
    sender = Sender.recv_acknack(sender, ack)

    deliver_pass.(deliver_pass, sender, receiver, delivered, pass + 1, dropped_once)
  end
end

{delivered, passes, dropped_once} =
  deliver_pass.(deliver_pass, sender, Receiver.new(), [], 0, MapSet.new())

IO.puts("delivered: #{Enum.join(delivered, " ")}")

IO.puts("dropped on first pass: #{MapSet.size(dropped_once)} -- recovered after #{passes} passes")

ok? = delivered == Enum.to_list(0..(n - 1))
IO.puts(if ok?, do: "RELIABLE OK", else: "RELIABLE FAIL")
if not ok?, do: System.halt(1)
