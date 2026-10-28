/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The operations of Table 47 on the C binding of a map: DDS-XTypes 1.3
 * §7.5.1.3.1 and §7.5.1.3.2.
 *
 * A generated header declares each map as
 *
 *     typedef struct <ENTRY> { <KEY_T> key; <ELEM_T> value; } <ENTRY>;
 *     typedef struct <MAP> {
 *         uint32_t _maximum;   the capacity: "the maximum size limits the
 *                              number of entries the map may contain"
 *         uint32_t _length;    the size: "how many entries are currently
 *                              present"
 *         <ENTRY> *_buffer;    the entries, in the order they were inserted
 *         bool     _release;   whether `_buffer` is the map's own heap
 *                              block (the operations free and replace it)
 *                              or somebody else's (the arena of a generated
 *                              reader, or the application's array)
 *     } <MAP>;
 *
 * and invokes ZD_MAP_DEFINE once, which defines the operations named
 * `<MAP>_new`, `<MAP>_delete`, `<MAP>_initialize`, `<MAP>_finalize`,
 * `<MAP>_copy`, `<MAP>_get_size`, `<MAP>_get_max_size`,
 * `<MAP>_set_max_size`, `<MAP>_clear`, `<MAP>_insert`,
 * `<MAP>_insert_or_assign`, `<MAP>_erase`, `<MAP>_get_first`,
 * `<MAP>_get_next`, `<MAP>_find_element`, `<MAP>_find_entry` and
 * `<MAP>_get_pair`.
 *
 * Return codes are those of DDS 1.4 §2.2.2.1.1; the clause says which
 * operation returns which, and the places it is silent are named below.
 *
 * - A key of the entry that is a string is stored as the pointer given
 *   and compared by its characters: the map does not own the characters,
 *   as no other string of this binding is owned by the structure that
 *   holds it.
 * - An entry removed by `erase` or `clear` is not finalized: an element is
 *   copied with memcpy or assignment, so the map owns no storage an
 *   element refers to.
 * - A key that is not in the map is `DDS_RETCODE_BAD_PARAMETER` for
 *   `erase`, as a key that is in it is for `insert` (§7.5.1.3.2.11).
 */
