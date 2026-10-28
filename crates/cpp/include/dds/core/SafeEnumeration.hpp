// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/SafeEnumeration.hpp — safe_enum (DDS-PSM-Cxx 1.0 §7.6.1): an
// enumeration wrapper whose enumerators are the members of the `def` struct,
// as in `HistoryKind::KEEP_LAST`.

#ifndef ZERODDS_DDS_CORE_SAFEENUMERATION_HPP
#define ZERODDS_DDS_CORE_SAFEENUMERATION_HPP

namespace dds {
namespace core {

template <typename def, typename inner = typename def::type>
class safe_enum : public def {
public:
    typedef typename def::type type;
    /// The spelling of the specification text (§7.6.1).
    typedef typename def::type Type;

    safe_enum(type v) : val(v) {}
    inner underlying() const { return val; }

    bool operator==(const safe_enum& s) const { return val == s.val; }
    bool operator!=(const safe_enum& s) const { return val != s.val; }
    bool operator<(const safe_enum& s) const { return val < s.val; }
    bool operator<=(const safe_enum& s) const { return val <= s.val; }
    bool operator>(const safe_enum& s) const { return val > s.val; }
    bool operator>=(const safe_enum& s) const { return val >= s.val; }

private:
    type val;
};

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_SAFEENUMERATION_HPP
