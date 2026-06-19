# IETF WebSocket RFC 6455 — Open + Partial Items

— no open items.

§6.1 send algorithm + §6.2 receive algorithm are implemented in
`crates/websocket-bridge/src/message.rs`
(`fragment_message`, `Reassembler`, `Message`).

§5.3 masking-key provider is exposed in `masking.rs::MaskingKeyProvider`
as a trait with `InsecureSplitmixProvider` (default) and
`ClosureMaskingKeyProvider` for caller-injected secure RNGs.