#ifndef ZERODDS_MAP_H
#define ZERODDS_MAP_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#ifdef __cplusplus
extern "C" {
#endif

/* DDS 1.4 §2.2.2.1.1 ReturnCode_t, where no DDS header has defined it. */
#ifndef DDS_RETCODE_OK
typedef int32_t DDS_ReturnCode_t;
#define DDS_RETCODE_OK 0
#define DDS_RETCODE_ERROR 1
#define DDS_RETCODE_UNSUPPORTED 2
#define DDS_RETCODE_BAD_PARAMETER 3
#define DDS_RETCODE_PRECONDITION_NOT_MET 4
#define DDS_RETCODE_OUT_OF_RESOURCES 5
#define DDS_RETCODE_NOT_ENABLED 6
#define DDS_RETCODE_IMMUTABLE_POLICY 7
#define DDS_RETCODE_INCONSISTENT_POLICY 8
#define DDS_RETCODE_ALREADY_DELETED 9
#define DDS_RETCODE_TIMEOUT 10
#define DDS_RETCODE_NO_DATA 11
#define DDS_RETCODE_ILLEGAL_OPERATION 12
#endif

/* The `MapEntry` of Table 47: a position in a map, or a sentinel. A
 * `valid` entry names the entry at `index`; the sentinel is what
 * `get_first` answers for an empty map, `find_entry` for an absent key
 * and `get_next` after the last entry. An entry stands for a position:
 * `erase` and `insert` invalidate the entries taken before them. */
#ifndef ZD_MAPENTRY_DEFINED
#define ZD_MAPENTRY_DEFINED
typedef struct ZdMapEntry {
    uint32_t index;
    bool valid;
} MapEntry;
#endif

/* Key kinds: how a key is compared and what makes it "NULL". */
#define ZD_MAP_KEYEQ_SCALAR(a, b) ((a) == (b))
#define ZD_MAP_KEYNULL_SCALAR(k) (0)
#define ZD_MAP_KEYEQ_STRING(a, b) (strcmp((a), (b)) == 0)
#define ZD_MAP_KEYNULL_STRING(k) ((k) == NULL)

/* Element passing: a primitive element is "generated as the type and not
 * as a pointer to the primitive type" (§7.5.1.3.2.11), every other one is
 * passed by pointer. */
#define ZD_MAP_IN_VAL(T) T
#define ZD_MAP_NULL_VAL(e) (0)
#define ZD_MAP_STORE_VAL(dst, e, T) ((dst) = (e))
#define ZD_MAP_IN_PTR(T) const T *
#define ZD_MAP_NULL_PTR(e) ((e) == NULL)
#define ZD_MAP_STORE_PTR(dst, e, T) memcpy(&(dst), (e), sizeof(T))

/* Whether `n` entries of type `T` do not fit in a `size_t` of bytes: only a platform whose
 * `size_t` is no wider than the 32-bit capacity can say yes. */
#if SIZE_MAX <= 0xFFFFFFFFu
#define ZD_MAP_TOO_LARGE(n, T) ((size_t)(n) > SIZE_MAX / sizeof(T))
#else
#define ZD_MAP_TOO_LARGE(n, T) (0)
#endif

/* `BOUND` is the declared bound of `map<K, V, N>`, or 0 for none. */
#define ZD_MAP_DEFINE(MAP, ENTRY, KEY_T, ELEM_T, KEYKIND, PASS, BOUND)                           \
    static inline void MAP##_initialize(MAP *map) {                                              \
        if (map != NULL) {                                                                       \
            map->_maximum = 0u;                                                                  \
            map->_length = 0u;                                                                   \
            map->_buffer = NULL;                                                                 \
            map->_release = false;                                                               \
        }                                                                                        \
    }                                                                                            \
    static inline uint32_t MAP##_get_size(const MAP *map) {                                      \
        return map != NULL ? map->_length : 0u;                                                  \
    }                                                                                            \
    static inline uint32_t MAP##_get_max_size(const MAP *map) {                                  \
        return map != NULL ? map->_maximum : 0u;                                                 \
    }                                                                                            \
    static inline void MAP##_clear(MAP *map) {                                                   \
        if (map != NULL) {                                                                       \
            map->_length = 0u;                                                                   \
        }                                                                                        \
    }                                                                                            \
    static inline DDS_ReturnCode_t MAP##_set_max_size(MAP *map, uint32_t max_size) {             \
        ENTRY *grown;                                                                            \
        if (map == NULL || max_size < map->_length ||                                            \
            ((BOUND) != 0u && max_size > (uint32_t)(BOUND))) {                                   \
            return DDS_RETCODE_BAD_PARAMETER;                                                    \
        }                                                                                        \
        if (max_size == map->_maximum) {                                                         \
            return DDS_RETCODE_OK;                                                               \
        }                                                                                        \
        if (max_size == 0u) {                                                                    \
            if (map->_release) {                                                                 \
                free(map->_buffer);                                                              \
            }                                                                                    \
            map->_buffer = NULL;                                                                 \
            map->_maximum = 0u;                                                                  \
            map->_release = false;                                                               \
            return DDS_RETCODE_OK;                                                               \
        }                                                                                        \
        if (ZD_MAP_TOO_LARGE(max_size, ENTRY)) {                                                 \
            return DDS_RETCODE_ERROR;                                                            \
        }                                                                                        \
        grown = (ENTRY *)malloc((size_t)max_size * sizeof(ENTRY));                               \
        if (grown == NULL) {                                                                     \
            return DDS_RETCODE_ERROR;                                                            \
        }                                                                                        \
        if (map->_length > 0u) {                                                                 \
            memcpy(grown, map->_buffer, (size_t)map->_length * sizeof(ENTRY));                   \
        }                                                                                        \
        if (map->_release) {                                                                     \
            free(map->_buffer);                                                                  \
        }                                                                                        \
        map->_buffer = grown;                                                                    \
        map->_maximum = max_size;                                                                \
        map->_release = true;                                                                    \
        return DDS_RETCODE_OK;                                                                   \
    }                                                                                            \
    static inline void MAP##_finalize(MAP *map) {                                                \
        MAP##_clear(map);                                                                        \
        (void)MAP##_set_max_size(map, 0u);                                                       \
    }                                                                                            \
    static inline MAP *MAP##_new(void) {                                                         \
        MAP *map = (MAP *)malloc(sizeof(MAP));                                                   \
        if (map != NULL) {                                                                       \
            MAP##_initialize(map);                                                               \
        }                                                                                        \
        return map;                                                                              \
    }                                                                                            \
    static inline void MAP##_delete(MAP *map) {                                                  \
        if (map != NULL) {                                                                       \
            MAP##_finalize(map);                                                                 \
            free(map);                                                                           \
        }                                                                                        \
    }                                                                                            \
    static inline uint32_t MAP##_zd_find(const MAP *map, KEY_T key) {                            \
        uint32_t i;                                                                              \
        for (i = 0u; i < map->_length; i = i + 1u) {                                             \
            if (ZD_MAP_KEYEQ_##KEYKIND(map->_buffer[i].key, key)) {                              \
                return i;                                                                        \
            }                                                                                    \
        }                                                                                        \
        return map->_length;                                                                     \
    }                                                                                            \
    static inline DDS_ReturnCode_t MAP##_copy(MAP *map, const MAP *other, bool autogrow) {       \
        uint32_t i;                                                                              \
        if (map == NULL || other == NULL) {                                                      \
            return DDS_RETCODE_BAD_PARAMETER;                                                    \
        }                                                                                        \
        if (map == other) {                                                                      \
            return DDS_RETCODE_OK;                                                               \
        }                                                                                        \
        if (other->_length > map->_maximum) {                                                    \
            DDS_ReturnCode_t grown;                                                              \
            if (!autogrow) {                                                                     \
                return DDS_RETCODE_PRECONDITION_NOT_MET;                                         \
            }                                                                                    \
            grown = MAP##_set_max_size(map, other->_length);                                     \
            if (grown != DDS_RETCODE_OK) {                                                       \
                return grown;                                                                    \
            }                                                                                    \
        }                                                                                        \
        for (i = 0u; i < other->_length; i = i + 1u) {                                           \
            map->_buffer[i] = other->_buffer[i];                                                 \
        }                                                                                        \
        map->_length = other->_length;                                                           \
        return DDS_RETCODE_OK;                                                                   \
    }                                                                                            \
    static inline DDS_ReturnCode_t MAP##_insert(MAP *map, KEY_T key,                             \
                                                ZD_MAP_IN_##PASS(ELEM_T) element) {              \
        if (map == NULL || ZD_MAP_KEYNULL_##KEYKIND(key) || ZD_MAP_NULL_##PASS(element)) {       \
            return DDS_RETCODE_BAD_PARAMETER;                                                    \
        }                                                                                        \
        if (MAP##_zd_find(map, key) < map->_length) {                                            \
            return DDS_RETCODE_BAD_PARAMETER;                                                    \
        }                                                                                        \
        if (map->_length >= map->_maximum) {                                                     \
            return DDS_RETCODE_PRECONDITION_NOT_MET;                                             \
        }                                                                                        \
        map->_buffer[map->_length].key = key;                                                    \
        ZD_MAP_STORE_##PASS(map->_buffer[map->_length].value, element, ELEM_T);                  \
        map->_length = map->_length + 1u;                                                        \
        return DDS_RETCODE_OK;                                                                   \
    }                                                                                            \
    static inline DDS_ReturnCode_t MAP##_insert_or_assign(MAP *map, KEY_T key,                   \
                                                          ZD_MAP_IN_##PASS(ELEM_T) element) {    \
        uint32_t at;                                                                             \
        if (map == NULL || ZD_MAP_KEYNULL_##KEYKIND(key) || ZD_MAP_NULL_##PASS(element)) {       \
            return DDS_RETCODE_BAD_PARAMETER;                                                    \
        }                                                                                        \
        at = MAP##_zd_find(map, key);                                                            \
        if (at < map->_length) {                                                                 \
            ZD_MAP_STORE_##PASS(map->_buffer[at].value, element, ELEM_T);                        \
            return DDS_RETCODE_OK;                                                               \
        }                                                                                        \
        return MAP##_insert(map, key, element);                                                  \
    }                                                                                            \
    static inline DDS_ReturnCode_t MAP##_erase(MAP *map, KEY_T key) {                            \
        uint32_t at;                                                                             \
        if (map == NULL || ZD_MAP_KEYNULL_##KEYKIND(key)) {                                      \
            return DDS_RETCODE_BAD_PARAMETER;                                                    \
        }                                                                                        \
        at = MAP##_zd_find(map, key);                                                            \
        if (at >= map->_length) {                                                                \
            return DDS_RETCODE_BAD_PARAMETER;                                                    \
        }                                                                                        \
        if (at + 1u < map->_length) {                                                            \
            memmove(&map->_buffer[at], &map->_buffer[at + 1u],                                   \
                    (size_t)(map->_length - at - 1u) * sizeof(ENTRY));                           \
        }                                                                                        \
        map->_length = map->_length - 1u;                                                        \
        return DDS_RETCODE_OK;                                                                   \
    }                                                                                            \
    static inline MapEntry MAP##_get_first(const MAP *map) {                                     \
        MapEntry entry;                                                                          \
        entry.index = 0u;                                                                        \
        entry.valid = map != NULL && map->_length > 0u;                                          \
        return entry;                                                                            \
    }                                                                                            \
    static inline bool MAP##_get_next(const MAP *map, MapEntry *entry) {                         \
        if (map == NULL || entry == NULL || !entry->valid || entry->index >= map->_length) {     \
            return false;                                                                        \
        }                                                                                        \
        if (entry->index + 1u < map->_length) {                                                  \
            entry->index = entry->index + 1u;                                                    \
            return true;                                                                         \
        }                                                                                        \
        entry->index = map->_length;                                                             \
        entry->valid = false;                                                                    \
        return false;                                                                            \
    }                                                                                            \
    static inline ELEM_T *MAP##_find_element(MAP *map, KEY_T key) {                              \
        uint32_t at;                                                                             \
        if (map == NULL || ZD_MAP_KEYNULL_##KEYKIND(key)) {                                      \
            return NULL;                                                                         \
        }                                                                                        \
        at = MAP##_zd_find(map, key);                                                            \
        return at < map->_length ? &map->_buffer[at].value : NULL;                               \
    }                                                                                            \
    static inline MapEntry MAP##_find_entry(const MAP *map, KEY_T key) {                         \
        MapEntry entry;                                                                          \
        entry.index = 0u;                                                                        \
        entry.valid = false;                                                                     \
        if (map != NULL && !ZD_MAP_KEYNULL_##KEYKIND(key)) {                                     \
            entry.index = MAP##_zd_find(map, key);                                               \
            entry.valid = entry.index < map->_length;                                            \
        }                                                                                        \
        return entry;                                                                            \
    }                                                                                            \
    static inline bool MAP##_get_pair(MAP *map, const MapEntry *entry, KEY_T *key,               \
                                      ELEM_T **element) {                                        \
        if (map == NULL || entry == NULL || key == NULL || element == NULL || !entry->valid ||   \
            entry->index >= map->_length) {                                                      \
            return false;                                                                        \
        }                                                                                        \
        *key = map->_buffer[entry->index].key;                                                   \
        *element = &map->_buffer[entry->index].value;                                            \
        return true;                                                                             \
    }

#ifdef __cplusplus
}
#endif

#endif /* ZERODDS_MAP_H */
