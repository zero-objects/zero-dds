// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a `MultiTopic` computes (DDS 1.4 §2.2.2.3.4, Annex B.2).
//!
//! A MultiTopic names topics in the `FROM` clause of its subscription
//! expression, joins them, keeps the fields of the `SELECT` list and filters
//! with `WHERE`. A DataReader on it "access[es] instances that are
//! 'constructed' at the DataReader side from the instances written by
//! multiple DataWriter entities".
//!
//! # Model
//!
//! A sample of a topic is a row: the top-level members of its type, by name
//! (a type is read through its `CompleteTypeObject`, so every topic and the
//! result type need one). The `NATURAL JOIN` is the SQL natural join: a tuple
//! of instances, one per topic, belongs to the result when every member name
//! that two of its topics share has the same value in both; the joined row
//! holds each name once, in the order of the `FROM` clause. The members that
//! take part in the join must have the same type (§2.2.2.3.4: "the type of
//! the fields used for the NATURAL JOIN operation must be the same"). Topics
//! that share no member name join as a cross product, which is the SQL
//! definition and the case of the example in Annex B.2.
//!
//! `SELECT` keeps and renames fields of the joined row (a dotted name goes
//! into a nested member); `WHERE` is evaluated on the joined row, in which
//! the aliases of the `SELECT` list are fields as well. The result is a
//! sample of the type named by the MultiTopic (`type_name`), whose members
//! are the fields of the `SELECT` list with the same names and types.
//!
//! # Instances
//!
//! The reader holds the latest sample of every instance of every topic. A
//! sample of one topic makes a result sample for each tuple it forms with
//! the live instances of the others; the result sample goes through the
//! ordinary reader of the result type, so the instance, view state, history
//! and the rest follow from its key. A result instance exists "as soon as
//! all the constituting Topic instances are in existence", and its
//! `instance_state` is "ALIVE if the instance_state of all the constituting
//! Topic instances is ALIVE", NOT_ALIVE_DISPOSED if one is disposed, and
//! NOT_ALIVE_NO_WRITERS otherwise (§2.2.2.3.4): when a constituting instance
//! changes state, every tuple it belongs to is told to the reader as a
//! dispose or unregister of its result instance. Its `view_state` "is NEW if
//! at least one of the constituting instances has view_state = NEW": a
//! constituting instance is NEW from its first sample, and again from the
//! first one after it was not alive, until a result sample has been made of
//! it; a result sample made of a NEW constituent makes the result instance
//! NEW for the application's next read or take.
//!
//! A member inherited from a base structure is a member of the row like
//! the others.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::Mutex;

use zerodds_sql_filter::{Expr, RowAccess, Select, TopicExpression, Value};
use zerodds_types::dynamic::{DynamicData, DynamicType, DynamicTypeMember, TypeKind};

use crate::error::{DdsError, Result};
use crate::instance_handle::InstanceHandle;
use crate::sample_info::InstanceStateKind;
use crate::serialized_type::{SerializedSample, SerializedTypeSupport, dynamic_field_value};
use crate::subscriber::DataReader;

/// Why a plan was refused: a static category for the error code, and the
/// detail that names the topic, field or member.
#[derive(Debug)]
pub(crate) struct PlanError {
    pub what: &'static str,
    pub detail: String,
}

impl PlanError {
    fn new(what: &'static str, detail: impl Into<String>) -> Self {
        Self {
            what,
            detail: detail.into(),
        }
    }
}

impl From<PlanError> for DdsError {
    fn from(e: PlanError) -> Self {
        DdsError::BadParameter { what: e.what }
    }
}

/// A topic of the `FROM` clause with the support that reads its samples.
#[derive(Clone)]
pub(crate) struct SourceTopic {
    pub name: String,
    pub support: Arc<SerializedTypeSupport>,
}

/// One field of the result.
#[derive(Debug, Clone)]
struct Output {
    /// The field of the joined row, dotted for a nested member.
    path: String,
    /// The name in the result.
    name: String,
}

