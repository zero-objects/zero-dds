// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/TEntityQos.hpp — TEntityQos (DDS-PSM-Cxx 1.0 §7.6.2): a container
// of QoS policies that sets and gets all the policies of an entity as a unit,
// composed with `qos << Policy`, converted between entity kinds by copying
// the policies both have (`DataWriterQos dwqos(tqos)`).

#ifndef ZERODDS_DDS_CORE_TENTITYQOS_HPP
#define ZERODDS_DDS_CORE_TENTITYQOS_HPP


#include "zerodds/psm/cxx.hpp"

#if ZERODDS_CXX11
#define ZERODDS_QOS_CONSTEXPR constexpr
#else
#define ZERODDS_QOS_CONSTEXPR
#endif

namespace dds {
namespace core {

template <typename DELEGATE>
class TEntityQos;

namespace detail {

/// The placeholder of the unused parameters of `PolicySet` and the end of its list.
struct NoPolicy {};

/// One cell of the list that holds the policies: the value and the rest of the list.
template <typename Head, typename Tail>
struct PolicyCell {
    PolicyCell() : head(), tail() {}
    Head head;
    Tail tail;
};

/// Whether the list `Cell` holds a policy of type `P`.
template <typename Cell, typename P>
struct PolicyContains : ::zerodds::cxx::false_type {};
template <typename H, typename T, typename P>
struct PolicyContains<PolicyCell<H, T>, P>
    : ::zerodds::cxx::integral_constant<
          bool, (::zerodds::cxx::is_same<H, P>::value && !::zerodds::cxx::is_same<P, NoPolicy>::value) ||
                    PolicyContains<T, P>::value> {};

/// Access to the policy of type `P` in the list `Cell`.
template <typename P, typename Cell>
struct PolicyAccess;
template <typename P, typename T>
struct PolicyAccess<P, PolicyCell<P, T> > {
    static P& get(PolicyCell<P, T>& cell) { return cell.head; }
    static const P& get(const PolicyCell<P, T>& cell) { return cell.head; }
};
template <typename P, typename H, typename T>
struct PolicyAccess<P, PolicyCell<H, T> > {
    static P& get(PolicyCell<H, T>& cell) { return PolicyAccess<P, T>::get(cell.tail); }
    static const P& get(const PolicyCell<H, T>& cell) {
        return PolicyAccess<P, T>::get(cell.tail);
    }
};

/// The policy set of a QoS kind, for `TEntityQos` and `PolicySetOf`.
template <typename Q>
struct PolicySetOf;
template <typename D>
struct PolicySetOf<TEntityQos<D> > {
    typedef D type;
};

/// Copies `src`'s policy of type `H` into `dst` when the QoS kind of `Other` has it.
template <typename H, typename Other>
void assign_policy(H& dst, const Other& other, ::zerodds::cxx::true_type) {
    dst = other.template policy<H>();
}
template <typename H, typename Other>
void assign_policy(H&, const Other&, ::zerodds::cxx::false_type) {}

/// Copies, for every policy of the list, the one `Other` has too.
template <typename Cell>
struct PolicyCopy {
    template <typename Other>
    static void run(Cell&, const Other&) {}
};
template <typename H, typename T>
struct PolicyCopy<PolicyCell<H, T> > {
    template <typename Other>
    static void run(PolicyCell<H, T>& cell, const Other& other) {
        typedef typename PolicySetOf<Other>::type OtherSet;
        assign_policy(cell.head, other,
                      ::zerodds::cxx::integral_constant<bool, OtherSet::template contains<H>::value>());
        PolicyCopy<T>::run(cell.tail, other);
    }
};

/// The policies a QoS kind holds, one value each (up to 16 kinds of policy; the unused
/// parameters stay `NoPolicy`).
template <typename P1, typename P2 = NoPolicy, typename P3 = NoPolicy, typename P4 = NoPolicy,
          typename P5 = NoPolicy, typename P6 = NoPolicy, typename P7 = NoPolicy,
          typename P8 = NoPolicy, typename P9 = NoPolicy, typename P10 = NoPolicy,
          typename P11 = NoPolicy, typename P12 = NoPolicy, typename P13 = NoPolicy,
          typename P14 = NoPolicy, typename P15 = NoPolicy, typename P16 = NoPolicy>
class PolicySet {
public:
    // The list P1, P2, ..., P16, NoPolicy.
    typedef PolicyCell<P16, NoPolicy> cells16_;
    typedef PolicyCell<P15, cells16_> cells15_;
    typedef PolicyCell<P14, cells15_> cells14_;
    typedef PolicyCell<P13, cells14_> cells13_;
    typedef PolicyCell<P12, cells13_> cells12_;
    typedef PolicyCell<P11, cells12_> cells11_;
    typedef PolicyCell<P10, cells11_> cells10_;
    typedef PolicyCell<P9, cells10_> cells9_;
    typedef PolicyCell<P8, cells9_> cells8_;
    typedef PolicyCell<P7, cells8_> cells7_;
    typedef PolicyCell<P6, cells7_> cells6_;
    typedef PolicyCell<P5, cells6_> cells5_;
    typedef PolicyCell<P4, cells5_> cells4_;
    typedef PolicyCell<P3, cells4_> cells3_;
    typedef PolicyCell<P2, cells3_> cells2_;
    typedef PolicyCell<P1, cells2_> cells_type;

