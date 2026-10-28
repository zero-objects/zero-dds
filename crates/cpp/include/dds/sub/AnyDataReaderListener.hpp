// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/AnyDataReaderListener.hpp — AnyDataReaderListener
// (DDS-PSM-Cxx 1.0 §7.10): the reader statuses a Subscriber listener hears,
// for readers of any topic type.

#ifndef ZERODDS_DDS_SUB_ANYDATAREADERLISTENER_HPP
#define ZERODDS_DDS_SUB_ANYDATAREADERLISTENER_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/status/Status.hpp"

namespace dds {
namespace sub {

class AnyDataReader;

class AnyDataReaderListener {
public:
    virtual ~AnyDataReaderListener() {}

    virtual void on_requested_deadline_missed(
        AnyDataReader& the_reader,
        const ::dds::core::status::RequestedDeadlineMissedStatus& status) = 0;
    virtual void on_requested_incompatible_qos(
        AnyDataReader& the_reader,
        const ::dds::core::status::RequestedIncompatibleQosStatus& status) = 0;
    virtual void on_sample_rejected(AnyDataReader& the_reader,
                                    const ::dds::core::status::SampleRejectedStatus& status) = 0;
    virtual void on_liveliness_changed(
        AnyDataReader& the_reader, const ::dds::core::status::LivelinessChangedStatus& status) = 0;
    virtual void on_data_available(AnyDataReader& the_reader) = 0;
    virtual void on_subscription_matched(
        AnyDataReader& the_reader, const ::dds::core::status::SubscriptionMatchedStatus& status) = 0;
    virtual void on_sample_lost(AnyDataReader& the_reader,
                                const ::dds::core::status::SampleLostStatus& status) = 0;
};

/// A listener that does nothing; derive and ZERODDS_OVERRIDE what you need.
class NoOpAnyDataReaderListener : public virtual AnyDataReaderListener {
public:
    virtual ~NoOpAnyDataReaderListener() {}

    void on_requested_deadline_missed(
        AnyDataReader&, const ::dds::core::status::RequestedDeadlineMissedStatus&) ZERODDS_OVERRIDE {}
    void on_requested_incompatible_qos(
        AnyDataReader&, const ::dds::core::status::RequestedIncompatibleQosStatus&) ZERODDS_OVERRIDE {}
    void on_sample_rejected(AnyDataReader&,
                            const ::dds::core::status::SampleRejectedStatus&) ZERODDS_OVERRIDE {}
    void on_liveliness_changed(AnyDataReader&,
                               const ::dds::core::status::LivelinessChangedStatus&) ZERODDS_OVERRIDE {}
    void on_data_available(AnyDataReader&) ZERODDS_OVERRIDE {}
    void on_subscription_matched(AnyDataReader&,
                                 const ::dds::core::status::SubscriptionMatchedStatus&) ZERODDS_OVERRIDE {}
    void on_sample_lost(AnyDataReader&, const ::dds::core::status::SampleLostStatus&) ZERODDS_OVERRIDE {}
};

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_ANYDATAREADERLISTENER_HPP
