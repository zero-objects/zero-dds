// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/AnyDataWriterListener.hpp — AnyDataWriterListener
// (DDS-PSM-Cxx 1.0 §7.9): the writer statuses a Publisher listener hears,
// for writers of any topic type.

#ifndef ZERODDS_DDS_PUB_ANYDATAWRITERLISTENER_HPP
#define ZERODDS_DDS_PUB_ANYDATAWRITERLISTENER_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/status/Status.hpp"

namespace dds {
namespace pub {

class AnyDataWriter;

class AnyDataWriterListener {
public:
    virtual ~AnyDataWriterListener() {}

    virtual void on_offered_deadline_missed(
        AnyDataWriter& writer, const ::dds::core::status::OfferedDeadlineMissedStatus& status) = 0;
    virtual void on_offered_incompatible_qos(
        AnyDataWriter& writer, const ::dds::core::status::OfferedIncompatibleQosStatus& status) = 0;
    virtual void on_liveliness_lost(AnyDataWriter& writer,
                                    const ::dds::core::status::LivelinessLostStatus& status) = 0;
    virtual void on_publication_matched(
        AnyDataWriter& writer, const ::dds::core::status::PublicationMatchedStatus& status) = 0;
};

/// A listener that does nothing; derive and ZERODDS_OVERRIDE what you need.
class NoOpAnyDataWriterListener : public virtual AnyDataWriterListener {
public:
    virtual ~NoOpAnyDataWriterListener() {}

    void on_offered_deadline_missed(
        AnyDataWriter&, const ::dds::core::status::OfferedDeadlineMissedStatus&) ZERODDS_OVERRIDE {}
    void on_offered_incompatible_qos(
        AnyDataWriter&, const ::dds::core::status::OfferedIncompatibleQosStatus&) ZERODDS_OVERRIDE {}
    void on_liveliness_lost(AnyDataWriter&,
                            const ::dds::core::status::LivelinessLostStatus&) ZERODDS_OVERRIDE {}
    void on_publication_matched(AnyDataWriter&,
                                const ::dds::core::status::PublicationMatchedStatus&) ZERODDS_OVERRIDE {}
};

} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_ANYDATAWRITERLISTENER_HPP
