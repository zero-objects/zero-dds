// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The samples `crates/rpc-rust-e2e/tests/wire_bytes_match_columns.rs` holds
// against the Rust and the Java column: header, padding and the union, for
// operations with members of eight octets, under classic CDR (XCDR1) and
// XCDR2, behind instance names of several lengths. One line per sample:
// `<kind> <encoding> <name length> <operation> <hex>`.
//
// Also nested `@appendable` and `@mutable` members, in the topic types of
// an interface (`Store`) and in a pair of types (`PReq`, `PRep`): classic
// CDR has no DHEADER for the former and a parameter list for the latter.
//
#include "wide.hpp"

#include <cstdint>
#include <cstdio>
#include <string>
#include <utility>
#include <vector>

namespace {

using Octets = std::vector<std::uint8_t>;

dds::SampleIdentity identity() {
  dds::SampleIdentity id;
  for (std::size_t at = 0; at < 16; ++at) {
    id.writer_guid[at] = 0x42;
  }
  id.sequence_number.high = 0x01020304;
  id.sequence_number.low = 0x05060708;
  return id;
}

void print_line(const char* kind, const char* encoding, std::size_t name_length,
                const char* operation, const Octets& header, const Octets& union_octets) {
  std::printf("%s %s %zu %s ", kind, encoding, name_length, operation);
  for (std::uint8_t octet : header) {
    std::printf("%02x", octet);
  }
  for (std::uint8_t octet : union_octets) {
    std::printf("%02x", octet);
  }
  std::printf("\n");
}

}  // namespace

int main() {
  const std::size_t name_lengths[] = {0, 1, 3, 4, 5};
  const std::pair<const char*, dds::rpc::WireEncoding> encodings[] = {
      {"xcdr1", dds::rpc::WireEncoding::XCDR1}, {"xcdr2", dds::rpc::WireEncoding::XCDR2}};

  for (const auto& [encoding_name, encoding] : encodings) {
    for (std::size_t length : name_lengths) {
      dds::rpc::RequestHeader header;
      header.requestId = identity();
      header.instanceName = std::string(length, 'n');
      Octets octets = dds::rpc::encode_request_header(header);
      octets.resize((octets.size() + 3u) & ~std::size_t{3u}, 0);

      wide::W_Request scale{};
      scale.data._d = wide::W_scale_Hash;
      scale.data.scale.d = 1.5;
      scale.data.scale.n = -2;
      scale.data.scale.u = 0x1122334455667788ULL;
      print_line("request", encoding_name, length, "scale", octets,
                 dds::rpc::dds_type_traits<wide::W_Request>::serialize_at(scale, octets.size(),
                                                                            encoding));

      wide::W_Request mix{};
      mix.data._d = wide::W_mix_Hash;
      mix.data.mix.o = 0x7f;
      mix.data.mix.u = 0x0102030405060708ULL;
      print_line("request", encoding_name, length, "mix", octets,
                 dds::rpc::dds_type_traits<wide::W_Request>::serialize_at(mix, octets.size(),
                                                                            encoding));
    }

    for (std::size_t length : {std::size_t{0}, std::size_t{3}}) {
      dds::rpc::RequestHeader header;
      header.requestId = identity();
      header.instanceName = std::string(length, 'n');
      Octets octets = dds::rpc::encode_request_header(header);
      octets.resize((octets.size() + 3u) & ~std::size_t{3u}, 0);

      nested::Store_Request put{};
      put.data._d = nested::Store_put_Hash;
      put.data.put.a.a = 1;
      put.data.put.a.b = 2.0;
      put.data.put.m.m = 3;
      put.data.put.m.n = 4;
      print_line("request", encoding_name, length, "put", octets,
                 dds::rpc::dds_type_traits<nested::Store_Request>::serialize_at(
                     put, octets.size(), encoding));

      nested::PReq pair{};
      pair.header = header;
      pair.ap.a = 1;
      pair.ap.b = 2.0;
      pair.mu.m = 3;
      pair.mu.n = 4;
      print_line("request", encoding_name, length, "pair", Octets{},
                 dds::rpc::dds_type_traits<nested::PReq>::serialize_at(pair, 0, encoding));
    }

    dds::rpc::ReplyHeader reply_header;
    reply_header.relatedRequestId = identity();
    reply_header.remoteEx = dds::rpc::REMOTE_EX_OK;
    const Octets octets = dds::rpc::encode_reply_header(reply_header);

    wide::W_Reply scale{};
    scale.data._d = wide::W_scale_Hash;
    scale.data.scale._d = dds::rpc::RETCODE_OK;
    scale.data.scale.result.return_ = 3.0;
    print_line("reply", encoding_name, 0, "scale", octets,
               dds::rpc::dds_type_traits<wide::W_Reply>::serialize_at(scale, octets.size(),
                                                                       encoding));

    wide::W_Reply mix{};
    mix.data._d = wide::W_mix_Hash;
    mix.data.mix._d = dds::rpc::RETCODE_OK;
    mix.data.mix.result.return_ = 0xAABBCCDDEEFF0011ULL;
    print_line("reply", encoding_name, 0, "mix", octets,
               dds::rpc::dds_type_traits<wide::W_Reply>::serialize_at(mix, octets.size(),
                                                                       encoding));

    nested::Store_Reply get{};
    get.data._d = nested::Store_get_Hash;
    get.data.get._d = dds::rpc::RETCODE_OK;
    get.data.get.result.return_.m = 3;
    get.data.get.result.return_.n = 4;
    print_line("reply", encoding_name, 0, "get", octets,
               dds::rpc::dds_type_traits<nested::Store_Reply>::serialize_at(get, octets.size(),
                                                                             encoding));

    nested::PRep pair_reply{};
    pair_reply.header = reply_header;
    pair_reply.ap.a = 1;
    pair_reply.ap.b = 2.0;
    pair_reply.mu.m = 3;
    pair_reply.mu.n = 4;
    print_line("reply", encoding_name, 0, "pair", Octets{},
               dds::rpc::dds_type_traits<nested::PRep>::serialize_at(pair_reply, 0, encoding));
  }
  return 0;
}
