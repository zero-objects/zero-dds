// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CIF — Component Implementation Framework (CORBA 3.3 Part 3 §9; CCM
//! 4.0 §8).
//!
//! Four executor modes (spec §8.1.4):
//!
//! | Mode | Usage | Lifecycle |
//! |---|---|---|
//! | Session | one-instance-per-session | session-scoped |
//! | Service | stateless | per-request |
//! | Process | long-running per-process | process-scoped |
//! | Entity | persistent | persistent |
//!
//! The component executor implements the component method logic;
//! the home executor manages lifecycle calls (`ccm_activate` etc.).
//!
//! Part 3 §9.3.2 adds the two implementation strategies: a home's
//! `create` returns a monolithic executor or an [`ExecutorLocator`]
//! ([`ComponentImplementation`]); for a locator the container obtains
//! the executor of a port before each invocation and releases it after
//! ([`ComponentImplementation::invoke`]). §9.2.9.1 and §9.2.11 add
//! segmented executors and the information a component reference
//! carries: [`SegmentLayout`] allocates the facet and segment
//! identifiers, [`ComponentReferenceInfo`] is the monolithic or segmented
//! reference information with its encoding into an object key, and
//! [`SegmentedComponent`] activates only the segment that provides the
//! facet a request targets.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::Any;

use crate::cidl::CompositionCategory;
use crate::component_def::ComponentDef;
use crate::context::ComponentContext;

/// Common executor trait (all four modes).
pub trait ComponentExecutor: Send + Sync {
    /// `set_session_context` / `set_service_context` etc. — the
    /// container injects the context before the first method call
    /// (spec §8.1.5).
    fn set_context(&mut self, context: Box<dyn ComponentContext>);

    /// `ccm_activate` — spec §8.1.5.4.
    fn ccm_activate(&mut self) -> Result<(), CifError> {
        Ok(())
    }

    /// `ccm_passivate` — spec §8.1.5.5.
    fn ccm_passivate(&mut self) -> Result<(), CifError> {
        Ok(())
    }

    /// `ccm_remove` — spec §8.1.5.6.
    fn ccm_remove(&mut self) -> Result<(), CifError> {
        Ok(())
    }
}

/// CIF-specific errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CifError {
    /// `CCMException` — generic error from the executor.
    CcmException(String),
    /// `Components::InvalidConfiguration` from `configuration_complete`
    /// (§9.3.2).
    InvalidConfiguration(String),
    /// A segmentation the CIF refuses (§9.2.9.1).
    InvalidSegmentation(String),
    /// Reference information that does not decode or names no facet or
    /// segment of the component (§9.2.11).
    InvalidReference(String),
}

/// Session executor — spec §8.1.4.1.
pub trait SessionExecutor: ComponentExecutor {
    /// Marker method indicating the executor is in session mode.
    fn session_marker(&self) {}
}

/// Keyed executor — spec §8.1.4.4 (Entity).
pub trait KeyedExecutor: ComponentExecutor {
    /// Returns the primary key of the entity executor.
    fn primary_key(&self) -> Vec<u8>;
}

/// A local object an executor is handed out as (Part 3 §9.3.2:
/// `obtain_executor` returns `Object` "because there is yet no IDL type
/// for the common base of all local objects").
pub type LocalExecutor = Arc<dyn Any + Send + Sync>;

/// `Components::ExecutorLocator` — Part 3 §9.3.2.
///
/// "If a home, in creating a component, returns an ExecutorLocator, the
/// container will invoke its `obtain_executor` operation prior to each
/// invocation to retrieve the implementation for a port. The port name
/// ... is the same as used in the component's interface description in
/// IDL, or the component's name for the 'main' executor."
pub trait ExecutorLocator: Send + Sync {
    /// `obtain_executor(in string name)`.
    ///
    /// # Errors
    /// `CCMException` when no executor can be located.
    fn obtain_executor(&self, name: &str) -> Result<LocalExecutor, CifError>;

    /// `release_executor(in Object exc)`: the invocation on `executor`
    /// has finished.
    ///
    /// # Errors
    /// `CCMException`.
    fn release_executor(&self, executor: &LocalExecutor) -> Result<(), CifError>;

    /// `configuration_complete()`: propagates
    /// `CCMObject::configuration_complete` to the implementation.
    ///
    /// # Errors
    /// `InvalidConfiguration`.
    fn configuration_complete(&self) -> Result<(), CifError>;
}

