// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// omg::types: the C++ types IDL4 to C++ Language Mapping 1.0 (OMG
// formal/25-03-03) names for the code it generates. Header only.
//
// - The type traits of Clause 7.1.4 and Tables 7.4 to 7.12.
// - omg::types::sequence, bounded_sequence (7.2.4.2.1), string,
//   bounded_string, wstring, bounded_wstring (7.2.4.2.2, 7.2.4.2.3),
//   array (7.2.4.4), map, bounded_map (7.14.3.1). The unbounded ones are
//   the std:: types; the bounded ones are classes with the semantics of
//   the std:: type, conversion to and from it, and a bound check that
//   throws std::length_error (the NOTEs of 7.2.4.2.1, 7.2.4.2.2 and
//   7.14.3.1).
// - omg::types::fixed (7.2.4.2.4) and fixed_n<Digits, Scale>, the type of
//   "effectively constant digits and scale" a fixed<d, s> maps to.
// - omg::types::string_view, wstring_view (7.2.3), optional (Table 7.15),
//   ref_type, weak_ref_type (7.4), Any (7.3), ranged (Table 7.17).
// - omg::types::int8_t to uint64_t (Annex C, Tables C.1 to C.3).
//
// Clause 7.1.3 makes C++11 the minimum; Annex C gives the rules for
// C++98 and C++03, and this header compiles under them with the parts
// those rules use.

#ifndef OMG_TYPES_HPP
#define OMG_TYPES_HPP

#include <algorithm>
#include <cstddef>
#include <cstdio>
#include <cstring>
#include <iterator>
#include <limits>
#include <map>
#include <istream>
#include <ostream>
#include <sstream>
#include <stdexcept>
#include <string>
#include <typeinfo>
#include <utility>
#include <vector>
#include <stdint.h>

#if __cplusplus >= 201103L || (defined(_MSVC_LANG) && _MSVC_LANG >= 201103L)
#define OMG_TYPES_CXX11 1
#include <array>
#include <cstdint>
#include <initializer_list>
#include <memory>
#include <type_traits>
#define OMG_TYPES_CONSTEXPR constexpr
#define OMG_TYPES_EXPLICIT_CONVERSION explicit
#else
#define OMG_TYPES_CONSTEXPR
#define OMG_TYPES_EXPLICIT_CONVERSION
#endif

#if __cplusplus >= 201402L || (defined(_MSVC_LANG) && _MSVC_LANG >= 201402L)
#define OMG_TYPES_CXX14 1
#endif

#if __cplusplus >= 201703L || (defined(_MSVC_LANG) && _MSVC_LANG >= 201703L)
#define OMG_TYPES_CXX17 1
#include <optional>
#include <string_view>
#endif

namespace omg {
namespace types {

// -- Annex C, Tables C.1 to C.3: the integer types --------------------------

typedef ::int8_t int8_t;
typedef ::uint8_t uint8_t;
typedef ::int16_t int16_t;
typedef ::uint16_t uint16_t;
typedef ::int32_t int32_t;
typedef ::uint32_t uint32_t;
typedef ::int64_t int64_t;
typedef ::uint64_t uint64_t;

// -- 7.2.3: string_view and wstring_view --------------------------------------

#if defined(OMG_TYPES_CXX17)
using std::string_view;
using std::wstring_view;
#elif defined(OMG_TYPES_CXX11)
// The interface and semantics of std::basic_string_view, as far as a
// constant needs them, usable in a constexpr declaration under C++11.
template <typename CharT>
class basic_string_view {
public:
    typedef CharT value_type;
    typedef std::size_t size_type;
    typedef const CharT* const_iterator;
    typedef const CharT* iterator;
    static constexpr size_type npos = static_cast<size_type>(-1);

    constexpr basic_string_view() noexcept : data_(nullptr), size_(0) {}
    constexpr basic_string_view(const CharT* text) : data_(text), size_(length(text)) {}
    constexpr basic_string_view(const CharT* text, size_type size) : data_(text), size_(size) {}
    basic_string_view(const std::basic_string<CharT>& text) noexcept
        : data_(text.data()), size_(text.size()) {}

    constexpr const CharT* data() const noexcept { return data_; }
    constexpr size_type size() const noexcept { return size_; }
    constexpr size_type length() const noexcept { return size_; }
    constexpr bool empty() const noexcept { return size_ == 0; }
    constexpr const CharT& operator[](size_type i) const { return data_[i]; }
    const CharT& at(size_type i) const {
        if (i >= size_) throw std::out_of_range("omg::types::basic_string_view::at");
        return data_[i];
    }
    constexpr const_iterator begin() const noexcept { return data_; }
    constexpr const_iterator end() const noexcept { return data_ + size_; }
    explicit operator std::basic_string<CharT>() const {
        return std::basic_string<CharT>(data_, size_);
    }
    std::basic_string<CharT> to_string() const { return std::basic_string<CharT>(data_, size_); }
    int compare(basic_string_view other) const noexcept {
        const size_type n = size_ < other.size_ ? size_ : other.size_;
        for (size_type i = 0; i < n; ++i) {
            if (data_[i] < other.data_[i]) return -1;
            if (other.data_[i] < data_[i]) return 1;
        }
        return size_ == other.size_ ? 0 : (size_ < other.size_ ? -1 : 1);
    }

private:
    static constexpr size_type length(const CharT* text, size_type n = 0) {
        return text[n] == CharT() ? n : length(text, n + 1);
    }
    const CharT* data_;
    size_type size_;
};

template <typename CharT>
constexpr typename basic_string_view<CharT>::size_type basic_string_view<CharT>::npos;

template <typename CharT>
inline bool operator==(basic_string_view<CharT> a, basic_string_view<CharT> b) noexcept {
    return a.compare(b) == 0;
}
template <typename CharT>
inline bool operator!=(basic_string_view<CharT> a, basic_string_view<CharT> b) noexcept {
    return a.compare(b) != 0;
}
template <typename CharT>
inline bool operator<(basic_string_view<CharT> a, basic_string_view<CharT> b) noexcept {
    return a.compare(b) < 0;
}
template <typename CharT>
inline std::basic_ostream<CharT>& operator<<(std::basic_ostream<CharT>& os,
                                             basic_string_view<CharT> v) {
    return os.write(v.data(), static_cast<std::streamsize>(v.size()));
}

typedef basic_string_view<char> string_view;
typedef basic_string_view<wchar_t> wstring_view;
#endif

// -- Table 7.15: optional -----------------------------------------------------

#if defined(OMG_TYPES_CXX17)
using std::nullopt;
using std::nullopt_t;
using std::optional;
#elif defined(OMG_TYPES_CXX11)
struct nullopt_t {
    explicit constexpr nullopt_t(int) {}
};
constexpr nullopt_t nullopt{0};

class bad_optional_access : public std::logic_error {
public:
    bad_optional_access() : std::logic_error("omg::types::optional: no value") {}
};

// The semantics of std::optional for an implementation that has to stay
// with C++11 (Table 7.15).
template <typename T>
class optional {
public:
    typedef T value_type;

    optional() noexcept : engaged_(false) {}
    optional(nullopt_t) noexcept : engaged_(false) {}
    optional(const T& value) : engaged_(false) { construct(value); }
    optional(T&& value) : engaged_(false) { construct(std::move(value)); }
    optional(const optional& other) : engaged_(false) {
        if (other.engaged_) construct(*other);
    }
    optional(optional&& other) : engaged_(false) {
        if (other.engaged_) construct(std::move(*other));
    }
    ~optional() { reset(); }

    optional& operator=(nullopt_t) noexcept {
        reset();
        return *this;
    }
    optional& operator=(const optional& other) {
        if (this != &other) {
            if (other.engaged_) {
                assign(*other);
            } else {
                reset();
            }
        }
        return *this;
    }
    optional& operator=(optional&& other) {
        if (this != &other) {
            if (other.engaged_) {
                assign(std::move(*other));
            } else {
                reset();
            }
        }
        return *this;
    }
    template <typename U,
              typename = typename std::enable_if<std::is_constructible<T, U&&>::value &&
                                                 !std::is_same<typename std::decay<U>::type,
                                                               optional>::value>::type>
    optional& operator=(U&& value) {
        assign(std::forward<U>(value));
        return *this;
    }

    bool has_value() const noexcept { return engaged_; }
    explicit operator bool() const noexcept { return engaged_; }
    T& value() {
        if (!engaged_) throw bad_optional_access();
        return *ptr();
    }
    const T& value() const {
        if (!engaged_) throw bad_optional_access();
        return *ptr();
    }
    template <typename U>
    T value_or(U&& fallback) const {
        return engaged_ ? *ptr() : static_cast<T>(std::forward<U>(fallback));
    }
    T& operator*() { return *ptr(); }
    const T& operator*() const { return *ptr(); }
    T* operator->() { return ptr(); }
    const T* operator->() const { return ptr(); }
    void reset() noexcept {
        if (engaged_) {
            ptr()->~T();
            engaged_ = false;
        }
    }
    template <typename... Args>
    T& emplace(Args&&... args) {
        reset();
        construct(std::forward<Args>(args)...);
        return *ptr();
    }
    void swap(optional& other) {
        optional tmp(std::move(other));
        other = std::move(*this);
        *this = std::move(tmp);
    }

private:
    template <typename... Args>
    void construct(Args&&... args) {
        ::new (static_cast<void*>(&storage_)) T(std::forward<Args>(args)...);
        engaged_ = true;
    }
    template <typename U>
    void assign(U&& value) {
        if (engaged_) {
            *ptr() = std::forward<U>(value);
        } else {
            construct(std::forward<U>(value));
        }
    }
    T* ptr() { return reinterpret_cast<T*>(&storage_); }
    const T* ptr() const { return reinterpret_cast<const T*>(&storage_); }

