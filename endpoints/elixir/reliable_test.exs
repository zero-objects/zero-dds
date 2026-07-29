# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Unit tests for ZeroDDS.Reliable (mirrors crates/xrce/src/reliable.rs) plus a
# byte-golden check against golden_heartbeat_le.bin / golden_acknack_le.bin.
# Run: elixir -r lib/reliable.ex reliable_test.exs [golden_dir]

import Bitwise

alias ZeroDDS.Reliable
alias ZeroDDS.Reliable.{Sender, Receiver}

{:ok, failures} = Agent.start_link(fn -> 0 end)

check = fn ok?, name ->
  if ok? do
    IO.puts("ok   #{name}")
  else
    IO.puts("FAIL #{name}")
    Agent.update(failures, &(&1 + 1))
  end
end

# --- sender ---

s = Sender.new()
{:ok, seq0, s} = Sender.submit(s, <<1, 2>>)
{:ok, seq1, s} = Sender.submit(s, <<3, 4>>)
check.(seq0 == 0 and seq1 == 1, "submit_assigns_monotonic_seqnrs")
check.(Sender.in_flight_count(s) == 2, "submit_two_in_flight")

huge = :binary.copy(<<0>>, Reliable.max_payload() + 1)

check.(
  match?({:error, :too_large, _}, Sender.submit(Sender.new(), huge)),
  "submit_rejects_payload_too_large"
)

s =
  Enum.reduce(1..Reliable.sender_window(), Sender.new(), fn _i, acc ->
    {:ok, _seq, acc} = Sender.submit(acc, <<0>>)
    acc
  end)

check.(
  match?({:error, :window_full, _}, Sender.submit(s, <<0>>)),
  "submit_rejects_when_window_full"
)

{:ok, _seq, s} = Sender.submit(Sender.new(), <<1>>)
{hb, _s} = Sender.pending_heartbeat(s, 0)
check.(hb != nil, "pending_heartbeat_fires_first_time")

check.(
  hb.first == 0 and hb.last == 0 and hb.stream_id == Reliable.reliable_stream_id(),
  "pending_heartbeat_body_first_last_stream"
)

{:ok, _seq, s} = Sender.submit(Sender.new(), <<1>>)
{hb1, s} = Sender.pending_heartbeat(s, 0)
{hb2, s} = Sender.pending_heartbeat(s, 100)
{hb3, _s} = Sender.pending_heartbeat(s, 600)
check.(hb1 != nil, "pending_heartbeat_fires_at_t0")
check.(hb2 == nil, "pending_heartbeat_silenced_before_period")
check.(hb3 != nil, "pending_heartbeat_fires_after_period")

{hb, _s} = Sender.pending_heartbeat(Sender.new(), 0)
check.(hb == nil, "pending_heartbeat_none_when_window_empty")

s = Sender.new()
{:ok, _seq, s} = Sender.submit(s, <<0xA0>>)
{:ok, _seq, s} = Sender.submit(s, <<0xA1>>)
{:ok, _seq, s} = Sender.submit(s, <<0xA2>>)
# base=2, bitmap bit0 set -> seq2 missing; 0+1 acked
s =
  Sender.recv_acknack(s, %{
    first_unacked: 2,
    nack_bitmap: 0x0001,
    stream_id: Reliable.reliable_stream_id()
  })

check.(
  Sender.in_flight_count(s) == 1 and Sender.get_in_flight(s, 2) != nil,
  "recv_acknack_clears_acked_keeps_missing"
)

s =
  Enum.reduce(1..5, Sender.new(), fn _i, acc ->
    {:ok, _seq, acc} = Sender.submit(acc, <<0>>)
    acc
  end)

s = Sender.recv_acknack(s, %{first_unacked: 5, nack_bitmap: 0, stream_id: 0x80})
check.(Sender.in_flight_count(s) == 0, "recv_acknack_full_clear_when_no_bits_set")

# --- receiver ---

r = Receiver.new()
{:ok, r} = Receiver.recv_data(r, 0, <<10>>)
{:ok, r} = Receiver.recv_data(r, 1, <<11>>)
{d, r} = Receiver.drain_in_order(r)

check.(
  length(d) == 2 and elem(Enum.at(d, 0), 0) == 0 and elem(Enum.at(d, 1), 0) == 1 and
    Receiver.expected(r) == 2,
  "recv_data_buffers_in_order"
)

r = Receiver.new()
{:ok, r} = Receiver.recv_data(r, 2, <<22>>)
{:ok, r} = Receiver.recv_data(r, 0, <<20>>)
{d1, r} = Receiver.drain_in_order(r)
check.(length(d1) == 1 and elem(Enum.at(d1, 0), 0) == 0, "recv_data_reorder_blocks_on_gap")
{:ok, r} = Receiver.recv_data(r, 1, <<21>>)
{d2, _r} = Receiver.drain_in_order(r)

