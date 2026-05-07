// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DLRL 1.2 Metamodel + Mapping + Entity-Hierarchie + Exception-Types.
//!
//! Spec-Quelle: `dds-1.2.pdf` §8 DLRL. Wir liefern die UML-Metamodel-
//! Klassen-Hierarchie + Mapping-Information + die 17 Entity-Klassen
//! + 8 Exception-Types + Default-Mapping-Convention als Stub-Layer.

use alloc::string::String;
use alloc::vec::Vec;

// ===========================================================================
// §8.1.3.2.1 ObjectRoot — Inheritance-Basis
// ===========================================================================

/// Spec §8.1.3.2.1 — `ObjectRoot` als gemeinsame abstrakte Basis fuer
/// alle DLRL-Objects.
pub trait ObjectRoot: Send + Sync {
    /// Spec §8.1.3.1 — Object-Identity (oid).
    fn oid(&self) -> u64;
    /// Repository-ID des Object-Type.
    fn repository_id(&self) -> &str;
    /// Spec §8.1.3.1 — `is_modified()`.
    fn is_modified(&self) -> bool;
    /// Spec §8.1.3.1 — `is_deleted()`.
    fn is_deleted(&self) -> bool;
}

// ===========================================================================
// §8.1.3.3 + §8.1.4.4 Metamodel mit Mapping-Information
// ===========================================================================

/// Spec §8.1.3.3 — `Class`-Metamodel-Element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassMetamodel {
    /// Class-Name.
    pub name: String,
    /// Repository-ID.
    pub repository_id: String,
    /// Spec §8.1.4.4 — DCPS-Topic-Mapping (Default = `<Class>`).
    pub main_topic: String,
    /// Spec §8.1.4.4 — Field-Name fuer oid (Default = `oid`).
    pub oid_field: String,
    /// Spec §8.1.4.4 — Field-Name fuer Class (Default = `class`).
    pub class_field: String,
    /// Spec §8.1.4.4 — `full_oid_required`-Flag.
    pub full_oid_required: bool,
    /// Spec §8.1.4.4 — `final` Flag (kein Sub-Class).
    pub final_class: bool,
}

/// Spec §8.1.3.3 — `MultiAttribute`-Element (Sequence/List).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiAttributeMetamodel {
    /// Attribute-Name.
    pub name: String,
    /// Topic-Mapping (Default = `<Class>.<attribute>`).
    pub topic: String,
    /// Spec §8.1.4.4 — `target_field`-Name.
    pub target_field: String,
    /// Spec §8.1.4.4 — `index_field` fuer Multi-Attribute.
    pub index_field: String,
    /// Spec §8.1.4.4 — `key_fields` (Liste).
    pub key_fields: Vec<String>,
}

/// Spec §8.1.3.3 — `MonoRelation`-Element (1:1-Beziehung).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonoRelationMetamodel {
    /// Relation-Name.
    pub name: String,
    /// Topic-Mapping.
    pub topic: String,
    /// Target-Fields (Liste).
    pub target_fields: Vec<String>,
    /// Key-Fields (Liste).
    pub key_fields: Vec<String>,
    /// `full_oid_required`-Flag.
    pub full_oid_required: bool,
    /// `is_composition`-Flag.
    pub is_composition: bool,
}

/// Spec §8.1.4.3 — Default-Mapping-Convention.
pub mod default_mapping {
    /// Default-Topic-Name = `<Class>`.
    #[must_use]
    pub fn topic_name(class: &str) -> alloc::string::String {
        class.into()
    }

    /// Default-OID-Field-Name.
    pub const DEFAULT_OID_FIELD: &str = "oid";

    /// Default-Class-Field-Name.
    pub const DEFAULT_CLASS_FIELD: &str = "class";

    /// Default-Index-Field-Name fuer Multi-Attribute.
    pub const DEFAULT_INDEX_FIELD: &str = "index";

    /// Default-Multi-Topic-Form = `<Class>.<attribute>`.
    #[must_use]
    pub fn multi_topic(class: &str, attribute: &str) -> alloc::string::String {
        alloc::format!("{class}.{attribute}")
    }
}

