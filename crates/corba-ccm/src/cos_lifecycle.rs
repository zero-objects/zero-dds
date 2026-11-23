// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `CosLifeCycle::FactoryFinder` with the operation CORBA 3.3 Part 3
//! §6.13.2.1 adds to it for components:
//!
//! ```text
//! module CosLifeCycle {
//!     interface FactoryFinder {
//!         Factories find_factories(in Key factory_key) raises (NoFactory);
//!         Factory   find_factory(in Key factory_key)   raises (NoFactory);
//!     };
//! };
//! ```
//!
//! The `factory_key` is a name in the stringified form of the
//! Interoperable Naming Service; it is the input of
//! `Components::HomeFinder::find_home_by_name`. The factory is the default
//! factory operation of the home found — the parameter-less `create`
//! (`KeylessCCMHome::create_component`, §6.8.1.1, §6.8.7) — as a
//! `CosLifeCycle::GenericFactory`. A home with a primary key has no
//! parameter-less factory (§6.8.1.2) and is no factory for a key.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use zerodds_corba_cosnaming::{Name, name_to_string};
use zerodds_corba_rt::CorbaAny;

use crate::home::{ComponentHomeFinder, ComponentRef, HomeDef, HomeFinder};

/// `CosLifeCycle::Key` — a `CosNaming::Name`.
pub type Key = Name;

/// `CosLifeCycle::NameValuePair`.
#[derive(Debug, Clone, PartialEq)]
pub struct NameValuePair {
    /// `name` (`CosNaming::Istring`).
    pub name: String,
    /// `value`.
    pub value: CorbaAny,
}

/// `CosLifeCycle::Criteria`.
pub type Criteria = Vec<NameValuePair>;

/// The repository id of `CosLifeCycle::FactoryFinder`.
pub const FACTORY_FINDER_ID: &str = "IDL:omg.org/CosLifeCycle/FactoryFinder:1.0";
/// The repository id of `CosLifeCycle::GenericFactory`.
pub const GENERIC_FACTORY_ID: &str = "IDL:omg.org/CosLifeCycle/GenericFactory:1.0";
/// The repository id of `CosLifeCycle::NoFactory`.
pub const NO_FACTORY_ID: &str = "IDL:omg.org/CosLifeCycle/NoFactory:1.0";
/// The repository id of `CosLifeCycle::InvalidCriteria`.
pub const INVALID_CRITERIA_ID: &str = "IDL:omg.org/CosLifeCycle/InvalidCriteria:1.0";

/// `CosLifeCycle::NoFactory`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoFactory {
    /// The key no factory was found for.
    pub search_key: Key,
}

/// What `GenericFactory::create_object` raises.
#[derive(Debug, Clone, PartialEq)]
pub enum CreateError {
    /// `NoFactory`: the factory does not create objects for this key.
    NoFactory(NoFactory),
    /// `InvalidCriteria` with the criteria the factory does not take.
    InvalidCriteria(Criteria),
}

/// `CosLifeCycle::GenericFactory`.
pub trait GenericFactory {
    /// `supports(k)`: `true` when the factory creates objects for `k`.
    fn supports(&self, k: &Key) -> bool;

    /// `create_object(k, the_criteria)`.
    ///
    /// # Errors
    /// [`CreateError`].
    fn create_object(&self, k: &Key, the_criteria: &Criteria) -> Result<ComponentRef, CreateError>;
}

/// The default factory of a keyless home as a `GenericFactory`
/// (§6.13.2.1): `create_object` is the home's parameter-less `create`.
#[derive(Debug, Clone)]
pub struct HomeFactory {
    key: Key,
    home: HomeDef,
    instances: Arc<HomeFinder>,
}

impl HomeFactory {
    /// The home whose default factory this is.
    #[must_use]
    pub fn home(&self) -> &HomeDef {
        &self.home
    }
}

impl GenericFactory for HomeFactory {
    /// The key the factory was found by.
    fn supports(&self, k: &Key) -> bool {
        *k == self.key
    }

    /// A new instance of the managed component type. The default factory
    /// has no parameters, so it takes no criteria.
    fn create_object(&self, k: &Key, the_criteria: &Criteria) -> Result<ComponentRef, CreateError> {
        if !self.supports(k) {
            return Err(CreateError::NoFactory(NoFactory {
                search_key: k.clone(),
            }));
        }
        if !the_criteria.is_empty() {
            return Err(CreateError::InvalidCriteria(the_criteria.clone()));
        }
        Ok((*self
            .instances
            .create(&self.home.managed_component_id, Vec::new()))
        .clone())
    }
}

/// `CosLifeCycle::FactoryFinder` over the homes of a
/// [`ComponentHomeFinder`] (§6.13.2.1); the components the factories
/// create go into `instances`.
#[derive(Debug)]
pub struct FactoryFinder<'a> {
    homes: &'a ComponentHomeFinder,
    instances: Arc<HomeFinder>,
}

impl<'a> FactoryFinder<'a> {
    /// A finder over `homes`.
    #[must_use]
    pub fn new(homes: &'a ComponentHomeFinder, instances: Arc<HomeFinder>) -> Self {
        Self { homes, instances }
    }

