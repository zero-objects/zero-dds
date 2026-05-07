// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/DataReaderListener.hpp — DDS-PSM-Cxx 1.0 §7.5.9.6.

#ifndef ZERODDS_DDS_SUB_DATAREADERLISTENER_HPP
#define ZERODDS_DDS_SUB_DATAREADERLISTENER_HPP

#include "dds/core/Status.hpp"
#include "dds/sub/Subscriber.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {

/// `DataReaderListener<T>` (Spec §7.5.9.6).
template <typename T>
class DataReaderListener {
public:
    virtual ~DataReaderListener() = default;
    /// Data-Available.
    virtual void on_data_available(DataReader<T>& /*dr*/) {}
    /// Sample-Rejected.
    virtual void on_sample_rejected(
        DataReader<T>& /*dr*/,
        const ::dds::core::status::SampleRejectedStatus& /*s*/) {}
    /// Liveliness-Changed.
    virtual void on_liveliness_changed(
        DataReader<T>& /*dr*/,
        const ::dds::core::status::LivelinessChangedStatus& /*s*/) {}
    /// Requested-Deadline-Missed.
    virtual void on_requested_deadline_missed(
        DataReader<T>& /*dr*/,
        const ::dds::core::status::RequestedDeadlineMissedStatus& /*s*/) {}
    /// Requested-Incompatible-QoS.
    virtual void on_requested_incompatible_qos(
        DataReader<T>& /*dr*/,
        const ::dds::core::status::RequestedIncompatibleQosStatus& /*s*/) {}
    /// Subscription-Matched.
    virtual void on_subscription_matched(
        DataReader<T>& /*dr*/,
        const ::dds::core::status::SubscriptionMatchedStatus& /*s*/) {}
    /// Sample-Lost.
    virtual void on_sample_lost(DataReader<T>& /*dr*/,
                                 const ::dds::core::status::SampleLostStatus& /*s*/) {}
};

template <typename T>
class _DataReaderListenerBridge {
public:
    static void attach(DataReader<T>& dr, DataReaderListener<T>* listener,
                       uint32_t status_mask) {
        if (listener == nullptr) {
            zerodds_dr_set_listener(dr.native_handle(), nullptr, 0);
            return;
        }
        thread_local zerodds_ZeroDdsDataReaderListener vt;
        vt.user_data = static_cast<void*>(listener);
        vt.on_data_available = &cb_data_available;
        vt.on_sample_rejected = &cb_sample_rejected;
        vt.on_liveliness_changed = &cb_liveliness_changed;
        vt.on_requested_deadline_missed = &cb_requested_deadline_missed;
        vt.on_requested_incompatible_qos = &cb_requested_incompatible_qos;
        vt.on_subscription_matched = &cb_subscription_matched;
        vt.on_sample_lost = &cb_sample_lost;
        zerodds_dr_set_listener(dr.native_handle(), &vt, status_mask);
    }

private:
    static DataReaderListener<T>* unwrap(void* ud) {
        return static_cast<DataReaderListener<T>*>(ud);
    }
    static void cb_data_available(void* ud, zerodds_ZeroDdsDataReader*) {
        if (auto l = unwrap(ud)) { DataReader<T> d; l->on_data_available(d); }
    }
    static void cb_sample_rejected(void* ud, zerodds_ZeroDdsDataReader*) {
        if (auto l = unwrap(ud)) {
            DataReader<T> d;
            ::dds::core::status::SampleRejectedStatus s;
            l->on_sample_rejected(d, s);
        }
    }
    static void cb_liveliness_changed(void* ud, zerodds_ZeroDdsDataReader*) {
        if (auto l = unwrap(ud)) {
            DataReader<T> d;
            ::dds::core::status::LivelinessChangedStatus s;
            l->on_liveliness_changed(d, s);
        }
    }
    static void cb_requested_deadline_missed(void* ud, zerodds_ZeroDdsDataReader*) {
        if (auto l = unwrap(ud)) {
            DataReader<T> d;
            ::dds::core::status::RequestedDeadlineMissedStatus s;
            l->on_requested_deadline_missed(d, s);
        }
    }
    static void cb_requested_incompatible_qos(void* ud, zerodds_ZeroDdsDataReader*) {
        if (auto l = unwrap(ud)) {
            DataReader<T> d;
            ::dds::core::status::RequestedIncompatibleQosStatus s;
            l->on_requested_incompatible_qos(d, s);
        }
    }
    static void cb_subscription_matched(void* ud, zerodds_ZeroDdsDataReader*) {
        if (auto l = unwrap(ud)) {
            DataReader<T> d;
            ::dds::core::status::SubscriptionMatchedStatus s;
            l->on_subscription_matched(d, s);
        }
    }
    static void cb_sample_lost(void* ud, zerodds_ZeroDdsDataReader*) {
        if (auto l = unwrap(ud)) {
            DataReader<T> d;
            ::dds::core::status::SampleLostStatus s;
            l->on_sample_lost(d, s);
        }
    }
};

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_DATAREADERLISTENER_HPP
