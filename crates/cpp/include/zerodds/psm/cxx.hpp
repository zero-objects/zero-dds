// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds/psm/cxx.hpp — what the headers of `dds/` need from the C++ standard library and the
// language, in a form that compiles as ISO/IEC C++ 2003 (DDS-PSM-Cxx 1.0 §3 [C++], §7.1: "This
// specification relies on C++03 features only").
//
// With a C++11 (or later) compiler every name here is the one of the standard library
// (`::zerodds::cxx::shared_ptr` is `std::shared_ptr`). With a C++03 compiler the same names are
// small own implementations: reference counting (`shared_ptr`, `weak_ptr`), `mutex`, `function`,
// `array`, `type_index` and the type traits the headers use. The C++11 additions of the
// specification (§7.12: move operations, initializer lists, ...) are written behind
// `#if ZERODDS_CXX11`.
//
// Macros:
//   ZERODDS_CXX11        1 when the compiler is C++11 or later, else 0
//   ZERODDS_NOEXCEPT     `noexcept`, in C++03 `throw()`
//   ZERODDS_OVERRIDE     `override` or nothing
//   ZERODDS_FINAL        `final` or nothing
//   ZERODDS_MOVE(x)      `std::move(x)` or `(x)`
//   ZERODDS_STATIC_ASSERT(cond, message)
//   ZERODDS_NONCOPYABLE(Class)   declares the copy operations deleted / private, ends in `public:`
//   nullptr              a null pointer constant also in C++03 (`::zerodds::cxx::nullptr_t`)
//   A condition of ZERODDS_STATIC_ASSERT that contains a comma is written in parentheses.

#ifndef ZERODDS_PSM_CXX_HPP
#define ZERODDS_PSM_CXX_HPP

#include <limits.h>
#include <stddef.h>
#include <stdint.h>
#include <time.h>

#include <algorithm>
#include <cstdlib>
#include <exception>
#include <new>
#include <string>
#include <sstream>
#include <stdexcept>
#include <typeinfo>

#if defined(_WIN32)
#include <windows.h>
#else
#include <pthread.h>
#endif

#if (defined(__cplusplus) && __cplusplus >= 201103L) || (defined(_MSC_VER) && _MSC_VER >= 1900)
#define ZERODDS_CXX11 1
#else
#define ZERODDS_CXX11 0
#endif

#if ZERODDS_CXX11
#include <array>
#include <functional>
#include <memory>
#include <mutex>
#include <type_traits>
#include <typeindex>
#define ZERODDS_NOEXCEPT noexcept
#define ZERODDS_OVERRIDE override
#define ZERODDS_FINAL final
#define ZERODDS_MOVE(x) std::move(x)
#define ZERODDS_STATIC_ASSERT(cond, message) static_assert(cond, message)
#define ZERODDS_NONCOPYABLE(Class)                \
    Class(const Class&) = delete;                 \
    Class& operator=(const Class&) = delete;
#else
#define ZERODDS_NOEXCEPT throw()
#define ZERODDS_OVERRIDE
#define ZERODDS_FINAL
#define ZERODDS_MOVE(x) (x)
#define ZERODDS_STATIC_ASSERT_JOIN2(a, b) a##b
#define ZERODDS_STATIC_ASSERT_JOIN(a, b) ZERODDS_STATIC_ASSERT_JOIN2(a, b)
#define ZERODDS_STATIC_ASSERT(cond, message)                                              \
    typedef char ZERODDS_STATIC_ASSERT_JOIN(zerodds_static_assert_, __LINE__)[(cond) ? 1 : -1] \
        ZERODDS_UNUSED_TYPEDEF
#define ZERODDS_NONCOPYABLE(Class) \
private:                           \
    Class(const Class&);           \
    Class& operator=(const Class&); \
public:
#if defined(__GNUC__)
#define ZERODDS_UNUSED_TYPEDEF __attribute__((unused))
#else
#define ZERODDS_UNUSED_TYPEDEF
#endif
#endif

