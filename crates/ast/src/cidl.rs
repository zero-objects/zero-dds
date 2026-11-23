// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The CIDL model: compositions and the PSDL declarations a CIDL file
//! holds. CORBA 3.3 Part 3 §8; Persistent State Service 2.0 §3.2.
//!
//! Plain data, names as written in IDL (`M::Home`), so that a runtime
//! that never sees an IDL file — a container host reading a deployment
//! plan — builds the same values the compiler lowers a `.cdl` file to.
//! `zerodds-idl-lower` produces it from a `.cdl` file
//! (`lower_cidl`); `zerodds-corba-ccm` re-exports it as its `cidl`
//! module.
//!
//! [`Composition::validate`] holds the rules that read names only:
//! the life cycle category constraints of Tables 8.3 to 8.5, the
//! segments of §8.11 and §8.13, which features and operations a
//! delegation may name (§8.14 to §8.18). The rules that read types —
//! Table 8.6 and the signature compatibility of §8.15 — need the whole
//! model and are the compiler's.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The life cycle category of a composition. §8.5, production (5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CompositionCategory {
    /// `session`: transient state, identity not persistent.
    #[default]
    Session,
    /// `service`: no state, no identity.
    Service,
    /// `process`: persistent state, no client-visible key.
    Process,
    /// `entity`: persistent state and a primary key.
    Entity,
}

impl CompositionCategory {
    /// Every category of production (5).
    pub const ALL: [Self; 4] = [Self::Entity, Self::Process, Self::Service, Self::Session];

    /// The keyword.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::Process => "process",
            Self::Service => "service",
            Self::Session => "session",
        }
    }

    /// The category a keyword names.
    #[must_use]
    pub fn from_keyword(keyword: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == keyword)
    }
}

/// `composition <category> <name> { … };` — §8.5.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Composition {
    /// The composition's scoped name, `M::ToonImpl`.
    pub name: String,
    /// The life cycle category.
    pub category: CompositionCategory,
    /// The mandatory home executor definition. §8.6.
    pub home_executor: HomeExecutorDef,
    /// The optional proxy home definition. §8.18.
    pub proxy_home: Option<ProxyHomeDef>,
}

/// `home executor <name> { … };` — §8.6, production (8).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HomeExecutorDef {
    /// The name of the home executor, the basis of the generated
    /// skeleton's name.
    pub name: String,
    /// `implements <home>`: the scoped name of the component home. §8.7.
    pub implements: String,
    /// `bindsTo <abstract storage home>`: the storage home the
    /// components are stored on. §8.8.
    pub binds_to: Option<String>,
    /// `storedOn <abstract storage home>`: the home executor's own
    /// state. §8.9.
    pub stored_on: Option<String>,
    /// `manages <executor> …`. §8.10.
    pub executor: ExecutorDef,
    /// `delegatesTo abstract storagehome (…)`. §8.15.
    pub storage_home_delegations: Vec<Delegation>,
    /// `delegatesTo executor (…)`. §8.16.
    pub executor_delegations: Vec<Delegation>,
    /// `abstract (…)`: home operations left unimplemented. §8.17.
    pub abstract_operations: Vec<String>,
}

/// `manages <name> [{ … }]` — §8.10.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutorDef {
    /// The component executor's name.
    pub name: String,
    /// The segments; none for a monolithic executor. §8.11.
    pub segments: Vec<SegmentDef>,
    /// `delegatesTo storage (…)`. §8.14.
    pub feature_delegations: Vec<FeatureDelegation>,
}

/// The segment identifier of the component segment. §8.11: "The segment
/// identifier value of the component segment is always zero."
pub const COMPONENT_SEGMENT: i16 = 0;

/// `segment <name> { … }` — §8.11 to §8.13.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SegmentDef {
    /// The segment's name, in the scope of the executor.
    pub name: String,
    /// Its segment identifier. Never [`COMPONENT_SEGMENT`]: that one is
    /// the implicit component segment's.
    pub id: i16,
    /// `storedOn <abstract storage home>`. §8.12.
    pub stored_on: Option<String>,
    /// `provides facet a, b`: the facets this segment provides. §8.13.
    pub facets: Vec<String>,
}

