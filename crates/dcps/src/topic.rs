// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Topic — der typed rendezvous-point zwischen DataWriter und DataReader.
//!
//! Spec-Referenz:
//! - OMG DDS 1.4 §2.2.2.3.1 `TopicDescription` (Base-Klasse fuer
//!   `Topic`, `ContentFilteredTopic`, `MultiTopic`),
//! - §2.2.2.3.2 `Topic` (concrete TopicDescription mit Type +
//!   Topic-QoS),
//! - §2.2.2.2.1.12 `lookup_topicdescription` (untypisiertes Lookup),
//! - §2.2.2.2.1.13 `create_contentfilteredtopic`.
//!
//! In v1.2 sind wir schlicht: der `Topic<T>` ist ein Handle, das
//! name + type_name traegt und einen generischen `PhantomData<T>`
//! fuer statische Typ-Sicherheit. Der Topic referenziert seinen
//! `DomainParticipant`, damit `TopicDescription::get_participant`
//! Spec-treu funktioniert. Der Cycle-Bruch sitzt in der
//! Topic-Registry: die haelt nur `Arc<TopicInner>`, **nicht** den
//! geklonten `DomainParticipant`-Handle.

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::marker::PhantomData;

#[cfg(feature = "std")]
use std::sync::RwLock;

use zerodds_sql_filter::{Expr, RowAccess, Value};

use crate::dds_type::DdsType;
use crate::entity::StatusMask;
use crate::error::{DdsError, Result};
use crate::listener::ArcTopicListener;
use crate::participant::DomainParticipant;
use crate::qos::TopicQos;

/// `TopicDescription`-Trait — Base-Interface fuer alles, woraus
/// ein `DataReader` (typed via `T`) Samples beziehen kann.
///
/// Spec-Referenz: OMG DDS 1.4 §2.2.2.3.1 "TopicDescription is the
/// most abstract description of a topic. It encapsulates the
/// information that is common to all the kinds of topic that can be
/// used: `Topic`, `ContentFilteredTopic`, `MultiTopic`."
///
/// Der Trait ist **object-safe** — wir wollen ihn als
/// `&dyn TopicDescription` aus `lookup_topicdescription` und
/// `find_topic` zurueckgeben koennen.
pub trait TopicDescription {
    /// Type-Name (z.B. `"std_msgs::msg::String"`).
    /// Spec §2.2.2.3.1 `get_type_name`.
    fn get_type_name(&self) -> &str;
    /// Topic-Name (z.B. `"ChatterTopic"`).
    /// Spec §2.2.2.3.1 `get_name`.
    fn get_name(&self) -> &str;
    /// Owning Participant.
    /// Spec §2.2.2.3.1 `get_participant`.
    fn get_participant(&self) -> &DomainParticipant;
}

/// Typed Topic-Handle.
#[derive(Debug)]
pub struct Topic<T: DdsType> {
    inner: Arc<TopicInner>,
    /// Owning Participant (kein Cycle: der Participant haelt im
    /// Topic-Registry nur den Inner, nicht den Handle). `None` bei
    /// Builtin-Topics, die ohne Participant konstruiert werden
    /// (siehe [`Self::new_orphan`]).
    participant: Option<DomainParticipant>,
    _t: PhantomData<T>,
}

/// Inner-State eines Topics (shared via Arc, weil Handles geklont werden).
pub(crate) struct TopicInner {
    /// Topic-Name (z.B. "ChatterTopic").
    pub name: String,
    /// Type-Name aus `T::TYPE_NAME` (eager copied, damit der Inner
    /// kein Generic-Typ hat — das vereinfacht die Participant-
    /// Registry-Datenstruktur in .2a).
    pub type_name: &'static str,
    /// Topic-QoS — mutable via `Entity::set_qos`.
    #[cfg(feature = "std")]
    pub qos: std::sync::Mutex<TopicQos>,
    #[cfg(not(feature = "std"))]
    pub qos: TopicQos,
    /// Entity-Lifecycle (DCPS §2.2.2.1).
    pub entity_state: Arc<crate::entity::EntityState>,
    /// optionaler `TopicListener` + StatusMask. Spec
    /// §2.2.2.3.2.x set_listener / Bubble-Up §2.2.4.2.3.
    #[cfg(feature = "std")]
    pub listener: std::sync::Mutex<Option<(ArcTopicListener, StatusMask)>>,
    /// Counter fuer InconsistentTopic-Detections (Spec §2.2.4.2.5).
    /// Inkrementiert in `record_inconsistent_topic`, ausgelesen in
    /// `inconsistent_topic_status` mit Delta-Detection.
    #[cfg(feature = "std")]
    pub inconsistent_topic_count: std::sync::atomic::AtomicI64,
    /// zuletzt gesehener Wert fuer Delta-Trigger.
    #[cfg(feature = "std")]
    pub last_inconsistent_topic: std::sync::atomic::AtomicI64,
}

impl core::fmt::Debug for TopicInner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        #[cfg(feature = "std")]
        let listener_present = self.listener.lock().map(|s| s.is_some()).unwrap_or(false);
        #[cfg(not(feature = "std"))]
        let listener_present = false;
        f.debug_struct("TopicInner")
            .field("name", &self.name)
            .field("type_name", &self.type_name)
            .field("listener_present", &listener_present)
            .finish_non_exhaustive()
    }
}

impl<T: DdsType> Topic<T> {
    /// Erzeugt einen neuen Topic-Handle. Wird normalerweise vom
    /// `DomainParticipant::create_topic<T>(name, qos)` aufgerufen.
    #[must_use]
    pub fn new(name: String, qos: TopicQos, participant: DomainParticipant) -> Self {
        Self {
            inner: Arc::new(TopicInner {
                name,
                type_name: T::TYPE_NAME,
                #[cfg(feature = "std")]
                qos: std::sync::Mutex::new(qos),
                #[cfg(not(feature = "std"))]
                qos,
                entity_state: crate::entity::EntityState::new(),
                #[cfg(feature = "std")]
                listener: std::sync::Mutex::new(None),
                #[cfg(feature = "std")]
                inconsistent_topic_count: std::sync::atomic::AtomicI64::new(0),
                #[cfg(feature = "std")]
                last_inconsistent_topic: std::sync::atomic::AtomicI64::new(-1),
            }),
            participant: Some(participant),
            _t: PhantomData,
        }
    }

    /// Erzeugt einen Topic-Handle **ohne** Participant — fuer Builtin-
    /// Topics (DCPSParticipant/Topic/Publication/Subscription), die im
    /// BuiltinSubscriber-Konstruktor entstehen, bevor der DomainParticipant
    /// fertig ist (Henne-Ei-Problem). `get_participant()` panickt
    /// auf einem orphan-Topic — Builtin-Reader umgehen das.
    #[must_use]
    pub fn new_orphan(name: String, qos: TopicQos) -> Self {
        Self {
            inner: Arc::new(TopicInner {
                name,
                type_name: T::TYPE_NAME,
                #[cfg(feature = "std")]
                qos: std::sync::Mutex::new(qos),
                #[cfg(not(feature = "std"))]
                qos,
                entity_state: crate::entity::EntityState::new(),
                #[cfg(feature = "std")]
                listener: std::sync::Mutex::new(None),
                #[cfg(feature = "std")]
                inconsistent_topic_count: std::sync::atomic::AtomicI64::new(0),
                #[cfg(feature = "std")]
                last_inconsistent_topic: std::sync::atomic::AtomicI64::new(-1),
            }),
            participant: None,
            _t: PhantomData,
        }
    }

