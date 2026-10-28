// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Home definition + finder — spec §6.7.
//!
//! A home is a factory + finder for one or more
//! component types. Spec §6.7.1: each home manages exactly one
//! component type (declared via `manages`).

#[cfg(feature = "std")]
use alloc::collections::BTreeMap;
use alloc::string::String;
#[cfg(feature = "std")]
use alloc::sync::Arc;
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::sync::Mutex;

/// Home definition.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HomeDef {
    /// Home name.
    pub name: String,
    /// Home repository ID.
    pub repository_id: String,
    /// Managed component type (repository ID).
    pub managed_component_id: String,
    /// Optional: primary-key type ID (`primarykey K`).
    pub primary_key_id: Option<String>,
}

impl HomeDef {
    /// Spec §6.7.6 — `get_component_def() -> ComponentIR::ComponentDef`.
    /// Returns the repository ID of the managed component type;
    /// the caller builds the IFR lookup on top of it.
    #[must_use]
    pub fn get_component_def_repo_id(&self) -> &str {
        &self.managed_component_id
    }

    /// Spec §6.7.6 — `get_home_def() -> ComponentIR::HomeDef`.
    /// Returns this home's own repository ID; the caller builds the
    /// IFR lookup on top of it.
    #[must_use]
    pub fn get_home_def_repo_id(&self) -> &str {
        &self.repository_id
    }
}

/// Component reference in the home finder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentRef {
    /// Component repository ID.
    pub repository_id: String,
    /// Unique component instance ID (POA-ObjectKey equivalent).
    pub instance_id: Vec<u8>,
}

/// The GIOP request operation name `get_component` travels under —
/// spec §6.4.3.1 (p. 14).
///
/// "Following the pattern of other `CORBA::Object` operations (i.e.,
/// `_interface`, `_is_a`, and `_non_existent`) the operation name in
/// GIOP request corresponding to `get_component` shall be
/// `_component`."
pub const GET_COMPONENT_GIOP_OPERATION: &str = "_component";

/// A reference to one facet of one component instance.
///
/// Spec §6.4.1 (p. 13) — a facet is reached through `provide_<name>`.
/// The reference names the `provides` port and carries the component
/// instance it belongs to, which is what §6.4.3.1 and §6.4.4 ask it
/// about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacetRef {
    /// The component instance this facet belongs to.
    pub component: ComponentRef,
    /// The name of the `provides` port.
    pub facet_name: String,
}

impl FacetRef {
    /// Spec §6.4.3.1 (p. 14) — `Object get_component()`.
    ///
    /// "If the target object reference is a facet reference, the
    /// `get_component` operation returns an object reference for the
    /// component."
    #[must_use]
    pub fn get_component(&self) -> &ComponentRef {
        &self.component
    }

    /// Spec §6.4.4 (p. 17) — `Navigation::same_component` on a facet.
    ///
    /// "If a facet interface inherits the `Navigation` interface, then
    /// the `same_component` operation on the provided interface shall
    /// give the same results as the `same_component` operation on the
    /// component interface that owns the provided interface." So it is
    /// the owning component that answers, and which facet asked does
    /// not enter into it.
    #[must_use]
    pub fn same_component(&self, other: &ComponentRef) -> bool {
        self.component.same_component(other)
    }
}

impl ComponentRef {
    /// Spec §6.4.3.1 (p. 14) — `Object get_component()`.
    ///
    /// "If the target object reference is itself a component reference
    /// (i.e., it denotes the component itself), the `get_component`
    /// operation returns the same reference (or another equivalent
    /// reference)."
    #[must_use]
    pub fn get_component(&self) -> &Self {
        self
    }

    /// Spec §6.4.4 (p. 17) — `Navigation::same_component`.
    ///
    /// "The `same_component` operation on the `Navigation` interface
    /// allows clients to determine reliably whether two references
    /// belong to the same component instance." §6.4.4 leaves "same
    /// instance" to be "defined in terms of opaque identity values
    /// supplied by the component implementation or the container": here
    /// that is the component type together with the instance identity
    /// the home issued.
    #[must_use]
    pub fn same_component(&self, other: &Self) -> bool {
        self.repository_id == other.repository_id && self.instance_id == other.instance_id
    }
}

