// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/rpc/core.hpp — the implementation classes of the DDS-RPC 1.0 C++ binding.
//
// The classes that the normative headers of §7.11.1 (request_reply.h, function_call.h)
// declare in `dds::rpc` are implemented here, in `dds::rpc::core`, as the objects the
// handles of `dds/rpc/request_reply.hpp` and `dds/rpc/function_call.hpp` point at
// (the `impl_` of the normative classes).

#ifndef ZERODDS_DDS_RPC_CORE_HPP
#define ZERODDS_DDS_RPC_CORE_HPP

#include <dds/rpc/base.hpp>

namespace dds {
namespace rpc {
namespace core {



// -- §7.11.1.4.6-10 The listeners --------------------------------------

template <typename TReq, typename TRep>
class Replier;
template <typename TReq, typename TRep>
class Requester;

/// §7.11.1.4.6: "ListenerBase is a 'marker' abstract base class, which
/// all generic listener abstract classes inherit from. It defines no
/// member functions."
class ListenerBase {
 public:
  virtual ~ListenerBase() = default;
};

/// §7.11.1.4.7: "SimpleReplierListener<TReq, TRep> is used to provide a
/// synchronous request listener for a Replier. […] the callback provides
/// the request object and expects the reply as the return value."
template <typename TReq, typename TRep>
class SimpleReplierListener : public ListenerBase {
 public:
  /// The reply to `request`, a heap object the replier takes over and
  /// deletes once it has sent it. `identity` is the identity of the
  /// request. A null pointer sends no reply now: the listener answers
  /// later with `Replier::send_reply` and the identity it was given.
  virtual TRep* process_request(const Sample<TReq>& request,
                                const dds::SampleIdentity& identity) = 0;
};

/// §7.11.1.4.8: "ReplierListener<TReq, TRep> interface is used to
/// provide an asynchronous request listener for a Replier. […] the
/// callback provides the Replier object and returns void."
template <typename TReq, typename TRep>
class ReplierListener : public ListenerBase {
 public:
  virtual void on_request_available(Replier<TReq, TRep>& replier) = 0;
};

/// §7.11.1.4.9: "SimpleRequesterListener<TRep> abstract class is used to
/// receive notifications of arrival of replies. […] the callback
/// provides access to the reply sample and returns void."
template <typename TRep>
class SimpleRequesterListener : public ListenerBase {
 public:
  /// `identity` is the identity of the request `reply` answers.
  virtual void process_reply(const Sample<TRep>& reply, const dds::SampleIdentity& identity) = 0;
};

/// §7.11.1.4.10: "RequesterListener<TReq, TRep> interface is used to
/// receive notification of reply arrival. […] the callback provides the
/// replier object and returns void."
template <typename TReq, typename TRep>
class RequesterListener : public ListenerBase {
 public:
  virtual void on_reply_available(Requester<TReq, TRep>& requester) = 0;
};

// -- §7.4.1 Synthesizing the topic names -------------------------------

/// Which language binding is asking for the name. §7.4.1: "For the
/// request-reply style language binding, <interface_name> and the
/// following underscore shall not be captured automatically."
enum class BindingStyle {
  /// §7.11.1.1.2 — the interface segment is captured.
  FunctionCall,
  /// §7.11.1.1.1 — it is not.
  RequestReply,
};

/// The request and the reply topic of one service.
struct TopicNames {
  std::string request;
  std::string reply;
};

/// The `<service_name>` §7.4.1 defaults to.
inline const char* default_service_name() { return "Service"; }

}  // namespace core
namespace detail {

/// `<user_def_alpha_num> ::= ^[[:alnum:]_ ]+$` — §7.4.1.
inline void validate_user_def_alpha_num(const std::string& text, const char* what) {
  if (text.empty()) {
    throw InvalidArgumentError(std::string("the ") + what + " is empty");
  }
  for (const char c : text) {
    const bool alnum = (c >= '0' && c <= '9') || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z');
    if (!alnum && c != '_' && c != ' ') {
      throw InvalidArgumentError(std::string("the ") + what + " \"" + text +
                                 "\" has a character outside [A-Za-z0-9_ ]");
    }
  }
}

}  // namespace detail
namespace core {

/// §7.4.1, §7.4.2.3: one pair of topic names, from the parts the
/// grammar names and the three sources §7.4.2.3 orders.
///
/// "When more than one method of specifying topic names is used, the
/// run-time specification shall take precedence over the IDL annotations
/// and the default mechanism in that order." So a run-time topic-name
/// suffix wins over everything; a run-time service name wins over an
/// annotated whole topic name (§7.4.2.2), which wins over the default
/// `<interface_name>_Service_Request`/`_Reply`. An empty string stands
/// for "not specified" in every argument but `qualified_interface`.
inline TopicNames synthesize_topic_names(BindingStyle style, const std::string& qualified_interface,
                                         const std::string& runtime_service_name,
                                         const std::string& annotated_service_name,
                                         const std::string& runtime_request_suffix,
                                         const std::string& runtime_reply_suffix,
                                         const std::string& annotated_request_topic,
                                         const std::string& annotated_reply_topic) {
  const std::string& service = !runtime_service_name.empty()
                                   ? runtime_service_name
                                   : (!annotated_service_name.empty()
                                          ? annotated_service_name
                                          : std::string(default_service_name()));
  detail::validate_user_def_alpha_num(service, "service name");
  const bool runtime_service = !runtime_service_name.empty();

  auto one = [&](const std::string& default_suffix, const std::string& runtime_suffix,
                 const std::string& annotated) -> std::string {
    auto compose = [&](const std::string& suffix) -> std::string {
      detail::validate_user_def_alpha_num(suffix, "topic name");
      if (style == BindingStyle::RequestReply) {
        return suffix;
      }
      detail::validate_user_def_alpha_num(qualified_interface, "interface name");
      return qualified_interface + "_" + suffix;
    };
    if (!runtime_suffix.empty()) {
      return compose(runtime_suffix);
    }
    if (!annotated.empty() && !runtime_service) {
      detail::validate_user_def_alpha_num(annotated, "topic name");
      return annotated;
    }
    return compose(service + default_suffix);
  };
  TopicNames names;
  names.request = one("_Request", runtime_request_suffix, annotated_request_topic);
  names.reply = one("_Reply", runtime_reply_suffix, annotated_reply_topic);
  return names;
}

// -- §7.11.1.4.11-12 The parameter containers --------------------------

/// What `RequesterParams`, `ReplierParams`, `ServiceParams` and
/// `ClientParams` share. §7.4.2.3: "The ServiceParams, ClientParams,
/// RequesterParams, and ReplierParams classes provide functions to
/// specify the service name and the topic name suffix." §7.11.1.4.11:
/// "It is designed to mimic the named-parameters feature available in
/// some programming languages, which improves readability."
template <typename Derived>
class EndpointParams {
 public:
  Derived& domain_participant(std::shared_ptr<DomainParticipant> participant) {
    participant_ = std::move(participant);
    return self();
  }
  /// The publisher the writers are created from; the participant's own
  /// when not set.
  Derived& publisher(std::shared_ptr<Publisher> publisher) {
    publisher_ = std::move(publisher);
    return self();
  }
  /// The subscriber the readers are created from; the participant's own
  /// when not set.
  Derived& subscriber(std::shared_ptr<Subscriber> subscriber) {
    subscriber_ = std::move(subscriber);
    return self();
  }
  /// The run-time `<service_name>` of §7.4.1.
  Derived& service_name(std::string name) {
    service_name_ = std::move(name);
    return self();
  }
  /// The service instance this endpoint is — a Replier or Service — or
  /// is bound to — a Requester or Client. §7.11.1.4.3.
  Derived& instance_name(std::string name) {
    instance_name_ = std::move(name);
    return self();
  }
  /// The run-time request topic name suffix of §7.4.2.3: the whole tail
  /// after `<interface_name>_`.
  Derived& request_topic_name_suffix(std::string suffix) {
    request_suffix_ = std::move(suffix);
    return self();
  }
  /// The run-time reply topic name suffix.
  Derived& reply_topic_name_suffix(std::string suffix) {
    reply_suffix_ = std::move(suffix);
    return self();
  }
  /// A whole request topic name, as `@DDSRequestTopic` gives (§7.4.2.2).
  /// A run-time service name or suffix takes precedence over it.
  Derived& request_topic_name(std::string name) {
    request_topic_ = std::move(name);
    return self();
  }
  /// A whole reply topic name, as `@DDSReplyTopic` gives.
  Derived& reply_topic_name(std::string name) {
    reply_topic_ = std::move(name);
    return self();
  }
  /// The `<service_name>` an `@service(name = "…")` annotation gave
  /// (§7.4.1), under the run-time one.
  Derived& annotated_service_name(std::string name) {
    annotated_service_name_ = std::move(name);
    return self();
  }
  /// The `<interface_name>` of §7.4.1: the module names joined by
  /// underscores, then the interface's name. Needed under the
  /// function-call style, which captures it.
  Derived& interface_name(std::string name) {
    interface_name_ = std::move(name);
    return self();
  }
  Derived& binding_style(BindingStyle style) {
    style_ = style;
    return self();
  }
  /// The QoS of the endpoints. Defaults to §7.10.2's. A QoS given here
  /// takes precedence over a profile (§7.4.2.3's order, run time first).
  Derived& qos(RpcQos qos) {
    qos_ = std::move(qos);
    qos_explicit_ = true;
    return self();
  }
  /// The QoS profile URL of the endpoints on the request topic, as
  /// `@Qos(RequestProfile = "…")` names it (§7.10.1).
  Derived& request_profile(std::string url) {
    request_profile_ = std::move(url);
    return self();
  }
  /// The QoS profile URL of the endpoints on the reply topic, as
  /// `@Qos(ReplyProfile = "…")` names it.
  Derived& reply_profile(std::string url) {
    reply_profile_ = std::move(url);
    return self();
  }
  /// The QoS of the DataWriter of this endpoint: the request writer of a
  /// Requester or a Client, the reply writer of a Replier or a Service.
  /// Run-time, so it takes precedence over `qos()` and the profiles
  /// (§7.4.2.3).
  Derived& datawriter_qos(EndpointQos qos) {
    datawriter_qos_ = std::move(qos);
    datawriter_qos_set_ = true;
    return self();
  }
  /// The QoS of the DataReader of this endpoint: the reply reader of a
  /// Requester or a Client, the request reader of a Replier or a Service.
  Derived& datareader_qos(EndpointQos qos) {
    datareader_qos_ = std::move(qos);
    datareader_qos_set_ = true;
    return self();
  }
  /// The CDR form of the samples this endpoint writes and, where the
  /// binding does not report the form a sample arrived in, reads. The
  /// default is classic CDR, the Basic mapping's (§7.5.1.1).
  Derived& wire_encoding(WireEncoding encoding) {
    wire_encoding_ = encoding;
    return self();
  }
  /// What happens to a request while the service has not matched. Only a
  /// requester reads it.
  Derived& service_matching(ServiceMatching matching) {
    matching_ = matching;
    return self();
  }
  /// What happens to a request while the client has not matched. Only a
  /// replier reads it.
  Derived& client_matching(ClientMatching matching) {
    client_matching_ = matching;
    return self();
  }
  /// Read a peer that announced none of the discovery extension (§7.6.2.1)
  /// as complete by the DDS match alone. The Enhanced algorithm (§7.6.2.2)
  /// needs the announcement to know which endpoints of a peer belong
  /// together, so by default such a peer is never complete; a peer that
  /// is not an Enhanced DDS-RPC implementation announces none.
  Derived& accept_unannounced(bool accept) {
    accept_unannounced_ = accept;
    return self();
  }
  /// The qualified names of the base interfaces, whose topics are the
  /// topic aliases the endpoints announce (§7.4.3).
  Derived& parent_interface_names(std::vector<std::string> parents) {
    parents_ = std::move(parents);
    return self();
  }
  /// How replies are tied to requests: the Basic profile's headers
  /// (§7.8.1, the default) or the Enhanced profile's sample identity
  /// (§7.8.2).
  Derived& correlation(Correlation correlation) {
    correlation_ = correlation;
    return self();
  }

  const std::shared_ptr<DomainParticipant>& domain_participant() const { return participant_; }
  const std::shared_ptr<Publisher>& publisher() const { return publisher_; }
  const std::shared_ptr<Subscriber>& subscriber() const { return subscriber_; }
  const std::string& service_name() const { return service_name_; }
  const std::string& instance_name() const { return instance_name_; }
  const std::string& request_topic_name_suffix() const { return request_suffix_; }
  const std::string& reply_topic_name_suffix() const { return reply_suffix_; }
  const std::string& request_topic_name() const { return request_topic_; }
  const std::string& reply_topic_name() const { return reply_topic_; }
  const std::string& annotated_service_name() const { return annotated_service_name_; }
  const std::string& interface_name() const { return interface_name_; }
  BindingStyle binding_style() const { return style_; }
  const RpcQos& qos() const { return qos_; }
  const std::string& request_profile() const { return request_profile_; }
  const std::string& reply_profile() const { return reply_profile_; }
  const EndpointQos& datawriter_qos() const { return datawriter_qos_; }
  const EndpointQos& datareader_qos() const { return datareader_qos_; }
  bool has_datawriter_qos() const { return datawriter_qos_set_; }
  bool has_datareader_qos() const { return datareader_qos_set_; }

  /// `resolved_qos()` with `datawriter_qos()` and `datareader_qos()` put
  /// where they belong: `writer_is_request` is true for the endpoints
  /// whose writer is on the request topic (a Requester, a Client).
  RpcQos resolved_qos_for(bool writer_is_request) const {
    RpcQos out = resolved_qos();
    if (datawriter_qos_set_) {
      (writer_is_request ? out.request : out.reply) = datawriter_qos_;
    }
    if (datareader_qos_set_) {
      (writer_is_request ? out.reply : out.request) = datareader_qos_;
    }
    return out;
  }

  /// The QoS the endpoints are created with: the explicit one, else what
  /// the profile URLs name (a side without a URL has §7.10.2's defaults).
  RpcQos resolved_qos() const {
    if (qos_explicit_ || (request_profile_.empty() && reply_profile_.empty())) {
      return qos_;
    }
    if (!participant_) {
      throw NullReferenceError("a QoS profile URL needs the domain participant to resolve it");
    }
    RpcQos out;
    if (!request_profile_.empty()) {
      out.request = participant_->load_qos_profile(request_profile_);
    }
    if (!reply_profile_.empty()) {
      out.reply = participant_->load_qos_profile(reply_profile_);
    }
    return out;
  }
  Correlation correlation() const { return correlation_; }
  WireEncoding wire_encoding() const { return wire_encoding_; }
  ServiceMatching service_matching() const { return matching_; }
  ClientMatching client_matching() const { return client_matching_; }
  bool accept_unannounced() const { return accept_unannounced_; }
  const std::vector<std::string>& parent_interface_names() const { return parents_; }