/// What a home's `create` returns (§9.3.3.6.3): "either a reference to a
/// monolithic executor or to an ExecutorLocator. In the former case, the
/// container assumes that the monolithic strategy is used, otherwise it
/// will use the locator strategy."
#[derive(Clone)]
pub enum ComponentImplementation {
    /// The monolithic executor (`CCM_<component>`).
    Monolithic(LocalExecutor),
    /// The locator.
    Locator(Arc<dyn ExecutorLocator>),
}

impl core::fmt::Debug for ComponentImplementation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Monolithic(_) => "Monolithic(..)",
            Self::Locator(_) => "Locator(..)",
        })
    }
}

impl ComponentImplementation {
    /// One invocation on port `name` (or the component's name for the
    /// main executor): the monolithic executor, or the locator's executor
    /// obtained before and released after `f`.
    ///
    /// # Errors
    /// What `obtain_executor` or `release_executor` raise.
    pub fn invoke<R>(
        &self,
        name: &str,
        f: impl FnOnce(&LocalExecutor) -> R,
    ) -> Result<R, CifError> {
        match self {
            Self::Monolithic(executor) => Ok(f(executor)),
            Self::Locator(locator) => {
                let executor = locator.obtain_executor(name)?;
                let result = f(&executor);
                locator.release_executor(&executor)?;
                Ok(result)
            }
        }
    }

    /// `configuration_complete`, which for a locator is propagated to it
    /// (§9.3.2); a monolithic executor implements it through
    /// `CCMObject`.
    ///
    /// # Errors
    /// `InvalidConfiguration`.
    pub fn configuration_complete(&self) -> Result<(), CifError> {
        match self {
            Self::Monolithic(_) => Ok(()),
            Self::Locator(locator) => locator.configuration_complete(),
        }
    }
}

// ---------------------------------------------------------------------------
// Segments and reference information (§9.2.9.1, §9.2.11)
// ---------------------------------------------------------------------------

/// A segment of an executor as CIDL declares it: its name and the facets
/// it provides (§9.2.9.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentDef {
    /// The segment's name.
    pub name: String,
    /// The facets of the component the segment provides.
    pub facets: Vec<String>,
}

/// The facet and segment identifiers the CIF allocates (§9.2.11.1,
/// §9.2.11.2): facet 0 is the component itself, the facets of the
/// component follow in declaration order from 1; segment 0 is the
/// component segment, which "supplies all of the facets not provided by
/// separate segments", the declared segments follow from 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentLayout {
    facets: Vec<(String, u32, u16)>,
    segments: Vec<(u16, String)>,
}

impl SegmentLayout {
    /// The identifier of the component segment.
    pub const COMPONENT_SEGMENT: u16 = 0;
    /// The facet identifier of the component itself.
    pub const COMPONENT_FACET: u32 = 0;

    /// The layout of `component` with `segments`.
    ///
    /// # Errors
    /// [`CifError::InvalidSegmentation`]: segments in a composition that is
    /// neither `entity` nor `process` ("The life cycle category of the
    /// composition must be entity or process if the executor specifies
    /// segmentation"), a segment without a facet ("Each segment must
    /// provide at least one facet"), a facet the component does not have,
    /// a facet in two segments, two segments of one name.
    pub fn new(
        category: CompositionCategory,
        component: &ComponentDef,
        segments: &[SegmentDef],
    ) -> Result<Self, CifError> {
        let invalid = |s: String| Err(CifError::InvalidSegmentation(s));
        if !segments.is_empty()
            && !matches!(
                category,
                CompositionCategory::Entity | CompositionCategory::Process
            )
        {
            return invalid(format!("a {category:?} composition cannot have segments"));
        }
        let mut owner: BTreeMap<&str, u16> = BTreeMap::new();
        let mut names: Vec<(u16, String)> =
            alloc::vec![(Self::COMPONENT_SEGMENT, component.name.clone())];
        for (index, segment) in segments.iter().enumerate() {
            let id = u16::try_from(index + 1)
                .map_err(|_| CifError::InvalidSegmentation("too many segments".to_string()))?;
            if names.iter().skip(1).any(|(_, n)| *n == segment.name) {
                return invalid(format!("segment `{}` is declared twice", segment.name));
            }
            if segment.facets.is_empty() {
                return invalid(format!("segment `{}` provides no facet", segment.name));
            }
            for facet in &segment.facets {
                if component.provide_facet(facet).is_none() {
                    return invalid(format!("`{facet}` is no facet of `{}`", component.name));
                }
                if owner.insert(facet.as_str(), id).is_some() {
                    return invalid(format!("facet `{facet}` is in two segments"));
                }
            }
            names.push((id, segment.name.clone()));
        }
        let mut facets = Vec::with_capacity(component.facets.len());
        for (index, facet) in component.facets.iter().enumerate() {
            let id = u32::try_from(index + 1)
                .map_err(|_| CifError::InvalidSegmentation("too many facets".to_string()))?;
            let segment = owner
                .get(facet.name.as_str())
                .copied()
                .unwrap_or(Self::COMPONENT_SEGMENT);
            facets.push((facet.name.clone(), id, segment));
        }
        Ok(Self {
            facets,
            segments: names,
        })
    }