namespace zerodds {
namespace cxx {

// ---- nullptr ---------------------------------------------------------------------------------

#if ZERODDS_CXX11 || defined(__clang__) || (defined(_MSC_VER) && _MSC_VER >= 1600)
// The compiler has `nullptr` (clang accepts it in C++03 as an extension).
typedef __typeof__(nullptr) nullptr_t;
#define ZERODDS_OWN_NULLPTR 0
#else
#define ZERODDS_OWN_NULLPTR 1
/// The type of the null pointer constant in C++03 (the one of Meyers, "Effective C++").
class nullptr_t {
public:
    template <typename T>
    operator T*() const { return 0; }
    template <typename C, typename T>
    operator T C::*() const { return 0; }

private:
    void operator&() const;
};
#endif

}  // namespace cxx
}  // namespace zerodds

#if ZERODDS_OWN_NULLPTR
/// `nullptr` for a C++03 compiler without it: an object, not a macro, so that the headers of the
/// standard library included after this one are not affected. (`nullptr` is not a keyword in C++03.)
#if defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wc++11-compat"
#endif
static const ::zerodds::cxx::nullptr_t nullptr = ::zerodds::cxx::nullptr_t();
#if defined(__GNUC__)
#pragma GCC diagnostic pop
#endif
#endif