check.(
  length(d2) == 2 and elem(Enum.at(d2, 0), 0) == 1 and elem(Enum.at(d2, 1), 0) == 2,
  "recv_data_reorder_delivers"
)

r = Receiver.new()
{:ok, r} = Receiver.recv_data(r, 0, <<1>>)
{_d, r} = Receiver.drain_in_order(r)
{:ok, r} = Receiver.recv_data(r, 0, <<99>>)
check.(Receiver.out_of_order_count(r) == 0, "recv_data_drops_duplicates")

r =
  Enum.reduce(1..Reliable.receiver_buffer(), Receiver.new(), fn i, acc ->
    {:ok, acc} = Receiver.recv_data(acc, i, <<1>>)
    acc
  end)

check.(
  match?({:error, :buffer_full, _}, Receiver.recv_data(r, Reliable.receiver_buffer() + 1, <<1>>)),
  "recv_data_rejects_when_buffer_full"
)

r = Receiver.new()
{:ok, r} = Receiver.recv_data(r, 1, <<1>>)
{:ok, r} = Receiver.recv_data(r, 3, <<3>>)
ack = Receiver.pending_acknack(r, 3)

check.(
  (ack.nack_bitmap &&& 0x1) != 0 and (ack.nack_bitmap &&& 0x4) != 0 and
    (ack.nack_bitmap &&& 0x2) == 0 and
    (ack.nack_bitmap &&& 0x8) == 0,
  "pending_acknack_marks_missing_slots"
)

s = Sender.new()
r = Receiver.new()
{:ok, _seq, s} = Sender.submit(s, <<1, 2>>)
{:ok, r} = Receiver.recv_data(r, 0, <<3>>)
s = Sender.reset(s)
r = Receiver.reset(r)

check.(
  Sender.in_flight_count(s) == 0 and Receiver.out_of_order_count(r) == 0 and
    Receiver.expected(r) == 0,
  "reset_clears_state_completely"
)

# --- end-to-end loss recovery (mirror reliable.rs) ---

sender = Sender.new()
receiver = Receiver.new()
{:ok, s0, sender} = Sender.submit(sender, <<10>>)
{:ok, s1, sender} = Sender.submit(sender, <<11>>)
{:ok, s2, sender} = Sender.submit(sender, <<12>>)
{:ok, receiver} = Receiver.recv_data(receiver, s0, <<10>>)
{:ok, receiver} = Receiver.recv_data(receiver, s2, <<12>>)
{d, receiver} = Receiver.drain_in_order(receiver)

check.(
  length(d) == 1 and elem(Enum.at(d, 0), 1) == <<10>>,
  "e2e_drain_blocks_on_lost_middle_sample"
)

ack = Receiver.pending_acknack(receiver, s2)
sender = Sender.recv_acknack(sender, ack)
check.(Sender.get_in_flight(sender, s1) != nil, "e2e_missing_sample_retransmittable")

{:ok, receiver} = Receiver.recv_data(receiver, s1, Sender.get_in_flight(sender, s1))
{d2, _receiver} = Receiver.drain_in_order(receiver)

check.(
  length(d2) == 2 and elem(Enum.at(d2, 0), 1) == <<11>> and elem(Enum.at(d2, 1), 1) == <<12>>,
  "e2e_delivers_all_after_retransmit"
)

# --- byte-golden ---

case System.argv() do
  [dir | _rest] ->
    hb_path = Path.join(dir, "golden_heartbeat_le.bin")
    an_path = Path.join(dir, "golden_acknack_le.bin")

    if File.exists?(hb_path) and File.exists?(an_path) do
      gold_hb = File.read!(hb_path)
      gold_an = File.read!(an_path)

      hb =
        Reliable.heartbeat_frame(Reliable.session_nokey(), Reliable.stream_none(), 1, 1, 3, 0x80)

      an = Reliable.acknack_frame(Reliable.session_nokey(), Reliable.stream_none(), 1, 1, 0, 0x80)

      check.(hb == gold_hb, "byte_golden_heartbeat")
      check.(an == gold_an, "byte_golden_acknack")

      case Reliable.parse_heartbeat(gold_hb) do
        {:ok, p} -> check.(p.first == 1 and p.last == 3, "golden_heartbeat_parse")
        :error -> check.(false, "golden_heartbeat_parse")
      end

      case Reliable.parse_acknack(gold_an) do
        {:ok, p} -> check.(p.first_unacked == 1, "golden_acknack_parse")
        :error -> check.(false, "golden_acknack_parse")
      end
    else
      IO.puts("skip byte-golden: goldens not found in #{dir}")
    end

  [] ->
    IO.puts("skip byte-golden: no golden dir arg")
end

failed = Agent.get(failures, & &1)

if failed == 0 do
  IO.puts("ALL OK")
  System.halt(0)
else
  IO.puts("#{failed} FAILED")
  System.halt(1)
end
