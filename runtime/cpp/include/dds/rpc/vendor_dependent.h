// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// vendor_dependent.h — the part of the C++ language binding of DDS-RPC 1.0 (§7.11.1)
// that the normative headers (normative/request_reply.h, normative/function_call.h and
// normative/sample.h, as published in omg-dds-rpc-cxx.zip) call "vendor dependent".
//
// What the OMG headers leave to the vendor, and what this file gives them:
//
//  * `OMG_DDS_RPC_REFERENCE_IMPLEMENTATION`: sample.h then declares nothing and relies on
//    this file for `dds::Sample`, `SampleRef`, `WriteSample`, `WriteSampleRef`,
//    `LoanedSamples`, `SharedSamples`, `SampleIterator`, `SampleInfo`, `SampleIdentity`,
//    `dds_type_traits` and `dds_entity_traits`. They are `dds/rpc/base.hpp`.
//  * `OMG_DDS_RPC_BASIC_PROFILE` and `OMG_DDS_RPC_ENHANCED_PROFILE`: both service mappings
//    are implemented, so the declarations of both profiles are in.
//  * `dds::details::vendor_dependent<T>::type`: the type of the `impl_` of each class.
//    Each is a pointer to the object of `dds/rpc/core.hpp` that implements the class; the
//    classes of the OMG header are handles on them (`request_reply.hpp`,
//    `function_call.hpp`).
//  * The forward declarations the OMG headers use before they declare them
//    (`RequesterParams`, `ReplierParams`).
//
// Included by the OMG headers themselves and by `dds/rpc.hpp`.

#ifndef ZERODDS_DDS_RPC_VENDOR_DEPENDENT_H
#define ZERODDS_DDS_RPC_VENDOR_DEPENDENT_H

// The standard headers the OMG headers need, before anything is redefined below.
#include <exception>
#include <functional>
#include <future>
#include <iterator>
#include <map>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

#include <dds/rpc/core.hpp>

#ifndef OMG_DDS_RPC_REFERENCE_IMPLEMENTATION
#define OMG_DDS_RPC_REFERENCE_IMPLEMENTATION 1
#endif
#ifndef OMG_DDS_RPC_BASIC_PROFILE
#define OMG_DDS_RPC_BASIC_PROFILE 1
#endif
#ifndef OMG_DDS_RPC_ENHANCED_PROFILE
#define OMG_DDS_RPC_ENHANCED_PROFILE 1
#endif
#ifndef OMG_DDS_RPC_HASCPP11
#define OMG_DDS_RPC_HASCPP11 1
#endif

namespace dds {

using rpc::dds_type_traits;

/// §7.11.1.4.23: the DDS entity types of the binding, by name. The abstract entities of
/// `dds/rpc/base.hpp` (`dds::rpc::entity_traits<>`) as shared pointers; a binding for another DDS API
/// specialises `entity_traits`.
struct dds_entity_traits {
  // The OMG classes take and give the entities by value, so the handles of the abstract
  // entities of `dds/rpc/base.hpp` are shared pointers to them.
  typedef std::shared_ptr<rpc::DomainParticipant> DomainParticipant;
  typedef std::shared_ptr<rpc::Publisher> Publisher;
  typedef std::shared_ptr<rpc::Subscriber> Subscriber;
  typedef std::shared_ptr<rpc::DataWriter> DataWriter;
  typedef std::shared_ptr<rpc::DataReader> DataReader;
  typedef rpc::entity_traits<>::DataWriterQos DataWriterQos;
  typedef rpc::entity_traits<>::DataReaderQos DataReaderQos;
};

namespace rpc {

class RPCEntity;
class ServiceProxy;
template <typename TReq, typename TRep>
class Requester;
template <typename TReq, typename TRep>
class Replier;
class ListenerBase;
class RequesterParams;
class ReplierParams;
class ServiceParams;
class ClientParams;
class ServerParams;
class ServiceEndpoint;
class ClientEndpoint;
class Server;

namespace vendor {

/// The params of a requester: the implementation's params, and what the normative class
/// has to give back that the implementation does not keep as such.
struct RequesterParamsImpl : core::RequesterParams {
  /// The adapters that let the implementation call the listeners of the OMG header.
  std::shared_ptr<void> adapter;
  /// The listeners as the application gave them.
  ListenerBase* user_simple = nullptr;
  ListenerBase* user_full = nullptr;
};

struct ReplierParamsImpl : core::ReplierParams {
  std::shared_ptr<void> adapter;
  ListenerBase* user_simple = nullptr;
  ListenerBase* user_full = nullptr;
};

}  // namespace vendor
}  // namespace rpc

namespace details {

template <typename T>
struct vendor_dependent;

template <>
struct vendor_dependent<rpc::RPCEntity> {
  typedef std::shared_ptr<rpc::core::RPCEntity> type;
};
template <typename TReq, typename TRep>
struct vendor_dependent<rpc::Requester<TReq, TRep>> {
  typedef std::shared_ptr<rpc::core::Requester<TReq, TRep>> type;
};
template <typename TReq, typename TRep>
struct vendor_dependent<rpc::Replier<TReq, TRep>> {
  typedef std::shared_ptr<rpc::core::Replier<TReq, TRep>> type;
};
template <>
struct vendor_dependent<rpc::RequesterParams> {
  typedef std::shared_ptr<rpc::vendor::RequesterParamsImpl> type;
};
template <>
struct vendor_dependent<rpc::ReplierParams> {
  typedef std::shared_ptr<rpc::vendor::ReplierParamsImpl> type;
};
template <>
struct vendor_dependent<rpc::ServiceParams> {
  typedef std::shared_ptr<rpc::core::ServiceParams> type;
};
template <>
struct vendor_dependent<rpc::ClientParams> {
  typedef std::shared_ptr<rpc::core::ClientParams> type;
};
// `ClientEndpoint::VendorDependent` is declared in the OMG header as
// `vendor_dependent<ServerParams>::type`, so this one type serves the params of a server and
// the implementation of a client endpoint: an untyped pointer.
template <>
struct vendor_dependent<rpc::ServerParams> {
  typedef std::shared_ptr<void> type;
};
template <>
struct vendor_dependent<rpc::ServiceEndpoint> {
  typedef std::shared_ptr<rpc::core::ServiceEndpoint> type;
};
template <>
struct vendor_dependent<rpc::Server> {
  typedef std::shared_ptr<rpc::core::Server> type;
};

}  // namespace details
}  // namespace dds

#endif  // ZERODDS_DDS_RPC_VENDOR_DEPENDENT_H
