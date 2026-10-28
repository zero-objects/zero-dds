#ifndef OMG_DDS_RPC_REQUEST_REPLY_H
#define OMG_DDS_RPC_REQUEST_REPLY_H

#include <string> 

#include "vendor_dependent.h"
#include "normative/sample.h"   // standard

namespace dds {

namespace rpc {

class RPCEntity 
{
public:
  RPCEntity();
  RPCEntity(const RPCEntity &);

  bool operator == (const RPCEntity &);
  void close();
  bool is_null() const;

protected:

  template <class Impl>
  explicit RPCEntity(Impl impl, int disambiguate);

protected:
  typedef details::vendor_dependent<RPCEntity>::type VendorDependent;
  VendorDependent impl_;

public:
  VendorDependent get_impl() const;
};

class ServiceProxy : public RPCEntity
{
protected:

  template <class Impl>
  explicit ServiceProxy(Impl impl, int disambiguate);

public:

  ServiceProxy();
  ServiceProxy(const ServiceProxy &);

  void bind(const std::string & instance_name);
  void unbind();
  bool is_bound() const;
  std::string get_bound_instance_name() const;
  std::vector<std::string> get_discoverd_service_instances() const;

  void wait_for_service();
  void wait_for_service(const dds::Duration & maxWait);

  void wait_for_service(std::string instanceName);
  void wait_for_service(const dds::Duration & maxWait,
                        std::string instanceName);

  void wait_for_services(int count);
  void wait_for_services(const dds::Duration & maxWait, int count);

  void wait_for_services(const std::vector<std::string> & instanceNames);
  void wait_for_services(const dds::Duration & maxWait,
                         const std::vector<std::string> & instanceNames);

  future<void> wait_for_service_async();
  future<void> wait_for_service_async(std::string instanceName);
  future<void> wait_for_services_async(int count);
  future<void> wait_for_services_async(const std::vector<std::string> & instanceNames);
};


template <typename TReq, typename TRep>
class Requester : public ServiceProxy
{
public:

    typedef TReq RequestType;

    typedef TRep ReplyType;

    typedef typename dds::dds_type_traits<TReq>::DataWriter RequestDataWriter;

    typedef typename dds::dds_type_traits<TRep>::DataReader ReplyDataReader;

    typedef typename dds::dds_type_traits<TRep>::LoanedSamplesType LoanedSamplesType;

    typedef RequesterParams Params;

    typedef typename details::vendor_dependent<Requester<TReq, TRep>>::type VendorDependent;

    Requester();
    
    explicit Requester(const RequesterParams& params);

    Requester (const Requester &);

    void swap(Requester & other);

    Requester & operator = (const Requester &);

    virtual ~Requester();

    void send_request(WriteSample<TReq> &request);
 
    void send_request(WriteSampleRef<TReq> & wsref);

    future<dds::Sample<TRep>> send_request_async(const TReq &);

#ifdef OMG_DDS_RPC_BASIC_PROFILE
    void send_request(TReq & request);
    void send_request_oneway(TReq &);
#endif 

#ifdef OMG_DDS_RPC_ENHANCED_PROFILE
    void send_request(const TReq & request);
    void send_request_oneway(const TReq &);
#endif

    bool receive_reply(
      Sample<TRep>& reply,
      const dds::Duration & timeout);

    bool receive_reply(
      SampleRef<TRep> reply,
      const dds::Duration & timeout);

    bool receive_reply(
      Sample<TRep>& reply,
      const dds::SampleIdentity & relatedRequestId,
      const dds::Duration & timeout);

    bool receive_reply(
      SampleRef<TRep> reply,
      const dds::SampleIdentity & relatedRequestId,
      const dds::Duration & timeout);

    LoanedSamples<TRep> receive_replies(const dds::Duration & max_wait);

    LoanedSamples<TRep> receive_replies(int min_count,
                                        int max_count,
                                        const dds::Duration & max_wait);

    bool wait_for_replies(
        int min_count,
        const dds::Duration & max_wait);

    bool wait_for_replies(const dds::Duration & max_wait);

    bool wait_for_replies(
        int min_count,
        const dds::Duration & max_wait,
        const dds::SampleIdentity & related_request_id);

    bool take_reply(Sample<TRep>& reply);

    bool take_reply(SampleRef<TRep> reply);

    bool take_reply(
        Sample<TRep>& reply, 
        const dds::SampleIdentity& related_request_id);

