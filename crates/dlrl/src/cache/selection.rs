// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `Selection`, `SelectionCriterion`, `FilterCriterion`,
//! `QueryCriterion` and `SelectionListener` (DDS 1.2 §8.1.6.3.9 to
//! §8.1.6.3.13).
//!
//! A [`Selection`] is the subset of the objects of one [`ObjectHome`]
//! (its class and the derived classes, in the Cache) that pass its
//! [`SelectionCriterion`]. It is created by
//! [`ObjectHome::create_selection`] and belongs to that home; the
//! criterion belongs to the Selection.
//!
//! * The criterion is a [`FilterCriterion`], an algorithm of the
//!   application (`check_object`, TRUE by default, §8.1.6.3.11), or a
//!   [`QueryCriterion`], an expression of Annex B with its parameters
//!   (§8.1.6.3.12). A query may navigate across relations; a related
//!   object that is not available makes its predicate unknown.
//! * The members are examined against the state of the Selection the
//!   last time it was examined (§8.1.6.3.9): with `auto_refresh` TRUE at
//!   each update round of the Cache, for each object the round created,
//!   modified or deleted (in the order of §8.1.6.4), with `auto_refresh`
//!   FALSE at each [`Selection::refresh`]. `create_selection` examines
//!   the objects of the Cache once, before a listener can be set.
//! * The [`SelectionListener`] hears `on_object_in` when an object
//!   enters, `on_object_out` when it leaves (also by its deletion) and
//!   `on_object_modified` when a member that stays has changed since it
//!   was last examined: its content, and with `concerns_contained` TRUE
//!   also the content of an object it contains (compositions, nested).

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use std::sync::{Mutex, MutexGuard};

use crate::criterion::QueryExpression;
use crate::mapping::Mapping;
use crate::metamodel::DlrlException;
use crate::model::ObjectRef;
use crate::rows::ObjectData;
use crate::types::{CriterionKind, MembershipState};

use super::CacheBase;
use super::home::{HomeInner, ObjectHome};
use super::object::{ObjectRoot, ObjectState};

/// `FilterCriterion` (§8.1.6.3.11): a criterion whose check is an
/// algorithm of the application. The generated `FooFilter` is this
/// trait for the objects of `Foo`.
pub trait FilterCriterion: Send + Sync {
    /// `check_object`: TRUE when `an_object` passes the filter.
    /// `membership_state` tells whether it passed the last time the
    /// Selection examined it. The provided behaviour returns TRUE.
    fn check_object(&self, an_object: &ObjectRoot, membership_state: MembershipState) -> bool {
        let _ = (an_object, membership_state);
        true
    }
}

pub(crate) struct QueryState {
    expr: QueryExpression,
    selection: Option<Weak<SelectionInner>>,
    /// Set by `delete_selection` of the Selection the criterion belonged
    /// to (§8.1.6.3.7: it "deletes the Selection and its attached
    /// SelectionCriterion").
    deleted: bool,
}

pub(crate) struct QueryInner {
    mapping: Arc<Mapping>,
    class: String,
    state: Mutex<QueryState>,
}

/// `QueryCriterion` (§8.1.6.3.12): a criterion whose check is a query
/// expression in the syntax of Annex B with its parameters. It also has
/// the `check_object` of a filter (the generated `FooQuery` derives from
/// `FooFilter`).
#[derive(Clone)]
pub struct QueryCriterion {
    pub(crate) inner: Arc<QueryInner>,
}

impl core::fmt::Debug for QueryCriterion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let st = self.lock();
        f.debug_struct("QueryCriterion")
            .field("class", &self.inner.class)
            .field("expression", &st.expr.expression())
            .field("parameters", &st.expr.parameters())
            .finish()
    }
}

