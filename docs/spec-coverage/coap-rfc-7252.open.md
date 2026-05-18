# CoAP RFC 7252 — Open + Partial Items

— keine offenen Items.

§2.3 Caching/Intermediaries, §5.6 Cache-State, §5.7 Proxying und
§10 HTTP-Cross-Proto-Mapping sind in
`crates/coap-bridge/src/caching_proxy.rs` implementiert
(`CoapCache`, `ProxyConfig`, `http_status_to_coap`,
`http_method_to_coap`).

§8 Multicast in `crates/coap-bridge/src/multicast.rs`;
§9 DTLS in `crates/coap-bridge/src/dtls.rs`.

§11-§13 IANA/Security/Acks sind als `n/a (informative)`
gefuehrt — IANA-Tables sind in `option::numbers` +
`message::CoapCode` reflektiert.

## Decision-Records (`n/a (rejected)`)

— keine.
