/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 */
/* OpenSplice Community Edition 6.9 (classic C++ API, SACPP) for the OMG
 * dds-rtps shape_main.cxx. The code paths it takes are the ones of the
 * vendors with the same CORBA-style C++ mapping (_var types, _narrow,
 * String_mgr members): OpenDDS and CoreDX. OpenSplice 6.9 has no
 * DataRepresentationQosPolicy and writes XCDR1 only; shape_main.cxx
 * refuses a request for anything else instead of ignoring it. */
#include <map>
#include <string>
#include "ccpp_dds_dcps.h"
#include "ccpp_shape.h"

/* get_instance() answers the interface; shape_main.cxx holds the class. */
#define OBTAIN_DOMAIN_PARTICIPANT_FACTORY \
    dynamic_cast<DDS::DomainParticipantFactory *>(DDS::DomainParticipantFactory::get_instance())

#define LISTENER_STATUS_MASK_ALL  DDS::STATUS_MASK_ANY_V1_2
#define LISTENER_STATUS_MASK_NONE DDS::STATUS_MASK_NONE
#define REGISTER_TYPE ShapeTypeTypeSupport_var ts = \
    new ShapeTypeTypeSupport; ts->register_type
#define STRING_IN .in()
#define STRING_INOUT .inout()
#define STRING_ASSIGN(LHS, RHS) LHS = static_cast<const char *>(RHS)
#define STRING_FREE DDS::string_free
#define DDS_BOOLEAN_TRUE  true
#define DDS_BOOLEAN_FALSE false

/* No DataRepresentationQosPolicy in 6.9; the option still parses. XCDR1
 * (0) is the only representation the product writes. */
namespace DDS {
typedef DDS::Short DataRepresentationId_t;
const DataRepresentationId_t XCDR_DATA_REPRESENTATION  = 0;
const DataRepresentationId_t XML_DATA_REPRESENTATION   = 1;
const DataRepresentationId_t XCDR2_DATA_REPRESENTATION = 2;
}

const char *get_qos_policy_name(DDS::QosPolicyId_t policy_id)
{
  switch (policy_id) {
  case DDS::USERDATA_QOS_POLICY_ID: return "USERDATA";
  case DDS::DURABILITY_QOS_POLICY_ID: return "DURABILITY";
  case DDS::PRESENTATION_QOS_POLICY_ID: return "PRESENTATION";
  case DDS::DEADLINE_QOS_POLICY_ID: return "DEADLINE";
  case DDS::LATENCYBUDGET_QOS_POLICY_ID: return "LATENCYBUDGET";
  case DDS::OWNERSHIP_QOS_POLICY_ID: return "OWNERSHIP";
  case DDS::OWNERSHIPSTRENGTH_QOS_POLICY_ID: return "OWNERSHIPSTRENGTH";
  case DDS::LIVELINESS_QOS_POLICY_ID: return "LIVELINESS";
  case DDS::TIMEBASEDFILTER_QOS_POLICY_ID: return "TIMEBASEDFILTER";
  case DDS::PARTITION_QOS_POLICY_ID: return "PARTITION";
  case DDS::RELIABILITY_QOS_POLICY_ID: return "RELIABILITY";
  case DDS::DESTINATIONORDER_QOS_POLICY_ID: return "DESTINATIONORDER";
  case DDS::HISTORY_QOS_POLICY_ID: return "HISTORY";
  case DDS::RESOURCELIMITS_QOS_POLICY_ID: return "RESOURCELIMITS";
  case DDS::ENTITYFACTORY_QOS_POLICY_ID: return "ENTITYFACTORY";
  case DDS::WRITERDATALIFECYCLE_QOS_POLICY_ID: return "WRITERDATALIFECYCLE";
  case DDS::READERDATALIFECYCLE_QOS_POLICY_ID: return "READERDATALIFECYCLE";
  case DDS::TOPICDATA_QOS_POLICY_ID: return "TOPICDATA";
  case DDS::GROUPDATA_QOS_POLICY_ID: return "GROUPDATA";
  case DDS::TRANSPORTPRIORITY_QOS_POLICY_ID: return "TRANSPORTPRIORITY";
  case DDS::LIFESPAN_QOS_POLICY_ID: return "LIFESPAN";
  case DDS::DURABILITYSERVICE_QOS_POLICY_ID: return "DURABILITYSERVICE";
  default: return "Unknown";
  }
}

void StringSeq_push(DDS::StringSeq &string_seq, const char *elem)
{
  const DDS::ULong i = string_seq.length();
  string_seq.length(i + 1);
  string_seq[i] = elem;
}

template <typename Seq>
DDS::ULong DDS_UInt8Seq_get_length(const Seq *seq) { return seq->length(); }

template <typename Seq>
void DDS_UInt8Seq_ensure_length(Seq *seq, DDS::ULong len, DDS::ULong = 0) { seq->length(len); }

template <typename Seq>
DDS::Octet *DDS_UInt8Seq_get_reference(Seq *seq, DDS::ULong idx) { return &(*seq)[idx]; }

template <typename Seq>
const DDS::Octet *DDS_UInt8Seq_get_reference(const Seq *seq, DDS::ULong idx) { return &(*seq)[idx]; }