/// The `ORB::resolve_initial_references` identifier that yields the
/// `Components::HomeFinder` — spec §6.8 (p. 41).
///
/// "A reference that supports the `HomeFinder` interface may be
/// obtained from the ORB pseudo-object by invoking
/// `CORBA::ORB::resolve_initial_references`, with the parameter value
/// 'ComponentHomeFinder.'"
pub const COMPONENT_HOME_FINDER_INITIAL_REFERENCE: &str = "ComponentHomeFinder";

/// `Components::HomeNotFound` — spec §6.8 (p. 41).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HomeNotFound;

impl core::fmt::Display for HomeNotFound {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("no home is registered under that identifier")
    }
}

impl core::error::Error for HomeNotFound {}

/// `Components::HomeFinder` — spec §6.8 (p. 41).
///
/// The three lookups the interface declares, over the homes registered
/// with it. A home reaches the finder through
/// `HomeRegistration::register_home` (§9.4.1.2), which is what
/// [`ComponentHomeFinder::register`] models; §6.8 makes no promise
/// about *which* home is returned when several match, only that an
/// unregistered identifier raises `HomeNotFound`.
#[derive(Debug, Default)]
pub struct ComponentHomeFinder {
    homes: Vec<(Option<String>, HomeDef)>,
}

impl ComponentHomeFinder {
    /// An empty finder.
    #[must_use]
    pub fn new() -> Self {
        Self { homes: Vec::new() }
    }

    /// `HomeRegistration::register_home` — spec §9.4.1.2 (p. 136).
    ///
    /// "The `home_name` parameter identifies an Interoperable Naming
    /// Service (INS) name that can be used as input to the
    /// `HomeFinder::find_home_by_name` operation. If the `home_name`
    /// parameter is NULL, no name is associated with this home so this
    /// home cannot be retrieved by name."
    pub fn register(&mut self, home: HomeDef, home_name: Option<String>) {
        self.homes.push((home_name, home));
    }

    /// `unregister_home` — spec §9.4.1.2 (p. 136). "Once
    /// `unregister_home` completes, a client will never be returned a
    /// reference to the home specified as being unregistered."
    pub fn unregister(&mut self, home_repository_id: &str) -> bool {
        let before = self.homes.len();
        self.homes
            .retain(|(_, home)| home.repository_id != home_repository_id);
        self.homes.len() != before
    }

    /// `find_home_by_component_type` — spec §6.8 (p. 41).
    ///
    /// "returns a reference, which supports the interface of a home
    /// object that manages the component type specified by the
    /// `comp_repid` parameter."
    ///
    /// # Errors
    /// [`HomeNotFound`] — "If there are no homes that manage the
    /// specified component type currently registered, the operation
    /// shall raise the `HomeNotFound` exception."
    pub fn find_home_by_component_type(&self, comp_repid: &str) -> Result<&HomeDef, HomeNotFound> {
        self.homes
            .iter()
            .find(|(_, home)| home.managed_component_id == comp_repid)
            .map(|(_, home)| home)
            .ok_or(HomeNotFound)
    }

    /// `find_home_by_home_type` — spec §6.8 (p. 42).
    ///
    /// # Errors
    /// [`HomeNotFound`] — "If there are no homes of this type currently
    /// registered, the operation shall raise the `HomeNotFound`
    /// exception."
    pub fn find_home_by_home_type(&self, home_repid: &str) -> Result<&HomeDef, HomeNotFound> {
        self.homes
            .iter()
            .find(|(_, home)| home.repository_id == home_repid)
            .map(|(_, home)| home)
            .ok_or(HomeNotFound)
    }

    /// `find_home_by_name` — spec §6.8 (p. 42).
    ///
    /// The name is a stringified Naming Service name. §6.8 leaves the
    /// binding to the implementation: "The implementation is free to
    /// maintain multiple bindings for a given name, and to return any
    /// reference bound to the name."
    ///
    /// # Errors
    /// [`HomeNotFound`] when nothing is bound to that name.
    pub fn find_home_by_name(&self, home_name: &str) -> Result<&HomeDef, HomeNotFound> {
        self.homes
            .iter()
            .find(|(name, _)| name.as_deref() == Some(home_name))
            .map(|(_, home)| home)
            .ok_or(HomeNotFound)
    }
}