/// `<feature> : <storage member>` — §8.14, production (22).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FeatureDelegation {
    /// A stateful feature of the component: an attribute, a receptacle
    /// or an event source.
    pub feature: String,
    /// A state member of the bound abstract storage type.
    pub storage_member: String,
}

/// `<operation> [: <target>]` — production (28).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Delegation {
    /// The home operation.
    pub operation: String,
    /// The operation it delegates to; `None` is the operation of the
    /// same name (§8.15, §8.16).
    pub target: Option<String>,
}

impl Delegation {
    /// The operation delegated to.
    #[must_use]
    pub fn target_name(&self) -> &str {
        self.target.as_deref().unwrap_or(&self.operation)
    }
}

/// `proxy home <name> { … };` — §8.18.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProxyHomeDef {
    /// The proxy home executor's name.
    pub name: String,
    /// `delegatesTo home (…)`: operations delegated to the actual home.
    pub home_delegations: Vec<Delegation>,
    /// `abstract (…)`.
    pub abstract_operations: Vec<String>,
}

/// What [`Composition::validate`] reads of the component home a
/// composition implements and of the component the home manages.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HomeFacts {
    /// Whether the home declares a primary key (`primarykey`).
    pub primary_key: bool,
    /// The home's explicit finder operations, its base homes' included.
    pub explicit_finders: Vec<String>,
    /// The factory operations: the explicit ones, its base homes'
    /// included, and the implicit `create`. §8.16, §9.2.8.3.2.
    pub factories: Vec<String>,
    /// Every operation of the home equivalent interface: explicit,
    /// orthodox and inherited ones.
    pub operations: Vec<String>,
    /// The facets of the managed component, its base components'
    /// included.
    pub facets: Vec<String>,
    /// The stateful features of the managed component: attributes,
    /// receptacles and event sources. §8.14.
    pub stateful_features: Vec<String>,
}

/// What [`Composition::validate`] reads of the abstract storage home a
/// home executor binds to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StorageHomeFacts {
    /// Its operations: local operations, factories and the two finders
    /// each key declares (PSS 2.0 §3.2.5.7), inherited ones included.
    pub operations: Vec<String>,
    /// The state members of its abstract storage type, inherited ones
    /// included.
    pub state_members: Vec<String>,
}

/// The part of a composition a [`CompositionError`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Subject {
    /// The composition as a whole (its category).
    Composition,
    /// The `bindsTo` declaration.
    BindsTo,
    /// The segment at this index.
    Segment(usize),
    /// The facet at index `.1` of the segment at index `.0`.
    SegmentFacet(usize, usize),
    /// The feature delegation at this index.
    FeatureDelegation(usize),
    /// The storage home delegation at this index.
    StorageHomeDelegation(usize),
    /// The executor delegation at this index.
    ExecutorDelegation(usize),
    /// The home executor's abstract operation at this index.
    AbstractOperation(usize),
    /// The proxy home's delegation at this index.
    ProxyDelegation(usize),
    /// The proxy home's abstract operation at this index.
    ProxyAbstractOperation(usize),
}

/// A rule of §8 a composition breaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionError {
    /// What the rule is about.
    pub subject: Subject,
    /// What is wrong, in a sentence.
    pub message: String,
    /// The clause that states the rule.
    pub clause: &'static str,
}

impl core::fmt::Display for CompositionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} ({})", self.message, self.clause)
    }
}

impl Composition {
    /// Whether the executor is segmented. §8.11.
    #[must_use]
    pub fn is_segmented(&self) -> bool {
        !self.home_executor.executor.segments.is_empty()
    }

    /// The segment identifier of the segment providing `facet`: the
    /// segment that declares it, else the component segment. §8.13:
    /// "Facets that are not explicitly declared in a segment definition
    /// are provided by the component segment."
    #[must_use]
    pub fn segment_of_facet(&self, facet: &str) -> i16 {
        self.home_executor
            .executor
            .segments
            .iter()
            .find(|segment| segment.facets.iter().any(|f| f == facet))
            .map_or(COMPONENT_SEGMENT, |segment| segment.id)
    }