  /// Copy every setting to `other`, params of another kind (the params of
  /// a `Client` become those of the `Requester` it calls through).
  /// `binding_style` is the one setting not copied: it belongs to the
  /// kind.
  template <typename Other>
  void copy_to(Other& other) const {
    other.domain_participant(participant_)
        .publisher(publisher_)
        .subscriber(subscriber_)
        .service_name(service_name_)
        .annotated_service_name(annotated_service_name_)
        .instance_name(instance_name_)
        .request_topic_name_suffix(request_suffix_)
        .reply_topic_name_suffix(reply_suffix_)
        .request_topic_name(request_topic_)
        .reply_topic_name(reply_topic_)
        .interface_name(interface_name_)
        .request_profile(request_profile_)
        .reply_profile(reply_profile_)
        .correlation(correlation_)
        .wire_encoding(wire_encoding_)
        .service_matching(matching_)
        .client_matching(client_matching_)
        .accept_unannounced(accept_unannounced_)
        .parent_interface_names(parents_);
    if (qos_explicit_) {
      other.qos(qos_);
    }
    if (datawriter_qos_set_) {
      other.datawriter_qos(datawriter_qos_);
    }
    if (datareader_qos_set_) {
      other.datareader_qos(datareader_qos_);
    }
  }

  /// §7.4.3: the topic aliases of the request and reply topics — "the
  /// rules in sub clause 7.4.1 for each parent interface". An alias equal
  /// to the topic itself (the request-reply style names no interface) is
  /// not one.
  std::pair<std::vector<std::string>, std::vector<std::string>> topic_aliases() const {
    std::pair<std::vector<std::string>, std::vector<std::string>> out;
    const TopicNames own = topic_names();
    for (const std::string& parent : parents_) {
      const TopicNames alias = synthesize_topic_names(style_, parent, service_name_,
                                                      annotated_service_name_, request_suffix_,
                                                      reply_suffix_, std::string(), std::string());
      if (alias.request != own.request) {
        out.first.push_back(alias.request);
      }
      if (alias.reply != own.reply) {
        out.second.push_back(alias.reply);
      }
    }
    return out;
  }

  /// The topic names §7.4.1 and §7.4.2.3 give these params.
  TopicNames topic_names() const {
    return synthesize_topic_names(style_, interface_name_, service_name_, annotated_service_name_,
                                  request_suffix_, reply_suffix_, request_topic_, reply_topic_);
  }

 protected:
  explicit EndpointParams(BindingStyle style) : style_(style) {}

  /// The publisher and subscriber the entities are created from: the
  /// params' own, else the participant's.
  std::shared_ptr<Publisher> resolve_publisher() const {
    if (publisher_) {
      return publisher_;
    }
    if (!participant_) {
      throw NullReferenceError("the params name no domain participant");
    }
    return participant_->publisher();
  }
  std::shared_ptr<Subscriber> resolve_subscriber() const {
    if (subscriber_) {
      return subscriber_;
    }
    if (!participant_) {
      throw NullReferenceError("the params name no domain participant");
    }
    return participant_->subscriber();
  }

 private:
  Derived& self() { return static_cast<Derived&>(*this); }

  std::shared_ptr<DomainParticipant> participant_;
  std::shared_ptr<Publisher> publisher_;
  std::shared_ptr<Subscriber> subscriber_;
  std::string service_name_;
  std::string annotated_service_name_;
  std::string instance_name_;
  std::string request_suffix_;
  std::string reply_suffix_;
  std::string request_topic_;
  std::string reply_topic_;
  std::string interface_name_;
  BindingStyle style_;
  RpcQos qos_;
  bool qos_explicit_ = false;
  EndpointQos datawriter_qos_;
  EndpointQos datareader_qos_;
  bool datawriter_qos_set_ = false;
  bool datareader_qos_set_ = false;
  WireEncoding wire_encoding_ = WireEncoding::XCDR1;
  std::string request_profile_;
  std::string reply_profile_;
  Correlation correlation_ = Correlation::HEADER;
  ServiceMatching matching_ = ServiceMatching::DEFAULT;
  ClientMatching client_matching_ = ClientMatching::DEFAULT;
  bool accept_unannounced_ = false;
  std::vector<std::string> parents_;
};

/// §7.11.1.4.11: "RequesterParams is a valuetype that serves as a
/// container of configuration parameters of a Requester."
class RequesterParams : public EndpointParams<RequesterParams> {
 public:
  RequesterParams() : EndpointParams(BindingStyle::RequestReply) {}

  RequesterParams& listener(std::shared_ptr<ListenerBase> listener) {
    listener_ = std::move(listener);
    return *this;
  }
  const std::shared_ptr<ListenerBase>& listener() const { return listener_; }

  /// The listener that gets the replies one by one. The params do not own
  /// it: it has to outlive the requester.
  template <class TRep>
  RequesterParams& simple_requester_listener(SimpleRequesterListener<TRep>* listener) {
    listener_ = std::shared_ptr<ListenerBase>(std::shared_ptr<ListenerBase>(), listener);
    simple_ = listener_.get();
    return *this;
  }
  /// The listener that is told a reply is there and reads it itself. Not
  /// owned, like the other.
  template <class TReq, class TRep>
  RequesterParams& requester_listener(RequesterListener<TReq, TRep>* listener) {
    listener_ = std::shared_ptr<ListenerBase>(std::shared_ptr<ListenerBase>(), listener);
    full_ = listener_.get();
    return *this;
  }
  ListenerBase* simple_requester_listener() const { return simple_; }
  ListenerBase* requester_listener() const { return full_; }

  std::shared_ptr<Publisher> resolved_publisher() const { return resolve_publisher(); }
  std::shared_ptr<Subscriber> resolved_subscriber() const { return resolve_subscriber(); }

 private:
  std::shared_ptr<ListenerBase> listener_;
  ListenerBase* simple_ = nullptr;
  ListenerBase* full_ = nullptr;
};

/// §7.11.1.4.12: "ReplierParams is a valuetype that serves as a
/// container of configuration parameters of a Replier."
class ReplierParams : public EndpointParams<ReplierParams> {
 public:
  ReplierParams() : EndpointParams(BindingStyle::RequestReply) {}

  ReplierParams& listener(std::shared_ptr<ListenerBase> listener) {
    listener_ = std::move(listener);
    return *this;
  }
  const std::shared_ptr<ListenerBase>& listener() const { return listener_; }

  /// The listener that answers the requests one by one. The params do not
  /// own it: it has to outlive the replier.
  template <class TReq, class TRep>
  ReplierParams& simple_replier_listener(SimpleReplierListener<TReq, TRep>* listener) {
    listener_ = std::shared_ptr<ListenerBase>(std::shared_ptr<ListenerBase>(), listener);
    simple_ = listener_.get();
    return *this;
  }
  /// The listener that is told a request is there and reads it itself. Not
  /// owned, like the other.
  template <class TReq, class TRep>
  ReplierParams& replier_listener(ReplierListener<TReq, TRep>* listener) {
    listener_ = std::shared_ptr<ListenerBase>(std::shared_ptr<ListenerBase>(), listener);
    full_ = listener_.get();
    return *this;
  }
  ListenerBase* simple_replier_listener() const { return simple_; }
  ListenerBase* replier_listener() const { return full_; }

  std::shared_ptr<Publisher> resolved_publisher() const { return resolve_publisher(); }
  std::shared_ptr<Subscriber> resolved_subscriber() const { return resolve_subscriber(); }

 private:
  std::shared_ptr<ListenerBase> listener_;
  ListenerBase* simple_ = nullptr;
  ListenerBase* full_ = nullptr;
};

/// §7.4.2.3: the container of the configuration of a `Service` of the
/// function-call style. The function-call style captures the interface
/// name in the topic names (§7.4.1).
class ServiceParams : public EndpointParams<ServiceParams> {
 public:
  ServiceParams() : EndpointParams(BindingStyle::FunctionCall) {}

  std::shared_ptr<Publisher> resolved_publisher() const { return resolve_publisher(); }
  std::shared_ptr<Subscriber> resolved_subscriber() const { return resolve_subscriber(); }
};

/// §7.4.2.3: the container of the configuration of a `Client` of the
/// function-call style.
class ClientParams : public EndpointParams<ClientParams> {
 public:
  ClientParams() : EndpointParams(BindingStyle::FunctionCall) {}

  std::shared_ptr<Publisher> resolved_publisher() const { return resolve_publisher(); }
  std::shared_ptr<Subscriber> resolved_subscriber() const { return resolve_subscriber(); }
};

// -- The header codec --------------------------------------------------

/// The XCDR2 encoding of the two headers, as §7.5.1.1.1 declares them
/// and `crates/rpc/src/wire_codec.rs` writes them: little-endian, the
/// GUID as sixteen octets, the sequence number as a signed high half and
/// an unsigned low half, a string as its length including the
/// terminator followed by the octets and the terminator.
}  // namespace core
namespace detail {

inline void put_u32(Octets& out, std::uint32_t value) {
  out.push_back(static_cast<std::uint8_t>(value & 0xFFu));
  out.push_back(static_cast<std::uint8_t>((value >> 8) & 0xFFu));
  out.push_back(static_cast<std::uint8_t>((value >> 16) & 0xFFu));
  out.push_back(static_cast<std::uint8_t>((value >> 24) & 0xFFu));
}

inline bool get_u32(const Octets& in, std::size_t& at, std::uint32_t& out) {
  if (at + 4 > in.size()) {
    return false;
  }
  out = static_cast<std::uint32_t>(in[at]) | (static_cast<std::uint32_t>(in[at + 1]) << 8) |
        (static_cast<std::uint32_t>(in[at + 2]) << 16) |
        (static_cast<std::uint32_t>(in[at + 3]) << 24);
  at += 4;
  return true;
}

// SampleIdentity contains SequenceNumber { long high; unsigned long low; },
// not an IDL unsigned long long. Preserve field order even in little endian.
// Confirmed by the unmodified Cyclone 11.0.1 ddsi_xt_typelookup.idl and its
// descriptor-produced byte oracle. The Rust/C++ test also asserts both words
// independently, so agreement cannot preserve the old private u64 encoding.
inline void put_identity(Octets& out, const dds::SampleIdentity& identity) {
  out.insert(out.end(), identity.writer_guid, identity.writer_guid + 16);
  put_u32(out, static_cast<std::uint32_t>(identity.sequence_number.high));
  put_u32(out, identity.sequence_number.low);
}

inline bool get_identity(const Octets& in, std::size_t& at, dds::SampleIdentity& out) {
  if (at + 16 > in.size()) {
    return false;
  }
  std::memcpy(out.writer_guid, in.data() + at, 16);
  at += 16;
  std::uint32_t high = 0;
  if (!get_u32(in, at, high) || !get_u32(in, at, out.sequence_number.low)) {
    return false;
  }
  out.sequence_number.high = static_cast<std::int32_t>(high);
  return true;
}

/// The bound of `InstanceName`, `typedef string<255> InstanceName`
/// (§7.5.1.1.1): characters, not counting the terminator.
constexpr std::size_t INSTANCE_NAME_BOUND = 255;

/// Write a bounded string. A string longer than `bound` is not a value of
/// the type and cannot be written: it is refused, not cut.
inline void put_string(Octets& out, const std::string& text,
                       std::size_t bound = INSTANCE_NAME_BOUND) {
  if (text.size() > bound) {
    throw InvalidArgumentError("a string of " + std::to_string(text.size()) +
                               " characters exceeds its bound of " + std::to_string(bound));
  }
  put_u32(out, static_cast<std::uint32_t>(text.size() + 1));
  out.insert(out.end(), text.begin(), text.end());
  out.push_back(0);
}

/// Read a bounded string. A sample whose string is longer than `bound`, or
/// is not terminated, is not a sample of the type: returns false.
inline bool get_string(const Octets& in, std::size_t& at, std::string& out,
                       std::size_t bound = INSTANCE_NAME_BOUND) {
  std::uint32_t length = 0;
  if (!get_u32(in, at, length) || length == 0) {
    return false;
  }
  if (length - 1 > bound || at + length > in.size() || in[at + length - 1] != 0) {
    return false;
  }
  out.assign(reinterpret_cast<const char*>(in.data() + at), length - 1);
  at += length;
  return true;
}

}  // namespace detail
namespace core {

/// The octets of a `RequestHeader`. §7.5.1.1.1.
inline Octets encode_request_header(const RequestHeader& header) {
  Octets out;
  detail::put_identity(out, header.requestId);
  detail::put_string(out, header.instanceName);
  return out;
}

/// The octets of a `ReplyHeader`. §7.5.1.1.1.
inline Octets encode_reply_header(const ReplyHeader& header) {
  Octets out;
  detail::put_identity(out, header.relatedRequestId);
  detail::put_u32(out, static_cast<std::uint32_t>(header.remoteEx));
  return out;
}

/// Read a `RequestHeader` and leave `at` on the payload.
inline bool decode_request_header(const Octets& in, std::size_t& at, RequestHeader& out) {
  return detail::get_identity(in, at, out.requestId) &&
         detail::get_string(in, at, out.instanceName);
}

/// Read a `ReplyHeader` and leave `at` on the payload.
inline bool decode_reply_header(const Octets& in, std::size_t& at, ReplyHeader& out) {
  std::uint32_t code = 0;
  if (!detail::get_identity(in, at, out.relatedRequestId) || !detail::get_u32(in, at, code)) {
    return false;
  }
  if (code > REMOTE_EX_UNKNOWN_EXCEPTION) {
    return false;
  }
  out.remoteEx = static_cast<RemoteExceptionCode_t>(code);
  return true;
}

// -- §7.11.1.4.2 RPCEntity and §7.11.1.4.4 ServiceProxy ----------------

/// §7.11.1.4.2: "RPCEntity is the base abstract class extended by all
/// the active RPC entities. It supports close() and is_null()
/// operations."
class RPCEntity {
 public:
  virtual ~RPCEntity() = default;
  /// Release the underlying DDS entities.
  virtual void close() = 0;
  /// Whether this reference holds nothing.
  virtual bool is_null() const = 0;
};

/// §7.11.1.4.4: "ServiceProxy class defines type-independent operations
/// for Requester and the Client. For example, binding to a specific
/// instance, waiting for an instance to discover, closing the service,
/// and more. ServiceProxy shall not be instantiated directly."
///
/// A reference type (§7.11.1.4.3, §7.11.1.5.4): a copy shares the
/// binding with the original.
class ServiceProxy : public RPCEntity {
 public:
  /// §7.11.1.4.3: "A requester reference may be bound to a specific
  /// service instance. Requests sent through a bound requester
  /// reference shall be sent to the bound service instance only."
  void bind(const std::string& instance_name) {
    {
      std::lock_guard<std::mutex> guard(binding_->lock);
      binding_->instance_name = instance_name;
    }
    bound(instance_name);
  }
  void unbind() { bind(std::string()); }
  bool is_bound() const { return !bound_instance_name().empty(); }
  std::string bound_instance_name() const {
    std::lock_guard<std::mutex> guard(binding_->lock);
    return binding_->instance_name;
  }
  /// The name of `bound_instance_name()` in the normative header.
  std::string get_bound_instance_name() const { return bound_instance_name(); }

  /// The names of the service instances that have been discovered and
  /// can be addressed: the instance a service announced, the empty name
  /// for the default one. The spelling is that of the normative header.
  virtual std::vector<std::string> get_discoverd_service_instances() const = 0;

  /// Whether the instance `instance_name` names has been discovered: the
  /// check, not a wait.
  virtual bool is_instance_discovered(const std::string& instance_name) const = 0;

