// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// DDS-RPC 1.0 §7.11.1: a program written against the normative headers
// (request_reply.h, function_call.h, sample.h of omg-dds-rpc-cxx.zip, shipped
// unchanged in dds/rpc/normative/) runs a request/reply and a function call
// against the ZeroDDS runtime, over the real C library on two participants of
// one domain.
//
// The request/reply part names only classes the normative headers declare:
// `Requester`, `Replier`, their params, `dds::Sample`, `dds::WriteSample`. The
// function-call part uses the generated Calculator client and service, whose
// bases are the normative `ServiceEndpoint` and `ClientEndpoint` classes.

#include <dds/rpc/normative/request_reply.h>
#include <dds/rpc/normative/function_call.h>

#include "calculator.hpp"

#include <dds/rpc/zerodds.hpp>

#include <atomic>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <memory>
#include <thread>
#include <type_traits>

namespace app {
struct Req {
  std::int32_t a = 0;
  std::int32_t b = 0;
};
struct Rep {
  std::int32_t sum = 0;
  std::int32_t remote_ex = 0;
};
}  // namespace app

namespace dds {
namespace rpc {
template <>
struct dds_type_traits<app::Req> : dds_type_traits_base<app::Req> {
  static const char* type_name() { return "NormReq"; }
  static Octets serialize(const app::Req& v) {
    Octets o;
    detail::put_u32(o, static_cast<std::uint32_t>(v.a));
    detail::put_u32(o, static_cast<std::uint32_t>(v.b));
    return o;
  }
  static bool deserialize(const Octets& in, std::size_t& at, app::Req& out) {
    std::uint32_t a = 0;
    std::uint32_t b = 0;
    if (!detail::get_u32(in, at, a) || !detail::get_u32(in, at, b)) {
      return false;
    }
    out.a = static_cast<std::int32_t>(a);
    out.b = static_cast<std::int32_t>(b);
    return true;
  }
};
template <>
struct dds_type_traits<app::Rep> : dds_type_traits_base<app::Rep> {
  static const char* type_name() { return "NormRep"; }
  static Octets serialize(const app::Rep& v) {
    Octets o;
    detail::put_u32(o, static_cast<std::uint32_t>(v.sum));
    detail::put_u32(o, static_cast<std::uint32_t>(v.remote_ex));
    return o;
  }
  static bool deserialize(const Octets& in, std::size_t& at, app::Rep& out) {
    std::uint32_t sum = 0;
    std::uint32_t ex = 0;
    if (!detail::get_u32(in, at, sum) || !detail::get_u32(in, at, ex)) {
      return false;
    }
    out.sum = static_cast<std::int32_t>(sum);
    out.remote_ex = static_cast<std::int32_t>(ex);
    return true;
  }
  static RemoteExceptionCode_t remote_exception(const app::Rep& v) {
    return static_cast<RemoteExceptionCode_t>(v.remote_ex);
  }
  static app::Rep from_remote_exception(RemoteExceptionCode_t code) {
    app::Rep r;
    r.remote_ex = static_cast<std::int32_t>(code);
    return r;
  }
};
}  // namespace rpc
}  // namespace dds

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
  std::int32_t add(std::int32_t a, std::int32_t b) override { return a + b; }
};

}  // namespace

int main() {
  const std::uint32_t domain = 88;
  auto requester_side = dds::rpc::zerodds::participant_on(domain);
  auto replier_side = dds::rpc::zerodds::participant_on(domain);

  // -- request/reply, the classes of request_reply.h -------------------------------------------
  dds::rpc::RequesterParams requester_params;
  requester_params.domain_participant(requester_side)
      .request_topic_name("NormReqTopic")
      .reply_topic_name("NormRepTopic");
  dds::rpc::ReplierParams replier_params;
  replier_params.domain_participant(replier_side)
      .request_topic_name("NormReqTopic")
      .reply_topic_name("NormRepTopic");
  dds::rpc::Requester<app::Req, app::Rep> requester(requester_params);
  dds::rpc::Replier<app::Req, app::Rep> replier(replier_params);
  check(requester.get_requester_params().request_topic_name() == "NormReqTopic",
        "the requester keeps its params");

  bool answered = false;
  for (int attempt = 0; attempt < 40 && !answered; ++attempt) {
    app::Req request;
    request.a = 20;
    request.b = 22;
    dds::WriteSample<app::Req> out(request);
    requester.send_request(out);
    dds::Sample<app::Req> received;
    if (replier.receive_request(received, dds::Duration::from_millisecs(250))) {
      app::Rep reply;
      reply.sum = received.data().a + received.data().b;
      replier.send_reply(reply, received.identity());
      dds::Sample<app::Rep> answer;
      if (requester.receive_reply(answer, out.identity(), dds::Duration::from_millisecs(2000))) {
        answered = answer.data().sum == 42;
      }
    }
  }
  check(answered, "request_reply.h: a request is answered with 20 + 22 = 42 over RTPS");

  // -- function call, the classes of function_call.h -------------------------------------------
  static_assert(std::is_base_of<dds::rpc::ClientEndpoint, robot::CalculatorClient>::value,
                "the generated client is a ClientEndpoint");
  static_assert(std::is_base_of<dds::rpc::ServiceEndpoint, robot::CalculatorService>::value,
                "the generated service is a ServiceEndpoint");
  dds::rpc::ServiceParams service_params;
  service_params.domain_participant(replier_side).service_name("NormCalc");
  dds::rpc::ClientParams client_params;
  client_params.domain_participant(requester_side).service_name("NormCalc");
  CalcImpl implementation;
  auto service = std::make_shared<robot::CalculatorService>(implementation, service_params);
  dds::rpc::Server server;
  dds::rpc::add(server, service);
  std::atomic<bool> stop{false};
  std::thread serving([&]() {
    while (!stop.load()) {
      if (dds::rpc::run_once(server) == 0) {
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
      }
    }
  });
  {
    robot::CalculatorClient client(client_params);
    client.wait_for_service(dds::Duration::from_millisecs(10000));
    check(client.add(40, 2) == 42, "function_call.h: add(40, 2) through the client is 42");
  }
  stop.store(true);
  serving.join();

  if (failures == 0) {
    std::puts("every check held");
    return 0;
  }
  return 1;
}