    /// Every rule of §8 that reads names, against the home the
    /// composition implements and, when it binds to one, the abstract
    /// storage home. An empty result is a valid composition.
    ///
    /// `storage` is `None` when the composition binds to nothing, or
    /// when the caller knows no more of the storage home than its name;
    /// the rules about its operations and state members are then not
    /// asked.
    #[must_use]
    pub fn validate(
        &self,
        home: &HomeFacts,
        storage: Option<&StorageHomeFacts>,
    ) -> Vec<CompositionError> {
        let mut errors = Vec::new();
        self.category_constraints(home, &mut errors);
        self.segment_rules(home, &mut errors);
        self.feature_delegation_rules(home, storage, &mut errors);
        self.delegation_rules(home, storage, &mut errors);
        errors
    }

    /// Tables 8.3, 8.4 and 8.5.
    fn category_constraints(&self, home: &HomeFacts, errors: &mut Vec<CompositionError>) {
        let category = self.category.as_str();
        let he = &self.home_executor;
        match self.category {
            CompositionCategory::Service | CompositionCategory::Session => {
                const TABLE: &str = "CORBA 3.3 Part 3 §8.5.1, Table 8.3";
                if he.binds_to.is_some() {
                    errors.push(error(
                        Subject::BindsTo,
                        format!(
                            "a {category} composition binds its home executor to no abstract \
                             storage home"
                        ),
                        TABLE,
                    ));
                }
                if home.primary_key {
                    errors.push(error(
                        Subject::Composition,
                        format!(
                            "the home `{}` declares a primary key, which a {category} \
                             composition's home does not",
                            he.implements
                        ),
                        TABLE,
                    ));
                }
                for finder in &home.explicit_finders {
                    errors.push(error(
                        Subject::Composition,
                        format!(
                            "the home `{}` declares the finder `{finder}`, and a {category} \
                             composition's home declares no explicit finder",
                            he.implements
                        ),
                        TABLE,
                    ));
                }
                for index in 0..he.executor.segments.len() {
                    errors.push(error(
                        Subject::Segment(index),
                        format!("a {category} composition's executor is not segmented"),
                        TABLE,
                    ));
                }
            }
            CompositionCategory::Process => {
                if home.primary_key {
                    errors.push(error(
                        Subject::Composition,
                        format!(
                            "the home `{}` declares a primary key, which a process \
                             composition's home does not",
                            he.implements
                        ),
                        "CORBA 3.3 Part 3 §8.5.1, Table 8.4",
                    ));
                }
            }
            CompositionCategory::Entity => {
                if !home.primary_key {
                    errors.push(error(
                        Subject::Composition,
                        format!(
                            "the home `{}` declares no primary key, which an entity \
                             composition's home must",
                            he.implements
                        ),
                        "CORBA 3.3 Part 3 §8.5.1, Table 8.5",
                    ));
                }
            }
        }
    }

    /// §8.11 and §8.13.
    fn segment_rules(&self, home: &HomeFacts, errors: &mut Vec<CompositionError>) {
        let segments = &self.home_executor.executor.segments;
        let mut facets_seen: Vec<(&str, usize)> = Vec::new();
        for (index, segment) in segments.iter().enumerate() {
            if segments[..index].iter().any(|s| s.name == segment.name) {
                errors.push(error(
                    Subject::Segment(index),
                    format!(
                        "the segment `{}` is defined twice in one executor",
                        segment.name
                    ),
                    "CORBA 3.3 Part 3 §8.11",
                ));
            }
            if segment.id == COMPONENT_SEGMENT
                || segments[..index].iter().any(|s| s.id == segment.id)
            {
                errors.push(error(
                    Subject::Segment(index),
                    format!(
                        "the segment `{}` has the identifier {}, which is the component \
                         segment's or another segment's",
                        segment.name, segment.id
                    ),
                    "CORBA 3.3 Part 3 §8.11",
                ));
            }
            if segment.facets.is_empty() {
                errors.push(error(
                    Subject::Segment(index),
                    format!("the segment `{}` provides no facet", segment.name),
                    "CORBA 3.3 Part 3 §9.2.9.1",
                ));
            }
            for (facet_index, facet) in segment.facets.iter().enumerate() {
                if !home.facets.iter().any(|f| f == facet) {
                    errors.push(error(
                        Subject::SegmentFacet(index, facet_index),
                        format!("the component managed by the home has no facet `{facet}`"),
                        "CORBA 3.3 Part 3 §8.13",
                    ));
                }
                if let Some((_, first)) = facets_seen.iter().find(|(f, _)| f == facet) {
                    let first = &segments[*first].name;
                    errors.push(error(
                        Subject::SegmentFacet(index, facet_index),
                        format!(
                            "the facet `{facet}` is already declared in the segment `{first}`; \
                             a facet name appears in a single segment definition"
                        ),
                        "CORBA 3.3 Part 3 §8.13",
                    ));
                } else {
                    facets_seen.push((facet, index));
                }
            }
        }
    }