/// What a MultiTopic computes, worked out once from its expression, the
/// topics it reads and the type it produces.
pub(crate) struct MultiTopicPlan {
    topics: Vec<SourceTopic>,
    /// The member names of each topic.
    members: Vec<Vec<String>>,
    /// The columns of the joined row, each name once.
    columns: Vec<String>,
    /// For each pair of topics `(a, b)` with `a < b`, the member names they
    /// share: the join columns of the pair.
    shared: BTreeMap<(usize, usize), Vec<String>>,
    outputs: Vec<Output>,
    /// The aliases of the `SELECT` list, which `WHERE` may name.
    aliases: Vec<Output>,
    condition: Option<Expr>,
    result: Arc<SerializedTypeSupport>,
}

fn aggregate_type(
    support: &SerializedTypeSupport,
    what: &str,
) -> core::result::Result<DynamicType, PlanError> {
    let type_ = support.get_type().map_err(|e| {
        PlanError::new(
            "multitopic: the type of a topic cannot be read",
            alloc::format!("{what}: {e}"),
        )
    })?;
    let type_ = type_.resolve_alias().map_err(|e| {
        PlanError::new(
            "multitopic: the type of a topic cannot be read",
            alloc::format!("{what}: {e:?}"),
        )
    })?;
    if type_.kind() != TypeKind::Structure {
        return Err(PlanError::new(
            "multitopic: the type of a topic is not a structure",
            alloc::format!("{what} has the type {}", type_.qualified_name()),
        ));
    }
    Ok(type_)
}

/// The deepest chain of base structures read.
const MAX_BASE_DEPTH: usize = 64;

/// The fields of a structure: those of its base structures first, then its
/// own (IDL 4.2 struct inheritance; Annex B.2 FIELDNAME "can refer to fields
/// at any depth in the data structure").
///
/// zerodds-lint: recursion-depth 64 (`MAX_BASE_DEPTH`, checked on entry)
fn all_members(type_: &DynamicType, depth: usize) -> Vec<DynamicTypeMember> {
    let mut out = Vec::new();
    if depth < MAX_BASE_DEPTH {
        if let Some(base) = type_.base_type().and_then(|b| b.resolve_alias().ok()) {
            out = all_members(&base, depth + 1);
        }
    }
    out.extend(type_.members());
    out
}

/// The field `name` of a structure, inherited ones included.
fn find_member(type_: &DynamicType, name: &str) -> Option<DynamicTypeMember> {
    all_members(type_, 0)
        .into_iter()
        .find(|member| member.name() == name)
}

/// The type at `path` (dotted member names) below `root`.
fn type_at(root: &DynamicType, path: &str) -> Option<DynamicType> {
    let mut current = root.resolve_alias().ok()?;
    for segment in path.split('.') {
        current = find_member(&current.resolve_alias().ok()?, segment)?
            .dynamic_type()
            .ok()?;
    }
    Some(current)
}

