// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/SubscriberListener.hpp — SubscriberListener (DDS-PSM-Cxx 1.0
// §7.10).

#ifndef ZERODDS_DDS_SUB_SUBSCRIBERLISTENER_HPP
#define ZERODDS_DDS_SUB_SUBSCRIBERLISTENER_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/sub/AnyDataReaderListener.hpp"

namespace dds {
namespace sub {

class Subscriber;

class SubscriberListener : public virtual AnyDataReaderListener {
public:
    virtual ~SubscriberListener() {}

    virtual void on_data_on_readers(Subscriber& sub) = 0;
};

class NoOpSubscriberListener : public virtual SubscriberListener,
                               public virtual NoOpAnyDataReaderListener {
public:
    virtual ~NoOpSubscriberListener() {}

    void on_data_on_readers(Subscriber&) ZERODDS_OVERRIDE {}
};

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_SUBSCRIBERLISTENER_HPP