namespace zerodds {
namespace cxx {

#if ZERODDS_CXX11

// ---- C++11 and later: the standard library ---------------------------------------------------

using std::shared_ptr;
using std::weak_ptr;
using std::make_shared;
using std::static_pointer_cast;
using std::dynamic_pointer_cast;
using std::const_pointer_cast;
using std::mutex;
using std::lock_guard;
using std::function;
using std::type_index;
using std::array;
using std::get;
using std::enable_if;
using std::is_same;
using std::is_integral;
using std::is_signed;
using std::is_unsigned;
using std::is_floating_point;
using std::is_base_of;
using std::is_convertible;
using std::integral_constant;
using std::true_type;
using std::false_type;

#else

// ---- C++03: own implementations --------------------------------------------------------------

// Type traits

template <typename T, T V>
struct integral_constant {
    static const T value = V;
    typedef T value_type;
    typedef integral_constant<T, V> type;
};
template <typename T, T V>
const T integral_constant<T, V>::value;
typedef integral_constant<bool, true> true_type;
typedef integral_constant<bool, false> false_type;

template <bool B, typename T = void>
struct enable_if {};
template <typename T>
struct enable_if<true, T> {
    typedef T type;
};

template <typename A, typename B>
struct is_same : false_type {};
template <typename A>
struct is_same<A, A> : true_type {};

namespace detail {
template <typename T> struct is_integral_base : false_type {};
template <> struct is_integral_base<bool> : true_type {};
template <> struct is_integral_base<char> : true_type {};
template <> struct is_integral_base<signed char> : true_type {};
template <> struct is_integral_base<unsigned char> : true_type {};
template <> struct is_integral_base<wchar_t> : true_type {};
template <> struct is_integral_base<short> : true_type {};
template <> struct is_integral_base<unsigned short> : true_type {};
template <> struct is_integral_base<int> : true_type {};
template <> struct is_integral_base<unsigned int> : true_type {};
template <> struct is_integral_base<long> : true_type {};
template <> struct is_integral_base<unsigned long> : true_type {};
#if defined(__GNUC__) || defined(_MSC_VER)
template <> struct is_integral_base<long long> : true_type {};
template <> struct is_integral_base<unsigned long long> : true_type {};
#endif
template <typename T> struct remove_cv_t { typedef T type; };
template <typename T> struct remove_cv_t<const T> { typedef T type; };
template <typename T> struct remove_cv_t<volatile T> { typedef T type; };
template <typename T> struct remove_cv_t<const volatile T> { typedef T type; };
template <typename T> struct is_float_base : false_type {};
template <> struct is_float_base<float> : true_type {};
template <> struct is_float_base<double> : true_type {};
template <> struct is_float_base<long double> : true_type {};
}  // namespace detail

template <typename T>
struct is_integral : detail::is_integral_base<typename detail::remove_cv_t<T>::type> {};
template <typename T>
struct is_floating_point : detail::is_float_base<typename detail::remove_cv_t<T>::type> {};
namespace detail {
/// The sign of an integer type; `T(-1)` is not evaluated for a type that is not an integer.
template <typename T, bool Integral>
struct is_negative_minus_one : false_type {};
template <typename T>
struct is_negative_minus_one<T, true> : integral_constant<bool, (T(-1) < T(0))> {};
}  // namespace detail

template <typename T>
struct is_signed : detail::is_negative_minus_one<T, is_integral<T>::value> {};
template <typename T>
struct is_unsigned
    : integral_constant<bool, is_integral<T>::value && !detail::is_negative_minus_one<T, is_integral<T>::value>::value> {};

namespace detail {
typedef char yes_type;
struct no_type { char pad[2]; };
template <typename B>
yes_type base_test(const volatile B*);
template <typename B>
no_type base_test(const volatile void*);
template <typename B, typename D>
struct is_base_of_impl {
    struct Host {
        operator const volatile B*() const;
        operator const volatile D*();
    };
    template <typename X>
    static yes_type check(const volatile D*, X);
    static no_type check(const volatile B*, int);
    static const bool value = sizeof(check(Host(), 0)) == sizeof(yes_type);
};
/// `T&` also for a `T` that is a reference already (no reference to reference in C++03).
template <typename T>
struct lvalue_ref {
    typedef T& type;
};
template <typename T>
struct lvalue_ref<T&> {
    typedef T& type;
};
template <typename From, typename To>
struct is_convertible_impl {
    static yes_type test(To);
    static no_type test(...);
    static typename lvalue_ref<From>::type make();
    static const bool value = sizeof(test(make())) == sizeof(yes_type);
};
}  // namespace detail

template <typename B, typename D>
struct is_base_of : integral_constant<bool, detail::is_base_of_impl<B, D>::value || is_same<B, D>::value> {};
template <typename From, typename To>
struct is_convertible : integral_constant<bool, detail::is_convertible_impl<From, To>::value> {};

// Reference counting: shared_ptr, weak_ptr

class bad_weak_ptr : public std::exception {
public:
    virtual const char* what() const throw() { return "bad_weak_ptr"; }
};

namespace detail {

inline long atomic_add(volatile long* p, long d) {
#if defined(__GNUC__)
    return __sync_add_and_fetch(p, d);
#elif defined(_MSC_VER)
    return _InterlockedExchangeAdd(p, d) + d;
#else
#error "zerodds/psm/cxx.hpp: no atomic operation for this C++03 compiler"
#endif
}

/// The control block: the strong count and the weak count (plus one while strong ones exist).
class counted_base {
public:
    counted_base() : use_(1), weak_(1) {}
    virtual ~counted_base() {}
    virtual void dispose() = 0;
    void add_ref() { atomic_add(&use_, 1); }
    bool add_ref_lock() {
        for (;;) {
            long n = use_;
            if (n == 0) {
                return false;
            }
#if defined(__GNUC__)
            if (__sync_bool_compare_and_swap(&use_, n, n + 1)) {
                return true;
            }
#else
            atomic_add(&use_, 1);
            return true;
#endif
        }
    }
    void release() {
        if (atomic_add(&use_, -1) == 0) {
            dispose();
            weak_release();
        }
    }
    void weak_add_ref() { atomic_add(&weak_, 1); }
    void weak_release() {
        if (atomic_add(&weak_, -1) == 0) {
            delete this;
        }
    }
    long use_count() const { return use_; }

private:
    counted_base(const counted_base&);
    counted_base& operator=(const counted_base&);
    volatile long use_;
    volatile long weak_;
};

template <typename P, typename D>
class counted_deleter : public counted_base {
public:
    counted_deleter(P p, D d) : p_(p), d_(d) {}
    virtual void dispose() { d_(p_); }

private:
    P p_;
    D d_;
};

template <typename T>
struct default_delete_ {
    void operator()(T* p) const { delete p; }
};

/// `shared_ptr<T>` stores `T*`; `shared_ptr<void>` has no reference to `void`.
template <typename T>
struct ref_of {
    typedef T& type;
};
template <>
struct ref_of<void> {
    typedef void type;
};
template <>
struct ref_of<const void> {
    typedef void type;
};

struct safe_bool_helper {
    void member() const {}
};
typedef void (safe_bool_helper::*safe_bool)() const;

}  // namespace detail

template <typename T>
class weak_ptr;

template <typename T>
class shared_ptr {
public:
    typedef T element_type;