impl MultiTopicPlan {
    /// Works out the plan.
    ///
    /// # Errors
    /// [`PlanError`] when the topics, the expression and the result type do
    /// not fit together: a join member of two types, a field no topic has, a
    /// `WHERE` that reads an unknown field, a result type that lacks a field
    /// or holds it in another type or has a member nothing fills.
    pub(crate) fn new(
        expression: &TopicExpression,
        topics: Vec<SourceTopic>,
        result: Arc<SerializedTypeSupport>,
    ) -> core::result::Result<Self, PlanError> {
        // The joined row: each member name once, in the order of FROM.
        let mut types: Vec<DynamicType> = Vec::new();
        let mut members: Vec<Vec<String>> = Vec::new();
        let mut column_types: BTreeMap<String, DynamicType> = BTreeMap::new();
        let mut columns: Vec<String> = Vec::new();
        for topic in &topics {
            let type_ = aggregate_type(&topic.support, &topic.name)?;
            let mut names = Vec::new();
            for member in all_members(&type_, 0) {
                let member_type = member.dynamic_type().map_err(|e| {
                    PlanError::new(
                        "multitopic: the type of a topic cannot be read",
                        alloc::format!("{}.{}: {e:?}", topic.name, member.name()),
                    )
                })?;
                match column_types.get(member.name()) {
                    None => {
                        column_types.insert(member.name().to_string(), member_type);
                        columns.push(member.name().to_string());
                    }
                    Some(seen) if seen.equals_ignoring_annotations(&member_type) => {}
                    Some(_) => {
                        return Err(PlanError::new(
                            "multitopic: topics share a member name but not its type",
                            alloc::format!(
                                "the member '{}' of '{}' has another type than in an earlier topic of the FROM clause",
                                member.name(),
                                topic.name
                            ),
                        ));
                    }
                }
                names.push(member.name().to_string());
            }
            types.push(type_);
            members.push(names);
        }
        let mut shared: BTreeMap<(usize, usize), Vec<String>> = BTreeMap::new();
        for a in 0..members.len() {
            for b in (a + 1)..members.len() {
                let both: Vec<String> = members[a]
                    .iter()
                    .filter(|name| members[b].contains(name))
                    .cloned()
                    .collect();
                if !both.is_empty() {
                    shared.insert((a, b), both);
                }
            }
        }

        // The fields kept.
        let mut outputs: Vec<Output> = Vec::new();
        match &expression.select {
            Select::All => {
                for column in &columns {
                    outputs.push(Output {
                        path: column.clone(),
                        name: column.clone(),
                    });
                }
            }
            Select::Fields(fields) => {
                for field in fields {
                    let first = field.field.split('.').next().unwrap_or_default();
                    let Some(column_type) = column_types.get(first) else {
                        return Err(PlanError::new(
                            "multitopic: a field of the SELECT list is a member of no topic",
                            alloc::format!("'{}' is a member of none of the topics", field.field),
                        ));
                    };
                    if type_at(
                        column_type,
                        field.field.get(first.len() + 1..).unwrap_or(""),
                    )
                    .is_none()
                        && field.field.len() > first.len()
                    {
                        return Err(PlanError::new(
                            "multitopic: a field of the SELECT list is a member of no topic",
                            alloc::format!("'{}' has no such nested member", field.field),
                        ));
                    }
                    outputs.push(Output {
                        path: field.field.clone(),
                        name: field.name().to_string(),
                    });
                }
            }
        }
        let aliases: Vec<Output> = outputs
            .iter()
            .filter(|o| o.path != o.name)
            .cloned()
            .collect();

        // WHERE reads the joined row and the aliases.
        if let Some(condition) = &expression.condition {
            for field in condition.collect_fields() {
                let first = zerodds_sql_filter::split_path(&field)
                    .and_then(|steps| match steps.first() {
                        Some(zerodds_sql_filter::PathStep::Member(name)) => Some(name.clone()),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        PlanError::new(
                            "multitopic: the WHERE condition reads a field no topic has",
                            alloc::format!("'{field}' is not a field name"),
                        )
                    })?;
                let known = column_types.contains_key(first.as_str())
                    || aliases.iter().any(|a| a.name == first);
                if !known {
                    return Err(PlanError::new(
                        "multitopic: the WHERE condition reads a field no topic has",
                        alloc::format!(
                            "'{field}': no topic of the FROM clause has a member '{first}' and no field is called so in the SELECT list"
                        ),
                    ));
                }
            }
        }

        // The result type holds the fields.
        let result_type = aggregate_type(&result, "the result type")?;
        for output in &outputs {
            let Some(member) = find_member(&result_type, &output.name) else {
                return Err(PlanError::new(
                    "multitopic: the result type has no member for a field of the SELECT list",
                    alloc::format!(
                        "the result type {} has no member '{}'",
                        result_type.qualified_name(),
                        output.name
                    ),
                ));
            };
            let first = output.path.split('.').next().unwrap_or_default();
            let source_type = column_types
                .get(first)
                .and_then(|t| {
                    output
                        .path
                        .get(first.len() + 1..)
                        .filter(|rest| !rest.is_empty())
                        .map_or_else(|| Some(t.clone()), |rest| type_at(t, rest))
                })
                .ok_or_else(|| {
                    PlanError::new(
                        "multitopic: a field of the SELECT list is a member of no topic",
                        alloc::format!("'{}'", output.path),
                    )
                })?;
            let target_type = member.dynamic_type().map_err(|e| {
                PlanError::new(
                    "multitopic: the type of the result cannot be read",
                    alloc::format!("{}: {e:?}", output.name),
                )
            })?;
            if !source_type.equals_ignoring_annotations(&target_type) {
                return Err(PlanError::new(
                    "multitopic: a member of the result type has another type than its field",
                    alloc::format!(
                        "the field '{}' has the type {}, the member '{}' the type {}",
                        output.path,
                        source_type.qualified_name(),
                        output.name,
                        target_type.qualified_name()
                    ),
                ));
            }
        }
        for member in all_members(&result_type, 0) {
            let optional = member.descriptor().map(|d| d.is_optional).unwrap_or(false);
            if !optional && !outputs.iter().any(|o| o.name == member.name()) {
                return Err(PlanError::new(
                    "multitopic: a member of the result type has no field in the SELECT list",
                    alloc::format!("the member '{}' is not filled", member.name()),
                ));
            }
        }

        Ok(Self {
            topics,
            members,
            columns,
            shared,
            outputs,
            aliases,
            condition: expression.condition.clone(),
            result,
        })
    }

