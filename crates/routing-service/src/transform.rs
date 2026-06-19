// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Content filtering and field transformation.
//!
//! Filtering and transformation operate on a decoded view of the sample. The
//! byte path forwards CDR bodies verbatim; to evaluate a SQL filter or rewrite
//! fields the body must be decoded against a known **type shape** (the struct's
//! member layout) into a dynamic value, processed, and re-encoded.
//!
//! [`build_processor`] returns:
//! * `Ok(None)` — no filter and no transform → byte pass-through.
//! * `Ok(Some(_))` — a processor decoding/filtering/re-encoding via the shape.
//! * `Err(_)` — a filter/transform was requested but the type shape could not
//!   be resolved or a rule was invalid.

use crate::config::Route;
use crate::error::Result;
use crate::forwarding::SampleProcessor;

mod codec;
mod processor;
pub mod shape;
mod sqlbridge;

pub use processor::DynamicProcessor;
pub use shape::{Member, ScalarKind, TypeRegistry, TypeShape};

/// Builds the per-route [`SampleProcessor`], or `None` for byte pass-through.
///
/// `shapes` resolves a type name to its [`TypeShape`]. When a route requests a
/// filter or transform, the input and output types must be present in `shapes`.
///
/// # Errors
/// [`crate::error::RoutingError`] when a requested filter/transform cannot be
/// built (unknown type, bad filter expression, invalid rule).
pub fn build_processor_with(
    route: &Route,
    in_type: &str,
    out_type: &str,
    shapes: &TypeRegistry,
) -> Result<Option<Box<dyn SampleProcessor>>> {
    if route.filter.is_none() && route.transform.is_none() {
        return Ok(None);
    }
    let proc = DynamicProcessor::build(route, in_type, out_type, shapes)?;
    Ok(Some(Box::new(proc)))
}
