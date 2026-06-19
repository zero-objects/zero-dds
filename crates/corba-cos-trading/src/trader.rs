// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `Trader` — register (`export`/`withdraw`) + lookup (`query`) over an
//! in-memory offer DB (CosTrading `Register` + `Lookup`).

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::constraint::{Constraint, ConstraintError};
use crate::dynamic::DynamicPropEval;
use crate::property::Value;
use crate::service_type::ServiceTypeRepository;

/// Unique offer identifier (assigned by the trader).
pub type OfferId = u64;

/// An exported service offer: service type + typed properties + object reference
/// (opaque IOR bytes).
#[derive(Debug, Clone, PartialEq)]
pub struct Offer {
    /// Service-type name (e.g. `"Printer"`).
    pub service_type: String,
    /// Typed (static) properties that constraints match against.
    pub properties: BTreeMap<String, Value>,
    /// Names of the properties computed dynamically per query
    /// (via the [`DynamicPropEval`] registered on the trader).
    pub dynamic_props: Vec<String>,
    /// The provider's object reference (opaque IOR bytes).
    pub ior: Vec<u8>,
}

impl Offer {
    /// New offer with service type + IOR bytes, without properties.
    #[must_use]
    pub fn new(service_type: impl Into<String>, ior: Vec<u8>) -> Self {
        Self {
            service_type: service_type.into(),
            properties: BTreeMap::new(),
            dynamic_props: Vec::new(),
            ior,
        }
    }

    /// Builder: sets a static property.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: Value) -> Self {
        self.properties.insert(name.into(), value);
        self
    }

    /// Builder: marks a property as dynamic (computed per query).
    #[must_use]
    pub fn with_dynamic(mut self, name: impl Into<String>) -> Self {
        self.dynamic_props.push(name.into());
        self
    }
}

/// Sort preference for `query` results (CosTrading `Policies`/preferences).
#[derive(Debug, Clone)]
pub enum Preference {
    /// First hits in export order.
    First,
    /// Descending by numeric property (largest first).
    Max(String),
    /// Ascending by numeric property (smallest first).
    Min(String),
}

/// Error from a checked export against a [`ServiceTypeRepository`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError {
    /// The offer's service type is not registered in the repository.
    UnknownType(String),
    /// A *mandatory* property of the service type is missing from the offer.
    MissingMandatory(String),
}

/// The trader (CosTrading `Register` + `Lookup`) over an in-memory DB.
///
/// Optionally bound to a [`ServiceTypeRepository`] (subtype matching + mandatory
/// check) and a [`DynamicPropEval`] (dynamic properties).
#[derive(Debug, Default)]
pub struct Trader {
    offers: BTreeMap<OfferId, Offer>,
    next_id: OfferId,
    type_repo: Option<ServiceTypeRepository>,
    dynamic: Option<Arc<dyn DynamicPropEval>>,
}

impl Trader {
    /// Empty trader.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds a [`ServiceTypeRepository`] for subtype matching + mandatory check.
    #[must_use]
    pub fn with_type_repository(mut self, repo: ServiceTypeRepository) -> Self {
        self.type_repo = Some(repo);
        self
    }

    /// Sets the service-type repository (also after the fact).
    pub fn set_type_repository(&mut self, repo: ServiceTypeRepository) {
        self.type_repo = Some(repo);
    }

    /// Registers an evaluator for dynamic properties.
    pub fn set_dynamic_evaluator(&mut self, eval: Arc<dyn DynamicPropEval>) {
        self.dynamic = Some(eval);
    }

    /// `Register::export` — adds an offer, returns its [`OfferId`].
    pub fn export(&mut self, offer: Offer) -> OfferId {
        let id = self.next_id;
        self.next_id += 1;
        self.offers.insert(id, offer);
        id
    }

