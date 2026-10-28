// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The function-call style of DDS-RPC 1.0 §7.11.1.5 over the Enhanced Service
// Mapping (§7.5.1.2), through the classes the generator writes for an
// interface: `CalcClient` sets the member of the operation in `Calc_Request`
// and reads the `Result` of that member from `Calc_Reply`, `CalcService`
// dispatches on the member that is present, and what crosses is the bare
// PL_CDR2 topic types of the generated codec, correlated by sample identity
// (§7.8.2). The bus under them is the in-memory binding of `loopback.hpp`.
//
// The member ids arrive as -D definitions computed by the generator's model
// (HASH of the member name & 0x0FFFFFFF, §7.5.1.2.1.2).

#include "calc.hpp"
#include "loopback.hpp"

#include <atomic>
#include <chrono>
#include <cstdio>
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

/// The octets of a little-endian 32-bit word.
void put32(Octets& out, std::uint32_t value) {
  for (int shift = 0; shift < 32; shift += 8) {
    out.push_back(static_cast<std::uint8_t>((value >> shift) & 0xFFu));
  }
}

/// The implementation of the interface a service answers with.
class CalcImpl : public robot::Calc {
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

  void label(std::string& cxx_return, const std::string& prefix) override {
    if (prefix == "boom") {
      throw std::runtime_error("the implementation failed");
    }
    cxx_return = prefix + "!";
  }

  void note(const std::string& text) override { last_note = text; ++notes; }

  // §7.5.1.2.6: an operation of the base interface is answered on the topics
  // of the derived one.
  std::int32_t ident() override { return 77; }

  std::int32_t total() override { return total_value; }
  void total(std::int32_t value) override { total_value = value; }

  std::int32_t total_value = 0;
  std::atomic<int> resets{0};
  std::atomic<int> notes{0};
  std::string last_note;
};

}  // namespace

