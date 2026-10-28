// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Interface evolution in the Enhanced Service Mapping (DDS-RPC 1.0 §7.7.2)
// with the generated C++ clients and services of two versions of one
// interface, over the in-memory binding of `loopback.hpp`. The old and the
// new version share the two topic names (§7.7: "as long as the same topic
// names are used"); they differ in the operations, in the order of the
// parameters of a @Mutable operation and in the exceptions an operation
// raises.
//
//   - §7.7.2.1, §7.7.2.2: an operation one side lacks is answered
//     REMOTE_EX_UNKNOWN_OPERATION; the others are served;
//   - §7.7.2.5.2: the parameters of a @Mutable operation are matched by
//     member id, so a reordering does not change which value reaches which
//     parameter;
//   - §7.7.2.5.6: an exception the client does not know is
//     REMOTE_EX_UNKNOWN_EXCEPTION; one both know is the user exception.

#include "calc.hpp"
#include "loopback.hpp"

#include <atomic>
#include <chrono>
#include <cstdio>
#include <thread>

namespace {

int failures = 0;

void check(bool held, const char* what) {
  if (!held) {
    std::fprintf(stderr, "FAIL %s\n", what);
    ++failures;
  }
}

constexpr const char* kRequestTopic = "Evolution_Service_Request";
constexpr const char* kReplyTopic = "Evolution_Service_Reply";

class OldImpl : public ev_old::Svc {
 public:
  std::int32_t add(std::int32_t a, std::int32_t b) override { return a + b; }
  std::int32_t legacy() override { return 7; }
  void mix(std::int32_t x, std::int32_t y) override {
    mixed_x = x;
    mixed_y = y;
  }
  std::int32_t fail() override {
    ev_old::E1 refusal;
    refusal.code = 11;
    throw refusal;
  }
  std::atomic<std::int32_t> mixed_x{0};
  std::atomic<std::int32_t> mixed_y{0};
};

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

/// Runs `server` on a thread until it goes out of scope.
class Serving {
 public:
  explicit Serving(dds::rpc::Server& server) : server_(server), thread_([this]() {
    while (!stop_) {
      if (dds::rpc::run_once(server_) == 0) {
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
      }
    }
  }) {}
  ~Serving() {
    stop_ = true;
    thread_.join();
  }

 private:
  dds::rpc::Server& server_;
  std::atomic<bool> stop_{false};
  std::thread thread_;
};

template <class Params>
Params shared_topics(Params params) {
  params.request_topic_name(kRequestTopic).reply_topic_name(kReplyTopic);
  return params;
}

/// An old client against a new service.
void old_client_new_service() {
  Bus bus;
  bus.per_reader_queues = true;
  bus.expose_identity = true;
  auto participant = std::make_shared<LoopbackParticipant>(bus);
  dds::rpc::RPCRuntime runtime(participant);
  NewImpl implementation;
  auto service = std::make_shared<ev_new::SvcService>(
      implementation, shared_topics(runtime.service_params()));
  dds::rpc::Server server;
  dds::rpc::add(server, service);
  Serving serving(server);
  ev_old::SvcClient client(shared_topics(runtime.client_params()));
  check(client.wait_for_service(std::chrono::seconds(2)), "the old client finds the new service");

  check(client.add(2, 3) == 5, "§7.7.2: an operation both versions have is served: add(2, 3) = 5");
  bool unknown_operation = false;
  try {
    (void)client.legacy();
  } catch (const dds::rpc::RemoteUnknownOperationError&) {
    unknown_operation = true;
  }
  check(unknown_operation,
        "§7.7.2.2: an operation the new version removed is REMOTE_EX_UNKNOWN_OPERATION");
  client.mix(1, 2);
  check(implementation.mixed_x == 1 && implementation.mixed_y == 2,
        "§7.7.2.5.2: a @Mutable operation reaches its parameters by name, not by position");
  bool unknown_exception = false;
  try {
    (void)client.fail();
  } catch (const dds::rpc::RemoteUnknownExceptionError&) {
    unknown_exception = true;
  }
  check(unknown_exception,
        "§7.7.2.5.6: an exception the old client does not know is REMOTE_EX_UNKNOWN_EXCEPTION");
}

/// A new client against an old service.
void new_client_old_service() {
  Bus bus;
  bus.per_reader_queues = true;
  bus.expose_identity = true;
  auto participant = std::make_shared<LoopbackParticipant>(bus);
  dds::rpc::RPCRuntime runtime(participant);
  OldImpl implementation;
  auto service = std::make_shared<ev_old::SvcService>(
      implementation, shared_topics(runtime.service_params()));
  dds::rpc::Server server;
  dds::rpc::add(server, service);
  Serving serving(server);
  ev_new::SvcClient client(shared_topics(runtime.client_params()));
  check(client.wait_for_service(std::chrono::seconds(2)), "the new client finds the old service");

  check(client.add(4, 5) == 9, "add(4, 5) = 9 against the old service");
  bool unknown_operation = false;
  try {
    (void)client.extra();
  } catch (const dds::rpc::RemoteUnknownOperationError&) {
    unknown_operation = true;
  }
  check(unknown_operation,
        "§7.7.2.1: an operation the old service lacks is REMOTE_EX_UNKNOWN_OPERATION");
  client.mix(10, 20);
  check(implementation.mixed_y == 10 && implementation.mixed_x == 20,
        "§7.7.2.5.2: the new client's (y = 10, x = 20) reach the old service's x and y by name");
  bool user_exception = false;
  try {
    (void)client.fail();
  } catch (const ev_new::E1& error) {
    user_exception = error.code == 11;
  }
  check(user_exception,
        "§7.7.2.5.6: an exception both versions declare is the user exception");
}

}  // namespace

int main() {
  old_client_new_service();
  new_client_old_service();
  if (failures == 0) {
    std::puts("every check held");
  }
  return failures == 0 ? 0 : 1;
}