    typename std::aligned_storage<sizeof(T), alignof(T)>::type storage_;
    bool engaged_;
};

template <typename T>
inline bool operator==(const optional<T>& a, const optional<T>& b) {
    return a.has_value() == b.has_value() && (!a.has_value() || *a == *b);
}
template <typename T>
inline bool operator!=(const optional<T>& a, const optional<T>& b) {
    return !(a == b);
}
template <typename T>
inline bool operator==(const optional<T>& a, nullopt_t) noexcept {
    return !a.has_value();
}
template <typename T>
inline bool operator!=(const optional<T>& a, nullopt_t) noexcept {
    return a.has_value();
}
template <typename T>
inline void swap(optional<T>& a, optional<T>& b) {
    a.swap(b);
}
#endif

// -- 7.4: references ----------------------------------------------------------

#if defined(OMG_TYPES_CXX11)
namespace detail {
// What a member access through a nil reference does. A platform header
// that defines OMG_TYPES_NIL_REFERENCE ahead of this one replaces it
// (omg/corba.hpp: CORBA::INV_OBJREF, A.1.5.3).
#if !defined(OMG_TYPES_NIL_REFERENCE)
[[noreturn]] inline void nil_reference() {
    throw std::logic_error("omg::types::ref_type: member access through a nil reference");
}
#define OMG_TYPES_NIL_REFERENCE() ::omg::types::detail::nil_reference()
#endif
}  // namespace detail

// "omg::types::ref_type<T> that delivers std::shared_ptr<T> semantics": a
// std::shared_ptr<T> whose member access answers a nil reference with
// OMG_TYPES_NIL_REFERENCE. It converts from and to std::shared_ptr, so the
// pointer casts and std::weak_ptr of <memory> take it as it is.
template <typename T>
class ref_type : public std::shared_ptr<T> {
public:
    typedef std::shared_ptr<T> pointer_type;

    ref_type() noexcept = default;
    ref_type(std::nullptr_t) noexcept {}
    ref_type(const pointer_type& p) noexcept : pointer_type(p) {}
    ref_type(pointer_type&& p) noexcept : pointer_type(std::move(p)) {}
    template <typename U, typename = typename std::enable_if<std::is_convertible<U*, T*>::value>::type>
    ref_type(const std::shared_ptr<U>& p) noexcept : pointer_type(p) {}
    template <typename U, typename = typename std::enable_if<std::is_convertible<U*, T*>::value>::type>
    ref_type(std::shared_ptr<U>&& p) noexcept : pointer_type(std::move(p)) {}

    T* operator->() const {
        if (this->get() == nullptr) OMG_TYPES_NIL_REFERENCE();
        return this->get();
    }
    typename std::add_lvalue_reference<T>::type operator*() const {
        if (this->get() == nullptr) OMG_TYPES_NIL_REFERENCE();
        return *this->get();
    }
};
template <typename T>
using weak_ref_type = std::weak_ptr<T>;

template <typename T, typename... Args>
inline ref_type<T> make_ref(Args&&... args) {
    return std::make_shared<T>(std::forward<Args>(args)...);
}

// -- 7.2.4.2, 7.2.4.4, 7.14.3.1: the unbounded template types -----------------

template <typename T>
using sequence = std::vector<T>;
using string = std::string;
using wstring = std::wstring;
template <typename T, std::size_t N>
using array = std::array<T, N>;
template <typename Key, typename T>
using map = std::map<Key, T>;
#endif

namespace detail {
// Element-wise comparison and swap of a C array (Annex C.2.1.2.3), whose
// == compares addresses and which std::swap of C++03 cannot exchange.
template <typename T>
inline bool equal(const T& a, const T& b) {
    return a == b;
}
template <typename T, std::size_t N>
inline bool equal(const T (&a)[N], const T (&b)[N]) {
    for (std::size_t i = 0; i < N; ++i) {
        if (!equal(a[i], b[i])) return false;
    }
    return true;
}
template <typename T>
inline void swap_values(T& a, T& b) {
    using std::swap;
    swap(a, b);
}
template <typename T, std::size_t N>
inline void swap_values(T (&a)[N], T (&b)[N]) {
    for (std::size_t i = 0; i < N; ++i) swap_values(a[i], b[i]);
}

#if !defined(OMG_TYPES_BOUND_EXCEEDED)
inline void bound_exceeded(const char* what, std::size_t size, std::size_t bound) {
    std::ostringstream text;
    text << what << ": " << size << " elements exceed the bound " << bound;
    throw std::length_error(text.str());
}
#define OMG_TYPES_BOUND_EXCEEDED(what, size, bound) ::omg::types::detail::bound_exceeded(what, size, bound)
#endif
}  // namespace detail

// -- 7.2.4.2.1: bounded_sequence ------------------------------------------------

// The semantics of std::vector<T>, at most N elements: an operation that
// would hold more throws std::length_error and leaves the sequence as it
// was.
template <typename T, std::size_t N>
class bounded_sequence {
public:
    typedef std::vector<T> vector_type;
    typedef typename vector_type::value_type value_type;
    typedef typename vector_type::allocator_type allocator_type;
    typedef typename vector_type::size_type size_type;
    typedef typename vector_type::difference_type difference_type;
    typedef typename vector_type::reference reference;
    typedef typename vector_type::const_reference const_reference;
    typedef typename vector_type::pointer pointer;
    typedef typename vector_type::const_pointer const_pointer;
    typedef typename vector_type::iterator iterator;
    typedef typename vector_type::const_iterator const_iterator;
    typedef typename vector_type::reverse_iterator reverse_iterator;
    typedef typename vector_type::const_reverse_iterator const_reverse_iterator;
#if defined(OMG_TYPES_CXX11)
    typedef const_iterator position;
#else
    typedef iterator position;
#endif

    bounded_sequence() {}
    explicit bounded_sequence(size_type count) : v_(check(count)) {}
    bounded_sequence(size_type count, const T& value) : v_(check(count), value) {}
    template <typename InputIt>
    bounded_sequence(InputIt first, InputIt last) : v_(first, last) {
        check(v_.size());
    }
    bounded_sequence(const vector_type& v) : v_(v) { check(v_.size()); }
#if defined(OMG_TYPES_CXX11)
    bounded_sequence(vector_type&& v) : v_(std::move(v)) { check(v_.size()); }
    bounded_sequence(std::initializer_list<T> init) : v_(init) { check(v_.size()); }
    bounded_sequence& operator=(std::initializer_list<T> init) {
        check(init.size());
        v_ = init;
        return *this;
    }
#endif
    bounded_sequence& operator=(const vector_type& v) {
        check(v.size());
        v_ = v;
        return *this;
    }

    operator const vector_type&() const { return v_; }
    const vector_type& to_vector() const { return v_; }

    reference at(size_type i) { return v_.at(i); }
    const_reference at(size_type i) const { return v_.at(i); }
    reference operator[](size_type i) { return v_[i]; }
    const_reference operator[](size_type i) const { return v_[i]; }
    reference front() { return v_.front(); }
    const_reference front() const { return v_.front(); }
    reference back() { return v_.back(); }
    const_reference back() const { return v_.back(); }
#if defined(OMG_TYPES_CXX11)
    T* data() noexcept { return v_.data(); }
    const T* data() const noexcept { return v_.data(); }
    const_iterator cbegin() const { return v_.cbegin(); }
    const_iterator cend() const { return v_.cend(); }
#endif

    iterator begin() { return v_.begin(); }
    const_iterator begin() const { return v_.begin(); }
    iterator end() { return v_.end(); }
    const_iterator end() const { return v_.end(); }
    reverse_iterator rbegin() { return v_.rbegin(); }
    const_reverse_iterator rbegin() const { return v_.rbegin(); }
    reverse_iterator rend() { return v_.rend(); }
    const_reverse_iterator rend() const { return v_.rend(); }

    bool empty() const { return v_.empty(); }
    size_type size() const { return v_.size(); }
    size_type max_size() const { return N; }
    void reserve(size_type n) { v_.reserve(check(n)); }
    size_type capacity() const { return v_.capacity(); }