    shared_ptr() : px_(0), pn_(0) {}
    shared_ptr(nullptr_t) : px_(0), pn_(0) {}
    template <typename Y>
    explicit shared_ptr(Y* p) : px_(p), pn_(0) {
        try {
            pn_ = new detail::counted_deleter<Y*, detail::default_delete_<Y> >(p, detail::default_delete_<Y>());
        } catch (...) {
            delete p;
            throw;
        }
    }
    template <typename Y, typename D>
    shared_ptr(Y* p, D d) : px_(p), pn_(0) {
        try {
            pn_ = new detail::counted_deleter<Y*, D>(p, d);
        } catch (...) {
            d(p);
            throw;
        }
    }
    shared_ptr(const shared_ptr& other) : px_(other.px_), pn_(other.pn_) {
        if (pn_ != 0) {
            pn_->add_ref();
        }
    }
    template <typename Y>
    shared_ptr(const shared_ptr<Y>& other) : px_(other.px_), pn_(other.pn_) {
        if (pn_ != 0) {
            pn_->add_ref();
        }
    }
    /// The aliasing constructor: shares `other`'s ownership, points at `p`.
    template <typename Y>
    shared_ptr(const shared_ptr<Y>& other, T* p) : px_(p), pn_(other.pn_) {
        if (pn_ != 0) {
            pn_->add_ref();
        }
    }
    template <typename Y>
    explicit shared_ptr(const weak_ptr<Y>& weak) : px_(0), pn_(0) {
        if (weak.pn_ == 0 || !weak.pn_->add_ref_lock()) {
            throw bad_weak_ptr();
        }
        px_ = weak.px_;
        pn_ = weak.pn_;
    }
    ~shared_ptr() {
        if (pn_ != 0) {
            pn_->release();
        }
    }
    shared_ptr& operator=(const shared_ptr& other) {
        shared_ptr(other).swap(*this);
        return *this;
    }
    template <typename Y>
    shared_ptr& operator=(const shared_ptr<Y>& other) {
        shared_ptr(other).swap(*this);
        return *this;
    }
    shared_ptr& operator=(nullptr_t) {
        shared_ptr().swap(*this);
        return *this;
    }

    void reset() { shared_ptr().swap(*this); }
    template <typename Y>
    void reset(Y* p) {
        shared_ptr(p).swap(*this);
    }
    template <typename Y, typename D>
    void reset(Y* p, D d) {
        shared_ptr(p, d).swap(*this);
    }
    void swap(shared_ptr& other) {
        std::swap(px_, other.px_);
        std::swap(pn_, other.pn_);
    }

    T* get() const { return px_; }
    typename detail::ref_of<T>::type operator*() const { return *px_; }
    T* operator->() const { return px_; }
    long use_count() const { return pn_ != 0 ? pn_->use_count() : 0; }
    bool unique() const { return use_count() == 1; }
    operator detail::safe_bool() const { return px_ != 0 ? &detail::safe_bool_helper::member : 0; }

    template <typename Y>
    bool owner_before(const shared_ptr<Y>& other) const { return pn_ < other.pn_; }

