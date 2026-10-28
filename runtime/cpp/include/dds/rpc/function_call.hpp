// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// function_call.hpp — the vendor-dependent definitions of the classes that
// normative/function_call.h (DDS-RPC 1.0 §7.11.1.5, omg-dds-rpc-cxx.zip) declares. See
// `request_reply.hpp`.

// The OMG header was read with `virtual` carrying a `public:` (normative/vendor_dependent.h).
#undef virtual

#ifndef ZERODDS_DDS_RPC_FUNCTION_CALL_HPP
#define ZERODDS_DDS_RPC_FUNCTION_CALL_HPP

#include <dds/rpc/request_reply.hpp>

namespace dds {
namespace rpc {

// -- Exception -----------------------------------------------------------------------------

inline Exception::Exception() {}
inline Exception::Exception(const char* msg) : message_(msg == nullptr ? "" : msg) {}
inline Exception::~Exception() throw() {}

// -- ServiceParams -------------------------------------------------------------------------

inline ServiceParams::ServiceParams() : impl_(std::make_shared<core::ServiceParams>()) {}
inline ServiceParams::ServiceParams(const ServiceParams& other)
    : impl_(std::make_shared<core::ServiceParams>(*other.impl_)) {}
inline ServiceParams& ServiceParams::operator=(const ServiceParams& that) {
  if (this != &that) {
    *impl_ = *that.impl_;
  }
  return *this;
}
inline ServiceParams& ServiceParams::service_name(const std::string& service_name) {
  impl_->service_name(service_name);
  return *this;
}
inline ServiceParams& ServiceParams::instance_name(const std::string& instance_name) {
  impl_->instance_name(instance_name);
  return *this;
}
inline ServiceParams& ServiceParams::request_topic_name(const std::string& req_topic) {
  impl_->request_topic_name(req_topic);
  return *this;
}
inline ServiceParams& ServiceParams::reply_topic_name(const std::string& rep_topic) {
  impl_->reply_topic_name(rep_topic);
  return *this;
}
inline ServiceParams& ServiceParams::datawriter_qos(dds_entity_traits::DataWriterQos qos) {
  impl_->datawriter_qos(std::move(qos));
  return *this;
}
inline ServiceParams& ServiceParams::datareader_qos(dds_entity_traits::DataReaderQos qos) {
  impl_->datareader_qos(std::move(qos));
  return *this;
}
inline ServiceParams& ServiceParams::publisher(dds_entity_traits::Publisher publisher) {
  impl_->publisher(std::move(publisher));
  return *this;
}
inline ServiceParams& ServiceParams::subscriber(dds_entity_traits::Subscriber subscriber) {
  impl_->subscriber(std::move(subscriber));
  return *this;
}
inline ServiceParams& ServiceParams::domain_participant(
    dds_entity_traits::DomainParticipant part) {
  impl_->domain_participant(std::move(part));
  return *this;
}
inline std::string ServiceParams::service_name() const { return impl_->service_name(); }
inline std::string ServiceParams::instance_name() const { return impl_->instance_name(); }
inline std::string ServiceParams::request_topic_name() const { return impl_->request_topic_name(); }
inline std::string ServiceParams::reply_topic_name() const { return impl_->reply_topic_name(); }
inline dds_entity_traits::DataWriterQos ServiceParams::datawriter_qos() const {
  return impl_->datawriter_qos();
}
inline dds_entity_traits::DataReaderQos ServiceParams::datareader_qos() const {
  return impl_->datareader_qos();
}
inline dds_entity_traits::Publisher ServiceParams::publisher() const { return impl_->publisher(); }
inline dds_entity_traits::Subscriber ServiceParams::subscriber() const {
  return impl_->subscriber();
}
inline dds_entity_traits::DomainParticipant ServiceParams::domain_participant() const {
  return impl_->domain_participant();
}
inline ServiceParams::VendorDependent ServiceParams::get_impl() const { return impl_; }

// -- ClientParams --------------------------------------------------------------------------

inline ClientParams::ClientParams() : impl_(std::make_shared<core::ClientParams>()) {}
inline ClientParams::ClientParams(const ClientParams& other)
    : impl_(std::make_shared<core::ClientParams>(*other.impl_)) {}
inline ClientParams& ClientParams::operator=(const ClientParams& that) {
  if (this != &that) {
    *impl_ = *that.impl_;
  }
  return *this;
}
inline ClientParams& ClientParams::service_name(const std::string& service_name) {
  impl_->service_name(service_name);
  return *this;
}
inline ClientParams& ClientParams::instance_name(const std::string& instance_name) {
  impl_->instance_name(instance_name);
  return *this;
}
inline ClientParams& ClientParams::request_topic_name(const std::string& req_topic) {
  impl_->request_topic_name(req_topic);
  return *this;
}
inline ClientParams& ClientParams::reply_topic_name(const std::string& rep_topic) {
  impl_->reply_topic_name(rep_topic);
  return *this;
}
inline ClientParams& ClientParams::datawriter_qos(const dds_entity_traits::DataWriterQos& qos) {
  impl_->datawriter_qos(qos);
  return *this;
}
inline ClientParams& ClientParams::datareader_qos(const dds_entity_traits::DataReaderQos& qos) {
  impl_->datareader_qos(qos);
  return *this;
}
inline ClientParams& ClientParams::publisher(dds_entity_traits::Publisher publisher) {
  impl_->publisher(std::move(publisher));
  return *this;
}
inline ClientParams& ClientParams::subscriber(dds_entity_traits::Subscriber subscriber) {
  impl_->subscriber(std::move(subscriber));
  return *this;
}
inline ClientParams& ClientParams::domain_participant(dds_entity_traits::DomainParticipant part) {
  impl_->domain_participant(std::move(part));
  return *this;
}
inline const std::string& ClientParams::service_name() const { return impl_->service_name(); }
inline const std::string& ClientParams::instance_name() const { return impl_->instance_name(); }
inline const std::string& ClientParams::request_topic_name() const {
  return impl_->request_topic_name();
}
inline const std::string& ClientParams::reply_topic_name() const {
  return impl_->reply_topic_name();
}
inline const dds_entity_traits::DataWriterQos& ClientParams::datawriter_qos() const {
  return impl_->datawriter_qos();
}
inline const dds_entity_traits::DataReaderQos& ClientParams::datareader_qos() const {
  return impl_->datareader_qos();
}
inline dds_entity_traits::Publisher ClientParams::publisher() const { return impl_->publisher(); }
inline dds_entity_traits::Subscriber ClientParams::subscriber() const {
  return impl_->subscriber();
}
inline dds_entity_traits::DomainParticipant ClientParams::domain_participant() const {
  return impl_->domain_participant();
}
inline ClientParams::VendorDependent ClientParams::get_impl() const { return impl_; }

// -- ServerParams --------------------------------------------------------------------------

inline ServerParams::ServerParams() : impl_(std::make_shared<core::ServerParams>()) {}
inline ServerParams::ServerParams(const ServerParams& other)
    : impl_(std::make_shared<core::ServerParams>(*std::static_pointer_cast<core::ServerParams>(
          other.impl_))) {}
inline ServerParams& ServerParams::operator=(const ServerParams& that) {
  if (this != &that) {
    *std::static_pointer_cast<core::ServerParams>(impl_) =
        *std::static_pointer_cast<core::ServerParams>(that.impl_);
  }
  return *this;
}
inline ServerParams& ServerParams::default_service_params(const ServiceParams& service_params) {
  std::static_pointer_cast<core::ServerParams>(impl_)->default_service_params(
      *service_params.get_impl());
  return *this;
}
inline ServiceParams ServerParams::default_service_params() const {
  ServiceParams out;
  *out.get_impl() = std::static_pointer_cast<core::ServerParams>(impl_)->default_service_params();
  return out;
}
inline ServerParams::VendorDependent ServerParams::get_impl() const { return impl_; }

// -- ServiceEndpoint -----------------------------------------------------------------------

inline ServiceEndpoint::ServiceEndpoint() {}
inline ServiceEndpoint::ServiceEndpoint(const ServiceEndpoint& other) : RPCEntity(other) {}
template <class Impl>
ServiceEndpoint::ServiceEndpoint(Impl impl, int) : RPCEntity(std::move(impl), 0) {}

template <class TReq>
typename dds_type_traits<TReq>::DataReader ServiceEndpoint::get_request_datareader() const {
  return vendor::live<core::ServiceEndpoint>(impl_).template get_request_datareader<TReq>();
}
template <class TRep>
typename dds_type_traits<TRep>::DataWriter ServiceEndpoint::get_reply_datawriter() const {
  return vendor::live<core::ServiceEndpoint>(impl_).template get_reply_datawriter<TRep>();
}
inline void ServiceEndpoint::pause() { vendor::live<core::ServiceEndpoint>(impl_).pause(); }
inline void ServiceEndpoint::resume() { vendor::live<core::ServiceEndpoint>(impl_).resume(); }
inline ServiceStatus ServiceEndpoint::status() const {
  return static_cast<ServiceStatus>(vendor::live<core::ServiceEndpoint>(impl_).status());
}
inline ServiceParams ServiceEndpoint::get_service_params() const {
  ServiceParams out;
  *out.get_impl() = vendor::live<core::ServiceEndpoint>(impl_).get_service_params();
  return out;
}
inline ServiceEndpoint::VendorDependent ServiceEndpoint::get_impl() const {
  return std::static_pointer_cast<core::ServiceEndpoint>(impl_);
}

// -- ClientEndpoint ------------------------------------------------------------------------

inline ClientEndpoint::ClientEndpoint() {}
inline ClientEndpoint::ClientEndpoint(const ClientEndpoint& other) : ServiceProxy(other) {}
template <class Impl>
ClientEndpoint::ClientEndpoint(Impl impl, int) : ServiceProxy(std::move(impl), 0) {}

template <class TReq>
typename dds_type_traits<TReq>::DataWriter ClientEndpoint::get_request_datawriter() const {
  return vendor::live<core::ClientEndpoint>(impl_).template get_request_datawriter<TReq>();
}
template <class TRep>
typename dds_type_traits<TRep>::DataReader ClientEndpoint::get_reply_datareader() const {
  return vendor::live<core::ClientEndpoint>(impl_).template get_reply_datareader<TRep>();
}
inline dds::rpc::ClientParams ClientEndpoint::get_client_params() {
  ClientParams out;
  *out.get_impl() = vendor::live<core::ClientEndpoint>(impl_).get_client_params();
  return out;
}
inline ClientEndpoint::VendorDependent ClientEndpoint::get_impl() const { return impl_; }

// -- Server --------------------------------------------------------------------------------

inline Server::Server() : RPCEntity(std::make_shared<core::Server>(), 0) {}
inline Server::Server(const ServerParams& server_params)
    : RPCEntity(std::make_shared<core::Server>(
                    *std::static_pointer_cast<core::ServerParams>(server_params.get_impl())),
                0) {}
inline void Server::run() { vendor::live<core::Server>(impl_).run(); }
inline void Server::run(const dds::Duration& max_wait) {
  vendor::live<core::Server>(impl_).run(max_wait);
}
inline Server::VendorDependent Server::get_impl() const {
  return std::static_pointer_cast<core::Server>(impl_);
}

}  // namespace rpc
}  // namespace dds

#endif  // ZERODDS_DDS_RPC_FUNCTION_CALL_HPP