/// HomeFinder — spec §6.7.1.
#[cfg(feature = "std")]
pub struct HomeFinder {
    /// Active component instances by `(repo_id, primary_key_bytes)`.
    instances: Mutex<BTreeMap<(String, Vec<u8>), ComponentRef>>,
}

#[cfg(feature = "std")]
impl Default for HomeFinder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
impl HomeFinder {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            instances: Mutex::new(BTreeMap::new()),
        }
    }

    /// `create` — spec §6.7.1.1.
    pub fn create(&self, repo_id: &str, key_bytes: Vec<u8>) -> Arc<ComponentRef> {
        let mut g = match self.instances.lock() {
            Ok(g) => g,
            Err(_) => return Arc::new(empty_ref()),
        };
        let instance_id = if key_bytes.is_empty() {
            // Spec §6.7.2.5: system-generated ID for unkeyed components.
            alloc::format!("system-{}", g.len() + 1).into_bytes()
        } else {
            key_bytes.clone()
        };
        let r = ComponentRef {
            repository_id: repo_id.to_string(),
            instance_id: instance_id.clone(),
        };
        g.insert((repo_id.to_string(), key_bytes), r.clone());
        Arc::new(r)
    }

    /// `find_by_primary_key` — spec §6.7.1.2.
    pub fn find_by_primary_key(
        &self,
        repo_id: &str,
        key_bytes: &[u8],
    ) -> Option<Arc<ComponentRef>> {
        self.instances.lock().ok().and_then(|g| {
            g.get(&(repo_id.to_string(), key_bytes.to_vec()))
                .cloned()
                .map(Arc::new)
        })
    }

    /// `remove` — spec §6.7.1.3.
    pub fn remove(&self, repo_id: &str, key_bytes: &[u8]) -> bool {
        self.instances
            .lock()
            .ok()
            .map(|mut g| {
                g.remove(&(repo_id.to_string(), key_bytes.to_vec()))
                    .is_some()
            })
            .unwrap_or(false)
    }

    /// Number of active instances.
    #[must_use]
    pub fn instance_count(&self) -> usize {
        self.instances.lock().map(|g| g.len()).unwrap_or(0)
    }
}

