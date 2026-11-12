// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds/classic.hpp — the types the classic OMG IDL to C++ mapping (IDL to C++ 1.3) names, in
// C++03: `CORBA::Long` and the other basic types, `CORBA::String_var`, `CORBA::WString_var`, and
// three class templates the generated declarations use — `zerodds::classic::Sequence<T>`,
// `zerodds::classic::Var<T>` and `zerodds::classic::Map<K, V, N>` — with the functions that copy
// and release the plain pointer of an `@optional` member (IDL4 to C++ 1.0 Table D.3).
//
// A header generated with `--cpp-mapping classic` declares its structures, unions, enumerations
// and typedefs in these terms, between the markers `ZD-CLASSIC-BEGIN` and `ZD-CLASSIC-END`; that
// part of the header is C++03 and includes this file and nothing newer. Memory follows the
// mapping: a string is a `char*` the `String_var` owns and frees with `CORBA::string_free`, a
// sequence owns its buffer, a variable-length member is copied deeply.

#ifndef ZERODDS_CLASSIC_HPP
#define ZERODDS_CLASSIC_HPP

#include <map>
#include <stdexcept>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

namespace CORBA {

typedef int16_t Short;
typedef int32_t Long;
typedef int64_t LongLong;
typedef uint16_t UShort;
typedef uint32_t ULong;
typedef uint64_t ULongLong;
typedef float Float;
typedef double Double;
typedef long double LongDouble;
typedef char Char;
typedef wchar_t WChar;
typedef bool Boolean;
typedef unsigned char Octet;
typedef signed char Int8;
typedef unsigned char UInt8;

/// `string_alloc(len)`: room for `len` characters and the terminating NUL.
inline char* string_alloc(ULong len) { return static_cast<char*>(malloc(static_cast<size_t>(len) + 1)); }

/// `string_dup(s)`: a copy to free with `string_free`; NULL stays NULL.
inline char* string_dup(const char* s) {
    if (s == 0) {
        return 0;
    }
    const size_t length = strlen(s);
    char* copy = static_cast<char*>(malloc(length + 1));
    if (copy != 0) {
        memcpy(copy, s, length + 1);
    }
    return copy;
}

/// `string_free(s)`: NULL is allowed.
inline void string_free(char* s) { free(s); }

/// `wstring_alloc(len)`.
inline wchar_t* wstring_alloc(ULong len) {
    return static_cast<wchar_t*>(malloc((static_cast<size_t>(len) + 1) * sizeof(wchar_t)));
}

/// `wstring_dup(s)`.
inline wchar_t* wstring_dup(const wchar_t* s) {
    if (s == 0) {
        return 0;
    }
    const size_t length = wcslen(s);
    wchar_t* copy = static_cast<wchar_t*>(malloc((length + 1) * sizeof(wchar_t)));
    if (copy != 0) {
        memcpy(copy, s, (length + 1) * sizeof(wchar_t));
    }
    return copy;
}

/// `wstring_free(s)`.
inline void wstring_free(wchar_t* s) { free(s); }

/// The `_var` type of `char*` (IDL to C++ 1.3 §6.? String_var): owns its string, copies on
/// assignment, frees on destruction.
class String_var {
public:
    String_var() : s_(0) {}
    String_var(char* p) : s_(p) {}
    String_var(const char* p) : s_(string_dup(p)) {}
    String_var(const String_var& other) : s_(string_dup(other.s_)) {}
    ~String_var() { string_free(s_); }

    String_var& operator=(char* p) {
        if (s_ != p) {
            string_free(s_);
            s_ = p;
        }
        return *this;
    }
    String_var& operator=(const char* p) {
        char* copy = string_dup(p);
        string_free(s_);
        s_ = copy;
        return *this;
    }
    String_var& operator=(const String_var& other) {
        if (this != &other) {
            char* copy = string_dup(other.s_);
            string_free(s_);
            s_ = copy;
        }
        return *this;
    }

    operator const char*() const { return s_; }
    const char* in() const { return s_; }
    char*& inout() { return s_; }
    /// Frees the string and hands out the pointer to fill.
    char*& out() {
        string_free(s_);
        s_ = 0;
        return s_;
    }
    /// Gives up ownership.
    char* _retn() {
        char* p = s_;
        s_ = 0;
        return p;
    }
    char& operator[](ULong index) { return s_[index]; }
    char operator[](ULong index) const { return s_[index]; }

private:
    char* s_;
};

/// The `_var` type of `wchar_t*`.
class WString_var {
public:
    WString_var() : s_(0) {}
    WString_var(wchar_t* p) : s_(p) {}
    WString_var(const wchar_t* p) : s_(wstring_dup(p)) {}
    WString_var(const WString_var& other) : s_(wstring_dup(other.s_)) {}
    ~WString_var() { wstring_free(s_); }

