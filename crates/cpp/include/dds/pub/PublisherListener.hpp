// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/PublisherListener.hpp — PublisherListener (DDS-PSM-Cxx 1.0 §7.9).

#ifndef ZERODDS_DDS_PUB_PUBLISHERLISTENER_HPP
#define ZERODDS_DDS_PUB_PUBLISHERLISTENER_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/pub/AnyDataWriterListener.hpp"

namespace dds {
namespace pub {

class PublisherListener : public virtual AnyDataWriterListener {
public:
    virtual ~PublisherListener() {}
};

class NoOpPublisherListener : public virtual PublisherListener,
                              public virtual NoOpAnyDataWriterListener {
public:
    virtual ~NoOpPublisherListener() {}
};

} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_PUBLISHERLISTENER_HPP
