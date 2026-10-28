// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/detail/Entity.hpp — Entity delegate base.

#ifndef ZERODDS_DDS_CORE_DETAIL_ENTITY_HPP
#define ZERODDS_DDS_CORE_DETAIL_ENTITY_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/TEntity.hpp"

namespace dds {
namespace core {
namespace detail {

// Base delegate for all entities.
class EntityDelegate {
public:
    virtual ~EntityDelegate() {}
    virtual void enable() = 0;
    virtual ::dds::core::status::StatusMask status_changes() const = 0;
    virtual ::dds::core::InstanceHandle instance_handle() const = 0;
    virtual void close() = 0;
    virtual void retain() = 0;
};

typedef TEntity<EntityDelegate> Entity;

} // namespace detail
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_DETAIL_ENTITY_HPP