    WString_var& operator=(wchar_t* p) {
        if (s_ != p) {
            wstring_free(s_);
            s_ = p;
        }
        return *this;
    }
    WString_var& operator=(const wchar_t* p) {
        wchar_t* copy = wstring_dup(p);
        wstring_free(s_);
        s_ = copy;
        return *this;
    }
    WString_var& operator=(const WString_var& other) {
        if (this != &other) {
            wchar_t* copy = wstring_dup(other.s_);
            wstring_free(s_);
            s_ = copy;
        }
        return *this;
    }

    operator const wchar_t*() const { return s_; }
    const wchar_t* in() const { return s_; }
    wchar_t*& inout() { return s_; }
    wchar_t* _retn() {
        wchar_t* p = s_;
        s_ = 0;
        return p;
    }

private:
    wchar_t* s_;
};

}  // namespace CORBA

namespace zerodds {
namespace classic {

/// `sequence<T>` (IDL to C++ 1.3 §6.? sequences): `length()`, `maximum()`, `operator[]`; the
/// buffer is owned and copied deeply. `length(n)` keeps the elements it has and default-
/// constructs the new ones.
template <class T>
class Sequence {
public:
    Sequence() : maximum_(0), length_(0), buffer_(0) {}
    explicit Sequence(CORBA::ULong max) : maximum_(max), length_(0), buffer_(0) { reserve(max); }
    Sequence(const Sequence& other) : maximum_(0), length_(0), buffer_(0) { *this = other; }
    ~Sequence() { release_all(); }

    Sequence& operator=(const Sequence& other) {
        if (this != &other) {
            T* copy = other.length_ == 0 ? 0 : new T[other.length_];
            for (CORBA::ULong i = 0; i < other.length_; ++i) {
                copy[i] = other.buffer_[i];
            }
            release_all();
            buffer_ = copy;
            length_ = other.length_;
            maximum_ = other.length_;
        }
        return *this;
    }

    CORBA::ULong length() const { return length_; }
    CORBA::ULong maximum() const { return maximum_; }

    void length(CORBA::ULong n) {
        if (n > maximum_) {
            reserve(n);
        }
        // Elements past the old length are fresh ones, not what an earlier shrink left behind.
        for (CORBA::ULong i = length_; i < n; ++i) {
            buffer_[i] = T();
        }
        length_ = n;
    }

    T& operator[](CORBA::ULong index) { return buffer_[index]; }
    const T& operator[](CORBA::ULong index) const { return buffer_[index]; }

    /// Releases the buffer: the sequence is empty with no capacity.
    void release_all() {
        delete[] buffer_;
        buffer_ = 0;
        length_ = 0;
        maximum_ = 0;
    }

private:
    void reserve(CORBA::ULong n) {
        if (n <= maximum_ && buffer_ != 0) {
            return;
        }
        T* grown = n == 0 ? 0 : new T[n];
        for (CORBA::ULong i = 0; i < length_; ++i) {
            grown[i] = buffer_[i];
        }
        delete[] buffer_;
        buffer_ = grown;
        maximum_ = n;
    }

    CORBA::ULong maximum_;
    CORBA::ULong length_;
    T* buffer_;
};

/// A `_var` of a variable-length type, and the holder of an `@external` member: an owned
/// pointer, copied deeply, `NULL` when empty.
template <class T>
class Var {
public:
    Var() : p_(0) {}
    Var(T* p) : p_(p) {}
    Var(const Var& other) : p_(other.p_ == 0 ? 0 : new T(*other.p_)) {}
    ~Var() { delete p_; }

    Var& operator=(T* p) {
        if (p_ != p) {
            delete p_;
            p_ = p;
        }
        return *this;
    }
    Var& operator=(const Var& other) {
        if (this != &other) {
            T* copy = other.p_ == 0 ? 0 : new T(*other.p_);
            delete p_;
            p_ = copy;
        }
        return *this;
    }