    /// Whether the executor is segmented.
    #[must_use]
    pub fn is_segmented(&self) -> bool {
        self.segments.len() > 1
    }

    /// The identifier of facet `name`.
    #[must_use]
    pub fn facet_id(&self, name: &str) -> Option<u32> {
        self.facets.iter().find(|f| f.0 == name).map(|f| f.1)
    }

    /// The segment that provides facet `name`.
    #[must_use]
    pub fn segment_of(&self, name: &str) -> Option<u16> {
        self.facets.iter().find(|f| f.0 == name).map(|f| f.2)
    }

    /// The segment that provides the facet `facet_id` (the component
    /// segment for facet 0).
    #[must_use]
    pub fn segment_of_facet_id(&self, facet_id: u32) -> Option<u16> {
        if facet_id == Self::COMPONENT_FACET {
            return Some(Self::COMPONENT_SEGMENT);
        }
        self.facets.iter().find(|f| f.1 == facet_id).map(|f| f.2)
    }

    /// The segment identifiers with their names, the component segment
    /// first: the symbolic constants of the generated skeletons
    /// (§9.2.11.2).
    #[must_use]
    pub fn segments(&self) -> &[(u16, String)] {
        &self.segments
    }
}

/// One element of the segment descriptors of segmented reference
/// information (§9.2.11.5): a segment and the state identifier of its
/// incarnation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentDescriptor {
    /// The segment.
    pub segment_id: u16,
    /// The state identifier (`StateIdValue` as bytes, §9.2.11.3).
    pub state_id: Vec<u8>,
}

/// The information model of a component object reference (§9.2.11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentReferenceInfo {
    /// §9.2.11.4: a facet identifier and a single state identifier.
    Monolithic {
        /// The target facet (0 for the component).
        facet_id: u32,
        /// The state identifier.
        state_id: Vec<u8>,
    },
    /// §9.2.11.5: the target facet, the segment implementing it and a
    /// descriptor for every segment, the component segment included.
    Segmented {
        /// The target facet.
        facet_id: u32,
        /// The segment the facet is implemented on.
        segment_id: u16,
        /// One descriptor per segment.
        segments: Vec<SegmentDescriptor>,
    },
}

const REFERENCE_MAGIC: &[u8; 4] = b"ZCIF";

fn take<'a>(input: &mut &'a [u8], n: usize) -> Result<&'a [u8], CifError> {
    if input.len() < n {
        return Err(CifError::InvalidReference(
            "the reference information is truncated".to_string(),
        ));
    }
    let (head, rest) = input.split_at(n);
    *input = rest;
    Ok(head)
}

fn take_u16(input: &mut &[u8]) -> Result<u16, CifError> {
    let b = take(input, 2)?;
    Ok(u16::from_be_bytes([b[0], b[1]]))
}

