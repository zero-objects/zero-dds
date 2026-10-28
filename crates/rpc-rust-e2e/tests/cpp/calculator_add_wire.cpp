// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The same two DDS-RPC 1.0 payloads
// `crates/rpc-rust-e2e/tests/wire_bytes_match_cpp.rs` gets from the
// generated Rust `Calculator_Call`/`Calculator_Return` types, here from the
// codec the C++ generator writes, so the two can be compared byte for byte.
//
#include "calculator.hpp"

#include <cstdint>
#include <cstdio>
#include <vector>

namespace {

void print_hex(const std::vector<std::uint8_t>& bytes) {
  for (std::uint8_t octet : bytes) {
    std::printf("%02x", octet);
  }
  std::printf("\n");
}

}  // namespace

int main() {
  // Calculator_Call::Add(Calculator_add_In{a: 3, b: 4}), through the codec
  // the generator wrote for the request type: the union's discriminator
  // (§7.5.1.1.5 rule 5a), then the `_In` structure's members in
  // declaration order (§7.5.1.1.4).
  robot::Calculator_Request request{};
  request.data._d = robot::Calculator_add_Hash;
  request.data.add.a = 3;
  request.data.add.b = 4;
  print_hex(dds::rpc::dds_type_traits<robot::Calculator_Request>::serialize(request));

  // Calculator_Return::Add(Calculator_add_Result::Ok(Calculator_add_Out{
  // return_: 7})).
  robot::Calculator_Reply reply{};
  reply.data._d = robot::Calculator_add_Hash;
  reply.data.add._d = dds::rpc::RETCODE_OK;
  reply.data.add.result.return_ = 7;
  print_hex(dds::rpc::dds_type_traits<robot::Calculator_Reply>::serialize(reply));
  return 0;
}
