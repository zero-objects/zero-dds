# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Reliable XRCE stream (spec §8.4.10/§8.4.11) for the native Elixir endpoint
# SDK. Self-contained: the sender/receiver state machines (immutable structs —
# Elixir has no mutation, so every call threads the struct through and returns
# the new one) + the WRITE_DATA/HEARTBEAT/ACKNACK frame codec, byte-identical
# to `crates/xrce` and the other endpoint SDKs. Importable beside `zerodds.ex`;
# no dependency on it.
#
# Frame layout (little-endian, 8-byte header + body):
#   [0]=session [1]=stream [2..4]=seq LE [4]=submessage id [5]=flags
#   [6..8]=body-len LE [8..]=body
#   WRITE_DATA id=0x07 flags=0x03 body=sample; header seq = RFC-1982 sample seq
#   HEARTBEAT  id=0x0B flags=0x01 body(5)= first i16 LE, last i16 LE, stream
#   ACKNACK    id=0x0A flags=0x01 body(5)= first_unacked i16 LE, nack[0], nack[1], stream
# Control frames (HEARTBEAT/ACKNACK) go out with header stream = StreamNone
# (0x00) — the stream this control message is *about* is carried in the body's
# trailing `stream` byte instead (matches `zerodds-endpoint-golden`'s goldens).
#
# BEAM is inherently message-passing: `ZeroDDS.Reliable.Drain` below is the
# async-decoupled writer, but it is honestly NOT a wait-free ring — it is a
# GenServer whose mailbox is the queue. `Drain.submit/2` is a `GenServer.cast`:
# the producer pays one message-send and returns; the drain process (owning
# the `Sender` state + the `:gen_udp` socket) does the submit/send/heartbeat/
# retransmit work off the producer's path.