    /// Topic-Name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.inner.name
    }

    /// Type-Name (aus `T::TYPE_NAME`).
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        self.inner.type_name
    }

    /// setzt den `TopicListener` + StatusMask. `None` loescht
    /// den Slot. Spec §2.2.2.3.2.x set_listener.
    #[cfg(feature = "std")]
    pub fn set_listener(&self, listener: Option<ArcTopicListener>, mask: StatusMask) {
        if let Ok(mut slot) = self.inner.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        self.inner.entity_state.set_listener_mask(mask);
    }

    /// aktueller Listener-Klon, sofern vorhanden.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcTopicListener> {
        self.inner
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
    }

    /// Snapshot der Bubble-Up-Kette (Topic → Participant) — fuer
    /// Hot-Path-Listener-Dispatch (z.B. on_inconsistent_topic).
    ///
    /// Aktuell hat der Hot-Path keinen Inconsistent-Topic-Trigger
    /// (dazu braucht es Cross-Vendor Type-Mismatch-Detection in WP
    /// 2.x); die Snapshot-API existiert bereits, damit sie spaeter
    /// ohne API-Change verdrahtet werden kann.
    #[cfg(feature = "std")]
    #[must_use]
    pub(crate) fn listener_chain(&self) -> crate::listener_dispatch::TopicListenerChain {
        let topic = self
            .inner
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, m)| (Arc::clone(l), *m)));
        let participant = self
            .participant
            .as_ref()
            .and_then(|p| p.snapshot_listener());
        crate::listener_dispatch::TopicListenerChain { topic, participant }
    }

    /// Recordet eine InconsistentTopic-Detection (z.B. wenn
    /// ein remote Topic mit demselben Namen, aber anderem Type-Name
    /// entdeckt wurde, oder wenn `create_topic` mit gleichem Namen aber
    /// unterschiedlichem Type aufgerufen wird). Spec §2.2.4.2.5.
    #[cfg(feature = "std")]
    pub fn record_inconsistent_topic(&self) {
        self.inner
            .inconsistent_topic_count
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }

    /// aktueller `InconsistentTopicStatus` + Trigger via
    /// Bubble-Up bei Delta gegenueber dem letzten Aufruf.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn inconsistent_topic_status(&self) -> crate::status::InconsistentTopicStatus {
        let curr = self
            .inner
            .inconsistent_topic_count
            .load(std::sync::atomic::Ordering::Acquire);
        let prev = self
            .inner
            .last_inconsistent_topic
            .swap(curr, std::sync::atomic::Ordering::AcqRel);
        let delta = if prev < 0 { curr } else { curr - prev };
        let status = crate::status::InconsistentTopicStatus {
            total_count: curr as i32,
            total_count_change: delta as i32,
        };
        // Trigger Listener nur bei tatsaechlichem Delta — sonst wird der
        // Listener bei jedem Status-Read mehrfach gefeuert (idempotent).
        let actually_changed = if prev < 0 { curr != 0 } else { prev != curr };
        if actually_changed {
            let chain = self.listener_chain();
            crate::listener_dispatch::dispatch_inconsistent_topic(
                &chain,
                self.inner.entity_state.instance_handle(),
                status,
            );
        }
        status
    }

    /// Topic-QoS (cloned).  Inner-Mutex erlaubt set_qos.
    #[must_use]
    pub fn qos(&self) -> TopicQos {
        #[cfg(feature = "std")]
        {
            self.inner.qos.lock().map(|q| q.clone()).unwrap_or_default()
        }
        #[cfg(not(feature = "std"))]
        {
            self.inner.qos.clone()
        }
    }

    /// Interner Zugriff auf den shared state (fuer Publisher/Subscriber).
    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> Arc<TopicInner> {
        Arc::clone(&self.inner)
    }

    /// Interner Konstruktor fuer shared-handle-Rehydrierung in der
    /// Participant-Topic-Registry (`create_topic` zweite Runde
    /// mit gleichem Namen + Typ).
    pub(crate) fn _from_inner_impl(inner: Arc<TopicInner>, participant: DomainParticipant) -> Self {
        Self {
            inner,
            participant: Some(participant),
            _t: PhantomData,
        }
    }
}

impl<T: DdsType> Clone for Topic<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            participant: self.participant.clone(),
            _t: PhantomData,
        }
    }
}

impl<T: DdsType> TopicDescription for Topic<T> {
    fn get_type_name(&self) -> &str {
        self.inner.type_name
    }
    fn get_name(&self) -> &str {
        &self.inner.name
    }
    /// Liefert den Owning-Participant. Panickt bei Builtin-Topics
    /// (orphan), die ueber [`Topic::new_orphan`] konstruiert wurden —
    /// Builtin-Reader rufen `get_participant()` nicht.
    #[allow(clippy::expect_used, clippy::panic)]
    fn get_participant(&self) -> &DomainParticipant {
        match &self.participant {
            Some(p) => p,
            None => panic!(
                "get_participant on orphan (builtin) topic — builtin readers must not call this"
            ),
        }
    }
}

// ============================================================================
// Entity-Trait (DCPS §2.2.2.1) —
// ============================================================================

#[cfg(feature = "std")]
impl<T: DdsType> crate::entity::Entity for Topic<T> {
    type Qos = TopicQos;

