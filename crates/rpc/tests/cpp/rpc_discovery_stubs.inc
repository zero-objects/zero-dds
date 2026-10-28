// The C API symbols of the DDS-RPC discovery extension (§7.6.2) that
// `dds/rpc/zerodds.hpp` reaches. A probe that does not exercise discovery
// still has to define them, and defines them as an endpoint that has
// announced nothing and matched nothing.
int zerodds_writer_set_rpc_discovery(zerodds_ZeroDdsWriter *, const char *, const uint8_t *,
                                     const char *const *, uintptr_t) {
    return 0;
}
int zerodds_reader_set_rpc_discovery(zerodds_ZeroDdsReader *, const char *, const uint8_t *,
                                     const char *const *, uintptr_t) {
    return 0;
}
int zerodds_writer_matched_rpc_readers(zerodds_ZeroDdsWriter *, zerodds_ZeroDdsRpcPeer *,
                                       uintptr_t, uintptr_t *count) {
    *count = 0;
    return 0;
}
int zerodds_reader_matched_rpc_writers(zerodds_ZeroDdsReader *, zerodds_ZeroDdsRpcPeer *,
                                       uintptr_t, uintptr_t *count) {
    *count = 0;
    return 0;
}
int zerodds_rpc_service_matching_complete(zerodds_ZeroDdsWriter *, zerodds_ZeroDdsReader *,
                                          const char *, uint8_t, uint8_t *complete) {
    *complete = 0;
    return 0;
}
int zerodds_rpc_client_matching_complete(zerodds_ZeroDdsReader *, zerodds_ZeroDdsWriter *,
                                         const uint8_t *, uint8_t, uint8_t *complete) {
    *complete = 0;
    return 0;
}
int zerodds_reader_guid(zerodds_ZeroDdsReader *, uint8_t *out_guid) {
    std::memset(out_guid, 0x6b, 16);
    return 0;
}