    void clear() { v_.clear(); }
    iterator insert(position pos, const T& value) {
        check(v_.size() + 1);
        return v_.insert(pos, value);
    }
    iterator insert(position pos, size_type count, const T& value) {
        check(v_.size() + count);
#if defined(OMG_TYPES_CXX11)
        return v_.insert(pos, count, value);
#else
        const difference_type at = pos - v_.begin();
        v_.insert(pos, count, value);
        return v_.begin() + at;
#endif
    }
    template <typename InputIt>
    iterator insert(position pos, InputIt first, InputIt last) {
        const vector_type added(first, last);
        check(v_.size() + added.size());
        const difference_type at = pos - static_cast<position>(v_.begin());
        v_.insert(pos, added.begin(), added.end());
        return v_.begin() + at;
    }
    iterator erase(position pos) { return v_.erase(pos); }
    iterator erase(position first, position last) { return v_.erase(first, last); }
    void push_back(const T& value) {
        check(v_.size() + 1);
        v_.push_back(value);
    }
#if defined(OMG_TYPES_CXX11)
    iterator insert(position pos, T&& value) {
        check(v_.size() + 1);
        return v_.insert(pos, std::move(value));
    }
    void push_back(T&& value) {
        check(v_.size() + 1);
        v_.push_back(std::move(value));
    }
    template <typename... Args>
    reference emplace_back(Args&&... args) {
        check(v_.size() + 1);
        v_.emplace_back(std::forward<Args>(args)...);
        return v_.back();
    }
    template <typename... Args>
    iterator emplace(position pos, Args&&... args) {
        check(v_.size() + 1);
        return v_.emplace(pos, std::forward<Args>(args)...);
    }
    void shrink_to_fit() { v_.shrink_to_fit(); }
#endif
    void pop_back() { v_.pop_back(); }
    void resize(size_type count) { v_.resize(check(count)); }
    void resize(size_type count, const T& value) { v_.resize(check(count), value); }
    void assign(size_type count, const T& value) { v_.assign(check(count), value); }
    template <typename InputIt>
    void assign(InputIt first, InputIt last) {
        vector_type replaced(first, last);
        check(replaced.size());
        v_.swap(replaced);
    }
    void swap(bounded_sequence& other) { v_.swap(other.v_); }

    friend bool operator==(const bounded_sequence& a, const bounded_sequence& b) {
        return a.v_ == b.v_;
    }
    friend bool operator!=(const bounded_sequence& a, const bounded_sequence& b) {
        return a.v_ != b.v_;
    }
    friend bool operator<(const bounded_sequence& a, const bounded_sequence& b) {
        return a.v_ < b.v_;
    }
    friend bool operator<=(const bounded_sequence& a, const bounded_sequence& b) {
        return a.v_ <= b.v_;
    }
    friend bool operator>(const bounded_sequence& a, const bounded_sequence& b) {
        return a.v_ > b.v_;
    }
    friend bool operator>=(const bounded_sequence& a, const bounded_sequence& b) {
        return a.v_ >= b.v_;
    }

private:
    static size_type check(size_type n) {
        if (n > N) OMG_TYPES_BOUND_EXCEEDED("omg::types::bounded_sequence", n, N);
        return n;
    }
    vector_type v_;
};

template <typename T, std::size_t N>
inline void swap(bounded_sequence<T, N>& a, bounded_sequence<T, N>& b) {
    a.swap(b);
}

// -- 7.2.4.2.2, 7.2.4.2.3: bounded_string and bounded_wstring ------------------

// The semantics of std::basic_string<CharT>, at most N characters.
template <typename CharT, std::size_t N>
class basic_bounded_string {
public:
    typedef std::basic_string<CharT> string_type;
    typedef typename string_type::value_type value_type;
    typedef typename string_type::size_type size_type;
    typedef typename string_type::reference reference;
    typedef typename string_type::const_reference const_reference;
    typedef typename string_type::iterator iterator;
    typedef typename string_type::const_iterator const_iterator;
    static const size_type npos = static_cast<size_type>(-1);

    basic_bounded_string() {}
    basic_bounded_string(const CharT* text) : s_(text) { check(s_.size()); }
    basic_bounded_string(const CharT* text, size_type count) : s_(text, check(count)) {}
    basic_bounded_string(size_type count, CharT c) : s_(check(count), c) {}
    basic_bounded_string(const string_type& text) : s_(text) { check(s_.size()); }
    template <typename InputIt>
    basic_bounded_string(InputIt first, InputIt last) : s_(first, last) {
        check(s_.size());
    }
#if defined(OMG_TYPES_CXX11)
    basic_bounded_string(string_type&& text) : s_(std::move(text)) { check(s_.size()); }
    basic_bounded_string(std::initializer_list<CharT> init) : s_(init) { check(s_.size()); }
#endif
    basic_bounded_string& operator=(const CharT* text) {
        string_type replaced(text);
        check(replaced.size());
        s_.swap(replaced);
        return *this;
    }
    basic_bounded_string& operator=(const string_type& text) {
        check(text.size());
        s_ = text;
        return *this;
    }

    operator const string_type&() const { return s_; }
    const string_type& str() const { return s_; }
    const CharT* c_str() const { return s_.c_str(); }
    const CharT* data() const { return s_.data(); }

    reference operator[](size_type i) { return s_[i]; }
    const_reference operator[](size_type i) const { return s_[i]; }
    reference at(size_type i) { return s_.at(i); }
    const_reference at(size_type i) const { return s_.at(i); }
    iterator begin() { return s_.begin(); }
    const_iterator begin() const { return s_.begin(); }
    iterator end() { return s_.end(); }
    const_iterator end() const { return s_.end(); }

    bool empty() const { return s_.empty(); }
    size_type size() const { return s_.size(); }
    size_type length() const { return s_.length(); }
    size_type max_size() const { return N; }
    void reserve(size_type n) { s_.reserve(check(n)); }
    size_type capacity() const { return s_.capacity(); }

    void clear() { s_.clear(); }
    void push_back(CharT c) {
        check(s_.size() + 1);
        s_.push_back(c);
    }
    void pop_back() { s_.erase(s_.size() - 1, 1); }
    basic_bounded_string& append(const string_type& text) {
        check(s_.size() + text.size());
        s_.append(text);
        return *this;
    }
    basic_bounded_string& append(const CharT* text) { return append(string_type(text)); }
    basic_bounded_string& append(size_type count, CharT c) {
        check(s_.size() + count);
        s_.append(count, c);
        return *this;
    }
    basic_bounded_string& operator+=(const string_type& text) { return append(text); }
    basic_bounded_string& operator+=(const CharT* text) { return append(text); }
    basic_bounded_string& operator+=(CharT c) {
        push_back(c);
        return *this;
    }
    basic_bounded_string& insert(size_type pos, const string_type& text) {
        check(s_.size() + text.size());
        s_.insert(pos, text);
        return *this;
    }
    basic_bounded_string& erase(size_type pos = 0, size_type count = npos) {
        s_.erase(pos, count);
        return *this;
    }
    void resize(size_type count) { s_.resize(check(count)); }
    void resize(size_type count, CharT c) { s_.resize(check(count), c); }
    basic_bounded_string& assign(const string_type& text) {
        check(text.size());
        s_.assign(text);
        return *this;
    }
    string_type substr(size_type pos = 0, size_type count = npos) const {
        return s_.substr(pos, count);
    }
    size_type find(const string_type& text, size_type pos = 0) const { return s_.find(text, pos); }
    size_type find(CharT c, size_type pos = 0) const { return s_.find(c, pos); }
    int compare(const string_type& other) const { return s_.compare(other); }
    void swap(basic_bounded_string& other) { s_.swap(other.s_); }

    friend bool operator==(const basic_bounded_string& a, const basic_bounded_string& b) {
        return a.s_ == b.s_;
    }
    friend bool operator!=(const basic_bounded_string& a, const basic_bounded_string& b) {
        return a.s_ != b.s_;
    }
    friend bool operator<(const basic_bounded_string& a, const basic_bounded_string& b) {
        return a.s_ < b.s_;
    }
    friend bool operator==(const basic_bounded_string& a, const string_type& b) {
        return a.s_ == b;
    }
    friend bool operator==(const string_type& a, const basic_bounded_string& b) {
        return a == b.s_;
    }
    friend bool operator!=(const basic_bounded_string& a, const string_type& b) {
        return a.s_ != b;
    }
    friend bool operator==(const basic_bounded_string& a, const CharT* b) { return a.s_ == b; }
    friend bool operator!=(const basic_bounded_string& a, const CharT* b) { return a.s_ != b; }
    friend std::basic_ostream<CharT>& operator<<(std::basic_ostream<CharT>& os,
                                                 const basic_bounded_string& s) {
        return os << s.s_;
    }

private:
    static size_type check(size_type n) {
        if (n > N) OMG_TYPES_BOUND_EXCEEDED("omg::types::bounded_string", n, N);
        return n;
    }
    string_type s_;
};

template <typename CharT, std::size_t N>
const typename basic_bounded_string<CharT, N>::size_type basic_bounded_string<CharT, N>::npos;

template <typename CharT, std::size_t N>
inline void swap(basic_bounded_string<CharT, N>& a, basic_bounded_string<CharT, N>& b) {
    a.swap(b);
}

#if defined(OMG_TYPES_CXX11)
template <std::size_t N>
using bounded_string = basic_bounded_string<char, N>;
template <std::size_t N>
using bounded_wstring = basic_bounded_string<wchar_t, N>;
#else
template <std::size_t N>
class bounded_string : public basic_bounded_string<char, N> {
public:
    bounded_string() {}
    bounded_string(const char* text) : basic_bounded_string<char, N>(text) {}
    bounded_string(const std::string& text) : basic_bounded_string<char, N>(text) {}
};
template <std::size_t N>
class bounded_wstring : public basic_bounded_string<wchar_t, N> {
public:
    bounded_wstring() {}
    bounded_wstring(const wchar_t* text) : basic_bounded_string<wchar_t, N>(text) {}
    bounded_wstring(const std::wstring& text) : basic_bounded_string<wchar_t, N>(text) {}
};
#endif

// -- 7.14.3.1: bounded_map ----------------------------------------------------

// The semantics of std::map<Key, T>, at most N entries.
template <typename Key, typename T, std::size_t N>
class bounded_map {
public:
    typedef std::map<Key, T> map_type;
    typedef typename map_type::key_type key_type;
    typedef typename map_type::mapped_type mapped_type;
    typedef typename map_type::value_type value_type;
    typedef typename map_type::size_type size_type;
    typedef typename map_type::iterator iterator;
    typedef typename map_type::const_iterator const_iterator;

