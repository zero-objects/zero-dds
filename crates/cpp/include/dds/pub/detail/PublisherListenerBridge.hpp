// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/detail/PublisherListenerBridge.hpp — binds a PublisherListener to
// the C API's listener table. Internal.

#ifndef ZERODDS_DDS_PUB_DETAIL_PUBLISHERLISTENERBRIDGE_HPP
#define ZERODDS_DDS_PUB_DETAIL_PUBLISHERLISTENERBRIDGE_HPP

#include <stdint.h>

#include "dds/pub/AnyDataWriter.hpp"
#include "dds/pub/PublisherListener.hpp"
#include "dds/pub/detail/DataWriterStatus.hpp"
#include "zerodds.h"

namespace dds {
namespace pub {

namespace detail {

class PublisherDelegate;

struct PublisherListenerBridge {
    /// `listener` is the PublisherListener the C callbacks cast their
    /// `user_data` to; the writer a callback is about arrives as an
    /// AnyDataWriter, with the status it holds.
    static void attach(PublisherDelegate& pub, PublisherListener* listener, uint32_t mask);

    static void cb_offered_deadline_missed(void* ud, zerodds_ZeroDdsDataWriter* raw) {
        if (PublisherListener* l = static_cast<PublisherListener*>(ud)) {
            AnyDataWriter w = make_native_any_writer(raw);
            l->on_offered_deadline_missed(w, offered_deadline_missed_status_of(raw));
        }
    }
    static void cb_liveliness_lost(void* ud, zerodds_ZeroDdsDataWriter* raw) {
        if (PublisherListener* l = static_cast<PublisherListener*>(ud)) {
            AnyDataWriter w = make_native_any_writer(raw);
            l->on_liveliness_lost(w, liveliness_lost_status_of(raw));
        }
    }
    static void cb_offered_incompatible_qos(void* ud, zerodds_ZeroDdsDataWriter* raw) {
        if (PublisherListener* l = static_cast<PublisherListener*>(ud)) {
            AnyDataWriter w = make_native_any_writer(raw);
            l->on_offered_incompatible_qos(w, offered_incompatible_qos_status_of(raw));
        }
    }
    static void cb_publication_matched(void* ud, zerodds_ZeroDdsDataWriter* raw) {
        if (PublisherListener* l = static_cast<PublisherListener*>(ud)) {
            AnyDataWriter w = make_native_any_writer(raw);
            l->on_publication_matched(w, publication_matched_status_of(raw));
        }
    }
};

} // namespace detail
} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_DETAIL_PUBLISHERLISTENERBRIDGE_HPP