    /// The topics of the `FROM` clause, in order.
    pub(crate) fn topics(&self) -> &[SourceTopic] {
        &self.topics
    }
}

/// The value of `path` (dotted) below `data`.
fn value_at<'a>(data: &'a DynamicData, path: &str) -> Option<&'a DynamicData> {
    let mut current = data;
    for segment in path.split('.') {
        current = current.member(segment)?;
    }
    Some(current)
}

/// A row of the joined topics and the aliases, for `WHERE`.
struct JoinedRow<'a>(&'a DynamicData);

impl RowAccess for JoinedRow<'_> {
    fn get(&self, path: &str) -> Option<Value> {
        dynamic_field_value(self.0, None, path)
    }
}

/// What the engine remembers of one instance of a topic.
struct SourceInstance {
    /// The members of its latest sample.
    row: Option<Vec<(String, DynamicData)>>,
    state: InstanceStateKind,
    /// A result sample has been made of the instance since it was last NEW
    /// (DDS 1.4 §2.2.2.3.4: its view_state is NEW until then).
    contributed: bool,
}

/// The tuple of instances a result instance is made of, and what the reader
/// was told of it last.
struct Combo {
    key_hash: [u8; 16],
    key_holder: Vec<u8>,
    state: InstanceStateKind,
}

#[derive(Default)]
struct EngineState {
    /// Per topic, its instances by handle.
    instances: Vec<BTreeMap<InstanceHandle, SourceInstance>>,
    /// The tuples, by the handle of each topic's instance.
    combos: BTreeMap<Vec<InstanceHandle>, Combo>,
}

/// The expression parameters of a MultiTopic, current at each evaluation.
pub(crate) type ParameterValues = Arc<dyn Fn() -> Vec<Value> + Send + Sync>;

/// A DataReader of a MultiTopic: reads the topics through readers of its
/// own, joins, and hands the result samples to the reader of the result.
pub(crate) struct MultiTopicEngine {
    plan: Arc<MultiTopicPlan>,
    readers: Vec<DataReader<SerializedSample>>,
    state: Mutex<EngineState>,
    sink: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    /// The instances of the reader of the result, whose view_state a NEW
    /// constituent resets.
    result_instances: crate::instance_tracker::InstanceTracker,
    parameters: ParameterValues,
    notify: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>>,
    runtime: Option<Arc<crate::runtime::DcpsRuntime>>,
}

