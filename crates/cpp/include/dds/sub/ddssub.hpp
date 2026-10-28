// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/ddssub.hpp — Sub module include (DDS-PSM-Cxx 1.0 §7.2).

#ifndef ZERODDS_DDS_SUB_DDSSUB_HPP
#define ZERODDS_DDS_SUB_DDSSUB_HPP

#include "dds/core/detail/conformance.hpp"

// Status Includes
#include "dds/sub/status/DataState.hpp"

// QoS Includes
#include "dds/sub/qos/DataReaderQos.hpp"
#include "dds/sub/qos/SubscriberQos.hpp"

// Sub Includes
#include "dds/sub/Rank.hpp"
#include "dds/sub/GenerationCount.hpp"
#include "dds/sub/AnyDataReaderListener.hpp"
#include "dds/sub/SampleInfo.hpp"
#include "dds/sub/LoanedSamples.hpp"
#include "dds/sub/Subscriber.hpp"
#include "dds/sub/CoherentAccess.hpp"
#include "dds/sub/SubscriberListener.hpp"
#include "dds/sub/DataReader.hpp"
#include "dds/sub/DataReaderListener.hpp"
#include "dds/sub/AnyDataReader.hpp"

// Condition Includes
#include "dds/sub/cond/ReadCondition.hpp"
#include "dds/sub/Query.hpp"
#include "dds/sub/cond/QueryCondition.hpp"
#include "dds/sub/find.hpp"

// Pretty Print Utilities
// std::ostream& operator<<(std::ostream& os, const dds::sub::status::DataState& s);
// std::ostream& operator<<(std::ostream& os, const dds::sub::Rank& r);
// std::ostream& operator<<(std::ostream& os, const dds::sub::SampleInfo& si);

#endif // ZERODDS_DDS_SUB_DDSSUB_HPP