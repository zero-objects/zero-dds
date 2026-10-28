#ifndef OMG_DDS_RPC_FUNCTION_CALL_H
#define OMG_DDS_RPC_FUNCTION_CALL_H

#include <string>
#include <vector>

#include "normative/request_reply.h" // standard

#include "vendor_dependent.h"

namespace dds { namespace rpc { 

class Server; 
enum ServiceStatus { CLOSED, PAUSED, RUNNING };

template <class Iface>
struct interface_traits
{
  typedef Iface                        InterfaceType;
  typedef typename Iface::ProxyType    ProxyType;
  typedef typename Iface::RequestType  RequestType;
  typedef typename Iface::ReplyType    ReplyType;
};

class Exception : public std::exception
{
  std::string message_;

public:
  Exception();
  Exception(const char *msg);

  virtual ~Exception() throw();
  virtual void raise() const = 0;
};

class ServiceParams
{
public:
  /* DomainParticipant argument is optional. When it's not provided, either lookup
    a default DomainParticipant or create one. 
  */
  ServiceParams();
  ServiceParams(const ServiceParams & other);
  ServiceParams & operator = (const ServiceParams & that);

  ServiceParams & service_name(const std::string &service_name);
  ServiceParams & instance_name(const std::string &instance_name);
  ServiceParams & request_topic_name(const std::string &req_topic);
  ServiceParams & reply_topic_name(const std::string &rep_topic);
  ServiceParams & datawriter_qos(dds_entity_traits::DataWriterQos qos);
  ServiceParams & datareader_qos(dds_entity_traits::DataReaderQos qos);
  ServiceParams & publisher(dds_entity_traits::Publisher publisher);
  ServiceParams & subscriber(dds_entity_traits::Subscriber subscriber);
  ServiceParams & domain_participant(dds_entity_traits::DomainParticipant part);

  std::string service_name() const;
  std::string instance_name() const;
  std::string request_topic_name() const;
  std::string reply_topic_name() const;
  dds_entity_traits::DataWriterQos datawriter_qos() const;
  dds_entity_traits::DataReaderQos datareader_qos() const;
  dds_entity_traits::Publisher publisher() const;
  dds_entity_traits::Subscriber subscriber() const;
  dds_entity_traits::DomainParticipant domain_participant() const;

protected:
  typedef details::vendor_dependent<ServiceParams>::type VendorDependent;
  VendorDependent impl_;

public:
  VendorDependent get_impl() const;
};

class ClientParams
{
public:
  /* DomainParticipant argument is optional. When it's not provided, either lookup
       a default DomainParticipant or create one.
  */
  ClientParams();
  ClientParams(const ClientParams & other);
  ClientParams & operator = (const ClientParams & that);

  ClientParams & service_name(const std::string &service_name);
  ClientParams & instance_name(const std::string &instance_name);
  ClientParams & request_topic_name(const std::string &req_topic);
  ClientParams & reply_topic_name(const std::string &rep_topic);
  ClientParams & datawriter_qos(const dds_entity_traits::DataWriterQos & qos);
  ClientParams & datareader_qos(const dds_entity_traits::DataReaderQos & qos);
  ClientParams & publisher(dds_entity_traits::Publisher publisher);
  ClientParams & subscriber(dds_entity_traits::Subscriber subscriber);
  ClientParams & domain_participant(dds_entity_traits::DomainParticipant part);

  const std::string & service_name() const;
  const std::string & instance_name() const;
  const std::string & request_topic_name() const;
  const std::string & reply_topic_name() const;
  const dds_entity_traits::DataWriterQos & datawriter_qos() const;
  const dds_entity_traits::DataReaderQos & datareader_qos() const;
  dds_entity_traits::Publisher publisher() const;
  dds_entity_traits::Subscriber subscriber() const;
  dds_entity_traits::DomainParticipant domain_participant() const;

protected:
  typedef details::vendor_dependent<ClientParams>::type VendorDependent;
  VendorDependent impl_;

public:
  VendorDependent get_impl() const;
};

class ServerParams
{
public:
  ServerParams();
  ServerParams(const ServerParams & other);
  ServerParams & operator = (const ServerParams & that);

  ServerParams & default_service_params(const ServiceParams & service_params);

  ServiceParams default_service_params() const;

protected:
  typedef details::vendor_dependent<ServerParams>::type VendorDependent;
  VendorDependent impl_;

public:
  VendorDependent get_impl() const;
};

class ServiceEndpoint : public RPCEntity
{
protected:

  template <class Impl>
  explicit ServiceEndpoint(Impl impl, int disambiguate);

public:

  typedef details::vendor_dependent<ServiceEndpoint>::type VendorDependent;
  
  ServiceEndpoint();
  ServiceEndpoint(const ServiceEndpoint &);

  template <class TReq>
  typename dds_type_traits<TReq>::DataReader  get_request_datareader() const;

  template <class TRep>
  typename dds_type_traits<TRep>::DataWriter get_reply_datawriter() const;

  void pause();
  void resume();
  ServiceStatus status() const;
  ServiceParams get_service_params() const;

  VendorDependent get_impl() const;
};

class ClientEndpoint : public ServiceProxy
{
protected:

  template <class Impl>
  explicit ClientEndpoint(Impl impl, int disambiguate);

public:

  typedef details::vendor_dependent<ServerParams>::type VendorDependent;

  ClientEndpoint();
  ClientEndpoint(const ClientEndpoint &);

  template <class TReq>
  typename dds_type_traits<TReq>::DataWriter get_request_datawriter() const;

  template <class TRep>
  typename dds_type_traits<TRep>::DataReader get_reply_datareader() const;

  dds::rpc::ClientParams get_client_params();

  VendorDependent get_impl() const;
};

class Server : public RPCEntity
{
public:

  typedef details::vendor_dependent<Server>::type VendorDependent;

   // may reuse an exising participant
   Server();

   explicit Server(const ServerParams & server_params);

   // blocking
   void run();

   // non-blocking
   void run(const dds::Duration & max_wait);

   VendorDependent get_impl() const;
};

} // namespace rpc
} // namespace dds

#include "function_call.hpp"

#endif // OMG_DDS_RPC_FUNCTION_CALL_H