    /// §8.14, the rules that read names.
    fn feature_delegation_rules(
        &self,
        home: &HomeFacts,
        storage: Option<&StorageHomeFacts>,
        errors: &mut Vec<CompositionError>,
    ) {
        const CLAUSE: &str = "CORBA 3.3 Part 3 §8.14";
        let delegations = &self.home_executor.executor.feature_delegations;
        for (index, delegation) in delegations.iter().enumerate() {
            if self.home_executor.binds_to.is_none() {
                errors.push(error(
                    Subject::FeatureDelegation(index),
                    format!(
                        "the feature `{}` is delegated to storage, and the home executor binds \
                         to no abstract storage home",
                        delegation.feature
                    ),
                    CLAUSE,
                ));
            }
            if !home.stateful_features.contains(&delegation.feature) {
                errors.push(error(
                    Subject::FeatureDelegation(index),
                    format!(
                        "the component has no stateful feature `{}`: an attribute, a \
                         receptacle or an event source",
                        delegation.feature
                    ),
                    CLAUSE,
                ));
            }
            if let Some(storage) = storage
                && !storage.state_members.contains(&delegation.storage_member)
            {
                errors.push(error(
                    Subject::FeatureDelegation(index),
                    format!(
                        "the abstract storage type of `{}` has no state member `{}`",
                        self.home_executor.binds_to.as_deref().unwrap_or_default(),
                        delegation.storage_member
                    ),
                    CLAUSE,
                ));
            }
            if delegations[..index]
                .iter()
                .any(|d| d.feature == delegation.feature)
            {
                errors.push(error(
                    Subject::FeatureDelegation(index),
                    format!(
                        "the feature `{}` is delegated twice; its state is stored in one \
                         member",
                        delegation.feature
                    ),
                    CLAUSE,
                ));
            }
        }
    }