impl MultiTopicEngine {
    pub(crate) fn new(
        plan: Arc<MultiTopicPlan>,
        readers: Vec<DataReader<SerializedSample>>,
        sink: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
        result_instances: crate::instance_tracker::InstanceTracker,
        parameters: ParameterValues,
        runtime: Option<Arc<crate::runtime::DcpsRuntime>>,
    ) -> Self {
        let state = EngineState {
            instances: (0..readers.len()).map(|_| BTreeMap::new()).collect(),
            combos: BTreeMap::new(),
        };
        Self {
            plan,
            readers,
            state: Mutex::new(state),
            sink,
            result_instances,
            parameters,
            notify: std::sync::OnceLock::new(),
            runtime,
        }
    }

    /// The readers of the topics.
    pub(crate) fn source_readers(&self) -> &[DataReader<SerializedSample>] {
        &self.readers
    }

    /// What tells the reader of the result that data arrived.
    pub(crate) fn set_notify(&self, notify: Box<dyn Fn() + Send + Sync>) {
        let _ = self.notify.set(notify);
    }

    /// Deletes the readers of the topics (the reader of the result is
    /// deleted).
    pub(crate) fn shutdown(&self) {
        for reader in &self.readers {
            reader.inner.delete();
        }
    }

    /// [`Self::pump`], then tells the reader of the result about what it
    /// produced (DDS 1.4 §2.2.2.3.4: the readers "are alerted of data
    /// modifications … whenever modifications occur to the data associated
    /// with any of the topics").
    pub(crate) fn pump_and_notify(&self) {
        match self.pump() {
            Ok(0) => {}
            Ok(_) => {
                if let Some(notify) = self.notify.get() {
                    notify();
                }
            }
            Err(error) => self.report("multitopic.pump.failed", &alloc::format!("{error}")),
        }
    }

    fn report(&self, event: &'static str, detail: &str) {
        use zerodds_foundation::observability::{Component, Event, Level};
        if let Some(rt) = self.runtime.as_ref() {
            rt.config.observability.record(
                &Event::new(Level::Warn, Component::Dcps, event).with_attr("detail", detail),
            );
        }
    }

