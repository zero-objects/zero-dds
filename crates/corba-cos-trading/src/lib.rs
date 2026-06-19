// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-corba-cos-trading`. Safety classification: **STANDARD**.
//!
//! OMG **Trading Object Service** (`CosTrading`) — pure-Rust `no_std + alloc`,
//! `forbid(unsafe_code)`. Service discovery via a **constraint language**:
//! providers export typed *offers* (service type + properties + object ref) and
//! consumers look them up with boolean constraint expressions over the
//! properties (e.g. `"Speed > 100 and Color == 'red'"`) plus optional
//! preferences for result ordering.
//!
//! Implements:
//! - **`Value`** — typed property values ([`property`]).
//! - **Constraint language** — tokenizer + recursive-descent parser + evaluator
//!   over the OMG trading constraint grammar subset ([`constraint`]).
//! - **`Trader`** — register (`export`/`withdraw`) + lookup (`query` with
//!   constraint + preferences) over an in-memory offer DB ([`trader`]).
//! - **`ServiceTypeRepository`** — service-type inheritance graph with
//!   subtype matching + mandatory check ([`service_type`]).
//! - **`DynamicPropEval`** — dynamic properties computed per query
//!   ([`dynamic`]).
//! - **`Federation`** — trader federation with `Link`/`Proxy`, query forwarding
//!   across trader boundaries ([`federation`]).
//!
//! Spec: OMG Trading Object Service. Part of the "optional profiles as
//! differentiation" strategy (subordinate to the OTS).

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

extern crate alloc;

pub mod constraint;
pub mod dynamic;
pub mod federation;
pub mod property;
pub mod service_type;
pub mod trader;

pub use constraint::{Constraint, ConstraintError};
pub use dynamic::DynamicPropEval;
pub use federation::{Federation, FederationError, FollowOption, Link, ProxyOffer};
pub use property::Value;
pub use service_type::{PropertyDecl, PropertyMode, ServiceType, ServiceTypeRepository, TypeError};
pub use trader::{ExportError, Offer, OfferId, Preference, Trader};