    bounded_map() {}
    bounded_map(const map_type& m) : m_(m) { check(m_.size()); }
    template <typename InputIt>
    bounded_map(InputIt first, InputIt last) : m_(first, last) {
        check(m_.size());
    }
#if defined(OMG_TYPES_CXX11)
    bounded_map(map_type&& m) : m_(std::move(m)) { check(m_.size()); }
    bounded_map(std::initializer_list<value_type> init) : m_(init) { check(m_.size()); }
#endif
    bounded_map& operator=(const map_type& m) {
        check(m.size());
        m_ = m;
        return *this;
    }

    operator const map_type&() const { return m_; }
    const map_type& to_map() const { return m_; }

    T& at(const Key& key) {
        iterator found = m_.find(key);
        if (found == m_.end()) throw std::out_of_range("omg::types::bounded_map::at");
        return found->second;
    }
    const T& at(const Key& key) const {
        const_iterator found = m_.find(key);
        if (found == m_.end()) throw std::out_of_range("omg::types::bounded_map::at");
        return found->second;
    }
    T& operator[](const Key& key) {
        if (m_.find(key) == m_.end()) check(m_.size() + 1);
        return m_[key];
    }
    iterator begin() { return m_.begin(); }
    const_iterator begin() const { return m_.begin(); }
    iterator end() { return m_.end(); }
    const_iterator end() const { return m_.end(); }
    bool empty() const { return m_.empty(); }
    size_type size() const { return m_.size(); }
    size_type max_size() const { return N; }
    void clear() { m_.clear(); }
    std::pair<iterator, bool> insert(const value_type& value) {
        if (m_.find(value.first) == m_.end()) check(m_.size() + 1);
        return m_.insert(value);
    }
#if defined(OMG_TYPES_CXX11)
    template <typename... Args>
    std::pair<iterator, bool> emplace(Args&&... args) {
        value_type value(std::forward<Args>(args)...);
        if (m_.find(value.first) == m_.end()) check(m_.size() + 1);
        return m_.insert(std::move(value));
    }
    template <typename M>
    std::pair<iterator, bool> insert_or_assign(const Key& key, M&& value) {
        iterator found = m_.find(key);
        if (found != m_.end()) {
            found->second = std::forward<M>(value);
            return std::make_pair(found, false);
        }
        check(m_.size() + 1);
        return m_.insert(value_type(key, std::forward<M>(value)));
    }
#endif
    size_type erase(const Key& key) { return m_.erase(key); }
    void erase(iterator pos) { m_.erase(pos); }
    iterator find(const Key& key) { return m_.find(key); }
    const_iterator find(const Key& key) const { return m_.find(key); }
    size_type count(const Key& key) const { return m_.count(key); }
    void swap(bounded_map& other) { m_.swap(other.m_); }

    friend bool operator==(const bounded_map& a, const bounded_map& b) { return a.m_ == b.m_; }
    friend bool operator!=(const bounded_map& a, const bounded_map& b) { return a.m_ != b.m_; }
    friend bool operator<(const bounded_map& a, const bounded_map& b) { return a.m_ < b.m_; }

private:
    static size_type check(size_type n) {
        if (n > N) OMG_TYPES_BOUND_EXCEEDED("omg::types::bounded_map", n, N);
        return n;
    }
    map_type m_;
};

template <typename Key, typename T, std::size_t N>
inline void swap(bounded_map<Key, T, N>& a, bounded_map<Key, T, N>& b) {
    a.swap(b);
}

// -- 7.2.4.2.4: fixed ---------------------------------------------------------

namespace detail {

// The text of a fixed-point literal, read by constant expressions: an
// optional sign, digits with at most one '.', and an optional d or D
// (OMG IDL 4.2 §7.2.6.5). Each helper is one return statement, so they are
// constexpr under C++11.
OMG_TYPES_CONSTEXPR inline bool fx_sign(char c) { return c == '-' || c == '+'; }
OMG_TYPES_CONSTEXPR inline bool fx_digit_char(char c) { return c >= '0' && c <= '9'; }
OMG_TYPES_CONSTEXPR inline std::size_t fx_begin(const char* p, std::size_t n) {
    return (n > 0 && fx_sign(p[0])) ? 1 : 0;
}
OMG_TYPES_CONSTEXPR inline std::size_t fx_end(const char* p, std::size_t n) {
    return (n > 0 && (p[n - 1] == 'd' || p[n - 1] == 'D')) ? n - 1 : n;
}
OMG_TYPES_CONSTEXPR inline std::size_t fx_dot_from(const char* p, std::size_t i, std::size_t end) {
    return i >= end ? end : (p[i] == '.' ? i : fx_dot_from(p, i + 1, end));
}
OMG_TYPES_CONSTEXPR inline std::size_t fx_dot(const char* p, std::size_t n) {
    return fx_dot_from(p, fx_begin(p, n), fx_end(p, n));
}
OMG_TYPES_CONSTEXPR inline std::size_t fx_scale(const char* p, std::size_t n) {
    return fx_dot(p, n) < fx_end(p, n) ? fx_end(p, n) - fx_dot(p, n) - 1 : 0;
}
OMG_TYPES_CONSTEXPR inline std::size_t fx_zeros_from(const char* p, std::size_t i, std::size_t end) {
    return (i < end && p[i] == '0') ? 1 + fx_zeros_from(p, i + 1, end) : 0;
}
// The digits before the point, leading zeros left out.
OMG_TYPES_CONSTEXPR inline std::size_t fx_integer_digits(const char* p, std::size_t n) {
    return fx_dot(p, n) - fx_begin(p, n) - fx_zeros_from(p, fx_begin(p, n), fx_dot(p, n));
}
OMG_TYPES_CONSTEXPR inline bool fx_well_formed_from(const char* p, std::size_t i, std::size_t end,
                                                    bool dot) {
    return i >= end ? true
                    : (fx_digit_char(p[i]) ? fx_well_formed_from(p, i + 1, end, dot)
                                           : (p[i] == '.' && !dot
                                                  ? fx_well_formed_from(p, i + 1, end, true)
                                                  : false));
}
OMG_TYPES_CONSTEXPR inline std::size_t fx_check(const char* p, std::size_t n) {
    return (fx_end(p, n) > fx_begin(p, n) &&
            fx_end(p, n) - fx_begin(p, n) > (fx_dot(p, n) < fx_end(p, n) ? 1u : 0u) &&
            fx_well_formed_from(p, fx_begin(p, n), fx_end(p, n), false))
               ? n
               : throw std::invalid_argument("omg::types::fixed: not a fixed-point literal");
}
// Digit k of the unscaled value, k = 0 the least significant.
OMG_TYPES_CONSTEXPR inline unsigned char fx_digit(const char* p, std::size_t n, std::size_t k) {
    return k < fx_scale(p, n)
               ? static_cast<unsigned char>(p[fx_end(p, n) - 1 - k] - '0')
               : (k - fx_scale(p, n) < fx_dot(p, n) - fx_begin(p, n)
                      ? static_cast<unsigned char>(p[fx_dot(p, n) - 1 - (k - fx_scale(p, n))] - '0')
                      : static_cast<unsigned char>(0));
}
// Digit k of the value written with `scale` digits after the point: the
// literal's further fraction digits are cut off (truncation).
OMG_TYPES_CONSTEXPR inline unsigned char fx_digit_at(const char* p, std::size_t n,
                                                     std::size_t scale, std::size_t k) {
    return scale >= fx_scale(p, n)
               ? (k < scale - fx_scale(p, n) ? static_cast<unsigned char>(0)
                                             : fx_digit(p, n, k - (scale - fx_scale(p, n))))
               : fx_digit(p, n, k + (fx_scale(p, n) - scale));
}
OMG_TYPES_CONSTEXPR inline bool fx_all_zero_from(const char* p, std::size_t i, std::size_t end) {
    return i >= end ? true
                    : ((p[i] == '0' || p[i] == '.') ? fx_all_zero_from(p, i + 1, end) : false);
}
OMG_TYPES_CONSTEXPR inline bool fx_negative(const char* p, std::size_t n) {
    return n > 0 && p[0] == '-' && !fx_all_zero_from(p, fx_begin(p, n), fx_end(p, n));
}
OMG_TYPES_CONSTEXPR inline unsigned short fx_digits_of(const char* p, std::size_t n) {
    return fx_integer_digits(p, n) + fx_scale(p, n) == 0
               ? static_cast<unsigned short>(1)
               : (fx_integer_digits(p, n) + fx_scale(p, n) <= 31
                      ? static_cast<unsigned short>(fx_integer_digits(p, n) + fx_scale(p, n))
                      : throw std::range_error("omg::types::fixed: more than 31 digits"));
}
OMG_TYPES_CONSTEXPR inline unsigned short fx_fits(const char* p, std::size_t n,
                                                  unsigned short digits, unsigned short scale) {
    return fx_integer_digits(p, n) <= static_cast<std::size_t>(digits - scale)
               ? digits
               : throw std::range_error("omg::types::fixed: the value has more integer digits "
                                        "than the type");
}

// The magnitude of a fixed-point value as decimal digits, least
// significant first, for the arithmetic.
typedef std::vector<unsigned char> fx_magnitude;

inline void fx_trim(fx_magnitude& m) {
    while (!m.empty() && m.back() == 0) m.pop_back();
}
inline int fx_compare(const fx_magnitude& a, const fx_magnitude& b) {
    fx_magnitude x(a), y(b);
    fx_trim(x);
    fx_trim(y);
    if (x.size() != y.size()) return x.size() < y.size() ? -1 : 1;
    for (std::size_t i = x.size(); i > 0; --i) {
        if (x[i - 1] != y[i - 1]) return x[i - 1] < y[i - 1] ? -1 : 1;
    }
    return 0;
}
inline fx_magnitude fx_add(const fx_magnitude& a, const fx_magnitude& b) {
    fx_magnitude out;
    unsigned carry = 0;
    for (std::size_t i = 0; i < a.size() || i < b.size() || carry; ++i) {
        unsigned sum = carry + (i < a.size() ? a[i] : 0) + (i < b.size() ? b[i] : 0);
        out.push_back(static_cast<unsigned char>(sum % 10));
        carry = sum / 10;
    }
    return out;
}
// a - b, a >= b.
inline fx_magnitude fx_sub(const fx_magnitude& a, const fx_magnitude& b) {
    fx_magnitude out;
    int borrow = 0;
    for (std::size_t i = 0; i < a.size(); ++i) {
        int d = static_cast<int>(a[i]) - borrow - (i < b.size() ? b[i] : 0);
        borrow = d < 0 ? 1 : 0;
        out.push_back(static_cast<unsigned char>(d < 0 ? d + 10 : d));
    }
    fx_trim(out);
    return out;
}
inline fx_magnitude fx_mul(const fx_magnitude& a, const fx_magnitude& b) {
    std::vector<unsigned> acc(a.size() + b.size() + 1, 0);
    for (std::size_t i = 0; i < a.size(); ++i) {
        for (std::size_t j = 0; j < b.size(); ++j) acc[i + j] += a[i] * b[j];
    }
    fx_magnitude out;
    unsigned carry = 0;
    for (std::size_t i = 0; i < acc.size(); ++i) {
        unsigned v = acc[i] + carry;
        out.push_back(static_cast<unsigned char>(v % 10));
        carry = v / 10;
    }
    while (carry) {
        out.push_back(static_cast<unsigned char>(carry % 10));
        carry /= 10;
    }
    fx_trim(out);
    return out;
}
inline fx_magnitude fx_shift(const fx_magnitude& a, std::size_t places) {
    fx_magnitude out(places, 0);
    out.insert(out.end(), a.begin(), a.end());
    return out;
}
// The truncated quotient a / b, b not zero.
inline fx_magnitude fx_div(const fx_magnitude& a, const fx_magnitude& b) {
    fx_magnitude quotient(a.size(), 0), rest;
    for (std::size_t i = a.size(); i > 0; --i) {
        rest.insert(rest.begin(), a[i - 1]);
        fx_trim(rest);
        unsigned char q = 0;
        while (fx_compare(rest, b) >= 0) {
            rest = fx_sub(rest, b);
            ++q;
        }
        quotient[i - 1] = q;
    }
    fx_trim(quotient);
    return quotient;
}

}  // namespace detail

template <uint16_t Digits, uint16_t Scale>
class fixed_n;

// "The IDL fixed type shall map to a C++ class named fixed" (7.2.4.2.4).
// At most 31 significant digits; a result with more keeps the 31 most
// significant and cuts the fraction (OMG IDL 4.2 §7.4.1.4.4.3), and one
// with more than 31 integer digits throws std::overflow_error.
class fixed {
public:
    static const unsigned short max_digits = 31;