    // public for the conversions of the other specialisations
    T* px_;
    detail::counted_base* pn_;
};

template <typename T>
class weak_ptr {
public:
    typedef T element_type;

    weak_ptr() : px_(0), pn_(0) {}
    weak_ptr(const weak_ptr& other) : px_(other.px_), pn_(other.pn_) {
        if (pn_ != 0) {
            pn_->weak_add_ref();
        }
    }
    template <typename Y>
    weak_ptr(const weak_ptr<Y>& other) : px_(other.px_), pn_(other.pn_) {
        if (pn_ != 0) {
            pn_->weak_add_ref();
        }
    }
    template <typename Y>
    weak_ptr(const shared_ptr<Y>& other) : px_(other.px_), pn_(other.pn_) {
        if (pn_ != 0) {
            pn_->weak_add_ref();
        }
    }
    ~weak_ptr() {
        if (pn_ != 0) {
            pn_->weak_release();
        }
    }
    weak_ptr& operator=(const weak_ptr& other) {
        weak_ptr(other).swap(*this);
        return *this;
    }
    template <typename Y>
    weak_ptr& operator=(const weak_ptr<Y>& other) {
        weak_ptr(other).swap(*this);
        return *this;
    }
    template <typename Y>
    weak_ptr& operator=(const shared_ptr<Y>& other) {
        weak_ptr(other).swap(*this);
        return *this;
    }
    void reset() { weak_ptr().swap(*this); }
    void swap(weak_ptr& other) {
        std::swap(px_, other.px_);
        std::swap(pn_, other.pn_);
    }
    long use_count() const { return pn_ != 0 ? pn_->use_count() : 0; }
    bool expired() const { return use_count() == 0; }
    shared_ptr<T> lock() const {
        shared_ptr<T> out;
        if (pn_ != 0 && pn_->add_ref_lock()) {
            out.px_ = px_;
            out.pn_ = pn_;
        }
        return out;
    }
    template <typename Y>
    bool owner_before(const weak_ptr<Y>& other) const { return pn_ < other.pn_; }

