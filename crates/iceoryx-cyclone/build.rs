// Links the iceoryx C binding only when the `posh` FFI is compiled in (Linux).
// The libiceoryx_binding_c shared object pulls its iceoryx POSH / hoofs /
// platform dependencies via DT_NEEDED; we add libstdc++ for the C++ runtime.
fn main() {
    let posh = std::env::var("CARGO_FEATURE_POSH").is_ok();
    let linux = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux");
    if posh && linux {
        // Allow an override for non-standard install prefixes.
        if let Ok(dir) = std::env::var("ICEORYX_LIB_DIR") {
            println!("cargo:rustc-link-search=native={dir}");
        }
        println!("cargo:rustc-link-search=native=/usr/lib/x86_64-linux-gnu");
        println!("cargo:rustc-link-lib=dylib=iceoryx_binding_c");
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }
}