// ===========================================================================
// §8.1.6.2 — 17 DLRL-Entity-Klassen
// ===========================================================================

/// Spec §8.1.6.2 — DLRL-Entity-Klassen-Liste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlrlEntityKind {
    /// `CacheFactory`.
    CacheFactory,
    /// `CacheBase`.
    CacheBase,
    /// `Cache`.
    Cache,
    /// `CacheAccess`.
    CacheAccess,
    /// `Contract`.
    Contract,
    /// `Selection`.
    Selection,
    /// `SelectionCriterion`.
    SelectionCriterion,
    /// `FilterCriterion`.
    FilterCriterion,
    /// `QueryCriterion`.
    QueryCriterion,
    /// `SelectionListener`.
    SelectionListener,
    /// `ObjectRoot`.
    ObjectRoot,
    /// `ObjectHome`.
    ObjectHome,
    /// `Collection`.
    Collection,
    /// `List`.
    List,
    /// `Set`.
    Set,
    /// `StrMap`.
    StrMap,
    /// `IntMap`.
    IntMap,
}

impl DlrlEntityKind {
    /// Spec §8.1.6.2 — Spec-Klassen-Name.
    #[must_use]
    pub const fn spec_name(self) -> &'static str {
        match self {
            Self::CacheFactory => "CacheFactory",
            Self::CacheBase => "CacheBase",
            Self::Cache => "Cache",
            Self::CacheAccess => "CacheAccess",
            Self::Contract => "Contract",
            Self::Selection => "Selection",
            Self::SelectionCriterion => "SelectionCriterion",
            Self::FilterCriterion => "FilterCriterion",
            Self::QueryCriterion => "QueryCriterion",
            Self::SelectionListener => "SelectionListener",
            Self::ObjectRoot => "ObjectRoot",
            Self::ObjectHome => "ObjectHome",
            Self::Collection => "Collection",
            Self::List => "List",
            Self::Set => "Set",
            Self::StrMap => "StrMap",
            Self::IntMap => "IntMap",
        }
    }

    /// Liste aller 17 Entity-Kinds.
    #[must_use]
    pub fn all() -> [Self; 17] {
        [
            Self::CacheFactory,
            Self::CacheBase,
            Self::Cache,
            Self::CacheAccess,
            Self::Contract,
            Self::Selection,
            Self::SelectionCriterion,
            Self::FilterCriterion,
            Self::QueryCriterion,
            Self::SelectionListener,
            Self::ObjectRoot,
            Self::ObjectHome,
            Self::Collection,
            Self::List,
            Self::Set,
            Self::StrMap,
            Self::IntMap,
        ]
    }
}

// ===========================================================================
// §8.1.6.2 — 8 Exception-Typen
// ===========================================================================

/// Spec §8.1.6.2 — DLRL-Exception-Hierarchie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DlrlException {
    /// `DCPSError` — Generic-Error vom DCPS-Layer.
    DcpsError {
        /// Reason-String.
        reason: String,
    },
    /// `BadHomeDefinition` — Class-/Home-Mismatch.
    BadHomeDefinition {
        /// Reason-String.
        reason: String,
    },
    /// `NotFound` — Object/Selection nicht im Cache.
    NotFound,
    /// `AlreadyExisting` — Object mit dem Key existiert bereits.
    AlreadyExisting,
    /// `AlreadyDeleted` — Operation auf bereits geloeschtem Object.
    AlreadyDeleted,
    /// `PreconditionNotMet` — Constraint-Violation (z.B.
    /// is_modified-Tracking-Pflicht).
    PreconditionNotMet {
        /// Constraint-Beschreibung.
        constraint: String,
    },
    /// `NoSuchElement` — Iterator-/Collection-Out-of-Range.
    NoSuchElement,
    /// `SQLError` — Annex-B-Query-Filter-Parse-Error.
    SqlError {
        /// Reason-String.
        reason: String,
    },
}

