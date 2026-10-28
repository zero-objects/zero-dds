// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The function-call style of DDS-RPC 1.0 §7.11.1.5, through the classes the
// generator writes for an interface: `CalculatorClient` calls, `CalculatorService`
// answers, a `Server` dispatches, and what crosses is the topic types of
// §7.5.1.1 in the octets of the generated codec. The bus under them is the
// in-memory binding of `loopback.hpp`.
//
// A thread serves while the main thread calls, so a synchronous call really
// waits for its reply.

#include "calculator.hpp"
#include "loopback.hpp"

#include <atomic>
#include <chrono>
#include <cstdio>
#include <stdexcept>
#include <string>
#include <thread>
#include <type_traits>

namespace {

int failures = 0;

void check(bool held, const char* what) {
  if (!held) {
    std::fprintf(stderr, "FAIL %s\n", what);
    ++failures;
  }
}

/// The implementation of the interface a service answers with.
class CalcImpl : public robot::Calculator {
 public:
  std::int32_t add(std::int32_t a, std::int32_t b) override { return a + b; }

  void reset() override {
    total_value = 0;
    ++resets;
  }

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

  double scale(double factor, std::int64_t big) override {
    return factor * static_cast<double>(big);
  }

  std::int32_t risky(std::int32_t x) override {
    // Rule 8: `template` is a reserved word, so the exception is
    // `cxx_template`.
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
};

/// The `Call` union of the request in `frame`, whose header is 29 octets and
/// is padded to 32.
std::int32_t discriminator_of(const Octets& frame) {
  return static_cast<std::int32_t>(
      static_cast<std::uint32_t>(frame[32]) | (static_cast<std::uint32_t>(frame[33]) << 8) |
      (static_cast<std::uint32_t>(frame[34]) << 16) | (static_cast<std::uint32_t>(frame[35]) << 24));
}

}  // namespace

int main() {
  Bus bus;
  bus.per_reader_queues = true;
  auto participant = std::make_shared<LoopbackParticipant>(bus);
  dds::rpc::RPCRuntime runtime(participant);

  CalcImpl implementation;
  CalcImpl implementation_x2;

  // Three services on three pairs of topics: the default, and one per CDR
  // form with its own service name (§7.4.2.3).
  auto service = std::make_shared<robot::CalculatorService>(implementation, runtime);
  dds::rpc::ServiceParams x1_service;
  x1_service.domain_participant(participant).service_name("x1");
  auto service_x1 = std::make_shared<robot::CalculatorService>(implementation, x1_service);
  dds::rpc::ServiceParams x2_service;
  x2_service.domain_participant(participant).service_name("x2");
  dds::rpc::wire_encoding(x2_service, dds::rpc::WireEncoding::XCDR2);
  auto service_x2 = std::make_shared<robot::CalculatorService>(implementation_x2, x2_service);
  check(service->service_name() == "Service", "§7.4.1: the service name defaults to \"Service\"");
  check(service_x1->service_name() == "x1", "and ServiceParams supply another");

  dds::rpc::Server server;
  dds::rpc::add(server, service);
  dds::rpc::add(server, service_x1);
  dds::rpc::add(server, service_x2);
  std::atomic<bool> stop{false};
  std::thread serving([&]() {
    while (!stop) {
      if (dds::rpc::run_once(server) == 0) {
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
      }
    }
  });

  robot::CalculatorClient client(runtime);
  check(client.wait_for_service(std::chrono::seconds(2)), "the client finds the service");

  // §7.9.1, §7.11.1.1.2 rule 6: the operations are plain calls.
  check(client.add(2, 40) == 42, "add through the generated client and service");
  client.reset();
  check(implementation.resets == 1, "a void operation reached the implementation");
  std::int32_t rest = -1;
  check(client.div(17, 5, rest) == 3 && rest == 2, "an out parameter comes back");

  // Rule 6.h: a constructed return value is the leading argument.
  robot::Pose start{10.0, 4.0};
  robot::Pose moved{};
  std::int32_t steps = 5;
  client.move(moved, start, steps);
  check(moved.x() == 15.0 && moved.y() == 8.0 && steps == 6,
        "a constructed return value and an inout parameter");
  std::string labelled;
  client.label(labelled, "robot");
  check(labelled == "robot!", "a string goes both ways");
  check(client.scale(1.5, 4) == 6.0, "a double and a long long");

  // §7.11.1.1.2 rule 6.f, DDS-Cxx-PSM §7.4.6: attributes.
  client.total(11);
  check(client.total() == 11 && implementation.total_value == 11, "an attribute is set and read");
  const robot::Pose& origin = client.origin();
  check(origin.x() == 3.5 && origin.y() == -1.25, "a readonly attribute of a structure");
  const robot::CalculatorClient& const_client = client;
  check(const_client.origin().x() == 3.5, "and through the const accessor");

  // §7.5.1.1.5: an exception the operation raises is thrown at the caller.
  bool too_fast = false;
  try {
    (void)client.div(1, 0, rest);
  } catch (const robot::TooFast& error) {
    too_fast = error.limit == 7 && error.why == "division by zero";
  }
  check(too_fast, "a declared exception is thrown at the caller, members intact");

  // §7.11.1.1.2 rule 8: an exception named by a reserved word has the
  // `cxx_` name, and it crosses the wire like any other.
  bool reserved = false;
  try {
    (void)client.risky(9);
  } catch (const robot::cxx_template& error) {
    reserved = error.code == 9;
  }
  check(reserved, "an exception whose name is a reserved word is thrown as cxx_<name>");

  // §7.5.2, §7.11.1.3: anything else is a remote exception code.
  bool unknown_exception = false;
  try {
    client.label(labelled, "boom");
  } catch (const dds::rpc::RemoteUnknownExceptionError&) {
    unknown_exception = true;
  }
  check(unknown_exception, "an undeclared exception of the implementation is UNKNOWN_EXCEPTION");

  // §7.7.1.1: a request for an operation the service does not have.
  {
    dds::rpc::core::RequesterParams params;
    params.domain_participant(participant)
        .interface_name("robot_Calculator")
        .binding_style(dds::rpc::BindingStyle::FunctionCall);
    robot::CalculatorRequester raw(params);
    robot::Calculator_Request request{};
    request.data._d = 0x5555;
    bool unknown_operation = false;
    try {
      (void)raw.call(request);
    } catch (const dds::rpc::RemoteUnsupportedError&) {
      unknown_operation = true;
    }
    check(unknown_operation, "an unknown operation of the Basic mapping is REMOTE_EX_UNSUPPORTED");
  }

  // §7.11.1.1.2 rule 7: the asynchronous operations return futures.
  auto sum = client.add_async(20, 22);
  check(sum.get() == 42, "add_async");
  auto done = client.reset_async();
  done.get();
  check(implementation.resets == 2, "a void operation returns future<void>");
  auto moved_async = client.move_async(start, steps);
  const auto moved_out = moved_async.get();
  check(moved_out.return_.x() == 16.0 && moved_out.steps == 7,
        "an operation with outputs returns future of its Out structure (rule 7.i)");
  // `reset` above cleared the attribute; set it again for the getter future.
  client.total(11);
  auto total_async = client.total_async();
  check(total_async.get() == 11, "an attribute getter returns future of its type (rule 7.f)");
  auto set_async = client.total_async(12);
  set_async.get();
  check(implementation.total_value == 12, "and the setter future<void> (rule 7.g)");
  auto boom = client.label_async("boom");
  bool async_unknown = false;
  try {
    (void)boom.get();
  } catch (const dds::rpc::RemoteUnknownExceptionError&) {
    async_unknown = true;
  }
  check(async_unknown, "a remote exception is the exception the future carries");
  auto too_fast_future = client.div_async(1, 0);
  bool async_declared = false;
  try {
    (void)too_fast_future.get();
  } catch (const robot::TooFast&) {
    async_declared = true;
  }
  check(async_declared, "and so is a declared exception");

  // §7.5.1.1: the octets. Header, padding to four, the union; classic CDR
  // aligns an eight-octet member to eight counted from the sample start,
  // XCDR2 to four.
  {
    dds::rpc::ClientParams x1_params;
    x1_params.domain_participant(participant).service_name("x1");
    robot::CalculatorClient x1_client(x1_params);
    dds::rpc::ClientParams x2_params;
    x2_params.domain_participant(participant).service_name("x2");
    dds::rpc::wire_encoding(x2_params, dds::rpc::WireEncoding::XCDR2);
    robot::CalculatorClient x2_client(x2_params);
    check(x1_client.scale(2.0, 3) == 6.0, "classic CDR round trip");
    check(x2_client.scale(2.0, 3) == 6.0, "XCDR2 round trip");

    auto frame_of = [&](const char* topic) -> Octets {
      std::lock_guard<std::recursive_mutex> guard(bus.lock);
      for (const Octets& frame : bus.topic(topic).history) {
        if (frame.size() > 36 && discriminator_of(frame) == robot::Calculator_scale_Hash) {
          return frame;
        }
      }
      return Octets{};
    };
    const Octets classic = frame_of("robot_Calculator_x1_Request");
    const Octets xcdr2 = frame_of("robot_Calculator_x2_Request");
    // 29 octets of header, padded to 32; the discriminator 32..36; then the
    // double at 40 (padded to eight) and the long long at 48 under classic
    // CDR, at 36 and 44 under XCDR2.
    check(classic.size() == 56, "classic CDR: the double aligns to eight from the sample start");
    check(xcdr2.size() == 52, "XCDR2: it aligns to four");
  }

  // §7.10.2: the mutable QoS policies change at run time.
  {
    dds::rpc::RpcQos qos = client.qos();
    qos.request.deadline_ns = 1000000;
    client.set_qos(qos);
    check(participant->qos_of("robot_Calculator_Service_Request").deadline_ns == 1000000,
          "ClientEndpoint::set_qos reaches the request writer");
  }

  // Figure 7.5 and §7.11.1.5.3: a Server is an RPCEntity; a Service is a
  // ServiceEndpoint, which is one; a Client is a ClientEndpoint, a ServiceProxy.
  static_assert(std::is_base_of<dds::rpc::RPCEntity, dds::rpc::Server>::value,
                "Server is an RPCEntity");
  static_assert(std::is_base_of<dds::rpc::RPCEntity, dds::rpc::ServiceEndpoint>::value,
                "ServiceEndpoint is an RPCEntity");
  static_assert(std::is_base_of<dds::rpc::ServiceEndpoint, dds::rpc::Service>::value,
                "Service is a ServiceEndpoint");
  static_assert(std::is_base_of<dds::rpc::ServiceProxy, dds::rpc::ClientEndpoint>::value,
                "ClientEndpoint is a ServiceProxy");
  static_assert(std::is_base_of<dds::rpc::ClientEndpoint, dds::rpc::Client>::value,
                "Client is a ClientEndpoint");

  // §7.11.1.5.2: the status of a service.
  check(service->status() == dds::rpc::RUNNING, "a service is RUNNING");
  service->pause();
  check(service->status() == dds::rpc::PAUSED, "a paused service is PAUSED");
  service->resume();
  check(service->status() == dds::rpc::RUNNING, "and RUNNING again once resumed");

  // §7.11.1.5.2: the params and the DDS entities of a service.
  check(service_x1->get_service_params().service_name() == "x1",
        "ServiceEndpoint::get_service_params gives the service name");
  check(dds::rpc::wire_encoding(service_x2->get_service_params()) == dds::rpc::WireEncoding::XCDR2,
        "and the CDR form");
  check(service->get_request_datareader<robot::Calculator_Request>() != nullptr &&
            service->get_reply_datawriter<robot::Calculator_Reply>() != nullptr,
        "ServiceEndpoint gives the request DataReader and the reply DataWriter");

  // §7.11.1.5.5: the params and the DDS entities of a client.
  check(client.get_request_datawriter<robot::Calculator_Request>() != nullptr &&
            client.get_reply_datareader<robot::Calculator_Reply>() != nullptr,
        "ClientEndpoint gives the request DataWriter and the reply DataReader");
  check(client.get_request_datawriter<robot::Calculator_Request>()->matched_count() >= 1,
        "and the request DataWriter is the one matched with the service");
  {
    dds::rpc::ClientParams named;
    named.domain_participant(participant).service_name("x1");
    robot::CalculatorClient named_client(named);
    check(named_client.get_client_params().service_name() == "x1",
          "ClientEndpoint::get_client_params gives the params the client runs with");
    check(named_client.get_bound_instance_name().empty(), "no instance is bound by default");
    named_client.wait_for_service(dds::Duration::from_millisecs(500));
    named_client.wait_for_services(1);
    named_client.wait_for_service_async().get();
    check(named_client.get_discoverd_service_instances().size() == 1,
          "the discovered instances of a client of an unnamed service: the default one");
  }

  // §7.11.1.5.1: a Service may belong to only one Server.
  {
    dds::rpc::Server other;
    bool refused = false;
    try {
      dds::rpc::add(other, service);
    } catch (const dds::rpc::IllegalOperationError&) {
      refused = true;
    }
    check(refused && dds::rpc::size(other) == 0, "a service another server holds is refused");
    bool twice = false;
    try {
      dds::rpc::add(server, service);
    } catch (const dds::rpc::IllegalOperationError&) {
      twice = true;
    }
    check(twice && dds::rpc::size(server) == 3, "and so is the service the server holds already");
    check(!other.is_null() && !server.is_null(), "an open server is not null");
  }

  // §7.11.1.5.3: ServerParams, and run(max_wait) that does not block.
  {
    dds::rpc::ServerParams server_params;
    server_params.default_service_params(x1_service);
    dds::rpc::Server configured(server_params);
    check(dds::rpc::default_service_params(configured).service_name() == "x1",
          "a Server made from ServerParams keeps the default service params");
    const auto before = std::chrono::steady_clock::now();
    configured.run(dds::Duration::from_millisecs(30));
    check(std::chrono::steady_clock::now() - before < std::chrono::seconds(2),
          "run(max_wait) returns when the time is up");
  }

  stop = true;
  serving.join();

  // §7.11.1.5.3: run() blocks and dispatches until the server is closed.
  {
    Bus run_bus;
    // Two clients and two services share the topics: a queue each, or the
    // reply to one client is taken by the reader of the other.
    run_bus.per_reader_queues = true;
    auto run_participant = std::make_shared<LoopbackParticipant>(run_bus);
    dds::rpc::RPCRuntime run_runtime(run_participant);
    CalcImpl impl;
    auto owned = std::make_shared<robot::CalculatorService>(impl, run_runtime);
    dds::rpc::Server runner;
    dds::rpc::add(runner, owned);
    std::atomic<bool> returned{false};
    std::thread running([&]() {
      runner.run();
      returned = true;
    });
    robot::CalculatorClient run_client(run_runtime);
    check(run_client.wait_for_service(std::chrono::seconds(2)), "the client finds the service");
    check(run_client.add(1, 2) == 3, "run() dispatches the requests as they arrive");
    check(!returned, "and does not return while the server is open");
    runner.close();
    running.join();
    check(returned && runner.is_null(), "close() ends run() and leaves the server null");
    check(owned->status() == dds::rpc::CLOSED && owned->is_null(),
          "and closes the services it held");

    // run(max_wait) returns once a round has answered a request.
    dds::rpc::Server second;
    CalcImpl second_impl;
    auto second_service = std::make_shared<robot::CalculatorService>(second_impl, run_runtime);
    dds::rpc::add(second, second_service);
    robot::CalculatorClient second_client(run_runtime);
    check(second_client.wait_for_service(std::chrono::seconds(2)), "the second client finds it");
    auto pending = second_client.add_async(4, 5);
    second.run(dds::Duration::from_millisecs(2000));
    check(pending.wait_for(std::chrono::seconds(2)) == std::future_status::ready &&
              pending.get() == 9,
          "run(max_wait) answered the request that was waiting");
    second.close();
    bool closed_refuses = false;
    try {
      dds::rpc::add(second, std::make_shared<robot::CalculatorService>(second_impl, run_runtime));
    } catch (const dds::rpc::AlreadyClosedError&) {
      closed_refuses = true;
    }
    check(closed_refuses, "a closed server takes no service");
  }

  server.close();
  check(server.is_null() && service->status() == dds::rpc::CLOSED,
        "closing the server closes its services");

  // §7.4.1, §7.3.1.3: a Client of the function-call style against a Replier
  // of the request-reply style. The two meet when the request-reply side names
  // the topics the client uses: the topic names the function-call style
  // synthesizes (`<interface>_<service>_Request`) are set explicitly, and the
  // type names are the generated ones on both sides.
  {
    Bus meeting_bus;
    auto meeting = std::make_shared<LoopbackParticipant>(meeting_bus);
    dds::rpc::RPCRuntime meeting_runtime(meeting);
    dds::rpc::core::ReplierParams replier_params;
    replier_params.domain_participant(meeting)
        .binding_style(dds::rpc::BindingStyle::RequestReply)
        .request_topic_name(robot::CalculatorSupport::request_topic_name())
        .reply_topic_name(robot::CalculatorSupport::reply_topic_name());
    robot::CalculatorReplier replier(replier_params);
    check(replier_params.topic_names().request == robot::CalculatorSupport::request_topic_name() &&
              replier_params.topic_names().reply == robot::CalculatorSupport::reply_topic_name(),
          "the request-reply style takes the topic names it is given");

    std::atomic<bool> meeting_stop{false};
    std::thread answering([&]() {
      while (!meeting_stop) {
        auto requests = replier.receive_requests();
        for (auto request : requests) {
          robot::Calculator_Reply reply{};
          reply.data._d = request.data().data._d;
          reply.data.add._d = dds::rpc::RETCODE_OK;
          reply.data.add.result.return_ = request.data().data.add.a + request.data().data.add.b;
          replier.send_reply(reply, request.info().identity);
        }
        if (requests.length() == 0) {
          std::this_thread::sleep_for(std::chrono::milliseconds(1));
        }
      }
    });
    robot::CalculatorClient meeting_client(meeting_runtime);
    check(meeting_client.wait_for_service(std::chrono::seconds(2)),
          "the client finds a replier of the request-reply style");
    check(meeting_client.add(30, 12) == 42,
          "a call of the function-call style is answered by a Replier of the request-reply style");
    meeting_stop = true;
    answering.join();
  }

  // §7.9.3: deleting the service deactivates it, and deactivation deletes the
  // DataReader and the DataWriter and nothing else.
  {
    Bus second_bus;
    auto second = std::make_shared<LoopbackParticipant>(second_bus);
    dds::rpc::RPCRuntime second_runtime(second);
    const std::string request_topic = robot::CalculatorSupport::request_topic_name();
    const std::string reply_topic = robot::CalculatorSupport::reply_topic_name();
    CalcImpl impl;
    {
      auto scoped = std::make_shared<robot::CalculatorService>(impl, second_runtime);
      check(second_bus.topic(request_topic).readers == 1 &&
                second_bus.topic(reply_topic).writers == 1,
            "creating the service creates its DataReader and DataWriter (§7.9.1)");
      auto copy = scoped;
      scoped.reset();
      check(second_bus.topic(request_topic).readers == 1,
            "a service is a reference: it lives while a copy does");
    }
    check(second_bus.topic(request_topic).readers == 0 &&
              second_bus.topic(reply_topic).writers == 0,
          "destroying the last copy deletes the DataReader and the DataWriter (§7.9.3)");

    robot::CalculatorService closed(impl, second_runtime);
    closed.close();
    check(closed.is_null() && second_bus.topic(request_topic).readers == 0,
          "close() deactivates the service");

    {
      robot::CalculatorClient scoped_client(second_runtime);
      check(second_bus.topic(request_topic).writers == 1 &&
                second_bus.topic(reply_topic).readers == 1,
            "creating the client creates its DataWriter and DataReader (§7.9.1)");
      robot::CalculatorClient copy = scoped_client;
      (void)copy;
    }
    check(second_bus.topic(request_topic).writers == 0 && second_bus.topic(reply_topic).readers == 0,
          "destroying the client deletes them");
    robot::CalculatorClient explicit_close(second_runtime);
    explicit_close.close();
    check(explicit_close.is_null() && second_bus.topic(request_topic).writers == 0,
          "ClientEndpoint::close() deletes them too");
  }

  if (failures != 0) {
    std::fprintf(stderr, "%d checks failed\n", failures);
    return 1;
  }
  std::printf("function-call round trip: every check held\n");
  return 0;
}