    OMG_TYPES_CONSTEXPR fixed()
        : neg_(false), digits_(1), scale_(0)
#if defined(OMG_TYPES_CXX11)
          , d_{}
#endif
    {
#if !defined(OMG_TYPES_CXX11)
        std::memset(d_, 0, sizeof d_);
#endif
    }

#if defined(OMG_TYPES_CXX11)
    constexpr explicit fixed(string_view text) : fixed(text.data(), text.size(), 0, 0, false) {}
    constexpr explicit fixed(const char* text) : fixed(string_view(text)) {}
#else
    explicit fixed(const char* text) { parse(text, std::strlen(text)); }
    explicit fixed(const std::string& text) { parse(text.data(), text.size()); }
#endif
    fixed(const fixed& other)
#if defined(OMG_TYPES_CXX11)
        = default;
    fixed(fixed&& val) = default;
    ~fixed() = default;
#else
        : neg_(other.neg_), digits_(other.digits_), scale_(other.scale_) {
        std::memcpy(d_, other.d_, sizeof d_);
    }
    ~fixed() {}
#endif

    explicit fixed(int v) { from_integer(static_cast<long long>(v)); }
    explicit fixed(unsigned v) { from_unsigned(v); }
    explicit fixed(long v) { from_integer(static_cast<long long>(v)); }
    explicit fixed(unsigned long v) { from_unsigned(v); }
    explicit fixed(long long v) { from_integer(v); }
    explicit fixed(unsigned long long v) { from_unsigned(v); }
    explicit fixed(short v) { from_integer(static_cast<long long>(v)); }
    explicit fixed(unsigned short v) { from_unsigned(v); }
    explicit fixed(signed char v) { from_integer(static_cast<long long>(v)); }
    explicit fixed(unsigned char v) { from_unsigned(v); }
    explicit fixed(double v) { from_floating(static_cast<long double>(v)); }
    explicit fixed(long double v) { from_floating(v); }

    // Conversions
    OMG_TYPES_EXPLICIT_CONVERSION operator int64_t() const {
        long double whole = 0;
        for (std::size_t k = digits_; k > scale_; --k) whole = whole * 10 + d_[k - 1];
        if (whole > static_cast<long double>(std::numeric_limits<int64_t>::max()) + (neg_ ? 1 : 0)) {
            throw std::overflow_error("omg::types::fixed: the value does not fit int64_t");
        }
        int64_t out = 0;
        for (std::size_t k = digits_; k > scale_; --k) out = out * 10 + (neg_ ? -d_[k - 1] : d_[k - 1]);
        return out;
    }
    OMG_TYPES_EXPLICIT_CONVERSION operator long double() const {
        long double out = 0;
        for (std::size_t k = digits_; k > 0; --k) out = out * 10 + d_[k - 1];
        for (unsigned short s = 0; s < scale_; ++s) out /= 10;
        return neg_ ? -out : out;
    }
    fixed round(uint16_t scale) const { return rescale(scale, true); }
    fixed truncate(uint16_t scale) const { return rescale(scale, false); }
    std::string to_string() const {
        std::string out;
        if (neg_) out += '-';
        // The integer part without the leading zeros a fixed<digits, scale>
        // has below its `digits`: a fixed<5,2> holding 1.5 is "1.50".
        std::size_t top = digits_;
        while (top > scale_ && d_[top - 1] == 0) --top;
        if (top <= scale_) {
            out += '0';
        } else {
            for (std::size_t k = top; k > scale_; --k) out += static_cast<char>('0' + d_[k - 1]);
        }
        if (scale_ > 0) {
            out += '.';
            for (std::size_t k = scale_; k > 0; --k) out += static_cast<char>('0' + d_[k - 1]);
        }
        return out;
    }