    fn get_qos(&self) -> Self::Qos {
        self.inner.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    /// TopicQos. Spec §2.2.3: alle Topic-Policies (TOPIC_DATA, DURABILITY,
    /// RELIABILITY, HISTORY, RESOURCE_LIMITS, ...) sind nach `enable()`
    /// **immutable** — nur TOPIC_DATA ist Changeable=YES.
    fn set_qos(&self, qos: Self::Qos) -> Result<()> {
        let enabled = self.inner.entity_state.is_enabled();
        if let Ok(mut current) = self.inner.qos.lock() {
            if enabled {
                // Spec §2.2.3: DURABILITY und RELIABILITY sind
                // Changeable=NO post-enable.
                if current.durability != qos.durability {
                    return Err(crate::entity::immutable_if_enabled("DURABILITY"));
                }
                if current.reliability != qos.reliability {
                    return Err(crate::entity::immutable_if_enabled("RELIABILITY"));
                }
            }
            *current = qos;
        }
        Ok(())
    }

    fn enable(&self) -> Result<()> {
        self.inner.entity_state.enable();
        Ok(())
    }

    fn entity_state(&self) -> Arc<crate::entity::EntityState> {
        Arc::clone(&self.inner.entity_state)
    }
}

/// Untypisiertes `TopicDescription`-Handle, das aus
/// `DomainParticipant::lookup_topicdescription` /
/// `DomainParticipant::find_topic` zurueckgegeben wird.
///
/// Die Spec-API liefert die abstrakte Base-Klasse, weil der Caller
/// im Generalfall den Typ noch nicht kennt (z.B. nach Discovery).
/// Wir packen Name + Type-Name + Participant in ein clone-bares
/// Handle; der Caller kann `get_type_name()` lesen und sich dann
/// per `create_topic::<T>` einen typed Handle holen.
#[derive(Debug, Clone)]
pub struct TopicDescriptionHandle {
    name: String,
    type_name: String,
    participant: DomainParticipant,
}

impl TopicDescriptionHandle {
    /// Konstruiere ein Handle (intern; v.a. fuer Tests + Lookup-API).
    pub(crate) fn new(name: String, type_name: String, participant: DomainParticipant) -> Self {
        Self {
            name,
            type_name,
            participant,
        }
    }
}

impl TopicDescription for TopicDescriptionHandle {
    fn get_type_name(&self) -> &str {
        &self.type_name
    }
    fn get_name(&self) -> &str {
        &self.name
    }
    fn get_participant(&self) -> &DomainParticipant {
        &self.participant
    }
}

/// `ContentFilteredTopic<T>` — Sub-Topic eines `Topic<T>` mit
/// Filter-Expression. Spec-Referenz: OMG DDS 1.4 §2.2.2.3.3.
///
/// "ContentFilteredTopic is a specialization of TopicDescription that
/// allows for content-based subscriptions. The selection of the
/// content is done using a filter_expression with parameters
/// (filter_parameters)."
///
/// Die Filter-Expression ist ein SQL-Subset (DDS-DCPS Annex B, BNF):
/// `field op literal-or-param`, `AND`/`OR`/`NOT`-Komposition, parens,
/// `LIKE`. Der konkrete Parser sitzt in `zerodds-sql-filter`.
///
/// **Lifecycle:** das CFT haelt einen Klon des Related-Topics (shared
/// `Arc<TopicInner>`) und ist clone-bar selbst. Filter-Parameter sind
/// veraenderbar zur Laufzeit (Spec §2.2.2.3.3.7).
#[derive(Debug)]
pub struct ContentFilteredTopic<T: DdsType> {
    name: String,
    related_topic: Topic<T>,
    /// Roh-Form der Expression (read-only — Spec ist explizit, dass
    /// nur die `filter_parameters` veraenderbar sind, nicht die
    /// Expression selbst).
    filter_expression: String,
    /// Vorab-geparste AST. Wir parsen einmal beim Konstruktor, damit
    /// `evaluate` heiss-pfad ist.
    parsed: Arc<Expr>,
    /// Filter-Parameter (`%0`, `%1`, ...). Werden als Strings
    /// uebergeben (Spec §2.2.2.3.3 — die Parameter sind Strings, die
    /// in die Expression substituiert werden). Wir tragen sie als
    /// Strings + parsed `Value` parallel, damit `set_filter_parameters`
    /// nur einmal die String→Value-Konversion macht.
    #[cfg(feature = "std")]
    params: Arc<RwLock<FilterParams>>,
    #[cfg(not(feature = "std"))]
    params: FilterParams,
    participant: DomainParticipant,
    _t: PhantomData<T>,
}

#[derive(Debug, Clone)]
struct FilterParams {
    raw: Vec<String>,
    values: Vec<Value>,
}

impl<T: DdsType> ContentFilteredTopic<T> {
    /// Konstruktor (intern aufgerufen von
    /// `DomainParticipant::create_contentfilteredtopic`).
    ///
    /// # Errors
    /// `BadParameter` wenn die Expression nicht parst oder ein
    /// referenzierter `%N`-Index nicht im `filter_parameters`-Vec
    /// existiert.
    pub(crate) fn new(
        name: String,
        related_topic: Topic<T>,
        filter_expression: String,
        filter_parameters: Vec<String>,
        participant: DomainParticipant,
    ) -> Result<Self> {
        let parsed =
            zerodds_sql_filter::parse(&filter_expression).map_err(|_| DdsError::BadParameter {
                what: "filter expression syntax",
            })?;
        // Pruefe Parameter-Indices.
        let used = parsed.collect_param_indices();
        if let Some(max) = used.iter().max() {
            if (*max as usize) >= filter_parameters.len() {
                return Err(DdsError::BadParameter {
                    what: "filter parameter %N out of range",
                });
            }
        }
        let values: Vec<Value> = filter_parameters
            .iter()
            .map(|s| param_string_to_value(s))
            .collect();
        let fp = FilterParams {
            raw: filter_parameters,
            values,
        };
        Ok(Self {
            name,
            related_topic,
            filter_expression,
            parsed: Arc::new(parsed),
            #[cfg(feature = "std")]
            params: Arc::new(RwLock::new(fp)),
            #[cfg(not(feature = "std"))]
            params: fp,
            participant,
            _t: PhantomData,
        })
    }

    /// Spec §2.2.2.3.3.4 `get_filter_expression`.
    #[must_use]
    pub fn get_filter_expression(&self) -> &str {
        &self.filter_expression
    }

    /// Spec §2.2.2.3.3.5 `get_filter_parameters`.
    #[must_use]
    pub fn get_filter_parameters(&self) -> Vec<String> {
        #[cfg(feature = "std")]
        {
            self.params
                .read()
                .map(|p| p.raw.clone())
                .unwrap_or_default()
        }
        #[cfg(not(feature = "std"))]
        {
            self.params.raw.clone()
        }
    }

    /// Spec §2.2.2.3.3.6 `set_filter_parameters`. Tauscht die
    /// Parameter (gleiche Anzahl bleibt nicht erzwungen — die Spec
    /// sagt "should match the number of `%n` tokens" als Empfehlung,
    /// wir verifizieren das hart).
    ///
    /// # Errors
    /// `BadParameter` wenn ein `%N`-Index ausserhalb des neuen Vecs
    /// liegt.
    pub fn set_filter_parameters(&self, params: Vec<String>) -> Result<()> {
        let used = self.parsed.collect_param_indices();
        if let Some(max) = used.iter().max() {
            if (*max as usize) >= params.len() {
                return Err(DdsError::BadParameter {
                    what: "filter parameter %N out of range",
                });
            }
        }
        let values: Vec<Value> = params.iter().map(|s| param_string_to_value(s)).collect();
        let fp = FilterParams {
            raw: params,
            values,
        };
        #[cfg(feature = "std")]
        {
            let mut w = self
                .params
                .write()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "filter params poisoned",
                })?;
            *w = fp;
        }
        #[cfg(not(feature = "std"))]
        {
            // im no_std-Pfad nicht mutable gehalten — Parameter werden
            // beim Konstruktor gesetzt; dieser Pfad ist im v1.2-MVP
            // ohnehin nicht aktiv (dcps verlangt std).
            let _ = fp;
            return Err(DdsError::PreconditionNotMet {
                reason: "set_filter_parameters needs std feature",
            });
        }
        Ok(())
    }

    /// Spec §2.2.2.3.3.3 `get_related_topic`.
    #[must_use]
    pub fn get_related_topic(&self) -> &Topic<T> {
        &self.related_topic
    }

    /// Wertet den Filter gegen ein decodiertes Sample aus. Liefert
    /// `Ok(true)` wenn das Sample passieren soll, `Ok(false)` wenn es
    /// gefiltert wird, `Err` wenn die Expression auf das Row-Schema
    /// nicht passt (Caller-Entscheidung: filter denies oder error).
    ///
    /// # Errors
    /// - `PreconditionNotMet` wenn die Filter-Parameter-Lock vergiftet ist.
    /// - `BadParameter` wenn ein Feld in der Expression im Row nicht
    ///   existiert oder ein Type-Mismatch auftritt.
    pub fn evaluate<R: RowAccess>(&self, row: &R) -> Result<bool> {
        #[cfg(feature = "std")]
        let params = {
            let r = self
                .params
                .read()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "filter params poisoned",
                })?;
            r.values.clone()
        };
        #[cfg(not(feature = "std"))]
        let params = self.params.values.clone();
        self.parsed
            .evaluate(row, &params)
            .map_err(|e| DdsError::BadParameter {
                what: match e {
                    zerodds_sql_filter::EvalError::UnknownField(_) => "filter unknown field",
                    zerodds_sql_filter::EvalError::MissingParam(_) => "filter missing param",
                    zerodds_sql_filter::EvalError::TypeMismatch(_) => "filter type mismatch",
                },
            })
    }
}

