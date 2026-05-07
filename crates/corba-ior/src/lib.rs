// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG CORBA 3.3 Part 2 §13.6 — Interoperable Object Reference (IOR).
//!
//! Crate `zerodds-corba-ior`. Safety classification: **STANDARD**.
//! Spec OMG CORBA 3.3 Part 2 §13.6.
//!
//! # Scope
//!
//! Voller IOR-Stack:
//!
//! * **IOR-Struct** (Spec §13.6.2): `string type_id` +
//!   `sequence<TaggedProfile> profiles`.
//! * **TaggedProfile** mit allen Standard-Profile-Tags (Spec §13.6.7.1)
//!   plus IIOP-Profile-Body via `crates/corba-iiop/`.
//! * **TaggedComponent** mit ueber 30 Standard-Component-Tags (Spec
//!   §13.6.7.3) und strukturierten Decodern fuer die wichtigsten:
//!   ORB_TYPE / CODE_SETS / ALTERNATE_IIOP_ADDRESS / SSL_SEC_TRANS /
//!   TLS_SEC_TRANS / RMI_CUSTOM_MAX_STREAM_FORMAT / JAVA_CODEBASE.
//! * **stringified-IOR** (Spec §13.6.10): `IOR:`-Prefix + Hex-
//!   Encoding einer CDR-Encapsulation. Bidirektional encode/decode.
//! * **corbaloc:** und **corbaname:** URL-Parser (Spec §13.6.10
//!   ueber das Naming-Service-Submapping).
//!
//! ## Beispiel
//!
//! ```
//! use zerodds_corba_ior::{Ior, ProfileId};
//! let ior = Ior::default();
//! assert!(ior.profiles.is_empty());
//! assert_eq!(ProfileId::InternetIop.as_u32(), 0);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod component_tags;
pub mod components;
pub mod error;
pub mod ior;
pub mod profile_tags;
pub mod stringified;
pub mod tagged_profile;
pub mod url;

pub use component_tags::ComponentId;
pub use components::{
    AlternateIiopAddress, CodeSetComponent, CodeSetComponentInfo, OrbType, Ssl,
    StreamFormatVersion, StructuredComponent, TaggedComponent, TlsSecTrans,
};
pub use error::{IorError, IorResult};
pub use ior::Ior;
pub use profile_tags::ProfileId;
pub use stringified::{STRINGIFIED_IOR_PREFIX, from_stringified, to_stringified};
pub use tagged_profile::TaggedProfile;
pub use url::{CorbalocAddress, CorbanameAddress, parse_corbaloc, parse_corbaname};