    /// §8.15 to §8.18, the rules that read names.
    fn delegation_rules(
        &self,
        home: &HomeFacts,
        storage: Option<&StorageHomeFacts>,
        errors: &mut Vec<CompositionError>,
    ) {
        let he = &self.home_executor;
        let is_home_op = |name: &str| home.operations.iter().any(|o| o == name);

        // §8.15.
        const STORAGE: &str = "CORBA 3.3 Part 3 §8.15";
        for (index, delegation) in he.storage_home_delegations.iter().enumerate() {
            let subject = Subject::StorageHomeDelegation(index);
            if he.binds_to.is_none() {
                errors.push(error(
                    subject,
                    format!(
                        "`{}` is delegated to the abstract storage home, and the home executor \
                         binds to none",
                        delegation.operation
                    ),
                    STORAGE,
                ));
            }
            if !is_home_op(&delegation.operation) {
                errors.push(not_a_home_operation(
                    subject,
                    &delegation.operation,
                    &he.implements,
                    STORAGE,
                ));
            }
            if let Some(storage) = storage
                && !storage
                    .operations
                    .iter()
                    .any(|o| o == delegation.target_name())
            {
                errors.push(error(
                    subject,
                    format!(
                        "the abstract storage home `{}` has no operation `{}`",
                        he.binds_to.as_deref().unwrap_or_default(),
                        delegation.target_name()
                    ),
                    STORAGE,
                ));
            }
            let earlier = &he.storage_home_delegations[..index];
            if earlier.iter().any(|d| d.operation == delegation.operation) {
                errors.push(error(
                    subject,
                    format!(
                        "`{}` delegates to more than one operation of the abstract storage home",
                        delegation.operation
                    ),
                    STORAGE,
                ));
            }
            if earlier
                .iter()
                .any(|d| d.target_name() == delegation.target_name())
            {
                errors.push(error(
                    subject,
                    format!(
                        "the storage home operation `{}` is the target of more than one \
                         delegation",
                        delegation.target_name()
                    ),
                    STORAGE,
                ));
            }
        }

        // §8.16.
        const EXECUTOR: &str = "CORBA 3.3 Part 3 §8.16";
        for (index, delegation) in he.executor_delegations.iter().enumerate() {
            let subject = Subject::ExecutorDelegation(index);
            if !home.factories.contains(&delegation.operation) {
                errors.push(error(
                    subject,
                    format!(
                        "`{}` is not a factory operation of `{}`; only factory operations, the \
                         explicit ones and the implicit `create`, are delegated to the executor",
                        delegation.operation, he.implements
                    ),
                    EXECUTOR,
                ));
            }
            if he.executor_delegations[..index]
                .iter()
                .any(|d| d.operation == delegation.operation)
            {
                errors.push(error(
                    subject,
                    format!(
                        "`{}` is delegated to the executor twice",
                        delegation.operation
                    ),
                    EXECUTOR,
                ));
            }
            if he
                .storage_home_delegations
                .iter()
                .any(|d| d.operation == delegation.operation)
            {
                errors.push(error(
                    subject,
                    format!(
                        "`{}` is delegated both to the abstract storage home and to the \
                         executor",
                        delegation.operation
                    ),
                    EXECUTOR,
                ));
            }
        }

        // §8.17.
        for (index, operation) in he.abstract_operations.iter().enumerate() {
            if !is_home_op(operation) {
                errors.push(not_a_home_operation(
                    Subject::AbstractOperation(index),
                    operation,
                    &he.implements,
                    "CORBA 3.3 Part 3 §8.17",
                ));
            }
        }

        // §8.18 and §9.2.10.
        if let Some(proxy) = &self.proxy_home {
            const PROXY: &str = "CORBA 3.3 Part 3 §8.18, §9.2.10";
            for (index, delegation) in proxy.home_delegations.iter().enumerate() {
                let subject = Subject::ProxyDelegation(index);
                for name in [delegation.operation.as_str(), delegation.target_name()] {
                    if !is_home_op(name) {
                        errors.push(not_a_home_operation(subject, name, &he.implements, PROXY));
                    }
                }
                if proxy.home_delegations[..index]
                    .iter()
                    .any(|d| d.operation == delegation.operation)
                {
                    errors.push(error(
                        subject,
                        format!("`{}` is delegated to the home twice", delegation.operation),
                        PROXY,
                    ));
                }
            }
            for (index, operation) in proxy.abstract_operations.iter().enumerate() {
                if !is_home_op(operation) {
                    errors.push(not_a_home_operation(
                        Subject::ProxyAbstractOperation(index),
                        operation,
                        &he.implements,
                        PROXY,
                    ));
                }
                if proxy
                    .home_delegations
                    .iter()
                    .any(|d| d.operation == *operation)
                {
                    errors.push(error(
                        Subject::ProxyAbstractOperation(index),
                        format!("`{operation}` is both delegated to the home and left abstract"),
                        PROXY,
                    ));
                }
            }
        }
    }
}

fn error(subject: Subject, message: String, clause: &'static str) -> CompositionError {
    CompositionError {
        subject,
        message,
        clause,
    }
}

fn not_a_home_operation(
    subject: Subject,
    operation: &str,
    home: &str,
    clause: &'static str,
) -> CompositionError {
    error(
        subject,
        format!("the home `{home}` has no operation `{operation}`"),
        clause,
    )
}

/// Which PSDL declaration a [`StorageDecl`] is. PSS 2.0 §3.2.5, §3.2.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageKind {
    /// `abstract storagetype`.
    AbstractStorageType,
    /// `abstract storagehome`.
    AbstractStorageHome,
    /// `storagetype`.
    StorageType,
    /// `storagehome`.
    StorageHome,
}

impl StorageKind {
    /// The declaration's keywords.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AbstractStorageType => "abstract storagetype",
            Self::AbstractStorageHome => "abstract storagehome",
            Self::StorageType => "storagetype",
            Self::StorageHome => "storagehome",
        }
    }

    /// Whether this is one of the two storage home kinds.
    #[must_use]
    pub const fn is_home(self) -> bool {
        matches!(self, Self::AbstractStorageHome | Self::StorageHome)
    }
}

