# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
defmodule ZeroddsWire.JsonTest do
  use ExUnit.Case, async: true

  alias ZeroddsWire.Json

  test "the dialect's terms encode" do
    assert Json.encode(%{"a" => 1, "b" => [true, false]}) in [
             ~s({"a":1,"b":[true,false]}),
             ~s({"b":[true,false],"a":1})
           ]

    assert Json.encode("été\n\"x\"") == ~s("été\\n\\"x\\"")
    assert Json.encode(1.5) == "1.5"
    assert Json.encode(-0.0) in ["-0.0", "-0.0"]
    assert Json.encode(3.141592653589793) == "3.141592653589793"
    assert Json.encode("16045690984833335998") == ~s("16045690984833335998")
  end

  test "a term outside the dialect is refused by name" do
    assert_raise ArgumentError, fn -> Json.encode(nil) end
    assert_raise ArgumentError, fn -> Json.encode(:nan) end
    assert_raise ArgumentError, fn -> Json.encode(%{1 => 2}) end
  end

  test "a control character escapes as its unicode form" do
    assert Json.encode(<<1>>) == ~s("\\u0001")
  end
end
