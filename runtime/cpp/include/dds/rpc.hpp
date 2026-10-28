// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/rpc.hpp — the C++ request-reply and function-call language bindings of
// DDS-RPC 1.0 §7.11.1. The entry point.
//
// §7.11.1 says "the machine readable files associated with this specification represent the
// normative language bindings for C++". Those files (request_reply.h, function_call.h and
// sample.h of omg-dds-rpc-cxx.zip) are in `dds/rpc/normative/`, unchanged. They declare the
// classes of `dds::rpc`; this header makes them complete:
//
//   dds/rpc/base.hpp            the types (Duration, SampleIdentity, Sample, the headers of
//                               §7.5.1.1.1, the exceptions of §7.11.1.3, the abstract entities
//                               of DDS)
//   dds/rpc/core.hpp            the implementation classes (`dds::rpc::core`)
//   dds/rpc/vendor_dependent.h  what the OMG headers call vendor dependent
//   dds/rpc/request_reply.hpp   the OMG classes as handles on the implementation classes
//   dds/rpc/function_call.hpp   (the same, for the function-call style)
//
// and this file adds what the generated code of an `interface` needs and the OMG headers do
// not declare: `Service`, `Client`, `RPCRuntime` and the helpers that turn the declarations
// of an interface into params.

#ifndef ZERODDS_DDS_RPC_HPP
#define ZERODDS_DDS_RPC_HPP

#include <dds/rpc/core.hpp>
#include <dds/rpc/vendor_dependent.h>
#include <dds/rpc/normative/function_call.h>

namespace dds {
namespace rpc {

// What the implementation classes have beside the OMG classes, by its name in `dds::rpc`.
using core::BindingStyle;
using core::InterfaceDefaults;
using core::TopicNames;
using core::default_service_name;
using core::ready_future;
// The codecs of the two headers of §7.5.1.1.1, which the generated type support calls.
using core::decode_reply_header;
using core::decode_request_header;
using core::encode_reply_header;
using core::encode_request_header;

namespace vendor {

/// What a `Service` or `Client` attaches: an implementation class as it is, a class of the
/// application (the service of a base interface) by its implementation.
template <typename T>
auto unwrap(const T& value, int) -> decltype(*value.get_impl()) {
  return *value.get_impl();
}
template <typename T>
const T& unwrap(const T& value, long) {
  return value;
}

/// The state of a service: the implementation's endpoint with its protected members open to
/// the `Service` of the application.
struct ServiceState : core::ServiceEndpoint {
  ServiceState() = default;
  using core::ServiceEndpoint::attach;
  using core::ServiceEndpoint::on_close;
  using core::ServiceEndpoint::on_set_qos;
};

/// The state of a client.
struct ClientState : core::ClientEndpoint {
  ClientState() = default;
  using core::ClientEndpoint::attach;
  using core::ClientEndpoint::attach_base_proxy;
};

/// A client of a base interface (a `ServiceProxy` of the OMG header) as the implementation's
/// proxy.
template <typename TClient>
class ProxyAdapter : public core::ServiceProxy {
 public:
  explicit ProxyAdapter(std::shared_ptr<TClient> client) : client_(std::move(client)) {}
  void close() override { client_->close(); }
  bool is_null() const override { return client_->is_null(); }
  void bound(const std::string& instance_name) override { client_->bind(instance_name); }
  bool is_instance_discovered(const std::string& instance_name) const override {
    const auto found = client_->get_discoverd_service_instances();
    return std::find(found.begin(), found.end(), instance_name) != found.end();
  }
  std::vector<std::string> get_discoverd_service_instances() const override {
    return client_->get_discoverd_service_instances();
  }
  std::shared_ptr<core::ServiceProxy> share_reference() const override {
    return std::make_shared<ProxyAdapter>(client_);
  }