  /// Whether the instance this proxy is bound to (the default instance,
  /// if unbound) has been discovered: the check, not a wait.
  bool is_service_discovered() const { return is_instance_discovered(bound_instance_name()); }

  /// Wait until the bound instance (the default one, if unbound) is
  /// discovered.
  void wait_for_service() const { wait_for_service(dds::Duration::infinite()); }

  /// As above, for at most `max_wait`; `TimeoutError` if it is not
  /// discovered by then.
  void wait_for_service(const dds::Duration& max_wait) const {
    wait_for_instance(max_wait, bound_instance_name());
  }

  /// Wait until the instance `instance_name` names is discovered.
  void wait_for_service(std::string instance_name) const {
    wait_for_instance(dds::Duration::infinite(), instance_name);
  }

  void wait_for_service(const dds::Duration& max_wait, std::string instance_name) const {
    wait_for_instance(max_wait, instance_name);
  }

  /// Wait for the bound instance for at most `timeout`. Returns whether it
  /// was discovered; the `Duration` overload throws instead.
  bool wait_for_service(std::chrono::milliseconds timeout) const {
    const detail::Deadline deadline(timeout);
    const std::string instance = bound_instance_name();
    while (!is_instance_discovered(instance)) {
      if (is_null() || deadline.expired()) {
        return false;
      }
      detail::pause_briefly();
    }
    return true;
  }

  /// Wait until at least `count` instances are discovered.
  void wait_for_services(int count) const { wait_for_services(dds::Duration::infinite(), count); }

  void wait_for_services(const dds::Duration& max_wait, int count) const {
    wait_until(max_wait, "service instances", [this, count]() {
      return static_cast<int>(get_discoverd_service_instances().size()) >= count;
    });
  }

  /// Wait until every instance in `instance_names` is discovered.
  void wait_for_services(const std::vector<std::string>& instance_names) const {
    wait_for_services(dds::Duration::infinite(), instance_names);
  }

  void wait_for_services(const dds::Duration& max_wait,
                         const std::vector<std::string>& instance_names) const {
    wait_until(max_wait, "service instances", [this, &instance_names]() {
      const std::vector<std::string> found = get_discoverd_service_instances();
      for (const std::string& wanted : instance_names) {
        if (std::find(found.begin(), found.end(), wanted) == found.end()) {
          return false;
        }
      }
      return true;
    });
  }

  /// The waits above, as futures. The wait runs on a thread of its own
  /// that keeps the proxy's entities alive; closing the proxy ends it and
  /// the future carries `AlreadyClosedError`.
  future<void> wait_for_service_async() const {
    const std::string instance = bound_instance_name();
    return run_wait([instance](const ServiceProxy& proxy) {
      proxy.wait_for_instance(dds::Duration::infinite(), instance);
    });
  }
  future<void> wait_for_service_async(std::string instance_name) const {
    return run_wait([instance_name](const ServiceProxy& proxy) {
      proxy.wait_for_instance(dds::Duration::infinite(), instance_name);
    });
  }
  future<void> wait_for_services_async(int count) const {
    return run_wait(
        [count](const ServiceProxy& proxy) { proxy.wait_for_services(dds::Duration::infinite(), count); });
  }
  future<void> wait_for_services_async(const std::vector<std::string>& instance_names) const {
    return run_wait([instance_names](const ServiceProxy& proxy) {
      proxy.wait_for_services(dds::Duration::infinite(), instance_names);
    });
  }

  /// A second reference to the same entities, for a wait that outlives
  /// the call.
  virtual std::shared_ptr<ServiceProxy> share_reference() const = 0;

 protected:
  /// What the copies of one reference share: the instance it is bound to.
  struct Binding {
    std::mutex lock;
    std::string instance_name;
  };

  /// Called after `bind`: a proxy that forwards to other proxies binds
  /// them to the same instance (`ClientEndpoint`).
  virtual void bound(const std::string& instance_name) { (void)instance_name; }

  ServiceProxy() : binding_(std::make_shared<Binding>()) {}
  /// A reference that shares `binding` with another (a listener's handle
  /// on the requester it was called for).
  explicit ServiceProxy(std::shared_ptr<Binding> binding) : binding_(std::move(binding)) {}

  std::shared_ptr<Binding> share_binding() const { return binding_; }

 private:
  /// Look until `ready` holds, for at most `max_wait`. The proxy being
  /// closed ends the wait with `AlreadyClosedError`, the time running out
  /// with `TimeoutError`.
  template <typename Ready>
  void wait_until(const dds::Duration& max_wait, const char* what, Ready ready) const {
    const detail::Deadline deadline(max_wait);
    while (!ready()) {
      if (is_null()) {
        throw AlreadyClosedError(std::string("the proxy was closed while waiting for ") + what);
      }
      if (deadline.expired()) {
        throw TimeoutError(std::string("the ") + what + " were not discovered in time");
      }
      detail::pause_briefly();
    }
  }

  void wait_for_instance(const dds::Duration& max_wait, const std::string& instance_name) const {
    wait_until(max_wait, "service", [this, &instance_name]() {
      return is_instance_discovered(instance_name);
    });
  }

  template <typename Wait>
  future<void> run_wait(Wait wait) const {
    auto promise = std::make_shared<std::promise<void>>();
    auto result = promise->get_future();
    std::shared_ptr<ServiceProxy> proxy = share_reference();
    // Detached: a future of `std::async` blocks in its destructor, and a
    // wait nobody collects would then block its caller for ever.
    std::thread([promise, proxy, wait]() {
      try {
        wait(*proxy);
        promise->set_value();
      } catch (...) {
        promise->set_exception(std::current_exception());
      }
    }).detach();
    return result;
  }

  std::shared_ptr<Binding> binding_;
};

// -- §7.11.1.4.3 Requester and §7.11.1.4.5 Replier ---------------------

}  // namespace core
namespace detail {

/// The remote exception code a thrown exception stands for, §7.11.1.3's
/// table read backwards: the service side reports a thrown remote
/// exception as its own code and anything else as
/// `REMOTE_EX_UNKNOWN_EXCEPTION` (§7.5.2).
inline RemoteExceptionCode_t remote_exception_code_of(const std::exception_ptr& error) {
  try {
    std::rethrow_exception(error);
  } catch (const RemoteUnsupportedError&) {
    return REMOTE_EX_UNSUPPORTED;
  } catch (const RemoteInvalidArgumentError&) {
    return REMOTE_EX_INVALID_ARGUMENT;
  } catch (const RemoteOutOfResourcesError&) {
    return REMOTE_EX_OUT_OF_RESOURCES;
  } catch (const RemoteUnknownOperationError&) {
    return REMOTE_EX_UNKNOWN_OPERATION;
  } catch (const RemoteUnknownExceptionError&) {
    return REMOTE_EX_UNKNOWN_EXCEPTION;
  } catch (...) {
    return REMOTE_EX_UNKNOWN_EXCEPTION;
  }
}

/// Set the value of `promise` to what `make` returns; a `void` promise is
/// set after `make` ran.
template <typename Result, typename Make>
void fulfil(std::promise<Result>& promise, Make make) {
  promise.set_value(make());
}
template <typename Make>
void fulfil(std::promise<void>& promise, Make make) {
  make();
  promise.set_value();
}

/// The exception a remote exception code raises, as a pointer a `future`
/// can carry (§7.11.1.4.13: a future "transports results (including
/// exceptions) across an asynchronous boundary").
inline std::exception_ptr remote_exception_ptr(RemoteExceptionCode_t code) {
  try {
    raise_remote_exception(code, "the service returned a remote exception");
  } catch (...) {
    return std::current_exception();
  }
  return nullptr;
}

/// An exception a listener raised where nobody could catch it: the thread
/// that read the sample is the binding's, and no caller waits for it. The
/// next call of the entity's own (`check_listener_error`) rethrows it; the
/// first one is kept, later ones are dropped behind it.
class ListenerErrorSlot {
 public:
  void remember_error(std::exception_ptr error) {
    std::lock_guard<std::mutex> guard(error_lock_);
    if (!listener_error_) {
      listener_error_ = std::move(error);
    }
  }

  void check_listener_error() {
    std::exception_ptr error;
    {
      std::lock_guard<std::mutex> guard(error_lock_);
      std::swap(error, listener_error_);
    }
    if (error) {
      std::rethrow_exception(error);
    }
  }

  std::exception_ptr take_listener_error() {
    std::lock_guard<std::mutex> guard(error_lock_);
    std::exception_ptr error;
    std::swap(error, listener_error_);
    return error;
  }

 private:
  std::mutex error_lock_;
  std::exception_ptr listener_error_;
};

}  // namespace detail
namespace core {

/// §7.11.1.4.3: "A requester sends requests and receives replies. […]
/// Requester is a reference type and when copied it makes a shallow
/// copy. An instance of a Requester is configured at the time of
/// construction using RequesterParams […] Requester is inherently
/// asynchronous as sending a request and receiving its corresponding
/// reply (or replies) are separated. Requester allows listener-based,
/// polling-based, and future-based reception of replies."
template <typename TReq, typename TRep>
class Requester : public ServiceProxy {
  class Impl;

 public:
  using RequestType = TReq;
  using ReplyType = TRep;
  typedef typename dds_type_traits<TReq>::DataWriter RequestDataWriter;
  typedef typename dds_type_traits<TRep>::DataReader ReplyDataReader;
  typedef typename dds_type_traits<TRep>::LoanedSamplesType LoanedSamplesType;
  typedef RequesterParams Params;

  /// A reference that holds nothing. `is_null()` is true.
  Requester() = default;

  explicit Requester(const RequesterParams& params) : impl_(std::make_shared<Impl>(params)) {
    if (!params.instance_name().empty()) {
      bind(params.instance_name());
    }
    // Replies are taken when they arrive: a future is fulfilled, and a
    // listener called, without the application reading (§7.11.1.4.9-.10).
    std::weak_ptr<Impl> weak = impl_;
    auto binding = share_binding();
    impl_->listen([weak, binding]() {
      if (auto impl = weak.lock()) {
        Requester handle(binding, impl);
        handle.on_arrival();
      }
    });
  }

  /// Send one request and return the identity the middleware gave it.
  dds::SampleIdentity send_request(const TReq& request) {
    WriteSample<TReq> sample(request);
    send_request(sample);
    return sample.identity();
  }

  /// §7.11.1.4.17: "When a request is sent as a WriteSample, upon
  /// function return, the WriteSample.identity() uniquely identifies
  /// the request sent."
  void send_request(WriteSample<TReq>& sample) {
    live().check_listener_error();
    live().send(sample, bound_instance_name());
  }

  /// §7.11.1.4.18: the same for a `WriteSampleRef`, which refers to the
  /// sample the caller keeps.
  void send_request(WriteSampleRef<TReq> sample) {
    if (sample.is_null()) {
      throw NullReferenceError("the WriteSampleRef refers to no sample");
    }
    WriteSample<TReq> copy(sample.data());
    send_request(copy);
    sample.set_identity(copy.identity());
  }

  /// Send a request of a `oneway` operation: no reply is expected, and
  /// none is waited for.
  void send_request_oneway(const TReq& request) { (void)send_request(request); }

  /// Poll for replies. §7.11.1.4.3: "Requester provides functions to
  /// allow polling reception of replies."
  ///
  /// Every reply not yet handed out, including one a
  /// [`receive_reply`] call took off the reader while looking for
  /// another. Returning only what arrived since the last call would
  /// lose those: the poll and the correlated read share one reader, and
  /// a sample is taken from a reader once.
  ///
  /// A reply that carried a remote exception code raises its exception
  /// here when no `receive_reply` or future claimed it.
  LoanedSamples<TRep> receive_replies() {
    live().check_listener_error();
    return live().receive_replies();
  }

  /// The reply to one request, by the correlation of §7.8.1.
  ///
  /// Returns false when no reply for `request_id` has arrived; every
  /// other reply taken in the meantime is kept, so a caller polling for
  /// two outstanding requests does not lose one by asking for the
  /// other (§7.2.3). A remote exception code in the reply raises
  /// (§7.11.1.3).
  bool receive_reply(const dds::SampleIdentity& request_id, Sample<TRep>& out) {
    live().check_listener_error();
    return live().receive_reply(request_id, out);
  }

  /// Wait for a reply to any request for at most `timeout` and take it
  /// into `reply`. Returns false when none arrived.
  bool receive_reply(Sample<TRep>& reply, const dds::Duration& timeout) {
    return wait_for_replies(1, timeout) && take_reply(reply);
  }
  bool receive_reply(SampleRef<TRep> reply, const dds::Duration& timeout) {
    return wait_for_replies(1, timeout) && take_reply(reply);
  }

  /// As above, for the reply to the request `related_request_id` names.
  bool receive_reply(Sample<TRep>& reply, const dds::SampleIdentity& related_request_id,
                     const dds::Duration& timeout) {
    return wait_for_replies(1, timeout, related_request_id) &&
           take_reply(reply, related_request_id);
  }
  bool receive_reply(SampleRef<TRep> reply, const dds::SampleIdentity& related_request_id,
                     const dds::Duration& timeout) {
    return wait_for_replies(1, timeout, related_request_id) &&
           take_reply(reply, related_request_id);
  }

  /// Wait for a reply to any request for at most `max_wait` and take
  /// every reply that has arrived by then.
  LoanedSamples<TRep> receive_replies(const dds::Duration& max_wait) {
    (void)wait_for_replies(1, max_wait);
    return take_replies(-1);
  }

  /// Wait until `min_count` replies have arrived, for at most `max_wait`,
  /// and take at most `max_count` of them (a negative count takes all).
  LoanedSamples<TRep> receive_replies(int min_count, int max_count, const dds::Duration& max_wait) {
    (void)wait_for_replies(min_count, max_wait);
    return take_replies(max_count);
  }

  /// Wait until `min_count` replies are there to be taken, for at most
  /// `max_wait`. A reply that carries a remote exception code counts: the
  /// call that takes it raises the exception.
  bool wait_for_replies(int min_count, const dds::Duration& max_wait) {
    return wait_matching(min_count, max_wait, nullptr);
  }
  bool wait_for_replies(const dds::Duration& max_wait) { return wait_for_replies(1, max_wait); }
  bool wait_for_replies(int min_count, const dds::Duration& max_wait,
                        const dds::SampleIdentity& related_request_id) {
    return wait_matching(min_count, max_wait, &related_request_id);
  }

  /// §7.2.3 style reads of what has arrived. A `take` removes the
  /// reply from the requester, a `read` leaves it where it is. They do not
  /// wait. Return whether a reply was there. A reply that carries a remote
  /// exception code raises its exception instead (§7.11.1.3).
  bool take_reply(Sample<TRep>& reply) { return collect_one(reply, nullptr, true); }
  bool take_reply(SampleRef<TRep> reply) { return collect_one(reply, nullptr, true); }
  bool take_reply(Sample<TRep>& reply, const dds::SampleIdentity& related_request_id) {
    return collect_one(reply, &related_request_id, true);
  }
  bool take_reply(SampleRef<TRep> reply, const dds::SampleIdentity& related_request_id) {
    return collect_one(reply, &related_request_id, true);
  }
  bool read_reply(Sample<TRep>& reply) { return collect_one(reply, nullptr, false); }
  bool read_reply(SampleRef<TRep> reply) { return collect_one(reply, nullptr, false); }
  bool read_reply(Sample<TRep>& reply, const dds::SampleIdentity& related_request_id) {
    return collect_one(reply, &related_request_id, false);
  }
  bool read_reply(SampleRef<TRep> reply, const dds::SampleIdentity& related_request_id) {
    return collect_one(reply, &related_request_id, false);
  }