    T* px_;
    detail::counted_base* pn_;
};

template <typename A, typename B>
bool operator==(const shared_ptr<A>& a, const shared_ptr<B>& b) { return a.get() == b.get(); }
template <typename A, typename B>
bool operator!=(const shared_ptr<A>& a, const shared_ptr<B>& b) { return a.get() != b.get(); }
template <typename A, typename B>
bool operator<(const shared_ptr<A>& a, const shared_ptr<B>& b) { return a.get() < b.get(); }
template <typename A>
bool operator==(const shared_ptr<A>& a, nullptr_t) { return a.get() == 0; }
template <typename A>
bool operator==(nullptr_t, const shared_ptr<A>& a) { return a.get() == 0; }
template <typename A>
bool operator!=(const shared_ptr<A>& a, nullptr_t) { return a.get() != 0; }
template <typename A>
bool operator!=(nullptr_t, const shared_ptr<A>& a) { return a.get() != 0; }

template <typename T, typename U>
shared_ptr<T> static_pointer_cast(const shared_ptr<U>& p) {
    return shared_ptr<T>(p, static_cast<T*>(p.get()));
}
template <typename T, typename U>
shared_ptr<T> const_pointer_cast(const shared_ptr<U>& p) {
    return shared_ptr<T>(p, const_cast<T*>(p.get()));
}
template <typename T, typename U>
shared_ptr<T> dynamic_pointer_cast(const shared_ptr<U>& p) {
    T* raw = dynamic_cast<T*>(p.get());
    return raw != 0 ? shared_ptr<T>(p, raw) : shared_ptr<T>();
}

template <typename T>
shared_ptr<T> make_shared() {
    return shared_ptr<T>(new T());
}
template <typename T, typename A1>
shared_ptr<T> make_shared(const A1& a1) {
    return shared_ptr<T>(new T(a1));
}
template <typename T, typename A1, typename A2>
shared_ptr<T> make_shared(const A1& a1, const A2& a2) {
    return shared_ptr<T>(new T(a1, a2));
}
template <typename T, typename A1, typename A2, typename A3>
shared_ptr<T> make_shared(const A1& a1, const A2& a2, const A3& a3) {
    return shared_ptr<T>(new T(a1, a2, a3));
}
template <typename T, typename A1, typename A2, typename A3, typename A4>
shared_ptr<T> make_shared(const A1& a1, const A2& a2, const A3& a3, const A4& a4) {
    return shared_ptr<T>(new T(a1, a2, a3, a4));
}
template <typename T, typename A1, typename A2, typename A3, typename A4, typename A5>
shared_ptr<T> make_shared(const A1& a1, const A2& a2, const A3& a3, const A4& a4, const A5& a5) {
    return shared_ptr<T>(new T(a1, a2, a3, a4, a5));
}
template <typename T, typename A1, typename A2, typename A3, typename A4, typename A5, typename A6>
shared_ptr<T> make_shared(const A1& a1, const A2& a2, const A3& a3, const A4& a4, const A5& a5,
                          const A6& a6) {
    return shared_ptr<T>(new T(a1, a2, a3, a4, a5, a6));
}
template <typename T, typename A1, typename A2, typename A3, typename A4, typename A5, typename A6,
          typename A7>
shared_ptr<T> make_shared(const A1& a1, const A2& a2, const A3& a3, const A4& a4, const A5& a5,
                          const A6& a6, const A7& a7) {
    return shared_ptr<T>(new T(a1, a2, a3, a4, a5, a6, a7));
}
template <typename T, typename A1, typename A2, typename A3, typename A4, typename A5, typename A6,
          typename A7, typename A8>
shared_ptr<T> make_shared(const A1& a1, const A2& a2, const A3& a3, const A4& a4, const A5& a5,
                          const A6& a6, const A7& a7, const A8& a8) {
    return shared_ptr<T>(new T(a1, a2, a3, a4, a5, a6, a7, a8));
}

// Mutex: POSIX threads, or the critical section of Windows

}  // namespace cxx
}  // namespace zerodds