fn take_u32(input: &mut &[u8]) -> Result<u32, CifError> {
    let b = take(input, 4)?;
    Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

impl ComponentReferenceInfo {
    /// §9.2.11.6: the state identifier of the component segment (or the
    /// single one of a monolithic executor), the identity of the
    /// component within its home.
    #[must_use]
    pub fn component_identity(&self) -> &[u8] {
        match self {
            Self::Monolithic { state_id, .. } => state_id,
            Self::Segmented { segments, .. } => segments
                .iter()
                .find(|d| d.segment_id == SegmentLayout::COMPONENT_SEGMENT)
                .map_or(&[], |d| d.state_id.as_slice()),
        }
    }

    /// §9.2.11.6: "Equivalence of component identity is defined as
    /// equivalence of state identifier values of the component segment."
    #[must_use]
    pub fn same_component(&self, other: &Self) -> bool {
        self.component_identity() == other.component_identity()
    }

    /// The target facet.
    #[must_use]
    pub fn facet_id(&self) -> u32 {
        match self {
            Self::Monolithic { facet_id, .. } | Self::Segmented { facet_id, .. } => *facet_id,
        }
    }

    /// The encoding into an `object_key` (§9.2.11: "no standard encoding
    /// within an object_key is specified"): `ZCIF`, a form octet (0
    /// monolithic, 1 segmented), the facet id, then the state id or the
    /// target segment and the descriptors; integers big-endian, byte
    /// strings with a 32-bit length.
    ///
    /// # Errors
    /// [`CifError::InvalidReference`] for a state id or a descriptor list
    /// longer than its length field.
    pub fn encode(&self) -> Result<Vec<u8>, CifError> {
        let length = |b: &[u8]| {
            u32::try_from(b.len())
                .map_err(|_| CifError::InvalidReference("a state id is too long".to_string()))
        };
        let mut out = Vec::new();
        out.extend_from_slice(REFERENCE_MAGIC);
        match self {
            Self::Monolithic { facet_id, state_id } => {
                out.push(0);
                out.extend_from_slice(&facet_id.to_be_bytes());
                out.extend_from_slice(&length(state_id)?.to_be_bytes());
                out.extend_from_slice(state_id);
            }
            Self::Segmented {
                facet_id,
                segment_id,
                segments,
            } => {
                out.push(1);
                out.extend_from_slice(&facet_id.to_be_bytes());
                out.extend_from_slice(&segment_id.to_be_bytes());
                let count = u16::try_from(segments.len()).map_err(|_| {
                    CifError::InvalidReference("too many segment descriptors".to_string())
                })?;
                out.extend_from_slice(&count.to_be_bytes());
                for d in segments {
                    out.extend_from_slice(&d.segment_id.to_be_bytes());
                    out.extend_from_slice(&length(&d.state_id)?.to_be_bytes());
                    out.extend_from_slice(&d.state_id);
                }
            }
        }
        Ok(out)
    }

    /// The reverse of [`Self::encode`].
    ///
    /// # Errors
    /// [`CifError::InvalidReference`] for another magic, an unknown form,
    /// a truncated key, trailing octets, or segmented information without
    /// the component segment or with the target segment missing.
    pub fn decode(key: &[u8]) -> Result<Self, CifError> {
        let mut input = key;
        if take(&mut input, 4)? != REFERENCE_MAGIC {
            return Err(CifError::InvalidReference(
                "not a component reference".to_string(),
            ));
        }
        let form = take(&mut input, 1)?[0];
        let facet_id = take_u32(&mut input)?;
        let info = match form {
            0 => {
                let n = take_u32(&mut input)? as usize;
                Self::Monolithic {
                    facet_id,
                    state_id: take(&mut input, n)?.to_vec(),
                }
            }
            1 => {
                let segment_id = take_u16(&mut input)?;
                let count = take_u16(&mut input)?;
                let mut segments = Vec::with_capacity(usize::from(count).min(input.len() / 6));
                for _ in 0..count {
                    let id = take_u16(&mut input)?;
                    let n = take_u32(&mut input)? as usize;
                    segments.push(SegmentDescriptor {
                        segment_id: id,
                        state_id: take(&mut input, n)?.to_vec(),
                    });
                }
                let has = |id: u16| segments.iter().any(|d| d.segment_id == id);
                if !has(SegmentLayout::COMPONENT_SEGMENT) || !has(segment_id) {
                    return Err(CifError::InvalidReference(
                        "the descriptors lack the component or the target segment".to_string(),
                    ));
                }
                Self::Segmented {
                    facet_id,
                    segment_id,
                    segments,
                }
            }
            other => {
                return Err(CifError::InvalidReference(format!(
                    "unknown reference form {other}"
                )));
            }
        };
        if !input.is_empty() {
            return Err(CifError::InvalidReference(
                "octets after the reference information".to_string(),
            ));
        }
        Ok(info)
    }
}

/// `create_executor_segment` of a home executor (§9.2.6, Example 1;
/// §9.2.11.2): the component implementor interprets the segment
/// identifier and returns the segment's executor.
pub trait SegmentFactory: Send + Sync {
    /// The executor of segment `segment_id` incarnating the state
    /// `state_id`.
    ///
    /// # Errors
    /// `CCMException`.
    fn create_executor_segment(
        &self,
        segment_id: u16,
        state_id: &[u8],
    ) -> Result<LocalExecutor, CifError>;
}

/// A component with a segmented executor: "Segments are independently
/// activated. When the container receives a request whose target is a
/// facet of a segmented executor, the container activates only the
/// segment that provides the required facet." (§9.2.9.1)
#[cfg(feature = "std")]
pub struct SegmentedComponent {
    layout: Arc<SegmentLayout>,
    factory: Arc<dyn SegmentFactory>,
    descriptors: Vec<SegmentDescriptor>,
    active: std::sync::Mutex<BTreeMap<u16, LocalExecutor>>,
}

#[cfg(feature = "std")]
impl core::fmt::Debug for SegmentedComponent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SegmentedComponent")
            .field("descriptors", &self.descriptors)
            .finish()
    }
}