impl<T: DdsType> Clone for ContentFilteredTopic<T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            related_topic: self.related_topic.clone(),
            filter_expression: self.filter_expression.clone(),
            parsed: Arc::clone(&self.parsed),
            #[cfg(feature = "std")]
            params: Arc::clone(&self.params),
            #[cfg(not(feature = "std"))]
            params: self.params.clone(),
            participant: self.participant.clone(),
            _t: PhantomData,
        }
    }
}

impl<T: DdsType> TopicDescription for ContentFilteredTopic<T> {
    /// Type-Name kommt vom Related-Topic — ein CFT teilt sich das
    /// Schema mit dem unterliegenden Topic.
    fn get_type_name(&self) -> &str {
        self.related_topic.type_name()
    }
    fn get_name(&self) -> &str {
        &self.name
    }
    fn get_participant(&self) -> &DomainParticipant {
        &self.participant
    }
}

/// `MultiTopic<T>` — Spec §2.2.2.3.4 (DDS 1.4 optionales Feature).
///
/// Eine MultiTopic kombiniert mehrere Underlying-Topics in einer
/// einzigen TopicDescription via `subscription_expression` (SQL-
/// Subset). Der Resulting-Type `T` ist User-definiert; die
/// Underlying-Topics koennen unterschiedliche Types haben.
///
/// **Cross-Topic-Sample-Routing:** Der Join-Operator ist ueber
/// [`hash_join_two`] und [`Self::evaluate_joined`] live. Die
/// `subscription_expression` referenziert Felder als
/// `<topic_name>.<field_path>` (dotted), und [`JoinedRow`]
/// dispatched die Lookups an die matchenden Topic-Quellen.
pub struct MultiTopic<T: DdsType> {
    name: String,
    type_name: String,
    /// Liste der Related-Topics als untypisierte Handles.
    related_topic_names: Vec<String>,
    subscription_expression: String,
    /// Vorab-geparste AST der Subscription-Expression.
    parsed: Arc<Expr>,
    /// Mutable Filter-Parameter (Spec §2.2.2.3.4.7
    /// `set_expression_parameters`).
    #[cfg(feature = "std")]
    params: Arc<RwLock<FilterParams>>,
    #[cfg(not(feature = "std"))]
    params: FilterParams,
    participant: DomainParticipant,
    _t: PhantomData<T>,
}

impl<T: DdsType> core::fmt::Debug for MultiTopic<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MultiTopic")
            .field("name", &self.name)
            .field("type_name", &self.type_name)
            .field("related_topic_names", &self.related_topic_names)
            .field("subscription_expression", &self.subscription_expression)
            .finish_non_exhaustive()
    }
}

impl<T: DdsType> MultiTopic<T> {
    /// Konstruktor (intern aufgerufen von
    /// `DomainParticipant::create_multitopic`).
    ///
    /// # Errors
    /// `BadParameter` bei leerem Namen oder leerer Expression, oder
    /// wenn die Expression nicht parst, oder wenn ein referenzierter
    /// `%N`-Parameter ausserhalb des `expression_parameters`-Vec liegt,
    /// oder wenn `related_topic_names` leer ist.
    pub(crate) fn new(
        name: String,
        type_name: String,
        related_topic_names: Vec<String>,
        subscription_expression: String,
        expression_parameters: Vec<String>,
        participant: DomainParticipant,
    ) -> Result<Self> {
        if related_topic_names.is_empty() {
            return Err(DdsError::BadParameter {
                what: "multitopic needs at least one related topic",
            });
        }
        let parsed = zerodds_sql_filter::parse(&subscription_expression).map_err(|_| {
            DdsError::BadParameter {
                what: "multitopic subscription expression syntax",
            }
        })?;
        let used = parsed.collect_param_indices();
        if let Some(max) = used.iter().max() {
            if (*max as usize) >= expression_parameters.len() {
                return Err(DdsError::BadParameter {
                    what: "multitopic expression parameter %N out of range",
                });
            }
        }
        let values: Vec<Value> = expression_parameters
            .iter()
            .map(|s| param_string_to_value(s))
            .collect();
        let fp = FilterParams {
            raw: expression_parameters,
            values,
        };
        Ok(Self {
            name,
            type_name,
            related_topic_names,
            subscription_expression,
            parsed: Arc::new(parsed),
            #[cfg(feature = "std")]
            params: Arc::new(RwLock::new(fp)),
            #[cfg(not(feature = "std"))]
            params: fp,
            participant,
            _t: PhantomData,
        })
    }

    /// Spec §2.2.2.3.4.4 `get_subscription_expression`.
    #[must_use]
    pub fn get_subscription_expression(&self) -> &str {
        &self.subscription_expression
    }

    /// Spec §2.2.2.3.4.5 `get_expression_parameters`.
    #[must_use]
    pub fn get_expression_parameters(&self) -> Vec<String> {
        #[cfg(feature = "std")]
        {
            self.params
                .read()
                .map(|p| p.raw.clone())
                .unwrap_or_default()
        }
        #[cfg(not(feature = "std"))]
        {
            self.params.raw.clone()
        }
    }