 private:
  std::shared_ptr<TClient> client_;
};

}  // namespace vendor

// -- the declarations of an interface, as params --------------------------------------------

/// The `Requester` params a `Client` calls through, under the declarations of its interface.
inline core::RequesterParams requester_params_of(const ClientParams& params,
                                            const InterfaceDefaults& defaults) {
  return core::requester_params_of(*params.get_impl(), defaults);
}

/// The `Replier` params a `Service` answers through.
inline core::ReplierParams replier_params_of(const ServiceParams& params,
                                       const InterfaceDefaults& defaults) {
  return core::replier_params_of(*params.get_impl(), defaults);
}

/// `requester_params_of` for an interface mapped by the Enhanced Service Mapping (§7.5.1.2).
inline core::RequesterParams enhanced_requester_params_of(const ClientParams& params,
                                                    const InterfaceDefaults& defaults) {
  return core::enhanced_requester_params_of(*params.get_impl(), defaults);
}

/// `replier_params_of` for an interface mapped by the Enhanced Service Mapping.
inline core::ReplierParams enhanced_replier_params_of(const ServiceParams& params,
                                                const InterfaceDefaults& defaults) {
  return core::enhanced_replier_params_of(*params.get_impl(), defaults);
}

/// The name a `Service` is discoverable under (§7.4.1, §7.4.2.3).
inline std::string service_name_of(const ServiceParams& params, const InterfaceDefaults& defaults) {
  return core::service_name_of(*params.get_impl(), defaults);
}

// -- what the params have beside the OMG classes --------------------------------------------

/// The wire encoding of the samples of a service or a client (default `XCDR1`, §7.6.3.1).
inline ServiceParams& wire_encoding(ServiceParams& params, WireEncoding encoding) {
  params.get_impl()->wire_encoding(encoding);
  return params;
}
inline ClientParams& wire_encoding(ClientParams& params, WireEncoding encoding) {
  params.get_impl()->wire_encoding(encoding);
  return params;
}
inline WireEncoding wire_encoding(const ServiceParams& params) {
  return params.get_impl()->wire_encoding();
}
inline WireEncoding wire_encoding(const ClientParams& params) {
  return params.get_impl()->wire_encoding();
}

// -- §7.11.2.2.3 RPCRuntime ------------------------------------------------------------------

/// The factory every other entity is obtained from.
///
/// §7.11.2.2.3 names it for Java — "RPCRuntime […] is to provide access to the RPCRuntime
/// singleton, which serves as a factory for all other entities" — and §7.11.2 says its sub
/// clauses describe what differs from C++, so the C++ binding has the same factory.
class RPCRuntime {
 public:
  explicit RPCRuntime(std::shared_ptr<DomainParticipant> participant)
      : participant_(std::move(participant)) {}

  const std::shared_ptr<DomainParticipant>& domain_participant() const { return participant_; }

  /// Params for a `Client`, on this runtime's participant.
  ClientParams client_params() const {
    ClientParams params;
    params.domain_participant(participant_);
    return params;
  }

  /// Params for a `Service`, on this runtime's participant.
  ServiceParams service_params() const {
    ServiceParams params;
    params.domain_participant(participant_);
    return params;
  }

  template <typename TReq, typename TRep>
  Requester<TReq, TRep> create_requester(const std::string& request_topic,
                                         const std::string& reply_topic) {
    RequesterParams params;
    params.request_topic_name(request_topic).reply_topic_name(reply_topic);
    return create_requester<TReq, TRep>(params);
  }

  /// A requester over `params`, on this runtime's participant unless the params name another.
  template <typename TReq, typename TRep>
  Requester<TReq, TRep> create_requester(RequesterParams params) {
    if (!params.domain_participant()) {
      params.domain_participant(participant_);
    }
    return Requester<TReq, TRep>(params);
  }

  template <typename TReq, typename TRep>
  Replier<TReq, TRep> create_replier(const std::string& request_topic,
                                     const std::string& reply_topic) {
    ReplierParams params;
    params.request_topic_name(request_topic).reply_topic_name(reply_topic);
    return create_replier<TReq, TRep>(params);
  }

  /// A replier over `params`, on this runtime's participant unless the params name another.
  template <typename TReq, typename TRep>
  Replier<TReq, TRep> create_replier(ReplierParams params) {
    if (!params.domain_participant()) {
      params.domain_participant(participant_);
    }
    return Replier<TReq, TRep>(params);
  }

 private:
  std::shared_ptr<DomainParticipant> participant_;
};

// -- §7.11.1.5.1 Service -----------------------------------------------------------------------

/// §7.11.1.5.1: "Service is a reference type and accepts a reference to the service
/// implementation. A Service instantiates the underlying DDS entities and makes the service
/// discoverable. Every Service inherits from ServiceEndpoint. A Service may belong to only one
/// Server."
///
/// The generated `<I>Service` of `crates/lang-rules/src/emit/interface_cpp.rs` is that class.
/// The OMG header does not say how a service gets into a server; `add(Server&, …)` does.
class Service : public ServiceEndpoint {
 public:
  virtual ~Service() {}
  /// Take the requests that have arrived and answer them. Returns how many were answered, so
  /// a `Server` can report progress.
  virtual std::size_t dispatch() = 0;
  /// The name the service is discoverable under. §7.4.1.
  virtual const std::string& service_name() const = 0;

  bool is_paused() const { return state().is_paused(); }
  /// §7.10.2: change the mutable QoS policies of the service's endpoints at run time.
  void set_qos(const RpcQos& qos) { state().set_qos(qos); }
  /// The QoS the service's endpoints have now.
  RpcQos qos() const { return state().qos(); }