    /// `find_factories(factory_key)`: the default factories of every
    /// keyless home bound to the stringified `factory_key`.
    ///
    /// # Errors
    /// [`NoFactory`] when the key is no valid name or no keyless home is
    /// bound to it.
    pub fn find_factories(&self, factory_key: &Key) -> Result<Vec<HomeFactory>, NoFactory> {
        let no_factory = || NoFactory {
            search_key: factory_key.clone(),
        };
        let name = name_to_string(factory_key).map_err(|_| no_factory())?;
        let found: Vec<HomeFactory> = self
            .homes
            .homes_by_name(&name)
            .into_iter()
            .filter(|home| home.primary_key_id.is_none())
            .map(|home| HomeFactory {
                key: factory_key.clone(),
                home: home.clone(),
                instances: Arc::clone(&self.instances),
            })
            .collect();
        if found.is_empty() {
            Err(no_factory())
        } else {
            Ok(found)
        }
    }

    /// `find_factory(factory_key)` (§6.13.2.1): the stringified key goes
    /// to `HomeFinder::find_home_by_name`; the default factory of that
    /// home is the factory.
    ///
    /// # Errors
    /// [`NoFactory`] when the key is no valid name, no home is bound to
    /// it, or the home has a primary key.
    pub fn find_factory(&self, factory_key: &Key) -> Result<HomeFactory, NoFactory> {
        let no_factory = || NoFactory {
            search_key: factory_key.clone(),
        };
        let name = name_to_string(factory_key).map_err(|_| no_factory())?;
        let home = self
            .homes
            .find_home_by_name(&name)
            .map_err(|_| no_factory())?;
        if home.primary_key_id.is_some() {
            return Err(no_factory());
        }
        Ok(HomeFactory {
            key: factory_key.clone(),
            home: home.clone(),
            instances: Arc::clone(&self.instances),
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_corba_cosnaming::{NameComponent, string_to_name};

    fn home(name: &str, keyed: bool) -> HomeDef {
        HomeDef {
            name: name.into(),
            repository_id: alloc::format!("IDL:demo/{name}:1.0"),
            managed_component_id: alloc::format!("IDL:demo/{name}Component:1.0"),
            primary_key_id: keyed.then(|| "IDL:demo/Key:1.0".into()),
        }
    }

    fn finder() -> ComponentHomeFinder {
        let mut f = ComponentHomeFinder::new();
        f.register(home("Trader", false), Some("Markets/Trader.home".into()));
        f.register(home("Keyed", true), Some("Markets/Keyed.home".into()));
        f.register(home("Spare", false), Some("Markets/Trader.home".into()));
        f.register(home("Hidden", false), None);
        f
    }

    #[test]
    fn corba_p3_6_13_2_1_find_factory_goes_through_the_home_finder() {
        let homes = finder();
        let instances = Arc::new(HomeFinder::new());
        let ff = FactoryFinder::new(&homes, Arc::clone(&instances));
        // The key in stringified form is the name the home is bound to.
        let key = alloc::vec![
            NameComponent::new("Markets"),
            NameComponent::with_kind("Trader", "home"),
        ];
        assert_eq!(key, string_to_name("Markets/Trader.home").unwrap());
        let factory = ff.find_factory(&key).unwrap();
        assert_eq!(
            factory.home(),
            homes.find_home_by_name("Markets/Trader.home").unwrap()
        );
        assert!(factory.supports(&key));
        let created = factory.create_object(&key, &Vec::new()).unwrap();
        assert_eq!(created.repository_id, "IDL:demo/TraderComponent:1.0");
        assert_eq!(instances.instance_count(), 1);
    }

    #[test]
    fn corba_p3_6_13_2_1_no_factory_without_a_keyless_home_bound_to_the_key() {
        let homes = finder();
        let ff = FactoryFinder::new(&homes, Arc::new(HomeFinder::new()));
        for written in ["Markets/Keyed.home", "Markets/Unknown", "Hidden"] {
            let key = string_to_name(written).unwrap();
            assert_eq!(
                ff.find_factory(&key).unwrap_err(),
                NoFactory {
                    search_key: key.clone()
                },
                "{written}"
            );
            assert!(ff.find_factories(&key).is_err(), "{written}");
        }
        // An empty key is no name.
        assert!(ff.find_factory(&Vec::new()).is_err());
    }

    #[test]
    fn corba_p3_6_13_2_1_find_factories_returns_every_keyless_home_of_the_name() {
        let homes = finder();
        let ff = FactoryFinder::new(&homes, Arc::new(HomeFinder::new()));
        let key = string_to_name("Markets/Trader.home").unwrap();
        let names: Vec<String> = ff
            .find_factories(&key)
            .unwrap()
            .iter()
            .map(|f| f.home().name.clone())
            .collect();
        assert_eq!(
            names,
            alloc::vec!["Trader".to_string(), "Spare".to_string()]
        );
    }

    #[test]
    fn corba_p3_6_13_2_1_the_default_factory_takes_no_criteria_and_no_other_key() {
        let homes = finder();
        let ff = FactoryFinder::new(&homes, Arc::new(HomeFinder::new()));
        let key = string_to_name("Markets/Trader.home").unwrap();
        let factory = ff.find_factory(&key).unwrap();
        let other = string_to_name("Markets/Other").unwrap();
        assert!(!factory.supports(&other));
        assert_eq!(
            factory.create_object(&other, &Vec::new()).unwrap_err(),
            CreateError::NoFactory(NoFactory { search_key: other })
        );
        let criteria = alloc::vec![NameValuePair {
            name: "region".into(),
            value: CorbaAny(zerodds_corba_rt::AnyValue::Str("EU".into())),
        }];
        assert_eq!(
            factory.create_object(&key, &criteria).unwrap_err(),
            CreateError::InvalidCriteria(criteria)
        );
    }
}