    /// Spec §2.2.2.3.4.6 `set_expression_parameters`.
    ///
    /// # Errors
    /// `BadParameter` wenn ein referenzierter `%N`-Parameter
    /// ausserhalb des neuen Vec liegt; `PreconditionNotMet` bei
    /// Lock-Poisoning.
    pub fn set_expression_parameters(&self, params: Vec<String>) -> Result<()> {
        let used = self.parsed.collect_param_indices();
        if let Some(max) = used.iter().max() {
            if (*max as usize) >= params.len() {
                return Err(DdsError::BadParameter {
                    what: "multitopic expression parameter %N out of range",
                });
            }
        }
        let values: Vec<Value> = params.iter().map(|s| param_string_to_value(s)).collect();
        let fp = FilterParams {
            raw: params,
            values,
        };
        #[cfg(feature = "std")]
        {
            let mut w = self
                .params
                .write()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "multitopic params poisoned",
                })?;
            *w = fp;
        }
        #[cfg(not(feature = "std"))]
        {
            let _ = fp;
            return Err(DdsError::PreconditionNotMet {
                reason: "set_expression_parameters needs std feature",
            });
        }
        Ok(())
    }

    /// Spec §2.2.2.3.4.3 `get_related_topic` (PSM gibt eine Sequence
    /// zurueck — wir liefern die Namen).
    #[must_use]
    pub fn get_related_topic_names(&self) -> &[String] {
        &self.related_topic_names
    }

    /// Wertet die `subscription_expression` gegen ein [`JoinedRow`]
    /// aus (Cross-Topic-Predicate). Spec §2.2.2.3.4.
    ///
    /// # Errors
    /// `PreconditionNotMet` bei Lock-Poisoning oder SQL-Eval-Fehler.
    pub fn evaluate_joined(&self, row: &JoinedRow<'_>) -> Result<bool> {
        #[cfg(feature = "std")]
        let values = {
            let p = self
                .params
                .read()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "multitopic params poisoned",
                })?;
            p.values.clone()
        };
        #[cfg(not(feature = "std"))]
        let values = self.params.values.clone();
        self.parsed
            .evaluate(row, &values)
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "multitopic SQL evaluation failed",
            })
    }
}

/// `JoinedRow` — `RowAccess`-Adapter ueber mehrere benannte
/// Topic-Quellen. Dotted-Pfade `topic.field.sub` werden am ersten
/// `.` geteilt: der Praefix matcht den Topic-Namen, der Rest wird
/// an dessen [`RowAccess::get`] weitergereicht.
///
/// Wenn keine Topic-Quelle den Praefix kennt, wird der gesamte Pfad
/// pauschal an alle Quellen weitergereicht (Fallback fuer
/// undotted-Feld-Referenzen).
pub struct JoinedRow<'a> {
    sources: Vec<(String, &'a dyn RowAccess)>,
}

impl<'a> JoinedRow<'a> {
    /// Konstruktor.
    #[must_use]
    pub fn new(sources: Vec<(String, &'a dyn RowAccess)>) -> Self {
        Self { sources }
    }
}

impl RowAccess for JoinedRow<'_> {
    fn get(&self, path: &str) -> Option<Value> {
        if let Some((prefix, rest)) = path.split_once('.') {
            for (name, src) in &self.sources {
                if name == prefix {
                    return src.get(rest);
                }
            }
        }
        // Kein Praefix-Match -> alle Quellen abfragen, erste Antwort
        // gewinnt.
        for (_, src) in &self.sources {
            if let Some(v) = src.get(path) {
                return Some(v);
            }
        }
        None
    }
}

/// Hash-Join-Helper fuer zwei typed Streams. Iteriert die linke
/// Liste, baut eine HashMap `key -> [&L]` (Build-Phase), iteriert
/// dann die rechte Liste (Probe-Phase) und erzeugt fuer jedes
/// matchende `(L, R)`-Paar via `combine` ein Resultat. Optional
/// wird ein zusaetzliches Predicate via `predicate` (z.B.
/// `MultiTopic::evaluate_joined`) auf jedem Paar geprueft.
///
/// Spec: §2.2.2.3.4 (MultiTopic) — der Hash-Join ist die idiomatische
/// O(n+m)-Implementation der `subscription_expression`.
#[cfg(feature = "std")]
#[allow(clippy::too_many_arguments)]
pub fn hash_join_two<L, R, T, KL, KR, C, P>(
    left: &[L],
    left_topic: &str,
    key_left: KL,
    right: &[R],
    right_topic: &str,
    key_right: KR,
    combine: C,
    predicate: P,
) -> Vec<T>
where
    L: RowAccess,
    R: RowAccess,
    KL: Fn(&L) -> String,
    KR: Fn(&R) -> String,
    C: Fn(&L, &R) -> T,
    P: Fn(&JoinedRow<'_>) -> Result<bool>,
{
    use std::collections::HashMap;
    let mut idx: HashMap<String, Vec<&L>> = HashMap::with_capacity(left.len());
    for l in left {
        idx.entry(key_left(l)).or_default().push(l);
    }
    let mut out = Vec::new();
    for r in right {
        let k = key_right(r);
        let Some(matches) = idx.get(&k) else { continue };
        for l in matches {
            let row = JoinedRow::new(alloc::vec![
                (left_topic.to_string(), *l as &dyn RowAccess),
                (right_topic.to_string(), r as &dyn RowAccess),
            ]);
            if predicate(&row).unwrap_or(false) {
                out.push(combine(l, r));
            }
        }
    }
    out
}

impl<T: DdsType> Clone for MultiTopic<T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            type_name: self.type_name.clone(),
            related_topic_names: self.related_topic_names.clone(),
            subscription_expression: self.subscription_expression.clone(),
            parsed: Arc::clone(&self.parsed),
            #[cfg(feature = "std")]
            params: Arc::clone(&self.params),
            #[cfg(not(feature = "std"))]
            params: self.params.clone(),
            participant: self.participant.clone(),
            _t: PhantomData,
        }
    }
}

impl<T: DdsType> TopicDescription for MultiTopic<T> {
    fn get_type_name(&self) -> &str {
        &self.type_name
    }
    fn get_name(&self) -> &str {
        &self.name
    }
    fn get_participant(&self) -> &DomainParticipant {
        &self.participant
    }
}