  /// At most `max_count` replies (a negative count: all), those to the
  /// request `related_request_id` names if one is given.
  LoanedSamples<TRep> take_replies(int max_count) { return collect(max_count, nullptr, true); }
  LoanedSamples<TRep> take_replies(int max_count, const dds::SampleIdentity& related_request_id) {
    return collect(max_count, &related_request_id, true);
  }
  LoanedSamples<TRep> take_replies(const dds::SampleIdentity& related_request_id) {
    return collect(-1, &related_request_id, true);
  }
  LoanedSamples<TRep> read_replies(int max_count) { return collect(max_count, nullptr, false); }
  LoanedSamples<TRep> read_replies(int max_count, const dds::SampleIdentity& related_request_id) {
    return collect(max_count, &related_request_id, false);
  }
  LoanedSamples<TRep> read_replies(const dds::SampleIdentity& related_request_id) {
    return collect(-1, &related_request_id, false);
  }

  /// Whether samples that carry no data (the state changes of the
  /// reply topic) are handed out too, as samples whose `valid_data` is
  /// false. Returns the setting it replaces. They are not by default.
  bool receive_nondata_samples(bool enable) { return live().receive_nondata_samples(enable); }

  /// The params the requester was made with.
  RequesterParams get_requester_params() const {
    return impl_ ? impl_->params() : RequesterParams();
  }
  /// The request DataWriter and the reply DataReader.
  RequestDataWriter get_request_datawriter() const { return live_const().request_writer(); }
  ReplyDataReader get_reply_datareader() const { return live_const().reply_reader(); }

  /// The future-based reception of §7.11.1.4.3: "no request-reply
  /// correlation is necessary because every future represents a reply
  /// to a unique request."
  ///
  /// The future is made at send time and fulfilled when the reply is
  /// read — on arrival, where the binding reports it, else by whichever
  /// later call reads the reader — `poll`, `receive_reply`,
  /// `receive_replies`. Resolving it inside this call would need the reply
  /// to have arrived before the request was sent, which is a `future` that
  /// is never pending and therefore not a `future`. A remote exception
  /// code in the reply is the exception the future carries.
  future<Sample<TRep>> send_request_async(const TReq& request) {
    live().check_listener_error();
    return live().send_async(request, bound_instance_name());
  }

  /// Send a request and call `complete` with its reply, or with the
  /// exception the reply carries, when it is read. `complete` runs on the
  /// thread that reads the reply, with the requester's state locked: it
  /// must be short and must not wait for another reply.
  void send_request_then(
      const TReq& request,
      std::function<void(std::exception_ptr, const Sample<TRep>&)> complete) {
    live().check_listener_error();
    live().send_then(request, bound_instance_name(), std::move(complete));
  }

  /// Send a request and get a future of what `unpack` makes of the reply
  /// (`R(const TRep&)`; a `void` result is a `future<void>`). What `unpack`
  /// throws, and the exception of a remote exception code, are the
  /// exception the future carries. The function-call style's
  /// `${operation}_async` (§7.11.1.1.2 rule 7) is this.
  template <typename Unpack>
  auto send_request_mapped(const TReq& request, Unpack unpack)
      -> future<std::decay_t<decltype(unpack(std::declval<const TRep&>()))>> {
    using Result = std::decay_t<decltype(unpack(std::declval<const TRep&>()))>;
    auto promise = std::make_shared<std::promise<Result>>();
    auto result = promise->get_future();
    send_request_then(request, [promise, unpack](std::exception_ptr error,
                                                 const Sample<TRep>& reply) {
      if (error) {
        promise->set_exception(error);
        return;
      }
      try {
        detail::fulfil(*promise, [&]() { return unpack(reply.data()); });
      } catch (...) {
        promise->set_exception(std::current_exception());
      }
    });
    return result;
  }

  /// Send a request and wait for its reply: the call of the function-call
  /// style (§7.9.1). Throws the exception a remote exception code names
  /// (§7.11.1.3) and `TimeoutError` when no reply arrives within the
  /// timeout of the QoS (`RpcQos::request_timeout`).
  Sample<TRep> call(const TReq& request) {
    live().check_listener_error();
    return live().call(request, bound_instance_name());
  }

  /// Read the replies that have arrived: fulfil the futures and callbacks
  /// that wait for one and keep the rest for `receive_reply`. A binding
  /// that tells the runtime when samples arrive needs no polling.
  void poll() { live().poll(); }

  /// Hand every reply that has arrived to the listener the params
  /// carried. §7.11.1.4.9 and §7.11.1.4.10.
  void dispatch_replies() {
    live().check_listener_error();
    auto listener = live().listener();
    if (!listener) {
      return;
    }
    if (auto* simple = dynamic_cast<SimpleRequesterListener<TRep>*>(listener.get())) {
      auto replies = receive_replies();
      for (auto reply : replies) {
        simple->process_reply(reply.sample(), reply.related_identity());
      }
      return;
    }
    if (auto* full = dynamic_cast<RequesterListener<TReq, TRep>*>(listener.get())) {
      full->on_reply_available(*this);
    }
  }

  void close() override {
    if (impl_) {
      impl_->close();
    }
  }
  bool is_null() const override { return !impl_ || impl_->closed(); }
  bool is_instance_discovered(const std::string& instance_name) const override {
    return impl_ && impl_->wait_for_service(instance_name);
  }
  std::vector<std::string> get_discoverd_service_instances() const override {
    return impl_ ? impl_->discovered_instances() : std::vector<std::string>();
  }
  std::shared_ptr<ServiceProxy> share_reference() const override {
    return std::make_shared<Requester>(*this);
  }

  /// §7.10.2: "The users may modify the mutable QoS policies at
  /// run-time." Throws `ImmutablePolicyError` for one that cannot change.
  void set_qos(const RpcQos& qos) { live().set_qos(qos); }

  /// The QoS the endpoints have now.
  RpcQos qos() const { return impl_ ? impl_->qos() : RpcQos{}; }

  /// How many requests wait for the service to match (§7.6.2.2.2).
  std::size_t queued_requests() const { return impl_ ? impl_->queued_requests() : 0; }

  /// The exception a listener or a completion callback threw on the
  /// thread that read the reply, if any, cleared by the call. Every
  /// other call on this requester raises it first, so it is not lost.
  std::exception_ptr take_listener_error() { return live().take_listener_error(); }

 private:
  bool wait_matching(int min_count, const dds::Duration& max_wait,
                     const dds::SampleIdentity* related) {
    const detail::Deadline deadline(max_wait);
    for (;;) {
      live().check_listener_error();
      if (live().available(related) >= min_count) {
        return true;
      }
      if (deadline.expired()) {
        return false;
      }
      detail::pause_briefly();
    }
  }

  LoanedSamples<TRep> collect(int max_count, const dds::SampleIdentity* related, bool remove) {
    live().check_listener_error();
    std::vector<Sample<TRep>> found;
    live().collect(max_count, related, remove, found);
    return LoanedSamples<TRep>(std::move(found));
  }

  template <typename Target>
  bool collect_one(Target& target, const dds::SampleIdentity* related, bool remove) {
    live().check_listener_error();
    std::vector<Sample<TRep>> found;
    live().collect(1, related, remove, found);
    if (found.empty()) {
      return false;
    }
    assign(target, found.front());
    return true;
  }
  static void assign(Sample<TRep>& to, const Sample<TRep>& from) { to = from; }
  static void assign(SampleRef<TRep>& to, const Sample<TRep>& from) {
    to.data() = from.data();
    to.info() = from.info();
  }

  /// A handle on an existing requester, for a listener: it shares the
  /// binding and the entities.
  Requester(std::shared_ptr<Binding> binding, std::shared_ptr<Impl> impl)
      : ServiceProxy(std::move(binding)), impl_(std::move(impl)) {}

  /// What every copy of the reference shares.
  class Impl : public detail::ListenerErrorSlot {
   public:
    explicit Impl(const RequesterParams& params)
        : params_(params),
          correlation_(params.correlation()),
          accept_unannounced_(params.accept_unannounced()),
          matching_(resolve_matching(params)),
          qos_(params.resolved_qos_for(true)),
          encoding_(params.wire_encoding()) {
      // The writers declare the CDR form their samples have.
      qos_.request.representation = encoding_;
      qos_.reply.representation = encoding_;
      qos_.request.type_information = detail::type_information_of<TReq>(0);
      qos_.reply.type_information = detail::type_information_of<TRep>(0);
      const TopicNames names = params.topic_names();
      auto publisher = params.resolved_publisher();
      auto subscriber = params.resolved_subscriber();
      // §7.5.1.1.6/7: the request and reply *types* are
      // `${interfaceName}_Request`/`_Reply`, not the topic name.
      writer_ = publisher->create_writer(names.request, dds_type_traits<TReq>::type_name(),
                                         qos_.request);
      reader_ = subscriber->create_reader(names.reply, dds_type_traits<TRep>::type_name(),
                                          qos_.reply);
      if (!writer_ || !reader_) {
        throw OutOfResourcesError("the binding created no request writer or reply reader");
      }
      // The request writer's own GUID. §7.2.1: the client's reader filters
      // "responses for remote invocations of other clients" by a
      // content-based filter, and a reply answers this requester only when
      // its related request id carries this GUID.
      const dds::SampleIdentity identity = writer_->next_identity();
      std::memcpy(request_writer_guid_, identity.writer_guid, sizeof(request_writer_guid_));
      if (correlation_ == Correlation::SAMPLE_IDENTITY && writer_->supports_rpc_discovery() &&
          reader_->supports_rpc_discovery()) {
        // §7.6.2.1: the client-side endpoints name each other, carry the
        // topic aliases, and carry no service instance name ("the
        // client-side DataWriter shall not include service_instance_name").
        const auto aliases = params.topic_aliases();
        RpcDiscovery request_writer;
        request_writer.has_related = true;
        reader_->guid(request_writer.related);
        request_writer.aliases = aliases.first;
        RpcDiscovery reply_reader;
        reply_reader.has_related = true;
        std::memcpy(reply_reader.related, request_writer_guid_, sizeof(request_writer_guid_));
        reply_reader.aliases = aliases.second;
        writer_->announce_rpc_discovery(request_writer);
        reader_->announce_rpc_discovery(reply_reader);
        discovery_ = true;
      }
    }

    ~Impl() { stop_listening(); }

    /// Ask the reader to call `action` when samples arrive.
    void listen(std::function<void()> action) { reader_->set_data_callback(std::move(action)); }

    bool closed() const {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      return closed_;
    }

    void close() {
      std::shared_ptr<DataReader> reader;
      {
        std::lock_guard<std::recursive_mutex> guard(lock_);
        if (closed_) {
          return;
        }
        closed_ = true;
        // A request still waiting for the service to match never goes out,
        // and a future waiting for its reply never gets one. The future is
        // told, so that the caller finds out rather than waiting for ever.
        queued_.clear();
        const std::exception_ptr closed_error = std::make_exception_ptr(
            AlreadyClosedError("the requester was closed before the reply arrived"));
        auto pending = std::move(pending_);
        pending_.clear();
        for (auto& entry : pending) {
          entry.complete(closed_error, nullptr);
        }
        reader = reader_;
      }
      // Outside the lock: stopping the arrival callback waits for one that
      // is running, and that one may be waiting for the lock.
      reader->set_data_callback(nullptr);
      std::lock_guard<std::recursive_mutex> guard(lock_);
      writer_.reset();
      reader_.reset();
    }

    /// Whether the client has discovered both endpoints of the service:
    /// §7.6.2.2.2, "the client-side has discovered both the service
    /// end-points". Sends the requests that waited for it.
    bool wait_for_service(const std::string& instance_name) {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      if (closed_ || !matched(instance_name)) {
        return false;
      }
      flush_queued();
      return true;
    }

    /// §7.10.2: change the mutable policies of the request writer and the
    /// reply reader. If the reader refuses, the writer goes back to what it
    /// had, so a refused call changes nothing.
    void set_qos(const RpcQos& qos) {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      check_open();
      writer_->set_qos(qos.request);
      try {
        reader_->set_qos(qos.reply);
      } catch (...) {
        writer_->set_qos(qos_.request);
        throw;
      }
      qos_ = qos;
    }

    RpcQos qos() const {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      return qos_;
    }

    /// The number of requests accepted but not yet written because the
    /// service has not matched (§7.6.2.2.2).
    std::size_t queued_requests() const {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      return queued_.size();
    }

    std::shared_ptr<ListenerBase> listener() const { return params_.listener(); }

    const RequesterParams& params() const { return params_; }

    std::shared_ptr<DataWriter> request_writer() const {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      if (closed_) {
        throw AlreadyClosedError("the requester is closed");
      }
      return writer_;
    }

    std::shared_ptr<DataReader> reply_reader() const {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      if (closed_) {
        throw AlreadyClosedError("the requester is closed");
      }
      return reader_;
    }

    /// The service instances the client has discovered both endpoints of
    /// (§7.6.2.2.2, Figure 7.4). Without the discovery extension the match
    /// is by count and cannot tell instances apart, so a matched service is
    /// the default instance.
    std::vector<std::string> discovered_instances() {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      std::vector<std::string> out;
      if (closed_ || !writer_ || !reader_) {
        return out;
      }
      if (!discovery_) {
        if (writer_->matched_count() > 0 && reader_->matched_count() > 0) {
          out.push_back(std::string());
        }
        return out;
      }
      for (const RpcPeer& peer : writer_->matched_rpc_readers()) {
        if (peer.announced.name_truncated) {
          continue;
        }
        const std::string name =
            peer.announced.has_instance_name ? peer.announced.instance_name : std::string();
        if (std::find(out.begin(), out.end(), name) == out.end() &&
            writer_->service_matching_complete(*reader_, name, accept_unannounced_)) {
          out.push_back(name);
        }
      }
      return out;
    }

    /// Whether samples without data are handed out; returns the old value.
    bool receive_nondata_samples(bool enable) {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      const bool before = receive_nondata_;
      receive_nondata_ = enable;
      return before;
    }

    /// How many replies are there to be taken: the ones that answer
    /// `related` (any, if null), the ones that carry a remote exception
    /// code included.
    int available(const dds::SampleIdentity* related) {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      drain();
      int count = 0;
      for (const Sample<TRep>& held : held_) {
        if (related == nullptr || held.related_identity() == *related) {
          ++count;
        }
      }
      for (const Failure& failure : failed_) {
        if (related == nullptr || failure.related == *related) {
          ++count;
        }
      }
      return count;
    }