defmodule ZeroDDS.Reliable do
  @moduledoc """
  Reliable-stream frame codec + RFC-1982 helpers. See `ZeroDDS.Reliable.Sender`
  and `ZeroDDS.Reliable.Receiver` for the state machines (mirrors
  `crates/xrce/src/reliable.rs`), and `ZeroDDS.Reliable.Drain` for the
  decoupled sender process.
  """

  @session_nokey 0x80
  @stream_none 0x00
  @reliable_stream_id 0x80
  @heartbeat_period_ms 500
  @sender_window 16
  @receiver_buffer 64
  @max_payload 65_535
  @sm_write_data 0x07
  @sm_acknack 0x0A
  @sm_heartbeat 0x0B
  @flag_write 0x03
  @flag_e_le 0x01

  def session_nokey, do: @session_nokey
  def stream_none, do: @stream_none
  def reliable_stream_id, do: @reliable_stream_id
  def heartbeat_period_ms, do: @heartbeat_period_ms
  def sender_window, do: @sender_window
  def receiver_buffer, do: @receiver_buffer
  def max_payload, do: @max_payload

  @doc "RFC-1982 16-bit `a < b` (half-window = 0x8000)."
  def seq_lt(a, b) do
    d = Integer.mod(b - a, 0x10000)
    d != 0 and d < 0x8000
  end

  @doc "RFC-1982 16-bit `a > b`."
  def seq_gt(a, b), do: seq_lt(b, a)

  # --- frame codec ---

  @doc "Reliable WRITE_DATA — identical bytes to `ZeroDDS.Endpoint.write_frame(0x80, 0x80, seq, sample)`."
  def write_frame(seq, sample) do
    n = byte_size(sample)

    <<@session_nokey::8, @reliable_stream_id::8, seq::little-16, @sm_write_data::8,
      @flag_write::8, n::little-16>> <> sample
  end

  @doc """
  Control-frame HEARTBEAT. Byte-identical to `golden_heartbeat_le.bin` for
  `(session=0x80, stream_hdr=StreamNone, msg_seq=1, first=1, last=3, stream_id=0x80)`.
  """
  def heartbeat_frame(session, stream_hdr, msg_seq, first, last, stream_id) do
    <<session::8, stream_hdr::8, msg_seq::little-16, @sm_heartbeat::8, @flag_e_le::8,
      5::little-16, first::little-16, last::little-16, stream_id::8>>
  end

  @doc """
  Control-frame ACKNACK. Byte-identical to `golden_acknack_le.bin` for
  `(session=0x80, stream_hdr=StreamNone, msg_seq=1, first_unacked=1, nack_bitmap=0, stream_id=0x80)`.
  """
  def acknack_frame(session, stream_hdr, msg_seq, first_unacked, nack_bitmap, stream_id) do
    <<session::8, stream_hdr::8, msg_seq::little-16, @sm_acknack::8, @flag_e_le::8, 5::little-16,
      first_unacked::little-16, nack_bitmap::little-16, stream_id::8>>
  end

  @doc "Parses a HEARTBEAT frame into `%{first:, last:, stream_id:}`, `:error` otherwise."
  def parse_heartbeat(
        <<_session::8, _stream::8, _seq::little-16, @sm_heartbeat::8, _flags::8, _len::little-16,
          first::little-16, last::little-16, stream_id::8>>
      ) do
    {:ok, %{first: first, last: last, stream_id: stream_id}}
  end

  def parse_heartbeat(_), do: :error

  @doc "Parses an ACKNACK frame into `%{first_unacked:, nack_bitmap:, stream_id:}`, `:error` otherwise."
  def parse_acknack(
        <<_session::8, _stream::8, _seq::little-16, @sm_acknack::8, _flags::8, _len::little-16,
          first_unacked::little-16, nack_bitmap::little-16, stream_id::8>>
      ) do
    {:ok, %{first_unacked: first_unacked, nack_bitmap: nack_bitmap, stream_id: stream_id}}
  end

  def parse_acknack(_), do: :error

  # --- sender ---

  defmodule Sender do
    @moduledoc """
    Sender-side reliable state (immutable — every call returns the new
    struct). Mirrors `ReliableStreamState`'s sender half in `reliable.rs`.
    """

    import Bitwise

    defstruct next_seq: 0, in_flight: %{}, last_heartbeat_ms: nil

    def new, do: %__MODULE__{}

    def in_flight_count(s), do: map_size(s.in_flight)

    def get_in_flight(s, seq), do: Map.get(s.in_flight, seq)

    def in_flight_pairs(s), do: Map.to_list(s.in_flight)

    @doc """
    Buffers a new sample. `{:ok, seq, sender}` on success, or
    `{:error, :too_large | :window_full, sender}` (caller must process
    ACKNACKs first to free the window).
    """
    def submit(s, payload) do
      cond do
        byte_size(payload) > ZeroDDS.Reliable.max_payload() ->
          {:error, :too_large, s}

        map_size(s.in_flight) >= ZeroDDS.Reliable.sender_window() ->
          {:error, :window_full, s}

        true ->
          seq = s.next_seq

          s = %{
            s
            | in_flight: Map.put(s.in_flight, seq, payload),
              next_seq: Integer.mod(seq + 1, 0x10000)
          }

          {:ok, seq, s}
      end
    end

    @doc """
    `{HeartbeatBody | nil, sender}` — fires (and rate-limits) a HEARTBEAT if
    in-flight samples exist and the period elapsed.
    """
    def pending_heartbeat(s, now_ms) do
      if map_size(s.in_flight) == 0 do
        {nil, s}
      else
        due =
          is_nil(s.last_heartbeat_ms) or
            now_ms - s.last_heartbeat_ms >= ZeroDDS.Reliable.heartbeat_period_ms()

        if due do
          [k0 | rest] = Map.keys(s.in_flight)
          # RFC-1982 window base (oldest unacked) + end (newest unacked); NOT the
          # numeric Enum.min/max, which is wrong across a 16-bit wrap: window
          # 0xFFFE,0xFFFF,0x0000,0x0001 -> base 0xFFFE / end 0x0001, not 0x0000 /
          # 0xFFFF. Mirrors window_base / serial_max_in_flight in crates/xrce.
          {first, last} =
            Enum.reduce(rest, {k0, k0}, fn k, {mn, mx} ->
              mn = if ZeroDDS.Reliable.seq_lt(k, mn), do: k, else: mn
              mx = if ZeroDDS.Reliable.seq_gt(k, mx), do: k, else: mx
              {mn, mx}
            end)

          hb = %{
            first: first,
            last: last,
            stream_id: ZeroDDS.Reliable.reliable_stream_id()
          }

          {hb, %{s | last_heartbeat_ms: now_ms}}
        else
          {nil, s}
        end
      end
    end

    @doc """
    Processes an incoming ACKNACK: everything strictly before `first_unacked`
    (RFC-1982) is acknowledged and dropped; within `[first_unacked,
    first_unacked+16)` a clear bit means acked (drop), a set bit means still
    missing (keep, for retransmit).
    """
    def recv_acknack(s, %{first_unacked: base, nack_bitmap: bitmap}) do
      in_flight =
        s.in_flight
        |> Enum.reject(fn {k, _v} ->
          diff = Integer.mod(base - k, 0x10000)
          diff != 0 and diff < 0x8000
        end)
        |> Map.new()

      in_flight =
        Enum.reduce(0..15, in_flight, fn i, acc ->
          seq = Integer.mod(base + i, 0x10000)
          bit = bitmap |> bsr(i) |> band(1)
          if bit == 0, do: Map.delete(acc, seq), else: acc
        end)

      %{s | in_flight: in_flight}
    end

    def reset(_s), do: new()
  end

  # --- receiver ---

  defmodule Receiver do
    @moduledoc """
    Receiver-side reliable state (immutable). Mirrors `ReliableStreamState`'s
    receiver half in `reliable.rs`.
    """

    import Bitwise

    defstruct expected: 0, received: %{}

    def new, do: %__MODULE__{}

    def expected(r), do: r.expected
    def out_of_order_count(r), do: map_size(r.received)

    @doc """
    Buffers an incoming sample (or silently accepts a duplicate as a no-op).
    `{:ok, receiver}`, or `{:error, :buffer_full, receiver}` (DoS bound).
    """
    def recv_data(r, seq, payload) do
      cond do
        ZeroDDS.Reliable.seq_lt(seq, r.expected) -> {:ok, r}
        Map.has_key?(r.received, seq) -> {:ok, r}
        map_size(r.received) >= ZeroDDS.Reliable.receiver_buffer() -> {:error, :buffer_full, r}
        true -> {:ok, %{r | received: Map.put(r.received, seq, payload)}}
      end
    end

    @doc "Delivers contiguously from `expected`, advancing it. `{[{seq, payload}, ...], receiver}`."
    def drain_in_order(r), do: drain_in_order(r, [])

    defp drain_in_order(r, acc) do
      case Map.pop(r.received, r.expected) do
        {nil, _received} ->
          {Enum.reverse(acc), r}

        {payload, received} ->
          delivered_seq = r.expected
          r = %{r | received: received, expected: Integer.mod(r.expected + 1, 0x10000)}
          drain_in_order(r, [{delivered_seq, payload} | acc])
      end
    end

    @doc """
    ACKNACK payload for the current window: a set bit means the slot is
    missing. `hint_last_seen` (a HEARTBEAT's `last`, or `nil`) bounds the
    window so slots beyond it are not marked missing.
    """
    def pending_acknack(r, hint_last_seen) do
      base = r.expected

      bitmap =
        Enum.reduce(0..15, 0, fn i, acc ->
          seq = Integer.mod(base + i, 0x10000)
          skip = not is_nil(hint_last_seen) and ZeroDDS.Reliable.seq_gt(seq, hint_last_seen)
          missing = not Map.has_key?(r.received, seq)
          if not skip and missing, do: bor(acc, bsl(1, i)), else: acc
        end)

      %{
        first_unacked: base,
        nack_bitmap: bitmap,
        stream_id: ZeroDDS.Reliable.reliable_stream_id()
      }
    end

    def reset(_r), do: new()
  end