/// Heuristik-Konversion eines `filter_parameter`-Strings in einen
/// `Value`: erst Bool, dann Int, dann Float, sonst String. Wir
/// strippen flankierende `'...'`-Anfuehrungszeichen, weil die Spec-
/// Beispiele die Strings oft so liefern.
fn param_string_to_value(s: &str) -> Value {
    let trimmed = s.trim();
    // Bool.
    if trimmed.eq_ignore_ascii_case("TRUE") {
        return Value::Bool(true);
    }
    if trimmed.eq_ignore_ascii_case("FALSE") {
        return Value::Bool(false);
    }
    // Int.
    if let Ok(i) = trimmed.parse::<i64>() {
        return Value::Int(i);
    }
    // Float.
    if let Ok(f) = trimmed.parse::<f64>() {
        return Value::Float(f);
    }
    // String (mit optionalem ''-strip).
    if trimmed.len() >= 2 && trimmed.starts_with('\'') && trimmed.ends_with('\'') {
        return Value::String(trimmed[1..trimmed.len() - 1].to_string());
    }
    Value::String(trimmed.to_string())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::dds_type::RawBytes;
    use crate::factory::DomainParticipantFactory;
    use crate::qos::DomainParticipantQos;

    #[test]
    fn topic_implements_topic_description() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let t = p
            .create_topic::<RawBytes>("Chatter", TopicQos::default())
            .unwrap();
        // Trait-Methoden funktionieren.
        let td: &dyn TopicDescription = &t;
        assert_eq!(td.get_name(), "Chatter");
        assert_eq!(td.get_type_name(), RawBytes::TYPE_NAME);
        assert_eq!(td.get_participant().domain_id(), 0);
    }

    #[test]
    fn topic_description_handle_is_cloneable() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(7, DomainParticipantQos::default());
        let h = TopicDescriptionHandle::new("X".into(), "T".into(), p.clone());
        let h2 = h.clone();
        assert_eq!(h2.get_name(), "X");
        assert_eq!(h2.get_type_name(), "T");
        assert_eq!(h2.get_participant().domain_id(), 7);
    }

    #[test]
    fn topic_description_trait_is_object_safe() {
        // Object-safety verifizieren: wir koennen `&dyn TopicDescription`
        // halten und nicht-statisch dispatchen. Aus mehreren konkreten
        // Implementierungen sammeln.
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(8, DomainParticipantQos::default());
        let t = p
            .create_topic::<RawBytes>("DynA", TopicQos::default())
            .unwrap();
        let h = TopicDescriptionHandle::new("DynB".into(), "T".into(), p.clone());
        let descs: Vec<&dyn TopicDescription> = vec![&t, &h];
        assert_eq!(descs.len(), 2);
        assert_eq!(descs[0].get_name(), "DynA");
        assert_eq!(descs[1].get_name(), "DynB");
    }

    #[test]
    fn topic_description_create_topic_rejects_empty_name() {
        // §2.2.2.2.1.4 create_topic: leerer Topic-Name → BadParameter.
        // Das ist die einzige Stelle, wo TopicDescription-Konsumenten
        // einen ungueltigen TopicDescription-Bauversuch erleben.
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(9, DomainParticipantQos::default());
        let res = p.create_topic::<RawBytes>("", TopicQos::default());
        assert!(matches!(
            res,
            Err(crate::error::DdsError::BadParameter { .. })
        ));
    }

    // -------- MultiTopic (§2.2.2.3.4) --------

    #[test]
    fn multitopic_compiles_and_implements_topic_description() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(13, DomainParticipantQos::default());
        let mt = p
            .create_multitopic::<RawBytes>(
                "Combined",
                "MyResultType",
                alloc::vec!["TopicA".into(), "TopicB".into()],
                "x > %0",
                alloc::vec!["10".into()],
            )
            .unwrap();
        let td: &dyn TopicDescription = &mt;
        assert_eq!(td.get_name(), "Combined");
        assert_eq!(td.get_type_name(), "MyResultType");
        assert_eq!(td.get_participant().domain_id(), 13);
        assert_eq!(mt.get_subscription_expression(), "x > %0");
        assert_eq!(mt.get_related_topic_names().len(), 2);
        assert_eq!(mt.get_expression_parameters().len(), 1);
    }

    #[test]
    fn multitopic_set_expression_parameters_roundtrip() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let mt = p
            .create_multitopic::<RawBytes>(
                "MT",
                "T",
                alloc::vec!["A".into()],
                "v = %0",
                alloc::vec!["100".into()],
            )
            .unwrap();
        assert_eq!(
            mt.get_expression_parameters(),
            alloc::vec!["100".to_string()]
        );
        mt.set_expression_parameters(alloc::vec!["200".into()])
            .unwrap();
        assert_eq!(
            mt.get_expression_parameters(),
            alloc::vec!["200".to_string()]
        );
    }

    #[test]
    fn multitopic_rejects_empty_name() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let res = p.create_multitopic::<RawBytes>(
            "",
            "T",
            alloc::vec!["A".into()],
            "x > 0",
            alloc::vec::Vec::new(),
        );
        assert!(matches!(res, Err(DdsError::BadParameter { .. })));
    }

    #[test]
    fn multitopic_rejects_empty_type_name() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let res = p.create_multitopic::<RawBytes>(
            "MT",
            "",
            alloc::vec!["A".into()],
            "x > 0",
            alloc::vec::Vec::new(),
        );
        assert!(matches!(res, Err(DdsError::BadParameter { .. })));
    }

    #[test]
    fn multitopic_rejects_empty_related_topics() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let res = p.create_multitopic::<RawBytes>(
            "MT",
            "T",
            alloc::vec::Vec::new(),
            "x > 0",
            alloc::vec::Vec::new(),
        );
        assert!(matches!(res, Err(DdsError::BadParameter { .. })));
    }

    #[test]
    fn multitopic_rejects_invalid_expression() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let res = p.create_multitopic::<RawBytes>(
            "MT",
            "T",
            alloc::vec!["A".into()],
            "x === bogus",
            alloc::vec::Vec::new(),
        );
        assert!(matches!(res, Err(DdsError::BadParameter { .. })));
    }

    #[test]
    fn multitopic_rejects_param_index_out_of_range() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        // Expression referenziert %1, params hat aber nur 1 Eintrag.
        let res = p.create_multitopic::<RawBytes>(
            "MT",
            "T",
            alloc::vec!["A".into()],
            "x = %1",
            alloc::vec!["only_zero".into()],
        );
        assert!(matches!(res, Err(DdsError::BadParameter { .. })));
    }

    #[test]
    fn multitopic_set_params_validates_index_range() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let mt = p
            .create_multitopic::<RawBytes>(
                "MT",
                "T",
                alloc::vec!["A".into()],
                "x = %0 OR y = %1",
                alloc::vec!["a".into(), "b".into()],
            )
            .unwrap();
        // Set mit zu wenig Params → BadParameter.
        let res = mt.set_expression_parameters(alloc::vec!["only_zero".into()]);
        assert!(matches!(res, Err(DdsError::BadParameter { .. })));
    }

    #[test]
    fn multitopic_clone_shares_params() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let mt = p
            .create_multitopic::<RawBytes>(
                "MT",
                "T",
                alloc::vec!["A".into()],
                "v = %0",
                alloc::vec!["init".into()],
            )
            .unwrap();
        let mt2 = mt.clone();
        // Update auf einer Klone-Instanz reflektiert sich auf der anderen
        // (shared Arc<RwLock<FilterParams>>).
        mt.set_expression_parameters(alloc::vec!["updated".into()])
            .unwrap();
        assert_eq!(
            mt2.get_expression_parameters(),
            alloc::vec!["updated".to_string()]
        );
    }

    // -------- MultiTopic Hash-Join (§2.2.2.3.4-r-cross-topic-join) --------

    struct OrderRow {
        id: i64,
        amount: i64,
    }
    impl RowAccess for OrderRow {
        fn get(&self, p: &str) -> Option<Value> {
            match p {
                "id" => Some(Value::Int(self.id)),
                "amount" => Some(Value::Int(self.amount)),
                _ => None,
            }
        }
    }

    struct CustomerRow {
        id: i64,
        country: String,
    }
    impl RowAccess for CustomerRow {
        fn get(&self, p: &str) -> Option<Value> {
            match p {
                "id" => Some(Value::Int(self.id)),
                "country" => Some(Value::String(self.country.clone())),
                _ => None,
            }
        }
    }

    #[test]
    fn joined_row_dispatches_dotted_paths_by_topic_prefix() {
        let o = OrderRow { id: 7, amount: 100 };
        let c = CustomerRow {
            id: 7,
            country: "DE".into(),
        };
        let row = JoinedRow::new(alloc::vec![
            ("Order".into(), &o as &dyn RowAccess),
            ("Customer".into(), &c as &dyn RowAccess),
        ]);
        assert_eq!(row.get("Order.amount"), Some(Value::Int(100)));
        assert_eq!(
            row.get("Customer.country"),
            Some(Value::String("DE".into()))
        );
        assert_eq!(row.get("Order.country"), None); // praefix matched -> kein Field
    }

    #[test]
    fn joined_row_undotted_falls_back_to_first_match() {
        let o = OrderRow { id: 7, amount: 100 };
        let c = CustomerRow {
            id: 9,
            country: "DE".into(),
        };
        let row = JoinedRow::new(alloc::vec![
            ("Order".into(), &o as &dyn RowAccess),
            ("Customer".into(), &c as &dyn RowAccess),
        ]);
        // "country" gibts nur in CustomerRow → wird gefunden via Fallback.
        assert_eq!(row.get("country"), Some(Value::String("DE".into())));
        // "amount" gibt es nur in OrderRow.
        assert_eq!(row.get("amount"), Some(Value::Int(100)));
    }

    #[test]
    fn multitopic_evaluate_joined_uses_dotted_paths() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(50, DomainParticipantQos::default());
        let mt = p
            .create_multitopic::<RawBytes>(
                "Sales",
                "Sale",
                alloc::vec!["Order".into(), "Customer".into()],
                "Order.id = Customer.id AND Customer.country = %0",
                alloc::vec!["DE".into()],
            )
            .unwrap();
        let o = OrderRow { id: 1, amount: 50 };
        let c = CustomerRow {
            id: 1,
            country: "DE".into(),
        };
        let row = JoinedRow::new(alloc::vec![
            ("Order".into(), &o as &dyn RowAccess),
            ("Customer".into(), &c as &dyn RowAccess),
        ]);
        assert!(mt.evaluate_joined(&row).unwrap());

        let c_us = CustomerRow {
            id: 1,
            country: "US".into(),
        };
        let row2 = JoinedRow::new(alloc::vec![
            ("Order".into(), &o as &dyn RowAccess),
            ("Customer".into(), &c_us as &dyn RowAccess),
        ]);
        assert!(!mt.evaluate_joined(&row2).unwrap());
    }

    #[test]
    fn hash_join_two_combines_matching_rows() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(51, DomainParticipantQos::default());
        let mt = p
            .create_multitopic::<RawBytes>(
                "Sales",
                "Sale",
                alloc::vec!["Order".into(), "Customer".into()],
                "Customer.country = %0",
                alloc::vec!["DE".into()],
            )
            .unwrap();
        let orders = alloc::vec![
            OrderRow { id: 1, amount: 50 },
            OrderRow { id: 2, amount: 70 },
            OrderRow { id: 3, amount: 90 },
        ];
        let customers = alloc::vec![
            CustomerRow {
                id: 1,
                country: "DE".into(),
            },
            CustomerRow {
                id: 2,
                country: "US".into(),
            },
            CustomerRow {
                id: 3,
                country: "DE".into(),
            },
        ];
        let out: alloc::vec::Vec<(i64, i64, String)> = hash_join_two(
            &orders,
            "Order",
            |o| o.id.to_string(),
            &customers,
            "Customer",
            |c| c.id.to_string(),
            |o, c| (o.id, o.amount, c.country.clone()),
            |row| mt.evaluate_joined(row),
        );
        assert_eq!(out.len(), 2);
        // Erwarte ids 1 und 3 (DE), nicht 2 (US).
        assert!(out.iter().any(|(i, _, _)| *i == 1));
        assert!(out.iter().any(|(i, _, _)| *i == 3));
        assert!(out.iter().all(|(_, _, c)| c == "DE"));
    }

    #[test]
    fn hash_join_two_returns_empty_when_no_keys_match() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(52, DomainParticipantQos::default());
        let mt = p
            .create_multitopic::<RawBytes>(
                "Sales",
                "Sale",
                alloc::vec!["Order".into(), "Customer".into()],
                "Order.id = Customer.id",
                alloc::vec::Vec::new(),
            )
            .unwrap();
        let orders = alloc::vec![OrderRow { id: 1, amount: 50 }];
        let customers = alloc::vec![CustomerRow {
            id: 99,
            country: "DE".into(),
        }];
        let out: alloc::vec::Vec<i64> = hash_join_two(
            &orders,
            "Order",
            |o| o.id.to_string(),
            &customers,
            "Customer",
            |c| c.id.to_string(),
            |o, _| o.id,
            |row| mt.evaluate_joined(row),
        );
        assert!(out.is_empty());
    }

    #[test]
    fn hash_join_two_emits_cartesian_for_duplicate_keys() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(53, DomainParticipantQos::default());
        let mt = p
            .create_multitopic::<RawBytes>(
                "Sales",
                "Sale",
                alloc::vec!["Order".into(), "Customer".into()],
                "Order.id = Customer.id",
                alloc::vec::Vec::new(),
            )
            .unwrap();
        // Zwei Orders mit id=1, ein Customer mit id=1
        let orders = alloc::vec![
            OrderRow { id: 1, amount: 10 },
            OrderRow { id: 1, amount: 20 },
        ];
        let customers = alloc::vec![CustomerRow {
            id: 1,
            country: "DE".into(),
        }];
        let out: alloc::vec::Vec<i64> = hash_join_two(
            &orders,
            "Order",
            |o| o.id.to_string(),
            &customers,
            "Customer",
            |c| c.id.to_string(),
            |o, _| o.amount,
            |row| mt.evaluate_joined(row),
        );
        assert_eq!(out.len(), 2);
        assert!(out.contains(&10));
        assert!(out.contains(&20));
    }

    #[test]
    fn hash_join_two_predicate_can_filter_pairs() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(54, DomainParticipantQos::default());
        // Predicate verlangt amount > 60.
        let mt = p
            .create_multitopic::<RawBytes>(
                "Sales",
                "Sale",
                alloc::vec!["Order".into(), "Customer".into()],
                "Order.amount > 60",
                alloc::vec::Vec::new(),
            )
            .unwrap();
        let orders = alloc::vec![
            OrderRow { id: 1, amount: 50 },
            OrderRow { id: 2, amount: 70 },
        ];
        let customers = alloc::vec![
            CustomerRow {
                id: 1,
                country: "DE".into(),
            },
            CustomerRow {
                id: 2,
                country: "DE".into(),
            },
        ];
        let out: alloc::vec::Vec<i64> = hash_join_two(
            &orders,
            "Order",
            |o| o.id.to_string(),
            &customers,
            "Customer",
            |c| c.id.to_string(),
            |o, _| o.amount,
            |row| mt.evaluate_joined(row),
        );
        // Nur amount=70 darf raus.
        assert_eq!(out, alloc::vec![70]);
    }

    #[test]
    fn delete_multitopic_rejects_foreign_participant() {
        let p1 = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let p2 = DomainParticipantFactory::instance()
            .create_participant_offline(1, DomainParticipantQos::default());
        let mt = p1
            .create_multitopic::<RawBytes>(
                "MT",
                "T",
                alloc::vec!["A".into()],
                "x > 0",
                alloc::vec::Vec::new(),
            )
            .unwrap();
        let res = p2.delete_multitopic(&mt);
        assert!(matches!(res, Err(DdsError::BadParameter { .. })));
    }

    #[test]
    fn topic_description_get_participant_returns_owning_participant() {
        // get_participant() liefert genau den Participant, an dem
        // create_topic aufgerufen wurde — kein anderer.
        let p1 = DomainParticipantFactory::instance()
            .create_participant_offline(11, DomainParticipantQos::default());
        let p2 = DomainParticipantFactory::instance()
            .create_participant_offline(12, DomainParticipantQos::default());
        let t = p1
            .create_topic::<RawBytes>("Owned", TopicQos::default())
            .unwrap();
        let td: &dyn TopicDescription = &t;
        assert_eq!(td.get_participant().domain_id(), 11);
        assert_ne!(td.get_participant().domain_id(), p2.domain_id());
    }

    // -------- ContentFilteredTopic --------

    use alloc::collections::BTreeMap;
    use zerodds_sql_filter::{RowAccess, Value};

    struct MapRow(BTreeMap<String, Value>);
    impl RowAccess for MapRow {
        fn get(&self, path: &str) -> Option<Value> {
            self.0.get(path).cloned()
        }
    }

    fn row(pairs: &[(&str, Value)]) -> MapRow {
        let mut m = BTreeMap::new();
        for (k, v) in pairs {
            m.insert((*k).into(), v.clone());
        }
        MapRow(m)
    }

    fn mk_p(domain: i32) -> DomainParticipant {
        DomainParticipantFactory::instance()
            .create_participant_offline(domain, DomainParticipantQos::default())
    }

    #[test]
    fn cft_compiles_and_evaluates_filter() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("Chatter", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic("ChatterFilt", &topic, "x > 10", alloc::vec::Vec::new())
            .unwrap();
        // CFT trait works.
        let td: &dyn TopicDescription = &cft;
        assert_eq!(td.get_name(), "ChatterFilt");
        assert_eq!(td.get_type_name(), RawBytes::TYPE_NAME);

        // Filter: x > 10
        let r_yes = row(&[("x", Value::Int(20))]);
        let r_no = row(&[("x", Value::Int(5))]);
        assert_eq!(cft.evaluate(&r_yes), Ok(true));
        assert_eq!(cft.evaluate(&r_no), Ok(false));
    }

    #[test]
    fn cft_with_params_can_be_updated() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic("Filt", &topic, "color = %0", alloc::vec!["RED".into()])
            .unwrap();
        assert_eq!(cft.get_filter_expression(), "color = %0");
        assert_eq!(cft.get_filter_parameters(), alloc::vec!["RED".to_string()]);

        let r = row(&[("color", Value::String("RED".into()))]);
        assert_eq!(cft.evaluate(&r), Ok(true));

        // Parameter updaten.
        cft.set_filter_parameters(alloc::vec!["BLUE".into()])
            .unwrap();
        assert_eq!(cft.evaluate(&r), Ok(false));
    }

    #[test]
    fn cft_get_related_topic() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("Base", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic("CF", &topic, "x = 1", alloc::vec::Vec::new())
            .unwrap();
        assert_eq!(cft.get_related_topic().name(), "Base");
    }

    #[test]
    fn cft_invalid_expression_rejected() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        let err = p
            .create_contentfilteredtopic("CF", &topic, "x === bogus", alloc::vec::Vec::new())
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn cft_param_index_out_of_range_rejected() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        // Expression nutzt %0 + %1, aber wir liefern nur einen.
        let err = p
            .create_contentfilteredtopic("CF", &topic, "x = %0 AND y = %1", alloc::vec!["1".into()])
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn cft_set_filter_parameters_validates_count() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic(
                "CF",
                &topic,
                "x = %0 AND y = %1",
                alloc::vec!["1".into(), "2".into()],
            )
            .unwrap();
        let err = cft
            .set_filter_parameters(alloc::vec!["1".into()])
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn cft_filter_with_string_param() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        // Strings: Caller liefert sie ohne ''-Anfuehrungszeichen
        // (Spec-Beispiele); die Wert-Konvertierung interpretiert sie
        // als String-Default.
        let cft = p
            .create_contentfilteredtopic("CF", &topic, "name LIKE %0", alloc::vec!["foo%".into()])
            .unwrap();
        let r = row(&[("name", Value::String("foobar".into()))]);
        assert_eq!(cft.evaluate(&r), Ok(true));
    }

    #[test]
    fn cft_filter_with_or_and_combination() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic(
                "CF",
                &topic,
                "(x > 10 AND x < 100) OR color = 'RED'",
                alloc::vec::Vec::new(),
            )
            .unwrap();
        // x in range, color irrelevant.
        let r1 = row(&[
            ("x", Value::Int(50)),
            ("color", Value::String("BLUE".into())),
        ]);
        assert_eq!(cft.evaluate(&r1), Ok(true));
        // x out, color RED.
        let r2 = row(&[("x", Value::Int(5)), ("color", Value::String("RED".into()))]);
        assert_eq!(cft.evaluate(&r2), Ok(true));
        // both fail.
        let r3 = row(&[
            ("x", Value::Int(5)),
            ("color", Value::String("BLUE".into())),
        ]);
        assert_eq!(cft.evaluate(&r3), Ok(false));
    }

    #[test]
    fn cft_unknown_field_returns_bad_parameter() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic("CF", &topic, "missing = 1", alloc::vec::Vec::new())
            .unwrap();
        let r = row(&[("x", Value::Int(1))]);
        let err = cft.evaluate(&r).unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn cft_clone_shares_params() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic("CF", &topic, "color = %0", alloc::vec!["RED".into()])
            .unwrap();
        let cft2 = cft.clone();
        // Update via cft → sichtbar in cft2 (Arc<RwLock>-shared).
        cft.set_filter_parameters(alloc::vec!["BLUE".into()])
            .unwrap();
        assert_eq!(
            cft2.get_filter_parameters(),
            alloc::vec!["BLUE".to_string()]
        );
    }

    #[test]
    fn param_string_to_value_heuristics() {
        assert_eq!(super::param_string_to_value("42"), Value::Int(42));
        assert_eq!(super::param_string_to_value("2.5"), Value::Float(2.5));
        assert_eq!(super::param_string_to_value("TRUE"), Value::Bool(true));
        assert_eq!(super::param_string_to_value("False"), Value::Bool(false));
        assert_eq!(
            super::param_string_to_value("'hello'"),
            Value::String("hello".into())
        );
        assert_eq!(
            super::param_string_to_value("plain"),
            Value::String("plain".into())
        );
    }
}
