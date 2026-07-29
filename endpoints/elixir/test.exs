# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Tests for the native Elixir endpoint: byte-identity vs the Rust goldens, plus
# sync + async loopback. Run with `elixir -r lib/zerodds.ex test.exs`.

golden_dir = System.get_env("GOLDEN_DIR") || "build"

fixture = fn endian ->
  ZeroDDS.Wire.writer(endian)
  |> ZeroDDS.Wire.put_u32(0xA1B2C3D4)
  |> ZeroDDS.Wire.put_u16(0x1234)
  |> ZeroDDS.Wire.put_u8(0x5A)
  |> ZeroDDS.Wire.put_f32(3.5)
  |> ZeroDDS.Wire.put_u64(0x0102030405060708)
  |> ZeroDDS.Wire.put_string("bay-12")
  |> ZeroDDS.Wire.put_seq_u8(<<0xDE, 0xAD, 0xBE, 0xEF>>)
  |> ZeroDDS.Wire.bytes()
end

sample_body = fn id ->
  ZeroDDS.Wire.writer(:little)
  |> ZeroDDS.Wire.put_u32(id)
  |> ZeroDDS.Wire.put_u16(0)
  |> ZeroDDS.Wire.put_u8(0)
  |> ZeroDDS.Wire.bytes()
end

# --- byte identity ---
for {endian, file} <- [{:little, "golden_le.bin"}, {:big, "golden_be.bin"}] do
  got = fixture.(endian)
  golden = File.read!(Path.join(golden_dir, file))

  if got != golden do
    raise "#{file}: not byte-identical (got #{byte_size(got)}, want #{byte_size(golden)})"
  end

  IO.puts("#{file}: #{byte_size(golden)} bytes byte-identical")
end

# --- sync loopback (receive with a timeout underneath) ---
t = ZeroDDS.MemTransport.new()
c = ZeroDDS.Client.new(t)
c = Enum.reduce(0..4, c, fn i, c -> ZeroDDS.Client.write(c, sample_body.(0x3000 + i)) end)

for i <- 0..4 do
  body = ZeroDDS.Client.poll(c)
  {id, _} = ZeroDDS.Wire.reader(body, :little) |> ZeroDDS.Wire.get_u32()
  if id != 0x3000 + i, do: raise("sync: got 0x#{Integer.to_string(id, 16)} want 0x#{Integer.to_string(0x3000 + i, 16)}")
end

IO.puts("sync loopback: 5 samples OK")

# --- async loopback (process/mailbox) ---
t2 = ZeroDDS.MemTransport.new()
w = ZeroDDS.Client.new(t2)
Enum.reduce(0..4, w, fn i, w -> ZeroDDS.Client.write(w, sample_body.(0x1000 + i)) end)

reader = ZeroDDS.AsyncReader.start(t2, self())

for i <- 0..4 do
  receive do
    {:zerodds_sample, body} ->
      {id, _} = ZeroDDS.Wire.reader(body, :little) |> ZeroDDS.Wire.get_u32()
      if id != 0x1000 + i, do: raise("async: got 0x#{Integer.to_string(id, 16)} want 0x#{Integer.to_string(0x1000 + i, 16)}")
  after
    2000 -> raise("async: timeout waiting for sample #{i}")
  end
end

ZeroDDS.AsyncReader.stop(reader)
IO.puts("async loopback: 5 samples OK")

IO.puts("ALL OK")
