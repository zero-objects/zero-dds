# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
defmodule ZeroddsWire.Json do
  @moduledoc """
  The little JSON the probe's read direction hands back.

  An encoder only, for exactly the terms the generated dumpers produce
  — maps keyed by strings, lists, strings, integers, floats and
  booleans — in the value dialect `crates/wire-probe/src/lib.rs`
  documents. Not canonical form: the caller canonicalises in Rust, the
  same way it does the C and Python columns' output, so key order and
  float spelling are whatever Elixir produces.

  Its own forty lines rather than the standard library's `JSON`, which
  arrived in Elixir 1.18: the probe should not gain a version floor for
  one call, and an encoder this small is auditable against the dialect
  it exists for. A term outside the dialect raises naming itself —
  encoding it as something plausible is the substitution class this
  project is arranged against.
  """

  @doc "Encode one term of the value dialect."
  def encode(nil), do: raise(ArgumentError, "null is not part of the value dialect")
  def encode(true), do: "true"
  def encode(false), do: "false"
  def encode(value) when is_integer(value), do: Integer.to_string(value)

  def encode(value) when is_float(value) do
    # Elixir's own shortest round-trip form; a whole number keeps its
    # point, so the term stays a float on the way back in.
    :erlang.float_to_binary(value, [:short])
  end

  def encode(value) when is_binary(value), do: encode_string(value)

  def encode(value) when is_list(value) do
    "[" <> Enum.map_join(value, ",", &encode/1) <> "]"
  end

  def encode(value) when is_map(value) and not is_struct(value) do
    inner =
      Enum.map_join(value, ",", fn
        {key, entry} when is_binary(key) -> encode_string(key) <> ":" <> encode(entry)
        {key, _} -> raise ArgumentError, "a JSON key is a string, not #{inspect(key)}"
      end)

    "{" <> inner <> "}"
  end

  def encode(other), do: raise(ArgumentError, "#{inspect(other)} has no JSON form")

  defp encode_string(text) do
    escaped =
      text
      |> String.to_charlist()
      |> Enum.map_join("", fn
        ?" -> "\\\""
        ?\\ -> "\\\\"
        ?\n -> "\\n"
        ?\r -> "\\r"
        ?\t -> "\\t"
        point when point < 0x20 ->
          "\\u" <> String.pad_leading(Integer.to_string(point, 16), 4, "0")

        point ->
          <<point::utf8>>
      end)

    "\"" <> escaped <> "\""
  end
end