    bool take_reply(
        SampleRef<TRep> reply, 
        const dds::SampleIdentity& related_request_id);

    LoanedSamples<TRep> take_replies(int max_count);

    LoanedSamples<TRep> take_replies(
        int max_count, 
        const dds::SampleIdentity& related_request_id);

    LoanedSamples<TRep> take_replies(
        const dds::SampleIdentity& related_request_id);

    inline bool read_reply(Sample<TRep>& reply);

    inline bool read_reply(SampleRef<TRep> reply);

    inline bool read_reply(
        Sample<TRep>& reply, 
        const dds::SampleIdentity& related_request_id);

    inline bool read_reply(
        SampleRef<TRep> reply, 
        const dds::SampleIdentity& related_request_id);

    LoanedSamples<TRep> read_replies(int max_count);

    LoanedSamples<TRep> read_replies(
        int max_count, 
        const dds::SampleIdentity& related_request_id);

    LoanedSamples<TRep> read_replies(
        const dds::SampleIdentity& related_request_id);

    bool receive_nondata_samples(bool enable);

    RequesterParams get_requester_params() const;

    RequestDataWriter get_request_datawriter() const;

    ReplyDataReader get_reply_datareader() const;

    VendorDependent get_impl();
};

template <typename TReq, typename TRep>
class Replier : public RPCEntity
{
  public:

    typedef TReq RequestType;

    typedef TRep ReplyType;

    typedef typename dds::dds_type_traits<TRep>::DataWriter ReplyDataWriter;

    typedef typename dds::dds_type_traits<TReq>::DataReader RequestDataReader;

    typedef typename dds::dds_type_traits<TReq>::LoanedSamplesType LoanedSamplesType;

    typedef ReplierParams Params;

    typedef typename details::vendor_dependent<Replier<TReq, TRep>>::type VendorDependent;

    Replier();

    Replier(const Replier &);

    Replier & operator = (const Replier &);

    explicit Replier(const ReplierParams & params);
    
    virtual ~Replier();

    void swap(Replier & other);

    void send_reply(
      WriteSample<TRep> & reply,
      const dds::SampleIdentity& related_request_id);

    void send_reply(
      WriteSampleRef<TRep> & reply,
      const dds::SampleIdentity& related_request_id);

#ifdef OMG_DDS_RPC_BASIC_PROFILE
    void send_reply(
      TRep & reply,
      const dds::SampleIdentity& related_request_id);
#endif

#ifdef OMG_DDS_RPC_ENHANCED_PROFILE
    void send_reply(
      const TRep & reply,
      const dds::SampleIdentity& related_request_id);
#endif

    bool receive_request(
        Sample<TReq> & request,
        const dds::Duration & max_wait);

    bool receive_request(
        SampleRef<TReq> request,
        const dds::Duration & max_wait);

    LoanedSamplesType receive_requests(
        const dds::Duration & max_wait);

    LoanedSamplesType receive_requests(
        int min_request_count,
        int max_request_count,
        const dds::Duration& max_wait);

    bool wait_for_requests(
        const dds::Duration & max_wait);

    bool wait_for_requests(
        int min_count, 
        const dds::Duration & max_wait);

    bool take_request(Sample<TReq> & request);

    bool take_request(SampleRef<TReq> request);

    LoanedSamplesType take_requests(int max_samples);

    bool read_request(Sample<TReq> & request);

    bool read_request(SampleRef<TReq> request);

    LoanedSamplesType read_requests(int max_samples);

    const ReplierParams & get_replier_params() const;

    bool receive_nondata_samples(bool enable);

    RequestDataReader get_request_datareader() const;

    ReplyDataWriter get_reply_datawriter() const;

    VendorDependent get_impl() const;
};

template <class TReq, class TRep>
void swap(Replier<TReq, TRep> & cr1,  
          Replier<TReq, TRep> & cr2)
{
    cr1.swap(cr2);
}

class ListenerBase 
{
  virtual ~ListenerBase();
};

template <class TReq, class TRep>
class SimpleReplierListener : public ListenerBase
{
public:
  virtual TRep * process_request(const Sample<TReq> &, 
                                 const dds::SampleIdentity &) = 0;
  virtual ~SimpleReplierListener();
};

template <class TReq, class TRep>
class ReplierListener : public ListenerBase
{
public:
  virtual void on_request_available(Replier<TReq, TRep> &) = 0;
  virtual ~ReplierListener();
};

template <class TRep>
class SimpleRequesterListener : public ListenerBase
{
public:
  virtual void process_reply(const Sample<TRep> &, 
                             const dds::SampleIdentity &) = 0;
  virtual ~SimpleRequesterListener();
};

template <class TReq, class TRep>
class RequesterListener : public ListenerBase
{
public:
  virtual void on_reply_available(Requester<TReq, TRep> &) = 0;
  virtual ~RequesterListener();
};

class RequesterParams 
{
public:
    /* Normative: DomainParticipant argument is optional. When it's not provided, either 
       lookup a default DomainParticipant or create one.
    */
    RequesterParams ();
    RequesterParams(const RequesterParams & other);
    RequesterParams & operator = (const RequesterParams & that);

