// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds/map.hpp — the Traditional C++ binding of a map: DDS-XTypes 1.3 §7.5.1.3.1 (Table 47)
// and §7.5.1.3.3.
//
// "The map class shall be named the same as the C structure [...] except that it is placed in
// the same namespace as the element type declaration." A generated header declares, beside each
// type `T` of the namespace `ns`, the three classes the clause asks for:
//
//     namespace ns {
//     using UInt32TMap = ::zerodds::Map<std::uint32_t, T>;
//     using UInt64TMap = ::zerodds::Map<std::uint64_t, T>;
//     using StringTMap = ::zerodds::Map<std::string, T>;
//     }
//
// and a member `map<unsigned long, T> m;` has the type of the first. A bounded `map<K, V, N>` is
// `Map<K, V, N>`.
//
// A map is a `std::vector<std::pair<K, V>>` that keeps the entries in the order they were
// inserted — the order the entries go on the wire in, which is what makes the order a setting of
// the codec (`MapEntryOrder`) and not of the container — with the capacity and the operations of
// Table 47:
//
//   * a new map has size and capacity 0 (`initialize`); `set_max_size` gives it room, and
//     `insert` past the capacity is RETCODE_PRECONDITION_NOT_MET (§7.5.1.3.2.11);
//   * `insert(key, element, replace)` is the clause's `insert` for `replace == false` and its
//     `insert_or_assign` for `replace == true` (§7.5.1.3.3.1);
//   * a map a reader filled has the capacity of the entries it read;
//   * `MapEntry` is a position, invalidated by the `erase` or `insert` that follows it.
//
// The other members of the base class are those of `std::vector`; a generated codec reaches the
// entries through them.

#ifndef ZERODDS_MAP_HPP
#define ZERODDS_MAP_HPP

#include <stddef.h>
#include <stdint.h>

#include <utility>
#include <vector>

#include "dds/return_code.hpp"

namespace zerodds {

template <class K, class V, uint32_t Bound = 0>
class Map : public std::vector<std::pair<K, V> > {
public:
    typedef std::vector<std::pair<K, V> > Base;
    typedef std::pair<K, V> Entry;

    /// A position in the map: `get_first`, `find_entry` and `get_next` answer one, and an
    /// invalid one is the sentinel.
    class MapEntry {
    public:
        MapEntry() : index_(0), valid_(false) {}
        bool valid() const { return valid_; }

    private:
        friend class Map;
        size_t index_;
        bool valid_;
    };

    /// `initialize`: "The initial size and capacity of the map shall be 0."
    Map() : max_(0) {}

    /// The entries of a vector, as the PSM conversion hands them over.
    Map(const Base& entries) : Base(entries), max_(0) {}

    /// `copy`: overwrite this map with `other`. Entries of this map that are not in `other` are
    /// erased and `other` is not changed. If `other` is larger than the capacity of this map,
    /// `autogrow` decides: the capacity grows, or the operation is
    /// RETCODE_PRECONDITION_NOT_MET and this map stays as it was.
    DDS::ReturnCode_t copy(const Map& other, bool autogrow = true) {
        if (&other == this) {
            return DDS::RETCODE_OK;
        }
        if (other.size() > get_max_size()) {
            if (!autogrow) {
                return DDS::RETCODE_PRECONDITION_NOT_MET;
            }
            const DDS::ReturnCode_t grown = set_max_size(static_cast<uint32_t>(other.size()));
            if (grown != DDS::RETCODE_OK) {
                return grown;
            }
        }
        try {
            Base::operator=(static_cast<const Base&>(other));
        } catch (...) {
            return DDS::RETCODE_ERROR;
        }
        return DDS::RETCODE_OK;
    }

    /// `get_size`: how many entries are present.
    uint32_t get_size() const { return static_cast<uint32_t>(Base::size()); }

    /// `get_max_size`: how many entries the map may contain. A map a codec filled holds the
    /// entries it read, so the capacity is never below the size.
    uint32_t get_max_size() const {
        const uint32_t size = get_size();
        return max_ < size ? size : max_;
    }

