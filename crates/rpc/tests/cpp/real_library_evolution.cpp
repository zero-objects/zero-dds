// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// DDS-RPC 1.0 §7.7.2.5, interface evolution of the operations and their
// parameters, with the generated clients and services of
// `dds/rpc/zerodds.hpp` linked against the real C library (libzerodds): the
// endpoints announce the TypeObjects of their types, so a client and a
// service match by XTypes assignability, not by type name. The return types
// (§7.7.2.5.5) are in real_library_return_types.cpp. Two runtimes on one
// domain.

#include <algorithm>
#include "evolution.hpp"

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

class NewImpl : public ev_new::Svc {
 public:
  std::int32_t add(std::int32_t a, std::int32_t b) override { return a + b; }
  std::int32_t extra() override { return 9; }
  void mix(std::int32_t y, std::int32_t x) override {
    mixed_x = x;
    mixed_y = y;
  }
  std::int32_t fail() override {
    ev_new::E2 refusal;
    refusal.code = 22;
    throw refusal;
  }
  std::atomic<std::int32_t> mixed_x{0};
  std::atomic<std::int32_t> mixed_y{0};
};

class ChangedImpl : public ev_changed::Svc {
 public:
  std::int32_t add(const std::string&, std::int32_t b) override { return b; }
  std::int32_t legacy() override { return 7; }
  void mix(std::int32_t, std::int32_t) override {}
  std::int32_t fail() override { return 0; }
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
    // §7.7.2.5: the endpoints announce the TypeObjects of their types, so the
    // match follows assignability and not the type name (`ev_old::` against
    // `ev_new::`).
    NewImpl new_impl;
    auto new_service = std::make_shared<ev_new::SvcService>(
        new_impl, evolution_topics(server_runtime.service_params(), "Evolution"));
    dds::rpc::add(server, new_service);
    ev_old::SvcClient old_client(evolution_topics(client_runtime.client_params(), "Evolution"));
    check(old_client.wait_for_service(std::chrono::seconds(10)),
          "an old client finds the new service (assignable types)");
    check(old_client.add(2, 3) == 5, "an operation both versions have is served");
    old_client.mix(1, 2);
    check(new_impl.mixed_x == 1 && new_impl.mixed_y == 2,
          "a @Mutable operation reaches its parameters by name");
    bool unknown_operation = false;
    try {
      (void)old_client.legacy();
    } catch (const dds::rpc::RemoteUnknownOperationError&) {
      unknown_operation = true;
    }
    check(unknown_operation, "an operation the new version removed is REMOTE_EX_UNKNOWN_OPERATION");

    // §7.7.2.5.3: a parameter that changed its type to one the old type is
    // not assignable to: the endpoints do not match.
    ChangedImpl changed_impl;
    auto changed_service = std::make_shared<ev_changed::SvcService>(
        changed_impl, evolution_topics(server_runtime.service_params(), "Changed"));
    dds::rpc::add(server, changed_service);
    ev_old::SvcClient mismatched(evolution_topics(client_runtime.client_params(), "Changed"));
    check(!mismatched.wait_for_service(std::chrono::seconds(3)),
          "a client does not find a service whose parameter type is not assignable");

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