 protected:
  Service() : ServiceEndpoint(std::make_shared<vendor::ServiceState>(), 0) {}

  /// The replier a service answers through: `close()` closes it, which deletes its DataReader
  /// and DataWriter (§7.9.3), and `set_qos()` changes its QoS.
  template <typename TReplier>
  void attach(const TReplier& replier) {
    state().attach(vendor::unwrap(replier, 0));
  }

  /// Register what `close()` has to release. Run once, at `close()` or when the last copy of
  /// the endpoint is gone.
  void on_close(std::function<void()> release) { state().on_close(std::move(release)); }

 private:
  vendor::ServiceState& state() const {
    return static_cast<vendor::ServiceState&>(*std::static_pointer_cast<core::ServiceEndpoint>(impl_));
  }
};

namespace vendor {

/// A `Service` of the application as the service the implementation's `Server` holds: it
/// shares the state of the service (paused, closed, owned).
class ServiceAdapter : public core::Service {
 public:
  explicit ServiceAdapter(std::shared_ptr<dds::rpc::Service> service)
      : core::Service(*std::static_pointer_cast<core::ServiceEndpoint>(service->get_impl())),
        service_(std::move(service)) {}
  std::size_t dispatch() override { return service_->dispatch(); }
  const std::string& service_name() const override { return service_->service_name(); }

 private:
  std::shared_ptr<dds::rpc::Service> service_;
};

}  // namespace vendor

// -- §7.11.1.5.3 Server ------------------------------------------------------------------------

/// Add one service to `server`. §7.11.1.5.1: "A Service may belong to only one Server": a
/// service that another server holds, or this one already does, is refused with
/// `IllegalOperationError`. A closed server takes none (`AlreadyClosedError`).
inline void add(Server& server, std::shared_ptr<Service> service) {
  if (!service) {
    throw NullReferenceError("the service is null");
  }
  server.get_impl()->add(std::make_shared<vendor::ServiceAdapter>(std::move(service)));
}

/// How many services `server` holds.
inline std::size_t size(const Server& server) { return server.get_impl()->size(); }

/// The defaults `server` gives the services it holds (§7.11.1.5.3 `ServerParams`).
inline ServiceParams default_service_params(const Server& server) {
  ServiceParams out;
  *out.get_impl() = server.get_impl()->default_service_params();
  return out;
}

/// Dispatch one round over every service of `server` that is not paused. Returns how many
/// requests were answered.
inline std::size_t run_once(Server& server) { return server.get_impl()->run_once(); }

// -- §7.11.1.5.4 Client ------------------------------------------------------------------------

/// §7.11.1.5.4: "Client is a reference type and provides functions to invoke operations on a
/// remote service synchronously and asynchronously. A Client instantiates the underlying DDS
/// entities and makes them discoverable. Every Client inherits publicly from '${interface}',
/// '${interface}Async', and ClientEndpoint (public virtual)."
///
/// The generated `<I>Client` of `crates/lang-rules/src/emit/interface_cpp.rs` is that class;
/// this is the third of its three bases, and it is `virtual` there for the reason the clause
/// gives.
class Client : public virtual ClientEndpoint {
 public:
  /// §7.10.2: change the mutable QoS policies of the client's endpoints, those of its base
  /// interfaces' clients included, at run time.
  void set_qos(const RpcQos& qos) { state().set_qos(qos); }
  using ServiceProxy::wait_for_service;
  /// Wait for a matching service for at most `timeout`; false when there is none by then.
  bool wait_for_service(std::chrono::milliseconds timeout) const {
    return state().wait_for_service(timeout);
  }
  /// The QoS the client's endpoints have now.
  RpcQos qos() const { return state().qos(); }

 protected:
  Client() { impl_ = std::make_shared<vendor::ClientState>(); }

  /// The requester the client calls through.
  template <typename TRequester>
  void attach(TRequester& requester) {
    state().attach(vendor::unwrap(requester, 0));
  }

  /// The client of a base interface, which reaches the base's operations on the base's own
  /// topics (§7.5.1.1.8). Closed, awaited and given QoS with this one.
  template <typename TClient>
  void attach_base(const TClient& base) {
    auto held = std::make_shared<TClient>(base);
    state().attach_base_proxy(std::make_shared<vendor::ProxyAdapter<TClient>>(held),
                              [held](const RpcQos& qos) { held->set_qos(qos); });
  }

 private:
  vendor::ClientState& state() const {
    return static_cast<vendor::ClientState&>(*std::static_pointer_cast<core::ClientEndpoint>(impl_));
  }
};

}  // namespace rpc
}  // namespace dds

#endif  // ZERODDS_DDS_RPC_HPP