    /// Collect up to `max_count` replies (a negative count: all) that
    /// answer `related` (any, if null) into `out`; `remove` takes them
    /// out of the requester, otherwise they stay. A reply that carries a
    /// remote exception code raises its exception first, and is gone.
    void collect(int max_count, const dds::SampleIdentity* related, bool remove,
                 std::vector<Sample<TRep>>& out) {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      drain();
      for (auto at = failed_.begin(); at != failed_.end(); ++at) {
        if (related == nullptr || at->related == *related) {
          const RemoteExceptionCode_t code = at->code;
          failed_.erase(at);
          raise_remote_exception(code, "the service returned a remote exception");
        }
      }
      const std::size_t limit =
          max_count < 0 ? std::numeric_limits<std::size_t>::max() : static_cast<std::size_t>(max_count);
      for (auto at = held_.begin(); at != held_.end() && out.size() < limit;) {
        if (related == nullptr || at->related_identity() == *related) {
          out.push_back(*at);
          at = remove ? held_.erase(at) : std::next(at);
        } else {
          ++at;
        }
      }
    }

    void send(WriteSample<TReq>& sample, const std::string& instance_name) {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      send_locked(sample, instance_name);
    }

    future<Sample<TRep>> send_async(const TReq& request, const std::string& instance_name) {
      auto promise = std::make_shared<std::promise<Sample<TRep>>>();
      auto result = promise->get_future();
      send_then(request, instance_name,
                [promise](std::exception_ptr error, const Sample<TRep>& sample) {
                  if (error) {
                    promise->set_exception(error);
                  } else {
                    promise->set_value(sample);
                  }
                });
      return result;
    }

    void send_then(const TReq& request, const std::string& instance_name,
                   std::function<void(std::exception_ptr, const Sample<TRep>&)> complete) {
      // Registered under the lock the send holds, so a reply that arrives
      // at once finds its entry.
      std::lock_guard<std::recursive_mutex> guard(lock_);
      WriteSample<TReq> sample(request);
      send_locked(sample, instance_name);
      register_pending(sample.identity(), std::move(complete));
      drain();
    }

    Sample<TRep> call(const TReq& request, const std::string& instance_name) {
      auto promise = std::make_shared<std::promise<Sample<TRep>>>();
      auto result = promise->get_future();
      dds::SampleIdentity identity;
      std::chrono::milliseconds timeout;
      {
        std::lock_guard<std::recursive_mutex> guard(lock_);
        timeout = qos_.request_timeout;
        WriteSample<TReq> sample(request);
        send_locked(sample, instance_name);
        identity = sample.identity();
        register_pending(identity, [promise](std::exception_ptr error, const Sample<TRep>& reply) {
          if (error) {
            promise->set_exception(error);
          } else {
            promise->set_value(reply);
          }
        });
        drain();
      }
      const auto deadline = std::chrono::steady_clock::now() + timeout;
      while (result.wait_for(std::chrono::milliseconds(0)) != std::future_status::ready) {
        // Woken by the arrival callback when the binding has one; the poll
        // is for a binding that has none, and for a service that is matched
        // later.
        if (result.wait_for(std::chrono::milliseconds(1)) == std::future_status::ready) {
          break;
        }
        poll();
        if (timeout.count() > 0 && std::chrono::steady_clock::now() >= deadline) {
          std::lock_guard<std::recursive_mutex> guard(lock_);
          pending_.erase(std::remove_if(pending_.begin(), pending_.end(),
                                        [&identity](const Pending& entry) {
                                          return entry.identity == identity;
                                        }),
                         pending_.end());
          throw TimeoutError("no reply to the request within the request timeout");
        }
      }
      return result.get();
    }

    LoanedSamples<TRep> receive_replies() {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      drain();
      if (!failed_.empty()) {
        const Failure failure = failed_.front();
        failed_.erase(failed_.begin());
        raise_remote_exception(failure.code, "the service returned a remote exception");
      }
      std::vector<Sample<TRep>> out;
      out.swap(held_);
      return LoanedSamples<TRep>(std::move(out));
    }

    bool receive_reply(const dds::SampleIdentity& request_id, Sample<TRep>& out) {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      drain();
      for (auto at = failed_.begin(); at != failed_.end(); ++at) {
        if (at->related == request_id) {
          const RemoteExceptionCode_t code = at->code;
          failed_.erase(at);
          raise_remote_exception(code, "the service returned a remote exception");
        }
      }
      for (auto at = held_.begin(); at != held_.end(); ++at) {
        if (at->related_identity() == request_id) {
          out = std::move(*at);
          held_.erase(at);
          return true;
        }
      }
      return false;
    }

    void poll() {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      if (closed_) {
        return;
      }
      drain();
    }

    /// The arrival callback's part: read what arrived.
    void on_arrival_drain() {
      std::lock_guard<std::recursive_mutex> guard(lock_);
      if (closed_) {
        return;
      }
      drain();
    }


   private:
    /// A request accepted while the service had not matched.
    struct Queued {
      Octets frame;
      dds::SampleIdentity identity;
      /// The service instance the request is bound to.
      std::string instance_name;
    };

    /// One outstanding request, by the identity its reply will carry.
    struct Pending {
      dds::SampleIdentity identity;
      std::function<void(std::exception_ptr, const Sample<TRep>&)> complete_fn;

      void complete(std::exception_ptr error, const Sample<TRep>* sample) const {
        complete_fn(error, sample != nullptr ? *sample : Sample<TRep>{});
      }
    };

    /// A reply that carried a remote exception code and that no future or
    /// callback has claimed.
    struct Failure {
      dds::SampleIdentity related;
      RemoteExceptionCode_t code;
    };

    enum class Decoded { DROP, SAMPLE, FAILURE };

    static ServiceMatching resolve_matching(const RequesterParams& params) {
      if (params.service_matching() != ServiceMatching::DEFAULT) {
        return params.service_matching();
      }
      // §7.6.2.2 belongs to the Enhanced profile; the Basic one relies on
      // plain DDS discovery and says so (§7.6.1).
      return params.correlation() == Correlation::SAMPLE_IDENTITY ? ServiceMatching::QUEUE
                                                                  : ServiceMatching::NONE;
    }

    void stop_listening() {
      if (reader_) {
        reader_->set_data_callback(nullptr);
      }
    }

    /// §7.6.2.2.2, Figure 7.4: whether the client has discovered both
    /// endpoints of the service instance `instance_name` names. Where the
    /// binding announces and reads the discovery extension that is the
    /// algorithm of the clause; otherwise it is the count of matched
    /// endpoints, which cannot tell instances apart (so a bound request is
    /// refused where it is sent).
    bool matched(const std::string& instance_name) const {
      if (!writer_ || !reader_) {
        return false;
      }
      if (discovery_) {
        return writer_->service_matching_complete(*reader_, instance_name, accept_unannounced_);
      }
      return writer_->matched_count() > 0 && reader_->matched_count() > 0;
    }

    void check_open() const {
      if (closed_) {
        throw AlreadyClosedError("the requester is closed");
      }
    }

    /// Hand a reply that is already here to the entry that waits for it,
    /// or keep the entry. The caller holds `lock_`.
    void register_pending(const dds::SampleIdentity& identity,
                          std::function<void(std::exception_ptr, const Sample<TRep>&)> complete) {
      Pending entry{identity, std::move(complete)};
      for (auto at = held_.begin(); at != held_.end(); ++at) {
        if (at->related_identity() == identity) {
          Sample<TRep> sample = std::move(*at);
          held_.erase(at);
          entry.complete(nullptr, &sample);
          return;
        }
      }
      for (auto at = failed_.begin(); at != failed_.end(); ++at) {
        if (at->related == identity) {
          const std::exception_ptr error = detail::remote_exception_ptr(at->code);
          failed_.erase(at);
          entry.complete(error, nullptr);
          return;
        }
      }
      pending_.push_back(std::move(entry));
    }

    /// Write the requests that waited, in the order they were accepted,
    /// once the service has matched.
    void flush_queued() {
      while (!queued_.empty()) {
        const Queued& next = queued_.front();
        if (!matched(next.instance_name)) {
          return;
        }
        const dds::SampleIdentity written = write_frame(next.frame, next.instance_name);
        if (written != next.identity) {
          // The identity the sample was promised (and, in the Basic
          // profile, put in its header) is not the one the writer gave it:
          // the reply could not be correlated.
          queued_.clear();
          throw IllegalOperationError(
              "the request writer stamped a queued request with another identity than the one "
              "it was promised");
        }
        queued_.pop_front();
      }
    }

    /// The caller holds `lock_`.
    void send_locked(WriteSample<TReq>& sample, const std::string& instance_name) {
      check_open();
      // Requests that waited for the service go first.
      flush_queued();
      const bool hold = matching_ != ServiceMatching::NONE && !matched(instance_name);
      if (hold && matching_ == ServiceMatching::REFUSE) {
        throw PreconditionNotMetError(
            "the service has not been matched yet; a request is not sent before the client has "
            "discovered both service endpoints (§7.6.2.2.2)");
      }
      // Identity allocation and the write under one lock, as the Rust
      // requester does: the header has to name the very identity the
      // writer stamps on the sample it is part of (§7.5.1.1.1 `requestId`,
      // §7.8.1), and no other send may slip in between the two calls. A
      // request that waits gets the identity it will have: the writer's
      // next one, moved past the requests ahead of it.
      const dds::SampleIdentity identity =
          detail::advance_identity(writer_->next_identity(), queued_.size());
      Octets frame;
      if (correlation_ == Correlation::SAMPLE_IDENTITY) {
        // §7.8.2: the Enhanced profile has no header. The request is the
        // bare topic type, and its identity is the one the writer gives
        // the sample.
        if (!instance_name.empty() && !discovery_) {
          // §7.6.2.1: the instance a request is bound to is matched
          // through `PID_SERVICE_INSTANCE_NAME` at discovery, not through a
          // header member. Sending the request to every instance instead
          // would break §7.11.1.4.3's "shall be sent to the bound service
          // instance only", unnoticed.
          throw UnsupportedError(
              "an Enhanced requester bound to an instance needs discovery of "
              "PID_SERVICE_INSTANCE_NAME, which this binding does not provide");
        }
        frame = detail::serialize_payload<TReq>(
            sample.data(), 0, encoding_, typename detail::has_serialize_at<TReq>::type());
      } else if (correlation_ == Correlation::HEADER_IN_TYPE) {
        // §7.3.1.4: the type carries its own header. The runtime stamps the
        // identity and the instance into a copy and writes the structure
        // as it is; its header is its first member, so the leading octets
        // are those of §7.5.1.1.1.
        RequestHeader header;
        header.requestId = identity;
        header.instanceName = instance_name;
        TReq stamped = sample.data();
        detail::stamp_request_header<TReq>(stamped, header,
                                           typename detail::has_set_request_header<TReq>::type());
        frame = detail::serialize_payload<TReq>(
            stamped, 0, encoding_, typename detail::has_serialize_at<TReq>::type());
      } else {
        RequestHeader header;
        header.requestId = identity;
        header.instanceName = instance_name;
        frame = encode_request_header(header);
        // The union follows the header on a four-octet boundary and its
        // members align counted from the start of the sample.
        frame.resize(detail::round_up_4(frame.size()), 0);
        const Octets payload = detail::serialize_payload<TReq>(
            sample.data(), frame.size(), encoding_, typename detail::has_serialize_at<TReq>::type());
        frame.insert(frame.end(), payload.begin(), payload.end());
      }
      if (hold) {
        queued_.push_back(Queued{std::move(frame), identity, instance_name});
        sample.set_identity(identity);
        return;
      }
      sample.set_identity(write_frame(frame, instance_name));
    }

    /// The readers of the service instance `instance_name` names that the
    /// client has discovered both endpoints of (§7.6.2.2.2, Figure 7.4):
    /// the readers whose announcement carries that name, the empty name for
    /// those that carry none.
    std::vector<EndpointGuid> readers_of_instance(const std::string& instance_name) const {
      std::vector<EndpointGuid> out;
      const std::vector<RpcPeer> writers = reader_->matched_rpc_writers();
      for (const RpcPeer& peer : writer_->matched_rpc_readers()) {
        if (!peer.announced.name_truncated &&
            service_matching_complete({peer}, writers, instance_name, accept_unannounced_)) {
          EndpointGuid guid;
          std::copy(peer.key, peer.key + 16, guid.begin());
          out.push_back(guid);
        }
      }
      return out;
    }

    /// Write one request frame. An Enhanced request that goes through the
    /// discovery extension reaches the readers of the instance it is bound
    /// to and no other (§7.11.1.4.3); every other request goes to all
    /// matched readers, as the Basic profile's does, where the replier
    /// reads the instance out of the header (§7.5.1.1.1 `instanceName`).
    dds::SampleIdentity write_frame(const Octets& frame, const std::string& instance_name) {
      if (correlation_ == Correlation::SAMPLE_IDENTITY && discovery_) {
        return writer_->write_to(frame, readers_of_instance(instance_name));
      }
      return writer_->write(frame);
    }

    /// Take everything the reader has: complete the entry of any request
    /// that waits for one, park the rest.
    ///
    /// A reply carrying a remote exception code is the exception of the
    /// entry that waits for it (§7.11.1.3: the call site is what has to see
    /// it); one nobody waits for is kept for `receive_reply` and
    /// `receive_replies` to raise, and the samples behind it in the same
    /// take are not lost.
    void drain() {
      check_open();
      flush_queued();
      for (auto& taken : reader_->take()) {
        if (!taken.second.valid_data) {
          // A state change of the topic, not a reply.
          if (receive_nondata_) {
            held_.push_back(Sample<TRep>(TRep{}, taken.second));
          }
          continue;
        }
        Sample<TRep> sample;
        Failure failure{};
        const Decoded decoded = decode_reply(taken.first, taken.second, sample, failure);
        if (decoded == Decoded::DROP) {
          continue;
        }
        const dds::SampleIdentity related =
            decoded == Decoded::SAMPLE ? sample.related_identity() : failure.related;
        const auto waiting = std::find_if(
            pending_.begin(), pending_.end(),
            [&related](const Pending& entry) { return entry.identity == related; });
        if (waiting != pending_.end()) {
          const Pending entry = *waiting;
          pending_.erase(waiting);
          if (decoded == Decoded::SAMPLE) {
            entry.complete(nullptr, &sample);
          } else {
            entry.complete(detail::remote_exception_ptr(failure.code), nullptr);
          }
          continue;
        }
        if (decoded == Decoded::SAMPLE) {
          held_.push_back(std::move(sample));
        } else {
          failed_.push_back(failure);
        }
      }
    }

    Decoded decode_reply(const Octets& octets, const dds::SampleInfo& info, Sample<TRep>& out,
                         Failure& failure) {
      if (correlation_ == Correlation::SAMPLE_IDENTITY) {
        return decode_enhanced_reply(octets, info, out, failure);
      }
      std::size_t at = 0;
      ReplyHeader header;
      if (!decode_reply_header(octets, at, header)) {
        return Decoded::DROP;
      }
      // §7.2.1: the client's reader uses a content-based filter so that
      // "responses for remote invocations of other clients are filtered".
      // A reply answers this requester only when its related request id
      // carries this request writer's GUID.
      if (std::memcmp(header.relatedRequestId.writer_guid, request_writer_guid_,
                      sizeof(request_writer_guid_)) != 0) {
        return Decoded::DROP;
      }
      if (header.remoteEx != REMOTE_EX_OK) {
        failure = Failure{header.relatedRequestId, header.remoteEx};
        return Decoded::FAILURE;
      }
      TRep value{};
      // The union follows a runtime-written header on a four-octet
      // boundary; a type that carries its own header is read whole.
      at = correlation_ == Correlation::HEADER_IN_TYPE ? 0 : detail::round_up_4(at);
      if (!detail::deserialize_payload<TRep>(octets, at, value,
                                              detail::encoding_of(info, encoding_),
                                              typename detail::has_deserialize_in<TRep>::type())) {
        return Decoded::DROP;
      }
      dds::SampleInfo carried = info;
      carried.related_identity = header.relatedRequestId;
      carried.has_related_identity = true;
      out = Sample<TRep>(std::move(value), carried);
      return Decoded::SAMPLE;
    }

