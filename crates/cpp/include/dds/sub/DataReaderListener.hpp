// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/DataReaderListener.hpp — DataReaderListener<T> (DDS-PSM-Cxx 1.0
// §7.10).

#ifndef ZERODDS_DDS_SUB_DATAREADERLISTENER_HPP
#define ZERODDS_DDS_SUB_DATAREADERLISTENER_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/sub/detail/DataReaderFwd.hpp"
#include "dds/core/detail/borrow.hpp"
#include "dds/core/status/Status.hpp"
#include "dds/sub/detail/DataReaderStatus.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {


template <typename T>
class DataReaderListener {
public:
    virtual ~DataReaderListener() {}

    virtual void on_requested_deadline_missed(
        DataReader<T>& the_reader,
        const ::dds::core::status::RequestedDeadlineMissedStatus& status) = 0;
    virtual void on_requested_incompatible_qos(
        DataReader<T>& the_reader,
        const ::dds::core::status::RequestedIncompatibleQosStatus& status) = 0;
    virtual void on_sample_rejected(DataReader<T>& the_reader,
                                    const ::dds::core::status::SampleRejectedStatus& status) = 0;
    virtual void on_liveliness_changed(
        DataReader<T>& the_reader, const ::dds::core::status::LivelinessChangedStatus& status) = 0;
    virtual void on_data_available(DataReader<T>& the_reader) = 0;
    virtual void on_subscription_matched(
        DataReader<T>& the_reader, const ::dds::core::status::SubscriptionMatchedStatus& status) = 0;
    virtual void on_sample_lost(DataReader<T>& the_reader,
                                const ::dds::core::status::SampleLostStatus& status) = 0;
};

/// A listener that does nothing; derive and ZERODDS_OVERRIDE what you need.
template <typename T>
class NoOpDataReaderListener : public virtual DataReaderListener<T> {
public:
    virtual ~NoOpDataReaderListener() {}

    void on_requested_deadline_missed(
        DataReader<T>&, const ::dds::core::status::RequestedDeadlineMissedStatus&) ZERODDS_OVERRIDE {}
    void on_requested_incompatible_qos(
        DataReader<T>&, const ::dds::core::status::RequestedIncompatibleQosStatus&) ZERODDS_OVERRIDE {}
    void on_sample_rejected(DataReader<T>&,
                            const ::dds::core::status::SampleRejectedStatus&) ZERODDS_OVERRIDE {}
    void on_liveliness_changed(DataReader<T>&,
                               const ::dds::core::status::LivelinessChangedStatus&) ZERODDS_OVERRIDE {}
    void on_data_available(DataReader<T>&) ZERODDS_OVERRIDE {}
    void on_subscription_matched(DataReader<T>&,
                                 const ::dds::core::status::SubscriptionMatchedStatus&) ZERODDS_OVERRIDE {}
    void on_sample_lost(DataReader<T>&, const ::dds::core::status::SampleLostStatus&) ZERODDS_OVERRIDE {}
};

namespace detail {

/// Binds a DataReaderListener to the C API's listener table. Internal.
template <typename T>
struct DataReaderListenerBridge {
    static zerodds_ZeroDdsDataReaderListener table(DataReaderListener<T>* listener) {
        zerodds_ZeroDdsDataReaderListener vt = zerodds_ZeroDdsDataReaderListener();
        vt.user_data = static_cast<void*>(listener);
        vt.on_data_available = &cb_data_available;
        vt.on_sample_rejected = &cb_sample_rejected;
        vt.on_liveliness_changed = &cb_liveliness_changed;
        vt.on_requested_deadline_missed = &cb_requested_deadline_missed;
        vt.on_requested_incompatible_qos = &cb_requested_incompatible_qos;
        vt.on_subscription_matched = &cb_subscription_matched;
        vt.on_sample_lost = &cb_sample_lost;
        return vt;
    }

    // The C API hands the callback the raw reader; it is rebuilt as the
    // reader it names, with the status the reader holds (DDS 1.2 §7.1.2.5.7).
    static void cb_data_available(void* ud, zerodds_ZeroDdsDataReader* raw) {
        if (DataReaderListener<T>* l = static_cast<DataReaderListener<T>*>(ud)) {
            DataReader<T> r = ::dds::core::detail::borrow<DataReader<T> >(raw);
            l->on_data_available(r);
        }
    }
    static void cb_sample_rejected(void* ud, zerodds_ZeroDdsDataReader* raw) {
        if (DataReaderListener<T>* l = static_cast<DataReaderListener<T>*>(ud)) {
            DataReader<T> r = ::dds::core::detail::borrow<DataReader<T> >(raw);
            l->on_sample_rejected(r, sample_rejected_status_of(raw));
        }
    }
    static void cb_liveliness_changed(void* ud, zerodds_ZeroDdsDataReader* raw) {
        if (DataReaderListener<T>* l = static_cast<DataReaderListener<T>*>(ud)) {
            DataReader<T> r = ::dds::core::detail::borrow<DataReader<T> >(raw);
            l->on_liveliness_changed(r, liveliness_changed_status_of(raw));
        }
    }
    static void cb_requested_deadline_missed(void* ud, zerodds_ZeroDdsDataReader* raw) {
        if (DataReaderListener<T>* l = static_cast<DataReaderListener<T>*>(ud)) {
            DataReader<T> r = ::dds::core::detail::borrow<DataReader<T> >(raw);
            l->on_requested_deadline_missed(r, requested_deadline_missed_status_of(raw));
        }
    }
    static void cb_requested_incompatible_qos(void* ud, zerodds_ZeroDdsDataReader* raw) {
        if (DataReaderListener<T>* l = static_cast<DataReaderListener<T>*>(ud)) {
            DataReader<T> r = ::dds::core::detail::borrow<DataReader<T> >(raw);
            l->on_requested_incompatible_qos(r, requested_incompatible_qos_status_of(raw));
        }
    }
    static void cb_subscription_matched(void* ud, zerodds_ZeroDdsDataReader* raw) {
        if (DataReaderListener<T>* l = static_cast<DataReaderListener<T>*>(ud)) {
            DataReader<T> r = ::dds::core::detail::borrow<DataReader<T> >(raw);
            l->on_subscription_matched(r, subscription_matched_status_of(raw));
        }
    }
    static void cb_sample_lost(void* ud, zerodds_ZeroDdsDataReader* raw) {
        if (DataReaderListener<T>* l = static_cast<DataReaderListener<T>*>(ud)) {
            DataReader<T> r = ::dds::core::detail::borrow<DataReader<T> >(raw);
            l->on_sample_lost(r, sample_lost_status_of(raw));
        }
    }
};

} // namespace detail
} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_DATAREADERLISTENER_HPP
