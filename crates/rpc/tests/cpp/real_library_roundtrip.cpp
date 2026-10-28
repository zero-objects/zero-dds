// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The generated function-call client and service of the Calculator interface
// over the ZeroDDS binding of `dds/rpc/zerodds.hpp`, linked against the real
// C library (libzerodds). Two runtimes on one domain, so the requests and
// replies cross the RTPS stack, and discovery (`wait_for_service`, named
// instances, §7.6.2) runs between them.

#include <algorithm>
#include "calculator.hpp"

#include <dds/rpc.hpp>
#include <dds/rpc/zerodds.hpp>

#include <atomic>
#include <chrono>
#include <cstdint>
#include <cstdio>
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

class CalcImpl : public robot::Calculator {
 public:
  explicit CalcImpl(std::int32_t offset = 0) : offset_(offset) {}
  std::int32_t add(std::int32_t a, std::int32_t b) override { return a + b + offset_; }
  void reset() override { total_value = 0; ++resets; }
  std::int32_t div(std::int32_t a, std::int32_t b, std::int32_t& rest) override {
    if (b == 0) {
      robot::TooFast error;
      error.limit = 7;
      error.why = "division by zero";
      throw error;
    }
    rest = a % b;
    return a / b;
  }
  void move(robot::Pose& cxx_return, const robot::Pose& start, std::int32_t& steps) override {
    cxx_return.x(start.x() + steps);
    cxx_return.y(start.y() * 2.0);
    steps += 1;
  }
  void label(std::string& cxx_return, const std::string& prefix) override {
    if (prefix == "boom") {
      throw std::runtime_error("the implementation failed");
    }
    cxx_return = prefix + "!";
  }
  double scale(double factor, std::int64_t big) override { return factor * static_cast<double>(big); }
  std::int32_t risky(std::int32_t x) override {
    robot::cxx_template error;
    error.code = x;
    throw error;
  }
  std::int32_t total() override { return total_value; }
  void total(std::int32_t value) override { total_value = value; }
  robot::Pose& origin() override { return origin_value; }
  const robot::Pose& origin() const override { return origin_value; }

  std::int32_t total_value = 0;
  std::atomic<int> resets{0};
  robot::Pose origin_value{3.5, -1.25};

 private:
  std::int32_t offset_;
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

int main() {
  const std::uint32_t domain = 87;
  auto server_side = dds::rpc::zerodds::participant_on(domain);
  auto client_side = dds::rpc::zerodds::participant_on(domain);
  dds::rpc::RPCRuntime server_runtime(server_side);
  dds::rpc::RPCRuntime client_runtime(client_side);

  CalcImpl implementation;
  CalcImpl implementation_alpha(1000);
  CalcImpl implementation_beta(2000);

  auto service = std::make_shared<robot::CalculatorService>(implementation, server_runtime);
  dds::rpc::ServiceParams alpha_params;
  alpha_params.domain_participant(server_side).service_name("named").instance_name("alpha");
  auto alpha = std::make_shared<robot::CalculatorService>(implementation_alpha, alpha_params);
  dds::rpc::ServiceParams beta_params;
  beta_params.domain_participant(server_side).service_name("named").instance_name("beta");
  auto beta = std::make_shared<robot::CalculatorService>(implementation_beta, beta_params);

  dds::rpc::Server server;
  dds::rpc::add(server, service);
  dds::rpc::add(server, alpha);
  dds::rpc::add(server, beta);
  std::atomic<bool> stop{false};
  std::thread serving([&]() {
    while (!stop) {
      if (dds::rpc::run_once(server) == 0) {
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
      }
    }
  });

  try {
    robot::CalculatorClient client(client_runtime);
    check(client.wait_for_service(std::chrono::seconds(10)),
          "discovery: the client finds the service through the C library");
    check(client.add(2, 40) == 42, "add over RTPS");
    client.reset();
    check(implementation.resets == 1, "a void operation reached the implementation");
    std::int32_t rest = -1;
    check(client.div(17, 5, rest) == 3 && rest == 2, "an out parameter comes back");
    robot::Pose start{10.0, 4.0};
    robot::Pose moved{};
    std::int32_t steps = 5;
    client.move(moved, start, steps);
    check(moved.x() == 15.0 && moved.y() == 8.0 && steps == 6, "a structure and an inout parameter");
    std::string labelled;
    client.label(labelled, "robot");
    check(labelled == "robot!", "a string");
    client.total(11);
    check(client.total() == 11, "an attribute");

    bool too_fast = false;
    try {
      (void)client.div(1, 0, rest);
    } catch (const robot::TooFast& error) {
      too_fast = error.limit == 7 && error.why == "division by zero";
    }
    check(too_fast, "a declared exception is thrown at the caller");

    bool unknown = false;
    try {
      client.label(labelled, "boom");
    } catch (const dds::rpc::RemoteUnknownExceptionError&) {
      unknown = true;
    }
    check(unknown, "an undeclared exception is UNKNOWN_EXCEPTION");

    // §7.6.2: two instances of one service name, addressed by instance name.
    dds::rpc::ClientParams named_params;
    named_params.domain_participant(client_side).service_name("named");
    robot::CalculatorClient named(named_params);
    named.wait_for_service(dds::Duration(10), "alpha");
    named.wait_for_service(dds::Duration(10), "beta");
    const std::vector<std::string> discovered = named.get_discoverd_service_instances();
    const auto has = [&discovered](const char* instance) {
      return std::find(discovered.begin(), discovered.end(), instance) != discovered.end();
    };
    check(has("alpha") && has("beta"),
          "both named instances are discovered");
    named.bind("alpha");
    check(named.add(1, 2) == 1003, "bound to alpha: only alpha answers");
    named.bind("beta");
    check(named.add(1, 2) == 2003, "bound to beta: only beta answers");

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