/// A PSDL declaration of a CIDL file, as plain data. PSS 2.0 §3.2.5,
/// §3.2.6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageDecl {
    /// The scoped name.
    pub name: String,
    /// Which declaration.
    pub kind: StorageKind,
    /// `of <type>` of a storage home.
    pub of: Option<String>,
    /// The bases (`: A, B`).
    pub bases: Vec<String>,
    /// `implements A, B` of a storage type or storage home.
    pub implements: Vec<String>,
    /// The state members it declares itself.
    pub state_members: Vec<StateMember>,
    /// Its local operations. PSS 2.0 §3.2.5.4, §3.2.6.1.
    pub operations: Vec<LocalOperation>,
    /// Its keys. PSS 2.0 §3.2.5.7.
    pub keys: Vec<KeyDecl>,
    /// Its factories. PSS 2.0 §3.2.5.8.
    pub factories: Vec<KeyDecl>,
    /// `primary key <key>`, or `primary key ref` as `"ref"`. §3.2.6.5.
    pub primary_key: Option<String>,
    /// `ref(a, b)` of a storage type. §3.2.6.3.
    pub ref_representation: Vec<String>,
    /// `stores m as T [scope H]` of a storage type. §3.2.6.2.
    pub stores: Vec<StoreDirective>,
}

/// `[readonly] state <type> <name>` — PSS 2.0 §3.2.5.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMember {
    /// The member's name.
    pub name: String,
    /// Its type, as written.
    pub type_: String,
    /// Whether it is `readonly`.
    pub readonly: bool,
}

/// A local operation of a PSDL declaration — PSS 2.0 §3.2.3 (23),
/// (24).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalOperation {
    /// The operation's name.
    pub name: String,
    /// Whether it is declared `const`, an operation of a storage type
    /// that "does not update any state member" (§3.2.5.4).
    pub is_const: bool,
}

/// A key or a factory: a name and the state members it lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyDecl {
    /// The key's or factory's name.
    pub name: String,
    /// The state members, in order.
    pub members: Vec<String>,
}

/// `stores <member> as <type> [scope <home>]` — PSS 2.0 §3.2.6.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreDirective {
    /// The state member.
    pub member: String,
    /// The concrete state type, as written.
    pub as_: String,
    /// The storage home of `scope`.
    pub scope: Option<String>,
}

