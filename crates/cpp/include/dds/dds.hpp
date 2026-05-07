// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/dds.hpp — DDS-PSM-Cxx 1.0 master include (Spec §2.2.3 file-layout).
//
// Spec-konforme C++-API ueber das ZeroDDS C-FFI (zerodds.h).
//
// Beispiel:
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

#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/Status.hpp"
#include "dds/core/Time.hpp"
#include "dds/core/cond/Condition.hpp"
#include "dds/core/policy/CorePolicy.hpp"
#include "dds/core/qos.hpp"
#include "dds/domain/DomainParticipant.hpp"
#include "dds/pub/DataWriterListener.hpp"
#include "dds/pub/Publisher.hpp"
#include "dds/sub/DataReaderListener.hpp"
#include "dds/sub/Subscriber.hpp"
#include "dds/sub/cond/ReadCondition.hpp"
#include "dds/topic/BuiltinTopic.hpp"
#include "dds/topic/ContentFilteredTopic.hpp"
#include "dds/topic/Topic.hpp"
#include "dds/topic/TopicTraits.hpp"

#endif // ZERODDS_DDS_DDS_HPP
