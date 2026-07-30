# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Deeper ASYNC example for the native Elixir endpoint: the same telemetry
# publisher, but the subscriber consumes via a process/mailbox (BEAM idiom) and
# decodes every field. Run: `elixir -r lib/zerodds.ex example_async.exs`

alias ZeroDDS.{Wire, Client, MemTransport, AsyncReader}

marshal = fn r, endian ->
  Wire.writer(endian)
  |> Wire.put_u32(r.id)
  |> Wire.put_f32(r.value)
  |> Wire.put_string(r.label)
  |> Wire.bytes()
end

decode = fn body ->
  r = Wire.reader(body, :little)
  {id, r} = Wire.get_u32(r)
  {value, r} = Wire.get_f32(r)
  {label, _r} = Wire.get_string(r)
  %{id: id, value: value, label: label}
end

total = 5
t = MemTransport.new()
c = Client.new(t)

# Publisher.
Enum.reduce(0..(total - 1), c, fn i, c ->
  r = %{id: 0x2000 + i, value: 100.0 - i, label: "sensor-#{String.pad_leading("#{i}", 2, "0")}"}
  Client.write(c, marshal.(r, :little))
end)

# Subscriber: an AsyncReader process sends {:zerodds_sample, body} to our mailbox.
reader = AsyncReader.start(t, self())

Enum.each(0..(total - 1), fn got ->
  receive do
    {:zerodds_sample, body} ->
      r = decode.(body)

      IO.puts(
        "async reading #{got}: id=0x#{Integer.to_string(r.id, 16)} " <>
          "value=#{Float.round(r.value, 1)} label=\"#{r.label}\""
      )
  after
    2000 -> raise("timeout waiting for sample #{got}")
  end
end)

AsyncReader.stop(reader)
IO.puts("ALL OK")