impl DlrlException {
    /// Spec-Exception-ID (Repository-ID-Form).
    #[must_use]
    pub const fn repository_id(&self) -> &'static str {
        match self {
            Self::DcpsError { .. } => "IDL:omg.org/DLRL/DCPSError:1.0",
            Self::BadHomeDefinition { .. } => "IDL:omg.org/DLRL/BadHomeDefinition:1.0",
            Self::NotFound => "IDL:omg.org/DLRL/NotFound:1.0",
            Self::AlreadyExisting => "IDL:omg.org/DLRL/AlreadyExisting:1.0",
            Self::AlreadyDeleted => "IDL:omg.org/DLRL/AlreadyDeleted:1.0",
            Self::PreconditionNotMet { .. } => "IDL:omg.org/DLRL/PreconditionNotMet:1.0",
            Self::NoSuchElement => "IDL:omg.org/DLRL/NoSuchElement:1.0",
            Self::SqlError { .. } => "IDL:omg.org/DLRL/SQLError:1.0",
        }
    }
}

// ===========================================================================
// §8.1.5 Cache-DCPS-Lifecycle (Attachment + Creation + QoS)
// ===========================================================================

/// Spec §8.1.5.1 — Cache-Attachment-State.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheAttachmentState {
    /// Cache nicht an DCPS-Pub/Sub gebunden.
    Detached,
    /// Cache an Subscriber gebunden (Read-Only).
    AttachedSubscriber,
    /// Cache an Publisher gebunden (Write-Only).
    AttachedPublisher,
    /// Cache an beide (Read-Write).
    AttachedBoth,
}

/// Spec §8.1.5 — Cache-Mode (Spec §8.1.6.1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheMode {
    /// `Transparent` — Cache laedt automatisch, kein Caller-Pull.
    Transparent,
    /// `OnDemand` — Caller laedt explizit via `refresh`.
    OnDemand,
}

// ===========================================================================
// Annex B — Query/Filter mit SQL-92-Subset
// ===========================================================================

/// Annex B — `QueryCriterion`-Expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryExpression {
    /// SQL-92-Subset-Expression (z.B. `"price > 100 AND symbol = 'AAPL'"`).
    pub expr: String,
    /// Bind-Parameter.
    pub params: Vec<String>,
}

impl QueryExpression {
    /// Konstruktor.
    #[must_use]
    pub fn new(expr: impl Into<String>) -> Self {
        Self {
            expr: expr.into(),
            params: Vec::new(),
        }
    }