    /// Compile-time: whether the set holds a policy of type `P`.
    template <typename P>
    struct contains : PolicyContains<cells_type, P> {};

    template <typename P>
    static ZERODDS_QOS_CONSTEXPR bool has() {
        return contains<P>::value;
    }

    template <typename P>
    const P& get() const {
        ZERODDS_STATIC_ASSERT((contains<P>::value), "this QoS kind has no such policy");
        return PolicyAccess<P, cells_type>::get(policies_);
    }
    template <typename P>
    P& get() {
        ZERODDS_STATIC_ASSERT((contains<P>::value), "this QoS kind has no such policy");
        return PolicyAccess<P, cells_type>::get(policies_);
    }

    /// Copies every policy `other` has too.
    template <typename Other>
    void copy_common_from(const Other& other) {
        PolicyCopy<cells_type>::run(policies_, other);
    }

private:
    cells_type policies_;
};

} // namespace detail

template <typename DELEGATE>
class TEntityQos {
public:
    TEntityQos() {}
#if ZERODDS_CXX11
    TEntityQos(const TEntityQos& other) = default;
    TEntityQos(TEntityQos&&) = default;
#endif
    /// Copies the policies this QoS kind shares with `qos`; the others keep
    /// their defaults.
    template <typename T>
    TEntityQos(const TEntityQos<T>& qos) {
        delegate_.copy_common_from(qos);
    }
#if ZERODDS_CXX11
    ~TEntityQos() = default;
#endif

    template <typename P>
    static ZERODDS_QOS_CONSTEXPR bool has_policy() {
        return DELEGATE::template contains<P>::value;
    }

    /// Sets a policy this QoS kind has.
    template <typename POLICY>
    TEntityQos& policy(const POLICY& p) {
        delegate_.template get<POLICY>() = p;
        return *this;
    }
    /// The value of a policy this QoS kind has.
    template <typename POLICY>
    const POLICY& policy() const {
        return delegate_.template get<POLICY>();
    }
    template <typename POLICY>
    POLICY& policy() {
        return delegate_.template get<POLICY>();
    }

    template <typename POLICY>
    TEntityQos& operator<<(const POLICY& p) {
        return policy(p);
    }
    template <typename POLICY>
    const TEntityQos& operator>>(POLICY& p) const {
        p = policy<POLICY>();
        return *this;
    }

#if ZERODDS_CXX11
    TEntityQos& operator=(const TEntityQos& other) = default;
#endif

#if ZERODDS_CXX11
    TEntityQos& operator=(TEntityQos&&) = default;
#endif
    template <typename T>
    TEntityQos& operator=(const TEntityQos<T>& other) {
        delegate_.copy_common_from(other);
        return *this;
    }

private:
    DELEGATE delegate_;
};

} // namespace core
} // namespace dds

#undef ZERODDS_QOS_CONSTEXPR

#endif // ZERODDS_DDS_CORE_TENTITYQOS_HPP