#[cfg(feature = "std")]
impl SegmentedComponent {
    /// A component whose segments incarnate the states `state_ids` (one
    /// per segment of the layout, in its order).
    ///
    /// # Errors
    /// [`CifError::InvalidSegmentation`] when the number of state ids is
    /// not the number of segments.
    pub fn new(
        layout: Arc<SegmentLayout>,
        factory: Arc<dyn SegmentFactory>,
        state_ids: Vec<Vec<u8>>,
    ) -> Result<Self, CifError> {
        if state_ids.len() != layout.segments().len() {
            return Err(CifError::InvalidSegmentation(format!(
                "{} state ids for {} segments",
                state_ids.len(),
                layout.segments().len()
            )));
        }
        let descriptors = layout
            .segments()
            .iter()
            .zip(state_ids)
            .map(|((segment_id, _), state_id)| SegmentDescriptor {
                segment_id: *segment_id,
                state_id,
            })
            .collect();
        Ok(Self {
            layout,
            factory,
            descriptors,
            active: std::sync::Mutex::new(BTreeMap::new()),
        })
    }

    /// The reference information of the component (`None`) or of one of
    /// its facets.
    ///
    /// # Errors
    /// [`CifError::InvalidReference`] for a name that is no facet.
    pub fn reference(&self, facet: Option<&str>) -> Result<ComponentReferenceInfo, CifError> {
        let (facet_id, segment_id) = match facet {
            None => (
                SegmentLayout::COMPONENT_FACET,
                SegmentLayout::COMPONENT_SEGMENT,
            ),
            Some(name) => (
                self.layout
                    .facet_id(name)
                    .ok_or_else(|| CifError::InvalidReference(format!("no facet `{name}`")))?,
                self.layout
                    .segment_of(name)
                    .ok_or_else(|| CifError::InvalidReference(format!("no facet `{name}`")))?,
            ),
        };
        Ok(ComponentReferenceInfo::Segmented {
            facet_id,
            segment_id,
            segments: self.descriptors.clone(),
        })
    }

    /// The executor a request with `info` targets: its segment, activated
    /// through the factory when it is not active yet. No other segment is
    /// activated.
    ///
    /// # Errors
    /// [`CifError::InvalidReference`] for information of another
    /// component, a monolithic one, or a facet the segment does not
    /// provide; what the factory raises.
    pub fn dispatch(&self, info: &ComponentReferenceInfo) -> Result<LocalExecutor, CifError> {
        let ComponentReferenceInfo::Segmented {
            facet_id,
            segment_id,
            segments,
        } = info
        else {
            return Err(CifError::InvalidReference(
                "monolithic information for a segmented executor".to_string(),
            ));
        };
        if *segments != self.descriptors {
            return Err(CifError::InvalidReference(
                "the reference denotes another component".to_string(),
            ));
        }
        if self.layout.segment_of_facet_id(*facet_id) != Some(*segment_id) {
            return Err(CifError::InvalidReference(format!(
                "segment {segment_id} does not provide facet {facet_id}"
            )));
        }
        let mut active = self
            .active
            .lock()
            .map_err(|_| CifError::CcmException("segment table poisoned".to_string()))?;
        if let Some(executor) = active.get(segment_id) {
            return Ok(Arc::clone(executor));
        }
        let state = self
            .descriptors
            .iter()
            .find(|d| d.segment_id == *segment_id)
            .map(|d| d.state_id.clone())
            .unwrap_or_default();
        let executor = self.factory.create_executor_segment(*segment_id, &state)?;
        active.insert(*segment_id, Arc::clone(&executor));
        Ok(executor)
    }

    /// The segments active now.
    #[must_use]
    pub fn active_segments(&self) -> Vec<u16> {
        self.active
            .lock()
            .map(|a| a.keys().copied().collect())
            .unwrap_or_default()
    }

    /// Passivates one segment; the next request to it activates it again.
    pub fn passivate(&self, segment_id: u16) -> bool {
        self.active
            .lock()
            .map(|mut a| a.remove(&segment_id).is_some())
            .unwrap_or(false)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};

