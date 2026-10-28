//! Native TypeLookup writer, standard generated Java reader and deliberate loss.
#![cfg(feature = "std")]

#[test]
#[ignore = "requires the installed Java TypeLookup consumer and canonical fixture directory"]
fn native_large_reply_repairs_lost_fragment() -> Result<(), Box<dyn std::error::Error>> {
    use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
    use zerodds_rtps::wire_types::GuidPrefix;
    let directory = std::env::var("TYPELOOKUP_FIXTURE_DIR")?;
    let classpath = std::env::var("TYPELOOKUP_JAVA_CLASSPATH")?;
    let domain_lease = zerodds_test_domains::lease_domain();
    let domain = domain_lease.id();
    let runtime = DcpsRuntime::start(
        domain,
        GuidPrefix::from_bytes([0x42; 12]),
        RuntimeConfig::default(),
    )?;
    let mut count = 0;
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "bin") {
            let object =
                zerodds_types::type_object::TypeObject::from_bytes_le(&std::fs::read(path)?)?;
            runtime.register_type_object(object)?;
            count += 1;
        }
    }
    assert!(count >= 66, "Root dependency closure was not registered");
    let output = std::process::Command::new("java")
        .args([
            "-Xmx256m",
            "-cp",
            &classpath,
            "TypeLookupProcess",
            "loss",
            &domain.to_string(),
            "424242424242424242424242",
            &runtime.user_locator().port.to_string(),
        ])
        .output();
    runtime.shutdown();
    let output = output?;
    assert!(
        output.status.success(),
        "Java loss peer failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("NACK_FRAG_REPAIRED_1,ROOT_EXACT"));
    Ok(())
}