    template <class TRep>
    RequesterParams & 	simple_requester_listener(
        SimpleRequesterListener<TRep> *listener);

    template <class TReq, class TRep>
    RequesterParams & 	requester_listener(
        RequesterListener<TReq, TRep> *listener);

    RequesterParams & 	domain_participant(dds_entity_traits::DomainParticipant participant);
    RequesterParams & 	publisher(dds_entity_traits::Publisher publisher);
    RequesterParams & 	subscriber(dds_entity_traits::Subscriber subscriber);
    RequesterParams & 	datawriter_qos(dds_entity_traits::DataWriterQos qos);
    RequesterParams & 	datareader_qos(dds_entity_traits::DataReaderQos qos);
    RequesterParams & 	service_name (const std::string &name);
    RequesterParams & 	request_topic_name (const std::string &name);
    RequesterParams & 	reply_topic_name (const std::string &name);

    dds_entity_traits::DomainParticipant domain_participant() const;
    dds_entity_traits::Publisher publisher() const;
    dds_entity_traits::Subscriber subscriber() const;
    dds_entity_traits::DataWriterQos datawriter_qos() const;
    dds_entity_traits::DataReaderQos datareader_qos() const;
    ListenerBase * simple_requester_listener() const;
    ListenerBase * requester_listener() const;
    std::string service_name() const;
    std::string request_topic_name() const;
    std::string reply_topic_name() const;

private:
    typedef details::vendor_dependent<RequesterParams>::type VendorDependent;
    VendorDependent impl_;

public:
    VendorDependent get_impl();
};


class ReplierParams 
{
public:
    /* Normative: DomainParticipant argument is optional. When it's not provided, either
       lookup a default DomainParticipant or create one.
    */
    ReplierParams ();
    ReplierParams(const ReplierParams & other);
    ReplierParams & operator = (const ReplierParams & that);

    template <class TReq, class TRep>
    ReplierParams & simple_replier_listener (
        SimpleReplierListener<TReq, TRep> *listener);    
    
    template <class TReq, class TRep>
    ReplierParams & replier_listener (
        ReplierListener<TReq, TRep> *listener);
    
    ReplierParams & domain_participant(dds_entity_traits::DomainParticipant participant);
    ReplierParams & service_name (const std::string &service_name);
    ReplierParams & instance_name (const std::string &service_name);
    ReplierParams & request_topic_name (const std::string &req_topic);
    ReplierParams & reply_topic_name (const std::string &rep_topic);
    ReplierParams & datawriter_qos(dds_entity_traits::DataWriterQos qos);
    ReplierParams & datareader_qos(dds_entity_traits::DataReaderQos qos);
    ReplierParams & publisher(dds_entity_traits::Publisher publisher);
    ReplierParams & subscriber(dds_entity_traits::Subscriber subscriber);

    dds_entity_traits::DomainParticipant domain_participant() const;
    ListenerBase * simple_replier_listener() const;
    ListenerBase * replier_listener() const;
    std::string service_name() const;
    std::string instance_name() const;
    std::string request_topic_name() const;
    std::string reply_topic_name() const;
    dds_entity_traits::DataWriterQos datawriter_qos() const;
    dds_entity_traits::DataReaderQos datareader_qos() const;
    dds_entity_traits::Publisher publisher() const;
    dds_entity_traits::Subscriber subscriber() const;

private:
  typedef details::vendor_dependent<ReplierParams>::type VendorDependent;
  VendorDependent impl_;

public:
  VendorDependent get_impl();
};

} // namespace rpc

} // namespace dds 

#include "request_reply.hpp" // vendor dependent

#endif // OMG_DDS_RPC_REQUEST_REPLY_H