impl PartialEq for QueryCriterion {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl QueryCriterion {
    /// A query over the objects of the class of `home`.
    ///
    /// # Errors
    /// `SQLError` for an expression that is not valid for the class or
    /// parameters that do not fit it (Annex B).
    pub fn new(
        home: &ObjectHome,
        expression: &str,
        parameters: &[String],
    ) -> Result<Self, DlrlException> {
        let mapping = Arc::clone(home.mapping());
        let expr =
            QueryExpression::new(mapping.model(), home.class_name(), expression, parameters)?;
        Ok(Self {
            inner: Arc::new(QueryInner {
                mapping,
                class: home.class_name().into(),
                state: Mutex::new(QueryState {
                    expr,
                    selection: None,
                    deleted: false,
                }),
            }),
        })
    }

    fn lock(&self) -> MutexGuard<'_, QueryState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `kind`: [`CriterionKind::Query`].
    #[must_use]
    pub fn kind(&self) -> CriterionKind {
        CriterionKind::Query
    }

    /// The class the query applies to.
    #[must_use]
    pub fn class_name(&self) -> &str {
        &self.inner.class
    }

    /// `expression`.
    #[must_use]
    pub fn expression(&self) -> String {
        self.lock().expr.expression().into()
    }

    /// `parameters`.
    #[must_use]
    pub fn parameters(&self) -> Vec<String> {
        self.lock().expr.parameters().to_vec()
    }

    /// The Selection the criterion belongs to, `None` before
    /// `create_selection` and after `delete_selection`.
    #[must_use]
    pub fn selection(&self) -> Option<Selection> {
        self.lock()
            .selection
            .as_ref()
            .and_then(Weak::upgrade)
            .map(|inner| Selection { inner })
    }

    /// TRUE once `delete_selection` deleted the Selection the criterion
    /// belonged to, and with it the criterion (§8.1.6.3.7). `set_query`
    /// and `set_parameters` of a deleted criterion raise `AlreadyDeleted`
    /// (§8.1.6.2, §8.2.1.1); it cannot be attached to another Selection.
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.lock().deleted
    }

    /// `set_query`: a new expression with its parameters. TRUE: they are
    /// changed; afterwards the owning Selection is refreshed if its
    /// `auto_refresh` is TRUE (§8.1.6.3.12). On an error the query keeps
    /// its expression and parameters; a failure of the expression or the
    /// parameters is the `SQLError` the IDL of §8.2.1.2.1 declares.
    ///
    /// # Errors
    /// `SQLError` for an expression that is not valid for the class or
    /// parameters that do not fit it; `AlreadyDeleted` for a criterion
    /// deleted with its Selection ([`QueryCriterion::is_deleted`]); the
    /// errors of [`Selection::refresh`].
    pub fn set_query(&self, expression: &str, arguments: &[String]) -> Result<bool, DlrlException> {
        if self.is_deleted() {
            return Err(Self::deleted());
        }
        let expr = QueryExpression::new(
            self.inner.mapping.model(),
            &self.inner.class,
            expression,
            arguments,
        )?;
        self.lock().expr = expr;
        self.after_change()?;
        Ok(true)
    }

    /// `set_parameters`: new values of the parameters, as many as the
    /// expression requires. TRUE: they are changed; afterwards the owning
    /// Selection is refreshed if its `auto_refresh` is TRUE.
    ///
    /// # Errors
    /// `SQLError` when the count or a value does not fit the expression;
    /// `AlreadyDeleted` for a criterion deleted with its Selection; the
    /// errors of [`Selection::refresh`].
    pub fn set_parameters(&self, arguments: &[String]) -> Result<bool, DlrlException> {
        {
            let mut st = self.lock();
            if st.deleted {
                return Err(Self::deleted());
            }
            st.expr.set_parameters(arguments)?;
        }
        self.after_change()?;
        Ok(true)
    }

    fn deleted() -> DlrlException {
        DlrlException::already_deleted("the QueryCriterion was deleted with its Selection")
    }

    fn after_change(&self) -> Result<(), DlrlException> {
        match self.selection() {
            Some(s) if s.auto_refresh() => s.refresh(),
            _ => Ok(()),
        }
    }