    // Operators
    fixed& operator=(const fixed& other) {
        neg_ = other.neg_;
        digits_ = other.digits_;
        scale_ = other.scale_;
        std::memcpy(d_, other.d_, sizeof d_);
        return *this;
    }
#if defined(OMG_TYPES_CXX11)
    fixed& operator=(fixed&& other) { return *this = static_cast<const fixed&>(other); }
#endif
    fixed& operator+=(const fixed& other) { return *this = add(*this, other, false); }
    fixed& operator-=(const fixed& other) { return *this = add(*this, other, true); }
    fixed& operator*=(const fixed& other) { return *this = multiply(*this, other); }
    fixed& operator/=(const fixed& other) { return *this = divide(*this, other); }
    fixed& operator++() { return *this += fixed(1); }
    fixed operator++(int) {
        fixed before(*this);
        *this += fixed(1);
        return before;
    }
    fixed& operator--() { return *this -= fixed(1); }
    fixed operator--(int) {
        fixed before(*this);
        *this -= fixed(1);
        return before;
    }
    fixed operator+() const { return *this; }
    fixed operator-() const {
        fixed out(*this);
        out.neg_ = !neg_ && !out.is_zero();
        return out;
    }
#if defined(OMG_TYPES_CXX11)
    explicit operator bool() const { return !is_zero(); }
#else
    operator bool() const { return !is_zero(); }
#endif

    // Other member functions
    uint16_t fixed_digits() const { return digits_; }
    uint16_t fixed_scale() const { return scale_; }

    friend int compare(const fixed& a, const fixed& b) {
        if (a.is_zero() && b.is_zero()) return 0;
        if (a.neg_ != b.neg_) return a.neg_ ? -1 : 1;
        const unsigned short scale = a.scale_ > b.scale_ ? a.scale_ : b.scale_;
        const int magnitude = detail::fx_compare(detail::fx_shift(a.magnitude(), scale - a.scale_),
                                                 detail::fx_shift(b.magnitude(), scale - b.scale_));
        return a.neg_ ? -magnitude : magnitude;
    }
    friend fixed add(const fixed& a, const fixed& b, bool subtract) {
        const unsigned short scale = a.scale_ > b.scale_ ? a.scale_ : b.scale_;
        const detail::fx_magnitude x = detail::fx_shift(a.magnitude(), scale - a.scale_);
        const detail::fx_magnitude y = detail::fx_shift(b.magnitude(), scale - b.scale_);
        const bool yneg = subtract ? !b.neg_ : b.neg_;
        if (a.neg_ == yneg) return make(a.neg_, detail::fx_add(x, y), scale);
        if (detail::fx_compare(x, y) >= 0) return make(a.neg_, detail::fx_sub(x, y), scale);
        return make(yneg, detail::fx_sub(y, x), scale);
    }
    friend fixed multiply(const fixed& a, const fixed& b) {
        return make(a.neg_ != b.neg_, detail::fx_mul(a.magnitude(), b.magnitude()),
                    static_cast<unsigned short>(a.scale_ + b.scale_));
    }
    friend fixed divide(const fixed& a, const fixed& b) {
        if (b.is_zero()) throw std::domain_error("omg::types::fixed: division by zero");
        // a / b = (A * 10^(b.scale + s)) / (B * 10^a.scale), with s digits
        // after the point: enough for 31 significant digits.
        const unsigned short s = max_digits;
        const detail::fx_magnitude n = detail::fx_shift(a.magnitude(), b.scale_ + s);
        const detail::fx_magnitude d = detail::fx_shift(b.magnitude(), a.scale_);
        fixed out = make(a.neg_ != b.neg_, detail::fx_div(n, d), s, true);
        return out;
    }

private:
#if defined(OMG_TYPES_CXX11)
    template <uint16_t, uint16_t>
    friend class fixed_n;

    constexpr fixed(const char* p, std::size_t n, unsigned short digits, unsigned short scale,
                    bool rescaled)
        : neg_(detail::fx_negative(p, detail::fx_check(p, n))),
          digits_(rescaled ? detail::fx_fits(p, n, digits, scale) : detail::fx_digits_of(p, n)),
          scale_(rescaled ? scale : static_cast<unsigned short>(detail::fx_scale(p, n))),
          d_{at(p, n, rescaled, scale, 0),  at(p, n, rescaled, scale, 1),
             at(p, n, rescaled, scale, 2),  at(p, n, rescaled, scale, 3),
             at(p, n, rescaled, scale, 4),  at(p, n, rescaled, scale, 5),
             at(p, n, rescaled, scale, 6),  at(p, n, rescaled, scale, 7),
             at(p, n, rescaled, scale, 8),  at(p, n, rescaled, scale, 9),
             at(p, n, rescaled, scale, 10), at(p, n, rescaled, scale, 11),
             at(p, n, rescaled, scale, 12), at(p, n, rescaled, scale, 13),
             at(p, n, rescaled, scale, 14), at(p, n, rescaled, scale, 15),
             at(p, n, rescaled, scale, 16), at(p, n, rescaled, scale, 17),
             at(p, n, rescaled, scale, 18), at(p, n, rescaled, scale, 19),
             at(p, n, rescaled, scale, 20), at(p, n, rescaled, scale, 21),
             at(p, n, rescaled, scale, 22), at(p, n, rescaled, scale, 23),
             at(p, n, rescaled, scale, 24), at(p, n, rescaled, scale, 25),
             at(p, n, rescaled, scale, 26), at(p, n, rescaled, scale, 27),
             at(p, n, rescaled, scale, 28), at(p, n, rescaled, scale, 29),
             at(p, n, rescaled, scale, 30)} {}

    static constexpr unsigned char at(const char* p, std::size_t n, bool rescaled,
                                      unsigned short scale, std::size_t k) {
        return rescaled ? detail::fx_digit_at(p, n, scale, k) : detail::fx_digit(p, n, k);
    }
#else
    void parse(const char* p, std::size_t n) {
        detail::fx_check(p, n);
        neg_ = detail::fx_negative(p, n);
        digits_ = detail::fx_digits_of(p, n);
        scale_ = static_cast<unsigned short>(detail::fx_scale(p, n));
        for (std::size_t k = 0; k < max_digits; ++k) d_[k] = detail::fx_digit(p, n, k);
    }
#endif