    /// §7.8.2.2: the related sample identity of the reply's SampleInfo is
    /// the request's identity. A reply without one, or whose related
    /// identity names another client's request writer, is not this
    /// requester's (§7.2.1's content-based filter).
    Decoded decode_enhanced_reply(const Octets& octets, const dds::SampleInfo& info,
                                  Sample<TRep>& out, Failure& failure) {
      if (!info.has_related_identity ||
          std::memcmp(info.related_identity.writer_guid, request_writer_guid_,
                      sizeof(request_writer_guid_)) != 0) {
        return Decoded::DROP;
      }
      std::size_t at = 0;
      TRep value{};
      if (!detail::deserialize_payload<TRep>(octets, at, value,
                                              detail::encoding_of(info, encoding_),
                                              typename detail::has_deserialize_in<TRep>::type())) {
        return Decoded::DROP;
      }
      // §7.5.1.2.5 rule 5: the code is the `remoteEx` member of the reply.
      const RemoteExceptionCode_t code = detail::remote_exception_of(
          value, typename detail::has_remote_exception<TRep>::type());
      if (code != REMOTE_EX_OK) {
        failure = Failure{info.related_identity, code};
        return Decoded::FAILURE;
      }
      out = Sample<TRep>(std::move(value), info);
      return Decoded::SAMPLE;
    }

    RequesterParams params_;
    Correlation correlation_;
    bool accept_unannounced_;
    /// Whether the binding announces and reads the discovery extension and
    /// this requester uses it (the Enhanced profile).
    bool discovery_ = false;
    ServiceMatching matching_;
    RpcQos qos_;
    WireEncoding encoding_;
    std::shared_ptr<DataWriter> writer_;
    std::shared_ptr<DataReader> reader_;
    std::vector<Sample<TRep>> held_;
    std::vector<Failure> failed_;
    std::vector<Pending> pending_;
    std::deque<Queued> queued_;
    bool receive_nondata_ = false;
    bool closed_ = false;
    /// Guards all of the state, the identity allocation and the write
    /// included: they have to see the same number. Recursive because a
    /// send flushes, a binding may deliver a reply from inside the write
    /// and a read may call a listener that reads again.
    mutable std::recursive_mutex lock_;
    /// The request writer's GUID, for §7.2.1's content-based reply filter.
    std::uint8_t request_writer_guid_[16] = {};
  };

  /// A reply has arrived: take it, and tell the listener. Runs on the
  /// binding's thread; what it throws is kept for the next call.
  void on_arrival() {
    try {
      if (impl_->closed()) {
        return;
      }
      impl_->on_arrival_drain();
      if (impl_->listener()) {
        dispatch_replies();
      }
    } catch (...) {
      impl_->remember_error(std::current_exception());
    }
  }

  Impl& live() {
    if (!impl_) {
      throw NullReferenceError("the requester holds no entities");
    }
    return *impl_;
  }
  const Impl& live_const() const {
    if (!impl_) {
      throw NullReferenceError("the requester holds no entities");
    }
    return *impl_;
  }

  std::shared_ptr<Impl> impl_;
};

/// §7.11.1.4.5: "A replier receives requests and send[s] replies. […]
/// Replier is a reference type and it is inexpensive to copy
/// (comparable to a pointer assignment). […] Replier allows
/// listener-based and polling-based reception of requests."
template <typename TReq, typename TRep>
class Replier : public RPCEntity {
  class Impl;

 public:
  using RequestType = TReq;
  using ReplyType = TRep;
  typedef typename dds_type_traits<TRep>::DataWriter ReplyDataWriter;
  typedef typename dds_type_traits<TReq>::DataReader RequestDataReader;
  typedef typename dds_type_traits<TReq>::LoanedSamplesType LoanedSamplesType;
  typedef ReplierParams Params;

  /// A reference that holds nothing. `is_null()` is true.
  Replier() = default;

  explicit Replier(const ReplierParams& params) : impl_(std::make_shared<Impl>(params)) {
    if (params.listener()) {
      // §7.9.2.2: the callback is "installed during service activation"
      // and "invoked for each received request": when the request arrives,
      // not when the application next looks.
      std::weak_ptr<Impl> weak = impl_;
      impl_->listen([weak]() {
        if (auto impl = weak.lock()) {
          Replier handle(impl);
          handle.on_arrival();
        }
      });
    }
  }

  /// Poll for requests: every request that has arrived.
  LoanedSamples<TReq> receive_requests() {
    live().check_listener_error();
    return live().receive_requests();
  }

  /// Wait for a request for at most `max_wait` and take it into
  /// `request`. Returns false when none arrived.
  bool receive_request(Sample<TReq>& request, const dds::Duration& max_wait) {
    return wait_for_requests(1, max_wait) && take_request(request);
  }
  bool receive_request(SampleRef<TReq> request, const dds::Duration& max_wait) {
    return wait_for_requests(1, max_wait) && take_request(request);
  }

  /// Wait for a request for at most `max_wait` and take every request
  /// that has arrived by then.
  LoanedSamplesType receive_requests(const dds::Duration& max_wait) {
    (void)wait_for_requests(1, max_wait);
    return take_requests(-1);
  }

  /// Wait until `min_request_count` requests have arrived, for at most
  /// `max_wait`, and take at most `max_request_count` of them (a negative
  /// count takes all).
  LoanedSamplesType receive_requests(int min_request_count, int max_request_count,
                                     const dds::Duration& max_wait) {
    (void)wait_for_requests(min_request_count, max_wait);
    return take_requests(max_request_count);
  }

  /// Wait until `min_count` requests are there to be taken, for at most
  /// `max_wait`.
  bool wait_for_requests(const dds::Duration& max_wait) { return wait_for_requests(1, max_wait); }
  bool wait_for_requests(int min_count, const dds::Duration& max_wait) {
    const detail::Deadline deadline(max_wait);
    for (;;) {
      live().check_listener_error();
      if (live().available() >= min_count) {
        return true;
      }
      if (deadline.expired()) {
        return false;
      }
      detail::pause_briefly();
    }
  }

  /// A `take` removes the request from the replier, a `read` leaves it
  /// where it is. They do not wait. Return whether a request was there.
  bool take_request(Sample<TReq>& request) { return collect_one(request, true); }
  bool take_request(SampleRef<TReq> request) { return collect_one(request, true); }
  LoanedSamplesType take_requests(int max_samples) { return collect(max_samples, true); }
  bool read_request(Sample<TReq>& request) { return collect_one(request, false); }
  bool read_request(SampleRef<TReq> request) { return collect_one(request, false); }
  LoanedSamplesType read_requests(int max_samples) { return collect(max_samples, false); }

  /// Whether samples that carry no data (the state changes of the
  /// request topic) are handed out too, as samples whose `valid_data` is
  /// false. Returns the setting it replaces. They are not by default.
  bool receive_nondata_samples(bool enable) { return live().receive_nondata_samples(enable); }

  /// The params the replier was made with.
  const ReplierParams& get_replier_params() const { return live_const().params(); }
  /// The request DataReader and the reply DataWriter.
  RequestDataReader get_request_datareader() const { return live_const().request_reader(); }
  ReplyDataWriter get_reply_datawriter() const { return live_const().reply_writer(); }

  /// §7.10.2: "The users may modify the mutable QoS policies at
  /// run-time." Throws `ImmutablePolicyError` for one that cannot change.
  void set_qos(const RpcQos& qos) { live().set_qos(qos); }

  /// The QoS the endpoints have now.
  RpcQos qos() const { return impl_ ? impl_->qos() : RpcQos{}; }

  /// Send one reply, correlated to the request it answers. §7.8.1.
  void send_reply(const TRep& reply, const dds::SampleIdentity& related_request_id) {
    send_reply(reply, related_request_id, REMOTE_EX_OK);
  }

  /// Send one reply carrying a remote exception code. §7.5.2.
  void send_reply(const TRep& reply, const dds::SampleIdentity& related_request_id,
                  RemoteExceptionCode_t code) {
    (void)live().send_reply(reply, related_request_id, code);
  }

  /// Send the reply in `reply`; the identity the writer gave it goes into
  /// the sample. §7.11.1.4.17.
  void send_reply(WriteSample<TRep>& reply, const dds::SampleIdentity& related_request_id) {
    reply.set_identity(live().send_reply(reply.data(), related_request_id, REMOTE_EX_OK));
  }

  /// §7.11.1.4.18: the same for a `WriteSampleRef`.
  void send_reply(WriteSampleRef<TRep>& reply, const dds::SampleIdentity& related_request_id) {
    if (reply.is_null()) {
      throw NullReferenceError("the WriteSampleRef refers to no sample");
    }
    reply.set_identity(live().send_reply(reply.data(), related_request_id, REMOTE_EX_OK));
  }

  /// Take every request that has arrived and answer it through the
  /// listener the params carried. §7.11.1.4.7 and §7.11.1.4.8.
  ///
  /// A synchronous listener (§7.11.1.4.7) that throws makes the service
  /// answer with the remote exception code of what it threw (§7.5.2,
  /// §7.11.1.3), or `REMOTE_EX_UNKNOWN_EXCEPTION` for any other exception.
  std::size_t dispatch_requests() {
    live().check_listener_error();
    auto listener = live().listener();
    if (!listener) {
      return 0;
    }
    if (auto* full = dynamic_cast<ReplierListener<TReq, TRep>*>(listener.get())) {
      full->on_request_available(*this);
      return 1;
    }
    auto* simple = dynamic_cast<SimpleReplierListener<TReq, TRep>*>(listener.get());
    if (!simple) {
      return 0;
    }
    std::size_t answered = 0;
    auto requests = receive_requests();
    for (auto request : requests) {
      const dds::SampleIdentity identity = request.info().identity;
      try {
        // The listener hands over a heap object; it is deleted here.
        const std::unique_ptr<TRep> reply(simple->process_request(request.sample(), identity));
        if (reply) {
          send_reply(*reply, identity);
        }
      } catch (const LocalError&) {
        throw;
      } catch (...) {
        send_reply(TRep{}, identity, detail::remote_exception_code_of(std::current_exception()));
      }
      ++answered;
    }
    return answered;
  }

  void close() override {
    if (impl_) {
      impl_->close();
    }
  }
  bool is_null() const override { return !impl_ || impl_->closed(); }

  /// The exception a listener threw on the thread that received the
  /// request, if any, cleared by the call. Every other call on this
  /// replier raises it first, so it is not lost.
  std::exception_ptr take_listener_error() { return live().take_listener_error(); }

 private:
  /// A handle on an existing replier, for a listener.
  explicit Replier(std::shared_ptr<Impl> impl) : impl_(std::move(impl)) {}

  /// What every copy of the reference shares.
  class Impl : public detail::ListenerErrorSlot {
   public:
    explicit Impl(const ReplierParams& params)
        : params_(params),
          instance_name_(params.instance_name()),
          correlation_(params.correlation()),
          accept_unannounced_(params.accept_unannounced()),
          hold_for_client_(resolve_client_matching(params)),
          qos_(params.resolved_qos_for(false)),
          encoding_(params.wire_encoding()) {
      qos_.request.representation = encoding_;
      qos_.reply.representation = encoding_;
      qos_.request.type_information = detail::type_information_of<TReq>(0);
      qos_.reply.type_information = detail::type_information_of<TRep>(0);
      const TopicNames names = params.topic_names();
      auto publisher = params.resolved_publisher();
      auto subscriber = params.resolved_subscriber();
      // §7.5.1.1.6/7: request and reply types, not topic names.
      reader_ = subscriber->create_reader(names.request, dds_type_traits<TReq>::type_name(),
                                          qos_.request);
      writer_ = publisher->create_writer(names.reply, dds_type_traits<TRep>::type_name(),
                                         qos_.reply);
      if (!writer_ || !reader_) {
        throw OutOfResourcesError("the binding created no request reader or reply writer");
      }
      if (correlation_ == Correlation::SAMPLE_IDENTITY) {
        if (writer_->supports_rpc_discovery() && reader_->supports_rpc_discovery()) {
          // §7.6.2.1: the service-side endpoints name each other and carry
          // the instance name "specified by the user at the service-side"
          // (none if none) and the topic aliases.
          const auto aliases = params.topic_aliases();
          RpcDiscovery reply_writer;
          reply_writer.has_instance_name = !instance_name_.empty();
          reply_writer.instance_name = instance_name_;
          reply_writer.has_related = true;
          reader_->guid(reply_writer.related);
          reply_writer.aliases = aliases.second;
          RpcDiscovery request_reader = reply_writer;
          std::memcpy(request_reader.related, writer_->next_identity().writer_guid,
                      sizeof(request_reader.related));
          request_reader.aliases = aliases.first;
          writer_->announce_rpc_discovery(reply_writer);
          reader_->announce_rpc_discovery(request_reader);
          discovery_ = true;
        } else if (!instance_name_.empty()) {
          // §7.6.2.1.1: an Enhanced service announces its instance name in
          // `PID_SERVICE_INSTANCE_NAME`. Accepting a name that is never
          // announced would leave a requester bound to it without a peer.
          throw UnsupportedError(
              "an Enhanced replier with an instance name needs to announce "
              "PID_SERVICE_INSTANCE_NAME, which this binding does not provide");
        }
      }
    }

    ~Impl() {
      if (reader_) {
        reader_->set_data_callback(nullptr);
      }
    }

    /// Ask the reader to call `action` when samples arrive.
    void listen(std::function<void()> action) { reader_->set_data_callback(std::move(action)); }

    bool closed() const {
      std::lock_guard<std::mutex> guard(lock_);
      return closed_;
    }

    void close() {
      std::shared_ptr<DataReader> reader;
      {
        std::lock_guard<std::mutex> guard(lock_);
        if (closed_) {
          return;
        }
        closed_ = true;
        reader = reader_;
      }
      // Outside the lock: stopping the arrival callback waits for one that
      // is running, and that one may be waiting for the lock.
      reader->set_data_callback(nullptr);
      std::lock_guard<std::mutex> guard(lock_);
      held_back_.clear();
      writer_.reset();
      reader_.reset();
    }

    std::shared_ptr<ListenerBase> listener() const { return params_.listener(); }

    /// §7.10.2: change the mutable policies of the request reader and the
    /// reply writer.
    void set_qos(const RpcQos& qos) {
      std::lock_guard<std::mutex> guard(lock_);
      if (closed_) {
        throw AlreadyClosedError("the replier is closed");
      }
      reader_->set_qos(qos.request);
      try {
        writer_->set_qos(qos.reply);
      } catch (...) {
        reader_->set_qos(qos_.request);
        throw;
      }
      qos_ = qos;
    }