    /// `Register::export` with validation against the service-type repository:
    /// the type must be registered and all *mandatory* properties (static or
    /// dynamic) must be present. Without a bound repository, behaves like
    /// [`export`](Self::export).
    ///
    /// # Errors
    /// [`ExportError::UnknownType`] or [`ExportError::MissingMandatory`].
    pub fn export_checked(&mut self, offer: Offer) -> Result<OfferId, ExportError> {
        if let Some(repo) = &self.type_repo {
            if !repo.contains(&offer.service_type) {
                return Err(ExportError::UnknownType(offer.service_type.clone()));
            }
            for mand in repo.mandatory_properties(&offer.service_type) {
                let present =
                    offer.properties.contains_key(&mand) || offer.dynamic_props.contains(&mand);
                if !present {
                    return Err(ExportError::MissingMandatory(mand));
                }
            }
        }
        Ok(self.export(offer))
    }

    /// `Register::withdraw` — removes an offer; `true` if it existed.
    pub fn withdraw(&mut self, id: OfferId) -> bool {
        self.offers.remove(&id).is_some()
    }

    /// Number of exported offers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.offers.len()
    }

    /// Whether no offers are exported.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.offers.is_empty()
    }

    /// `Lookup::query` — all offers of `service_type` that match `constraint`,
    /// sorted by `preference`, limited to `how_many` (0 = unlimited).
    ///
    /// # Errors
    /// [`ConstraintError`] if `constraint` cannot be parsed.
    pub fn query(
        &self,
        service_type: &str,
        constraint: &str,
        preference: &Preference,
        how_many: usize,
    ) -> Result<Vec<(OfferId, &Offer)>, ConstraintError> {
        let c = Constraint::parse(constraint)?;
        // BTreeMap iterates by ascending OfferId = export order. For each hit we
        // keep the effective (static + dynamically resolved) properties so that
        // preferences also see dynamic values.
        let mut hits: Vec<(OfferId, &Offer, BTreeMap<String, Value>)> = self
            .offers
            .iter()
            .filter(|(_, o)| self.type_matches(&o.service_type, service_type))
            .filter_map(|(id, o)| {
                let eff = self.effective_props(o);
                c.eval(&eff).then_some((*id, o, eff))
            })
            .collect();

        match preference {
            Preference::First => {}
            Preference::Max(prop) => {
                hits.sort_by(|a, b| {
                    let av =
                        a.2.get(prop)
                            .and_then(Value::as_f64)
                            .unwrap_or(f64::NEG_INFINITY);
                    let bv =
                        b.2.get(prop)
                            .and_then(Value::as_f64)
                            .unwrap_or(f64::NEG_INFINITY);
                    bv.partial_cmp(&av).unwrap_or(core::cmp::Ordering::Equal)
                });
            }
            Preference::Min(prop) => {
                hits.sort_by(|a, b| {
                    let av =
                        a.2.get(prop)
                            .and_then(Value::as_f64)
                            .unwrap_or(f64::INFINITY);
                    let bv =
                        b.2.get(prop)
                            .and_then(Value::as_f64)
                            .unwrap_or(f64::INFINITY);
                    av.partial_cmp(&bv).unwrap_or(core::cmp::Ordering::Equal)
                });
            }
        }

        if how_many > 0 && hits.len() > how_many {
            hits.truncate(how_many);
        }
        Ok(hits.into_iter().map(|(id, o, _)| (id, o)).collect())
    }

    /// Whether an offer type satisfies the query-type requirement: exactly equal
    /// or — with a bound repository — a (transitive) subtype.
    fn type_matches(&self, offer_type: &str, query_type: &str) -> bool {
        offer_type == query_type
            || self
                .type_repo
                .as_ref()
                .is_some_and(|r| r.is_a(offer_type, query_type))
    }

    /// Effective properties of an offer: static plus the dynamic ones resolved
    /// via the registered [`DynamicPropEval`].
    fn effective_props(&self, offer: &Offer) -> BTreeMap<String, Value> {
        let mut eff = offer.properties.clone();
        if let Some(ev) = &self.dynamic {
            for name in &offer.dynamic_props {
                if let Some(v) = ev.evaluate(&offer.service_type, name, &offer.properties) {
                    eff.insert(name.clone(), v);
                }
            }
        }
        eff
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::dynamic::DynamicPropEval;
    use crate::service_type::{PropertyMode, ServiceType, ServiceTypeRepository};

    fn numeric(offer: &Offer, prop: &str) -> Option<f64> {
        offer.properties.get(prop).and_then(Value::as_f64)
    }

    fn seed() -> Trader {
        let mut t = Trader::new();
        t.export(
            Offer::new("Printer", alloc::vec![1])
                .with("Speed", Value::Int(20))
                .with("Color", Value::Bool(true)),
        );
        t.export(
            Offer::new("Printer", alloc::vec![2])
                .with("Speed", Value::Int(60))
                .with("Color", Value::Bool(false)),
        );
        t.export(
            Offer::new("Printer", alloc::vec![3])
                .with("Speed", Value::Int(40))
                .with("Color", Value::Bool(true)),
        );
        t.export(Offer::new("Scanner", alloc::vec![9]).with("Dpi", Value::Int(600)));
        t
    }

    #[test]
    fn query_filters_by_type_and_constraint() {
        let t = seed();
        let r = t
            .query("Printer", "Color == TRUE", &Preference::First, 0)
            .unwrap();
        let iors: Vec<&[u8]> = r.iter().map(|(_, o)| o.ior.as_slice()).collect();
        assert_eq!(iors, alloc::vec![&[1u8][..], &[3u8][..]]); // color printers only
    }

    #[test]
    fn query_respects_service_type() {
        let t = seed();
        let r = t.query("Scanner", "", &Preference::First, 0).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1.ior, alloc::vec![9]);
    }

    #[test]
    fn preference_max_orders_descending() {
        let t = seed();
        let r = t
            .query("Printer", "Speed > 0", &Preference::Max("Speed".into()), 0)
            .unwrap();
        let speeds: Vec<f64> = r
            .iter()
            .map(|(_, o)| numeric(o, "Speed").unwrap())
            .collect();
        assert_eq!(speeds, alloc::vec![60.0, 40.0, 20.0]);
    }

    #[test]
    fn preference_min_and_how_many() {
        let t = seed();
        let r = t
            .query("Printer", "", &Preference::Min("Speed".into()), 2)
            .unwrap();
        assert_eq!(r.len(), 2);
        let speeds: Vec<f64> = r
            .iter()
            .map(|(_, o)| numeric(o, "Speed").unwrap())
            .collect();
        assert_eq!(speeds, alloc::vec![20.0, 40.0]); // smallest first, limited to 2
    }

    #[test]
    fn export_withdraw() {
        let mut t = Trader::new();
        let id = t.export(Offer::new("X", alloc::vec![0]));
        assert_eq!(t.len(), 1);
        assert!(t.withdraw(id));
        assert!(t.is_empty());
        assert!(!t.withdraw(id)); // already gone
    }

    #[test]
    fn complex_constraint_query() {
        let t = seed();
        let r = t
            .query(
                "Printer",
                "Speed >= 40 and Color == TRUE",
                &Preference::First,
                0,
            )
            .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1.ior, alloc::vec![3]); // Speed 40, color
    }

    #[test]
    fn bad_constraint_errors() {
        let t = seed();
        assert!(
            t.query("Printer", "Speed >", &Preference::First, 0)
                .is_err()
        );
    }

    // ---- Subtype matching (service-type hierarchies) -----------------------

    fn typed_trader() -> Trader {
        let mut repo = ServiceTypeRepository::new();
        repo.add(ServiceType::new("Printer").prop("Speed", PropertyMode::Mandatory))
            .unwrap();
        repo.add(ServiceType::new("ColorPrinter").extends("Printer"))
            .unwrap();
        let mut t = Trader::new().with_type_repository(repo);
        t.export(Offer::new("Printer", alloc::vec![1]).with("Speed", Value::Int(20)));
        t.export(Offer::new("ColorPrinter", alloc::vec![2]).with("Speed", Value::Int(50)));
        t
    }

    #[test]
    fn query_on_base_type_matches_subtypes() {
        let t = typed_trader();
        // A query on "Printer" also finds the ColorPrinter offer.
        let r = t.query("Printer", "", &Preference::First, 0).unwrap();
        let iors: Vec<&[u8]> = r.iter().map(|(_, o)| o.ior.as_slice()).collect();
        assert_eq!(iors, alloc::vec![&[1u8][..], &[2u8][..]]);
        // A query on "ColorPrinter" finds only the ColorPrinter offer.
        let r2 = t.query("ColorPrinter", "", &Preference::First, 0).unwrap();
        assert_eq!(r2.len(), 1);
        assert_eq!(r2[0].1.ior, alloc::vec![2]);
    }

    #[test]
    fn export_checked_validates_mandatory_and_type() {
        let mut repo = ServiceTypeRepository::new();
        repo.add(ServiceType::new("Printer").prop("Speed", PropertyMode::Mandatory))
            .unwrap();
        let mut t = Trader::new().with_type_repository(repo);
        // Missing mandatory property → error.
        assert_eq!(
            t.export_checked(Offer::new("Printer", alloc::vec![1])),
            Err(ExportError::MissingMandatory("Speed".into()))
        );
        // Unknown type → error.
        assert_eq!(
            t.export_checked(Offer::new("Ghost", alloc::vec![1]).with("Speed", Value::Int(1))),
            Err(ExportError::UnknownType("Ghost".into()))
        );
        // Complete → ok.
        assert!(
            t.export_checked(Offer::new("Printer", alloc::vec![1]).with("Speed", Value::Int(9)))
                .is_ok()
        );
    }

    #[test]
    fn export_checked_accepts_dynamic_mandatory() {
        let mut repo = ServiceTypeRepository::new();
        repo.add(ServiceType::new("Server").prop("Load", PropertyMode::Mandatory))
            .unwrap();
        let mut t = Trader::new().with_type_repository(repo);
        // A mandatory property provided as dynamic satisfies the check.
        assert!(
            t.export_checked(Offer::new("Server", alloc::vec![1]).with_dynamic("Load"))
                .is_ok()
        );
    }

    // ---- Dynamic Properties ------------------------------------------------

    #[derive(Debug)]
    struct LoadEval;
    impl DynamicPropEval for LoadEval {
        fn evaluate(
            &self,
            _service_type: &str,
            property: &str,
            statics: &BTreeMap<String, Value>,
        ) -> Option<Value> {
            if property == "Load" {
                let base = statics
                    .get("BaseLoad")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                Some(Value::Float(base + 10.0))
            } else {
                None
            }
        }
    }

    #[test]
    fn dynamic_property_resolved_in_constraint() {
        let mut t = Trader::new();
        t.set_dynamic_evaluator(Arc::new(LoadEval));
        t.export(
            Offer::new("Server", alloc::vec![1])
                .with("BaseLoad", Value::Float(5.0))
                .with_dynamic("Load"),
        );
        t.export(
            Offer::new("Server", alloc::vec![2])
                .with("BaseLoad", Value::Float(80.0))
                .with_dynamic("Load"),
        );
        // Load = BaseLoad + 10 → offer 1: 15, offer 2: 90. Constraint Load < 50.
        let r = t
            .query("Server", "Load < 50", &Preference::First, 0)
            .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1.ior, alloc::vec![1]);
    }

    #[test]
    fn dynamic_property_used_in_preference() {
        let mut t = Trader::new();
        t.set_dynamic_evaluator(Arc::new(LoadEval));
        t.export(
            Offer::new("Server", alloc::vec![1])
                .with("BaseLoad", Value::Float(50.0))
                .with_dynamic("Load"),
        );
        t.export(
            Offer::new("Server", alloc::vec![2])
                .with("BaseLoad", Value::Float(5.0))
                .with_dynamic("Load"),
        );
        // Min(Load): smallest dynamic load first → offer 2 (15) before offer 1 (60).
        let r = t
            .query("Server", "", &Preference::Min("Load".into()), 0)
            .unwrap();
        assert_eq!(r[0].1.ior, alloc::vec![2]);
        assert_eq!(r[1].1.ior, alloc::vec![1]);
    }
}