    /// `check_object`: TRUE when the query is TRUE for `an_object`. A
    /// relation is navigated in the Cache or CacheAccess of the object.
    #[must_use]
    pub fn check_object(&self, an_object: &ObjectRoot, membership_state: MembershipState) -> bool {
        let _ = membership_state;
        // The service's own evaluation: it does not load the state of an
        // object whose home has auto_deref FALSE (§8.1.6.3.7).
        let data = an_object.current_data();
        let base = an_object.owner();
        let resolve = |r: &ObjectRef| -> Option<ObjectData> {
            base.as_ref()
                .and_then(|b| b.lookup(r))
                .map(|o| o.current_data())
        };
        let expr = self.lock().expr.clone();
        expr.check(&data, &resolve)
    }
}

/// `SelectionCriterion` (§8.1.6.3.10): the criterion of a Selection,
/// a [`FilterCriterion`] or a [`QueryCriterion`].
#[derive(Clone)]
pub enum SelectionCriterion {
    /// A `FilterCriterion` of the application.
    Filter(Arc<dyn FilterCriterion>),
    /// A `QueryCriterion`.
    Query(QueryCriterion),
}

impl core::fmt::Debug for SelectionCriterion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Filter(_) => f.write_str("SelectionCriterion::Filter"),
            Self::Query(q) => f.debug_tuple("SelectionCriterion::Query").field(q).finish(),
        }
    }
}

impl SelectionCriterion {
    /// `kind`.
    #[must_use]
    pub fn kind(&self) -> CriterionKind {
        match self {
            Self::Filter(_) => CriterionKind::Filter,
            Self::Query(_) => CriterionKind::Query,
        }
    }

    fn check(&self, o: &ObjectRoot, membership: MembershipState) -> bool {
        match self {
            Self::Filter(f) => f.check_object(o, membership),
            Self::Query(q) => q.check_object(o, membership),
        }
    }
}

/// `SelectionListener` (§8.1.6.3.13): the application is made aware of
/// the changes of a Selection. The generated `FooSelectionListener` is
/// this trait for the objects of `Foo`.
pub trait SelectionListener: Send + Sync {
    /// An object entered the Selection.
    fn on_object_in(&self, the_object: &ObjectRoot);
    /// An object left the Selection.
    fn on_object_out(&self, the_object: &ObjectRoot);
    /// The contents of a member changed.
    fn on_object_modified(&self, the_object: &ObjectRoot);
}

/// One member with its state at its last examination.
struct Member {
    object: ObjectRoot,
    seen: Vec<ObjectData>,
}

pub(crate) struct SelectionState {
    members: Vec<Member>,
    not_members: BTreeSet<ObjectRef>,
    listener: Option<Arc<dyn SelectionListener>>,
    deleted: bool,
}

pub(crate) struct SelectionInner {
    home: Weak<HomeInner>,
    criterion: SelectionCriterion,
    auto_refresh: bool,
    concerns_contained: bool,
    state: Mutex<SelectionState>,
}

/// `Selection` (§8.1.6.3.9): the objects of one home that pass a
/// criterion. The generated `FooSelection` is a Selection of `FooHome`.
#[derive(Clone)]
pub struct Selection {
    pub(crate) inner: Arc<SelectionInner>,
}

