// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// DDS-RPC 1.0 §7.7.2.5.5, the return type of an operation evolving, with the
// generated clients and services of `dds/rpc/zerodds.hpp` linked against the
// real C library (libzerodds): a return type that gained a member is
// assignable, one widened from `long` to `long long` is not. Two runtimes on
// one domain.

#include <algorithm>
#include "return_types.hpp"

#include <dds/rpc.hpp>
#include <dds/rpc/zerodds.hpp>

#include <atomic>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <memory>
#include <stdexcept>
#include <string>
#include <thread>

namespace {

int failures = 0;

void check(bool held, const char* what) {
  if (!held) {
    std::fprintf(stderr, "FAIL %s\n", what);
    ++failures;
  }
}

class RetNewImpl : public ev_ret_new::Svc {
 public:
  void get(ev_ret_new::Pair& cxx_return) override {
    cxx_return.a(5);
    cxx_return.b(6);
  }
};

class WideNewImpl : public ev_wide_new::Svc {
 public:
  std::int64_t value() override { return 1LL << 40; }
};

template <class Params>
Params evolution_topics(Params params, const char* name) {
  params.request_topic_name(std::string(name) + "_Request")
      .reply_topic_name(std::string(name) + "_Reply");
  return params;
}

}  // namespace

int main(int argc, char** argv) {
  // The domain the test leased for this run (crates/test-domains): the
  // domain number is the UDP port, a resource of the whole host.
  if (argc != 2) {
    std::fprintf(stderr, "usage: %s <domain>\n", argv[0]);
    return 2;
  }
  const std::uint32_t domain = static_cast<std::uint32_t>(std::strtoul(argv[1], nullptr, 10));
  auto server_side = dds::rpc::zerodds::participant_on(domain);
  auto client_side = dds::rpc::zerodds::participant_on(domain);
  dds::rpc::RPCRuntime server_runtime(server_side);
  dds::rpc::RPCRuntime client_runtime(client_side);

  dds::rpc::Server server;
  std::atomic<bool> stop{false};
  std::thread serving([&]() {
    while (!stop) {
      if (dds::rpc::run_once(server) == 0) {
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
      }
    }
  });

  try {
    // §7.7.2.5.5: a return type that gained a member is assignable, so the
    // old client reads the reply of the new service; one that widened from
    // `long` to `long long` is not, and the endpoints do not match.
    RetNewImpl ret_impl;
    auto ret_service = std::make_shared<ev_ret_new::SvcService>(
        ret_impl, evolution_topics(server_runtime.service_params(), "Return"));
    dds::rpc::add(server, ret_service);
    ev_ret_old::SvcClient ret_client(evolution_topics(client_runtime.client_params(), "Return"));
    check(ret_client.wait_for_service(std::chrono::seconds(10)),
          "an old client finds the service whose return type gained a member");
    ev_ret_old::Pair pair;
    ret_client.get(pair);
    check(pair.a() == 5, "the old client reads the member it knows from the new reply");
    WideNewImpl wide_impl;
    auto wide_service = std::make_shared<ev_wide_new::SvcService>(
        wide_impl, evolution_topics(server_runtime.service_params(), "Wide"));
    dds::rpc::add(server, wide_service);
    ev_wide_old::SvcClient wide_client(evolution_topics(client_runtime.client_params(), "Wide"));
    check(!wide_client.wait_for_service(std::chrono::seconds(3)),
          "a client does not find a service whose return type widened from long to long long");
  } catch (const std::exception& error) {
    std::fprintf(stderr, "FAIL exception: %s\n", error.what());
    ++failures;
  }

  stop = true;
  serving.join();
  if (failures == 0) {
    std::printf("every check held\n");
    return 0;
  }
  return 1;
}