    T* operator->() const { return p_; }
    T& operator*() const { return *p_; }
    T* ptr() const { return p_; }
    const T& in() const { return *p_; }
    T& inout() { return *p_; }
    T* _retn() {
        T* p = p_;
        p_ = 0;
        return p;
    }
    bool operator==(const Var& other) const {
        if (p_ == 0 || other.p_ == 0) {
            return p_ == other.p_;
        }
        return *p_ == *other.p_;
    }

private:
    T* p_;
};

/// `map<K, V>` and `map<K, V, N>` (IDL4 to C++ 1.0 Annex D.2.1.3.1): a default constructor that
/// leaves the map empty, `operator[]`, `maximum()`, `length()`, `clear()`, `insert()`,
/// `insert_or_assign()` and `erase()`; an unbounded map (`N` 0) also has the copy constructor,
/// the assignment, the maximum constructor and `maximum(ULong)`. A bounded map holds at most `N`
/// entries, an unbounded one at most its `maximum()`, which starts at the greatest `ULong`.
template <class K, class V, CORBA::ULong N = 0>
class Map {
public:
    typedef std::map<K, V> Entries;
    typedef typename Entries::const_iterator const_iterator;

    Map() : maximum_(N == 0 ? 0xFFFFFFFFu : N) {}
    /// The maximum constructor: "allowing internal preallocations if necessary".
    explicit Map(CORBA::ULong maximum) : maximum_(N == 0 ? maximum : N) {}
    Map(const Map& other) : maximum_(other.maximum_), entries_(other.entries_) {}
    ~Map() {}
    Map& operator=(const Map& other) {
        if (this != &other) {
            maximum_ = other.maximum_;
            entries_ = other.entries_;
        }
        return *this;
    }

    /// The value of `key`; a key the map does not hold is inserted with a default value, as
    /// `std::map` does, and `std::length_error` when the map is full.
    V& operator[](const K& key) {
        typename Entries::iterator found = entries_.find(key);
        if (found != entries_.end()) {
            return found->second;
        }
        if (length() + 1 > maximum_) {
            throw std::length_error("zerodds::classic::Map: a new key past the maximum");
        }
        return entries_[key];
    }
    /// The value of `key`, or a default value when the map does not hold it.
    const V& operator[](const K& key) const {
        typename Entries::const_iterator found = entries_.find(key);
        return found != entries_.end() ? found->second : absent_;
    }

    CORBA::ULong maximum() const { return maximum_; }
    /// The maximum of an unbounded map; the entries past it, in key order, are dropped.
    void maximum(CORBA::ULong max_size) {
        if (N != 0) {
            return;
        }
        maximum_ = max_size;
        while (entries_.size() > maximum_) {
            typename Entries::iterator last = entries_.end();
            --last;
            entries_.erase(last);
        }
    }
    CORBA::ULong length() const { return static_cast<CORBA::ULong>(entries_.size()); }
    void clear() { entries_.clear(); }

    /// FALSE when the key is held and `replace` is FALSE, and when a new entry would take the
    /// length past the maximum; TRUE when the entry is in the map.
    CORBA::Boolean insert(const K& key, const V& element, CORBA::Boolean replace = true) {
        typename Entries::iterator found = entries_.find(key);
        if (found != entries_.end()) {
            if (!replace) {
                return false;
            }
            found->second = element;
            return true;
        }
        if (length() + 1 > maximum_) {
            return false;
        }
        entries_.insert(std::make_pair(key, element));
        return true;
    }
    CORBA::Boolean insert_or_assign(const K& key, const V& element) {
        return insert(key, element, true);
    }
    CORBA::Boolean erase(const K& key) { return entries_.erase(key) == 1; }

    const_iterator begin() const { return entries_.begin(); }
    const_iterator end() const { return entries_.end(); }
    bool operator==(const Map& other) const { return entries_ == other.entries_; }

private:
    CORBA::ULong maximum_;
    Entries entries_;
    V absent_;
};

/// The plain pointer of an `@optional` member (IDL4 to C++ 1.0 Table D.3), owned by the structure
/// that holds it: `clone` is its deep copy, `release` frees it; NULL stays NULL.
template <class T>
inline T* clone(const T* p) {
    return p == 0 ? 0 : new T(*p);
}
inline char* clone(const char* p) { return CORBA::string_dup(p); }
inline wchar_t* clone(const wchar_t* p) { return CORBA::wstring_dup(p); }
template <class T>
inline void release(T* p) {
    delete p;
}
inline void release(char* p) { CORBA::string_free(p); }
inline void release(wchar_t* p) { CORBA::wstring_free(p); }

/// The copy of a member, element by element for a C array.
template <class T>
inline void copy_value(T& to, const T& from) {
    to = from;
}
template <class T, size_t S>
inline void copy_value(T (&to)[S], const T (&from)[S]) {
    for (size_t i = 0; i < S; ++i) {
        copy_value(to[i], from[i]);
    }
}

}  // namespace classic
}  // namespace zerodds

#endif  // ZERODDS_CLASSIC_HPP