namespace zerodds {
namespace cxx {

class mutex {
public:
    mutex() {
#if defined(_WIN32)
        InitializeCriticalSection(&m_);
#else
        pthread_mutex_init(&m_, 0);
#endif
    }
    ~mutex() {
#if defined(_WIN32)
        DeleteCriticalSection(&m_);
#else
        pthread_mutex_destroy(&m_);
#endif
    }
    void lock() {
#if defined(_WIN32)
        EnterCriticalSection(&m_);
#else
        pthread_mutex_lock(&m_);
#endif
    }
    void unlock() {
#if defined(_WIN32)
        LeaveCriticalSection(&m_);
#else
        pthread_mutex_unlock(&m_);
#endif
    }

private:
    mutex(const mutex&);
    mutex& operator=(const mutex&);
#if defined(_WIN32)
    CRITICAL_SECTION m_;
#else
    pthread_mutex_t m_;
#endif
};

template <typename M>
class lock_guard {
public:
    explicit lock_guard(M& m) : m_(m) { m_.lock(); }
    ~lock_guard() { m_.unlock(); }

private:
    lock_guard(const lock_guard&);
    lock_guard& operator=(const lock_guard&);
    M& m_;
};

// type_index: `typeid` as the key of an associative container

class type_index {
public:
    type_index(const std::type_info& info) : info_(&info) {}
    bool operator==(const type_index& o) const { return *info_ == *o.info_; }
    bool operator!=(const type_index& o) const { return !(*this == o); }
    bool operator<(const type_index& o) const { return info_->before(*o.info_) != 0; }
    const char* name() const { return info_->name(); }

private:
    const std::type_info* info_;
};

// function<R(A...)>: function objects and function pointers, up to two arguments

class bad_function_call : public std::exception {
public:
    virtual const char* what() const throw() { return "bad_function_call"; }
};

template <typename Sig>
class function;

namespace detail {
/// The ownership half of the three `function` specialisations: one heap
/// holder, cloned on copy, released on destruction. `held()` throws
/// `bad_function_call` on an empty function, as `std::function` does.
template <typename HolderBase>
class function_storage {
public:
    operator safe_bool() const { return h_ != 0 ? &safe_bool_helper::member : 0; }

protected:
    function_storage() : h_(0) {}
    explicit function_storage(HolderBase* h) : h_(h) {}
    function_storage(const function_storage& o) : h_(o.h_ != 0 ? o.h_->clone() : 0) {}
    function_storage& operator=(const function_storage& o) {
        function_storage(o).swap_storage(*this);
        return *this;
    }
    ~function_storage() { delete h_; }
    void swap_storage(function_storage& o) { std::swap(h_, o.h_); }
    HolderBase& held() const {
        if (h_ == 0) {
            throw bad_function_call();
        }
        return *h_;
    }

private:
    HolderBase* h_;
};

template <typename R>
struct function_holder0 {
    virtual ~function_holder0() {}
    virtual R call() = 0;
    virtual function_holder0* clone() const = 0;
};
template <typename R, typename A1>
struct function_holder1 {
    virtual ~function_holder1() {}
    virtual R call(A1) = 0;
    virtual function_holder1* clone() const = 0;
};
template <typename R, typename A1, typename A2>
struct function_holder2 {
    virtual ~function_holder2() {}
    virtual R call(A1, A2) = 0;
    virtual function_holder2* clone() const = 0;
};
} // namespace detail

template <typename R>
class function<R()> : public detail::function_storage<detail::function_holder0<R> > {
    typedef detail::function_holder0<R> holder_base;
    typedef detail::function_storage<holder_base> storage;
    template <typename F>
    struct holder : holder_base {
        explicit holder(const F& f) : f_(f) {}
        virtual R call() { return f_(); }
        virtual holder_base* clone() const { return new holder(f_); }
        F f_;
    };

public:
    function() {}
    template <typename F>
    function(F f) : storage(new holder<F>(f)) {}
    void swap(function& o) { this->swap_storage(o); }
    R operator()() const { return this->held().call(); }
};

template <typename R, typename A1>
class function<R(A1)> : public detail::function_storage<detail::function_holder1<R, A1> > {
    typedef detail::function_holder1<R, A1> holder_base;
    typedef detail::function_storage<holder_base> storage;
    template <typename F>
    struct holder : holder_base {
        explicit holder(const F& f) : f_(f) {}
        virtual R call(A1 a1) { return f_(a1); }
        virtual holder_base* clone() const { return new holder(f_); }
        F f_;
    };

public:
    function() {}
    template <typename F>
    function(F f) : storage(new holder<F>(f)) {}
    void swap(function& o) { this->swap_storage(o); }
    R operator()(A1 a1) const { return this->held().call(a1); }
};

template <typename R, typename A1, typename A2>
class function<R(A1, A2)> : public detail::function_storage<detail::function_holder2<R, A1, A2> > {
    typedef detail::function_holder2<R, A1, A2> holder_base;
    typedef detail::function_storage<holder_base> storage;
    template <typename F>
    struct holder : holder_base {
        explicit holder(const F& f) : f_(f) {}
        virtual R call(A1 a1, A2 a2) { return f_(a1, a2); }
        virtual holder_base* clone() const { return new holder(f_); }
        F f_;
    };

public:
    function() {}
    template <typename F>
    function(F f) : storage(new holder<F>(f)) {}
    void swap(function& o) { this->swap_storage(o); }
    R operator()(A1 a1, A2 a2) const { return this->held().call(a1, a2); }
};

// array<T, N>: an aggregate, as `std::array`

template <typename T, size_t N>
struct array {
    typedef T value_type;
    typedef T* iterator;
    typedef const T* const_iterator;
    typedef T& reference;
    typedef const T& const_reference;
    typedef size_t size_type;

