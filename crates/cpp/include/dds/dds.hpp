// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/dds.hpp — DDS-PSM-Cxx 1.0 master include (Spec §2.2.3 file-layout).
//
// Spec-compliant C++ API over the ZeroDDS C-FFI (zerodds.h).
//
// Example:
//
//   #include <dds/dds.hpp>
//   using namespace dds;
//
//   int main() {
//       domain::DomainParticipant dp(0);
//       topic::Topic<core::ByteSeq> t(dp, "Chatter");
//       pub::Publisher pub(dp);
//       pub::DataWriter<core::ByteSeq> dw(pub, t);
//       sub::Subscriber sub(dp);
//       sub::DataReader<core::ByteSeq> dr(sub, t);
//
//       core::ByteSeq msg = {'h', 'i'};
//       dw.write(msg);
//
//       auto loaned = dr.take();
//       for (auto& s : loaned) {
//           if (s.info().valid_data) { /* use s.data() */ }
//       }
//   }

#ifndef ZERODDS_DDS_DDS_HPP
#define ZERODDS_DDS_DDS_HPP

#include "dds/core/macros.hpp"
#include "dds/core/conformance.hpp"
#include "dds/core/array.hpp"
#include "dds/core/Optional.hpp"
#include "dds/core/refmacros.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/status/Status.hpp"
#include "dds/core/Time.hpp"
#include "dds/core/Duration.hpp"
#include "dds/core/cond/Condition.hpp"
#include "dds/core/policy/CorePolicy.hpp"
#include "dds/core/QosProvider.hpp"
#include "dds/core/qos_bridge.hpp"
#include "dds/pub/DataWriter.hpp"
#include "dds/pub/DataWriterListener.hpp"
#include "dds/pub/Publisher.hpp"
#include "dds/pub/discovery.hpp"
#include "dds/pub/find.hpp"
#include "dds/pub/CoherentSet.hpp"
#include "dds/pub/SuspendedPublication.hpp"
#include "dds/sub/DataReader.hpp"
#include "dds/sub/DataReaderListener.hpp"
#include "dds/sub/Subscriber.hpp"
#include "dds/sub/discovery.hpp"
#include "dds/sub/find.hpp"
#include "dds/sub/CoherentAccess.hpp"
#include "dds/sub/cond/ReadCondition.hpp"
#include "dds/topic/BuiltinTopic.hpp"
#include "dds/topic/BuiltinTopicKey.hpp"
#include "dds/topic/find.hpp"
#include "dds/topic/ContentFilteredTopic.hpp"
#include "dds/topic/MultiTopic.hpp"
#include "dds/topic/Topic.hpp"
#include "dds/topic/TopicTraits.hpp"
#include "dds/domain/DomainParticipant.hpp"
#include "dds/domain/discovery.hpp"
#include "dds/domain/DomainId.hpp"
#include "dds/domain/find.hpp"
#include "dds/topic/discovery.hpp"

// Callback implementations that require full type definitions (included last
// to avoid circular dependencies).

#endif // ZERODDS_DDS_DDS_HPP
