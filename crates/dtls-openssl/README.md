# zerodds-dtls-openssl

DTLS 1.2 (RFC 6347) over UDP on OpenSSL 3.2 or later, as RFC 7252 §9 asks
for CoAP: pre-shared keys (RFC 4279), raw public keys (RFC 7250) checked
against names of RFC 6920 (`ni:`, `nih:`, binary), and X.509 certificates
checked against trust anchors and the peer's name. The suites RFC 7252
makes mandatory, TLS_PSK_WITH_AES_128_CCM_8 and
TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8, are offered first; CCM and GCM follow
for peers whose OpenSSL leaves CCM-8 out.

The server runs one UDP socket, a thread per session, the cookie exchange
of RFC 6347 §4.2.1 and at most 64 handshakes at a time. Links the system
libssl; the feature `vendored` builds OpenSSL from source.

Used by `zerodds-coap-bridge` (feature `dtls`).
