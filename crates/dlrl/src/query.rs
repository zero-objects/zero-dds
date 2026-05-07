// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Query-Engine — DDS 1.4 §B.7.
//!
//! Spec §B.7: DLRL bietet objektzentrische Queries mit Filter,
//! Order und Limit ueber den Object-Cache.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::object_cache::{ObjectCache, ObjectRef};

/// Sortierreihenfolge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Aufsteigend.
    Ascending,
    /// Absteigend.
    Descending,
}

/// Query-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    /// Limit ueberschreitet u32::MAX.
    LimitTooLarge,
    /// Topic-Filter ist leer-string (Spec verbietet das).
    EmptyTopic,
}

impl core::fmt::Display for QueryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::LimitTooLarge => f.write_str("limit exceeds u32::MAX"),
            Self::EmptyTopic => f.write_str("topic filter must be non-empty"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for QueryError {}

/// Query-Result — Slice-aehnliche Liste von ObjectRefs.
pub type QueryResult = Vec<ObjectRef>;

/// Custom-Filter-Function.
pub type FilterFn = Box<dyn Fn(&ObjectRef) -> bool + Send + Sync>;

/// Sort-Key-Extraction-Function.
pub type SortKeyFn = Box<dyn Fn(&ObjectRef) -> Vec<u8> + Send + Sync>;

/// Query-Builder. Spec §B.7.
pub struct Query {
    topic_filter: Option<String>,
    state_filter: Option<crate::object_cache::ObjectState>,
    custom_filter: Option<FilterFn>,
    sort: Option<(SortOrder, SortKeyFn)>,
    limit: Option<usize>,
}

impl core::fmt::Debug for Query {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Query")
            .field("topic_filter", &self.topic_filter)
            .field("state_filter", &self.state_filter)
            .field("has_custom_filter", &self.custom_filter.is_some())
            .field("has_sort", &self.sort.is_some())
            .field("limit", &self.limit)
            .finish()
    }
}

impl Default for Query {
    fn default() -> Self {
        Self::new()
    }
}

impl Query {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            topic_filter: None,
            state_filter: None,
            custom_filter: None,
            sort: None,
            limit: None,
        }
    }

    /// Topic-Filter — nur Objekte dieses Topics.
    ///
    /// # Errors
    /// `EmptyTopic` wenn der Filter ein Leerstring ist.
    pub fn topic(mut self, topic: &str) -> Result<Self, QueryError> {
        if topic.is_empty() {
            return Err(QueryError::EmptyTopic);
        }
        self.topic_filter = Some(topic.into());
        Ok(self)
    }

    /// State-Filter.
    #[must_use]
    pub fn state(mut self, state: crate::object_cache::ObjectState) -> Self {
        self.state_filter = Some(state);
        self
    }

    /// Custom-Filter.
    #[must_use]
    pub fn filter<F>(mut self, f: F) -> Self
    where
        F: Fn(&ObjectRef) -> bool + Send + Sync + 'static,
    {
        self.custom_filter = Some(Box::new(f));
        self
    }

    /// Sortierung — Caller liefert eine Key-Extraction-Funktion.
    #[must_use]
    pub fn order_by<F>(mut self, order: SortOrder, key_fn: F) -> Self
    where
        F: Fn(&ObjectRef) -> Vec<u8> + Send + Sync + 'static,
    {
        self.sort = Some((order, Box::new(key_fn)));
        self
    }

    /// Limit.
    ///
    /// # Errors
    /// `LimitTooLarge` wenn `limit > u32::MAX`.
    pub fn limit(mut self, limit: usize) -> Result<Self, QueryError> {
        if limit > u32::MAX as usize {
            return Err(QueryError::LimitTooLarge);
        }
        self.limit = Some(limit);
        Ok(self)
    }

    /// Spec §B.7.1 — Execute Query gegen einen ObjectCache.
    pub fn execute(&self, cache: &ObjectCache) -> QueryResult {
        let mut out: Vec<ObjectRef> = cache
            .iter()
            .filter(|o| match &self.topic_filter {
                Some(t) => o.id.topic == *t,
                None => true,
            })
            .filter(|o| match &self.state_filter {
                Some(s) => o.lifecycle == *s,
                None => true,
            })
            .filter(|o| match &self.custom_filter {
                Some(f) => f(o),
                None => true,
            })
            .cloned()
            .collect();
        if let Some((order, key_fn)) = &self.sort {
            out.sort_by_key(|o| key_fn(o));
            if matches!(order, SortOrder::Descending) {
                out.reverse();
            }
        }
        if let Some(n) = self.limit {
            out.truncate(n);
        }
        out
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::object_cache::{ObjectCache, ObjectId};

    fn populate(c: &mut ObjectCache) {
        c.register(
            ObjectId::new("Trade".into(), b"AAPL".to_vec()),
            alloc::vec![3],
        );
        c.register(
            ObjectId::new("Trade".into(), b"GOOG".to_vec()),
            alloc::vec![1],
        );
        c.register(
            ObjectId::new("Quote".into(), b"AAPL".to_vec()),
            alloc::vec![2],
        );
    }

    #[test]
    fn empty_query_returns_all() {
        let mut c = ObjectCache::new();
        populate(&mut c);
        let r = Query::new().execute(&c);
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn topic_filter_narrows_result() {
        let mut c = ObjectCache::new();
        populate(&mut c);
        let r = Query::new().topic("Trade").unwrap().execute(&c);
        assert_eq!(r.len(), 2);
        assert!(r.iter().all(|o| o.id.topic == "Trade"));
    }

    #[test]
    fn empty_topic_rejected() {
        let err = Query::new().topic("").unwrap_err();
        assert_eq!(err, QueryError::EmptyTopic);
    }

    #[test]
    fn limit_caps_result_size() {
        let mut c = ObjectCache::new();
        populate(&mut c);
        let r = Query::new().limit(2).unwrap().execute(&c);
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn order_by_sorts_ascending() {
        let mut c = ObjectCache::new();
        populate(&mut c);
        let r = Query::new()
            .order_by(SortOrder::Ascending, |o| o.id.key.clone())
            .execute(&c);
        assert_eq!(r[0].id.key, b"AAPL"); // appears in both Trade & Quote, stable sort
    }

    #[test]
    fn order_by_descending_reverses() {
        let mut c = ObjectCache::new();
        populate(&mut c);
        let r_asc = Query::new()
            .order_by(SortOrder::Ascending, |o| o.id.key.clone())
            .execute(&c);
        let r_desc = Query::new()
            .order_by(SortOrder::Descending, |o| o.id.key.clone())
            .execute(&c);
        assert_eq!(r_asc[0].id.key, r_desc[r_desc.len() - 1].id.key);
    }

    #[test]
    fn custom_filter_applied() {
        let mut c = ObjectCache::new();
        populate(&mut c);
        let r = Query::new()
            .filter(|o| o.state == alloc::vec![3])
            .execute(&c);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id.key, b"AAPL");
    }

    #[test]
    fn limit_too_large_rejected() {
        let err = Query::new().limit(u32::MAX as usize + 1).unwrap_err();
        assert_eq!(err, QueryError::LimitTooLarge);
    }

    #[test]
    fn state_filter_only_returns_matching() {
        let mut c = ObjectCache::new();
        populate(&mut c);
        let r = Query::new()
            .state(crate::object_cache::ObjectState::New)
            .execute(&c);
        assert_eq!(r.len(), 3);
    }
}
