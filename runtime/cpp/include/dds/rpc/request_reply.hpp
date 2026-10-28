// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// request_reply.hpp — the vendor-dependent definitions of the classes that
// normative/request_reply.h (DDS-RPC 1.0 §7.11.1.4, omg-dds-rpc-cxx.zip) declares.
//
// The OMG header declares the classes and their members; the object a handle stands for is its
// `impl_`, here a pointer to the implementation class of `dds/rpc/core.hpp` (see
// `vendor_dependent.h`). A copy of a handle is another reference to the same object
// (§7.11.1.4.2: "RPCEntity is a reference type"); `Params` classes are values and copy deeply.

// The OMG header was read with `virtual` carrying a `public:` (normative/vendor_dependent.h).
#undef virtual

#ifndef ZERODDS_DDS_RPC_REQUEST_REPLY_HPP
#define ZERODDS_DDS_RPC_REQUEST_REPLY_HPP

#include <dds/rpc/vendor_dependent.h>

namespace dds {
namespace rpc {
namespace vendor {

/// The implementation object a handle points at, or `NullReferenceError`.
template <typename Core, typename Pointer>
Core& live(const Pointer& impl) {
  if (!impl) {
    throw NullReferenceError("the RPC entity is null");
  }
  return static_cast<Core&>(*impl);
}

/// A handle on an implementation object that exists already: the `impl_` is protected, so a
/// derived class sets it.
template <typename Handle, typename Impl>
struct Adopt : Handle {
  explicit Adopt(std::shared_ptr<Impl> impl) { this->impl_ = std::move(impl); }
};

// -- the listeners of the OMG header, as the implementation calls them ---------------------

template <typename TReq, typename TRep>
struct SimpleReplierAdapter : core::SimpleReplierListener<TReq, TRep> {
  explicit SimpleReplierAdapter(dds::rpc::SimpleReplierListener<TReq, TRep>* user) : user_(user) {}
  TRep* process_request(const Sample<TReq>& request, const dds::SampleIdentity& id) override {
    return user_->process_request(request, id);
  }
  dds::rpc::SimpleReplierListener<TReq, TRep>* user_;
};

template <typename TReq, typename TRep>
struct ReplierListenerAdapter : core::ReplierListener<TReq, TRep> {
  explicit ReplierListenerAdapter(dds::rpc::ReplierListener<TReq, TRep>* user) : user_(user) {}
  void on_request_available(core::Replier<TReq, TRep>& replier) override {
    Adopt<dds::rpc::Replier<TReq, TRep>, core::Replier<TReq, TRep>> handle(
        std::make_shared<core::Replier<TReq, TRep>>(replier));
    user_->on_request_available(handle);
  }
  dds::rpc::ReplierListener<TReq, TRep>* user_;
};

template <typename TRep>
struct SimpleRequesterAdapter : core::SimpleRequesterListener<TRep> {
  explicit SimpleRequesterAdapter(dds::rpc::SimpleRequesterListener<TRep>* user) : user_(user) {}
  void process_reply(const Sample<TRep>& reply, const dds::SampleIdentity& id) override {
    user_->process_reply(reply, id);
  }
  dds::rpc::SimpleRequesterListener<TRep>* user_;
};

template <typename TReq, typename TRep>
struct RequesterListenerAdapter : core::RequesterListener<TReq, TRep> {
  explicit RequesterListenerAdapter(dds::rpc::RequesterListener<TReq, TRep>* user) : user_(user) {}
  void on_reply_available(core::Requester<TReq, TRep>& requester) override {
    Adopt<dds::rpc::Requester<TReq, TRep>, core::Requester<TReq, TRep>> handle(
        std::make_shared<core::Requester<TReq, TRep>>(requester));
    user_->on_reply_available(handle);
  }
  dds::rpc::RequesterListener<TReq, TRep>* user_;
};

}  // namespace vendor

// -- RPCEntity -----------------------------------------------------------------------------

inline RPCEntity::RPCEntity() {}
inline RPCEntity::RPCEntity(const RPCEntity& other) : impl_(other.impl_) {}
template <class Impl>
RPCEntity::RPCEntity(Impl impl, int) : impl_(std::move(impl)) {}
inline bool RPCEntity::operator==(const RPCEntity& other) { return impl_ == other.impl_; }
inline void RPCEntity::close() {
  if (impl_) {
    impl_->close();
  }
}
inline bool RPCEntity::is_null() const { return !impl_ || impl_->is_null(); }
inline RPCEntity::VendorDependent RPCEntity::get_impl() const { return impl_; }

// -- ServiceProxy --------------------------------------------------------------------------

inline ServiceProxy::ServiceProxy() {}
inline ServiceProxy::ServiceProxy(const ServiceProxy& other) : RPCEntity(other) {}
template <class Impl>
ServiceProxy::ServiceProxy(Impl impl, int) : RPCEntity(std::move(impl), 0) {}

inline void ServiceProxy::bind(const std::string& instance_name) {
  vendor::live<core::ServiceProxy>(impl_).bind(instance_name);
}
inline void ServiceProxy::unbind() { vendor::live<core::ServiceProxy>(impl_).unbind(); }
inline bool ServiceProxy::is_bound() const {
  return vendor::live<core::ServiceProxy>(impl_).is_bound();
}
inline std::string ServiceProxy::get_bound_instance_name() const {
  return vendor::live<core::ServiceProxy>(impl_).get_bound_instance_name();
}
inline std::vector<std::string> ServiceProxy::get_discoverd_service_instances() const {
  return vendor::live<core::ServiceProxy>(impl_).get_discoverd_service_instances();
}
inline void ServiceProxy::wait_for_service() {
  vendor::live<core::ServiceProxy>(impl_).wait_for_service();
}
inline void ServiceProxy::wait_for_service(const dds::Duration& max_wait) {
  vendor::live<core::ServiceProxy>(impl_).wait_for_service(max_wait);
}
inline void ServiceProxy::wait_for_service(std::string instance_name) {
  vendor::live<core::ServiceProxy>(impl_).wait_for_service(std::move(instance_name));
}
inline void ServiceProxy::wait_for_service(const dds::Duration& max_wait,
                                           std::string instance_name) {
  vendor::live<core::ServiceProxy>(impl_).wait_for_service(max_wait, std::move(instance_name));
}
inline void ServiceProxy::wait_for_services(int count) {
  vendor::live<core::ServiceProxy>(impl_).wait_for_services(count);
}
inline void ServiceProxy::wait_for_services(const dds::Duration& max_wait, int count) {
  vendor::live<core::ServiceProxy>(impl_).wait_for_services(max_wait, count);
}
inline void ServiceProxy::wait_for_services(const std::vector<std::string>& instance_names) {
  vendor::live<core::ServiceProxy>(impl_).wait_for_services(instance_names);
}
inline void ServiceProxy::wait_for_services(const dds::Duration& max_wait,
                                            const std::vector<std::string>& instance_names) {
  vendor::live<core::ServiceProxy>(impl_).wait_for_services(max_wait, instance_names);
}
inline future<void> ServiceProxy::wait_for_service_async() {
  return vendor::live<core::ServiceProxy>(impl_).wait_for_service_async();
}
inline future<void> ServiceProxy::wait_for_service_async(std::string instance_name) {
  return vendor::live<core::ServiceProxy>(impl_).wait_for_service_async(std::move(instance_name));
}
inline future<void> ServiceProxy::wait_for_services_async(int count) {
  return vendor::live<core::ServiceProxy>(impl_).wait_for_services_async(count);
}
inline future<void> ServiceProxy::wait_for_services_async(
    const std::vector<std::string>& instance_names) {
  return vendor::live<core::ServiceProxy>(impl_).wait_for_services_async(instance_names);
}

// -- RequesterParams -----------------------------------------------------------------------

inline RequesterParams::RequesterParams() : impl_(std::make_shared<vendor::RequesterParamsImpl>()) {}
inline RequesterParams::RequesterParams(const RequesterParams& other)
    : impl_(std::make_shared<vendor::RequesterParamsImpl>(*other.impl_)) {}
inline RequesterParams& RequesterParams::operator=(const RequesterParams& that) {
  if (this != &that) {
    *impl_ = *that.impl_;
  }
  return *this;
}

template <class TRep>
RequesterParams& RequesterParams::simple_requester_listener(SimpleRequesterListener<TRep>* listener) {
  auto adapter = std::make_shared<vendor::SimpleRequesterAdapter<TRep>>(listener);
  impl_->core::RequesterParams::simple_requester_listener(
      static_cast<core::SimpleRequesterListener<TRep>*>(adapter.get()));
  impl_->adapter = adapter;
  impl_->user_simple = listener;
  return *this;
}
template <class TReq, class TRep>
RequesterParams& RequesterParams::requester_listener(RequesterListener<TReq, TRep>* listener) {
  auto adapter = std::make_shared<vendor::RequesterListenerAdapter<TReq, TRep>>(listener);
  impl_->core::RequesterParams::requester_listener(
      static_cast<core::RequesterListener<TReq, TRep>*>(adapter.get()));
  impl_->adapter = adapter;
  impl_->user_full = listener;
  return *this;
}
inline RequesterParams& RequesterParams::domain_participant(
    dds_entity_traits::DomainParticipant participant) {
  impl_->domain_participant(std::move(participant));
  return *this;
}
inline RequesterParams& RequesterParams::publisher(dds_entity_traits::Publisher publisher) {
  impl_->publisher(std::move(publisher));
  return *this;
}
inline RequesterParams& RequesterParams::subscriber(dds_entity_traits::Subscriber subscriber) {
  impl_->subscriber(std::move(subscriber));
  return *this;
}
inline RequesterParams& RequesterParams::datawriter_qos(dds_entity_traits::DataWriterQos qos) {
  impl_->datawriter_qos(std::move(qos));
  return *this;
}
inline RequesterParams& RequesterParams::datareader_qos(dds_entity_traits::DataReaderQos qos) {
  impl_->datareader_qos(std::move(qos));
  return *this;
}
inline RequesterParams& RequesterParams::service_name(const std::string& name) {
  impl_->service_name(name);
  return *this;
}
inline RequesterParams& RequesterParams::request_topic_name(const std::string& name) {
  impl_->request_topic_name(name);
  return *this;
}
inline RequesterParams& RequesterParams::reply_topic_name(const std::string& name) {
  impl_->reply_topic_name(name);
  return *this;
}
inline dds_entity_traits::DomainParticipant RequesterParams::domain_participant() const {
  return impl_->domain_participant();
}
inline dds_entity_traits::Publisher RequesterParams::publisher() const {
  return impl_->publisher();
}
inline dds_entity_traits::Subscriber RequesterParams::subscriber() const {
  return impl_->subscriber();
}
inline dds_entity_traits::DataWriterQos RequesterParams::datawriter_qos() const {
  return impl_->datawriter_qos();
}
inline dds_entity_traits::DataReaderQos RequesterParams::datareader_qos() const {
  return impl_->datareader_qos();
}
inline ListenerBase* RequesterParams::simple_requester_listener() const {
  return impl_->user_simple;
}
inline ListenerBase* RequesterParams::requester_listener() const { return impl_->user_full; }
inline std::string RequesterParams::service_name() const { return impl_->service_name(); }
inline std::string RequesterParams::request_topic_name() const {
  return impl_->request_topic_name();
}
inline std::string RequesterParams::reply_topic_name() const { return impl_->reply_topic_name(); }
inline RequesterParams::VendorDependent RequesterParams::get_impl() { return impl_; }

// -- ReplierParams -------------------------------------------------------------------------

inline ReplierParams::ReplierParams() : impl_(std::make_shared<vendor::ReplierParamsImpl>()) {}
inline ReplierParams::ReplierParams(const ReplierParams& other)
    : impl_(std::make_shared<vendor::ReplierParamsImpl>(*other.impl_)) {}
inline ReplierParams& ReplierParams::operator=(const ReplierParams& that) {
  if (this != &that) {
    *impl_ = *that.impl_;
  }
  return *this;
}

template <class TReq, class TRep>
ReplierParams& ReplierParams::simple_replier_listener(SimpleReplierListener<TReq, TRep>* listener) {
  auto adapter = std::make_shared<vendor::SimpleReplierAdapter<TReq, TRep>>(listener);
  impl_->core::ReplierParams::simple_replier_listener(
      static_cast<core::SimpleReplierListener<TReq, TRep>*>(adapter.get()));
  impl_->adapter = adapter;
  impl_->user_simple = listener;
  return *this;
}
template <class TReq, class TRep>
ReplierParams& ReplierParams::replier_listener(ReplierListener<TReq, TRep>* listener) {
  auto adapter = std::make_shared<vendor::ReplierListenerAdapter<TReq, TRep>>(listener);
  impl_->core::ReplierParams::replier_listener(
      static_cast<core::ReplierListener<TReq, TRep>*>(adapter.get()));
  impl_->adapter = adapter;
  impl_->user_full = listener;
  return *this;
}
inline ReplierParams& ReplierParams::domain_participant(
    dds_entity_traits::DomainParticipant participant) {
  impl_->domain_participant(std::move(participant));
  return *this;
}
inline ReplierParams& ReplierParams::service_name(const std::string& service_name) {
  impl_->service_name(service_name);
  return *this;
}
inline ReplierParams& ReplierParams::instance_name(const std::string& service_name) {
  impl_->instance_name(service_name);
  return *this;
}
inline ReplierParams& ReplierParams::request_topic_name(const std::string& req_topic) {
  impl_->request_topic_name(req_topic);
  return *this;
}
inline ReplierParams& ReplierParams::reply_topic_name(const std::string& rep_topic) {
  impl_->reply_topic_name(rep_topic);
  return *this;
}
inline ReplierParams& ReplierParams::datawriter_qos(dds_entity_traits::DataWriterQos qos) {
  impl_->datawriter_qos(std::move(qos));
  return *this;
}
inline ReplierParams& ReplierParams::datareader_qos(dds_entity_traits::DataReaderQos qos) {
  impl_->datareader_qos(std::move(qos));
  return *this;
}
inline ReplierParams& ReplierParams::publisher(dds_entity_traits::Publisher publisher) {
  impl_->publisher(std::move(publisher));
  return *this;
}
inline ReplierParams& ReplierParams::subscriber(dds_entity_traits::Subscriber subscriber) {
  impl_->subscriber(std::move(subscriber));
  return *this;
}
inline dds_entity_traits::DomainParticipant ReplierParams::domain_participant() const {
  return impl_->domain_participant();
}
inline ListenerBase* ReplierParams::simple_replier_listener() const { return impl_->user_simple; }
inline ListenerBase* ReplierParams::replier_listener() const { return impl_->user_full; }
inline std::string ReplierParams::service_name() const { return impl_->service_name(); }
inline std::string ReplierParams::instance_name() const { return impl_->instance_name(); }
inline std::string ReplierParams::request_topic_name() const { return impl_->request_topic_name(); }
inline std::string ReplierParams::reply_topic_name() const { return impl_->reply_topic_name(); }
inline dds_entity_traits::DataWriterQos ReplierParams::datawriter_qos() const {
  return impl_->datawriter_qos();
}
inline dds_entity_traits::DataReaderQos ReplierParams::datareader_qos() const {
  return impl_->datareader_qos();
}
inline dds_entity_traits::Publisher ReplierParams::publisher() const { return impl_->publisher(); }
inline dds_entity_traits::Subscriber ReplierParams::subscriber() const {
  return impl_->subscriber();
}
inline ReplierParams::VendorDependent ReplierParams::get_impl() { return impl_; }

// -- the listeners -------------------------------------------------------------------------

inline ListenerBase::~ListenerBase() {}
template <class TReq, class TRep>
SimpleReplierListener<TReq, TRep>::~SimpleReplierListener() {}
template <class TReq, class TRep>
ReplierListener<TReq, TRep>::~ReplierListener() {}
template <class TRep>
SimpleRequesterListener<TRep>::~SimpleRequesterListener() {}
template <class TReq, class TRep>
RequesterListener<TReq, TRep>::~RequesterListener() {}

// -- Requester -----------------------------------------------------------------------------

namespace vendor {
template <typename TReq, typename TRep>
core::Requester<TReq, TRep>& requester_of(const std::shared_ptr<core::RPCEntity>& impl) {
  return live<core::Requester<TReq, TRep>>(impl);
}
/// The params of the OMG header for params of the implementation.
inline RequesterParams params_of(const core::RequesterParams& core_params) {
  RequesterParams out;
  static_cast<core::RequesterParams&>(*out.get_impl()) = core_params;
  return out;
}
inline ReplierParams params_of(const core::ReplierParams& core_params) {
  ReplierParams out;
  static_cast<core::ReplierParams&>(*out.get_impl()) = core_params;
  return out;
}
}  // namespace vendor

template <typename TReq, typename TRep>
Requester<TReq, TRep>::Requester() {}

template <typename TReq, typename TRep>
Requester<TReq, TRep>::Requester(const RequesterParams& params)
    : ServiceProxy(std::make_shared<core::Requester<TReq, TRep>>(
                       static_cast<const core::RequesterParams&>(
                           *const_cast<RequesterParams&>(params).get_impl())),
                   0) {}

template <typename TReq, typename TRep>
Requester<TReq, TRep>::Requester(const Requester& other) : ServiceProxy(other) {}

template <typename TReq, typename TRep>
void Requester<TReq, TRep>::swap(Requester& other) {
  std::swap(impl_, other.impl_);
}

template <typename TReq, typename TRep>
Requester<TReq, TRep>& Requester<TReq, TRep>::operator=(const Requester& other) {
  impl_ = other.impl_;
  return *this;
}

template <typename TReq, typename TRep>
Requester<TReq, TRep>::~Requester() {}

template <typename TReq, typename TRep>
void Requester<TReq, TRep>::send_request(WriteSample<TReq>& request) {
  vendor::requester_of<TReq, TRep>(impl_).send_request(request);
}
template <typename TReq, typename TRep>
void Requester<TReq, TRep>::send_request(WriteSampleRef<TReq>& wsref) {
  vendor::requester_of<TReq, TRep>(impl_).send_request(wsref);
}
template <typename TReq, typename TRep>
future<dds::Sample<TRep>> Requester<TReq, TRep>::send_request_async(const TReq& request) {
  return vendor::requester_of<TReq, TRep>(impl_).send_request_async(request);
}
template <typename TReq, typename TRep>
void Requester<TReq, TRep>::send_request(TReq& request) {
  (void)vendor::requester_of<TReq, TRep>(impl_).send_request(static_cast<const TReq&>(request));
}
template <typename TReq, typename TRep>
void Requester<TReq, TRep>::send_request_oneway(TReq& request) {
  vendor::requester_of<TReq, TRep>(impl_).send_request_oneway(static_cast<const TReq&>(request));
}
template <typename TReq, typename TRep>
void Requester<TReq, TRep>::send_request(const TReq& request) {
  (void)vendor::requester_of<TReq, TRep>(impl_).send_request(request);
}
template <typename TReq, typename TRep>
void Requester<TReq, TRep>::send_request_oneway(const TReq& request) {
  vendor::requester_of<TReq, TRep>(impl_).send_request_oneway(request);
}

template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::receive_reply(Sample<TRep>& reply, const dds::Duration& timeout) {
  return vendor::requester_of<TReq, TRep>(impl_).receive_reply(reply, timeout);
}
template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::receive_reply(SampleRef<TRep> reply, const dds::Duration& timeout) {
  return vendor::requester_of<TReq, TRep>(impl_).receive_reply(reply, timeout);
}
template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::receive_reply(Sample<TRep>& reply,
                                          const dds::SampleIdentity& related_request_id,
                                          const dds::Duration& timeout) {
  return vendor::requester_of<TReq, TRep>(impl_).receive_reply(reply, related_request_id, timeout);
}
template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::receive_reply(SampleRef<TRep> reply,
                                          const dds::SampleIdentity& related_request_id,
                                          const dds::Duration& timeout) {
  return vendor::requester_of<TReq, TRep>(impl_).receive_reply(reply, related_request_id, timeout);
}
template <typename TReq, typename TRep>
LoanedSamples<TRep> Requester<TReq, TRep>::receive_replies(const dds::Duration& max_wait) {
  return vendor::requester_of<TReq, TRep>(impl_).receive_replies(max_wait);
}
template <typename TReq, typename TRep>
LoanedSamples<TRep> Requester<TReq, TRep>::receive_replies(int min_count, int max_count,
                                                           const dds::Duration& max_wait) {
  return vendor::requester_of<TReq, TRep>(impl_).receive_replies(min_count, max_count, max_wait);
}
template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::wait_for_replies(int min_count, const dds::Duration& max_wait) {
  return vendor::requester_of<TReq, TRep>(impl_).wait_for_replies(min_count, max_wait);
}
template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::wait_for_replies(const dds::Duration& max_wait) {
  return vendor::requester_of<TReq, TRep>(impl_).wait_for_replies(max_wait);
}
template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::wait_for_replies(int min_count, const dds::Duration& max_wait,
                                             const dds::SampleIdentity& related_request_id) {
  return vendor::requester_of<TReq, TRep>(impl_).wait_for_replies(min_count, max_wait,
                                                                  related_request_id);
}
template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::take_reply(Sample<TRep>& reply) {
  return vendor::requester_of<TReq, TRep>(impl_).take_reply(reply);
}
template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::take_reply(SampleRef<TRep> reply) {
  return vendor::requester_of<TReq, TRep>(impl_).take_reply(reply);
}
template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::take_reply(Sample<TRep>& reply,
                                       const dds::SampleIdentity& related_request_id) {
  return vendor::requester_of<TReq, TRep>(impl_).take_reply(reply, related_request_id);
}
template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::take_reply(SampleRef<TRep> reply,
                                       const dds::SampleIdentity& related_request_id) {
  return vendor::requester_of<TReq, TRep>(impl_).take_reply(reply, related_request_id);
}
template <typename TReq, typename TRep>
LoanedSamples<TRep> Requester<TReq, TRep>::take_replies(int max_count) {
  return vendor::requester_of<TReq, TRep>(impl_).take_replies(max_count);
}
template <typename TReq, typename TRep>
LoanedSamples<TRep> Requester<TReq, TRep>::take_replies(
    int max_count, const dds::SampleIdentity& related_request_id) {
  return vendor::requester_of<TReq, TRep>(impl_).take_replies(max_count, related_request_id);
}
template <typename TReq, typename TRep>
LoanedSamples<TRep> Requester<TReq, TRep>::take_replies(
    const dds::SampleIdentity& related_request_id) {
  return vendor::requester_of<TReq, TRep>(impl_).take_replies(related_request_id);
}
template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::read_reply(Sample<TRep>& reply) {
  return vendor::requester_of<TReq, TRep>(impl_).read_reply(reply);
}
template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::read_reply(SampleRef<TRep> reply) {
  return vendor::requester_of<TReq, TRep>(impl_).read_reply(reply);
}
template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::read_reply(Sample<TRep>& reply,
                                       const dds::SampleIdentity& related_request_id) {
  return vendor::requester_of<TReq, TRep>(impl_).read_reply(reply, related_request_id);
}
template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::read_reply(SampleRef<TRep> reply,
                                       const dds::SampleIdentity& related_request_id) {
  return vendor::requester_of<TReq, TRep>(impl_).read_reply(reply, related_request_id);
}
template <typename TReq, typename TRep>
LoanedSamples<TRep> Requester<TReq, TRep>::read_replies(int max_count) {
  return vendor::requester_of<TReq, TRep>(impl_).read_replies(max_count);
}
template <typename TReq, typename TRep>
LoanedSamples<TRep> Requester<TReq, TRep>::read_replies(
    int max_count, const dds::SampleIdentity& related_request_id) {
  return vendor::requester_of<TReq, TRep>(impl_).read_replies(max_count, related_request_id);
}
template <typename TReq, typename TRep>
LoanedSamples<TRep> Requester<TReq, TRep>::read_replies(
    const dds::SampleIdentity& related_request_id) {
  return vendor::requester_of<TReq, TRep>(impl_).read_replies(related_request_id);
}
template <typename TReq, typename TRep>
bool Requester<TReq, TRep>::receive_nondata_samples(bool enable) {
  return vendor::requester_of<TReq, TRep>(impl_).receive_nondata_samples(enable);
}
template <typename TReq, typename TRep>
RequesterParams Requester<TReq, TRep>::get_requester_params() const {
  return vendor::params_of(vendor::requester_of<TReq, TRep>(impl_).get_requester_params());
}
template <typename TReq, typename TRep>
typename Requester<TReq, TRep>::RequestDataWriter Requester<TReq, TRep>::get_request_datawriter()
    const {
  return vendor::requester_of<TReq, TRep>(impl_).get_request_datawriter();
}
template <typename TReq, typename TRep>
typename Requester<TReq, TRep>::ReplyDataReader Requester<TReq, TRep>::get_reply_datareader()
    const {
  return vendor::requester_of<TReq, TRep>(impl_).get_reply_datareader();
}
template <typename TReq, typename TRep>
typename Requester<TReq, TRep>::VendorDependent Requester<TReq, TRep>::get_impl() {
  return std::static_pointer_cast<core::Requester<TReq, TRep>>(impl_);
}

// -- Replier -------------------------------------------------------------------------------

namespace vendor {
template <typename TReq, typename TRep>
core::Replier<TReq, TRep>& replier_of(const std::shared_ptr<core::RPCEntity>& impl) {
  return live<core::Replier<TReq, TRep>>(impl);
}
}  // namespace vendor

template <typename TReq, typename TRep>
Replier<TReq, TRep>::Replier() {}

template <typename TReq, typename TRep>
Replier<TReq, TRep>::Replier(const Replier& other) : RPCEntity(other) {}

template <typename TReq, typename TRep>
Replier<TReq, TRep>& Replier<TReq, TRep>::operator=(const Replier& other) {
  impl_ = other.impl_;
  return *this;
}

template <typename TReq, typename TRep>
Replier<TReq, TRep>::Replier(const ReplierParams& params)
    : RPCEntity(std::make_shared<core::Replier<TReq, TRep>>(
                    static_cast<const core::ReplierParams&>(
                        *const_cast<ReplierParams&>(params).get_impl())),
                0) {}

template <typename TReq, typename TRep>
Replier<TReq, TRep>::~Replier() {}

template <typename TReq, typename TRep>
void Replier<TReq, TRep>::swap(Replier& other) {
  std::swap(impl_, other.impl_);
}

template <typename TReq, typename TRep>
void Replier<TReq, TRep>::send_reply(WriteSample<TRep>& reply,
                                     const dds::SampleIdentity& related_request_id) {
  vendor::replier_of<TReq, TRep>(impl_).send_reply(reply, related_request_id);
}
template <typename TReq, typename TRep>
void Replier<TReq, TRep>::send_reply(WriteSampleRef<TRep>& reply,
                                     const dds::SampleIdentity& related_request_id) {
  vendor::replier_of<TReq, TRep>(impl_).send_reply(reply, related_request_id);
}
template <typename TReq, typename TRep>
void Replier<TReq, TRep>::send_reply(TRep& reply, const dds::SampleIdentity& related_request_id) {
  vendor::replier_of<TReq, TRep>(impl_).send_reply(static_cast<const TRep&>(reply),
                                                   related_request_id);
}
template <typename TReq, typename TRep>
void Replier<TReq, TRep>::send_reply(const TRep& reply,
                                     const dds::SampleIdentity& related_request_id) {
  vendor::replier_of<TReq, TRep>(impl_).send_reply(reply, related_request_id);
}
template <typename TReq, typename TRep>
bool Replier<TReq, TRep>::receive_request(Sample<TReq>& request, const dds::Duration& max_wait) {
  return vendor::replier_of<TReq, TRep>(impl_).receive_request(request, max_wait);
}
template <typename TReq, typename TRep>
bool Replier<TReq, TRep>::receive_request(SampleRef<TReq> request, const dds::Duration& max_wait) {
  return vendor::replier_of<TReq, TRep>(impl_).receive_request(request, max_wait);
}
template <typename TReq, typename TRep>
typename Replier<TReq, TRep>::LoanedSamplesType Replier<TReq, TRep>::receive_requests(
    const dds::Duration& max_wait) {
  return vendor::replier_of<TReq, TRep>(impl_).receive_requests(max_wait);
}
template <typename TReq, typename TRep>
typename Replier<TReq, TRep>::LoanedSamplesType Replier<TReq, TRep>::receive_requests(
    int min_request_count, int max_request_count, const dds::Duration& max_wait) {
  return vendor::replier_of<TReq, TRep>(impl_).receive_requests(min_request_count,
                                                                max_request_count, max_wait);
}
template <typename TReq, typename TRep>
bool Replier<TReq, TRep>::wait_for_requests(const dds::Duration& max_wait) {
  return vendor::replier_of<TReq, TRep>(impl_).wait_for_requests(max_wait);
}
template <typename TReq, typename TRep>
bool Replier<TReq, TRep>::wait_for_requests(int min_count, const dds::Duration& max_wait) {
  return vendor::replier_of<TReq, TRep>(impl_).wait_for_requests(min_count, max_wait);
}
template <typename TReq, typename TRep>
bool Replier<TReq, TRep>::take_request(Sample<TReq>& request) {
  return vendor::replier_of<TReq, TRep>(impl_).take_request(request);
}
template <typename TReq, typename TRep>
bool Replier<TReq, TRep>::take_request(SampleRef<TReq> request) {
  return vendor::replier_of<TReq, TRep>(impl_).take_request(request);
}
template <typename TReq, typename TRep>
typename Replier<TReq, TRep>::LoanedSamplesType Replier<TReq, TRep>::take_requests(
    int max_samples) {
  return vendor::replier_of<TReq, TRep>(impl_).take_requests(max_samples);
}
template <typename TReq, typename TRep>
bool Replier<TReq, TRep>::read_request(Sample<TReq>& request) {
  return vendor::replier_of<TReq, TRep>(impl_).read_request(request);
}
template <typename TReq, typename TRep>
bool Replier<TReq, TRep>::read_request(SampleRef<TReq> request) {
  return vendor::replier_of<TReq, TRep>(impl_).read_request(request);
}
template <typename TReq, typename TRep>
typename Replier<TReq, TRep>::LoanedSamplesType Replier<TReq, TRep>::read_requests(
    int max_samples) {
  return vendor::replier_of<TReq, TRep>(impl_).read_requests(max_samples);
}
template <typename TReq, typename TRep>
const ReplierParams& Replier<TReq, TRep>::get_replier_params() const {
  // The OMG header returns a reference: the params of this handle are kept with it.
  static thread_local ReplierParams copy;
  copy = vendor::params_of(vendor::replier_of<TReq, TRep>(impl_).get_replier_params());
  return copy;
}
template <typename TReq, typename TRep>
bool Replier<TReq, TRep>::receive_nondata_samples(bool enable) {
  return vendor::replier_of<TReq, TRep>(impl_).receive_nondata_samples(enable);
}
template <typename TReq, typename TRep>
typename Replier<TReq, TRep>::RequestDataReader Replier<TReq, TRep>::get_request_datareader()
    const {
  return vendor::replier_of<TReq, TRep>(impl_).get_request_datareader();
}
template <typename TReq, typename TRep>
typename Replier<TReq, TRep>::ReplyDataWriter Replier<TReq, TRep>::get_reply_datawriter() const {
  return vendor::replier_of<TReq, TRep>(impl_).get_reply_datawriter();
}
template <typename TReq, typename TRep>
typename Replier<TReq, TRep>::VendorDependent Replier<TReq, TRep>::get_impl() const {
  return std::static_pointer_cast<core::Replier<TReq, TRep>>(impl_);
}

}  // namespace rpc
}  // namespace dds

#endif  // ZERODDS_DDS_RPC_REQUEST_REPLY_HPP