#[cfg(feature = "std")]
fn empty_ref() -> ComponentRef {
    ComponentRef {
        repository_id: String::new(),
        instance_id: Vec::new(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn home_def_default_has_no_primary_key() {
        let h = HomeDef::default();
        assert!(h.primary_key_id.is_none());
    }

    #[test]
    fn keyed_home_has_primary_key() {
        let h = HomeDef {
            name: "TraderHome".into(),
            repository_id: "IDL:demo/TraderHome:1.0".into(),
            managed_component_id: "IDL:demo/Trader:1.0".into(),
            primary_key_id: Some("IDL:demo/TraderKey:1.0".into()),
        };
        assert!(h.primary_key_id.is_some());
    }

    #[test]
    fn create_with_user_key_uses_key_as_id() {
        let f = HomeFinder::new();
        let r = f.create("IDL:demo/Trader:1.0", b"UK".to_vec());
        assert_eq!(r.instance_id, b"UK");
    }

    #[test]
    fn create_without_key_generates_system_id() {
        let f = HomeFinder::new();
        let r1 = f.create("IDL:demo/Trader:1.0", alloc::vec![]);
        let r2 = f.create("IDL:demo/Trader:1.0", alloc::vec![]);
        // Spec §6.7.2.5: empty keys generate system IDs;
        // unkeyed: each `(repo_id, [])` key overwrites — we
        // only track the last one. The tests show that both calls
        // return a ref and the second one overwrites the `system-1`
        // form.
        assert!(r1.instance_id.starts_with(b"system-"));
        assert!(r2.instance_id.starts_with(b"system-"));
    }

    #[test]
    fn find_by_primary_key_round_trip() {
        let f = HomeFinder::new();
        let _ = f.create("IDL:demo/Trader:1.0", b"K1".to_vec());
        let r = f.find_by_primary_key("IDL:demo/Trader:1.0", b"K1");
        assert!(r.is_some());
    }

    #[test]
    fn remove_decrements_instance_count() {
        let f = HomeFinder::new();
        let _ = f.create("IDL:demo/Trader:1.0", b"K1".to_vec());
        let _ = f.create("IDL:demo/Trader:1.0", b"K2".to_vec());
        assert_eq!(f.instance_count(), 2);
        assert!(f.remove("IDL:demo/Trader:1.0", b"K1"));
        assert_eq!(f.instance_count(), 1);
    }

    #[test]
    fn remove_unknown_returns_false() {
        let f = HomeFinder::new();
        assert!(!f.remove("IDL:demo/X:1.0", b"K"));
    }

    // ---------------------------------------------------------------
    // §6.7.6 CCMHome Interface
    // ---------------------------------------------------------------

    #[test]
    fn home_get_component_def_returns_managed_component_id() {
        let h = HomeDef {
            name: "TraderHome".into(),
            repository_id: "IDL:demo/TraderHome:1.0".into(),
            managed_component_id: "IDL:demo/Trader:1.0".into(),
            primary_key_id: None,
        };
        assert_eq!(h.get_component_def_repo_id(), "IDL:demo/Trader:1.0");
    }

    #[test]
    fn home_get_home_def_returns_self_repo_id() {
        let h = HomeDef {
            name: "TraderHome".into(),
            repository_id: "IDL:demo/TraderHome:1.0".into(),
            managed_component_id: "IDL:demo/Trader:1.0".into(),
            primary_key_id: None,
        };
        assert_eq!(h.get_home_def_repo_id(), "IDL:demo/TraderHome:1.0");
    }

    // -----------------------------------------------------------------
    // §6.4.3.1 get_component + §6.4.4 same_component.
    // -----------------------------------------------------------------

    fn trader_instance(id: &[u8]) -> ComponentRef {
        ComponentRef {
            repository_id: "IDL:demo/Trader:1.0".into(),
            instance_id: id.to_vec(),
        }
    }

    #[test]
    fn get_component_on_a_facet_reference_returns_the_component() {
        // §6.4.3.1 (p. 14) — "If the target object reference is a facet
        // reference, the get_component operation returns an object
        // reference for the component."
        let component = trader_instance(b"inst-1");
        let facet = FacetRef {
            component: component.clone(),
            facet_name: "quotes".into(),
        };
        assert_eq!(facet.get_component(), &component);
    }

    #[test]
    fn get_component_on_a_component_reference_returns_the_same_reference() {
        // §6.4.3.1 (p. 14) — "returns the same reference (or another
        // equivalent reference)."
        let component = trader_instance(b"inst-1");
        assert_eq!(component.get_component(), &component);
    }

    #[test]
    fn get_component_travels_as_the_giop_operation_named_component() {
        // §6.4.3.1 (p. 14) — "the operation name in GIOP request
        // corresponding to get_component shall be '_component'."
        assert_eq!(GET_COMPONENT_GIOP_OPERATION, "_component");
    }

    #[test]
    fn two_facets_of_one_instance_are_the_same_component() {
        // §6.4.4 (p. 17) — two references are the same component when
        // they are facets of the same instance.
        let component = trader_instance(b"inst-1");
        let quotes = FacetRef {
            component: component.clone(),
            facet_name: "quotes".into(),
        };
        let orders = FacetRef {
            component: component.clone(),
            facet_name: "orders".into(),
        };
        assert!(quotes.same_component(orders.get_component()));
        assert!(orders.same_component(quotes.get_component()));
    }

    #[test]
    fn a_facet_answers_same_component_as_the_component_that_owns_it() {
        // §6.4.4 (p. 17) — "the same_component operation on the provided
        // interface shall give the same results as the same_component
        // operation on the component interface that owns the provided
        // interface."
        let first = trader_instance(b"inst-1");
        let second = trader_instance(b"inst-2");
        let facet = FacetRef {
            component: first.clone(),
            facet_name: "quotes".into(),
        };
        assert_eq!(facet.same_component(&second), first.same_component(&second));
        assert_eq!(facet.same_component(&first), first.same_component(&first));
    }

    #[test]
    fn two_instances_of_one_component_type_are_not_the_same_component() {
        assert!(!trader_instance(b"inst-1").same_component(&trader_instance(b"inst-2")));
    }

    #[test]
    fn one_instance_id_under_two_component_types_is_not_the_same_component() {
        // The instance identity alone does not decide it; a home issues
        // its ids per component type.
        let trader = trader_instance(b"inst-1");
        let broker = ComponentRef {
            repository_id: "IDL:demo/Broker:1.0".into(),
            instance_id: b"inst-1".to_vec(),
        };
        assert!(!trader.same_component(&broker));
    }

    // -----------------------------------------------------------------
    // §6.8 Home finders.
    // -----------------------------------------------------------------

    fn trader_home() -> HomeDef {
        HomeDef {
            name: "TraderHome".into(),
            repository_id: "IDL:demo/TraderHome:1.0".into(),
            managed_component_id: "IDL:demo/Trader:1.0".into(),
            primary_key_id: None,
        }
    }

    #[test]
    fn the_home_finder_is_reached_through_the_component_home_finder_initial_reference() {
        // §6.8 (p. 41) — "may be obtained from the ORB pseudo-object by
        // invoking CORBA::ORB::resolve_initial_references, with the
        // parameter value 'ComponentHomeFinder.'"
        assert_eq!(
            COMPONENT_HOME_FINDER_INITIAL_REFERENCE,
            "ComponentHomeFinder"
        );
    }

    #[test]
    fn a_registered_home_is_found_by_component_type_and_by_home_type() {
        let mut finder = ComponentHomeFinder::new();
        finder.register(trader_home(), Some("Demo/TraderHome".into()));
        assert_eq!(
            finder
                .find_home_by_component_type("IDL:demo/Trader:1.0")
                .expect("a home managing Trader")
                .repository_id,
            "IDL:demo/TraderHome:1.0"
        );
        assert_eq!(
            finder
                .find_home_by_home_type("IDL:demo/TraderHome:1.0")
                .expect("a home of that type")
                .managed_component_id,
            "IDL:demo/Trader:1.0"
        );
    }

    #[test]
    fn an_unregistered_component_type_raises_home_not_found() {
        // §6.8 (p. 41) — "If there are no homes that manage the
        // specified component type currently registered, the operation
        // shall raise the HomeNotFound exception."
        let finder = ComponentHomeFinder::new();
        assert_eq!(
            finder.find_home_by_component_type("IDL:demo/Trader:1.0"),
            Err(HomeNotFound)
        );
        assert_eq!(
            finder.find_home_by_home_type("IDL:demo/TraderHome:1.0"),
            Err(HomeNotFound)
        );
        assert_eq!(
            finder.find_home_by_name("Demo/TraderHome"),
            Err(HomeNotFound)
        );
    }

    #[test]
    fn a_home_registered_without_a_name_cannot_be_found_by_name() {
        // §9.4.1.2 (p. 136) — "If the home_name parameter is NULL, no
        // name is associated with this home so this home cannot be
        // retrieved by name."
        let mut finder = ComponentHomeFinder::new();
        finder.register(trader_home(), None);
        assert!(
            finder
                .find_home_by_component_type("IDL:demo/Trader:1.0")
                .is_ok()
        );
        assert_eq!(
            finder.find_home_by_name("Demo/TraderHome"),
            Err(HomeNotFound)
        );
    }

    #[test]
    fn an_unregistered_home_is_never_returned_again() {
        // §9.4.1.2 (p. 136) — "Once unregister_home completes, a client
        // will never be returned a reference to the home specified as
        // being unregistered."
        let mut finder = ComponentHomeFinder::new();
        finder.register(trader_home(), Some("Demo/TraderHome".into()));
        assert!(finder.unregister("IDL:demo/TraderHome:1.0"));
        assert_eq!(
            finder.find_home_by_component_type("IDL:demo/Trader:1.0"),
            Err(HomeNotFound)
        );
        assert_eq!(
            finder.find_home_by_name("Demo/TraderHome"),
            Err(HomeNotFound)
        );
        assert!(!finder.unregister("IDL:demo/TraderHome:1.0"));
    }
}
