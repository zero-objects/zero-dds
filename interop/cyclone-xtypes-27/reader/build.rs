// SPDX-License-Identifier: Apache-2.0
// The #27 matrix runs the same `Robot` under two extensibilities. The
// retired `zerodds-idlc generate` used to rewrite `src/robot.rs` per
// case; the type is now generated here, and `run_matrix.sh` picks the
// case through `ZERODDS_ROBOT_EXTENSIBILITY` (`appendable`, the XTypes
// default, or `final`, which is what `--cyclone` used to mean).
//
// `dds_type_impls` writes the `DdsType` impl from the same model the
// codec came from. It replaces the bridge that used to stand beside this
// file by hand, and it is what puts the type's `TypeObject` into the
// endpoint: `Publisher::create_datawriter` and
// `Subscriber::create_datareader` register `DdsType::type_object()` with
// the participant's TypeLookup server, and the SEDP announcement builds
// `PID_TYPE_INFORMATION` out of that registration (XTypes 1.3 §7.6.3.2,
// issue #42). A hand-written bridge that left `type_object()` at its
// default `None` announced a topic type name and no type at all, which is
// precisely what the matrix is here to measure.
//
// The extensibility no longer needs a second file either: the generated
// impl states the extensibility of the model it was built from, so the
// codec and the announced `EXTENSIBILITY` cannot disagree.
fn main() -> Result<(), zerodds_build::Error> {
    println!("cargo:rerun-if-env-changed=ZERODDS_ROBOT_EXTENSIBILITY");
    let ext = std::env::var("ZERODDS_ROBOT_EXTENSIBILITY").unwrap_or_else(|_| "appendable".into());
    let chosen = match ext.as_str() {
        "appendable" => zerodds_build::DefaultExt::Appendable,
        "final" => zerodds_build::DefaultExt::Final,
        other => {
            // A misspelled case must not silently fall back to a wire
            // form the matrix then mislabels.
            panic!("ZERODDS_ROBOT_EXTENSIBILITY is `{other}`; it takes `appendable` or `final`");
        }
    };
    zerodds_build::Config::new()
        .include_dir("..")
        .default_extensibility(chosen)
        .dds_type_impls(true)
        .compile(&["../robot.idl"])
}
