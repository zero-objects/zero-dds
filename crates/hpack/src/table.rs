// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! HPACK static + dynamic table — RFC 7541 §2.3.
//!
//! Static table: 61 fixed entries (Spec Appendix A).
//! Dynamic table: FIFO with a `header_table_size` limit (configured
//! by the caller via SETTINGS_HEADER_TABLE_SIZE).

use alloc::collections::VecDeque;
use alloc::string::String;

/// Static-table entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticTableEntry {
    /// Header name.
    pub name: &'static str,
    /// Default value (may be empty).
    pub value: &'static str,
}

/// RFC 7541 Appendix A — 61 static-table entries (indices 1..=61).
pub const STATIC_TABLE: [StaticTableEntry; 61] = [
    StaticTableEntry {
        name: ":authority",
        value: "",
    },
    StaticTableEntry {
        name: ":method",
        value: "GET",
    },
    StaticTableEntry {
        name: ":method",
        value: "POST",
    },
    StaticTableEntry {
        name: ":path",
        value: "/",
    },
    StaticTableEntry {
        name: ":path",
        value: "/index.html",
    },
    StaticTableEntry {
        name: ":scheme",
        value: "http",
    },
    StaticTableEntry {
        name: ":scheme",
        value: "https",
    },
    StaticTableEntry {
        name: ":status",
        value: "200",
    },
    StaticTableEntry {
        name: ":status",
        value: "204",
    },
    StaticTableEntry {
        name: ":status",
        value: "206",
    },
    StaticTableEntry {
        name: ":status",
        value: "304",
    },
    StaticTableEntry {
        name: ":status",
        value: "400",
    },
    StaticTableEntry {
        name: ":status",
        value: "404",
    },
    StaticTableEntry {
        name: ":status",
        value: "500",
    },
    StaticTableEntry {
        name: "accept-charset",
        value: "",
    },
    StaticTableEntry {
        name: "accept-encoding",
        value: "gzip, deflate",
    },
    StaticTableEntry {
        name: "accept-language",
        value: "",
    },
    StaticTableEntry {
        name: "accept-ranges",
        value: "",
    },
    StaticTableEntry {
        name: "accept",
        value: "",
    },
    StaticTableEntry {
        name: "access-control-allow-origin",
        value: "",
    },
    StaticTableEntry {
        name: "age",
        value: "",
    },
    StaticTableEntry {
        name: "allow",
        value: "",
    },
    StaticTableEntry {
        name: "authorization",
        value: "",
    },
    StaticTableEntry {
        name: "cache-control",
        value: "",
    },
    StaticTableEntry {
        name: "content-disposition",
        value: "",
    },
    StaticTableEntry {
        name: "content-encoding",
        value: "",
    },
    StaticTableEntry {
        name: "content-language",
        value: "",
    },
    StaticTableEntry {
        name: "content-length",
        value: "",
    },
    StaticTableEntry {
        name: "content-location",
        value: "",
    },
    StaticTableEntry {
        name: "content-range",
        value: "",
    },
    StaticTableEntry {
        name: "content-type",
        value: "",
    },
    StaticTableEntry {
        name: "cookie",
        value: "",
    },
    StaticTableEntry {
        name: "date",
        value: "",
    },
    StaticTableEntry {
        name: "etag",
        value: "",
    },
    StaticTableEntry {
        name: "expect",
        value: "",
    },
    StaticTableEntry {
        name: "expires",
        value: "",
    },
    StaticTableEntry {
        name: "from",
        value: "",
    },
    StaticTableEntry {
        name: "host",
        value: "",
    },
    StaticTableEntry {
        name: "if-match",
        value: "",
    },
    StaticTableEntry {
        name: "if-modified-since",
        value: "",
    },
    StaticTableEntry {
        name: "if-none-match",
        value: "",
    },
    StaticTableEntry {
        name: "if-range",
        value: "",
    },
    StaticTableEntry {
        name: "if-unmodified-since",
        value: "",
    },
    StaticTableEntry {
        name: "last-modified",
        value: "",
    },
    StaticTableEntry {
        name: "link",
        value: "",
    },
    StaticTableEntry {
        name: "location",
        value: "",
    },
    StaticTableEntry {
        name: "max-forwards",
        value: "",
    },
    StaticTableEntry {
        name: "proxy-authenticate",
        value: "",
    },
    StaticTableEntry {
        name: "proxy-authorization",
        value: "",
    },
    StaticTableEntry {
        name: "range",
        value: "",
    },
    StaticTableEntry {
        name: "referer",
        value: "",
    },
    StaticTableEntry {
        name: "refresh",
        value: "",
    },
    StaticTableEntry {
        name: "retry-after",
        value: "",
    },
    StaticTableEntry {
        name: "server",
        value: "",
    },
    StaticTableEntry {
        name: "set-cookie",
        value: "",
    },
    StaticTableEntry {
        name: "strict-transport-security",
        value: "",
    },
    StaticTableEntry {
        name: "transfer-encoding",
        value: "",
    },
    StaticTableEntry {
        name: "user-agent",
        value: "",
    },
    StaticTableEntry {
        name: "vary",
        value: "",
    },
    StaticTableEntry {
        name: "via",
        value: "",
    },
    StaticTableEntry {
        name: "www-authenticate",
        value: "",
    },
];