    RpcQos qos() const {
      std::lock_guard<std::mutex> guard(lock_);
      return qos_;
    }

    /// Every request that has been taken from the reader and answered
    /// the requirements of §7.6.2.2.1: `receive_requests()` hands them all
    /// out, the `take` and `read` calls some of them.
    LoanedSamples<TReq> receive_requests() {
      std::lock_guard<std::mutex> guard(lock_);
      pull();
      std::vector<Sample<TReq>> out;
      out.swap(ready_);
      return LoanedSamples<TReq>(std::move(out));
    }

    /// How many requests are there to be taken.
    int available() {
      std::lock_guard<std::mutex> guard(lock_);
      pull();
      return static_cast<int>(ready_.size());
    }

    /// Collect up to `max_count` requests (a negative count: all) into
    /// `out`; `remove` takes them out of the replier, otherwise they stay.
    void collect(int max_count, bool remove, std::vector<Sample<TReq>>& out) {
      std::lock_guard<std::mutex> guard(lock_);
      pull();
      const std::size_t limit = max_count < 0 ? ready_.size()
                                              : std::min(ready_.size(), static_cast<std::size_t>(max_count));
      out.insert(out.end(), ready_.begin(), ready_.begin() + static_cast<std::ptrdiff_t>(limit));
      if (remove) {
        ready_.erase(ready_.begin(), ready_.begin() + static_cast<std::ptrdiff_t>(limit));
      }
    }

    bool receive_nondata_samples(bool enable) {
      std::lock_guard<std::mutex> guard(lock_);
      const bool before = receive_nondata_;
      receive_nondata_ = enable;
      return before;
    }

    const ReplierParams& params() const { return params_; }

    std::shared_ptr<DataReader> request_reader() const {
      std::lock_guard<std::mutex> guard(lock_);
      if (closed_) {
        throw AlreadyClosedError("the replier is closed");
      }
      return reader_;
    }

    std::shared_ptr<DataWriter> reply_writer() const {
      std::lock_guard<std::mutex> guard(lock_);
      if (closed_) {
        throw AlreadyClosedError("the replier is closed");
      }
      return writer_;
    }

   private:
    /// Take what has arrived into `ready_`. The caller holds `lock_`.
    void pull() {
      if (closed_) {
        throw AlreadyClosedError("the replier is closed");
      }
      std::vector<Sample<TReq>>& out = ready_;
      // What an earlier call held back goes first.
      std::vector<std::pair<Octets, dds::SampleInfo>> incoming;
      incoming.swap(held_back_);
      for (auto& taken : reader_->take()) {
        incoming.push_back(std::move(taken));
      }
      if (hold_for_client_ && !discovery_ && !incoming.empty() && writer_->matched_count() == 0) {
        // §7.6.2.2.1: while the reply writer has discovered no client reply
        // reader, a reply could not be delivered, so no request is
        // processed. The request is kept for the call after the match.
        held_back_ = std::move(incoming);
        return;
      }
      for (auto& taken : incoming) {
        if (!taken.second.valid_data) {
          // A state change of the topic, not a request.
          if (receive_nondata_) {
            out.emplace_back(TReq{}, taken.second);
          }
          continue;
        }
        if (hold_for_client_ && discovery_ && correlation_ == Correlation::SAMPLE_IDENTITY &&
            !reader_->client_matching_complete(*writer_, taken.second.identity.writer_guid,
                                               accept_unannounced_)) {
          // §7.6.2.2.1, Figure 7.3: the mask of this client. Its request
          // writer is masked until the service has discovered both its
          // endpoints; the request is kept, not rejected for good.
          held_back_.push_back(std::move(taken));
          continue;
        }
        if (correlation_ == Correlation::SAMPLE_IDENTITY) {
          // §7.8.2.1: the identity of the request is the identity of its
          // DATA, which the reader reports in the SampleInfo. Without it
          // no reply can be tied to the request.
          if (taken.second.identity.is_unknown()) {
            throw UnsupportedError(
                "the binding reports no sample identity; an Enhanced replier cannot correlate");
          }
          std::size_t at = 0;
          TReq value{};
          if (!detail::deserialize_payload<TReq>(
                  taken.first, at, value, detail::encoding_of(taken.second, encoding_),
                  typename detail::has_deserialize_in<TReq>::type())) {
            // §7.5.2: a request that cannot be decoded (a @Choice with no
            // member or with two, §7.5.1.2.1.1) is answered
            // REMOTE_EX_INVALID_ARGUMENT, not dropped.
            write_reply(writer_, TRep{}, taken.second.identity, REMOTE_EX_INVALID_ARGUMENT);
            continue;
          }
          out.emplace_back(std::move(value), taken.second);
          continue;
        }
        std::size_t at = 0;
        RequestHeader header;
        if (!decode_request_header(taken.first, at, header)) {
          continue;
        }
        // §7.11.1.4.3: "Requests sent through a bound requester reference
        // shall be sent to the bound service instance only." The empty
        // name is the default instance and matches only itself, so a named
        // replier does not answer a request that names no instance, and an
        // unnamed replier does not answer one bound to an instance.
        if (header.instanceName != instance_name_) {
          continue;
        }
        TReq value{};
        at = correlation_ == Correlation::HEADER_IN_TYPE ? 0 : detail::round_up_4(at);
        if (!detail::deserialize_payload<TReq>(
                taken.first, at, value, detail::encoding_of(taken.second, encoding_),
                typename detail::has_deserialize_in<TReq>::type())) {
          // §7.5.2: the header named the request, so the answer can say
          // that its arguments could not be read.
          write_reply(writer_, TRep{}, header.requestId, REMOTE_EX_INVALID_ARGUMENT);
          continue;
        }
        dds::SampleInfo info = taken.second;
        // §7.8.1: the Basic profile carries the request identity explicitly
        // in the header, so that is where the replier reads it — not from
        // the transport's sample metadata, which a binding may leave unset.
        info.identity = header.requestId;
        out.emplace_back(std::move(value), info);
      }
    }

   public:
    /// Send a reply; returns the identity the writer gave it.
    dds::SampleIdentity send_reply(const TRep& reply, const dds::SampleIdentity& related_request_id,
                                   RemoteExceptionCode_t code) {
      std::shared_ptr<DataWriter> writer;
      {
        std::lock_guard<std::mutex> guard(lock_);
        if (closed_) {
          throw AlreadyClosedError("the replier is closed");
        }
        writer = writer_;
      }
      return write_reply(writer, reply, related_request_id, code);
    }

   private:
    /// Writes a reply on `writer`; the caller holds `lock_` or has taken the
    /// writer out from under it.
    dds::SampleIdentity write_reply(const std::shared_ptr<DataWriter>& writer, const TRep& reply,
                                    const dds::SampleIdentity& related_request_id,
                                    RemoteExceptionCode_t code) {
      if (correlation_ == Correlation::SAMPLE_IDENTITY) {
        // §7.8.2.3: "Before sending a reply, the service implementation
        // needs to set the related sample identity of the reply sample."
        // The code is the reply's own `remoteEx` member (§7.5.1.2.5).
        if (code == REMOTE_EX_OK) {
          return writer->write_related(
              detail::serialize_payload<TRep>(reply, 0, encoding_,
                                              typename detail::has_serialize_at<TRep>::type()),
              related_request_id);
        }
        return writer->write_related(
            serialize_remote_exception(code,
                                       typename detail::can_build_remote_exception<TRep>::type()),
            related_request_id);
      }
      ReplyHeader header;
      header.relatedRequestId = related_request_id;
      header.remoteEx = code;
      if (correlation_ == Correlation::HEADER_IN_TYPE) {
        // §7.3.1.4: the reply type carries its own header; the runtime
        // names the request and the outcome in it. A reply that reports a
        // remote exception is the reply type as the caller gave it.
        TRep stamped = reply;
        detail::stamp_reply_header<TRep>(stamped, header,
                                         typename detail::has_set_reply_header<TRep>::type());
        return writer->write(detail::serialize_payload<TRep>(
            stamped, 0, encoding_, typename detail::has_serialize_at<TRep>::type()));
      }
      Octets octets = encode_reply_header(header);
      octets.resize(detail::round_up_4(octets.size()), 0);
      const Octets payload = detail::serialize_payload<TRep>(
          reply, octets.size(), encoding_, typename detail::has_serialize_at<TRep>::type());
      octets.insert(octets.end(), payload.begin(), payload.end());
      return writer->write(octets);
    }

   public:

   private:
    static bool resolve_client_matching(const ReplierParams& params) {
      switch (params.client_matching()) {
        case ClientMatching::HOLD:
          return true;
        case ClientMatching::NONE:
          return false;
        case ClientMatching::DEFAULT:
          break;
      }
      // §7.6.2.2 belongs to the Enhanced profile (§7.6.1 for the Basic one).
      return params.correlation() == Correlation::SAMPLE_IDENTITY;
    }

    static Octets serialize_remote_exception(RemoteExceptionCode_t code, std::true_type) {
      return dds_type_traits<TRep>::serialize(dds_type_traits<TRep>::from_remote_exception(code));
    }
    static Octets serialize_remote_exception(RemoteExceptionCode_t, std::false_type) {
      throw UnsupportedError(
          "the Enhanced reply type has no remoteEx member to carry a remote exception code");
    }

    ReplierParams params_;
    std::shared_ptr<DataWriter> writer_;
    std::shared_ptr<DataReader> reader_;
    std::string instance_name_;
    Correlation correlation_;
    bool accept_unannounced_;
    /// Whether the binding announces and reads the discovery extension and
    /// this replier uses it (the Enhanced profile).
    bool discovery_ = false;
    bool hold_for_client_;
    RpcQos qos_;
    WireEncoding encoding_;
    /// Requests taken from the reader and not yet processed because the
    /// client has not matched.
    std::vector<std::pair<Octets, dds::SampleInfo>> held_back_;
    /// Requests that are ready to be handed out.
    std::vector<Sample<TReq>> ready_;
    bool receive_nondata_ = false;
    bool closed_ = false;
    mutable std::mutex lock_;
  };

  /// A request has arrived: answer it. Runs on the binding's thread; what
  /// it throws is kept for the next call.
  void on_arrival() {
    try {
      if (impl_->closed()) {
        return;
      }
      (void)dispatch_requests();
    } catch (...) {
      impl_->remember_error(std::current_exception());
    }
  }

  Impl& live() {
    if (!impl_) {
      throw NullReferenceError("the replier holds no entities");
    }
    return *impl_;
  }
  const Impl& live_const() const {
    if (!impl_) {
      throw NullReferenceError("the replier holds no entities");
    }
    return *impl_;
  }

  LoanedSamplesType collect(int max_count, bool remove) {
    live().check_listener_error();
    std::vector<Sample<TReq>> found;
    live().collect(max_count, remove, found);
    return LoanedSamples<TReq>(std::move(found));
  }

  template <typename Target>
  bool collect_one(Target& target, bool remove) {
    live().check_listener_error();
    std::vector<Sample<TReq>> found;
    live().collect(1, remove, found);
    if (found.empty()) {
      return false;
    }
    assign(target, found.front());
    return true;
  }
  static void assign(Sample<TReq>& to, const Sample<TReq>& from) { to = from; }
  static void assign(SampleRef<TReq>& to, const Sample<TReq>& from) {
    to.data() = from.data();
    to.info() = from.info();
  }

  std::shared_ptr<Impl> impl_;
};

// -- What a generated interface tells the runtime -----------------------

/// What an interface declares about its endpoints, as the generated
/// `${interface}Support::defaults()` gives it: the `<interface_name>` of
/// §7.4.1, the `@service(name = "…")`, the whole topic names of
/// `@DDSRequestTopic`/`@DDSReplyTopic` (§7.4.2.2) and the profile URLs of
/// `@Qos` (§7.10.1). An empty string is "not declared".
struct InterfaceDefaults {
  const char* interface_name;
  const char* service_name;
  const char* request_topic;
  const char* reply_topic;
  const char* request_profile;
  const char* reply_profile;
};

}  // namespace core
namespace detail {

/// Fill in what `params` leave open from the interface's declarations:
/// the run-time settings stay, §7.4.2.3 puts them first.
template <typename Params>
void apply_defaults(Params& params, const core::InterfaceDefaults& defaults) {
  if (params.interface_name().empty()) {
    params.interface_name(defaults.interface_name);
  }
  if (params.annotated_service_name().empty()) {
    params.annotated_service_name(defaults.service_name);
  }
  if (params.request_topic_name().empty()) {
    params.request_topic_name(defaults.request_topic);
  }
  if (params.reply_topic_name().empty()) {
    params.reply_topic_name(defaults.reply_topic);
  }
  if (params.request_profile().empty()) {
    params.request_profile(defaults.request_profile);
  }
  if (params.reply_profile().empty()) {
    params.reply_profile(defaults.reply_profile);
  }
}

}  // namespace detail
namespace core {

/// The `Requester` params a `Client` calls through, under the
/// declarations of its interface.
inline RequesterParams requester_params_of(const ClientParams& params,
                                           const InterfaceDefaults& defaults) {
  RequesterParams out;
  params.copy_to(out);
  out.binding_style(params.binding_style());
  detail::apply_defaults(out, defaults);
  return out;
}

/// The `Replier` params a `Service` answers through.
inline ReplierParams replier_params_of(const ServiceParams& params,
                                       const InterfaceDefaults& defaults) {
  ReplierParams out;
  params.copy_to(out);
  out.binding_style(params.binding_style());
  detail::apply_defaults(out, defaults);
  return out;
}

/// `requester_params_of` for an interface mapped by the Enhanced Service
/// Mapping (§7.5.1.2): the sample has no header and a reply is tied to its
/// request by the sample identity (§7.8.2), and the topic types are PL_CDR2
/// (XCDR2), which their `dds_type_traits` write whatever the params say.
inline RequesterParams enhanced_requester_params_of(const ClientParams& params,
                                                    const InterfaceDefaults& defaults) {
  RequesterParams out = requester_params_of(params, defaults);
  out.correlation(Correlation::SAMPLE_IDENTITY).wire_encoding(WireEncoding::XCDR2);
  return out;
}

/// `replier_params_of` for an interface mapped by the Enhanced Service
/// Mapping. See `enhanced_requester_params_of`.
inline ReplierParams enhanced_replier_params_of(const ServiceParams& params,
                                                const InterfaceDefaults& defaults) {
  ReplierParams out = replier_params_of(params, defaults);
  out.correlation(Correlation::SAMPLE_IDENTITY).wire_encoding(WireEncoding::XCDR2);
  return out;
}

/// The name a `Service` is discoverable under: the run-time service name
/// of `params`, else the interface's (§7.4.1, §7.4.2.3).
inline std::string service_name_of(const ServiceParams& params,
                                   const InterfaceDefaults& defaults) {
  if (!params.service_name().empty()) {
    return params.service_name();
  }
  if (!params.annotated_service_name().empty()) {
    return params.annotated_service_name();
  }
  return std::string(defaults.service_name);
}

/// A future that is ready: the result of a call with nothing to wait for.
inline future<void> ready_future() {
  std::promise<void> promise;
  promise.set_value();
  return promise.get_future();
}

// -- §7.11.1.5 The function-call style ---------------------------------

/// The state of a service. §7.11.1.5.2: pause and resume change it, close
/// ends it.
enum ServiceStatus { CLOSED, PAUSED, RUNNING };

class Server;

/// §7.11.1.5.2: "A ServiceEndpoint provides type-independent functions
/// to manage a service (e.g., pause, resume, close, etc.). A
/// ServiceEndpoint shall not be instantiated directly; it can be
/// obtained from a Service object. ServiceEndpoint is a reference type."
class ServiceEndpoint : public RPCEntity {
 public:
  void pause() { state_->paused = true; }
  void resume() { state_->paused = false; }
  bool is_paused() const { return state_->paused; }

