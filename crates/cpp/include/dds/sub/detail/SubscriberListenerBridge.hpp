// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/detail/SubscriberListenerBridge.hpp — binds a SubscriberListener to
// the C API's listener table. Internal.

#ifndef ZERODDS_DDS_SUB_DETAIL_SUBSCRIBERLISTENERBRIDGE_HPP
#define ZERODDS_DDS_SUB_DETAIL_SUBSCRIBERLISTENERBRIDGE_HPP

#include <stdint.h>

#include "dds/sub/AnyDataReader.hpp"
#include "dds/sub/SubscriberListener.hpp"
#include "dds/sub/detail/DataReaderStatus.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {

class Subscriber;

namespace detail {

class SubscriberDelegate;

struct SubscriberListenerBridge {
    static void attach(SubscriberDelegate& sub, SubscriberListener* listener, uint32_t mask);

    static void cb_data_on_readers(void* ud, zerodds_ZeroDdsSubscriber* raw);

    static void cb_sample_lost(void* ud, zerodds_ZeroDdsDataReader* raw) {
        if (SubscriberListener* l = static_cast<SubscriberListener*>(ud)) {
            AnyDataReader r = make_native_any_reader(raw);
            l->on_sample_lost(r, sample_lost_status_of(raw));
        }
    }
    static void cb_sample_rejected(void* ud, zerodds_ZeroDdsDataReader* raw) {
        if (SubscriberListener* l = static_cast<SubscriberListener*>(ud)) {
            AnyDataReader r = make_native_any_reader(raw);
            l->on_sample_rejected(r, sample_rejected_status_of(raw));
        }
    }
    static void cb_liveliness_changed(void* ud, zerodds_ZeroDdsDataReader* raw) {
        if (SubscriberListener* l = static_cast<SubscriberListener*>(ud)) {
            AnyDataReader r = make_native_any_reader(raw);
            l->on_liveliness_changed(r, liveliness_changed_status_of(raw));
        }
    }
    static void cb_subscription_matched(void* ud, zerodds_ZeroDdsDataReader* raw) {
        if (SubscriberListener* l = static_cast<SubscriberListener*>(ud)) {
            AnyDataReader r = make_native_any_reader(raw);
            l->on_subscription_matched(r, subscription_matched_status_of(raw));
        }
    }
    static void cb_requested_deadline_missed(void* ud, zerodds_ZeroDdsDataReader* raw) {
        if (SubscriberListener* l = static_cast<SubscriberListener*>(ud)) {
            AnyDataReader r = make_native_any_reader(raw);
            l->on_requested_deadline_missed(r, requested_deadline_missed_status_of(raw));
        }
    }
    static void cb_requested_incompatible_qos(void* ud, zerodds_ZeroDdsDataReader* raw) {
        if (SubscriberListener* l = static_cast<SubscriberListener*>(ud)) {
            AnyDataReader r = make_native_any_reader(raw);
            l->on_requested_incompatible_qos(r, requested_incompatible_qos_status_of(raw));
        }
    }
    static void cb_data_available(void* ud, zerodds_ZeroDdsDataReader* raw) {
        if (SubscriberListener* l = static_cast<SubscriberListener*>(ud)) {
            AnyDataReader r = make_native_any_reader(raw);
            l->on_data_available(r);
        }
    }
};

} // namespace detail
} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_DETAIL_SUBSCRIBERLISTENERBRIDGE_HPP
