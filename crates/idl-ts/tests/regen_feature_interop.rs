//! L3 feature-interop regenerator (run explicitly, not part of the default
//! suite): emits the TS PSM for `features.idl` straight from
//! `generate_ts_source`, so the cross-PSM gen file stays in lockstep with this
//! crate's codegen WITHOUT depending on the `zerodds-idlc` binary (which links
//! sibling backends owned by other agents).
//!
//!   cargo test -p zerodds-idl-ts --test regen_feature_interop -- --ignored
//!
//! Writes `_interop/ts/gen/30_features.ts`.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use std::path::Path;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_ts::generate_ts_source;

#[test]
#[ignore = "regenerator: writes into zerodds-examples, run explicitly"]
fn regenerate_features_gen() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let idl_path = here
        .join("../../zerodds-examples/idl-conformance/_interop/features.idl")
        .canonicalize()
        .expect("features.idl path");
    let gen_path =
        here.join("../../zerodds-examples/idl-conformance/_interop/ts/gen/30_features.ts");

    let idl = std::fs::read_to_string(&idl_path).expect("read features.idl");
    let ast = zerodds_idl::parse(&idl, &ParserConfig::default()).expect("parse features.idl");
    let ts = generate_ts_source(&ast).expect("generate TS for features.idl");

    std::fs::write(&gen_path, ts).expect("write 30_features.ts");
    #[allow(clippy::print_stderr)]
    {
        eprintln!("wrote {}", gen_path.display());
    }
}
