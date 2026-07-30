# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Deeper SYNC example for the native Elixir endpoint: a sensor-telemetry
# publisher writes typed Reading samples; a subscriber polls and decodes every
# field. Run: `elixir -r lib/zerodds.ex example_sync.exs`

alias ZeroDDS.{Wire, Client, MemTransport}

# Reading mirrors an IDL `@final struct Reading { uint32 id; float value; string label; }`.
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

# Publisher: frame + deliver 5 typed readings with varying values.
c =
  Enum.reduce(0..(total - 1), c, fn i, c ->
    r = %{id: 0x1000 + i, value: 20.0 + i * 0.5, label: "bay-#{String.pad_leading("#{i}", 2, "0")}"}
    Client.write(c, marshal.(r, :little))
  end)

# Subscriber: poll; decode every field; stop at total.
poll_loop = fn poll_loop, got ->
  if got >= total do
    got
  else
    case Client.poll(c) do
      nil ->
        got

      body ->
        r = decode.(body)
        IO.puts(
          "sync reading #{got}: id=0x#{Integer.to_string(r.id, 16)} " <>
            "value=#{Float.round(r.value, 1)} label=\"#{r.label}\""
        )

        poll_loop.(poll_loop, got + 1)
    end
  end
end

got = poll_loop.(poll_loop, 0)
if got != total, do: raise("incomplete: #{got}/#{total}")
IO.puts("ALL OK")
