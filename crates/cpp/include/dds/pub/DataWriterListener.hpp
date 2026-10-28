// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/DataWriterListener.hpp — DataWriterListener<T> (DDS-PSM-Cxx 1.0
// §7.9).
//
//   struct Mine : dds::pub::NoOpDataWriterListener<Track> {
//       void on_publication_matched(dds::pub::DataWriter<Track>&,
//                                   const dds::core::status::PublicationMatchedStatus&) ZERODDS_OVERRIDE {}
//   };
//   Mine mine;
//   dw.listener(&mine, dds::core::status::StatusMask::all());

#ifndef ZERODDS_DDS_PUB_DATAWRITERLISTENER_HPP
#define ZERODDS_DDS_PUB_DATAWRITERLISTENER_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/detail/borrow.hpp"
#include "dds/core/status/Status.hpp"
#include "dds/pub/detail/DataWriterStatus.hpp"
#include "dds/pub/detail/DataWriterFwd.hpp"
#include "zerodds.h"

namespace dds {
namespace pub {

template <typename T>
class DataWriterListener {
public:
    virtual ~DataWriterListener() {}

    virtual void on_offered_deadline_missed(
        DataWriter<T>& writer, const ::dds::core::status::OfferedDeadlineMissedStatus& status) = 0;
    // The normative header takes this writer by value, the other callbacks by reference; an
    // ZERODDS_OVERRIDE written against it has to match.
    virtual void on_offered_incompatible_qos(
        DataWriter<T> writer, const ::dds::core::status::OfferedIncompatibleQosStatus& status) = 0;
    virtual void on_liveliness_lost(DataWriter<T>& writer,
                                    const ::dds::core::status::LivelinessLostStatus& status) = 0;
    virtual void on_publication_matched(
        DataWriter<T>& writer, const ::dds::core::status::PublicationMatchedStatus& status) = 0;
};

/// A listener that does nothing; derive and override what you need.
template <typename T>
class NoOpDataWriterListener : public virtual DataWriterListener<T> {
public:
    virtual ~NoOpDataWriterListener() {}

    void on_offered_deadline_missed(
        DataWriter<T>&, const ::dds::core::status::OfferedDeadlineMissedStatus&) ZERODDS_OVERRIDE {}
    void on_offered_incompatible_qos(
        DataWriter<T>, const ::dds::core::status::OfferedIncompatibleQosStatus&) ZERODDS_OVERRIDE {}
    void on_liveliness_lost(DataWriter<T>&,
                            const ::dds::core::status::LivelinessLostStatus&) ZERODDS_OVERRIDE {}
    void on_publication_matched(DataWriter<T>&,
                                const ::dds::core::status::PublicationMatchedStatus&) ZERODDS_OVERRIDE {}
};

namespace detail {

/// Binds a DataWriterListener to the C API's listener table. Internal.
template <typename T>
struct DataWriterListenerBridge {
    static zerodds_ZeroDdsDataWriterListener table(DataWriterListener<T>* listener) {
        zerodds_ZeroDdsDataWriterListener vt = zerodds_ZeroDdsDataWriterListener();
        vt.user_data = static_cast<void*>(listener);
        vt.on_liveliness_lost = &cb_liveliness_lost;
        vt.on_offered_deadline_missed = &cb_offered_deadline_missed;
        vt.on_offered_incompatible_qos = &cb_offered_incompatible_qos;
        vt.on_publication_matched = &cb_publication_matched;
        return vt;
    }

    // The C API hands the callback the raw writer; it is rebuilt as the
    // writer it names, with the status the writer holds (DDS 1.2 §7.1.2.4.4).
    static void cb_liveliness_lost(void* ud, zerodds_ZeroDdsDataWriter* raw) {
        if (DataWriterListener<T>* l = static_cast<DataWriterListener<T>*>(ud)) {
            DataWriter<T> w = ::dds::core::detail::borrow<DataWriter<T> >(raw);
            l->on_liveliness_lost(w, liveliness_lost_status_of(raw));
        }
    }
    static void cb_offered_deadline_missed(void* ud, zerodds_ZeroDdsDataWriter* raw) {
        if (DataWriterListener<T>* l = static_cast<DataWriterListener<T>*>(ud)) {
            DataWriter<T> w = ::dds::core::detail::borrow<DataWriter<T> >(raw);
            l->on_offered_deadline_missed(w, offered_deadline_missed_status_of(raw));
        }
    }
    static void cb_offered_incompatible_qos(void* ud, zerodds_ZeroDdsDataWriter* raw) {
        if (DataWriterListener<T>* l = static_cast<DataWriterListener<T>*>(ud)) {
            DataWriter<T> w = ::dds::core::detail::borrow<DataWriter<T> >(raw);
            l->on_offered_incompatible_qos(w, offered_incompatible_qos_status_of(raw));
        }
    }
    static void cb_publication_matched(void* ud, zerodds_ZeroDdsDataWriter* raw) {
        if (DataWriterListener<T>* l = static_cast<DataWriterListener<T>*>(ud)) {
            DataWriter<T> w = ::dds::core::detail::borrow<DataWriter<T> >(raw);
            l->on_publication_matched(w, publication_matched_status_of(raw));
        }
    }
};

} // namespace detail
} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_DATAWRITERLISTENER_HPP
