// An OpenDDS reader of KeyedPointA that reports instance states, for
// crates/zerodds-c-api/tests/writer_dispose_sample_vendors.rs. The protocol
// is the one of ../fastdds/reader.cpp; the -DCPS* arguments go first:
//
//   reader -DCPSConfigFile rtps.ini DOMAIN TOPIC SECONDS SNAPSHOT_FILE STOP_FILE
#include "KeyedPointATypeSupportImpl.h"
#include <ace/OS_NS_stdlib.h>
#include <dds/DCPS/Marked_Default_Qos.h>
#include <dds/DCPS/Service_Participant.h>
#include <dds/DCPS/StaticIncludes.h>
#if OPENDDS_DO_MANUAL_STATIC_INCLUDES
#  include <dds/DCPS/RTPS/RtpsDiscovery.h>
#  include <dds/DCPS/transport/rtps_udp/RtpsUdp.h>
#endif
#include <dds/DdsDcpsInfrastructureC.h>
#include <chrono>
#include <iostream>
#include <map>
#include <string>
#include <sys/stat.h>
#include <thread>

static bool exists(const char* path)
{
  struct stat st;
  return stat(path, &st) == 0;
}

static const char* state_name(DDS::InstanceStateKind s)
{
  switch (s) {
  case DDS::ALIVE_INSTANCE_STATE: return "ALIVE";
  case DDS::NOT_ALIVE_DISPOSED_INSTANCE_STATE: return "NOT_ALIVE_DISPOSED";
  case DDS::NOT_ALIVE_NO_WRITERS_INSTANCE_STATE: return "NOT_ALIVE_NO_WRITERS";
  default: return "UNKNOWN";
  }
}

int ACE_TMAIN(int argc, ACE_TCHAR* argv[])
{
  DDS::DomainParticipantFactory_var dpf = TheParticipantFactoryWithArgs(argc, argv);
  if (argc != 6) {
    std::cerr << "usage: reader [-DCPS...] DOMAIN TOPIC SECONDS SNAPSHOT_FILE STOP_FILE\n";
    return 2;
  }
  const int domain = ACE_OS::atoi(argv[1]);
  const char* snapshot = argv[4];
  const char* stop = argv[5];
  DDS::DomainParticipant_var dp = dpf->create_participant(
    domain, PARTICIPANT_QOS_DEFAULT, 0, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
  KeyedPointATypeSupport_var ts = new KeyedPointATypeSupportImpl;
  ts->register_type(dp, "");
  CORBA::String_var tn = ts->get_type_name();
  DDS::Topic_var topic = dp->create_topic(
    argv[2], tn, TOPIC_QOS_DEFAULT, 0, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
  DDS::Subscriber_var sub = dp->create_subscriber(
    SUBSCRIBER_QOS_DEFAULT, 0, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
  DDS::DataReaderQos rq;
  sub->get_default_datareader_qos(rq);
  rq.reliability.kind = DDS::RELIABLE_RELIABILITY_QOS;
  rq.history.kind = DDS::KEEP_ALL_HISTORY_QOS;
  DDS::DataReader_var dr = sub->create_datareader(
    topic, rq, 0, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
  KeyedPointADataReader_var rd = KeyedPointADataReader::_narrow(dr);
  if (!rd) {
    std::cerr << "no reader\n";
    return 1;
  }
  std::cout << "READY" << std::endl;

  std::map<DDS::InstanceHandle_t, CORBA::Long> ids;
  std::map<DDS::InstanceHandle_t, std::string> states;
  bool matched = false, snapped = false;
  const auto end = std::chrono::steady_clock::now() +
    std::chrono::milliseconds(static_cast<long>(ACE_OS::strtod(argv[3], 0) * 1000));
  while (std::chrono::steady_clock::now() < end && !exists(stop)) {
    DDS::SubscriptionMatchedStatus st;
    rd->get_subscription_matched_status(st);
    if (!matched && st.current_count >= 1) {
      std::cout << "MATCHED" << std::endl;
      matched = true;
    }
    KeyedPointA s;
    DDS::SampleInfo info;
    while (rd->take_next_sample(s, info) == DDS::RETCODE_OK) {
      if (info.valid_data) {
        if (ids.find(info.instance_handle) == ids.end()) {
          std::cout << "SEEN id=" << s.id << std::endl;
        }
        ids[info.instance_handle] = s.id;
      }
      states[info.instance_handle] = state_name(info.instance_state);
    }
    if (!snapped && exists(snapshot)) {
      for (const auto& e : states) {
        auto id = ids.find(e.first);
        std::cout << "SNAP id=" << (id == ids.end() ? std::string("?") : std::to_string(id->second))
                  << " state=" << e.second << std::endl;
      }
      std::cout << "SNAPDONE" << std::endl;
      snapped = true;
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(20));
  }
  for (const auto& e : states) {
    auto id = ids.find(e.first);
    std::cout << "FINAL id=" << (id == ids.end() ? std::string("?") : std::to_string(id->second))
              << " state=" << e.second << std::endl;
  }
  dp->delete_contained_entities();
  dpf->delete_participant(dp);
  TheServiceParticipant->shutdown();
  return 0;
}