    bool is_zero() const {
        for (std::size_t k = 0; k < max_digits; ++k) {
            if (d_[k] != 0) return false;
        }
        return true;
    }
    detail::fx_magnitude magnitude() const {
        detail::fx_magnitude out(d_, d_ + digits_);
        detail::fx_trim(out);
        return out;
    }
    // A value from its magnitude: at most 31 digits, the fraction cut where
    // there are more; `strip` drops trailing zeros of the fraction.
    static fixed make(bool neg, detail::fx_magnitude m, unsigned short scale, bool strip = false) {
        detail::fx_trim(m);
        while (m.size() > max_digits && scale > 0) {
            m.erase(m.begin());
            --scale;
        }
        if (m.size() > max_digits) {
            throw std::overflow_error("omg::types::fixed: more than 31 integer digits");
        }
        if (strip) {
            while (scale > 0 && !m.empty() && m.front() == 0) {
                m.erase(m.begin());
                --scale;
            }
            if (m.empty()) scale = 0;
        }
        fixed out;
        std::memset(out.d_, 0, sizeof out.d_);
        for (std::size_t k = 0; k < m.size(); ++k) out.d_[k] = m[k];
        out.scale_ = scale;
        out.digits_ = static_cast<unsigned short>(m.size() > scale ? m.size() : scale);
        if (out.digits_ == 0) out.digits_ = 1;
        out.neg_ = neg && !m.empty();
        return out;
    }
    fixed rescale(uint16_t scale, bool rounded) const {
        if (scale >= scale_) return *this;
        detail::fx_magnitude m = magnitude();
        const std::size_t cut = scale_ - scale;
        bool up = false;
        if (rounded && cut <= m.size()) up = m[cut - 1] >= 5;
        m.erase(m.begin(), m.begin() + static_cast<std::ptrdiff_t>(cut < m.size() ? cut : m.size()));
        if (up) {
            detail::fx_magnitude one(1, 1);
            m = detail::fx_add(m, one);
        }
        return make(neg_, m, scale);
    }
    void from_unsigned(unsigned long long v) {
        std::ostringstream text;
        text << v;
        assign_text(text.str());
    }
    void from_integer(long long v) {
        std::ostringstream text;
        text << v;
        assign_text(text.str());
    }
    void from_floating(long double v) {
        if (!(v == v) || v > 1e31L || v < -1e31L) {
            throw std::overflow_error("omg::types::fixed: the value does not fit 31 digits");
        }
        char buffer[96];
#if defined(OMG_TYPES_CXX11)
        std::snprintf(buffer, sizeof buffer, "%.31Lf", v);
#else
        std::sprintf(buffer, "%.31Lf", v);
#endif
        std::string text(buffer);
        // 31 significant digits: cut the fraction where there are more.
        std::size_t significant = 0, keep = text.size();
        bool started = false;
        for (std::size_t i = 0; i < text.size(); ++i) {
            const char c = text[i];
            if (c < '0' || c > '9') continue;
            if (c != '0') started = true;
            if (started && ++significant > max_digits) {
                keep = i;
                break;
            }
        }
        text.erase(keep);
        if (!text.empty() && text[text.size() - 1] == '.') text.erase(text.size() - 1);
        fixed parsed = from_text(text);
        *this = make(parsed.neg_, parsed.magnitude(), parsed.scale_, true);
    }
    void assign_text(const std::string& text) { *this = from_text(text); }
    static fixed from_text(const std::string& text) {
#if defined(OMG_TYPES_CXX11)
        return fixed(string_view(text.data(), text.size()));
#else
        return fixed(text);
#endif
    }

protected:
    // The digit count a fixed<digits, scale> holds, leading zeros included.
    void set_digits(unsigned short digits) { digits_ = digits; }

private:
    bool neg_;
    unsigned short digits_;
    unsigned short scale_;
    unsigned char d_[max_digits];
};

inline std::string to_string(const fixed& value) { return value.to_string(); }
inline void swap(fixed& a, fixed& b) {
    fixed t(a);
    a = b;
    b = t;
}
inline std::istream& operator>>(std::istream& is, fixed& obj) {
    std::string text;
    if (is >> text) {
        try {
#if defined(OMG_TYPES_CXX11)
            obj = fixed(string_view(text.data(), text.size()));
#else
            obj = fixed(text);
#endif
        } catch (const std::exception&) {
            is.setstate(std::ios::failbit);
        }
    }
    return is;
}
inline std::ostream& operator<<(std::ostream& os, const fixed& obj) { return os << obj.to_string(); }
inline fixed operator+(const fixed& lhs, const fixed& rhs) { return add(lhs, rhs, false); }
inline fixed operator-(const fixed& lhs, const fixed& rhs) { return add(lhs, rhs, true); }
inline fixed operator*(const fixed& lhs, const fixed& rhs) { return multiply(lhs, rhs); }
inline fixed operator/(const fixed& lhs, const fixed& rhs) { return divide(lhs, rhs); }
inline bool operator>(const fixed& lhs, const fixed& rhs) { return compare(lhs, rhs) > 0; }
inline bool operator<(const fixed& lhs, const fixed& rhs) { return compare(lhs, rhs) < 0; }
inline bool operator>=(const fixed& lhs, const fixed& rhs) { return compare(lhs, rhs) >= 0; }
inline bool operator<=(const fixed& lhs, const fixed& rhs) { return compare(lhs, rhs) <= 0; }
inline bool operator==(const fixed& lhs, const fixed& rhs) { return compare(lhs, rhs) == 0; }
inline bool operator!=(const fixed& lhs, const fixed& rhs) { return compare(lhs, rhs) != 0; }

// fixed<Digits, Scale>: "a different type, with effectively constant
// digits and scale, that provides the same C++ interface and can be
// implicitly converted from/to the fixed class" (7.2.4.2.4). A value with
// more fraction digits than Scale is cut to Scale; one with more integer
// digits than Digits - Scale throws std::range_error.
template <uint16_t Digits, uint16_t Scale>
class fixed_n : public fixed {
#if defined(OMG_TYPES_CXX11)
    static_assert(Digits >= 1 && Digits <= 31 && Scale <= Digits,
                  "fixed<digits, scale>: 1 <= digits <= 31 and scale <= digits");
#endif

public:
#if defined(OMG_TYPES_CXX11)
    constexpr fixed_n() : fixed("0", 1, Digits, Scale, true) {}
    constexpr explicit fixed_n(string_view text)
        : fixed(text.data(), text.size(), Digits, Scale, true) {}
    constexpr explicit fixed_n(const char* text) : fixed_n(string_view(text)) {}
#else
    fixed_n() { assign(fixed()); }
    explicit fixed_n(const char* text) { assign(fixed(text)); }
    explicit fixed_n(const std::string& text) { assign(fixed(text)); }
#endif
    fixed_n(const fixed& value) { assign(value); }
    explicit fixed_n(int v) { assign(fixed(v)); }
    explicit fixed_n(long long v) { assign(fixed(v)); }
    explicit fixed_n(unsigned long long v) { assign(fixed(v)); }
    explicit fixed_n(double v) { assign(fixed(v)); }
    explicit fixed_n(long double v) { assign(fixed(v)); }

    fixed_n& operator=(const fixed& value) {
        assign(value);
        return *this;
    }
    fixed_n& operator+=(const fixed& other) { return *this = static_cast<const fixed&>(*this) + other; }
    fixed_n& operator-=(const fixed& other) { return *this = static_cast<const fixed&>(*this) - other; }
    fixed_n& operator*=(const fixed& other) { return *this = static_cast<const fixed&>(*this) * other; }
    fixed_n& operator/=(const fixed& other) { return *this = static_cast<const fixed&>(*this) / other; }

private:
    void assign(const fixed& value) {
        const std::string text = value.truncate(Scale).to_string();
        std::string padded = text;
        const std::size_t dot = padded.find('.');
        std::size_t have = dot == std::string::npos ? 0 : padded.size() - dot - 1;
        if (dot == std::string::npos && Scale > 0) padded += '.';
        for (; have < Scale; ++have) padded += '0';
#if defined(OMG_TYPES_CXX11)
        fixed::operator=(fixed(padded.data(), padded.size(), Digits, Scale, true));
#else
        fixed parsed(padded);
        if (parsed.fixed_digits() - parsed.fixed_scale() > Digits - Scale) {
            throw std::range_error("omg::types::fixed: the value has more integer digits than the type");
        }
        fixed::operator=(parsed);
        set_digits(Digits);
#endif
    }
};

// -- 7.3: Any -----------------------------------------------------------------

// "operations that allow programmers to insert and access the value
// contained in an any instance as well as the actual type of that value"
// (7.3). The DDS platform's any: any copyable value, its type as
// std::type_info.
class Any {
public:
    Any() : held_(0) {}
    Any(const Any& other) : held_(other.held_ ? other.held_->clone() : 0) {}
#if defined(OMG_TYPES_CXX11)
    Any(Any&& other) noexcept : held_(other.held_) { other.held_ = nullptr; }
    Any& operator=(Any&& other) noexcept {
        if (this != &other) {
            delete held_;
            held_ = other.held_;
            other.held_ = nullptr;
        }
        return *this;
    }
#endif
    Any& operator=(const Any& other) {
        if (this != &other) {
            Any copy(other);
            swap(copy);
        }
        return *this;
    }
    ~Any() { delete held_; }

