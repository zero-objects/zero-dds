// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-Vendor-Interop-Matrix-Renderer.
//!
//! Crate `zerodds-interop-matrix`. Safety classification: **COMFORT**.
//!
//! Reads JSON result files (output of `ci/jobs/interop-matrix.yml`),
//! renders a static HTML page that by vendor × DDS profile
//! organized. Colour code per cell:
//!
//! * `pass` → green
//! * `partial` → yellow
//! * `fail` → red
//! * `na` (not applicable) → gray
//! * otherwise → unknown (orange)
//!
//! ## JSON-Format
//!
//! ```json
//! {
//!   "generated_at": "2026-05-03T10:00:00Z",
//!   "git_sha": "abcdef0",
//!   "profiles": ["rtps_pubsub", "security_auth", "xtypes_struct", "xml_wire"],
//!   "vendors": [
//!     {
//!       "name": "Cyclone DDS",
//!       "version": "0.10.5",
//!       "results": {
//!         "rtps_pubsub": {"status": "pass", "note": "10000 samples / 60s"},
//!         "security_auth": {"status": "pass"},
//!         "xtypes_struct": {"status": "partial", "note": "Mutable-Type fail"},
//!         "xml_wire": {"status": "na"}
//!       }
//!     }
//!   ]
//! }
//! ```

#![warn(missing_docs)]
#![allow(clippy::print_stdout, clippy::print_stderr)]

pub mod model;
pub mod parser;
pub mod render;

pub use model::{Cell, Matrix, Status, VendorRow};
pub use parser::{ParseError, parse_matrix_json};
pub use render::render_html;
