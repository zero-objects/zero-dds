// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The generated function-call client and service of the Calculator interface
// over the ZeroDDS binding of `dds/rpc/zerodds.hpp`, linked against the real
// C library (libzerodds). Two runtimes on one domain, so the requests and
// replies cross the RTPS stack, and discovery (`wait_for_service`, named
// instances, §7.6.2) runs between them. The type evolution of §7.7.2.5 is in
// real_library_evolution.cpp.

#include <algorithm>
#include "calculator.hpp"

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