impl core::fmt::Debug for Selection {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Selection")
            .field("criterion", &self.inner.criterion)
            .field("auto_refresh", &self.inner.auto_refresh)
            .field("concerns_contained", &self.inner.concerns_contained)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Selection {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

/// What one examination of an object found.
enum Event {
    In,
    Out,
    Modified,
}

impl Selection {
    pub(crate) fn new(
        home: &ObjectHome,
        criterion: SelectionCriterion,
        auto_refresh: bool,
        concerns_contained: bool,
    ) -> Self {
        Self {
            inner: Arc::new(SelectionInner {
                home: Arc::downgrade(&home.inner),
                criterion,
                auto_refresh,
                concerns_contained,
                state: Mutex::new(SelectionState {
                    members: Vec::new(),
                    not_members: BTreeSet::new(),
                    listener: None,
                    deleted: false,
                }),
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, SelectionState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `criterion`.
    #[must_use]
    pub fn criterion(&self) -> &SelectionCriterion {
        &self.inner.criterion
    }

    /// `auto_refresh`.
    #[must_use]
    pub fn auto_refresh(&self) -> bool {
        self.inner.auto_refresh
    }

    /// `concerns_contained`.
    #[must_use]
    pub fn concerns_contained(&self) -> bool {
        self.inner.concerns_contained
    }

    /// `members`, in the order they entered.
    #[must_use]
    pub fn members(&self) -> Vec<ObjectRoot> {
        self.lock()
            .members
            .iter()
            .map(|m| m.object.clone())
            .collect()
    }

    /// `listener`.
    #[must_use]
    pub fn listener(&self) -> Option<Arc<dyn SelectionListener>> {
        self.lock().listener.clone()
    }

    /// TRUE once `delete_selection` deleted the Selection.
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.lock().deleted
    }

    /// `AlreadyDeleted` for a deleted Selection: every operation raises
    /// it (§8.1.6.2, §8.2.1.1). The attributes keep their value (no
    /// members, no listener).
    pub(crate) fn alive(&self) -> Result<(), DlrlException> {
        if self.is_deleted() {
            Err(DlrlException::already_deleted("the Selection is deleted"))
        } else {
            Ok(())
        }
    }

    /// The home that created the Selection; `None` once it is deleted.
    #[must_use]
    pub fn home(&self) -> Option<ObjectHome> {
        if self.lock().deleted {
            return None;
        }
        self.inner.home.upgrade().map(|inner| ObjectHome { inner })
    }

    /// `set_listener`: the listener of the Selection, `None` (NULL)
    /// discards it; returns the previous one.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted Selection.
    pub fn set_listener(
        &self,
        listener: Option<Arc<dyn SelectionListener>>,
    ) -> Result<Option<Arc<dyn SelectionListener>>, DlrlException> {
        let mut st = self.lock();
        if st.deleted {
            return Err(DlrlException::already_deleted("the Selection is deleted"));
        }
        Ok(core::mem::replace(&mut st.listener, listener))
    }

    /// Ends the Selection: no members, no listener. `delete_criterion`
    /// (from `delete_selection`) also deletes its `QueryCriterion`
    /// (§8.1.6.3.7); without it (a `create_selection` that failed) the
    /// criterion only belongs to no Selection.
    pub(crate) fn mark_deleted(&self, delete_criterion: bool) {
        let mut st = self.lock();
        st.deleted = true;
        st.members.clear();
        st.not_members.clear();
        st.listener = None;
        drop(st);
        if let SelectionCriterion::Query(q) = &self.inner.criterion {
            let mut qs = q.lock();
            qs.selection = None;
            qs.deleted |= delete_criterion;
        }
    }

    /// `refresh`: examines the objects of the home's class and derived
    /// classes in the Cache, and the members, against the state of the
    /// last examination; the listener hears the changes.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a Selection deleted by `delete_selection`
    /// (§8.1.6.2) and for a Selection of a deleted Cache;
    /// `PreconditionNotMet` when the home or its Cache is gone.
    pub fn refresh(&self) -> Result<(), DlrlException> {
        self.alive()?;
        let home = self
            .home()
            .ok_or_else(|| DlrlException::precondition("the home of the Selection is gone"))?;
        let cache = home
            .cache()
            .ok_or_else(|| DlrlException::precondition("the Cache of the home is gone"))?;
        cache.alive()?;
        // Not in the middle of an update round of another thread.
        let _round = cache.enter_round();
        let candidates = home.get_objects(&CacheBase::Cache(cache.clone()))?;
        for o in &candidates {
            self.examine(o, true);
        }
        let gone: Vec<ObjectRoot> = self
            .lock()
            .members
            .iter()
            .filter(|m| !candidates.contains(&m.object))
            .map(|m| m.object.clone())
            .collect();
        for o in &gone {
            self.leave(o);
        }
        Ok(())
    }

    fn membership(st: &SelectionState, o: &ObjectRoot, r: &ObjectRef) -> MembershipState {
        if st.members.iter().any(|m| m.object == *o) {
            MembershipState::AlreadyMember
        } else if st.not_members.contains(r) {
            MembershipState::NotMember
        } else {
            MembershipState::UndefinedMembership
        }
    }

    /// The state of a member to compare at its next examination: its
    /// content, with `concerns_contained` also that of its contained
    /// objects.
    fn fingerprint(&self, o: &ObjectRoot) -> Vec<ObjectData> {
        let mut out = alloc::vec![o.current_data()];
        if self.inner.concerns_contained {
            out.extend(o.contained_objects().iter().map(ObjectRoot::current_data));
        }
        out
    }

    /// Examines one object and, with `notify`, tells the listener what
    /// changed (`Cache::load` applies a round without listener).
    pub(crate) fn examine(&self, o: &ObjectRoot, notify: bool) {
        let r = o.reference();
        let deleted = o.read_state() == ObjectState::Deleted;
        let (membership, deleted_selection) = {
            let st = self.lock();
            (Self::membership(&st, o, &r), st.deleted)
        };
        if deleted_selection {
            return;
        }
        let pass = !deleted && self.inner.criterion.check(o, membership);
        let seen = pass.then(|| self.fingerprint(o));
        let (event, listener) = {
            let mut st = self.lock();
            let event = match (membership, seen) {
                (MembershipState::AlreadyMember, Some(seen)) => {
                    match st.members.iter_mut().find(|m| m.object == *o) {
                        Some(m) if m.seen != seen => {
                            m.seen = seen;
                            Some(Event::Modified)
                        }
                        _ => None,
                    }
                }
                (MembershipState::AlreadyMember, None) => {
                    st.members.retain(|m| m.object != *o);
                    if !deleted {
                        st.not_members.insert(r);
                    }
                    Some(Event::Out)
                }
                (_, Some(seen)) => {
                    st.not_members.remove(&r);
                    st.members.push(Member {
                        object: o.clone(),
                        seen,
                    });
                    Some(Event::In)
                }
                (_, None) => {
                    if deleted {
                        st.not_members.remove(&r);
                    } else {
                        st.not_members.insert(r);
                    }
                    None
                }
            };
            (event, st.listener.clone().filter(|_| notify))
        };
        if let (Some(event), Some(l)) = (event, listener) {
            match event {
                Event::In => l.on_object_in(o),
                Event::Out => l.on_object_out(o),
                Event::Modified => l.on_object_modified(o),
            }
        }
    }

    /// A member that is no longer in the Cache leaves.
    fn leave(&self, o: &ObjectRoot) {
        let listener = {
            let mut st = self.lock();
            let before = st.members.len();
            st.members.retain(|m| m.object != *o);
            if st.members.len() == before {
                return;
            }
            st.listener.clone()
        };
        if let Some(l) = listener {
            l.on_object_out(o);
        }
    }

    pub(crate) fn attach_query(&self) -> Result<(), DlrlException> {
        if let SelectionCriterion::Query(q) = &self.inner.criterion {
            let mut st = q.lock();
            if st.deleted {
                return Err(DlrlException::BadParameter {
                    message: "the QueryCriterion was deleted with its Selection".into(),
                });
            }
            if st.selection.as_ref().and_then(Weak::upgrade).is_some() {
                return Err(DlrlException::BadParameter {
                    message: "the QueryCriterion belongs to another Selection".into(),
                });
            }
            st.selection = Some(Arc::downgrade(&self.inner));
        }
        Ok(())
    }
}
