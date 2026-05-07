// DDS-RPC 1.0 §7.11.2 / §7.10 — generic Replier interface.
package org.zerodds.rpc;

/**
 * Server-side endpoint for a DDS-RPC service. Generated Replier classes
 * hold a {@code Replier<Object,Object>} and route incoming requests to
 * a {@code <Service>Service} handler implementation.
 *
 * @param <TIn>  input payload type
 * @param <TOut> output payload type
 */
public interface Replier<TIn, TOut> extends AutoCloseable {
    /** Sends a reply for a previously received request. */
    void sendReply(TOut payload, ReplyHeader header);

    /** Returns the next pending request, or {@code null} if none is available. */
    Sample<TIn> takeRequest();

    /** Releases native resources. */
    @Override
    void close();

    /** Carrier for an incoming request — payload + header. */
    final class Sample<T> {
        public final T payload;
        public final RequestHeader header;

        public Sample(T payload, RequestHeader header) {
            this.payload = payload;
            this.header = header;
        }
    }
}
