// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/detail/EntityAdapter.hpp — presents any entity reference type of
// this library as a `dds::core::Entity` (DDS-PSM-Cxx 1.0 §7.5.1.2), which
// `StatusCondition::entity()` returns.

#ifndef ZERODDS_DDS_CORE_DETAIL_ENTITYADAPTER_HPP
#define ZERODDS_DDS_CORE_DETAIL_ENTITYADAPTER_HPP


#include "zerodds/psm/cxx.hpp"
#include <memory>
#include <utility>

#include "dds/core/Entity.hpp"

namespace dds {
namespace core {
namespace detail {

/// Holds a copy of the entity reference `E` (which shares the entity) and
/// forwards the five entity operations.
template <typename E>
class EntityAdapter : public EntityDelegate {
public:
    explicit EntityAdapter(const E& e) : e_(e) {}

    void enable() ZERODDS_OVERRIDE { e_.enable(); }
    ::dds::core::status::StatusMask status_changes() const ZERODDS_OVERRIDE { return e_.status_changes(); }
    ::dds::core::InstanceHandle instance_handle() const ZERODDS_OVERRIDE { return e_.instance_handle(); }
    void close() ZERODDS_OVERRIDE { e_.close(); }
    void retain() ZERODDS_OVERRIDE { e_.retain(); }

private:
    E e_;
};

/// The `dds::core::Entity` for `e`.
template <typename E>
::dds::core::Entity as_entity(const E& e) {
    return ::dds::core::Entity(::zerodds::cxx::static_pointer_cast<EntityDelegate>(
        ::zerodds::cxx::make_shared<EntityAdapter<E> >(e)));
}

} // namespace detail
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_DETAIL_ENTITYADAPTER_HPP
