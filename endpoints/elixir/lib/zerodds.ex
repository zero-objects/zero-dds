# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Native pure-Elixir endpoint SDK (ADR 0013): a from-scratch XCDR wire-core on
# the BEAM, byte-identical to the Rust core and the other SDKs. Sync (receive
# with a timeout) and async (a process/mailbox — the idiomatic OTP model).
#
# No native addon, no NIF: Elixir binaries + bitstring syntax carry the wire.

defmodule ZeroDDS.Wire do
  @moduledoc "XCDR writer/reader. Alignment relative to buffer start, cap 4."

  # --- Writer: a %{buf, endian} accumulator (endian :little | :big) ---

  def writer(endian), do: %{buf: <<>>, endian: endian}

  defp align(w, a) do
    cap = min(a, 4)
    pad = rem(cap - rem(byte_size(w.buf), cap), cap)
    %{w | buf: w.buf <> :binary.copy(<<0>>, pad)}
  end

  # le: the value already encoded little-endian; reversed for :big.
  defp put(w, a, le) do
    w = align(w, a)
    bytes = if w.endian == :big, do: rev(le), else: le
    %{w | buf: w.buf <> bytes}
  end

  defp rev(bin), do: bin |> :binary.bin_to_list() |> Enum.reverse() |> :binary.list_to_bin()

  def put_u8(w, v), do: %{w | buf: w.buf <> <<v::8>>}
  def put_u16(w, v), do: put(w, 2, <<v::little-16>>)
  def put_u32(w, v), do: put(w, 4, <<v::little-32>>)
  def put_u64(w, v), do: put(w, 4, <<v::little-64>>)
  def put_f32(w, v), do: put(w, 4, <<v::float-little-32>>)
  def put_bytes(w, b), do: %{w | buf: w.buf <> b}

  def put_string(w, s) do
    w = put_u32(w, byte_size(s) + 1)
    w = put_bytes(w, s)
    put_u8(w, 0)
  end

  def put_seq_u8(w, b) do
    w = put_u32(w, byte_size(b))
    put_bytes(w, b)
  end

  def bytes(w), do: w.buf

  # --- Reader: a %{bin, pos, endian} cursor ---

  def reader(bin, endian), do: %{bin: bin, pos: 0, endian: endian}

  defp take(r, a, n) do
    cap = min(a, 4)
    pad = rem(cap - rem(r.pos, cap), cap)
    pos = r.pos + pad
    chunk = binary_part(r.bin, pos, n)
    chunk = if r.endian == :big, do: rev(chunk), else: chunk
    {chunk, %{r | pos: pos + n}}
  end

  def get_u32(r) do
    {chunk, r} = take(r, 4, 4)
    <<v::little-32>> = chunk
    {v, r}
  end

  # --- primitives beyond get_u32 — the byte-exact inverse of the Writer ---

  def get_u8(r) do
    <<v::8>> = binary_part(r.bin, r.pos, 1)
    {v, %{r | pos: r.pos + 1}}
  end

  def get_u16(r) do
    {chunk, r} = take(r, 2, 2)
    <<v::little-16>> = chunk
    {v, r}
  end

  def get_u64(r) do
    {chunk, r} = take(r, 4, 8)
    <<v::little-64>> = chunk
    {v, r}
  end

  def get_f32(r) do
    {chunk, r} = take(r, 4, 4)
    <<v::float-little-32>> = chunk
    {v, r}
  end

  def get_string(r) do
    {n, r} = get_u32(r)
    s = binary_part(r.bin, r.pos, n - 1)
    {s, %{r | pos: r.pos + n}}
  end

  def get_seq_u8(r) do
    {n, r} = get_u32(r)
    b = binary_part(r.bin, r.pos, n)
    {b, %{r | pos: r.pos + n}}
  end
end

defmodule ZeroDDS.Endpoint do
  @moduledoc "XRCE WRITE_DATA framing (8-byte header + sample body)."

  @session_nokey 0x80
  @stream_best_effort 0x01

  def session_nokey, do: @session_nokey
  def stream_best_effort, do: @stream_best_effort

  def write_frame(session, stream, seq, sample) do
    n = byte_size(sample)
    <<session::8, stream::8, seq::little-16, 0x07::8, 0x03::8, n::little-16>> <> sample
  end

  def read_frame(frame)
      when byte_size(frame) >= 8 and binary_part(frame, 4, 1) == <<0x07>> do
    {:ok, binary_part(frame, 8, byte_size(frame) - 8)}
  end

  def read_frame(_), do: :error
end

defmodule ZeroDDS.Client do
  @moduledoc """
  Sync endpoint. `transport` is `%{deliver: fun/1, receive: fun/0}` where
  receive returns a frame binary or nil.
  """

  def new(transport) do
    %{
      transport: transport,
      session: ZeroDDS.Endpoint.session_nokey(),
      stream: ZeroDDS.Endpoint.stream_best_effort(),
      seq: 1
    }
  end

  def write(c, sample) do
    frame = ZeroDDS.Endpoint.write_frame(c.session, c.stream, c.seq, sample)
    c.transport.deliver.(frame)
    %{c | seq: rem(c.seq + 1, 0x10000)}
  end

  # One non-blocking receive: returns the sample body, or nil.
  def poll(c) do
    case c.transport.receive.() do
      nil ->
        nil

      frame ->
        case ZeroDDS.Endpoint.read_frame(frame) do
          {:ok, body} -> body
          :error -> nil
        end
    end
  end
end

defmodule ZeroDDS.AsyncReader do
  @moduledoc """
  Async endpoint, idiomatic BEAM: spawns a process that polls the transport and
  sends `{:zerodds_sample, body}` to `target`. The consumer receives in its own
  mailbox. `stop/1` tells the reader to exit.
  """

  def start(transport, target) do
    spawn(fn -> loop(transport, target) end)
  end

  def stop(pid), do: send(pid, :zerodds_stop)

  defp loop(transport, target) do
    receive do
      :zerodds_stop -> :ok
    after
      0 ->
        case transport.receive.() do
          nil ->
            Process.sleep(1)
            loop(transport, target)

          frame ->
            case ZeroDDS.Endpoint.read_frame(frame) do
              {:ok, body} -> send(target, {:zerodds_sample, body})
              :error -> :ok
            end

            loop(transport, target)
        end
    end
  end
end

defmodule ZeroDDS.MemTransport do
  @moduledoc "In-memory FIFO transport (an Agent) for tests + example."

  def new do
    {:ok, pid} = Agent.start_link(fn -> :queue.new() end)

    %{
      deliver: fn frame ->
        Agent.update(pid, fn q -> :queue.in(frame, q) end)
        true
      end,
      receive: fn ->
        Agent.get_and_update(pid, fn q ->
          case :queue.out(q) do
            {{:value, f}, q2} -> {f, q2}
            {:empty, q2} -> {nil, q2}
          end
        end)
      end
    }
  end
end