end

defmodule ZeroDDS.Reliable.Drain do
  @moduledoc """
  Async-decoupled reliable sender: a GenServer that owns the `Sender` state
  and the `:gen_udp` socket. The producer `submit/2`s (a `cast`) and returns
  immediately — this IS the BEAM decoupling mechanism: message-passing, not a
  wait-free ring. `submit/2` pays one mailbox send, not a socket syscall; the
  drain process does the submit-into-history + WRITE_DATA send + periodic
  HEARTBEAT + on-ACKNACK retransmit off the producer's path.

  Ownership gotcha: `:gen_udp.controlling_process/2` must be called by the
  socket's *current* owner. So the caller opens the socket, starts this
  GenServer (whose `init/1` merely stashes it — it does not touch the socket
  yet), THEN calls `controlling_process(socket, drain_pid)`, THEN
  `activate/1` to hand the drain process permission to flip `active: true`
  and start its heartbeat/retransmit ticks. See `reliable_app.exs`.
  """

  use GenServer

  alias ZeroDDS.Reliable
  alias ZeroDDS.Reliable.Sender

  @heartbeat_tick_ms 50

  defstruct socket: nil, sender: nil, msg_seq: 1, closed: false, waiters: [], pending: nil

  # --- client API ---

  def start_link(socket), do: GenServer.start_link(__MODULE__, socket)

  @doc """
  Hands the drain process permission to activate the (already
  `controlling_process`-transferred) socket and start ticking. Blocks
  (`call`) until done, so the caller can safely `submit/2` right after.
  """
  def activate(pid), do: GenServer.call(pid, :activate)

  @doc "Enqueues `payload` for reliable delivery. Returns immediately — a `cast`, no I/O on the caller."
  def submit(pid, payload), do: GenServer.cast(pid, {:submit, payload})

  @doc "No more samples; blocks until the send window has fully drained (all acked)."
  def finish(pid, timeout \\ 30_000), do: GenServer.call(pid, :finish, timeout)

  # --- server callbacks ---

  @impl true
  def init(socket) do
    {:ok, %__MODULE__{socket: socket, sender: Sender.new(), pending: :queue.new()}}
  end

  @impl true
  def handle_call(:activate, _from, state) do
    :ok = :inet.setopts(state.socket, active: true)
    Process.send_after(self(), :tick, @heartbeat_tick_ms)
    {:reply, :ok, state}
  end

  def handle_call(:finish, from, state) do
    state = %{state | closed: true, waiters: [from | state.waiters]}
    {:noreply, maybe_finish(state)}
  end

  @impl true
  def handle_cast({:submit, payload}, state) do
    state = %{state | pending: :queue.in(payload, state.pending)}
    {:noreply, maybe_finish(drain_pending(state))}
  end

  @impl true
  def handle_info(:tick, state) do
    state = maybe_send_heartbeat(state)
    state = drain_pending(state)
    Process.send_after(self(), :tick, @heartbeat_tick_ms)
    {:noreply, maybe_finish(state)}
  end

  def handle_info({:udp, socket, _ip, _port, data}, %{socket: socket} = state) do
    state =
      case Reliable.parse_acknack(data) do
        {:ok, ack} ->
          state = %{state | sender: Sender.recv_acknack(state.sender, ack)}
          retransmit_all(state)
          state

        :error ->
          state
      end

    {:noreply, maybe_finish(drain_pending(state))}
  end

  # udp errors (e.g. a transient :econnrefused from an unread peer) — ignore.
  def handle_info({:udp_error, socket, _reason}, %{socket: socket} = state), do: {:noreply, state}

  # --- internals ---

  # FIFO-drains `pending` into the Sender until the window is full — preserves
  # submission order (no timer-based retry races).
  defp drain_pending(state) do
    case :queue.out(state.pending) do
      {{:value, payload}, rest} ->
        case Sender.submit(state.sender, payload) do
          {:ok, seq, sender} ->
            _ = :gen_udp.send(state.socket, Reliable.write_frame(seq, payload))
            drain_pending(%{state | sender: sender, pending: rest})

          {:error, :window_full, sender} ->
            %{state | sender: sender, pending: :queue.in_r(payload, rest)}

          {:error, :too_large, sender} ->
            IO.puts(
              :stderr,
              "ZeroDDS.Reliable.Drain: dropping oversized payload (#{byte_size(payload)} bytes)"
            )

            drain_pending(%{state | sender: sender, pending: rest})
        end

      {:empty, _pending} ->
        state
    end
  end

  defp maybe_send_heartbeat(state) do
    {hb, sender} = Sender.pending_heartbeat(state.sender, System.monotonic_time(:millisecond))
    state = %{state | sender: sender}

    case hb do
      nil ->
        state

      hb ->
        frame =
          Reliable.heartbeat_frame(
            Reliable.session_nokey(),
            Reliable.stream_none(),
            state.msg_seq,
            hb.first,
            hb.last,
            hb.stream_id
          )

        _ = :gen_udp.send(state.socket, frame)
        %{state | msg_seq: Integer.mod(state.msg_seq + 1, 0x10000)}
    end
  end

  defp retransmit_all(state) do
    Enum.each(Sender.in_flight_pairs(state.sender), fn {seq, payload} ->
      _ = :gen_udp.send(state.socket, Reliable.write_frame(seq, payload))
    end)
  end

  defp maybe_finish(state) do
    if state.closed and :queue.is_empty(state.pending) and
         Sender.in_flight_count(state.sender) == 0 do
      Enum.each(state.waiters, &GenServer.reply(&1, :ok))
      %{state | waiters: []}
    else
      state
    end
  end
end