    /// Takes what the readers of the topics hold, joins it, and puts the
    /// result samples into the reader of the result. Returns how many
    /// samples and markers that made.
    ///
    /// # Errors
    /// A reader of a topic refuses its `take`.
    pub(crate) fn pump(&self) -> Result<usize> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "multitopic state poisoned",
            })?;
        let mut produced = Vec::new();
        for (index, reader) in self.readers.iter().enumerate() {
            for sample in reader.take_with_info()? {
                let handle = sample.info.instance_handle;
                let stamp = sample.info.source_timestamp;
                if sample.info.valid_data {
                    // NEW from the first sample, and again from the first
                    // one after the instance was not alive.
                    let contributed = state.instances[index]
                        .get(&handle)
                        .is_some_and(|i| i.state == InstanceStateKind::Alive && i.contributed);
                    let row = sample
                        .data
                        .data()
                        .members()
                        .map(<[(String, DynamicData)]>::to_vec)
                        .unwrap_or_default();
                    state.instances[index].insert(
                        handle,
                        SourceInstance {
                            row: Some(row),
                            state: InstanceStateKind::Alive,
                            contributed,
                        },
                    );
                    self.emit(&mut state, index, handle, stamp, &mut produced);
                } else {
                    let entry = state.instances[index]
                        .entry(handle)
                        .or_insert(SourceInstance {
                            row: None,
                            state: sample.info.instance_state,
                            contributed: false,
                        });
                    entry.state = sample.info.instance_state;
                    Self::retire(&mut state, index, handle, stamp, &mut produced);
                }
            }
        }
        let count = produced.len();
        if count > 0 {
            self.sink
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "multitopic result inbox poisoned",
                })?
                .extend(produced);
        }
        Ok(count)
    }

    /// The tuples the instance `handle` of topic `index` forms with the
    /// live instances of the other topics.
    fn tuples(
        &self,
        state: &EngineState,
        index: usize,
        handle: InstanceHandle,
    ) -> Vec<Vec<InstanceHandle>> {
        let mut out = Vec::new();
        let mut chosen: Vec<InstanceHandle> = Vec::new();
        self.extend_tuple(state, index, handle, &mut chosen, &mut out);
        out
    }

    /// zerodds-lint: recursion-depth = number of topics of the FROM clause
    fn extend_tuple(
        &self,
        state: &EngineState,
        fixed_index: usize,
        fixed_handle: InstanceHandle,
        chosen: &mut Vec<InstanceHandle>,
        out: &mut Vec<Vec<InstanceHandle>>,
    ) {
        let next = chosen.len();
        if next == self.readers.len() {
            out.push(chosen.clone());
            return;
        }
        let candidates: Vec<InstanceHandle> = if next == fixed_index {
            alloc::vec![fixed_handle]
        } else {
            state.instances[next]
                .iter()
                .filter(|(_, i)| i.row.is_some() && i.state == InstanceStateKind::Alive)
                .map(|(h, _)| *h)
                .collect()
        };
        for candidate in candidates {
            if self.agrees(state, chosen, next, candidate) {
                chosen.push(candidate);
                self.extend_tuple(state, fixed_index, fixed_handle, chosen, out);
                chosen.pop();
            }
        }
    }

    /// Whether the instance `candidate` of topic `next` has the same value
    /// as each earlier instance of the tuple in every member they share.
    fn agrees(
        &self,
        state: &EngineState,
        chosen: &[InstanceHandle],
        next: usize,
        candidate: InstanceHandle,
    ) -> bool {
        let Some(row) = state.instances[next]
            .get(&candidate)
            .and_then(|i| i.row.as_ref())
        else {
            return false;
        };
        for (earlier, earlier_handle) in chosen.iter().enumerate() {
            let Some(names) = self.plan.shared.get(&(earlier, next)) else {
                continue;
            };
            let Some(earlier_row) = state.instances[earlier]
                .get(earlier_handle)
                .and_then(|i| i.row.as_ref())
            else {
                return false;
            };
            for name in names {
                let a = earlier_row.iter().find(|(n, _)| n == name).map(|(_, v)| v);
                let b = row.iter().find(|(n, _)| n == name).map(|(_, v)| v);
                if a.is_none() || a != b {
                    return false;
                }
            }
        }
        true
    }

    /// The joined row of a tuple: each column once, in the order of the
    /// `FROM` clause.
    fn joined(&self, state: &EngineState, tuple: &[InstanceHandle]) -> Vec<(String, DynamicData)> {
        let mut row = Vec::new();
        for column in &self.plan.columns {
            for (index, handle) in tuple.iter().enumerate() {
                if !self.plan.members[index].contains(column) {
                    continue;
                }
                let value = state.instances[index]
                    .get(handle)
                    .and_then(|i| i.row.as_ref())
                    .and_then(|r| r.iter().find(|(n, _)| n == column))
                    .map(|(_, v)| v.clone());
                if let Some(value) = value {
                    row.push((column.clone(), value));
                    break;
                }
            }
        }
        row
    }

    /// A sample of the instance `handle` of topic `index`: a result sample
    /// for each tuple it forms and that `WHERE` lets through.
    fn emit(
        &self,
        state: &mut EngineState,
        index: usize,
        handle: InstanceHandle,
        stamp: crate::time::Time,
        produced: &mut Vec<crate::runtime::UserSample>,
    ) {
        let values = (self.parameters)();
        for tuple in self.tuples(state, index, handle) {
            let joined = self.joined(state, &tuple);
            let mut with_aliases = joined.clone();
            for alias in &self.plan.aliases {
                if let Some(value) = value_at(&DynamicData::Aggregate(joined.clone()), &alias.path)
                {
                    with_aliases.push((alias.name.clone(), value.clone()));
                }
            }
            if let Some(condition) = &self.plan.condition {
                let row = DynamicData::Aggregate(with_aliases);
                if !matches!(condition.evaluate(&JoinedRow(&row), &values), Ok(true)) {
                    continue;
                }
            }
            let whole = DynamicData::Aggregate(joined);
            let mut result = Vec::new();
            let mut complete = true;
            for output in &self.plan.outputs {
                match value_at(&whole, &output.path) {
                    Some(value) => result.push((output.name.clone(), value.clone())),
                    None => complete = false,
                }
            }
            if !complete {
                self.report(
                    "multitopic.result.refused",
                    "a field of the SELECT list has no value in the joined row",
                );
                continue;
            }
            let sample = match self
                .plan
                .result
                .from_dynamic_data(DynamicData::Aggregate(result))
            {
                Ok(sample) => sample,
                Err(error) => {
                    self.report("multitopic.result.refused", &error);
                    continue;
                }
            };
            let (key_hash, key_holder) = self
                .plan
                .result
                .key_material(&sample)
                .unwrap_or(([0u8; 16], Vec::new()));
            // §2.2.2.3.4: NEW if at least one constituent is.
            let constituent_new = tuple.iter().enumerate().any(|(i, h)| {
                state.instances[i]
                    .get(h)
                    .is_some_and(|instance| !instance.contributed)
            });
            if constituent_new {
                self.result_instances.mark_view_new(&key_hash);
            }
            for (i, h) in tuple.iter().enumerate() {
                if let Some(instance) = state.instances[i].get_mut(h) {
                    instance.contributed = true;
                }
            }
            produced.push(crate::runtime::UserSample::Alive {
                key_hash: sample.key_hash(),
                payload: crate::sample_bytes::SampleBytes::from_vec(sample.body().to_vec()),
                writer_guid: [0u8; 16],
                writer_strength: 0,
                // XCDR2, little-endian: the body `from_dynamic_data` made.
                representation: 1,
                big_endian: false,
                source_timestamp: Some(crate::time::time_to_he_timestamp(stamp)),
                reception_timestamp: None,
                source_sequence_number: -1,
                coherent_set_sn: None,
                lifespan_nanos: 0,
                related_sample_identity: None,
                group: None,
                crypto_content: false,
            });
            state.combos.insert(
                tuple,
                Combo {
                    key_hash,
                    key_holder,
                    state: InstanceStateKind::Alive,
                },
            );
        }
    }

    /// The instance `handle` of topic `index` changed its state: every
    /// tuple it belongs to takes the state §2.2.2.3.4 gives from its
    /// constituents, and the reader is told of a result instance that is no
    /// longer alive.
    fn retire(
        state: &mut EngineState,
        index: usize,
        handle: InstanceHandle,
        stamp: crate::time::Time,
        produced: &mut Vec<crate::runtime::UserSample>,
    ) {
        let EngineState { instances, combos } = state;
        for (tuple, combo) in combos.iter_mut() {
            if tuple.get(index) != Some(&handle) {
                continue;
            }
            let mut disposed = false;
            let mut no_writers = false;
            for (i, h) in tuple.iter().enumerate() {
                match instances[i].get(h).map(|inst| inst.state) {
                    Some(InstanceStateKind::NotAliveDisposed) => disposed = true,
                    Some(InstanceStateKind::NotAliveNoWriters) => no_writers = true,
                    _ => {}
                }
            }
            let new_state = if disposed {
                InstanceStateKind::NotAliveDisposed
            } else if no_writers {
                InstanceStateKind::NotAliveNoWriters
            } else {
                InstanceStateKind::Alive
            };
            if new_state == combo.state {
                continue;
            }
            combo.state = new_state;
            let kind = match new_state {
                InstanceStateKind::NotAliveDisposed => {
                    zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed
                }
                InstanceStateKind::NotAliveNoWriters => {
                    zerodds_rtps::history_cache::ChangeKind::NotAliveUnregistered
                }
                InstanceStateKind::Alive => continue,
            };
            produced.push(crate::runtime::UserSample::Lifecycle {
                key_hash: combo.key_hash,
                key_holder: (!combo.key_holder.is_empty()).then(|| combo.key_holder.clone()),
                kind,
                writer_guid: [0u8; 16],
                source_timestamp: Some(crate::time::time_to_he_timestamp(stamp)),
                group: None,
                coherent_set_sn: None,
                source_sequence_number: -1,
            });
        }
    }
}