/// Header field (owned) — stored in the dynamic table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderField {
    /// Name.
    pub name: String,
    /// Value.
    pub value: String,
}

impl HeaderField {
    /// Spec §4.1 — estimated size (32 + name.len + value.len).
    #[must_use]
    pub fn size(&self) -> usize {
        32 + self.name.len() + self.value.len()
    }
}

/// Combined lookup — index into static + dynamic. RFC 7541 §2.3.
///
/// Static = 1..=61, dynamic = 62..=N.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    dynamic: VecDeque<HeaderField>,
    /// Current total size of the dynamic table (Spec §4.1).
    size: usize,
    /// Max size per SETTINGS_HEADER_TABLE_SIZE.
    max_size: usize,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            dynamic: VecDeque::new(),
            size: 0,
            max_size: 4096, // Spec §6.5.2 default
        }
    }
}

impl Table {
    /// Constructor with `max_size`.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            dynamic: VecDeque::new(),
            size: 0,
            max_size,
        }
    }

    /// Adds a new entry to the dynamic table (at the front).
    /// Spec §4.4.
    pub fn add(&mut self, field: HeaderField) {
        let new_size = field.size();
        if new_size > self.max_size {
            // Spec §4.4: an oversized entry => clear the table.
            self.dynamic.clear();
            self.size = 0;
            return;
        }
        // Evict from back until fits.
        while self.size + new_size > self.max_size {
            if let Some(removed) = self.dynamic.pop_back() {
                self.size -= removed.size();
            } else {
                break;
            }
        }
        self.size += new_size;
        self.dynamic.push_front(field);
    }

    /// Lookup by combined index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<HeaderField> {
        if index == 0 {
            return None;
        }
        if index <= STATIC_TABLE.len() {
            let e = STATIC_TABLE[index - 1];
            return Some(HeaderField {
                name: alloc::string::ToString::to_string(e.name),
                value: alloc::string::ToString::to_string(e.value),
            });
        }
        let dyn_index = index - STATIC_TABLE.len() - 1;
        self.dynamic.get(dyn_index).cloned()
    }

    /// Searches for a header-field match. Returns `(index, full_match)`.
    /// `full_match=true`: name + value match.
    /// `full_match=false`: only the name matches.
    #[must_use]
    pub fn find(&self, name: &str, value: &str) -> Option<(usize, bool)> {
        let mut name_only: Option<usize> = None;
        // Static
        for (i, e) in STATIC_TABLE.iter().enumerate() {
            if e.name == name {
                if e.value == value {
                    return Some((i + 1, true));
                }
                name_only.get_or_insert(i + 1);
            }
        }
        // Dynamic
        for (i, h) in self.dynamic.iter().enumerate() {
            let idx = STATIC_TABLE.len() + 1 + i;
            if h.name == name {
                if h.value == value {
                    return Some((idx, true));
                }
                name_only.get_or_insert(idx);
            }
        }
        name_only.map(|i| (i, false))
    }

    /// Current dynamic-table size (Spec §4.1).
    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }

    /// Max size.
    #[must_use]
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Sets a new max size — on shrink, entries are evicted from the
    /// back.
    pub fn set_max_size(&mut self, new_max: usize) {
        self.max_size = new_max;
        while self.size > self.max_size {
            if let Some(removed) = self.dynamic.pop_back() {
                self.size -= removed.size();
            } else {
                break;
            }
        }
    }

    /// Number of dynamic-table entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.dynamic.len()
    }

    /// `true` if the dynamic table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dynamic.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn hf(n: &str, v: &str) -> HeaderField {
        HeaderField {
            name: n.into(),
            value: v.into(),
        }
    }

    #[test]
    fn static_table_has_61_entries() {
        assert_eq!(STATIC_TABLE.len(), 61);
        assert_eq!(STATIC_TABLE[0].name, ":authority");
        assert_eq!(STATIC_TABLE[60].name, "www-authenticate");
    }

    #[test]
    fn lookup_static_index_1() {
        let t = Table::new(4096);
        let h = t.get(1).unwrap();
        assert_eq!(h.name, ":authority");
        assert_eq!(h.value, "");
    }

    #[test]
    fn lookup_static_index_2_get() {
        let t = Table::new(4096);
        let h = t.get(2).unwrap();
        assert_eq!(h.name, ":method");
        assert_eq!(h.value, "GET");
    }

    #[test]
    fn dynamic_add_and_lookup() {
        let mut t = Table::new(4096);
        t.add(hf("custom", "value"));
        assert_eq!(t.len(), 1);
        let h = t.get(62).unwrap();
        assert_eq!(h.name, "custom");
        assert_eq!(h.value, "value");
    }

    #[test]
    fn dynamic_evicts_oldest_on_overflow() {
        let mut t = Table::new(64); // small
        t.add(hf("a", "1")); // size = 32 + 1 + 1 = 34
        t.add(hf("b", "2")); // would push out 'a'
        assert_eq!(t.len(), 1);
        assert_eq!(t.get(62).unwrap().name, "b");
    }

    #[test]
    fn entry_too_large_clears_table() {
        let mut t = Table::new(50);
        t.add(hf("a", "1"));
        let huge = hf("very-long-name-that-exceeds-max", "value");
        t.add(huge);
        assert!(t.is_empty(), "table should be cleared per Spec §4.4");
    }

    #[test]
    fn find_static_full_match() {
        let t = Table::new(4096);
        let r = t.find(":method", "GET");
        assert_eq!(r, Some((2, true)));
    }

    #[test]
    fn find_static_name_only() {
        let t = Table::new(4096);
        let r = t.find(":method", "PATCH");
        assert!(r.unwrap().0 == 2 || r.unwrap().0 == 3);
        assert!(!r.unwrap().1);
    }

    #[test]
    fn find_dynamic() {
        let mut t = Table::new(4096);
        t.add(hf("custom", "value"));
        let r = t.find("custom", "value");
        assert_eq!(r, Some((62, true)));
    }

    #[test]
    fn set_max_size_shrinks_evicting() {
        let mut t = Table::new(4096);
        t.add(hf("a", "1"));
        t.add(hf("b", "2"));
        t.set_max_size(34);
        assert!(t.len() <= 1);
    }

    #[test]
    fn header_field_size_is_32_plus_name_plus_value() {
        let h = hf("abc", "12345");
        assert_eq!(h.size(), 32 + 3 + 5);
    }

    #[test]
    fn lookup_zero_index_returns_none() {
        let t = Table::new(4096);
        assert!(t.get(0).is_none());
    }

    #[test]
    fn find_for_name_string() {
        let t = Table::new(4096);
        let r = t.find(":method", "GET");
        let h = t.get(r.unwrap().0).unwrap();
        assert_eq!(h.value.to_string(), "GET");
    }
}