  /// CLOSED once `close()` has run, PAUSED between `pause()` and
  /// `resume()`, RUNNING otherwise.
  ServiceStatus status() const {
    if (state_->closed) {
      return CLOSED;
    }
    return state_->paused ? PAUSED : RUNNING;
  }

  void close() override { state_->close(); }
  bool is_null() const override { return state_->closed; }

  /// The params the service runs with: those it was made from, with what
  /// its interface declares put in (§7.4.2.3).
  ServiceParams get_service_params() const {
    if (!state_->params) {
      throw NullReferenceError("the service holds no entities");
    }
    return state_->params();
  }

  /// The request DataReader of the service.
  template <class TReq>
  typename dds_type_traits<TReq>::DataReader get_request_datareader() const {
    if (!state_->request_reader) {
      throw NullReferenceError("the service holds no entities");
    }
    return state_->request_reader();
  }

  /// The reply DataWriter of the service.
  template <class TRep>
  typename dds_type_traits<TRep>::DataWriter get_reply_datawriter() const {
    if (!state_->reply_writer) {
      throw NullReferenceError("the service holds no entities");
    }
    return state_->reply_writer();
  }

  /// §7.10.2: change the mutable QoS policies of the service's endpoints
  /// at run time.
  void set_qos(const RpcQos& qos) {
    if (state_->set_qos.empty()) {
      throw NullReferenceError("the service holds no entities");
    }
    for (const auto& apply : state_->set_qos) {
      apply(qos);
    }
  }

  /// The QoS the service's endpoints have now.
  RpcQos qos() const { return state_->qos ? state_->qos() : RpcQos{}; }

 protected:
  ServiceEndpoint() : state_(std::make_shared<State>()) {}

  /// The replier a service answers through: `close()` closes it, which
  /// deletes its DataReader and DataWriter (§7.9.3), and `set_qos()`
  /// changes its QoS. A copy of it, so the endpoint and the service share
  /// the entities (they are references).
  template <typename TReplier>
  void attach(const TReplier& replier) {
    auto held = std::make_shared<TReplier>(replier);
    on_close([held]() { held->close(); });
    on_set_qos([held](const RpcQos& qos) { held->set_qos(qos); });
    if (!state_->qos) {
      state_->qos = [held]() { return held->qos(); };
    }
    capture_entities(held, 0);
  }

  /// Register what `set_qos()` applies the QoS to.
  void on_set_qos(std::function<void(const RpcQos&)> apply) {
    state_->set_qos.push_back(std::move(apply));
  }

  /// Register what `close()` has to release. §7.9.3: "Deactivation shall
  /// delete the underlying DDS entities". Run once, at `close()` or when
  /// the last copy of the endpoint is gone.
  void on_close(std::function<void()> release) { state_->releasers.push_back(std::move(release)); }

 private:
  friend class Server;

  struct State {
    std::atomic<bool> paused{false};
    std::atomic<bool> closed{false};
    /// Whether a `Server` holds the service. §7.11.1.5.1: "A Service may
    /// belong to only one Server."
    std::atomic<bool> owned{false};
    std::vector<std::function<void()>> releasers;
    std::vector<std::function<void(const RpcQos&)>> set_qos;
    std::function<RpcQos()> qos;
    std::function<ServiceParams()> params;
    std::function<std::shared_ptr<DataReader>()> request_reader;
    std::function<std::shared_ptr<DataWriter>()> reply_writer;

    void close() {
      if (closed.exchange(true)) {
        return;
      }
      for (auto& release : releasers) {
        release();
      }
      releasers.clear();
    }
    ~State() { close(); }
  };

  /// The entities and params of the replier of the interface itself: a
  /// service attached after it, one of a base interface, is not a replier
  /// and leaves them as they are.
  template <typename TReplier>
  auto capture_entities(const std::shared_ptr<TReplier>& held, int)
      -> decltype(held->get_replier_params(), void()) {
    if (state_->params) {
      return;
    }
    state_->params = [held]() {
      ServiceParams out;
      held->get_replier_params().copy_to(out);
      return out;
    };
    state_->request_reader = [held]() { return held->get_request_datareader(); };
    state_->reply_writer = [held]() { return held->get_reply_datawriter(); };
  }
  template <typename TReplier>
  void capture_entities(const std::shared_ptr<TReplier>&, long) {}

  /// Claim the service for a server; false if another already holds it.
  bool claim() { return !state_->owned.exchange(true); }
  void release_claim() { state_->owned = false; }

  std::shared_ptr<State> state_;
};

/// §7.11.1.5.5: "A ClientEndpoint provides functions to obtain the
/// underlying DDS entities at the client side. ClientEndpoint inherits
/// from ServiceProxy. A ClientEndpoint shall not be instantiated
/// directly; it can be obtained from a Client object."
class ClientEndpoint : public ServiceProxy {
 public:
  void close() override {
    if (target_) {
      target_->close();
    }
    for (auto& base : bases_) {
      base->close();
    }
  }
  bool is_null() const override { return !target_ || target_->is_null(); }

  /// §7.11.1.4.3: requests sent through a bound reference go to the bound
  /// instance only. The client sends through its requester and those of its
  /// base interfaces, so the binding is theirs too.
  void bound(const std::string& instance_name) override {
    if (target_) {
      target_->bind(instance_name);
    }
    for (const auto& base : bases_) {
      base->bind(instance_name);
    }
  }

  bool is_instance_discovered(const std::string& instance_name) const override {
    if (!target_ || !target_->is_instance_discovered(instance_name)) {
      return false;
    }
    for (const auto& base : bases_) {
      if (!base->is_instance_discovered(instance_name)) {
        return false;
      }
    }
    return true;
  }

  /// The instances discovered for the interface and for every base
  /// interface: an instance serves the client only if it serves all of
  /// them.
  std::vector<std::string> get_discoverd_service_instances() const override {
    if (!target_) {
      return std::vector<std::string>();
    }
    std::vector<std::string> found = target_->get_discoverd_service_instances();
    for (const auto& base : bases_) {
      const std::vector<std::string> of_base = base->get_discoverd_service_instances();
      found.erase(std::remove_if(found.begin(), found.end(),
                                 [&of_base](const std::string& name) {
                                   return std::find(of_base.begin(), of_base.end(), name) ==
                                          of_base.end();
                                 }),
                  found.end());
    }
    return found;
  }

  std::shared_ptr<ServiceProxy> share_reference() const override {
    return std::make_shared<ClientEndpoint>(*this);
  }

  /// The request DataWriter of the client.
  template <class TReq>
  typename dds_type_traits<TReq>::DataWriter get_request_datawriter() const {
    if (!request_writer_) {
      throw NullReferenceError("the client holds no entities");
    }
    return request_writer_();
  }

  /// The reply DataReader of the client.
  template <class TRep>
  typename dds_type_traits<TRep>::DataReader get_reply_datareader() const {
    if (!reply_reader_) {
      throw NullReferenceError("the client holds no entities");
    }
    return reply_reader_();
  }

  /// The params the client runs with: those it was made from, with what
  /// its interface declares put in (§7.4.2.3).
  ClientParams get_client_params() const {
    if (!params_) {
      throw NullReferenceError("the client holds no entities");
    }
    return params_();
  }

  /// §7.10.2: change the mutable QoS policies of the client's endpoints,
  /// those of its base interfaces' clients included, at run time.
  void set_qos(const RpcQos& qos) {
    if (set_qos_.empty()) {
      throw NullReferenceError("the client holds no entities");
    }
    for (const auto& apply : set_qos_) {
      apply(qos);
    }
  }

  /// The QoS the client's endpoints have now.
  RpcQos qos() const { return qos_ ? qos_() : RpcQos{}; }

 protected:
  ClientEndpoint() = default;

  /// The requester the client calls through. A copy of it, so the
  /// endpoint and the client share the entities (they are references).
  template <typename TRequester>
  void attach(const TRequester& requester) {
    auto held = std::make_shared<TRequester>(requester);
    target_ = held;
    set_qos_.insert(set_qos_.begin(), [held](const RpcQos& qos) { held->set_qos(qos); });
    qos_ = [held]() { return held->qos(); };
    params_ = [held]() {
      ClientParams out;
      held->get_requester_params().copy_to(out);
      return out;
    };
    request_writer_ = [held]() { return held->get_request_datawriter(); };
    reply_reader_ = [held]() { return held->get_reply_datareader(); };
  }

  /// The client of a base interface, which reaches the base's operations
  /// on the base's own topics (§7.5.1.1.8). Closed, awaited and given QoS
  /// with this one.
  template <typename TClient>
  void attach_base(const TClient& base) {
    auto held = std::make_shared<TClient>(base);
    bases_.push_back(held);
    set_qos_.push_back([held](const RpcQos& qos) { held->set_qos(qos); });
  }

  /// As `attach_base`, for a client of a base interface that is a proxy of the application's
  /// class (the normative `ServiceProxy`) and not an implementation class: `proxy` stands for
  /// it.
  void attach_base_proxy(std::shared_ptr<ServiceProxy> proxy,
                         std::function<void(const RpcQos&)> set_qos) {
    bases_.push_back(std::move(proxy));
    set_qos_.push_back(std::move(set_qos));
  }

 private:
  std::shared_ptr<ServiceProxy> target_;
  std::vector<std::shared_ptr<ServiceProxy>> bases_;
  std::vector<std::function<void(const RpcQos&)>> set_qos_;
  std::function<RpcQos()> qos_;
  std::function<ClientParams()> params_;
  std::function<std::shared_ptr<DataWriter>()> request_writer_;
  std::function<std::shared_ptr<DataReader>()> reply_reader_;
};

/// §7.11.1.5.1: "Service is a reference type and accepts a reference to
/// the service implementation. A Service instantiates the underlying DDS
/// entities and makes the service discoverable. Every Service inherits
/// from ServiceEndpoint. A Service may belong to only one Server."
class Service : public ServiceEndpoint {
 public:
  virtual ~Service() = default;

 protected:
  Service() = default;
  /// A service that shares the state of `endpoint`: the one the `Server` holds for a service of
  /// the application's class.
  explicit Service(const ServiceEndpoint& endpoint) : ServiceEndpoint(endpoint) {}

 public:
  /// Take the requests that have arrived and answer them. Returns how
  /// many were answered, so a `Server` can report progress.
  virtual std::size_t dispatch() = 0;
  /// The name the service is discoverable under. §7.4.1.
  virtual const std::string& service_name() const = 0;
};

/// §7.11.1.5.3 (normative header): the parameters of a `Server`.
class ServerParams {
 public:
  ServerParams() = default;

  /// The params a service made through this server starts from.
  ServerParams& default_service_params(const ServiceParams& service_params) {
    service_params_ = service_params;
    return *this;
  }
  ServiceParams default_service_params() const { return service_params_; }

 private:
  ServiceParams service_params_;
};

/// §7.11.1.5.3: "A Server is a container of one or more Services.
/// Server.run() function begins dispatching the requests to the
/// Service implementation. Server is a reference type."
///
/// Figure 7.5: a `Server` is an `RPCEntity`; `close()` closes the
/// services it holds and leaves it null.
class Server : public RPCEntity {
 public:
  Server() : state_(std::make_shared<State>()) {}

  /// A server whose `default_service_params()` are `params`'.
  explicit Server(const ServerParams& params) : state_(std::make_shared<State>()) {
    state_->params = params;
  }

  /// Add one service. §7.11.1.5.1: "A Service may belong to only one
  /// Server": a service that another server holds, or this one already
  /// does, is refused with `IllegalOperationError`. A closed server takes
  /// none (`AlreadyClosedError`).
  void add(std::shared_ptr<Service> service) {
    if (!service) {
      throw NullReferenceError("the service is null");
    }
    std::lock_guard<std::mutex> guard(state_->lock);
    if (state_->closed) {
      throw AlreadyClosedError("the server was closed");
    }
    if (!service->claim()) {
      throw IllegalOperationError("a Service may belong to only one Server (§7.11.1.5.1)");
    }
    state_->services.push_back(std::move(service));
  }

  std::size_t size() const {
    std::lock_guard<std::mutex> guard(state_->lock);
    return state_->services.size();
  }

  /// The params a service made through this server starts from.
  ServiceParams default_service_params() const {
    std::lock_guard<std::mutex> guard(state_->lock);
    return state_->params.default_service_params();
  }

  /// Dispatch one round over every service that is not paused. Returns
  /// how many requests were answered.
  std::size_t run_once() {
    std::vector<std::shared_ptr<Service>> services;
    {
      std::lock_guard<std::mutex> guard(state_->lock);
      services = state_->services;
    }
    std::size_t answered = 0;
    for (auto& service : services) {
      if (service->is_paused() || service->is_null()) {
        continue;
      }
      try {
        answered += service->dispatch();
      } catch (const AlreadyClosedError&) {
        // The service was closed (by `close()` of this server or of the
        // service) between the look above and the dispatch.
        if (!service->is_null()) {
          throw;
        }
      }
    }
    return answered;
  }

  /// Dispatch until `until` says to stop. A parameter rather than a signal
  /// handler: a library that installed one would take it from the
  /// application.
  void run(const std::function<bool()>& until) {
    while (!until()) {
      (void)run_once();
    }
  }

  /// §7.11.1.5.3 (normative header): "blocking". Dispatch the requests as
  /// they arrive until the server is closed.
  void run() {
    while (!is_null()) {
      if (run_once() == 0) {
        detail::pause_briefly();
      }
    }
  }

  /// §7.11.1.5.3 (normative header): "non-blocking". Dispatch the requests
  /// that arrive within `max_wait`: returns as soon as a round has
  /// answered one, or when the time is up, or when the server is closed.
  void run(const dds::Duration& max_wait) {
    const detail::Deadline deadline(max_wait);
    while (!is_null()) {
      if (run_once() > 0 || deadline.expired()) {
        return;
      }
      detail::pause_briefly();
    }
  }

  /// Close the services this server holds and the server. Every
  /// reference to it is null afterwards, and a `run()` of it returns.
  void close() override {
    std::vector<std::shared_ptr<Service>> services;
    {
      std::lock_guard<std::mutex> guard(state_->lock);
      state_->closed = true;
      services.swap(state_->services);
    }
    for (auto& service : services) {
      service->close();
      service->release_claim();
    }
  }

  bool is_null() const override { return state_->closed; }

 private:
  struct State {
    mutable std::mutex lock;
    std::vector<std::shared_ptr<Service>> services;
    ServerParams params;
    std::atomic<bool> closed{false};
  };
  std::shared_ptr<State> state_;
};


}  // namespace core
}  // namespace rpc
}  // namespace dds

#endif  // ZERODDS_DDS_RPC_CORE_HPP
