// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG CORBA 3.3 Part 2 §13.6 — Interoperable Object Reference (IOR).
//!
//! Crate `zerodds-corba-ior`. Safety classification: **STANDARD**.
//! Spec OMG CORBA 3.3 Part 2 §13.6.
//!
//! # Scope
//!
//! Full IOR stack:
//!
//! * **IOR struct** (spec §13.6.2): `string type_id` +
//!   `sequence<TaggedProfile> profiles`.
//! * **TaggedProfile** with all standard profile tags (spec §13.6.7.1)
//!   plus IIOP profile body via `crates/corba-iiop/`.
//! * **TaggedComponent** with over 30 standard component tags (spec
//!   §13.6.7.3) and structured decoders for the most important ones:
//!   ORB_TYPE / CODE_SETS / ALTERNATE_IIOP_ADDRESS / SSL_SEC_TRANS /
//!   TLS_SEC_TRANS / RMI_CUSTOM_MAX_STREAM_FORMAT / JAVA_CODEBASE.
//! * **stringified IOR** (spec §13.6.10): `IOR:` prefix + hex
//!   encoding of a CDR encapsulation. Bidirectional encode/decode.
//! * **corbaloc:** and **corbaname:** URL parser (spec §13.6.10
//!   over the naming-service submapping).
//!
//! ## Example
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