int main() {
  Bus bus;
  bus.per_reader_queues = true;
  // §7.8.2: the binding reports the identity of a sample and its related
  // identity, which is how the Enhanced profile correlates.
  bus.expose_identity = true;
  auto participant = std::make_shared<LoopbackParticipant>(bus);
  dds::rpc::RPCRuntime runtime(participant);

  CalcImpl implementation;
  auto service = std::make_shared<robot::CalcService>(implementation, runtime);

  dds::rpc::Server server;
  dds::rpc::add(server, service);
  std::atomic<bool> stop{false};
  std::thread serving([&]() {
    while (!stop) {
      if (dds::rpc::run_once(server) == 0) {
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
      }
    }
  });

  robot::CalcClient client(runtime);
  check(client.wait_for_service(std::chrono::seconds(2)), "the client finds the service");

  // The operations are plain calls.
  check(client.add(2, 40) == 42, "add through the generated client and service");
  client.reset();
  check(implementation.resets == 1, "a void operation reached the implementation");
  std::int32_t rest = -1;
  check(client.div(17, 5, rest) == 3 && rest == 2, "an out parameter comes back");
  std::string labelled;
  client.label(labelled, "robot");
  check(labelled == "robot!", "a string goes both ways");
  check(client.ident() == 77, "an inherited operation is answered on the derived topics");
  client.total(11);
  check(client.total() == 11 && implementation.total_value == 11, "an attribute is set and read");

  // §7.5.1.2.3 rule 6: an exception the operation raises is the member of
  // the Result structure that carries it.
  bool too_fast = false;
  try {
    (void)client.div(1, 0, rest);
  } catch (const robot::TooFast& error) {
    too_fast = error.limit == 7 && error.why == "division by zero";
  }
  check(too_fast, "a declared exception is thrown at the caller, members intact");

  // §7.5.2: anything else is a remote exception code, the `remoteEx` member
  // of the reply (§7.5.1.2.5 rule 5).
  bool unknown_exception = false;
  try {
    client.label(labelled, "boom");
  } catch (const dds::rpc::RemoteUnknownExceptionError&) {
    unknown_exception = true;
  }
  check(unknown_exception, "an undeclared exception of the implementation is UNKNOWN_EXCEPTION");

  // A oneway operation has a request and no reply.
  client.note("hello");
  for (int waited = 0; waited < 2000 && implementation.notes == 0; ++waited) {
    std::this_thread::sleep_for(std::chrono::milliseconds(1));
  }
  check(implementation.notes == 1 && implementation.last_note == "hello",
        "a oneway operation reached the implementation");

  // The asynchronous operations return futures.
  auto sum = client.add_async(20, 22);
  check(sum.get() == 42, "add_async");
  auto done = client.reset_async();
  done.get();
  check(implementation.resets == 2, "a void operation returns future<void>");
  auto divided = client.div_async(1, 0);
  bool async_declared = false;
  try {
    (void)divided.get();
  } catch (const robot::TooFast&) {
    async_declared = true;
  }
  check(async_declared, "a declared exception is the exception the future carries");

  // §7.5.1.2: the octets. A request is a PL_CDR2 object with one member:
  // DHEADER, the EMHEADER of the operation (length code 4: NEXTINT follows),
  // the NEXTINT, and `Calc_add_In`, two `long`s without headers.
  {
    Octets expected;
    put32(expected, 16);
    put32(expected, (4u << 28) | ZD_ID_ADD);
    put32(expected, 8);
    put32(expected, 2);
    put32(expected, 40);
    Octets found;
    {
      std::lock_guard<std::recursive_mutex> guard(bus.lock);
      for (const Octets& frame : bus.topic("robot_Calc_Service_Request").history) {
        if (frame.size() == expected.size() && frame[4] == expected[4] && frame[5] == expected[5] &&
            frame[6] == expected[6] && frame[7] == expected[7]) {
          found = frame;
          break;
        }
      }
    }
    check(found == expected, "the request of add(2, 40) is the octets of §7.5.1.2.4");
    robot::Calc_Request typed;
    typed.add.emplace();
    typed.add->a = 2;
    typed.add->b = 40;
    check(dds::rpc::dds_type_traits<robot::Calc_Request>::serialize(typed) == expected,
          "and so is the octets of the codec");
  }
  // The result: DHEADER, the EMHEADER of `add`, the NEXTINT, then the Result
  // structure: DHEADER, the EMHEADER of `result`, the NEXTINT and the Out
  // structure, one `long` (final, no headers).
  {
    Octets expected;
    put32(expected, 24);
    put32(expected, (4u << 28) | ZD_ID_ADD);
    put32(expected, 16);
    put32(expected, 12);
    put32(expected, (4u << 28) | ZD_ID_RESULT);
    put32(expected, 4);
    put32(expected, 42);
    robot::Calc_Reply typed;
    typed.add.emplace();
    typed.add->result.emplace();
    typed.add->result->return_ = 42;
    check(dds::rpc::dds_type_traits<robot::Calc_Reply>::serialize(typed) == expected,
          "the reply of add is the octets of §7.5.1.2.5");
    robot::Calc_Reply decoded;
    std::size_t at = 0;
    check(dds::rpc::dds_type_traits<robot::Calc_Reply>::deserialize(expected, at, decoded) &&
              at == expected.size() && decoded.add && decoded.add->result &&
              decoded.add->result->return_ == 42,
          "which decodes");
  }
  // §7.5.1.2.5 rule 5: `remoteEx` is a 32-bit member, length code 2.
  {
    Octets expected;
    put32(expected, 8);
    put32(expected, (2u << 28) | ZD_ID_REMOTE_EX);
    put32(expected, 4);
    const auto reply = dds::rpc::dds_type_traits<robot::Calc_Reply>::from_remote_exception(
        dds::rpc::REMOTE_EX_UNKNOWN_OPERATION);
    check(dds::rpc::dds_type_traits<robot::Calc_Reply>::serialize(reply) == expected,
          "remoteEx is the octets of §7.5.1.2.5 rule 5");
    check(dds::rpc::dds_type_traits<robot::Calc_Reply>::remote_exception(reply) ==
              dds::rpc::REMOTE_EX_UNKNOWN_OPERATION,
          "and the traits read the code back");
  }
  // A mutable structure skips a member it does not know and takes the
  // default of one that is absent (XTypes 1.3 §7.2.4.4.8).
  {
    Octets bytes;
    Octets body;
    put32(body, (4u << 28) | 0x123u);
    put32(body, 3);
    body.push_back(1);
    body.push_back(2);
    body.push_back(3);
    body.push_back(0);
    put32(body, (2u << 28) | ZD_ID_X);
    put32(body, 9);
    put32(bytes, static_cast<std::uint32_t>(body.size()));
    bytes.insert(bytes.end(), body.begin(), body.end());
    robot::Fast_square_In decoded;
    zerodds::wire::Reader reader(bytes, zerodds::wire::Endianness::Little, 4);
    zd_dec_enh_robot_Fast_square_In(reader, decoded);
    check(reader.reader_status() == zerodds::wire::Status::Ok && decoded.x == 9,
          "a mutable structure skips a member it does not know");
    Octets empty;
    put32(empty, 0);
    robot::Fast_square_In absent;
    absent.x = 5;
    zerodds::wire::Reader reader_empty(empty, zerodds::wire::Endianness::Little, 4);
    zd_dec_enh_robot_Fast_square_In(reader_empty, absent);
    check(reader_empty.reader_status() == zerodds::wire::Status::Ok && absent.x == 0,
          "and one that is absent takes its default");
  }
  // §7.7.2: a request for an operation the interface does not have is
  // answered REMOTE_EX_UNKNOWN_OPERATION, not dropped.
  {
    Octets request;
    Octets body;
    put32(body, (4u << 28) | 0x00ABCDEFu);
    put32(body, 4);
    put32(body, 0);
    put32(request, static_cast<std::uint32_t>(body.size()));
    request.insert(request.end(), body.begin(), body.end());
    auto foreign = participant->publisher()->create_writer("robot_Calc_Service_Request",
                                                            "robot::Calc_Request",
                                                            dds::rpc::EndpointQos{});
    (void)foreign->write(request);
    Octets expected;
    put32(expected, 8);
    put32(expected, (2u << 28) | ZD_ID_REMOTE_EX);
    put32(expected, 4);
    bool answered = false;
    for (int waited = 0; waited < 2000 && !answered; ++waited) {
      {
        std::lock_guard<std::recursive_mutex> guard(bus.lock);
        for (const auto& frame : bus.topic("robot_Calc_Service_Reply").written) {
          answered = answered || frame.first == expected;
        }
      }
      std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
    check(answered, "an unknown operation is answered REMOTE_EX_UNKNOWN_OPERATION");
  }

  // §7.5.1.2.1.1: "exactly one member shall be present at any given time".
  // A request with no member and one with two cannot be decoded, and the
  // service answers it REMOTE_EX_INVALID_ARGUMENT (§7.5.2), not nothing.
  {
    using RequestTraits = dds::rpc::dds_type_traits<robot::Calc_Request>;
    Octets none;
    put32(none, 0);
    robot::Calc_Request refused;
    std::size_t at = 0;
    check(!RequestTraits::deserialize(none, at, refused), "a request with no member is refused");

    robot::Calc_Request first;
    first.add.emplace();
    first.add->a = 1;
    first.add->b = 2;
    robot::Calc_Request second;
    second.reset.emplace();
    const Octets one = RequestTraits::serialize(first);
    const Octets two = RequestTraits::serialize(second);
    Octets both;
    put32(both, static_cast<std::uint32_t>((one.size() - 4) + (two.size() - 4)));
    both.insert(both.end(), one.begin() + 4, one.end());
    both.insert(both.end(), two.begin() + 4, two.end());
    at = 0;
    check(!RequestTraits::deserialize(both, at, refused), "a request with two members is refused");

    auto forge = participant->publisher()->create_writer("robot_Calc_Service_Request",
                                                         "robot::Calc_Request",
                                                         dds::rpc::EndpointQos{});
    Octets invalid;
    put32(invalid, 8);
    put32(invalid, (2u << 28) | ZD_ID_REMOTE_EX);
    put32(invalid, static_cast<std::uint32_t>(dds::rpc::REMOTE_EX_INVALID_ARGUMENT));
    for (const Octets* request : {&none, &both}) {
      (void)forge->write(*request);
    }
    int answers = 0;
    for (int waited = 0; waited < 2000 && answers < 2; ++waited) {
      answers = 0;
      {
        std::lock_guard<std::recursive_mutex> guard(bus.lock);
        for (const auto& frame : bus.topic("robot_Calc_Service_Reply").written) {
          answers += frame.first == invalid ? 1 : 0;
        }
      }
      std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
    check(answers == 2, "a request that cannot be decoded is answered REMOTE_EX_INVALID_ARGUMENT");
  }
  // §7.7.2.5.6: a Result that holds an exception member the client does not
  // know is REMOTE_EX_UNKNOWN_EXCEPTION at the caller. A second runtime has
  // no service; the test reads the request and forges the reply.
  {
    Bus lone_bus;
    lone_bus.per_reader_queues = true;
    lone_bus.expose_identity = true;
    auto lone_participant = std::make_shared<LoopbackParticipant>(lone_bus);
    dds::rpc::RPCRuntime lone_runtime(lone_participant);
    auto spy = lone_participant->subscriber()->create_reader(
        "robot_Calc_Service_Request", "robot::Calc_Request", dds::rpc::EndpointQos{});
    auto forger = lone_participant->publisher()->create_writer(
        "robot_Calc_Service_Reply", "robot::Calc_Reply", dds::rpc::EndpointQos{});
    robot::CalcClient lone(lone_runtime);
    auto pending = lone.add_async(1, 2);
    std::vector<std::pair<Octets, dds::SampleInfo>> seen;
    for (int waited = 0; waited < 2000 && seen.empty(); ++waited) {
      seen = spy->take();
      if (seen.empty()) {
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
      }
    }
    check(!seen.empty(), "the request of the lone client was written");
    if (!seen.empty()) {
      Octets foreign;
      put32(foreign, 24);
      put32(foreign, (4u << 28) | ZD_ID_ADD);
      put32(foreign, 16);
      put32(foreign, 12);
      put32(foreign, (4u << 28) | 0x00ABCDEFu);
      put32(foreign, 4);
      put32(foreign, 0);
      (void)forger->write_related(foreign, seen[0].second.identity);
      bool unknown_member = false;
      try {
        (void)pending.get();
      } catch (const dds::rpc::RemoteUnknownExceptionError&) {
        unknown_member = true;
      }
      check(unknown_member,
            "an exception member the client does not know is REMOTE_EX_UNKNOWN_EXCEPTION");
    }
  }

  stop = true;
  serving.join();
  if (failures == 0) {
    std::puts("every check held");
  }
  return failures == 0 ? 0 : 1;
}
