// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The C++ request-reply and function-call bindings of DDS-RPC 1.0
// §7.11.1, exercised end to end over an in-process binding of the three
// entity classes `dds/rpc.hpp` leaves abstract.
//
// Every class §7.11.1.4.1 and §7.11.1.5 name is used here, not merely
// declared: a header that compiles proves the names exist, and only a
// run proves the Requester correlates, the Replier answers, the
// listeners fire and the loaned samples come back.
//
// The generated header of the Calculator interface is included first —
// `crates/lang-rules/src/emit/interface_cpp.rs` wrote it — so this is
// also the check that the generator and the runtime agree on the topic
// type names.

#include "calculator.hpp"
#include "loopback.hpp"

#include <dds/rpc.hpp>

#include <cassert>
#include <chrono>
#include <algorithm>
#include <cstdio>
#include <functional>
#include <iterator>
#include <map>
#include <memory>
#include <stdexcept>
#include <string>
#include <type_traits>
#include <vector>

namespace {

using dds::rpc::Octets;

// Figure 7.5: the class hierarchy of the active RPC entities, checked when
// the program compiles.
static_assert(std::is_base_of<dds::rpc::RPCEntity, dds::rpc::ServiceProxy>::value,
              "ServiceProxy is an RPCEntity");
static_assert(std::is_base_of<dds::rpc::core::ServiceProxy, robot::CalculatorRequester>::value,
              "Requester is a ServiceProxy");
static_assert(std::is_base_of<dds::rpc::core::RPCEntity, robot::CalculatorReplier>::value,
              "Replier is an RPCEntity");
static_assert(std::is_base_of<dds::rpc::RPCEntity, dds::rpc::ServiceEndpoint>::value,
              "ServiceEndpoint is an RPCEntity");
static_assert(std::is_base_of<dds::rpc::ServiceEndpoint, dds::rpc::Service>::value,
              "Service is a ServiceEndpoint");
static_assert(std::is_base_of<dds::rpc::ServiceProxy, dds::rpc::ClientEndpoint>::value,
              "ClientEndpoint is a ServiceProxy");
static_assert(std::is_base_of<dds::rpc::ClientEndpoint, dds::rpc::Client>::value,
              "Client is a ClientEndpoint");
static_assert(std::is_base_of<dds::rpc::RPCEntity, dds::rpc::Server>::value,
              "Server is an RPCEntity");
static_assert(std::is_base_of<dds::rpc::core::ListenerBase,
                              dds::rpc::core::SimpleReplierListener<int, int>>::value &&
                  std::is_base_of<dds::rpc::core::ListenerBase, dds::rpc::core::ReplierListener<int, int>>::value &&
                  std::is_base_of<dds::rpc::core::ListenerBase, dds::rpc::core::SimpleRequesterListener<int>>::value &&
                  std::is_base_of<dds::rpc::core::ListenerBase, dds::rpc::core::RequesterListener<int, int>>::value,
              "the four listeners are ListenerBase");

/// An Enhanced-profile topic pair, §7.5.1.2: no header, the bare types. The
/// reply carries its own `remoteEx` member (§7.5.1.2.5 rule 5).
struct EnhancedRequest {
  std::int32_t a = 0;
  std::int32_t b = 0;
};
struct EnhancedReply {
  std::int32_t sum = 0;
  std::int32_t remote_ex = 0;
};
/// A reply type with no `remoteEx` member.
struct PlainReply {
  std::int32_t value = 0;
};



/// §7.5.1.1.6 rule 5a gives the case label as the HASH of the operation
/// name, and the generated header declares it beside the union.
constexpr std::int32_t OP_ADD = robot::Calculator_add_Hash;


}  // namespace

// -- the codecs the two topic types need -------------------------------
//
// §7.11.1.4.22's `dds_type_traits<T>`. The two topic types of the
// Calculator interface have theirs from the generated header; the pair
// below, which stands for an Enhanced-profile interface (§7.5.1.2), is
// specialised by hand.

namespace dds {
namespace rpc {

template <>
struct dds_type_traits<EnhancedRequest> : dds_type_traits_base<EnhancedRequest> {
  static const char* type_name() { return "EnhancedRequest"; }
  static Octets serialize(const EnhancedRequest& value) {
    Octets out;
    detail::put_u32(out, static_cast<std::uint32_t>(value.a));
    detail::put_u32(out, static_cast<std::uint32_t>(value.b));
    return out;
  }
  static bool deserialize(const Octets& in, std::size_t& at, EnhancedRequest& out) {
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
struct dds_type_traits<EnhancedReply> : dds_type_traits_base<EnhancedReply> {
  static const char* type_name() { return "EnhancedReply"; }
  static Octets serialize(const EnhancedReply& value) {
    Octets out;
    detail::put_u32(out, static_cast<std::uint32_t>(value.sum));
    detail::put_u32(out, static_cast<std::uint32_t>(value.remote_ex));
    return out;
  }
  static bool deserialize(const Octets& in, std::size_t& at, EnhancedReply& out) {
    std::uint32_t sum = 0;
    std::uint32_t code = 0;
    if (!detail::get_u32(in, at, sum) || !detail::get_u32(in, at, code)) {
      return false;
    }
    out.sum = static_cast<std::int32_t>(sum);
    out.remote_ex = static_cast<std::int32_t>(code);
    return true;
  }
  static RemoteExceptionCode_t remote_exception(const EnhancedReply& value) {
    return static_cast<RemoteExceptionCode_t>(value.remote_ex);
  }
  static EnhancedReply from_remote_exception(RemoteExceptionCode_t code) {
    EnhancedReply reply;
    reply.remote_ex = static_cast<std::int32_t>(code);
    return reply;
  }
};

template <>
struct dds_type_traits<PlainReply> : dds_type_traits_base<PlainReply> {
  static const char* type_name() { return "PlainReply"; }
  static Octets serialize(const PlainReply& value) {
    Octets out;
    detail::put_u32(out, static_cast<std::uint32_t>(value.value));
    return out;
  }
  static bool deserialize(const Octets& in, std::size_t& at, PlainReply& out) {
    std::uint32_t value = 0;
    if (!detail::get_u32(in, at, value)) {
      return false;
    }
    out.value = static_cast<std::int32_t>(value);
    return true;
  }
};

}  // namespace rpc
}  // namespace dds

namespace {


robot::Calculator_Request add_request(std::int32_t a, std::int32_t b) {
  robot::Calculator_Request request{};
  request.data._d = OP_ADD;
  request.data.add.a = a;
  request.data.add.b = b;
  return request;
}

/// §7.11.1.4.7's synchronous replier listener.
class AddReplier
    : public dds::rpc::core::SimpleReplierListener<robot::Calculator_Request, robot::Calculator_Reply> {
 public:
  robot::Calculator_Reply* process_request(
      const dds::rpc::Sample<robot::Calculator_Request>& request,
      const dds::SampleIdentity& identity) override {
    ++calls;
    last_identity = identity;
    auto* reply = new robot::Calculator_Reply{};
    reply->data._d = request.data().data._d;
    reply->data.add._d = dds::rpc::RETCODE_OK;
    reply->data.add.result.return_ = request.data().data.add.a + request.data().data.add.b;
    return reply;
  }

  dds::SampleIdentity last_identity;

  int calls = 0;
};

/// §7.11.1.4.9's requester listener.
class CollectReplies : public dds::rpc::core::SimpleRequesterListener<robot::Calculator_Reply> {
 public:
  void process_reply(const dds::rpc::Sample<robot::Calculator_Reply>& reply,
                     const dds::SampleIdentity& identity) override {
    sums.push_back(reply.data().data.add.result.return_);
    identities.push_back(identity);
  }

  std::vector<std::int32_t> sums;
  std::vector<dds::SampleIdentity> identities;
};

/// §7.11.1.5.1's `Service`, over the generated interface.
class CalculatorService final : public dds::rpc::Service {
 public:
  CalculatorService(std::shared_ptr<dds::rpc::DomainParticipant> participant, std::string name)
      : name_(std::move(name)), listener_(std::make_shared<AddReplier>()) {
    dds::rpc::core::ReplierParams params;
    params.domain_participant(std::move(participant))
        .request_topic_name(robot::CalculatorSupport::request_topic_name())
        .reply_topic_name(robot::CalculatorSupport::reply_topic_name())
        // §7.11.1.4.3: the instance this replier answers; it must equal
        // the instance the requester bound to.
        .instance_name(name_);
    replier_ = robot::CalculatorReplier(params);
  }

  /// §7.11.1.5.3: a Server dispatches, so the replier here has no arrival
  /// listener and the requests wait for `dispatch()`.
  std::size_t dispatch() override {
    std::size_t answered = 0;
    auto requests = replier_.receive_requests();
    for (auto request : requests) {
      const std::unique_ptr<robot::Calculator_Reply> reply(
          listener_->process_request(request.sample(), request.info().identity));
      replier_.send_reply(*reply, request.info().identity);
      ++answered;
    }
    return answered;
  }
  const std::string& service_name() const override { return name_; }
  int answered() const { return listener_->calls; }