    T elems_[N > 0 ? N : 1];

    reference operator[](size_t i) { return elems_[i]; }
    const_reference operator[](size_t i) const { return elems_[i]; }
    reference at(size_t i) {
        if (i >= N) {
            throw std::out_of_range("array::at");
        }
        return elems_[i];
    }
    const_reference at(size_t i) const {
        if (i >= N) {
            throw std::out_of_range("array::at");
        }
        return elems_[i];
    }
    reference front() { return elems_[0]; }
    const_reference front() const { return elems_[0]; }
    reference back() { return elems_[N - 1]; }
    const_reference back() const { return elems_[N - 1]; }
    T* data() { return elems_; }
    const T* data() const { return elems_; }
    iterator begin() { return elems_; }
    const_iterator begin() const { return elems_; }
    iterator end() { return elems_ + N; }
    const_iterator end() const { return elems_ + N; }
    bool empty() const { return N == 0; }
    size_t size() const { return N; }
    size_t max_size() const { return N; }
    void fill(const T& v) {
        for (size_t i = 0; i < N; ++i) {
            elems_[i] = v;
        }
    }
};

template <size_t I, typename T, size_t N>
T& get(array<T, N>& a) {
    ZERODDS_STATIC_ASSERT((I < N), "array index out of range");
    return a.elems_[I];
}
template <size_t I, typename T, size_t N>
const T& get(const array<T, N>& a) {
    ZERODDS_STATIC_ASSERT((I < N), "array index out of range");
    return a.elems_[I];
}

template <typename T, size_t N>
bool operator==(const array<T, N>& a, const array<T, N>& b) {
    return std::equal(a.begin(), a.end(), b.begin());
}
template <typename T, size_t N>
bool operator!=(const array<T, N>& a, const array<T, N>& b) {
    return !(a == b);
}
template <typename T, size_t N>
bool operator<(const array<T, N>& a, const array<T, N>& b) {
    return std::lexicographical_compare(a.begin(), a.end(), b.begin(), b.end());
}

#endif  // ZERODDS_CXX11

}  // namespace cxx
}  // namespace zerodds

namespace zerodds {
namespace cxx {

// ---- helpers that exist in both modes --------------------------------------------------------

/// `std::to_string` (C++11) for the integer types.
inline std::string to_string(int v) { std::ostringstream o; o << v; return o.str(); }
inline std::string to_string(unsigned v) { std::ostringstream o; o << v; return o.str(); }
inline std::string to_string(long v) { std::ostringstream o; o << v; return o.str(); }
inline std::string to_string(unsigned long v) { std::ostringstream o; o << v; return o.str(); }
#if defined(__GNUC__) || defined(_MSC_VER)
inline std::string to_string(long long v) { std::ostringstream o; o << v; return o.str(); }
inline std::string to_string(unsigned long long v) { std::ostringstream o; o << v; return o.str(); }
#endif

/// A monotonic clock in microseconds (the epoch is unspecified).
inline uint64_t monotonic_microseconds() {
#if defined(_WIN32)
    return static_cast<uint64_t>(GetTickCount64()) * 1000u;
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return static_cast<uint64_t>(ts.tv_sec) * 1000000u + static_cast<uint64_t>(ts.tv_nsec) / 1000u;
#endif
}

/// Sleeps for `ms` milliseconds.
inline void sleep_milliseconds(unsigned ms) {
#if defined(_WIN32)
    Sleep(ms);
#else
    struct timespec ts;
    ts.tv_sec = static_cast<time_t>(ms / 1000u);
    ts.tv_nsec = static_cast<long>(ms % 1000u) * 1000000L;
    nanosleep(&ts, 0);
#endif
}

}  // namespace cxx
}  // namespace zerodds

#endif  // ZERODDS_PSM_CXX_HPP