    /// `set_max_size`: RETCODE_BAD_PARAMETER if the new maximum is below the size (§7.5.1.3.2.9),
    /// or above the bound of a bounded map; RETCODE_ERROR if the room cannot be had.
    DDS::ReturnCode_t set_max_size(uint32_t max_size) {
        if (max_size < get_size() || (Bound != 0 && max_size > Bound)) {
            return DDS::RETCODE_BAD_PARAMETER;
        }
        try {
            Base::reserve(max_size);
        } catch (...) {
            return DDS::RETCODE_ERROR;
        }
        max_ = max_size;
        return DDS::RETCODE_OK;
    }

    // `clear` is the base class's: the size is 0 and the capacity does not change.

    /// `insert` for `replace == false`, `insert_or_assign` for `replace == true`: an existing key
    /// is RETCODE_BAD_PARAMETER or has its element replaced, a new one past the capacity is
    /// RETCODE_PRECONDITION_NOT_MET.
    DDS::ReturnCode_t insert(const K& key, const V& element, bool replace = true) {
        const size_t at = position_of(key);
        if (at != Base::size()) {
            if (!replace) {
                return DDS::RETCODE_BAD_PARAMETER;
            }
            try {
                (*this)[at].second = element;
            } catch (...) {
                return DDS::RETCODE_ERROR;
            }
            return DDS::RETCODE_OK;
        }
        if (Base::size() >= get_max_size()) {
            return DDS::RETCODE_PRECONDITION_NOT_MET;
        }
        try {
            Base::push_back(Entry(key, element));
        } catch (...) {
            return DDS::RETCODE_ERROR;
        }
        return DDS::RETCODE_OK;
    }

    /// `erase`: the entry of the key is removed and the others keep their order. A key that is
    /// not in the map is RETCODE_BAD_PARAMETER, as one that is in it is for `insert`.
    DDS::ReturnCode_t erase(const K& key) {
        const size_t at = position_of(key);
        if (at == Base::size()) {
            return DDS::RETCODE_BAD_PARAMETER;
        }
        Base::erase(Base::begin() + static_cast<std::ptrdiff_t>(at));
        return DDS::RETCODE_OK;
    }

    /// `get_first`: the first entry, or the sentinel for an empty map.
    MapEntry get_first() const {
        MapEntry entry;
        entry.valid_ = !Base::empty();
        return entry;
    }

    /// `get_next`: advance to the next entry. After the last one the entry is the sentinel and
    /// the answer is false.
    bool get_next(MapEntry& entry) const {
        if (!entry.valid_ || entry.index_ >= Base::size()) {
            return false;
        }
        if (entry.index_ + 1 < Base::size()) {
            entry.index_ += 1;
            return true;
        }
        entry.index_ = Base::size();
        entry.valid_ = false;
        return false;
    }

    /// `find_element`: the element of the key, or a null pointer.
    V* find_element(const K& key) {
        const size_t at = position_of(key);
        return at == Base::size() ? 0 : &(*this)[at].second;
    }

    const V* find_element(const K& key) const {
        const size_t at = position_of(key);
        return at == Base::size() ? 0 : &(*this)[at].second;
    }

    /// `find_entry`: the entry of the key, or the sentinel.
    MapEntry find_entry(const K& key) const {
        MapEntry entry;
        const size_t at = position_of(key);
        entry.index_ = at;
        entry.valid_ = at != Base::size();
        return entry;
    }

    /// `get_pair`: the key and the element of an entry; false for the sentinel and for a null
    /// argument.
    bool get_pair(const MapEntry& entry, K* key, V** element) {
        if (key == 0 || element == 0 || !entry.valid_ || entry.index_ >= Base::size()) {
            return false;
        }
        *key = (*this)[entry.index_].first;
        *element = &(*this)[entry.index_].second;
        return true;
    }

private:
    size_t position_of(const K& key) const {
        for (size_t i = 0; i < Base::size(); ++i) {
            if ((*this)[i].first == key) {
                return i;
            }
        }
        return Base::size();
    }

    uint32_t max_;
};

}  // namespace zerodds

#endif  // ZERODDS_MAP_HPP