 private:
  std::string name_;
  std::shared_ptr<AddReplier> listener_;
  robot::CalculatorReplier replier_;
};

int failures = 0;

void check(bool held, const char* what) {
  if (!held) {
    std::fprintf(stderr, "FAIL %s\n", what);
    ++failures;
  }
}

/// §7.11.1.3, §7.11.1.4.3, §7.4.2.3, §7.11.1.4.23: the shape of the classes.
void check_class_form(const std::shared_ptr<LoopbackParticipant>& participant) {
  using dds::rpc::BindingStyle;
  using dds::rpc::core::synthesize_topic_names;

  // §7.4.1: the function-call style captures the interface name, the
  // request-reply style does not, and the service name defaults to
  // "Service".
  const auto fc = synthesize_topic_names(BindingStyle::FunctionCall, "robot_Calculator", "", "",
                                         "", "", "", "");
  check(fc.request == "robot_Calculator_Service_Request" &&
            fc.reply == "robot_Calculator_Service_Reply",
        "the function-call default topic names capture the interface name");
  const auto rr = synthesize_topic_names(BindingStyle::RequestReply, "robot_Calculator", "", "",
                                         "", "", "", "");
  check(rr.request == "Service_Request" && rr.reply == "Service_Reply",
        "the request-reply style does not capture the interface name");
  // §7.4.2.3: a run-time service name, a run-time suffix and an
  // annotated whole name, in that order of precedence.
  const auto named = synthesize_topic_names(BindingStyle::FunctionCall, "robot_Calculator", "Lab",
                                            "", "", "", "AnnotatedRequest", "");
  check(named.request == "robot_Calculator_Lab_Request",
        "a run-time service name beats an annotated topic name");
  check(named.reply == "robot_Calculator_Lab_Reply", "and builds the reply topic too");
  const auto suffixed = synthesize_topic_names(BindingStyle::FunctionCall, "robot_Calculator",
                                               "Lab", "", "Calls", "Answers", "", "");
  check(suffixed.request == "robot_Calculator_Calls" && suffixed.reply == "robot_Calculator_Answers",
        "a run-time topic suffix replaces the whole tail");
  const auto annotated = synthesize_topic_names(BindingStyle::FunctionCall, "robot_Calculator",
                                                "", "", "", "", "ReqTopic", "RepTopic");
  check(annotated.request == "ReqTopic" && annotated.reply == "RepTopic",
        "an annotated whole topic name beats the default");
  bool refused = false;
  try {
    (void)synthesize_topic_names(BindingStyle::FunctionCall, "robot_Calculator", "bad-name", "",
                                 "", "", "", "");
  } catch (const dds::rpc::InvalidArgumentError&) {
    refused = true;
  }
  check(refused, "a service name outside [A-Za-z0-9_ ] is a local InvalidArgumentError");

  // §7.4.2.3: the params carry the service name and the suffixes, and the
  // requester announces the names they give.
  {
    dds::rpc::core::RequesterParams params;
    params.domain_participant(participant)
        .interface_name("robot_Calculator")
        .binding_style(BindingStyle::FunctionCall)
        .service_name("Lab");
    robot::CalculatorRequester lab(params);
    check(participant->type_name_of("robot_Calculator_Lab_Request") == "robot::Calculator_Request",
          "a service name in the params names the request topic");
    check(participant->type_name_of("robot_Calculator_Lab_Reply") == "robot::Calculator_Reply",
          "and the reply topic");
    check(!lab.is_bound(), "a service name is not an instance name");
  }

  // §7.10.2: nothing specified, the defaults reach the endpoints.
  {
    dds::rpc::core::RequesterParams params;
    params.domain_participant(participant)
        .request_topic_name("QosRequest")
        .reply_topic_name("QosReply");
    robot::CalculatorRequester defaults(params);
    const dds::rpc::EndpointQos expected;
    check(participant->qos_of("QosRequest") == expected, "the request writer has the defaults");
    check(expected.reliability == dds::rpc::ReliabilityKind::RELIABLE &&
              expected.history == dds::rpc::HistoryKind::KEEP_ALL &&
              expected.durability == dds::rpc::DurabilityKind::VOLATILE,
          "the defaults are §7.10.2's");

    dds::rpc::RpcQos qos;
    qos.request.history = dds::rpc::HistoryKind::KEEP_LAST;
    qos.request.history_depth = 7;
    qos.reply.reliability = dds::rpc::ReliabilityKind::BEST_EFFORT;
    dds::rpc::core::RequesterParams tuned;
    tuned.domain_participant(participant)
        .request_topic_name("QosRequestTuned")
        .reply_topic_name("QosReplyTuned")
        .qos(qos);
    robot::CalculatorRequester custom(tuned);
    check(participant->qos_of("QosRequestTuned").history_depth == 7 &&
              participant->qos_of("QosRequestTuned").history == dds::rpc::HistoryKind::KEEP_LAST,
          "the request QoS of the params reaches the request writer");
    check(participant->qos_of("QosReplyTuned").reliability ==
              dds::rpc::ReliabilityKind::BEST_EFFORT,
          "the reply QoS of the params reaches the reply reader");
  }

  // §7.11.1.4.3, §7.11.1.4.5: Requester and Replier are reference types.
  {
    dds::rpc::core::RequesterParams params;
    params.domain_participant(participant)
        .request_topic_name("RefRequest")
        .reply_topic_name("RefReply");
    robot::CalculatorRequester original(params);
    robot::CalculatorRequester copy = original;
    copy.bind("shared");
    check(original.is_bound() && original.bound_instance_name() == "shared",
          "a copy of a requester shares its binding");
    copy.close();
    check(original.is_null(), "closing a copy closes the entities every copy shares");
  }
  {
    robot::CalculatorRequester empty;
    check(empty.is_null(), "a default-constructed requester holds nothing");
    bool null_reference = false;
    try {
      (void)empty.send_request(add_request(1, 1));
    } catch (const dds::rpc::NullReferenceError&) {
      null_reference = true;
    }
    check(null_reference, "sending through an empty requester is a NullReferenceError");

    dds::rpc::core::RequesterParams params;
    params.domain_participant(participant)
        .request_topic_name("ClosedRequest")
        .reply_topic_name("ClosedReply");
    robot::CalculatorRequester closed(params);
    closed.close();
    bool already_closed = false;
    try {
      (void)closed.send_request(add_request(1, 1));
    } catch (const dds::rpc::AlreadyClosedError&) {
      already_closed = true;
    }
    check(already_closed, "sending through a closed requester is an AlreadyClosedError");

    dds::rpc::core::ReplierParams no_participant;
    bool local = false;
    try {
      robot::CalculatorReplier orphan(no_participant);
    } catch (const dds::rpc::LocalError&) {
      local = true;
    }
    check(local, "a replier with no participant is a local error, not a remote one");
  }

  // §7.11.1.4.18: a WriteSampleRef takes the identity of the request sent.
  {
    dds::rpc::core::RequesterParams params;
    params.domain_participant(participant)
        .request_topic_name("WsrRequest")
        .reply_topic_name("WsrReply");
    robot::CalculatorRequester requester(params);
    auto sample = std::make_shared<dds::rpc::WriteSample<robot::Calculator_Request>>(
        add_request(4, 5));
    dds::rpc::WriteSampleRef<robot::Calculator_Request> reference(sample);
    requester.send_request(reference);
    check(!reference.identity().is_unknown() && reference.identity() == sample->identity(),
          "send_request(WriteSampleRef) writes the identity back into the sample it refers to");
  }

  // §7.11.1.4.23: the traits name the entity types, the QoS types included.
  static_assert(
      std::is_same<dds::dds_entity_traits::DataWriterQos, dds::rpc::EndpointQos>::value,
      "dds_entity_traits names DataWriterQos");
  static_assert(
      std::is_same<dds::dds_entity_traits::Publisher, std::shared_ptr<dds::rpc::Publisher>>::value,
      "dds_entity_traits names Publisher");
  static_assert(
      std::is_same<dds::dds_entity_traits::Subscriber, std::shared_ptr<dds::rpc::Subscriber>>::value,
      "dds_entity_traits names Subscriber");

  // §7.11.1.5.2: ServiceEndpoint is a reference type.
  {
    auto service = std::make_shared<CalculatorService>(participant, "refs");
    dds::rpc::ServiceEndpoint& endpoint = *service;
    service->pause();
    check(endpoint.status() == dds::rpc::PAUSED, "pause() is seen through the base");
  }
}


/// §7.8.2, §7.5.1.2: the Enhanced profile correlates by the sample identity
/// and sends no header.
void check_enhanced_profile() {
  using dds::rpc::Correlation;
  Bus bus;
  bus.expose_identity = true;
  auto participant = std::make_shared<LoopbackParticipant>(bus);

  dds::rpc::core::RequesterParams requester_params;
  requester_params.domain_participant(participant)
      .request_topic_name("EnhancedRequest")
      .reply_topic_name("EnhancedReply")
      .correlation(Correlation::SAMPLE_IDENTITY);
  dds::rpc::core::Requester<EnhancedRequest, EnhancedReply> requester(requester_params);
  dds::rpc::core::ReplierParams replier_params;
  replier_params.domain_participant(participant)
      .request_topic_name("EnhancedRequest")
      .reply_topic_name("EnhancedReply")
      .correlation(Correlation::SAMPLE_IDENTITY);
  dds::rpc::core::Replier<EnhancedRequest, EnhancedReply> replier(replier_params);

  const dds::SampleIdentity id = requester.send_request(EnhancedRequest{2, 3});
  check(!id.is_unknown(), "the request has the identity of its sample");
  check(bus.topic("EnhancedRequest").samples.size() == 1 &&
            bus.topic("EnhancedRequest").samples[0].first.size() == 8,
        "the Enhanced request is the bare topic type, with no header (§7.8.2)");

  auto requests = replier.receive_requests();
  check(requests.length() == 1 && requests[0].data().a == 2 && requests[0].data().b == 3,
        "the replier reads the bare request");
  check(requests[0].info().identity == id,
        "the request's identity is the identity of its DATA (§7.8.2.1)");

  replier.send_reply(EnhancedReply{5, 0}, requests[0].info().identity);
  // The requester reads its replies when they arrive, so the reply is read
  // from what was written, not from the queue.
  const auto& sent = bus.topic("EnhancedReply").written;
  check(sent.size() == 1 && sent[0].first.size() == 8,
        "the Enhanced reply is the bare topic type");
  check(sent[0].second.has_related_identity && sent[0].second.related_identity == id,
        "the reply carries the request's identity as its related sample identity (§7.8.2.3)");

  dds::rpc::Sample<EnhancedReply> reply;
  check(requester.receive_reply(id, reply) && reply.data().sum == 5,
        "the requester correlates by the related sample identity (§7.8.2.2)");
  check(reply.related_identity() == id, "the Sample reports it");

  // §7.2.1: a reply to another client's request is not this requester's, and
  // a reply with no related identity cannot be tied to any request.
  {
    auto foreign = participant->publisher()->create_writer("EnhancedReply", "EnhancedReply",
                                                            dds::rpc::EndpointQos{});
    dds::SampleIdentity other;
    other.writer_guid[0] = 0xEE;
    other.sequence_number.low = 1;
    (void)foreign->write_related(dds::rpc::dds_type_traits<EnhancedReply>::serialize({7, 0}),
                                 other);
    (void)foreign->write(dds::rpc::dds_type_traits<EnhancedReply>::serialize({8, 0}));
    check(requester.receive_replies().length() == 0,
          "a reply to another client, or with no related identity, is filtered out");
  }

  // §7.5.1.2.5 rule 5: the code is the `remoteEx` member of the reply.
  {
    const dds::SampleIdentity failing = requester.send_request(EnhancedRequest{0, 0});
    auto taken = replier.receive_requests();
    check(taken.length() == 1, "the replier took the second request");
    replier.send_reply(EnhancedReply{}, taken[0].info().identity,
                       dds::rpc::REMOTE_EX_UNKNOWN_OPERATION);
    bool raised = false;
    try {
      dds::rpc::Sample<EnhancedReply> unused;
      (void)requester.receive_reply(failing, unused);
    } catch (const dds::rpc::RemoteUnknownOperationError&) {
      raised = true;
    }
    check(raised, "the remoteEx member of an Enhanced reply raises the remote exception");
  }

  // A reply type with no remoteEx member cannot carry a code.
  {
    dds::rpc::core::ReplierParams plain_params;
    plain_params.domain_participant(participant)
        .request_topic_name("PlainRequest")
        .reply_topic_name("PlainReply")
        .correlation(Correlation::SAMPLE_IDENTITY);
    dds::rpc::core::Replier<EnhancedRequest, PlainReply> plain(plain_params);
    bool unsupported = false;
    try {
      plain.send_reply(PlainReply{}, dds::SampleIdentity{}, dds::rpc::REMOTE_EX_UNSUPPORTED);
    } catch (const dds::rpc::UnsupportedError&) {
      unsupported = true;
    }
    check(unsupported, "a code cannot be sent in a reply type that has no remoteEx");
  }

  // §7.6.2.1: an instance name travels in discovery in the Enhanced profile,
  // which this binding cannot announce or read; it is refused, not ignored.
  {
    requester.bind("instance");
    bool unsupported = false;
    try {
      (void)requester.send_request(EnhancedRequest{1, 1});
    } catch (const dds::rpc::UnsupportedError&) {
      unsupported = true;
    }
    check(unsupported, "an Enhanced request bound to an instance is refused");
    requester.unbind();

    dds::rpc::core::ReplierParams named = replier_params;
    named.instance_name("Named");
    bool refused = false;
    try {
      dds::rpc::core::Replier<EnhancedRequest, EnhancedReply> named_replier(named);
    } catch (const dds::rpc::UnsupportedError&) {
      refused = true;
    }
    check(refused, "an Enhanced replier with an instance name is refused");
  }
}


/// §7.6.2.2: no request goes out before the client has matched the service,
/// and no request is processed before the service has matched the client.
void check_service_matching() {
  using dds::rpc::ClientMatching;
  using dds::rpc::Correlation;
  using dds::rpc::ServiceMatching;
  using Req = dds::rpc::core::Requester<EnhancedRequest, EnhancedReply>;
  using Rep = dds::rpc::core::Replier<EnhancedRequest, EnhancedReply>;

  // §7.6.2.2.2: the request waits, and goes out when the service matches.
  {
    Bus bus;
    bus.expose_identity = true;
    auto participant = std::make_shared<LoopbackParticipant>(bus);
    dds::rpc::core::RequesterParams requester_params;
    requester_params.domain_participant(participant)
        .request_topic_name("MReq")
        .reply_topic_name("MRep")
        .correlation(Correlation::SAMPLE_IDENTITY);
    Req requester(requester_params);
    check(!requester.is_service_discovered(), "no service has matched yet");

    const dds::SampleIdentity first = requester.send_request(EnhancedRequest{1, 2});
    const dds::SampleIdentity second = requester.send_request(EnhancedRequest{3, 4});
    check(bus.topic("MReq").samples.empty(), "nothing is written before the service matches");
    check(requester.queued_requests() == 2, "both requests wait");
    check(!first.is_unknown() && first != second &&
              second.sequence_number.low == first.sequence_number.low + 1,
          "a queued request is given the identity it will have");

    dds::rpc::core::ReplierParams replier_params;
    replier_params.domain_participant(participant)
        .request_topic_name("MReq")
        .reply_topic_name("MRep")
        .correlation(Correlation::SAMPLE_IDENTITY);
    Rep replier(replier_params);
    check(requester.is_service_discovered(), "the service has matched");
    check(requester.queued_requests() == 0 && bus.topic("MReq").samples.size() == 2,
          "the queued requests were written when it did");
    auto requests = replier.receive_requests();
    check(requests.length() == 2 && requests[0].info().identity == first &&
              requests[1].info().identity == second,
          "they went out in order under the identities they were given");
    check(requests[0].data().a == 1 && requests[1].data().a == 3, "with their own contents");
  }

  // The Basic header names the identity the queued request will have.
  {
    Bus bus;
    auto participant = std::make_shared<LoopbackParticipant>(bus);
    dds::rpc::core::RequesterParams requester_params;
    requester_params.domain_participant(participant)
        .request_topic_name("BReq")
        .reply_topic_name("BRep")
        .service_matching(ServiceMatching::QUEUE);
    robot::CalculatorRequester requester(requester_params);
    const dds::SampleIdentity id = requester.send_request(add_request(1, 2));
    check(bus.topic("BReq").samples.empty(), "a Basic request waits for the service too");
    dds::rpc::core::ReplierParams replier_params;
    replier_params.domain_participant(participant)
        .request_topic_name("BReq")
        .reply_topic_name("BRep");
    robot::CalculatorReplier replier(replier_params);
    check(requester.is_service_discovered(), "the Basic service matched");
    auto requests = replier.receive_requests();
    check(requests.length() == 1 && requests[0].info().identity == id,
          "the header carries the identity the request was given when it waited");
  }

  // REFUSE reports it, NONE writes at once.
  {
    Bus bus;
    auto participant = std::make_shared<LoopbackParticipant>(bus);
    dds::rpc::core::RequesterParams refuse;
    refuse.domain_participant(participant)
        .request_topic_name("RReq")
        .reply_topic_name("RRep")
        .service_matching(ServiceMatching::REFUSE);
    robot::CalculatorRequester refusing(refuse);
    bool refused = false;
    try {
      (void)refusing.send_request(add_request(1, 1));
    } catch (const dds::rpc::PreconditionNotMetError&) {
      refused = true;
    }
    check(refused, "an unmatched service refuses the request when told to");
    check(bus.topic("RReq").samples.empty(), "and writes nothing");

    dds::rpc::core::RequesterParams none;
    none.domain_participant(participant)
        .request_topic_name("NReq")
        .reply_topic_name("NRep")
        .correlation(Correlation::SAMPLE_IDENTITY)
        .service_matching(ServiceMatching::NONE);
    Req unguarded(none);
    (void)unguarded.send_request(EnhancedRequest{9, 9});
    check(bus.topic("NReq").samples.size() == 1, "NONE writes before the service matched");
  }

  // §7.6.2.2.1: the replier holds a request until a client's reply reader
  // is matched, and does not lose it.
  {
    Bus bus;
    bus.expose_identity = true;
    auto participant = std::make_shared<LoopbackParticipant>(bus);
    dds::rpc::core::ReplierParams replier_params;
    replier_params.domain_participant(participant)
        .request_topic_name("HReq")
        .reply_topic_name("HRep")
        .correlation(Correlation::SAMPLE_IDENTITY);
    Rep replier(replier_params);
    auto early = participant->publisher()->create_writer("HReq", "EnhancedRequest",
                                                          dds::rpc::EndpointQos{});
    (void)early->write(dds::rpc::dds_type_traits<EnhancedRequest>::serialize({5, 6}));
    check(replier.receive_requests().length() == 0,
          "a request is held while no client reply reader is matched");
    auto client_reader = participant->subscriber()->create_reader("HRep", "EnhancedReply",
                                                                   dds::rpc::EndpointQos{});
    auto released = replier.receive_requests();
    check(released.length() == 1 && released[0].data().a == 5,
          "and handed out, not lost, once the client matched");

    dds::rpc::core::ReplierParams eager = replier_params;
    eager.request_topic_name("EReq2").reply_topic_name("ERep2").client_matching(
        ClientMatching::NONE);
    Rep unguarded(eager);
    auto writer = participant->publisher()->create_writer("EReq2", "EnhancedRequest",
                                                           dds::rpc::EndpointQos{});
    (void)writer->write(dds::rpc::dds_type_traits<EnhancedRequest>::serialize({7, 7}));
    check(unguarded.receive_requests().length() == 1, "NONE processes the request at once");
  }

  // §7.6.2.1: the client side announces no service_instance_name. This
  // runtime announces nothing about an instance at all; the binding is the
  // header member of the Basic profile, and a Requester that is bound
  // announces no endpoint data.
  {
    Bus bus;
    auto participant = std::make_shared<LoopbackParticipant>(bus);
    dds::rpc::core::RequesterParams params;
    params.domain_participant(participant)
        .request_topic_name("IReq")
        .reply_topic_name("IRep")
        .instance_name("bound-to");
    robot::CalculatorRequester bound(params);
    check(bound.is_bound() && bound.bound_instance_name() == "bound-to",
          "the instance a requester is bound to is kept for its requests only");
  }
}


/// §7.6.2: with a binding that announces and reads the discovery extension,
/// the Enhanced endpoints announce it, the client addresses the service
/// instance it is bound to, and the service masks each client until it has
/// discovered both of its endpoints.
void check_enhanced_discovery() {
  using dds::rpc::ClientMatching;
  using dds::rpc::Correlation;
  using Req = dds::rpc::core::Requester<EnhancedRequest, EnhancedReply>;
  using Rep = dds::rpc::core::Replier<EnhancedRequest, EnhancedReply>;

  Bus bus;
  bus.expose_identity = true;
  bus.rpc_discovery = true;
  auto participant = std::make_shared<LoopbackParticipant>(bus);

  dds::rpc::core::ReplierParams replier_params;
  replier_params.domain_participant(participant)
      .request_topic_name("DReq")
      .reply_topic_name("DRep")
      .correlation(Correlation::SAMPLE_IDENTITY)
      .instance_name("calc-A")
      .parent_interface_names({"Base"});
  // §7.6.2.1.1: an Enhanced service with an instance name is no longer
  // refused: the name is announced.
  Rep replier(replier_params);

  dds::rpc::core::RequesterParams requester_params;
  requester_params.domain_participant(participant)
      .request_topic_name("DReq")
      .reply_topic_name("DRep")
      .correlation(Correlation::SAMPLE_IDENTITY);
  Req requester(requester_params);

  // What each endpoint announced.
  {
    std::lock_guard<std::recursive_mutex> guard(bus.lock);
    Topic& request = bus.topic("DReq");
    Topic& reply = bus.topic("DRep");
    check(request.writer_objects.size() == 1 && request.reader_objects.size() == 1 &&
              reply.writer_objects.size() == 1 && reply.reader_objects.size() == 1,
          "one endpoint of each kind");
    const auto& request_reader = request.reader_objects[0]->announced();
    check(request_reader.has_instance_name && request_reader.instance_name == "calc-A",
          "the service-side request reader announces the instance name (§7.6.2.1.2)");
    check(request_reader.has_related &&
              std::equal(request_reader.related, request_reader.related + 16,
                         reply.writer_objects[0]->guid()),
          "and names its reply writer");
    check(request_reader.aliases.size() == 1,
          "and carries the topic alias of the parent interface (§7.4.3)");
    const auto& reply_writer = reply.writer_objects[0]->announced();
    check(reply_writer.has_instance_name && reply_writer.instance_name == "calc-A" &&
              reply_writer.has_related &&
              std::equal(reply_writer.related, reply_writer.related + 16,
                         request.reader_objects[0]->key()),
          "the reply writer announces the name and its request reader (§7.6.2.1.1)");
    const auto& request_writer = request.writer_objects[0]->announced();
    check(!request_writer.has_instance_name && request_writer.has_related &&
              std::equal(request_writer.related, request_writer.related + 16,
                         reply.reader_objects[0]->key()),
          "the client announces no instance name, only its reply reader (§7.6.2.1.1)");
    const auto& reply_reader = reply.reader_objects[0]->announced();
    check(!reply_reader.has_instance_name && reply_reader.has_related,
          "the client's reply reader announces no instance name either (§7.6.2.1.2)");
  }

  // Figure 7.4 with the instance name: only the bound instance is served.
  requester.bind("calc-A");
  check(requester.is_service_discovered(), "the client has discovered calc-A");
  {
    const dds::Duration soon = dds::Duration::from_millisecs(30);
    const std::vector<std::string> found = requester.get_discoverd_service_instances();
    check(found.size() == 1 && found[0] == "calc-A",
          "the discovered instances are those the services announced");
    check(requester.get_bound_instance_name() == "calc-A", "the bound instance");
    requester.wait_for_service();
    requester.wait_for_service(soon);
    requester.wait_for_service(std::string("calc-A"));
    requester.wait_for_services(1);
    requester.wait_for_services(std::vector<std::string>{"calc-A"});
    requester.wait_for_service_async("calc-A").get();
    requester.wait_for_services_async(1).get();
    bool timed_out = false;
    try {
      requester.wait_for_service(soon, std::string("calc-B"));
    } catch (const dds::rpc::TimeoutError&) {
      timed_out = true;
    }
    check(timed_out, "waiting for an instance nobody announced times out");
    timed_out = false;
    try {
      requester.wait_for_services(soon, std::vector<std::string>{"calc-A", "calc-B"});
    } catch (const dds::rpc::TimeoutError&) {
      timed_out = true;
    }
    check(timed_out, "and so does waiting for a set of which one is missing");
    timed_out = false;
    try {
      requester.wait_for_services(soon, 2);
    } catch (const dds::rpc::TimeoutError&) {
      timed_out = true;
    }
    check(timed_out, "and for more instances than there are");
  }
  const dds::SampleIdentity id = requester.send_request(EnhancedRequest{1, 2});
  check(bus.topic("DReq").samples.size() == 1, "a request bound to calc-A goes out");
  requester.bind("calc-B");
  check(!requester.is_service_discovered(), "calc-B is not served");
  (void)requester.send_request(EnhancedRequest{3, 4});
  check(bus.topic("DReq").samples.size() == 1 && requester.queued_requests() == 1,
        "a request bound to an instance that is not served waits");
  requester.unbind();
  check(!requester.is_service_discovered(), "the unnamed instance is not served either");

  // Figure 7.3: the request of the matched client is processed; one from a
  // writer the service has not discovered is held.
  auto taken = replier.receive_requests();
  check(taken.length() == 1 && taken[0].info().identity == id,
        "the matched client's request is processed");
  auto stranger = participant->publisher()->create_writer("DReq", "EnhancedRequest",
                                                           dds::rpc::EndpointQos{});
  (void)stranger->write(dds::rpc::dds_type_traits<EnhancedRequest>::serialize({7, 7}));
  check(replier.receive_requests().length() == 0,
        "a request of a client writer that announced no reply reader is held (Figure 7.3)");

  // The named alternative reads the stranger as complete by the match alone.
  dds::rpc::core::ReplierParams lenient = replier_params;
  lenient.request_topic_name("LReq").reply_topic_name("LRep").instance_name("").accept_unannounced(
      true);
  Rep accepting(lenient);
  auto outsider = participant->publisher()->create_writer("LReq", "EnhancedRequest",
                                                           dds::rpc::EndpointQos{});
  (void)outsider->write(dds::rpc::dds_type_traits<EnhancedRequest>::serialize({8, 8}));
  check(accepting.receive_requests().length() == 1,
        "accept_unannounced processes a request of an unannounced client");

  // A name the binding could only read cut short is never compared: the
  // cut of a longer name must not match an instance of that name.
  {
    dds::rpc::RpcPeer reader;
    dds::rpc::RpcPeer writer;
    reader.key[0] = 1;
    writer.key[0] = 2;
    reader.announced.has_instance_name = true;
    reader.announced.instance_name = "calc";
    reader.announced.has_related = true;
    reader.announced.related[0] = 2;
    writer.announced.has_related = true;
    writer.announced.related[0] = 1;
    check(dds::rpc::service_matching_complete({reader}, {writer}, "calc", false),
          "a whole name matches");
    reader.announced.name_truncated = true;
    check(!dds::rpc::service_matching_complete({reader}, {writer}, "calc", false),
          "a truncated name does not");
  }

  // ClientMatching::NONE and a stranger.
  dds::rpc::core::ReplierParams unguarded = replier_params;
  unguarded.request_topic_name("UReq").reply_topic_name("URep").instance_name("").client_matching(
      ClientMatching::NONE);
  Rep open(unguarded);
  auto other = participant->publisher()->create_writer("UReq", "EnhancedRequest",
                                                        dds::rpc::EndpointQos{});
  (void)other->write(dds::rpc::dds_type_traits<EnhancedRequest>::serialize({9, 9}));
  check(open.receive_requests().length() == 1, "ClientMatching::NONE has no mask");
}

/// DDS-RPC 1.0 §7.11.1.4.3: "Requests sent through a bound requester
/// reference shall be sent to the bound service instance only." With the
/// discovery extension (§7.6.2.1) the instance is the readers that announced
/// its name, and the Enhanced request, which has no header to carry the name,
/// is written to those readers and to no other.
void check_instance_addressing() {
  using dds::rpc::Correlation;
  using Req = dds::rpc::core::Requester<EnhancedRequest, EnhancedReply>;
  using Rep = dds::rpc::core::Replier<EnhancedRequest, EnhancedReply>;

  Bus bus;
  bus.expose_identity = true;
  bus.rpc_discovery = true;
  bus.per_reader_queues = true;
  auto participant = std::make_shared<LoopbackParticipant>(bus);

  auto make_replier = [&participant](const char* name) {
    dds::rpc::core::ReplierParams params;
    params.domain_participant(participant)
        .request_topic_name("AReq")
        .reply_topic_name("ARep")
        .correlation(Correlation::SAMPLE_IDENTITY)
        .instance_name(name);
    return Rep(params);
  };
  Rep calc_a = make_replier("calc-A");
  Rep calc_b = make_replier("calc-B");
  dds::rpc::core::RequesterParams requester_params;
  requester_params.domain_participant(participant)
      .request_topic_name("AReq")
      .reply_topic_name("ARep")
      .correlation(Correlation::SAMPLE_IDENTITY);
  Req requester(requester_params);

  requester.bind("calc-A");
  const dds::SampleIdentity to_a = requester.send_request(EnhancedRequest{1, 2});
  auto at_a = calc_a.receive_requests();
  check(at_a.length() == 1 && at_a[0].identity() == to_a && at_a[0].data().a == 1,
        "a request bound to calc-A reaches calc-A, with the identity it was promised");
  check(calc_b.receive_requests().length() == 0, "and not calc-B");

  requester.bind("calc-B");
  const dds::SampleIdentity to_b = requester.send_request(EnhancedRequest{3, 4});
  auto at_b = calc_b.receive_requests();
  check(at_b.length() == 1 && at_b[0].identity() == to_b && at_b[0].data().a == 3,
        "a request bound to calc-B reaches calc-B");
  check(calc_a.receive_requests().length() == 0, "and not calc-A");

  // The default instance is the one that announced no name: with none, the
  // request waits (§7.6.2.2.2), and goes to the unnamed one once it is there.
  requester.unbind();
  const dds::SampleIdentity to_default = requester.send_request(EnhancedRequest{5, 6});
  check(requester.queued_requests() == 1 && calc_a.receive_requests().length() == 0 &&
            calc_b.receive_requests().length() == 0,
        "a request for the default instance waits while there is none");
  Rep unnamed = make_replier("");
  check(requester.is_service_discovered(), "the unnamed instance is discovered");
  auto at_default = unnamed.receive_requests();
  check(at_default.length() == 1 && at_default[0].identity() == to_default &&
            at_default[0].data().a == 5 && requester.queued_requests() == 0,
        "and the waiting request reaches it");
  check(calc_a.receive_requests().length() == 0 && calc_b.receive_requests().length() == 0,
        "and neither of the named ones");
}

/// §7.10: QoS in the params, QoS profile URLs, mutable policies at run time.
void check_qos(const std::shared_ptr<LoopbackParticipant>& participant) {
  using dds::rpc::EndpointQos;
  using dds::rpc::HistoryKind;
  using dds::rpc::ReliabilityKind;

  // §7.10.1: `@Qos(RequestProfile, ReplyProfile)` name one profile each.
  EndpointQos request_profile;
  request_profile.history = HistoryKind::KEEP_LAST;
  request_profile.history_depth = 5;
  EndpointQos reply_profile;
  reply_profile.reliability = ReliabilityKind::BEST_EFFORT;
  participant->define_profile("file:///qos.xml#Requests", request_profile);
  participant->define_profile("file:///qos.xml#Replies", reply_profile);
  {
    dds::rpc::core::RequesterParams params;
    params.domain_participant(participant)
        .request_topic_name("ProfRequest")
        .reply_topic_name("ProfReply")
        .request_profile("file:///qos.xml#Requests")
        .reply_profile("file:///qos.xml#Replies");
    robot::CalculatorRequester requester(params);
    check(participant->qos_of("ProfRequest") == request_profile,
          "RequestProfile gives the request writer its QoS");
    check(participant->qos_of("ProfReply") == reply_profile,
          "ReplyProfile gives the reply reader its QoS");

    // A QoS given at run time wins over a profile.
    dds::rpc::RpcQos explicit_qos;
    explicit_qos.request.history_depth = 9;
    explicit_qos.request.history = HistoryKind::KEEP_LAST;
    dds::rpc::core::RequesterParams both = params;
    both.request_topic_name("ProfRequest2").reply_topic_name("ProfReply2").qos(explicit_qos);
    robot::CalculatorRequester explicit_requester(both);
    check(participant->qos_of("ProfRequest2").history_depth == 9,
          "a run-time QoS takes precedence over a profile");
  }
  {
    dds::rpc::core::ReplierParams params;
    params.domain_participant(participant)
        .request_topic_name("ProfRequestR")
        .reply_topic_name("ProfReplyR")
        .request_profile("file:///qos.xml#Requests")
        .reply_profile("file:///qos.xml#Replies");
    robot::CalculatorReplier replier(params);
    check(participant->qos_of("ProfRequestR") == request_profile &&
              participant->qos_of("ProfReplyR") == reply_profile,
          "the replier takes the same profiles");
  }
  {
    dds::rpc::core::RequesterParams params;
    params.domain_participant(participant)
        .request_topic_name("MissingRequest")
        .reply_topic_name("MissingReply")
        .request_profile("file:///missing.xml");
    bool refused = false;
    try {
      robot::CalculatorRequester requester(params);
    } catch (const dds::rpc::InvalidArgumentError&) {
      refused = true;
    }
    check(refused, "a profile that does not resolve is refused, not replaced by defaults");
  }

  // §7.10.2: the mutable policies change at run time, the immutable ones
  // do not, and a refused change changes nothing.
  {
    dds::rpc::core::RequesterParams params;
    params.domain_participant(participant)
        .request_topic_name("MutRequest")
        .reply_topic_name("MutReply");
    robot::CalculatorRequester requester(params);
    dds::rpc::RpcQos tuned = requester.qos();
    tuned.request.deadline_ns = 500;
    tuned.reply.deadline_ns = 700;
    requester.set_qos(tuned);
    check(participant->qos_of("MutRequest").deadline_ns == 500 &&
              participant->qos_of("MutReply").deadline_ns == 700,
          "set_qos applies the mutable policies to both endpoints");
    check(requester.qos().request.deadline_ns == 500, "and the requester reports them");

    dds::rpc::RpcQos refused_qos = tuned;
    refused_qos.request.deadline_ns = 900;
    refused_qos.reply.reliability = ReliabilityKind::BEST_EFFORT;
    bool immutable = false;
    try {
      requester.set_qos(refused_qos);
    } catch (const dds::rpc::ImmutablePolicyError&) {
      immutable = true;
    }
    check(immutable, "an immutable policy cannot change");
    check(participant->qos_of("MutRequest").deadline_ns == 500,
          "and the request writer went back to what it had");

    dds::rpc::core::ReplierParams replier_params;
    replier_params.domain_participant(participant)
        .request_topic_name("MutRequestR")
        .reply_topic_name("MutReplyR");
    robot::CalculatorReplier replier(replier_params);
    dds::rpc::RpcQos replier_qos = replier.qos();
    replier_qos.reply.lifespan_ns = 42;
    replier.set_qos(replier_qos);
    check(participant->qos_of("MutReplyR").lifespan_ns == 42, "a replier changes its QoS too");
  }
}


/// §7.11.1.4.8's asynchronous replier listener: the callback gets the
/// replier and answers through it.
class AsyncReplier
    : public dds::rpc::core::ReplierListener<robot::Calculator_Request, robot::Calculator_Reply> {
 public:
  void on_request_available(
      dds::rpc::core::Replier<robot::Calculator_Request, robot::Calculator_Reply>& replier) override {
    auto requests = replier.receive_requests();
    for (const auto& request : requests) {
      robot::Calculator_Reply reply{};
      reply.data._d = request.data().data._d;
      reply.data.add._d = dds::rpc::RETCODE_OK;
      reply.data.add.result.return_ = request.data().data.add.a * request.data().data.add.b;
      replier.send_reply(reply, request.info().identity);
      ++calls;
    }
  }

  int calls = 0;
};

/// §7.11.1.4.10's requester listener: the callback gets the requester.
class AsyncRequester
    : public dds::rpc::core::RequesterListener<robot::Calculator_Request, robot::Calculator_Reply> {
 public:
  void on_reply_available(
      dds::rpc::core::Requester<robot::Calculator_Request, robot::Calculator_Reply>& requester) override {
    auto replies = requester.receive_replies();
    for (const auto& reply : replies) {
      products.push_back(reply.data().data.add.result.return_);
    }
    if (fail) {
      throw std::runtime_error("the listener failed");
    }
  }

  std::vector<std::int32_t> products;
  bool fail = false;
};

/// A synchronous listener that cannot answer.
class RefusingReplier
    : public dds::rpc::core::SimpleReplierListener<robot::Calculator_Request, robot::Calculator_Reply> {
 public:
  robot::Calculator_Reply* process_request(const dds::rpc::Sample<robot::Calculator_Request>&,
                                            const dds::SampleIdentity&) override {
    if (remote) {
      throw dds::rpc::RemoteInvalidArgumentError("the argument is not acceptable");
    }
    throw std::runtime_error("something else");
  }

  bool remote = true;
};

/// §7.9.2.2, §7.11.1.4.7-.10: a listener runs when the sample arrives, with
/// no pump. The loopback binding calls the readers' callbacks from the write,
/// as a binding with a receive thread calls them from that thread.
void check_arrival_listeners(const std::shared_ptr<LoopbackParticipant>& participant) {
  auto params_for = [&participant](const char* request, const char* reply) {
    dds::rpc::core::ReplierParams params;
    params.domain_participant(participant).request_topic_name(request).reply_topic_name(reply);
    return params;
  };
  auto requester_for = [&participant](const char* request, const char* reply) {
    dds::rpc::core::RequesterParams params;
    params.domain_participant(participant).request_topic_name(request).reply_topic_name(reply);
    return params;
  };

  // The synchronous replier listener and the requester listener, no pump.
  {
    auto add = std::make_shared<AddReplier>();
    auto collect = std::make_shared<CollectReplies>();
    robot::CalculatorReplier replier(params_for("L1Req", "L1Rep").listener(add));
    robot::CalculatorRequester requester(requester_for("L1Req", "L1Rep").listener(collect));
    (void)requester.send_request(add_request(20, 22));
    check(add->calls == 1, "the synchronous replier listener ran when the request arrived");
    check(collect->sums.size() == 1 && collect->sums[0] == 42,
          "the requester listener ran when the reply arrived");
  }

  // A future is fulfilled when the reply arrives, with no read.
  {
    auto add = std::make_shared<AddReplier>();
    robot::CalculatorReplier replier(params_for("L2Req", "L2Rep").listener(add));
    robot::CalculatorRequester requester(requester_for("L2Req", "L2Rep"));
    auto future = requester.send_request_async(add_request(1, 2));
    check(future.wait_for(std::chrono::seconds(0)) == std::future_status::ready,
          "a future is ready once the reply arrived, with no read");
    check(future.get().data().data.add.result.return_ == 3, "1 + 2 through the future");
  }

  // The asynchronous listeners get the entity they answer or read through.
  {
    auto multiply = std::make_shared<AsyncReplier>();
    auto products = std::make_shared<AsyncRequester>();
    robot::CalculatorReplier replier(params_for("L3Req", "L3Rep").listener(multiply));
    robot::CalculatorRequester requester(requester_for("L3Req", "L3Rep").listener(products));
    (void)requester.send_request(add_request(6, 7));
    check(multiply->calls == 1, "the asynchronous replier listener ran on arrival");
    check(products->products.size() == 1 && products->products[0] == 42,
          "the asynchronous requester listener read the reply on arrival");
  }

  // §7.5.2, §7.11.1.3: a listener that throws a remote exception answers
  // with its code, any other exception is REMOTE_EX_UNKNOWN_EXCEPTION, and
  // the future carries the exception.
  {
    auto refusing = std::make_shared<RefusingReplier>();
    robot::CalculatorReplier replier(params_for("L4Req", "L4Rep").listener(refusing));
    robot::CalculatorRequester requester(requester_for("L4Req", "L4Rep"));
    auto invalid = requester.send_request_async(add_request(1, 1));
    bool invalid_argument = false;
    try {
      (void)invalid.get();
    } catch (const dds::rpc::RemoteInvalidArgumentError&) {
      invalid_argument = true;
    }
    check(invalid_argument, "a thrown remote exception comes back as its own code");
    refusing->remote = false;
    auto unknown = requester.send_request_async(add_request(1, 1));
    bool unknown_exception = false;
    try {
      (void)unknown.get();
    } catch (const dds::rpc::RemoteUnknownExceptionError&) {
      unknown_exception = true;
    }
    check(unknown_exception, "any other exception comes back as REMOTE_EX_UNKNOWN_EXCEPTION");
  }

  // What a listener throws on the binding's thread is kept, not lost.
  {
    auto add = std::make_shared<AddReplier>();
    auto failing = std::make_shared<AsyncRequester>();
    failing->fail = true;
    robot::CalculatorReplier replier(params_for("L5Req", "L5Rep").listener(add));
    robot::CalculatorRequester requester(requester_for("L5Req", "L5Rep").listener(failing));
    (void)requester.send_request(add_request(1, 1));
    bool rethrown = false;
    try {
      (void)requester.receive_replies();
    } catch (const std::runtime_error& error) {
      rethrown = std::string(error.what()) == "the listener failed";
    }
    check(rethrown, "an exception of a listener is raised by the next call");
    check(requester.take_listener_error() == nullptr, "and raised once");
  }

  // A closed replier's listener is not called any more.
  {
    auto add = std::make_shared<AddReplier>();
    robot::CalculatorReplier replier(params_for("L6Req", "L6Rep").listener(add));
    robot::CalculatorRequester requester(requester_for("L6Req", "L6Rep"));
    (void)requester.send_request(add_request(1, 1));
    check(add->calls == 1, "the listener ran while the replier was open");
    replier.close();
    (void)requester.send_request(add_request(2, 2));
    check(add->calls == 1, "and not after close()");
  }
}


/// §7.5.1.1.1, §7.5.1.1.6 and the wire the Rust column writes: the header,
/// padding to four octets, the union. Classic CDR by default.
void check_wire_layout() {
  Bus bus;
  auto participant = std::make_shared<LoopbackParticipant>(bus);
  dds::rpc::core::RequesterParams params;
  params.domain_participant(participant)
      .request_topic_name("WireRequest")
      .reply_topic_name("WireReply");
  robot::CalculatorRequester requester(params);
  (void)requester.send_request(add_request(2, 40));
  const Octets& frame = bus.topic("WireRequest").samples.at(0).first;
  // 16 octets of GUID, 8 of sequence number, the empty instance name as a
  // length of one and its NUL: 29 octets, padded to 32; the union is a
  // `long` discriminator and the two `long` members.
  check(frame.size() == 44, "the request is header, padding and a three-long union");
  auto le32 = [&frame](std::size_t at) {
    return static_cast<std::uint32_t>(frame[at]) | (static_cast<std::uint32_t>(frame[at + 1]) << 8) |
           (static_cast<std::uint32_t>(frame[at + 2]) << 16) |
           (static_cast<std::uint32_t>(frame[at + 3]) << 24);
  };
  check(le32(24) == 1 && frame[28] == 0, "the instance name is an empty string");
  check(frame[29] == 0 && frame[30] == 0 && frame[31] == 0, "padded to a four-octet boundary");
  check(le32(32) == static_cast<std::uint32_t>(OP_ADD), "the discriminator is the hash of `add`");
  check(le32(36) == 2 && le32(40) == 40, "the members follow");
}


/// The octets the two headers take, for the Rust side to compare
/// against `crates/rpc/src/wire_codec.rs`. Printed as hex, one header
/// per line, when the program is asked for them.
void print_header_octets() {
  dds::SampleIdentity identity;
  for (std::size_t at = 0; at < 16; ++at) {
    identity.writer_guid[at] = 0x42;
  }
  identity.sequence_number.high = 0x01020304;
  identity.sequence_number.low = 0x05060708;
  // Independently fixed high/low words, not bytes produced by our encoder.
  Octets canonical_identity(16, 0x42);
  const Octets sequence_words{4, 3, 2, 1, 8, 7, 6, 5};
  canonical_identity.insert(canonical_identity.end(), sequence_words.begin(), sequence_words.end());
  dds::SampleIdentity decoded;
  std::size_t consumed = 0;
  assert(dds::rpc::detail::get_identity(canonical_identity, consumed, decoded));
  assert(consumed == canonical_identity.size() && decoded == identity);

  dds::rpc::RequestHeader request;
  request.requestId = identity;
  request.instanceName = "calc-A";

  dds::rpc::ReplyHeader reply;
  reply.relatedRequestId = identity;
  reply.remoteEx = dds::rpc::REMOTE_EX_INVALID_ARGUMENT;

  for (const Octets& octets : {dds::rpc::core::encode_request_header(request),
                               dds::rpc::core::encode_reply_header(reply)}) {
    for (std::uint8_t octet : octets) {
      std::printf("%02x", octet);
    }
    std::printf("\n");
  }
}

robot::Calculator_Reply sum_reply(std::int32_t value) {
  robot::Calculator_Reply reply{};
  reply.data._d = add_request(0, 0).data._d;
  reply.data.add._d = dds::rpc::RETCODE_OK;
  reply.data.add.result.return_ = value;
  return reply;
}

std::int32_t sum_of(const dds::rpc::Sample<robot::Calculator_Reply>& reply) {
  return reply.data().data.add.result.return_;
}

/// §7.11.1.4.22: the dependent types, and §7.11.1.4.15-.21 with the members
/// of the normative header.
void check_sample_carriers() {
  using Req = robot::Calculator_Request;
  using Traits = dds::rpc::dds_type_traits<Req>;

  // §7.11.1.4.22: dds_type_traits<T> names the dependent types.
  static_assert(std::is_same<Traits::Seq, std::vector<Req>>::value, "Seq");
  static_assert(std::is_same<Traits::ConstSeq, const std::vector<Req>>::value, "ConstSeq");
  static_assert(std::is_same<Traits::DataReader, std::shared_ptr<dds::rpc::DataReader>>::value,
                "DataReader");
  static_assert(std::is_same<Traits::DataWriter, std::shared_ptr<dds::rpc::DataWriter>>::value,
                "DataWriter");
  static_assert(std::is_same<Traits::SampleRefType, dds::rpc::SampleRef<Req>>::value,
                "SampleRefType");
  static_assert(std::is_same<Traits::SampleIteratorValueType, dds::rpc::SampleRef<Req>>::value,
                "SampleIteratorValueType");
  static_assert(
      std::is_same<Traits::ConstSampleIteratorValueType, dds::rpc::SampleRef<const Req>>::value,
      "ConstSampleIteratorValueType");
  static_assert(std::is_same<Traits::LoanedSamplesType, dds::rpc::LoanedSamples<Req>>::value,
                "LoanedSamplesType");
  static_assert(std::is_same<Traits::SharedSamplesType, dds::rpc::SharedSamples<Req>>::value,
                "SharedSamplesType");
  static_assert(std::is_same<Traits::iterator, dds::rpc::SampleIterator<Req, false>>::value,
                "iterator");
  static_assert(std::is_same<Traits::const_iterator, dds::rpc::SampleIterator<Req, true>>::value,
                "const_iterator");
  static_assert(std::is_same<dds::rpc::dds_type_traits<dds::SampleInfo>::Seq,
                             dds::SampleInfoSeq>::value,
                "the sequence of infos");
  static_assert(std::is_same<dds::rpc::Sample<Req>::Seq, Traits::Seq>::value, "Sample::Seq");
  static_assert(std::is_same<dds::rpc::Sample<Req>::DataReader, Traits::DataReader>::value,
                "Sample::DataReader");
  static_assert(std::is_same<dds::Sample<Req>, dds::rpc::Sample<Req>>::value,
                "the carriers are in dds too");
  static_assert(!std::is_copy_constructible<dds::rpc::LoanedSamples<Req>>::value,
                "LoanedSamples is not copyable");
  static_assert(std::is_move_constructible<dds::rpc::LoanedSamples<Req>>::value,
                "but movable");
  static_assert(std::is_copy_constructible<dds::rpc::SharedSamples<Req>>::value,
                "SharedSamples is copyable");

  const Req first = add_request(1, 2);
  const Req second = add_request(3, 4);
  dds::SampleInfo info;
  info.identity.sequence_number.low = 5;

  // Sample<T>.
  dds::rpc::Sample<Req> sample(first, info);
  sample.set_data(second);
  check(sample.data().data.add.a == 3, "Sample::set_data(const T&)");
  sample.set_data(&first);
  check(sample.data().data.add.a == 1, "Sample::set_data(const T*)");
  bool null_data = false;
  try {
    sample.set_data(static_cast<const Req*>(nullptr));
  } catch (const dds::rpc::NullReferenceError&) {
    null_data = true;
  }
  check(null_data, "a null data pointer is a null reference");
  dds::SampleInfo other_info;
  other_info.identity.sequence_number.low = 9;
  sample.set_info(other_info);
  check(sample.identity().sequence_number.low == 9, "Sample::set_info(const SampleInfo&)");
  sample.set_info(&info);
  check(sample.identity().sequence_number.low == 5, "Sample::set_info(const SampleInfo*)");
  sample.data().data.add.a = 10;
  sample.info().related_identity.sequence_number.low = 6;
  check(sample.data().data.add.a == 10 && sample.related_identity().sequence_number.low == 6,
        "the accessors are mutable");
  dds::rpc::Sample<Req> emptied;
  emptied.swap(sample);
  check(emptied.data().data.add.a == 10 && sample.data().data.add.a == 0, "Sample::swap");
  Req& as_data = emptied;
  check(as_data.data.add.a == 10, "a Sample converts to its data");

  // SampleRef<T>.
  dds::rpc::SampleRef<Req> ref(emptied);
  check(!ref.is_nil_data() && !ref.is_nil_info() && ref.identity() == emptied.identity() &&
            ref.related_identity() == emptied.related_identity(),
        "a SampleRef refers to the data and the info");
  ref.data().data.add.a = 77;
  check(emptied.data().data.add.a == 77, "and copying it is shallow");
  check(ref->data().data.add.a == 77, "operator-> gives the reference itself");
  dds::rpc::Sample<Req> copy = ref;
  check(copy.data().data.add.a == 77, "a Sample is made from a SampleRef");
  dds::rpc::SampleRef<Req> nil;
  check(nil.is_nil_data() && nil.is_nil_info(), "a default SampleRef is nil");
  nil.set_data(emptied.data());
  check(!nil.is_nil_data() && nil.is_nil_info(), "set_data(T&)");
  nil.set_info(&emptied.info());
  check(!nil.is_nil_info(), "set_info(SampleInfo*)");
  nil.set_data(static_cast<Req*>(nullptr));
  check(nil.is_nil_data(), "set_data(T*)");
  dds::rpc::SampleRef<Req> swapped;
  swapped.swap(ref);
  check(ref.is_nil_data() && !swapped.is_nil_data(), "SampleRef::swap");
  bool nil_data = false;
  try {
    (void)ref.data();
  } catch (const dds::rpc::NullReferenceError&) {
    nil_data = true;
  }
  check(nil_data, "the data of a nil SampleRef is a null reference");

  // LoanedSamples<T> and SampleIterator<T>.
  std::vector<dds::rpc::Sample<Req>> three;
  for (std::uint32_t at = 1; at <= 3; ++at) {
    dds::SampleInfo each;
    each.identity.sequence_number.low = at;
    three.emplace_back(add_request(static_cast<std::int32_t>(at), 0), each);
  }
  dds::rpc::LoanedSamples<Req> loaned(std::move(three));
  check(loaned.length() == 3 && loaned.data_seq().size() == 3 && loaned.info_seq().size() == 3,
        "LoanedSamples::data_seq and info_seq hold the samples");
  check(loaned[1].identity().sequence_number.low == 2 && loaned[2].data().data.add.a == 3,
        "operator[] gives the sample at the position");
  check(std::distance(loaned.begin(), loaned.end()) == 3, "begin and end");
  check((loaned.begin() + 2)->identity().sequence_number.low == 3 &&
            loaned.begin()[1].data().data.add.a == 2 &&
            (loaned.end() - loaned.begin()) == 3,
        "the iterator is random access");
  dds::rpc::SampleIterator<Req, true> constant = loaned.begin();
  const dds::rpc::LoanedSamples<Req>& read_only = loaned;
  check((*constant).data().data.add.a == 1 && !constant.is_end() &&
            read_only.begin() == constant,
        "a const_iterator is made from an iterator, and begin() of a const container is one");
  constant += 3;
  check(constant.is_end() && constant == read_only.end(), "is_end() at the end");
  int sum = 0;
  for (auto each : loaned) {
    sum += each.data().data.add.a;
  }
  check(sum == 6, "a range-for walks the container");
  bool out_of_range = false;
  try {
    (void)loaned[3];
  } catch (const std::out_of_range&) {
    out_of_range = true;
  }
  check(out_of_range, "an index beyond the end is out_of_range");

  // The loan of a reader goes in and out.
  std::vector<Req> data{first, second};
  dds::SampleInfoSeq infos(2);
  infos[1].identity.sequence_number.low = 2;
  auto moved_in = dds::rpc::LoanedSamples<Req>::move_construct_from_loans(
      std::shared_ptr<dds::rpc::DataReader>(), data, infos);
  check(moved_in.length() == 2 && data.empty() && infos.empty(),
        "move_construct_from_loans takes the sequences");
  std::shared_ptr<dds::rpc::DataReader> reader_back;
  std::vector<Req> data_back;
  dds::SampleInfoSeq infos_back;
  moved_in.release(reader_back, data_back, infos_back);
  check(moved_in.length() == 0 && data_back.size() == 2 &&
            infos_back[1].identity.sequence_number.low == 2,
        "release hands them back");
  loaned.return_loan();
  check(loaned.length() == 0 && loaned.empty(), "return_loan empties the container");

  // SharedSamples<T>.
  std::vector<dds::rpc::Sample<Req>> two;
  two.emplace_back(first, info);
  two.emplace_back(second, other_info);
  dds::rpc::LoanedSamples<Req> to_share(std::move(two));
  dds::rpc::SharedSamples<Req> shared = to_share.to_shared();
  dds::rpc::SharedSamples<Req> shared_copy = shared;
  check(to_share.length() == 0 && shared.length() == 2 &&
            &shared_copy.data_seq()[0] == &shared.data_seq()[0],
        "SharedSamples: the copies refer to the same samples, the loan has left the container");
  check(shared[1].identity().sequence_number.low == 9 && shared_copy.info_seq().size() == 2,
        "and gives them by index and as sequences");
  int walked = 0;
  for (auto each : shared) {
    walked += each.data().data.add.a;
  }
  check(walked == 4, "and by iteration");

  // WriteSample<T> and WriteSampleRef<T>.
  dds::rpc::WriteSample<Req> write(first);
  dds::SampleIdentity identity;
  identity.sequence_number.low = 4;
  write.set_identity(identity);
  dds::rpc::WriteSampleRef<Req> write_ref(write);
  check(write_ref.identity() == identity, "a WriteSampleRef over a WriteSample shows its identity");
  write_ref.set_data(second);
  check(write.data().data.add.a == 3, "and its data is the sample's");
  dds::rpc::WriteSample<Req> from_ref(write_ref);
  check(from_ref.identity() == identity && from_ref.data().data.add.a == 3,
        "a WriteSample is made from a WriteSampleRef");
  from_ref.set_data(first);
  from_ref.swap(write);
  check(write.data().data.add.a == 1 && from_ref.data().data.add.a == 3, "WriteSample::swap");
  const Req bare = add_request(8, 9);
  dds::rpc::WriteSampleRef<Req> bare_ref(bare);
  bare_ref.set_identity(identity);
  check(bare_ref.identity() == identity && bare_ref.data().data.add.a == 8,
        "a WriteSampleRef over a bare value keeps the identity itself");
}

/// DDS-RPC 1.0 §7.11.1.3: the exceptions are DDS exceptions.
void check_exception_hierarchy() {
#ifdef ZERODDS_RPC_HAS_DDS_CORE_EXCEPTION
  static_assert(std::is_base_of<dds::core::Exception, dds::rpc::RemoteUnsupportedError>::value,
                "RemoteUnsupportedError");
  static_assert(std::is_base_of<dds::core::Exception, dds::rpc::RemoteInvalidArgumentError>::value,
                "RemoteInvalidArgumentError");
  static_assert(std::is_base_of<dds::core::Exception, dds::rpc::RemoteOutOfResourcesError>::value,
                "RemoteOutOfResourcesError");
  static_assert(std::is_base_of<dds::core::Exception, dds::rpc::RemoteUnknownOperationError>::value,
                "RemoteUnknownOperationError");
  static_assert(std::is_base_of<dds::core::Exception, dds::rpc::RemoteUnknownExceptionError>::value,
                "RemoteUnknownExceptionError");
  static_assert(std::is_base_of<dds::core::Exception, dds::rpc::LocalError>::value, "LocalError");
  static_assert(std::is_base_of<dds::core::Exception, dds::rpc::TimeoutError>::value,
                "TimeoutError");
  static_assert(std::is_base_of<std::logic_error, dds::rpc::RemoteUnsupportedError>::value,
                "and the std base stays");
  // §7.11.1.3: a local error is the class of that name in dds::core, with the
  // std base that class has, and still a LocalError.
  static_assert(std::is_base_of<dds::core::TimeoutError, dds::rpc::TimeoutError>::value &&
                    std::is_base_of<dds::rpc::LocalError, dds::rpc::TimeoutError>::value &&
                    std::is_base_of<std::runtime_error, dds::rpc::TimeoutError>::value,
                "TimeoutError is dds::core::TimeoutError, a LocalError and a std::runtime_error");
  static_assert(std::is_base_of<dds::core::AlreadyClosedError, dds::rpc::AlreadyClosedError>::value &&
                    std::is_base_of<std::logic_error, dds::rpc::AlreadyClosedError>::value,
                "AlreadyClosedError is dds::core::AlreadyClosedError and a std::logic_error");
  static_assert(std::is_base_of<dds::core::UnsupportedError, dds::rpc::UnsupportedError>::value &&
                    std::is_base_of<dds::core::InvalidArgumentError, dds::rpc::InvalidArgumentError>::value &&
                    std::is_base_of<dds::core::OutOfResourcesError, dds::rpc::OutOfResourcesError>::value &&
                    std::is_base_of<dds::core::NullReferenceError, dds::rpc::NullReferenceError>::value &&
                    std::is_base_of<dds::core::IllegalOperationError, dds::rpc::IllegalOperationError>::value &&
                    std::is_base_of<dds::core::PreconditionNotMetError, dds::rpc::PreconditionNotMetError>::value &&
                    std::is_base_of<dds::core::ImmutablePolicyError, dds::rpc::ImmutablePolicyError>::value &&
                    std::is_base_of<dds::core::InconsistentPolicyError, dds::rpc::InconsistentPolicyError>::value,
                "every local error is the dds::core class of its name");
  {
    bool as_core = false;
    bool as_std = false;
    bool as_local = false;
    bool as_base = false;
    try {
      throw dds::rpc::TimeoutError("late");
    } catch (const dds::core::TimeoutError& error) {
      as_core = std::string(error.what()) == "late";
    }
    try {
      throw dds::rpc::TimeoutError("late");
    } catch (const std::runtime_error&) {
      as_std = true;
    }
    try {
      throw dds::rpc::AlreadyClosedError("closed");
    } catch (const dds::rpc::LocalError& error) {
      as_local = std::string(error.what()) == "closed";
    }
    try {
      throw dds::rpc::InvalidArgumentError("bad");
    } catch (const dds::core::Exception& error) {
      as_base = std::string(error.what()) == "bad";
    }
    check(as_core && as_std && as_local && as_base,
          "a local error is caught as its dds::core class, its std base, LocalError and dds::core::Exception");
  }

  bool remote = false;
  try {
    dds::rpc::raise_remote_exception(dds::rpc::REMOTE_EX_INVALID_ARGUMENT, "bad");
  } catch (const dds::core::Exception& error) {
    remote = std::string(error.what()) == "bad";
  }
  check(remote, "a remote exception is caught as dds::core::Exception, with its message");
  bool local = false;
  try {
    throw dds::rpc::TimeoutError("late");
  } catch (const dds::core::Exception& error) {
    local = std::string(error.what()) == "late";
  }
  check(local, "and so is a local one");
  bool std_base = false;
  try {
    throw dds::rpc::RemoteUnsupportedError("nope");
  } catch (const std::exception& error) {
    std_base = std::string(error.what()) == "nope";
  }
  check(std_base, "and as std::exception");
#else
  check(false, "built without the DDS-PSM-Cxx headers: the exception hierarchy is not measured");
#endif
}

/// §7.11.1.4.3, §7.11.1.4.5: the polling and waiting members of the normative
/// header, on raw repliers and requesters over their own topics.
void check_normative_members(const std::shared_ptr<LoopbackParticipant>& participant) {
  using Req = robot::Calculator_Request;
  using Rep = robot::Calculator_Reply;
  const dds::Duration none = dds::Duration::zero();
  const dds::Duration soon = dds::Duration::from_millisecs(300);

  dds::rpc::core::RequesterParams request_params;
  request_params.domain_participant(participant)
      .request_topic_name("NormativeRequest")
      .reply_topic_name("NormativeReply");
  robot::CalculatorRequester requester(request_params);
  dds::rpc::core::ReplierParams reply_params;
  reply_params.domain_participant(participant)
      .request_topic_name("NormativeRequest")
      .reply_topic_name("NormativeReply");
  robot::CalculatorReplier replier(reply_params);

  // The types and the entities.
  static_assert(std::is_same<robot::CalculatorRequester::RequestDataWriter,
                             std::shared_ptr<dds::rpc::DataWriter>>::value,
                "Requester::RequestDataWriter");
  static_assert(std::is_same<robot::CalculatorRequester::LoanedSamplesType,
                             dds::rpc::LoanedSamples<Rep>>::value,
                "Requester::LoanedSamplesType");
  static_assert(std::is_same<robot::CalculatorReplier::LoanedSamplesType,
                             dds::rpc::LoanedSamples<Req>>::value,
                "Replier::LoanedSamplesType");
  static_assert(std::is_same<robot::CalculatorRequester::Params, dds::rpc::core::RequesterParams>::value,
                "Requester::Params");
  static_assert(std::is_same<robot::CalculatorReplier::Params, dds::rpc::core::ReplierParams>::value,
                "Replier::Params");
  check(requester.get_request_datawriter() != nullptr &&
            requester.get_reply_datareader() != nullptr &&
            replier.get_request_datareader() != nullptr &&
            replier.get_reply_datawriter() != nullptr,
        "the four DDS entities are reachable");
  check(requester.get_requester_params().request_topic_name() == "NormativeRequest" &&
            replier.get_replier_params().reply_topic_name() == "NormativeReply",
        "and the params the endpoints were made with");

  // Nothing there: the waits time out, the takes say so.
  check(!replier.wait_for_requests(none) && !replier.wait_for_requests(1, none),
        "no request yet");
  dds::rpc::Sample<Req> request;
  check(!replier.take_request(request) && !replier.read_request(request) &&
            !replier.receive_request(request, none),
        "and none is taken, read or received");
  check(replier.take_requests(5).length() == 0 && replier.read_requests(5).length() == 0 &&
            replier.receive_requests(none).length() == 0,
        "not as sequences either");

  // Requests, read and taken.
  requester.send_request_oneway(add_request(1, 1));
  const dds::SampleIdentity first = requester.send_request(add_request(1, 2));
  const dds::SampleIdentity second = requester.send_request(add_request(3, 4));
  check(replier.wait_for_requests(3, soon), "three requests are there");
  check(replier.read_request(request) && request.data().data.add.b == 1,
        "read_request gives the oldest and leaves it");
  check(replier.read_requests(2).length() == 2 && replier.read_requests(-1).length() == 3,
        "read_requests gives up to the count, or all");
  check(replier.take_request(request) && request.data().data.add.b == 1,
        "take_request takes the oldest (the oneway one)");
  dds::rpc::Sample<Req> by_reference_target;
  dds::rpc::SampleRef<Req> by_reference(by_reference_target);
  check(replier.take_request(by_reference) && by_reference_target.identity() == first &&
            by_reference_target.data().data.add.b == 2,
        "take_request(SampleRef) fills the sample the reference refers to");
  check(replier.receive_request(request, soon) && request.identity() == second,
        "receive_request waits for the next");
  check(replier.take_requests(5).length() == 0, "and the replier is empty");

  // Replies, by related request.
  dds::rpc::WriteSample<Rep> first_reply(sum_reply(3));
  replier.send_reply(first_reply, first);
  check(!first_reply.identity().is_unknown(), "send_reply(WriteSample) writes the identity back");
  dds::rpc::WriteSample<Rep> second_reply_sample(sum_reply(7));
  dds::rpc::WriteSampleRef<Rep> second_reply(second_reply_sample);
  replier.send_reply(second_reply, second);
  check(!second_reply_sample.identity().is_unknown(),
        "and so does send_reply(WriteSampleRef) into the sample it refers to");

  check(requester.wait_for_replies(2, soon) && requester.wait_for_replies(soon) &&
            requester.wait_for_replies(1, soon, second),
        "two replies are there");
  check(!requester.wait_for_replies(3, none), "not three");
  dds::rpc::Sample<Rep> reply;
  check(requester.read_reply(reply, second) && sum_of(reply) == 7 &&
            reply.related_identity() == second,
        "read_reply finds the reply to a request");
  check(requester.read_replies(10).length() == 2 && requester.read_replies(second).length() == 1,
        "read leaves the replies where they are");
  check(requester.read_reply(reply) && sum_of(reply) == 3, "read_reply gives the oldest");
  auto taken = requester.take_replies(first);
  check(taken.length() == 1 && sum_of(taken[0].sample()) == 3 &&
            taken[0].related_identity() == first,
        "take_replies by related request");
  dds::rpc::Sample<Rep> received;
  check(requester.receive_reply(received, second, soon) && sum_of(received) == 7,
        "receive_reply by related request");
  check(!requester.take_reply(received) && !requester.read_reply(received) &&
            !requester.receive_reply(received, none),
        "all taken");
  check(requester.take_replies(5).length() == 0 && requester.receive_replies(none).length() == 0 &&
            requester.read_replies(5).length() == 0,
        "as sequences too");

  // receive_replies(min, max, wait) and the reference forms.
  const dds::SampleIdentity third = requester.send_request(add_request(5, 6));
  const dds::SampleIdentity fourth = requester.send_request(add_request(7, 8));
  auto requests = replier.receive_requests(2, 2, soon);
  check(requests.length() == 2, "receive_requests(min, max, wait)");
  replier.send_reply(sum_reply(11), third);
  replier.send_reply(sum_reply(15), fourth);
  dds::rpc::Sample<Rep> target;
  dds::rpc::SampleRef<Rep> target_ref(target);
  check(requester.take_reply(target_ref, fourth) && sum_of(target) == 15,
        "take_reply(SampleRef, related)");
  check(requester.receive_reply(target_ref, soon) && sum_of(target) == 11,
        "receive_reply(SampleRef, wait)");
  const dds::SampleIdentity fifth = requester.send_request(add_request(1, 1));
  (void)replier.take_requests(1);
  replier.send_reply(sum_reply(2), fifth);
  auto several = requester.receive_replies(1, 1, soon);
  check(several.length() == 1 && sum_of(several[0].sample()) == 2, "receive_replies(min, max, wait)");

  // Samples without data are not handed out unless asked for.
  check(!requester.receive_nondata_samples(true) && requester.receive_nondata_samples(false),
        "receive_nondata_samples returns the setting it replaces (requester)");
  check(!replier.receive_nondata_samples(true) && replier.receive_nondata_samples(false),
        "and the replier's");

  // The proxy.
  check(requester.get_bound_instance_name().empty(), "unbound");
  requester.bind("somewhere");
  check(requester.get_bound_instance_name() == "somewhere" && requester.is_bound(),
        "get_bound_instance_name");
  requester.unbind();
  check(requester.is_service_discovered(), "the replier is discovered");
  requester.wait_for_service();
  requester.wait_for_service(soon);
  requester.wait_for_services(1);
  requester.wait_for_services(soon, 1);
  requester.wait_for_services(soon, std::vector<std::string>{""});
  requester.wait_for_service_async().get();
  requester.wait_for_services_async(1).get();
  requester.wait_for_services_async(std::vector<std::string>{""}).get();
  check(requester.get_discoverd_service_instances() == std::vector<std::string>{""},
        "without the discovery extension the matched service is the default instance");

  // A wait ends with AlreadyClosedError when the proxy is closed under it,
  // and a wait for what is not there with TimeoutError.
  dds::rpc::core::RequesterParams lonely_params;
  lonely_params.domain_participant(participant)
      .request_topic_name("LonelyRequest")
      .reply_topic_name("LonelyReply");
  robot::CalculatorRequester lonely(lonely_params);
  bool timed_out = false;
  try {
    lonely.wait_for_service(dds::Duration::from_millisecs(30));
  } catch (const dds::rpc::TimeoutError&) {
    timed_out = true;
  }
  check(timed_out, "wait_for_service(max_wait) on a service that is not there times out");
  check(!lonely.wait_for_service(std::chrono::milliseconds(20)),
        "and the chrono overload says so with false");
  auto waiting = lonely.wait_for_service_async();
  lonely.close();
  bool closed_error = false;
  try {
    waiting.get();
  } catch (const dds::rpc::AlreadyClosedError&) {
    closed_error = true;
  }
  check(closed_error, "closing the proxy ends an asynchronous wait with AlreadyClosedError");
}

// -- the signatures of the normative header ------------------------------
//
// `omg-dds-rpc-cxx/request_reply.h` and `function_call.h` of the
// specification, member by member: each expression is the call as the header
// declares it, with the argument types it declares, and its type is the
// return type it declares. Compile-time only.

#define ZD_SIGNATURE(expression, type)                                                    \
  static_assert(std::is_same<decltype(expression), type>::value, "signature: " #expression)

namespace signatures {

using Req = robot::Calculator_Request;
using Rep = robot::Calculator_Reply;
using Requester = dds::rpc::Requester<Req, Rep>;
using Replier = dds::rpc::Replier<Req, Rep>;
using std::declval;
template <typename T>
using Loaned = dds::rpc::LoanedSamples<T>;

// dds::rpc::Requester
ZD_SIGNATURE(declval<Requester&>().send_request(declval<dds::rpc::WriteSample<Req>&>()), void);
ZD_SIGNATURE(declval<Requester&>().send_request(declval<dds::rpc::WriteSampleRef<Req>&>()), void);
ZD_SIGNATURE(declval<Requester&>().send_request_async(declval<const Req&>()),
             dds::rpc::future<dds::rpc::Sample<Rep>>);
ZD_SIGNATURE(declval<Requester&>().send_request_oneway(declval<Req&>()), void);
ZD_SIGNATURE(declval<Requester&>().send_request_oneway(declval<const Req&>()), void);
ZD_SIGNATURE(declval<Requester&>().receive_reply(declval<dds::rpc::Sample<Rep>&>(),
                                                 declval<const dds::Duration&>()),
             bool);
ZD_SIGNATURE(declval<Requester&>().receive_reply(declval<dds::rpc::SampleRef<Rep>>(),
                                                 declval<const dds::Duration&>()),
             bool);
ZD_SIGNATURE(declval<Requester&>().receive_reply(declval<dds::rpc::Sample<Rep>&>(),
                                                 declval<const dds::SampleIdentity&>(),
                                                 declval<const dds::Duration&>()),
             bool);
ZD_SIGNATURE(declval<Requester&>().receive_reply(declval<dds::rpc::SampleRef<Rep>>(),
                                                 declval<const dds::SampleIdentity&>(),
                                                 declval<const dds::Duration&>()),
             bool);
ZD_SIGNATURE(declval<Requester&>().receive_replies(declval<const dds::Duration&>()), Loaned<Rep>);
ZD_SIGNATURE(declval<Requester&>().receive_replies(1, 2, declval<const dds::Duration&>()),
             Loaned<Rep>);
ZD_SIGNATURE(declval<Requester&>().wait_for_replies(1, declval<const dds::Duration&>()), bool);
ZD_SIGNATURE(declval<Requester&>().wait_for_replies(declval<const dds::Duration&>()), bool);
ZD_SIGNATURE(declval<Requester&>().wait_for_replies(1, declval<const dds::Duration&>(),
                                                    declval<const dds::SampleIdentity&>()),
             bool);
ZD_SIGNATURE(declval<Requester&>().take_reply(declval<dds::rpc::Sample<Rep>&>()), bool);
ZD_SIGNATURE(declval<Requester&>().take_reply(declval<dds::rpc::SampleRef<Rep>>()), bool);
ZD_SIGNATURE(declval<Requester&>().take_reply(declval<dds::rpc::Sample<Rep>&>(),
                                              declval<const dds::SampleIdentity&>()),
             bool);
ZD_SIGNATURE(declval<Requester&>().take_reply(declval<dds::rpc::SampleRef<Rep>>(),
                                              declval<const dds::SampleIdentity&>()),
             bool);
ZD_SIGNATURE(declval<Requester&>().take_replies(1), Loaned<Rep>);
ZD_SIGNATURE(declval<Requester&>().take_replies(1, declval<const dds::SampleIdentity&>()),
             Loaned<Rep>);
ZD_SIGNATURE(declval<Requester&>().take_replies(declval<const dds::SampleIdentity&>()),
             Loaned<Rep>);
ZD_SIGNATURE(declval<Requester&>().read_reply(declval<dds::rpc::Sample<Rep>&>()), bool);
ZD_SIGNATURE(declval<Requester&>().read_reply(declval<dds::rpc::SampleRef<Rep>>()), bool);
ZD_SIGNATURE(declval<Requester&>().read_reply(declval<dds::rpc::Sample<Rep>&>(),
                                              declval<const dds::SampleIdentity&>()),
             bool);
ZD_SIGNATURE(declval<Requester&>().read_reply(declval<dds::rpc::SampleRef<Rep>>(),
                                              declval<const dds::SampleIdentity&>()),
             bool);
ZD_SIGNATURE(declval<Requester&>().read_replies(1), Loaned<Rep>);
ZD_SIGNATURE(declval<Requester&>().read_replies(1, declval<const dds::SampleIdentity&>()),
             Loaned<Rep>);
ZD_SIGNATURE(declval<Requester&>().read_replies(declval<const dds::SampleIdentity&>()),
             Loaned<Rep>);
ZD_SIGNATURE(declval<Requester&>().receive_nondata_samples(true), bool);
ZD_SIGNATURE(declval<const Requester&>().get_requester_params(), dds::rpc::RequesterParams);
ZD_SIGNATURE(declval<const Requester&>().get_request_datawriter(),
             dds::rpc::dds_type_traits<Req>::DataWriter);
ZD_SIGNATURE(declval<const Requester&>().get_reply_datareader(),
             dds::rpc::dds_type_traits<Rep>::DataReader);

// dds::rpc::Replier
ZD_SIGNATURE(declval<Replier&>().send_reply(declval<dds::rpc::WriteSample<Rep>&>(),
                                            declval<const dds::SampleIdentity&>()),
             void);
ZD_SIGNATURE(declval<Replier&>().send_reply(declval<dds::rpc::WriteSampleRef<Rep>&>(),
                                            declval<const dds::SampleIdentity&>()),
             void);
ZD_SIGNATURE(declval<Replier&>().send_reply(declval<const Rep&>(),
                                            declval<const dds::SampleIdentity&>()),
             void);
ZD_SIGNATURE(declval<Replier&>().receive_request(declval<dds::rpc::Sample<Req>&>(),
                                                 declval<const dds::Duration&>()),
             bool);
ZD_SIGNATURE(declval<Replier&>().receive_request(declval<dds::rpc::SampleRef<Req>>(),
                                                 declval<const dds::Duration&>()),
             bool);
ZD_SIGNATURE(declval<Replier&>().receive_requests(declval<const dds::Duration&>()), Loaned<Req>);
ZD_SIGNATURE(declval<Replier&>().receive_requests(1, 2, declval<const dds::Duration&>()),
             Loaned<Req>);
ZD_SIGNATURE(declval<Replier&>().wait_for_requests(declval<const dds::Duration&>()), bool);
ZD_SIGNATURE(declval<Replier&>().wait_for_requests(1, declval<const dds::Duration&>()), bool);
ZD_SIGNATURE(declval<Replier&>().take_request(declval<dds::rpc::Sample<Req>&>()), bool);
ZD_SIGNATURE(declval<Replier&>().take_request(declval<dds::rpc::SampleRef<Req>>()), bool);
ZD_SIGNATURE(declval<Replier&>().take_requests(1), Loaned<Req>);
ZD_SIGNATURE(declval<Replier&>().read_request(declval<dds::rpc::Sample<Req>&>()), bool);
ZD_SIGNATURE(declval<Replier&>().read_request(declval<dds::rpc::SampleRef<Req>>()), bool);
ZD_SIGNATURE(declval<Replier&>().read_requests(1), Loaned<Req>);
ZD_SIGNATURE(declval<Replier&>().receive_nondata_samples(true), bool);
ZD_SIGNATURE(declval<const Replier&>().get_replier_params(), const dds::rpc::ReplierParams&);
ZD_SIGNATURE(declval<const Replier&>().get_request_datareader(),
             dds::rpc::dds_type_traits<Req>::DataReader);
ZD_SIGNATURE(declval<const Replier&>().get_reply_datawriter(),
             dds::rpc::dds_type_traits<Rep>::DataWriter);

// dds::rpc::ServiceProxy
ZD_SIGNATURE(declval<Requester&>().bind(std::string()), void);
ZD_SIGNATURE(declval<Requester&>().unbind(), void);
ZD_SIGNATURE(declval<Requester&>().is_bound(), bool);
ZD_SIGNATURE(declval<const Requester&>().get_bound_instance_name(), std::string);
ZD_SIGNATURE(declval<const Requester&>().get_discoverd_service_instances(),
             std::vector<std::string>);
ZD_SIGNATURE(declval<Requester&>().wait_for_service(), void);
ZD_SIGNATURE(declval<Requester&>().wait_for_service(declval<const dds::Duration&>()), void);
ZD_SIGNATURE(declval<Requester&>().wait_for_service(std::string()), void);
ZD_SIGNATURE(declval<Requester&>().wait_for_service(declval<const dds::Duration&>(), std::string()),
             void);
ZD_SIGNATURE(declval<Requester&>().wait_for_services(1), void);
ZD_SIGNATURE(declval<Requester&>().wait_for_services(declval<const dds::Duration&>(), 1), void);
ZD_SIGNATURE(declval<Requester&>().wait_for_services(declval<const std::vector<std::string>&>()),
             void);
ZD_SIGNATURE(declval<Requester&>().wait_for_services(declval<const dds::Duration&>(),
                                                     declval<const std::vector<std::string>&>()),
             void);
ZD_SIGNATURE(declval<Requester&>().wait_for_service_async(), dds::rpc::future<void>);
ZD_SIGNATURE(declval<Requester&>().wait_for_service_async(std::string()), dds::rpc::future<void>);
ZD_SIGNATURE(declval<Requester&>().wait_for_services_async(1), dds::rpc::future<void>);
ZD_SIGNATURE(declval<Requester&>().wait_for_services_async(declval<const std::vector<std::string>&>()),
             dds::rpc::future<void>);

// dds::rpc::RPCEntity and the function-call classes
ZD_SIGNATURE(declval<dds::rpc::ServiceEndpoint&>().status(), dds::rpc::ServiceStatus);
ZD_SIGNATURE(declval<const dds::rpc::ServiceEndpoint&>().get_service_params(),
             dds::rpc::ServiceParams);
ZD_SIGNATURE(declval<const dds::rpc::ServiceEndpoint&>().get_request_datareader<Req>(),
             dds::rpc::dds_type_traits<Req>::DataReader);
ZD_SIGNATURE(declval<const dds::rpc::ServiceEndpoint&>().get_reply_datawriter<Rep>(),
             dds::rpc::dds_type_traits<Rep>::DataWriter);
ZD_SIGNATURE(declval<const dds::rpc::ClientEndpoint&>().get_request_datawriter<Req>(),
             dds::rpc::dds_type_traits<Req>::DataWriter);
ZD_SIGNATURE(declval<const dds::rpc::ClientEndpoint&>().get_reply_datareader<Rep>(),
             dds::rpc::dds_type_traits<Rep>::DataReader);
ZD_SIGNATURE(declval<dds::rpc::ClientEndpoint&>().get_client_params(), dds::rpc::ClientParams);
ZD_SIGNATURE(declval<dds::rpc::Server&>().run(), void);
ZD_SIGNATURE(declval<dds::rpc::Server&>().run(declval<const dds::Duration&>()), void);
ZD_SIGNATURE(declval<dds::rpc::ServerParams&>().default_service_params(
                 declval<const dds::rpc::ServiceParams&>()),
             dds::rpc::ServerParams&);
ZD_SIGNATURE(declval<const dds::rpc::ServerParams&>().default_service_params(),
             dds::rpc::ServiceParams);

// The params
ZD_SIGNATURE(declval<dds::rpc::RequesterParams&>().datawriter_qos(dds::rpc::EndpointQos()),
             dds::rpc::RequesterParams&);
ZD_SIGNATURE(declval<dds::rpc::RequesterParams&>().datareader_qos(dds::rpc::EndpointQos()),
             dds::rpc::RequesterParams&);
ZD_SIGNATURE(declval<dds::rpc::RequesterParams&>().simple_requester_listener(
                 declval<dds::rpc::SimpleRequesterListener<Rep>*>()),
             dds::rpc::RequesterParams&);
ZD_SIGNATURE(declval<dds::rpc::RequesterParams&>().requester_listener(
                 declval<dds::rpc::RequesterListener<Req, Rep>*>()),
             dds::rpc::RequesterParams&);
ZD_SIGNATURE(declval<const dds::rpc::RequesterParams&>().simple_requester_listener(),
             dds::rpc::ListenerBase*);
ZD_SIGNATURE(declval<const dds::rpc::RequesterParams&>().requester_listener(),
             dds::rpc::ListenerBase*);
ZD_SIGNATURE(declval<dds::rpc::ReplierParams&>().simple_replier_listener(
                 declval<dds::rpc::SimpleReplierListener<Req, Rep>*>()),
             dds::rpc::ReplierParams&);
ZD_SIGNATURE(declval<dds::rpc::ReplierParams&>().replier_listener(
                 declval<dds::rpc::ReplierListener<Req, Rep>*>()),
             dds::rpc::ReplierParams&);
ZD_SIGNATURE(declval<const dds::rpc::ReplierParams&>().simple_replier_listener(),
             dds::rpc::ListenerBase*);
ZD_SIGNATURE(declval<const dds::rpc::ReplierParams&>().replier_listener(),
             dds::rpc::ListenerBase*);
ZD_SIGNATURE(declval<dds::rpc::ServiceParams&>().datawriter_qos(dds::rpc::EndpointQos()),
             dds::rpc::ServiceParams&);
ZD_SIGNATURE(declval<dds::rpc::ClientParams&>().datareader_qos(dds::rpc::EndpointQos()),
             dds::rpc::ClientParams&);

}  // namespace signatures

#undef ZD_SIGNATURE


}  // namespace

int main(int argc, char** argv) {
  if (argc > 1 && std::string(argv[1]) == "--header-octets") {
    print_header_octets();
    return 0;
  }
  Bus bus;
  auto participant = std::make_shared<LoopbackParticipant>(bus);

  // §7.5.1.1.1: `typedef string<255> InstanceName`. A name of 255
  // characters is a value of the type; one of 256 is not, and is neither
  // written nor read.
  {
    dds::rpc::RequestHeader header;
    header.instanceName = std::string(255, 'n');
    const Octets at_bound = dds::rpc::core::encode_request_header(header);
    dds::rpc::RequestHeader back;
    std::size_t consumed = 0;
    check(dds::rpc::core::decode_request_header(at_bound, consumed, back) &&
              back.instanceName == header.instanceName,
          "an InstanceName of 255 characters round-trips");
    header.instanceName = std::string(256, 'n');
    bool refused = false;
    try {
      (void)dds::rpc::core::encode_request_header(header);
    } catch (const dds::rpc::InvalidArgumentError&) {
      refused = true;
    }
    check(refused, "an InstanceName of 256 characters is refused on encode");
    // The octets of such a header, as a peer that does not check could send them.
    Octets wire;
    dds::rpc::detail::put_identity(wire, dds::SampleIdentity{});
    dds::rpc::detail::put_u32(wire, 257);
    wire.insert(wire.end(), 256, static_cast<std::uint8_t>('n'));
    wire.push_back(0);
    consumed = 0;
    check(!dds::rpc::core::decode_request_header(wire, consumed, back),
          "an InstanceName of 256 characters is refused on decode");
  }

  // §7.8.2: RELATED_SAMPLE_IDENTITY_INVALID is {GUID_UNKNOWN, SEQUENCENUMBER_UNKNOWN},
  // and SEQUENCENUMBER_UNKNOWN of RTPS is {high = -1, low = 0}.
  {
    const dds::SampleIdentity invalid = dds::related_sample_identity_invalid();
    bool zero_guid = true;
    for (std::uint8_t octet : invalid.writer_guid) {
      zero_guid = zero_guid && octet == 0;
    }
    check(zero_guid, "the invalid related identity has the unknown GUID");
    check(invalid.sequence_number.high == -1 && invalid.sequence_number.low == 0,
          "the invalid related identity has SEQUENCENUMBER_UNKNOWN {-1, 0}");
    check(invalid != dds::SampleIdentity{} && !invalid.is_unknown(),
          "and it is not the unset identity of a sample not yet written");
  }

  // §7.11.1.4.11 and §7.11.1.4.12: the named-parameter containers.
  dds::rpc::core::RequesterParams requester_params;
  requester_params.domain_participant(participant)
      .request_topic_name(robot::CalculatorSupport::request_topic_name())
      .reply_topic_name(robot::CalculatorSupport::reply_topic_name());

  auto service = std::make_shared<CalculatorService>(participant, "Calculator");

  // §7.11.1.1.1: the typed per-service Requester the generator wrote.
  robot::CalculatorRequester requester(requester_params);

  // §7.5.1.1.6/7: the topic announces the request/reply *type* name
  // `${interfaceName}_Request`/`_Reply`, not the topic name. A binding
  // that published the topic name as the type name would fail this.
  check(participant->type_name_of(robot::CalculatorSupport::request_topic_name()) ==
            "robot::Calculator_Request",
        "the request topic announces the request type name");
  check(participant->type_name_of(robot::CalculatorSupport::reply_topic_name()) ==
            "robot::Calculator_Reply",
        "the reply topic announces the reply type name");

  // §7.11.1.4.2: RPCEntity.
  check(!requester.is_null(), "a constructed requester is not null");
  // §7.11.1.4.4: ServiceProxy.
  check(!requester.is_bound(), "a requester starts unbound");
  requester.bind("Calculator");
  check(requester.is_bound(), "bind() binds");
  check(requester.bound_instance_name() == "Calculator", "the bound name is kept");
  check(requester.is_service_discovered(), "the replier's reader matched the requester's writer");

  // §7.11.1.4.17: WriteSample carries back the identity the middleware
  // gave the request.
  dds::rpc::WriteSample<robot::Calculator_Request> outgoing(add_request(2, 40));
  check(outgoing.identity().is_unknown(), "an unsent WriteSample has no identity");
  requester.send_request(outgoing);
  check(!outgoing.identity().is_unknown(), "send_request() writes the identity back");
  const dds::SampleIdentity first = outgoing.identity();

  // §7.11.1.5.3: the Server dispatches into the Service.
  dds::rpc::Server server;
  dds::rpc::add(server, service);
  check(dds::rpc::size(server) == 1, "the server holds one service");
  // §7.11.1.5.3: a Server is a reference; a copy shares the services and the
  // state of the original, so a request is answered through either.
  dds::rpc::Server copied = server;
  check(dds::rpc::size(copied) == 1, "a copy of a Server holds the same service");
  check(dds::rpc::run_once(copied) == 1, "one request was answered through the copy");
  check(service->answered() == 1, "the replier listener saw it");

  // §7.8.1: the reply is correlated by the request's identity.
  dds::rpc::Sample<robot::Calculator_Reply> reply;
  check(requester.receive_reply(first, reply), "the reply arrived");
  check(reply.data().data.add.result.return_ == 42, "2 + 40");
  check(reply.related_identity() == first, "related_request_id is the request's identity");

  // §7.2.1: the client's reader uses a content-based filter, so a reply
  // that answers another client's request (a foreign request-writer GUID)
  // is not handed out. A requester without the filter would return it.
  {
    auto foreign_writer = participant->publisher()->create_writer(
        robot::CalculatorSupport::reply_topic_name(), "robot::Calculator_Reply",
        dds::rpc::EndpointQos{});
    dds::rpc::ReplyHeader foreign_header;
    foreign_header.relatedRequestId.writer_guid[0] = 0xEE;
    foreign_header.relatedRequestId.sequence_number.low = 1;
    foreign_header.remoteEx = dds::rpc::REMOTE_EX_OK;
    (void)foreign_writer->write(dds::rpc::core::encode_reply_header(foreign_header));
    check(requester.receive_replies().length() == 0,
          "a reply to another client is filtered out");
  }

  // §7.11.1.4.3: a replier that names no instance answers only the
  // default instance, so a request bound to "Calculator" is ignored. The
  // old filter skipped this when either side's name was empty.
  {
    dds::rpc::core::ReplierParams unnamed_params;
    unnamed_params.domain_participant(participant)
        .request_topic_name(robot::CalculatorSupport::request_topic_name())
        .reply_topic_name(robot::CalculatorSupport::reply_topic_name());
    robot::CalculatorReplier unnamed(unnamed_params);
    requester.send_request(add_request(0, 0));
    check(unnamed.receive_requests().length() == 0,
          "an unnamed replier ignores a request bound to an instance");
  }

  // §7.2.1.3: two outstanding requests, answered in order, taken in the
  // other order. A requester that dropped what it was not asked for
  // would lose the first.
  requester.send_request(add_request(1, 1));
  const dds::SampleIdentity second = requester.send_request(add_request(10, 10));
  check(dds::rpc::run_once(server) == 2, "both were answered");
  dds::rpc::Sample<robot::Calculator_Reply> later;
  check(requester.receive_reply(second, later), "the second reply is found first");
  check(later.data().data.add.result.return_ == 20, "10 + 10");
  dds::rpc::Sample<robot::Calculator_Reply> earlier;
  check(requester.receive_reply(second, earlier) == false, "a reply is taken once");

  // §7.11.1.4.19-21: the loaned samples and their iterator.
  requester.send_request(add_request(3, 4));
  check(dds::rpc::run_once(server) == 1, "the fourth request was answered");
  auto loaned = requester.receive_replies();
  check(loaned.length() == 2, "the parked 1 + 1 and the fresh 3 + 4 were loaned");
  std::int32_t seen = 0;
  for (dds::rpc::SampleIterator<robot::Calculator_Reply> at = loaned.begin(); at != loaned.end();
       ++at) {
    seen += at->data().data.add.result.return_;
  }
  check(seen == 9, "(1 + 1) + (3 + 4) through the iterator");
  auto shared = loaned.to_shared();
  auto copy = shared;
  check(copy.length() == 2, "a SharedSamples copy refers to the same samples");

  // The fifth request, answered by the Server.
  requester.send_request(add_request(5, 6));
  check(dds::rpc::run_once(server) == 1, "the fifth request was answered");
  check(requester.receive_replies().length() == 1, "and its reply is held for the poll");

  // §7.11.1.4.13 and §7.11.1.4.3: the future-based reception. The
  // future is pending until the reply arrives, which is what
  // distinguishes it from the correlated read above.
  dds::rpc::future<dds::rpc::Sample<robot::Calculator_Reply>> pending =
      requester.send_request_async(add_request(7, 8));
  check(
      pending.wait_for(std::chrono::seconds(0)) != std::future_status::ready,
      "the future is pending before the service has answered");
  check(dds::rpc::run_once(server) == 1, "the seventh request was answered");
  // Any read of the reader fulfils it; this one is the polling call.
  (void)requester.receive_replies();
  check(
      pending.wait_for(std::chrono::seconds(0)) == std::future_status::ready,
      "the future is ready once the reply has been read");
  check(pending.get().data().data.add.result.return_ == 15, "7 + 8 through a future");

  // §7.11.1.4.14: a `shared_future` is the copyable half of the same
  // thing.
  dds::rpc::shared_future<dds::rpc::Sample<robot::Calculator_Reply>> shared_reply =
      requester.send_request_async(add_request(20, 22)).share();
  check(dds::rpc::run_once(server) == 1, "the eighth request was answered");
  (void)requester.receive_replies();
  auto shared_copy = shared_reply;
  check(shared_copy.get().data().data.add.result.return_ == 42, "20 + 22 through a shared_future");

  // §7.5.2 and §7.11.1.3: a reply carrying a remote exception code
  // raises the class the table names.
  {
    dds::rpc::core::ReplierParams params;
    params.domain_participant(participant)
        .request_topic_name(robot::CalculatorSupport::request_topic_name())
        .reply_topic_name(robot::CalculatorSupport::reply_topic_name())
        // The requester is bound to "Calculator" and §7.11.1.4.3 sends its
        // requests to that instance only: an unnamed replier does not take
        // them.
        .instance_name("Calculator");
    robot::CalculatorReplier raw(params);
    const dds::SampleIdentity id = requester.send_request(add_request(0, 0));
    auto requests = raw.receive_requests();
    check(requests.length() == 1, "the raw replier polled one request");
    robot::Calculator_Reply refusal{};
    raw.send_reply(refusal, id, dds::rpc::REMOTE_EX_UNKNOWN_OPERATION);
    bool raised = false;
    try {
      dds::rpc::Sample<robot::Calculator_Reply> unused;
      (void)requester.receive_reply(id, unused);
    } catch (const dds::rpc::RemoteUnknownOperationError&) {
      raised = true;
    }
    check(raised, "REMOTE_EX_UNKNOWN_OPERATION raises RemoteUnknownOperationError");
  }

  // §7.11.1.5.2: ServiceEndpoint's pause and resume.
  service->pause();
  requester.send_request(add_request(1, 2));
  check(dds::rpc::run_once(server) == 0, "a paused service dispatches nothing");
  service->resume();
  check(dds::rpc::run_once(server) == 1, "a resumed one does");

  check_class_form(participant);
  check_enhanced_profile();
  check_service_matching();
  check_enhanced_discovery();
  check_instance_addressing();
  check_qos(participant);
  check_arrival_listeners(participant);
  check_wire_layout();
  check_sample_carriers();
  check_exception_hierarchy();
  check_normative_members(participant);

  // §7.11.1.4.2: close() and is_null().
  requester.close();
  check(requester.is_null(), "a closed requester is null");
  server.close();

  if (failures != 0) {
    std::fprintf(stderr, "%d checks failed\n", failures);
    return 1;
  }
  std::printf("dds::rpc round trip: every check held\n");
  return 0;
}