    /// Fuegt einen Bind-Parameter hinzu.
    pub fn add_param(&mut self, p: impl Into<String>) {
        self.params.push(p.into());
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    // §8.1.3.2.1 ObjectRoot
    struct DummyObject;
    impl ObjectRoot for DummyObject {
        fn oid(&self) -> u64 {
            42
        }
        fn repository_id(&self) -> &str {
            "IDL:demo/Dummy:1.0"
        }
        fn is_modified(&self) -> bool {
            false
        }
        fn is_deleted(&self) -> bool {
            false
        }
    }

    #[test]
    fn object_root_trait_callable() {
        let o = DummyObject;
        assert_eq!(o.oid(), 42);
        assert_eq!(o.repository_id(), "IDL:demo/Dummy:1.0");
        assert!(!o.is_modified());
        assert!(!o.is_deleted());
    }

    // §8.1.3.3 / §8.1.4.4 Metamodel
    #[test]
    fn class_metamodel_default_fields() {
        let c = ClassMetamodel {
            name: "Trade".into(),
            repository_id: "IDL:demo/Trade:1.0".into(),
            main_topic: default_mapping::topic_name("Trade"),
            oid_field: default_mapping::DEFAULT_OID_FIELD.into(),
            class_field: default_mapping::DEFAULT_CLASS_FIELD.into(),
            full_oid_required: false,
            final_class: false,
        };
        assert_eq!(c.main_topic, "Trade");
        assert_eq!(c.oid_field, "oid");
        assert_eq!(c.class_field, "class");
    }

    #[test]
    fn default_mapping_topic_name_uses_class() {
        assert_eq!(default_mapping::topic_name("Trader"), "Trader");
    }

    #[test]
    fn default_mapping_multi_topic_dot_separated() {
        assert_eq!(
            default_mapping::multi_topic("Order", "items"),
            "Order.items"
        );
    }

    #[test]
    fn default_mapping_constants_match_spec() {
        assert_eq!(default_mapping::DEFAULT_OID_FIELD, "oid");
        assert_eq!(default_mapping::DEFAULT_CLASS_FIELD, "class");
        assert_eq!(default_mapping::DEFAULT_INDEX_FIELD, "index");
    }

    #[test]
    fn multi_attribute_metamodel_construct() {
        let m = MultiAttributeMetamodel {
            name: "items".into(),
            topic: "Order.items".into(),
            target_field: "value".into(),
            index_field: "index".into(),
            key_fields: alloc::vec!["order_id".into()],
        };
        assert_eq!(m.key_fields.len(), 1);
    }

    #[test]
    fn mono_relation_metamodel_construct() {
        let r = MonoRelationMetamodel {
            name: "owner".into(),
            topic: "Trade.owner".into(),
            target_fields: alloc::vec!["trader_id".into()],
            key_fields: alloc::vec!["trade_id".into()],
            full_oid_required: true,
            is_composition: false,
        };
        assert!(r.full_oid_required);
    }

    // §8.1.6.2 17 Entity-Klassen
    #[test]
    fn dlrl_entity_kinds_count_17() {
        assert_eq!(DlrlEntityKind::all().len(), 17);
    }

    #[test]
    fn dlrl_entity_kinds_distinct() {
        let kinds = DlrlEntityKind::all();
        for (i, k1) in kinds.iter().enumerate() {
            for k2 in kinds.iter().skip(i + 1) {
                assert_ne!(k1, k2);
            }
        }
    }

    #[test]
    fn dlrl_entity_kind_spec_names_match() {
        assert_eq!(DlrlEntityKind::CacheFactory.spec_name(), "CacheFactory");
        assert_eq!(DlrlEntityKind::ObjectRoot.spec_name(), "ObjectRoot");
        assert_eq!(DlrlEntityKind::IntMap.spec_name(), "IntMap");
    }

    // §8.1.6.2 8 Exception-Typen
    #[test]
    fn dlrl_exception_repository_ids_distinct() {
        let exceptions = [
            DlrlException::DcpsError { reason: "x".into() },
            DlrlException::BadHomeDefinition { reason: "x".into() },
            DlrlException::NotFound,
            DlrlException::AlreadyExisting,
            DlrlException::AlreadyDeleted,
            DlrlException::PreconditionNotMet {
                constraint: "x".into(),
            },
            DlrlException::NoSuchElement,
            DlrlException::SqlError { reason: "x".into() },
        ];
        let mut seen = std::collections::HashSet::new();
        for e in &exceptions {
            assert!(
                seen.insert(e.repository_id()),
                "duplicate: {}",
                e.repository_id()
            );
        }
        assert_eq!(seen.len(), 8);
    }

    #[test]
    fn dlrl_exception_repo_id_omg_namespace() {
        for e in [
            DlrlException::NotFound,
            DlrlException::AlreadyExisting,
            DlrlException::NoSuchElement,
        ] {
            assert!(e.repository_id().starts_with("IDL:omg.org/DLRL/"));
        }
    }

    // §8.1.5 Cache-Lifecycle
    #[test]
    fn cache_attachment_states_distinct() {
        assert_ne!(
            CacheAttachmentState::Detached,
            CacheAttachmentState::AttachedSubscriber
        );
        assert_ne!(
            CacheAttachmentState::AttachedPublisher,
            CacheAttachmentState::AttachedBoth
        );
    }

    #[test]
    fn cache_modes_distinct() {
        assert_ne!(CacheMode::Transparent, CacheMode::OnDemand);
    }

    // Annex B
    #[test]
    fn query_expression_construct() {
        let mut q = QueryExpression::new("price > ?");
        q.add_param("100");
        assert_eq!(q.expr, "price > ?");
        assert_eq!(q.params, alloc::vec!["100".to_string()]);
    }
}
