# IETF WebSocket RFC 6455 — Open + Partial Items

— keine offenen Items.

§6.1 Send-Algorithm + §6.2 Receive-Algorithm sind in
`crates/websocket-bridge/src/message.rs` implementiert
(`fragment_message`, `Reassembler`, `Message`).

§5.3 Masking-Key-Provider ist in `masking.rs::MaskingKeyProvider`
als Trait ausgewiesen mit `InsecureSplitmixProvider` (Default) und
`ClosureMaskingKeyProvider` fuer caller-injected secure RNGs.