    struct LoggingContext {
        log: alloc::sync::Arc<AtomicUsize>,
    }

    impl ComponentContext for LoggingContext {
        fn get_caller_principal(&self) -> Option<alloc::vec::Vec<u8>> {
            self.log.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    struct DemoExecutor {
        ctx: Option<Box<dyn ComponentContext>>,
        activations: AtomicUsize,
        passivations: AtomicUsize,
    }

    impl ComponentExecutor for DemoExecutor {
        fn set_context(&mut self, c: Box<dyn ComponentContext>) {
            self.ctx = Some(c);
        }
        fn ccm_activate(&mut self) -> Result<(), CifError> {
            self.activations.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        fn ccm_passivate(&mut self) -> Result<(), CifError> {
            self.passivations.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    impl SessionExecutor for DemoExecutor {}

    #[test]
    fn lifecycle_calls_increment_counters() {
        let mut e = DemoExecutor {
            ctx: None,
            activations: AtomicUsize::new(0),
            passivations: AtomicUsize::new(0),
        };
        e.set_context(Box::new(LoggingContext {
            log: alloc::sync::Arc::new(AtomicUsize::new(0)),
        }));
        assert!(e.ctx.is_some());
        e.ccm_activate().unwrap();
        e.ccm_activate().unwrap();
        e.ccm_passivate().unwrap();
        assert_eq!(e.activations.load(Ordering::Relaxed), 2);
        assert_eq!(e.passivations.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn cif_error_carries_diagnostic() {
        let e = CifError::CcmException("permission denied".into());
        match e {
            CifError::CcmException(s) => assert_eq!(s, "permission denied"),
            other => panic!("{other:?}"),
        }
    }

    // -- Part 3 §9.3.2, §9.2.9.1, §9.2.11 ------------------------------------

    use crate::component_def::FacetDef;

    struct Locator {
        log: std::sync::Mutex<alloc::vec::Vec<String>>,
        fail_complete: bool,
    }

    impl ExecutorLocator for Locator {
        fn obtain_executor(&self, name: &str) -> Result<LocalExecutor, CifError> {
            if name == "missing" {
                return Err(CifError::CcmException("no executor".into()));
            }
            self.log
                .lock()
                .unwrap()
                .push(alloc::format!("obtain {name}"));
            Ok(Arc::new(String::from(name)))
        }
        fn release_executor(&self, executor: &LocalExecutor) -> Result<(), CifError> {
            let name = executor
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_default();
            self.log
                .lock()
                .unwrap()
                .push(alloc::format!("release {name}"));
            Ok(())
        }
        fn configuration_complete(&self) -> Result<(), CifError> {
            if self.fail_complete {
                return Err(CifError::InvalidConfiguration("unconnected".into()));
            }
            self.log.lock().unwrap().push("complete".into());
            Ok(())
        }
    }

    #[test]
    fn corba_p3_9_3_2_a_locator_is_asked_before_and_released_after_each_invocation() {
        let locator = Arc::new(Locator {
            log: std::sync::Mutex::new(alloc::vec::Vec::new()),
            fail_complete: false,
        });
        let implementation = ComponentImplementation::Locator(locator.clone());
        let seen = implementation
            .invoke("tweety", |e| {
                locator.log.lock().unwrap().push("invoke".into());
                e.downcast_ref::<String>().cloned()
            })
            .unwrap();
        assert_eq!(seen.as_deref(), Some("tweety"));
        // The main executor is obtained by the component's name.
        implementation.invoke("Toon", |_| ()).unwrap();
        implementation.configuration_complete().unwrap();
        assert_eq!(
            *locator.log.lock().unwrap(),
            [
                "obtain tweety",
                "invoke",
                "release tweety",
                "obtain Toon",
                "release Toon",
                "complete"
            ]
        );
        assert!(matches!(
            implementation.invoke("missing", |_| ()),
            Err(CifError::CcmException(_))
        ));
        let failing = ComponentImplementation::Locator(Arc::new(Locator {
            log: std::sync::Mutex::new(alloc::vec::Vec::new()),
            fail_complete: true,
        }));
        assert!(matches!(
            failing.configuration_complete(),
            Err(CifError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn corba_p3_9_3_2_a_monolithic_executor_serves_every_port() {
        let executor: LocalExecutor = Arc::new(7_u32);
        let implementation = ComponentImplementation::Monolithic(Arc::clone(&executor));
        for port in ["tweety", "sylvester", "Toon"] {
            let same = implementation
                .invoke(port, |e| Arc::ptr_eq(e, &executor))
                .unwrap();
            assert!(same, "{port}");
        }
        implementation.configuration_complete().unwrap();
    }

    /// Example 1 of §9.2.6: `component Toon { provides Bird tweety;
    /// provides Cat sylvester; }`.
    fn toon() -> ComponentDef {
        ComponentDef {
            name: "Toon".into(),
            repository_id: "IDL:LooneyToons/Toon:1.0".into(),
            facets: alloc::vec![
                FacetDef {
                    name: "tweety".into(),
                    interface_id: "IDL:LooneyToons/Bird:1.0".into(),
                },
                FacetDef {
                    name: "sylvester".into(),
                    interface_id: "IDL:LooneyToons/Cat:1.0".into(),
                },
            ],
            ..ComponentDef::default()
        }
    }

    fn bird_segment() -> alloc::vec::Vec<SegmentDef> {
        alloc::vec![SegmentDef {
            name: "BirdSegment".into(),
            facets: alloc::vec!["tweety".into()],
        }]
    }

    #[test]
    fn corba_p3_9_2_9_1_segments_get_ids_and_the_component_segment_keeps_the_rest() {
        let layout =
            SegmentLayout::new(CompositionCategory::Entity, &toon(), &bird_segment()).unwrap();
        assert!(layout.is_segmented());
        assert_eq!(
            layout.segments(),
            [(0, "Toon".to_string()), (1, "BirdSegment".to_string())]
        );
        assert_eq!(layout.facet_id("tweety"), Some(1));
        assert_eq!(layout.facet_id("sylvester"), Some(2));
        assert_eq!(layout.segment_of("tweety"), Some(1));
        assert_eq!(layout.segment_of("sylvester"), Some(0));
        assert_eq!(layout.segment_of_facet_id(0), Some(0));
        assert_eq!(layout.segment_of_facet_id(9), None);
        // Monolithic: every facet on the component segment.
        let mono = SegmentLayout::new(CompositionCategory::Session, &toon(), &[]).unwrap();
        assert!(!mono.is_segmented());
        assert_eq!(mono.segment_of("tweety"), Some(0));
        // The rules of §9.2.9.1.
        let refused = |category, segments: alloc::vec::Vec<SegmentDef>| {
            matches!(
                SegmentLayout::new(category, &toon(), &segments),
                Err(CifError::InvalidSegmentation(_))
            )
        };
        assert!(refused(CompositionCategory::Session, bird_segment()));
        assert!(refused(CompositionCategory::Service, bird_segment()));
        assert!(!refused(CompositionCategory::Process, bird_segment()));
        let segment = |name: &str, facets: &[&str]| SegmentDef {
            name: name.into(),
            facets: facets.iter().map(|f| (*f).to_string()).collect(),
        };
        assert!(refused(
            CompositionCategory::Entity,
            alloc::vec![segment("Empty", &[])]
        ));
        assert!(refused(
            CompositionCategory::Entity,
            alloc::vec![segment("A", &["tweety"]), segment("B", &["tweety"])]
        ));
        assert!(refused(
            CompositionCategory::Entity,
            alloc::vec![segment("A", &["nemo"])]
        ));
        assert!(refused(
            CompositionCategory::Entity,
            alloc::vec![segment("A", &["tweety"]), segment("A", &["sylvester"])]
        ));
    }

    #[test]
    fn corba_p3_9_2_11_reference_information_round_trips_and_names_the_component() {
        let mono = ComponentReferenceInfo::Monolithic {
            facet_id: 2,
            state_id: b"P0".to_vec(),
        };
        let key = mono.encode().unwrap();
        assert_eq!(&key[..4], b"ZCIF");
        assert_eq!(ComponentReferenceInfo::decode(&key).unwrap(), mono);
        // Figure 9.4: facet B on segment S1, descriptors (0, P0), (S1, P1).
        let segmented = ComponentReferenceInfo::Segmented {
            facet_id: 2,
            segment_id: 1,
            segments: alloc::vec![
                SegmentDescriptor {
                    segment_id: 0,
                    state_id: b"P0".to_vec(),
                },
                SegmentDescriptor {
                    segment_id: 1,
                    state_id: b"P1".to_vec(),
                },
            ],
        };
        let key = segmented.encode().unwrap();
        assert_eq!(ComponentReferenceInfo::decode(&key).unwrap(), segmented);
        // §9.2.11.6: the component segment's state id is the identity.
        assert_eq!(segmented.component_identity(), b"P0");
        assert!(segmented.same_component(&mono));
        let other = ComponentReferenceInfo::Monolithic {
            facet_id: 2,
            state_id: b"P9".to_vec(),
        };
        assert!(!segmented.same_component(&other));
        // Malformed keys.
        for bad in [
            &key[..key.len() - 1],
            &[b"XCIF".as_slice(), &key[4..]].concat()[..],
            &[key.as_slice(), &[0]].concat()[..],
        ] {
            assert!(matches!(
                ComponentReferenceInfo::decode(bad),
                Err(CifError::InvalidReference(_))
            ));
        }
        let mut wrong_form = key.clone();
        wrong_form[4] = 7;
        assert!(ComponentReferenceInfo::decode(&wrong_form).is_err());
        let without_component = ComponentReferenceInfo::Segmented {
            facet_id: 1,
            segment_id: 1,
            segments: alloc::vec![SegmentDescriptor {
                segment_id: 1,
                state_id: b"P1".to_vec(),
            }],
        };
        assert!(ComponentReferenceInfo::decode(&without_component.encode().unwrap()).is_err());
    }

    struct Segments {
        created: std::sync::Mutex<alloc::vec::Vec<(u16, alloc::vec::Vec<u8>)>>,
    }

    impl SegmentFactory for Segments {
        fn create_executor_segment(
            &self,
            segment_id: u16,
            state_id: &[u8],
        ) -> Result<LocalExecutor, CifError> {
            self.created
                .lock()
                .unwrap()
                .push((segment_id, state_id.to_vec()));
            Ok(Arc::new(segment_id))
        }
    }

    #[test]
    fn corba_p3_9_2_9_1_a_request_activates_only_the_segment_of_its_facet() {
        let layout = Arc::new(
            SegmentLayout::new(CompositionCategory::Entity, &toon(), &bird_segment()).unwrap(),
        );
        let factory = Arc::new(Segments {
            created: std::sync::Mutex::new(alloc::vec::Vec::new()),
        });
        let toon = SegmentedComponent::new(
            Arc::clone(&layout),
            factory.clone(),
            alloc::vec![b"P0".to_vec(), b"P1".to_vec()],
        )
        .unwrap();
        let tweety = toon.reference(Some("tweety")).unwrap();
        // The reference goes through the object key and back.
        let key = tweety.encode().unwrap();
        let decoded = ComponentReferenceInfo::decode(&key).unwrap();
        let executor = toon.dispatch(&decoded).unwrap();
        assert_eq!(executor.downcast_ref::<u16>(), Some(&1));
        assert_eq!(toon.active_segments(), [1]);
        assert_eq!(*factory.created.lock().unwrap(), [(1, b"P1".to_vec())]);
        // A second request to the same segment does not create it again.
        toon.dispatch(&decoded).unwrap();
        assert_eq!(factory.created.lock().unwrap().len(), 1);
        // sylvester lives on the component segment.
        let sylvester = toon.reference(Some("sylvester")).unwrap();
        assert_eq!(
            toon.dispatch(&sylvester).unwrap().downcast_ref::<u16>(),
            Some(&0)
        );
        assert_eq!(toon.active_segments(), [0, 1]);
        assert!(toon.passivate(1));
        assert_eq!(toon.active_segments(), [0]);
        // The component reference and the facet references name one
        // component.
        assert!(toon.reference(None).unwrap().same_component(&tweety));
        // A facet on the wrong segment, another component, an unknown
        // facet, a monolithic reference.
        let ComponentReferenceInfo::Segmented { segments, .. } = tweety.clone() else {
            panic!("segmented");
        };
        let wrong_segment = ComponentReferenceInfo::Segmented {
            facet_id: 1,
            segment_id: 0,
            segments: segments.clone(),
        };
        assert!(toon.dispatch(&wrong_segment).is_err());
        let mut other = segments;
        other[0].state_id = b"Q0".to_vec();
        assert!(
            toon.dispatch(&ComponentReferenceInfo::Segmented {
                facet_id: 1,
                segment_id: 1,
                segments: other,
            })
            .is_err()
        );
        assert!(toon.reference(Some("nemo")).is_err());
        assert!(
            toon.dispatch(&ComponentReferenceInfo::Monolithic {
                facet_id: 1,
                state_id: b"P0".to_vec(),
            })
            .is_err()
        );
        assert!(SegmentedComponent::new(layout, factory, alloc::vec![b"P0".to_vec()]).is_err());
    }
}
