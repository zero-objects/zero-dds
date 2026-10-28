// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// `dds::core::external<T>`, clause by clause. DDS-XTypes 1.3 §7.5.1.2.3.3.1
// to §7.5.1.2.3.3.15 and Table 46. Silent and exit 0 is the pass.
#include <cstdio>
#include <memory>
#include <stdexcept>

#include "dds/core/external.hpp"

namespace {

int failures = 0;

#define CHECK(condition)                                                       \
    do {                                                                       \
        if (!(condition)) {                                                    \
            std::printf("FAIL line %d: %s\n", __LINE__, #condition);           \
            ++failures;                                                        \
        }                                                                      \
    } while (0)

/// An object that counts its destructions.
struct Probe {
    static int destroyed;
    int value;
    explicit Probe(int v = 0) : value(v) {}
    Probe(const Probe &other) : value(other.value) {}
    Probe &operator=(const Probe &other) {
        value = other.value;
        return *this;
    }
    ~Probe() { ++destroyed; }
};
int Probe::destroyed = 0;

using dds::core::external;
using dds::core::external_locked_error;

void default_constructor() {
    external<Probe> e;
    CHECK(!e);
    CHECK(e.get() == nullptr);
    CHECK(!e.is_locked());
}

void constructor_from_a_pointer() {
    Probe *p = new Probe(3);
    external<Probe> e(p);
    CHECK(e.get() == p);
    CHECK(!e.is_locked());
    external<Probe> l(new Probe(4), true);
    CHECK(l.is_locked());
    bool refused = false;
    try {
        external<Probe> bad(static_cast<Probe *>(nullptr), true);
    } catch (const std::logic_error &) {
        refused = true;
    }
    CHECK(refused);  // locked_ can only be true for a non-NULL pointer
}

void constructor_from_a_shared_pointer() {
    auto shared = std::make_shared<Probe>(5);
    external<Probe> e(shared);
    CHECK(e.get() == shared.get());
    CHECK(e.get_shared_ptr() == shared);
    CHECK(!e.is_locked());
}

void copy_constructor() {
    external<Probe> a(new Probe(1));
    external<Probe> shared(a);
    CHECK(shared.get() == a.get());  // not locked: the reference is shared
    CHECK(!shared.is_locked());

    external<Probe> locked(new Probe(2), true);
    external<Probe> deep(locked);
    CHECK(deep.get() != locked.get());  // locked: a new T, a deep copy
    CHECK(deep->value == 2);
    CHECK(!deep.is_locked());  // either way the new object is not locked
}

void destructor() {
    Probe::destroyed = 0;
    {
        external<Probe> a(new Probe(1));
        {
            external<Probe> b(a);
            CHECK(Probe::destroyed == 0);
        }
        CHECK(Probe::destroyed == 0);  // not the last reference: only counted down
    }
    CHECK(Probe::destroyed == 1);  // the last reference releases the object
}

void accessors() {
    external<Probe> e(new Probe(7));
    const external<Probe> &c = e;
    CHECK((*e).value == 7);
    CHECK((*c).value == 7);
    CHECK(e->value == 7);
    CHECK(c->value == 7);
    CHECK(e.get() == c.get());
    e->value = 8;
    CHECK(c.get()->value == 8);
    external<Probe> empty;
    bool refused = false;
    try {
        (void)*empty;
    } catch (const std::logic_error &) {
        refused = true;
    }
    CHECK(refused);
}

void comparison_and_bool() {
    external<Probe> a(new Probe(1));
    external<Probe> same(a);
    external<Probe> other(new Probe(1));
    external<Probe> empty1;
    external<Probe> empty2;
    CHECK(a == same);
    CHECK(a != other);
    CHECK(empty1 == empty2);  // both empty
    CHECK(a != empty1);
    CHECK(static_cast<bool>(a));
    CHECK(!static_cast<bool>(empty1));
}

void lock_and_is_locked() {
    external<Probe> e(new Probe(1));
    CHECK(!e.is_locked());
    e.lock();
    CHECK(e.is_locked());
    external<Probe> empty;
    bool refused = false;
    try {
        empty.lock();
    } catch (const std::logic_error &) {
        refused = true;
    }
    CHECK(refused);
}

// Table 46, row by row.
void assignment_destination_locked_is_an_error() {
    external<Probe> dst(new Probe(1), true);
    external<Probe> src(new Probe(2));
    bool refused = false;
    try {
        dst = src;
    } catch (const external_locked_error &) {
        refused = true;
    }
    CHECK(refused);
    CHECK(dst->value == 1);
}

void assignment_source_empty_resets_the_destination() {
    external<Probe> dst(new Probe(1));
    external<Probe> src;
    dst = src;
    CHECK(!dst);
    external<Probe> locked_src;  // an empty source is never locked
    external<Probe> dst2(new Probe(1));
    dst2 = locked_src;
    CHECK(!dst2);
}

void assignment_empty_destination_locked_source_creates_and_deep_copies() {
    external<Probe> src(new Probe(9), true);
    external<Probe> dst;
    dst = src;
    CHECK(dst);
    CHECK(dst.get() != src.get());
    CHECK(dst->value == 9);
}

void assignment_existing_destination_locked_source_reuses_and_deep_copies() {
    external<Probe> src(new Probe(9), true);
    external<Probe> dst(new Probe(1));
    Probe *existing = dst.get();
    dst = src;
    CHECK(dst.get() == existing);  // the existing object is reused
    CHECK(dst->value == 9);
    CHECK(dst.get() != src.get());
}

void assignment_unlocked_source_is_a_shallow_copy() {
    external<Probe> src(new Probe(9));
    external<Probe> dst(new Probe(1));
    dst = src;
    CHECK(dst.get() == src.get());
    external<Probe> empty_dst;
    empty_dst = src;
    CHECK(empty_dst.get() == src.get());
}

}  // namespace

int main() {
    default_constructor();
    constructor_from_a_pointer();
    constructor_from_a_shared_pointer();
    copy_constructor();
    destructor();
    accessors();
    comparison_and_bool();
    lock_and_is_locked();
    assignment_destination_locked_is_an_error();
    assignment_source_empty_resets_the_destination();
    assignment_empty_destination_locked_source_creates_and_deep_copies();
    assignment_existing_destination_locked_source_reuses_and_deep_copies();
    assignment_unlocked_source_is_a_shallow_copy();
    return failures == 0 ? 0 : 1;
}