    // Insert a copy of `value`.
    template <typename T>
    void insert(const T& value) {
        holder_base* next = new holder<T>(value);
        delete held_;
        held_ = next;
    }
    // Copy the value out when it is a T; false, and `out` unchanged, when
    // it is not.
    template <typename T>
    bool extract(T& out) const {
        if (!held_ || held_->type() != typeid(T)) return false;
        out = static_cast<const holder<T>*>(held_)->value;
        return true;
    }
    // The actual type of the value; typeid(void) for an empty any.
    const std::type_info& type() const { return held_ ? held_->type() : typeid(void); }
    bool has_value() const { return held_ != 0; }
    void reset() {
        delete held_;
        held_ = 0;
    }
    void swap(Any& other) {
        holder_base* t = held_;
        held_ = other.held_;
        other.held_ = t;
    }
    // Two anys are equal when both are empty, or both hold a value of one
    // type and the values compare equal: what a structure's comparison
    // (7.2.4.3.1) asks of an `any` member. A type inserted needs operator==.
    bool operator==(const Any& other) const {
        if (!held_ || !other.held_) return held_ == other.held_;
        return held_->equals(*other.held_);
    }
    bool operator!=(const Any& other) const { return !(*this == other); }

private:
    struct holder_base {
        virtual ~holder_base() {}
        virtual holder_base* clone() const = 0;
        virtual const std::type_info& type() const = 0;
        virtual bool equals(const holder_base& other) const = 0;
    };
    template <typename T>
    struct holder : holder_base {
        explicit holder(const T& v) : value(v) {}
        holder_base* clone() const { return new holder(value); }
        const std::type_info& type() const { return typeid(T); }
        bool equals(const holder_base& other) const {
            return other.type() == typeid(T) && value == static_cast<const holder&>(other).value;
        }
        T value;
    };
    holder_base* held_;
};

template <typename T>
inline void operator<<=(Any& any, const T& value) {
    any.insert(value);
}
template <typename T>
inline bool operator>>=(const Any& any, T& out) {
    return any.extract(out);
}
inline void swap(Any& a, Any& b) { a.swap(b); }

// -- Table 7.17: ranged -------------------------------------------------------

// "The implementation of omg::types::ranged<T, min, max> shall perform the
// corresponding checks and throw std::out_of_range if ... the value
// assigned to a ranged element is out of the predefined range." Default
// constructed it holds T's default value, or Min when that is outside.
template <typename T, T Min, T Max>
class ranged {
public:
    typedef T value_type;
    ranged() : v_(T() < Min || Max < T() ? Min : T()) {}
    ranged(T value) : v_(check(value)) {}
    ranged& operator=(T value) {
        v_ = check(value);
        return *this;
    }
    operator T() const { return v_; }
    T value() const { return v_; }
    static T min() { return Min; }
    static T max() { return Max; }

private:
    static T check(T value) {
        if (value < Min || Max < value) throw std::out_of_range("omg::types::ranged: out of range");
        return value;
    }
    T v_;
};

// The same for a type whose bounds C++ cannot pass as template arguments
// before C++20 (float, double, long double): `Limits` has static
// functions min() and max().
template <typename T, typename Limits>
class ranged_by {
public:
    typedef T value_type;
    ranged_by() : v_(T() < Limits::min() || Limits::max() < T() ? Limits::min() : T()) {}
    ranged_by(T value) : v_(check(value)) {}
    ranged_by& operator=(T value) {
        v_ = check(value);
        return *this;
    }
    operator T() const { return v_; }
    T value() const { return v_; }

private:
    static T check(T value) {
        if (value < Limits::min() || Limits::max() < value) {
            throw std::out_of_range("omg::types::ranged: out of range");
        }
        return value;
    }
    T v_;
};

// -- 7.1.4: the type traits -----------------------------------------------------

#if defined(OMG_TYPES_CXX11)

// Whether T is a type the mapping produces. True for the built-in types,
// enumerations and the template types above; the generated code says so
// for each struct, union, exception, bit set, bit mask, interface and
// value type it declares.
template <typename T>
struct is_idl_type
    : std::integral_constant<bool, std::is_arithmetic<T>::value || std::is_enum<T>::value> {};
template <typename T>
struct is_idl_type<std::vector<T>> : is_idl_type<T> {};
template <typename T, std::size_t N>
struct is_idl_type<bounded_sequence<T, N>> : is_idl_type<T> {};
template <typename CharT>
struct is_idl_type<std::basic_string<CharT>> : std::true_type {};
template <typename CharT, std::size_t N>
struct is_idl_type<basic_bounded_string<CharT, N>> : std::true_type {};
template <typename T, std::size_t N>
struct is_idl_type<std::array<T, N>> : is_idl_type<T> {};
template <typename K, typename T>
struct is_idl_type<std::map<K, T>> : std::integral_constant<bool, is_idl_type<K>::value && is_idl_type<T>::value> {};
template <typename K, typename T, std::size_t N>
struct is_idl_type<bounded_map<K, T, N>>
    : std::integral_constant<bool, is_idl_type<K>::value && is_idl_type<T>::value> {};
template <>
struct is_idl_type<fixed> : std::true_type {};
template <uint16_t D, uint16_t S>
struct is_idl_type<fixed_n<D, S>> : std::true_type {};
template <>
struct is_idl_type<Any> : std::true_type {};
template <typename T>
struct is_idl_type<optional<T>> : is_idl_type<T> {};
template <typename T, T Min, T Max>
struct is_idl_type<ranged<T, Min, Max>> : std::true_type {};
template <typename T, typename L>
struct is_idl_type<ranged_by<T, L>> : std::true_type {};
template <typename T>
struct is_idl_type<std::shared_ptr<T>> : is_idl_type<T> {};

// Whether T is an interface or a value type: passed as ref_type<T>.
template <typename T>
struct is_reference_type : std::false_type {};

namespace detail {
template <typename T>
struct by_value : std::integral_constant<bool, std::is_arithmetic<T>::value || std::is_enum<T>::value> {};
template <typename T, bool reference = is_reference_type<T>::value>
struct passing {
    typedef T value;
    typedef typename std::conditional<by_value<T>::value, T, const T&>::type in;
    typedef T& out;
};
template <typename T>
struct passing<T, true> {
    typedef ref_type<T> value;
    typedef const ref_type<T>& in;
    typedef ref_type<T>& out;
};
template <typename T>
struct enable_for_idl : std::enable_if<is_idl_type<T>::value> {};
}  // namespace detail

// Table 7.1. "The trait is undefined when the type T is incorrect."
template <typename T, typename Enable = void>
struct value_type;
template <typename T>
struct value_type<T, typename detail::enable_for_idl<T>::type> {
    typedef typename detail::passing<T>::value type;
};
template <typename T, typename Enable = void>
struct in_type;
template <typename T>
struct in_type<T, typename detail::enable_for_idl<T>::type> {
    typedef typename detail::passing<T>::in type;
};
template <typename T, typename Enable = void>
struct out_type;
template <typename T>
struct out_type<T, typename detail::enable_for_idl<T>::type> {
    typedef typename detail::passing<T>::out type;
};
template <typename T, typename Enable = void>
struct inout_type;
template <typename T>
struct inout_type<T, typename detail::enable_for_idl<T>::type> {
    typedef typename detail::passing<T>::out type;
};

// Tables 7.4, 7.5, 7.6, 7.10: is_bounded and bound.
template <typename T>
struct is_bounded;
template <typename T>
struct bound;
template <typename T>
struct is_bounded<std::vector<T>> : std::false_type {};
template <typename T>
struct bound<std::vector<T>>
    : std::integral_constant<std::size_t, std::numeric_limits<std::size_t>::max()> {};
template <typename T, std::size_t N>
struct is_bounded<bounded_sequence<T, N>> : std::true_type {};
template <typename T, std::size_t N>
struct bound<bounded_sequence<T, N>> : std::integral_constant<std::size_t, N> {};
template <typename CharT>
struct is_bounded<std::basic_string<CharT>> : std::false_type {};
template <typename CharT>
struct bound<std::basic_string<CharT>>
    : std::integral_constant<std::size_t, std::numeric_limits<std::size_t>::max()> {};
template <typename CharT, std::size_t N>
struct is_bounded<basic_bounded_string<CharT, N>> : std::true_type {};
template <typename CharT, std::size_t N>
struct bound<basic_bounded_string<CharT, N>> : std::integral_constant<std::size_t, N> {};
template <typename K, typename T>
struct is_bounded<std::map<K, T>> : std::false_type {};
template <typename K, typename T>
struct bound<std::map<K, T>>
    : std::integral_constant<std::size_t, std::numeric_limits<std::size_t>::max()> {};
template <typename K, typename T, std::size_t N>
struct is_bounded<bounded_map<K, T, N>> : std::true_type {};
template <typename K, typename T, std::size_t N>
struct bound<bounded_map<K, T, N>> : std::integral_constant<std::size_t, N> {};

// Table 7.7: digits and scale.
template <typename T>
struct digits;
template <typename T>
struct scale;
template <uint16_t D, uint16_t S>
struct digits<fixed_n<D, S>> : std::integral_constant<std::size_t, D> {};
template <uint16_t D, uint16_t S>
struct scale<fixed_n<D, S>> : std::integral_constant<std::size_t, S> {};

// Table 7.9: dimensions.
template <typename T>
struct dimensions;
namespace detail {
template <typename T>
struct array_rank : std::integral_constant<std::size_t, 0> {};
template <typename T, std::size_t N>
struct array_rank<std::array<T, N>> : std::integral_constant<std::size_t, 1 + array_rank<T>::value> {};
}  // namespace detail
template <typename T, std::size_t N>
struct dimensions<std::array<T, N>> : detail::array_rank<std::array<T, N>> {};

// Tables 7.8 and 7.12: bit_bound and underlying_type, which the generated
// code specializes for an enumeration with @bit_bound and for a bit mask.
template <typename T>
struct bit_bound;
template <typename T>
struct underlying_type;

// Table 7.11: key and elements.
template <typename T>
struct key;
template <typename T>
struct elements;
template <typename K, typename T>
struct key<std::map<K, T>> {
    typedef K type;
};
template <typename K, typename T>
struct elements<std::map<K, T>> {
    typedef T type;
};
template <typename K, typename T, std::size_t N>
struct key<bounded_map<K, T, N>> {
    typedef K type;
};
template <typename K, typename T, std::size_t N>
struct elements<bounded_map<K, T, N>> {
    typedef T type;
};

// "Each trait struct with a type or a value member shall have an alias
// ending in _t or _v, respectively." The _v aliases are variable templates,
// which C++14 introduced.
template <typename T>
using value_type_t = typename value_type<T>::type;
template <typename T>
using in_type_t = typename in_type<T>::type;
template <typename T>
using out_type_t = typename out_type<T>::type;
template <typename T>
using inout_type_t = typename inout_type<T>::type;
template <typename T>
using underlying_type_t = typename underlying_type<T>::type;
template <typename T>
using key_t = typename key<T>::type;
template <typename T>
using elements_t = typename elements<T>::type;
#if defined(OMG_TYPES_CXX14)
template <typename T>
constexpr bool is_bounded_v = is_bounded<T>::value;
template <typename T>
constexpr std::size_t bound_v = bound<T>::value;
template <typename T>
constexpr std::size_t digits_v = digits<T>::value;
template <typename T>
constexpr std::size_t scale_v = scale<T>::value;
template <typename T>
constexpr std::size_t dimensions_v = dimensions<T>::value;
template <typename T>
constexpr uint32_t bit_bound_v = bit_bound<T>::value;
#endif

#endif  // OMG_TYPES_CXX11

}  // namespace types
}  // namespace omg

#if defined(OMG_TYPES_CXX11)
namespace std {
// A reference hashes as the std::shared_ptr it is.
template <typename T>
struct hash<::omg::types::ref_type<T>> {
    std::size_t operator()(const ::omg::types::ref_type<T>& r) const noexcept {
        return std::hash<std::shared_ptr<T>>()(r);
    }
};
}  // namespace std
#endif

#endif  // OMG_TYPES_HPP
