# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Runnable example for the native Elixir endpoint: sync (poll) and async
# (process/mailbox). Run with `elixir -r lib/zerodds.ex example.exs`.

sample = fn id, label ->
  ZeroDDS.Wire.writer(:little)
  |> ZeroDDS.Wire.put_u32(id)
  |> ZeroDDS.Wire.put_string(label)
  |> ZeroDDS.Wire.bytes()
end

# --- sync ---
t = ZeroDDS.MemTransport.new()
c = ZeroDDS.Client.new(t)
ZeroDDS.Client.write(c, sample.(0x42, "sync-hello"))
body = ZeroDDS.Client.poll(c)
{id, _} = ZeroDDS.Wire.reader(body, :little) |> ZeroDDS.Wire.get_u32()
IO.puts("sync: received id=0x#{Integer.to_string(id, 16)}")

# --- async (process/mailbox) ---
t2 = ZeroDDS.MemTransport.new()
w = ZeroDDS.Client.new(t2)
Enum.reduce(0..2, w, fn i, w -> ZeroDDS.Client.write(w, sample.(0x100 + i, "async")) end)

reader = ZeroDDS.AsyncReader.start(t2, self())

for _ <- 0..2 do
  receive do
    {:zerodds_sample, b} ->
      {id, _} = ZeroDDS.Wire.reader(b, :little) |> ZeroDDS.Wire.get_u32()
      IO.puts("async: received id=0x#{Integer.to_string(id, 16)}")
  after
    2000 -> raise "async: timeout"
  end
end

ZeroDDS.AsyncReader.stop(reader)
IO.puts("ALL OK")