/// Everything a CIDL file declared beyond IDL.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CidlModel {
    /// The PSDL declarations, in source order.
    pub storage: Vec<StorageDecl>,
    /// The compositions, in source order.
    pub compositions: Vec<Composition>,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    fn composition(category: CompositionCategory) -> Composition {
        Composition {
            name: "M::Impl".into(),
            category,
            home_executor: HomeExecutorDef {
                name: "HomeImpl".into(),
                implements: "M::H".into(),
                executor: ExecutorDef {
                    name: "Exec".into(),
                    ..ExecutorDef::default()
                },
                ..HomeExecutorDef::default()
            },
            proxy_home: None,
        }
    }

    fn keyless() -> HomeFacts {
        HomeFacts {
            factories: vec!["create".into()],
            operations: vec!["create".into()],
            facets: vec!["a".into(), "b".into()],
            ..HomeFacts::default()
        }
    }

    fn clauses(errors: &[CompositionError]) -> Vec<&'static str> {
        errors.iter().map(|e| e.clause).collect()
    }

    #[test]
    fn corba_p3_8_5_1_session_and_service_forbid_binding_primary_key_finders_and_segments() {
        for category in [CompositionCategory::Session, CompositionCategory::Service] {
            let plain = composition(category);
            assert!(plain.validate(&keyless(), None).is_empty(), "{category:?}");

            let mut bound = composition(category);
            bound.home_executor.binds_to = Some("M::SH".into());
            let errors = bound.validate(&keyless(), None);
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].subject, Subject::BindsTo);

            let keyed = HomeFacts {
                primary_key: true,
                ..keyless()
            };
            assert_eq!(plain.validate(&keyed, None).len(), 1);

            let finders = HomeFacts {
                explicit_finders: vec!["find_it".into()],
                ..keyless()
            };
            let errors = plain.validate(&finders, None);
            assert_eq!(errors.len(), 1);
            assert!(errors[0].message.contains("find_it"));

            let mut segmented = composition(category);
            segmented.home_executor.executor.segments = vec![SegmentDef {
                name: "S".into(),
                id: 1,
                stored_on: None,
                facets: vec!["a".into()],
            }];
            let errors = segmented.validate(&keyless(), None);
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].subject, Subject::Segment(0));
            assert_eq!(clauses(&errors), vec!["CORBA 3.3 Part 3 §8.5.1, Table 8.3"]);
        }
    }

    #[test]
    fn corba_p3_8_5_1_process_forbids_a_primary_key() {
        let process = composition(CompositionCategory::Process);
        let keyed = HomeFacts {
            primary_key: true,
            ..keyless()
        };
        assert_eq!(
            clauses(&process.validate(&keyed, None)),
            vec!["CORBA 3.3 Part 3 §8.5.1, Table 8.4"]
        );
        // Everything Table 8.3 forbids is allowed for process.
        let mut bound = composition(CompositionCategory::Process);
        bound.home_executor.binds_to = Some("M::SH".into());
        bound.home_executor.executor.segments = vec![SegmentDef {
            name: "S".into(),
            id: 1,
            stored_on: Some("M::SH".into()),
            facets: vec!["a".into()],
        }];
        let finders = HomeFacts {
            explicit_finders: vec!["find_it".into()],
            ..keyless()
        };
        assert!(bound.validate(&finders, None).is_empty());
    }

    #[test]
    fn corba_p3_8_5_1_entity_requires_a_primary_key() {
        let entity = composition(CompositionCategory::Entity);
        assert_eq!(
            clauses(&entity.validate(&keyless(), None)),
            vec!["CORBA 3.3 Part 3 §8.5.1, Table 8.5"]
        );
        let keyed = HomeFacts {
            primary_key: true,
            ..keyless()
        };
        assert!(entity.validate(&keyed, None).is_empty());
    }

    #[test]
    fn corba_p3_8_13_a_facet_belongs_to_one_segment_and_the_rest_to_the_component_segment() {
        let mut c = composition(CompositionCategory::Process);
        c.home_executor.executor.segments = vec![
            SegmentDef {
                name: "S1".into(),
                id: 1,
                stored_on: None,
                facets: vec!["a".into()],
            },
            SegmentDef {
                name: "S2".into(),
                id: 2,
                stored_on: None,
                facets: vec!["a".into(), "nope".into()],
            },
        ];
        let errors = c.validate(&keyless(), None);
        let subjects: Vec<Subject> = errors.iter().map(|e| e.subject).collect();
        assert_eq!(
            subjects,
            vec![Subject::SegmentFacet(1, 0), Subject::SegmentFacet(1, 1)]
        );
        assert_eq!(c.segment_of_facet("a"), 1);
        assert_eq!(c.segment_of_facet("b"), COMPONENT_SEGMENT);
    }

    #[test]
    fn corba_p3_8_11_segment_names_and_ids_are_distinct_and_not_zero() {
        let mut c = composition(CompositionCategory::Process);
        let segment = |name: &str, id: i16, facet: &str| SegmentDef {
            name: name.to_string(),
            id,
            stored_on: None,
            facets: vec![facet.to_string()],
        };
        c.home_executor.executor.segments = vec![
            segment("S", 1, "a"),
            segment("S", 1, "b"),
            segment("T", 0, "x"),
        ];
        let subjects: Vec<Subject> = c
            .validate(&keyless(), None)
            .iter()
            .map(|e| e.subject)
            .collect();
        assert_eq!(
            subjects,
            vec![
                Subject::Segment(1),
                Subject::Segment(1),
                Subject::Segment(2),
                Subject::SegmentFacet(2, 0)
            ]
        );
    }

    #[test]
    fn corba_p3_8_14_feature_delegation_names_a_bound_stateful_feature_and_a_member() {
        let home = HomeFacts {
            stateful_features: vec!["balance".into()],
            ..keyless()
        };
        let storage = StorageHomeFacts {
            operations: vec![],
            state_members: vec!["amount".into()],
        };
        let mut c = composition(CompositionCategory::Process);
        c.home_executor.executor.feature_delegations = vec![FeatureDelegation {
            feature: "balance".into(),
            storage_member: "amount".into(),
        }];
        // No bindsTo.
        assert_eq!(c.validate(&home, Some(&storage)).len(), 1);
        c.home_executor.binds_to = Some("M::SH".into());
        assert!(c.validate(&home, Some(&storage)).is_empty());
        c.home_executor
            .executor
            .feature_delegations
            .push(FeatureDelegation {
                feature: "owner".into(),
                storage_member: "who".into(),
            });
        c.home_executor
            .executor
            .feature_delegations
            .push(FeatureDelegation {
                feature: "balance".into(),
                storage_member: "amount".into(),
            });
        let subjects: Vec<Subject> = c
            .validate(&home, Some(&storage))
            .iter()
            .map(|e| e.subject)
            .collect();
        assert_eq!(
            subjects,
            vec![
                Subject::FeatureDelegation(1),
                Subject::FeatureDelegation(1),
                Subject::FeatureDelegation(2)
            ]
        );
    }

    #[test]
    fn corba_p3_8_15_storage_home_delegation_targets_one_operation_each_way() {
        let home = HomeFacts {
            operations: vec!["create".into(), "op".into(), "other".into()],
            ..keyless()
        };
        let storage = StorageHomeFacts {
            operations: vec!["op".into(), "do_it".into()],
            state_members: vec![],
        };
        let mut c = composition(CompositionCategory::Process);
        c.home_executor.binds_to = Some("M::SH".into());
        c.home_executor.storage_home_delegations = vec![
            Delegation {
                operation: "op".into(),
                target: None,
            },
            Delegation {
                operation: "other".into(),
                target: Some("do_it".into()),
            },
        ];
        assert!(c.validate(&home, Some(&storage)).is_empty());
        c.home_executor.storage_home_delegations.push(Delegation {
            operation: "op".into(),
            target: Some("do_it".into()),
        });
        c.home_executor.storage_home_delegations.push(Delegation {
            operation: "missing".into(),
            target: Some("absent".into()),
        });
        let subjects: Vec<Subject> = c
            .validate(&home, Some(&storage))
            .iter()
            .map(|e| e.subject)
            .collect();
        assert_eq!(
            subjects,
            vec![
                Subject::StorageHomeDelegation(2),
                Subject::StorageHomeDelegation(2),
                Subject::StorageHomeDelegation(3),
                Subject::StorageHomeDelegation(3)
            ]
        );
    }

    #[test]
    fn corba_p3_8_16_only_factories_delegate_to_the_executor() {
        let home = HomeFacts {
            factories: vec!["create".into(), "make".into()],
            operations: vec!["create".into(), "make".into(), "op".into()],
            ..keyless()
        };
        let mut c = composition(CompositionCategory::Session);
        c.home_executor.executor_delegations = vec![
            Delegation {
                operation: "make".into(),
                target: Some("build".into()),
            },
            Delegation {
                operation: "create".into(),
                target: None,
            },
        ];
        assert!(c.validate(&home, None).is_empty());
        c.home_executor.executor_delegations.push(Delegation {
            operation: "op".into(),
            target: None,
        });
        let errors = c.validate(&home, None);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].subject, Subject::ExecutorDelegation(2));
    }

    #[test]
    fn corba_p3_8_17_8_18_abstract_and_proxy_operations_are_home_operations() {
        let home = HomeFacts {
            operations: vec!["create".into(), "op".into(), "op2".into()],
            ..keyless()
        };
        let mut c = composition(CompositionCategory::Session);
        c.home_executor.abstract_operations = vec!["op".into(), "nope".into()];
        c.proxy_home = Some(ProxyHomeDef {
            name: "Proxy".into(),
            home_delegations: vec![
                Delegation {
                    operation: "op".into(),
                    target: None,
                },
                Delegation {
                    operation: "op2".into(),
                    target: Some("gone".into()),
                },
            ],
            abstract_operations: vec!["op".into()],
        });
        let subjects: Vec<Subject> = c.validate(&home, None).iter().map(|e| e.subject).collect();
        assert_eq!(
            subjects,
            vec![
                Subject::AbstractOperation(1),
                Subject::ProxyDelegation(1),
                Subject::ProxyAbstractOperation(0)
            ]
        );
    }

    #[test]
    fn corba_p3_8_5_the_category_keywords_round_trip() {
        for category in CompositionCategory::ALL {
            assert_eq!(
                CompositionCategory::from_keyword(category.as_str()),
                Some(category)
            );
        }
        assert_eq!(CompositionCategory::from_keyword("Entity"), None);
    }
}
