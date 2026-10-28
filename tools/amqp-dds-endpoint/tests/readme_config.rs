#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! The configuration the README shows is one the loader reads: it used to
//! show `<listen_uri>` under a `<dds_amqp_endpoint>` root, which the loader
//! refused (the root) or would have dropped (the element).

use zerodds_amqp_endpoint::annex_a::SaslMechanism;

#[test]
fn the_readme_configuration_reads_as_it_says() {
    let readme = include_str!("../README.md");
    let start = readme.find("```xml\n").expect("an xml block") + "```xml\n".len();
    let end = start + readme[start..].find("```").expect("its end");
    let cfg = zerodds_amqp_endpoint::config_xml::parse_config(&readme[start..end]).unwrap();
    let ep = &cfg.endpoints[0];
    assert_eq!(ep.listen_uri, "amqp://0.0.0.0:5671");
    assert_eq!(ep.limits.max_frame_size, 65_536);
    assert!(ep.tls.enabled && !ep.tls.require_client_cert);
    assert_eq!(
        ep.sasl.enabled_mechanisms,
        [SaslMechanism::SaslScramSha256, SaslMechanism::SaslPlain]
    );
    assert_eq!(
        ep.sasl.credential_store_uri,
        "file:///etc/zerodds/amqp-users"
    );
}
