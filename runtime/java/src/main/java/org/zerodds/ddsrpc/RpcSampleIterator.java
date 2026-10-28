// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import java.util.Collections;
import java.util.List;
import java.util.ListIterator;
import org.omg.dds.rpc.Sample;

/**
 * The {@code Sample.Iterator} the requester and the replier return — DDS-RPC 1.0 §7.11.2.2.7:
 * "The Sample.Iterator interface is the same as in [DDS-Java-PSM]", so this is an
 * {@code org.omg.dds.sub.Sample.Iterator}, a {@code java.util.ListIterator} over the samples
 * that is {@code java.io.Closeable} ([DDS-Java-PSM] §7.7.2). Its elements are the
 * {@link Sample rpc.Sample}s the RPC layer made, with their identities; {@link #next()} and
 * {@link #previous()} return them as such.
 *
 * <p>The samples of a {@code read} or {@code take} may be a loan from the reader, and
 * {@link #close()} returns it. The samples here are copies the RPC layer owns, so returning
 * the loan releases them: a closed iterator has no next and no previous element, a call that
 * would move it throws {@link IllegalStateException}, and closing again does nothing. The
 * samples are a fixed list, so {@code add}, {@code set} and {@code remove} are the optional
 * operations a {@code ListIterator} may refuse.
 *
 * @param <T> the topic type
 */
public final class RpcSampleIterator<T> implements org.omg.dds.sub.Sample.Iterator<T> {

    private final ListIterator<Sample<T>> walk;
    private boolean closed = false;

    public RpcSampleIterator(List<Sample<T>> samples) {
        this.walk = Collections.unmodifiableList(samples).listIterator();
    }

    private void open() {
        if (closed) {
            throw new IllegalStateException("the loan was returned by close()");
        }
    }

    /** A sample this iterator would hold: an {@code rpc.Sample}, or none. */
    private static <T> Sample<T> own(org.omg.dds.sub.Sample<T> sample) {
        if (sample != null && !(sample instanceof Sample)) {
            throw new IllegalArgumentException("an RPC sample iterator holds rpc.Sample only");
        }
        return (Sample<T>) sample;
    }

    @Override
    public boolean hasNext() {
        return !closed && walk.hasNext();
    }

    @Override
    public Sample<T> next() {
        open();
        return walk.next();
    }

    @Override
    public boolean hasPrevious() {
        return !closed && walk.hasPrevious();
    }

    @Override
    public Sample<T> previous() {
        open();
        return walk.previous();
    }

    @Override
    public int nextIndex() {
        open();
        return walk.nextIndex();
    }

    @Override
    public int previousIndex() {
        open();
        return walk.previousIndex();
    }

    @Override
    public void remove() {
        open();
        walk.remove();
    }

    @Override
    public void set(org.omg.dds.sub.Sample<T> sample) {
        open();
        walk.set(own(sample));
    }

    @Override
    public void add(org.omg.dds.sub.Sample<T> sample) {
        open();
        walk.add(own(sample));
    }

    /**
     * Return the loan. Closing twice is allowed and does nothing the second time; it throws no
     * checked exception.
     */
    @Override
    public void close() {
        closed = true;
    }

    /** Return the loan: the same as {@link #close()}. */
    public void returnLoan() {
        close();
    }

    /** Whether the loan was returned. */
    public boolean isClosed() {
        return closed;
    }
}
